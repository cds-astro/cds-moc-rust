use std::{
  error::Error,
  fs::File,
  io::stdout,
  io::{BufReader, Read, Write},
  ops::Range,
  path::PathBuf,
};

use clap::Parser;

use moclib::{
  deser::{
    ascii::{from_ascii_ivoa, to_ascii_ivoa},
    fits::{MocIdxType, MocQtyType, STMocType, error::FitsError, from_fits_ivoa},
  },
  moc::{
    CellMOCIterator, CellOrCellRangeMOCIntoIterator, CellOrCellRangeMOCIterator,
    RangeMOCIntoIterator, RangeMOCIterator,
    range::{RangeMOC, op::convert::convert_from_u64},
  },
  moc2d::{RangeMOC2ElemIt, RangeMOC2Iterator},
  qty::{Hpx, MocQty, Time},
  ranges::{BorrowedRanges, Ranges, SNORanges},
};

use crate::{
  list::{write_leaves, write_leaves_with_smoc},
  rtree::{LeafEntry, STMocPPFile},
  time::TimeType,
};

#[derive(Debug, Parser)]
/// Query a ST-MOC PP file.
pub struct Query {
  #[clap(value_name = "FILE")]
  /// The stmoc-pp to be queried.
  file: PathBuf,
  #[clap(short = 't')]
  /// Add columns containing time range intersection
  print_time: bool,
  #[structopt(short = 'o', long = "output-time-fmt", default_value = "jd")]
  /// Output time format: 'jd' (julian date), 'mjd' (modified julian date), 'usec' (microsec since JD=0),
  /// 'isorfc' (Gregorian date-time, Rfc3339, WARNING: no conversion to TCB),
  /// or 'isosimple' (Gregorian date, 'YYYY-MM-DDTHH:MM:SS' WARNING: no conversion to TCB)
  time_fmt: TimeType,
  #[clap(short = 's')]
  /// Add a column containing the inlined intersection S-MOCs
  print_smoc: bool,
  #[clap(long)]
  /// Use sequential scan instead of the RTree structure (debug)
  seq: bool,
  #[clap(subcommand)]
  /// Query mode.
  qtype: QueryType,
}

impl Query {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let stmpp = STMocPPFile::new(self.file)?;
    self.qtype.exec(
      false,
      stmpp,
      self.seq,
      self.print_time,
      self.time_fmt,
      self.print_smoc,
    )
  }
}

#[derive(Debug, Parser)]
pub enum QueryType {
  #[clap(name = "simple", allow_negative_numbers = true)]
  /// Query by single time range and S-MOC
  Simple {
    #[structopt(short = 'i', long = "input-time-fmt", default_value = "jd")]
    /// Input time format: 'jd', 'mjd', 'usec', 'isorfc' or 'isosimple'
    time_fmt: TimeType,
    /// Time range lower bound (inclusive)
    time_start: String,
    /// Time range upper bound (exclusive)
    time_end: String,
    /// Space MOC (ASCII). TIP: use double quotes
    smoc_ascii: String,
    #[structopt(short = 'm', long = "sky-fraction-min")]
    /// Minimal intersecting area (expressed as a sky fraction in `[0, 1]`) to select an entry.
    minimal_sky_fraction: Option<f64>,
  },
  #[clap(name = "stmoc")]
  /// Query by ST-MOC
  STMoc {
    #[clap(value_name = "FILE")]
    /// Path of the input ST-MOC FITS file
    path: PathBuf,
    #[structopt(short = 'm', long = "sky-fraction-min")]
    /// Minimal intersecting area (expressed as a sky fraction in `[0, 1]`) to select an entry.
    minimal_sky_fraction: Option<f64>,
  },
}

impl QueryType {
  pub fn exec(
    self,
    print_cgi: bool,
    stmpp: STMocPPFile,
    sequential: bool,
    print_time: bool,
    output_time_fmt: TimeType,
    print_smoc: bool,
  ) -> Result<(), Box<dyn Error>> {
    let reader = stmpp.get_reader();
    let mut writer = stdout().lock();
    match self {
      Self::Simple {
        time_fmt,
        time_start,
        time_end,
        smoc_ascii,
        minimal_sky_fraction: None,
      } => {
        let smoc_u64 = from_ascii_ivoa::<u64, Hpx<u64>>(smoc_ascii.as_str())?
          .into_cellcellrange_moc_iter()
          .ranges()
          .into_range_moc();
        let smoc_u32 = to_u32_possibly_degrading(&smoc_u64);
        let smoc_u32_ranges = &smoc_u32.moc_ranges().0;
        let smoc_u64_ranges = &smoc_u64.moc_ranges().0;
        let tstart = time_fmt.parse(time_start.as_str())?;
        let tend = time_fmt.parse(time_end.as_str())?;
        let time_range = tstart..tend;
        if sequential {
          write_result(
            writer,
            reader.smoc_filter(
              reader.sequential_query(&time_range),
              smoc_u32_ranges,
              smoc_u64_ranges,
            ),
            &time_range,
            smoc_u32_ranges,
            smoc_u64_ranges,
            print_cgi,
            true,
            print_time,
            &output_time_fmt,
            print_smoc,
          )
        } else {
          write_result(
            writer,
            reader.smoc_filter(
              reader.rtree_query(&time_range),
              smoc_u32_ranges,
              smoc_u64_ranges,
            ),
            &time_range,
            smoc_u32_ranges,
            smoc_u64_ranges,
            print_cgi,
            true,
            print_time,
            &output_time_fmt,
            print_smoc,
          )
        }?;
      }
      Self::Simple {
        time_fmt,
        time_start,
        time_end,
        smoc_ascii,
        minimal_sky_fraction: Some(minimum_sky_fraction),
      } => {
        let smoc_u64 = from_ascii_ivoa::<u64, Hpx<u64>>(smoc_ascii.as_str())?
          .into_cellcellrange_moc_iter()
          .ranges()
          .into_range_moc();
        let smoc_u32 = to_u32_possibly_degrading(&smoc_u64);
        let smoc_u32_depth = smoc_u32.depth_max();
        let smoc_u32_ranges = &smoc_u32.moc_ranges().0;
        let smoc_u64_depth = smoc_u64.depth_max();
        let smoc_u64_ranges = &smoc_u64.moc_ranges().0;
        let tstart = time_fmt.parse(time_start.as_str())?;
        let tend = time_fmt.parse(time_end.as_str())?;
        let time_range = tstart..tend;
        if sequential {
          write_result(
            writer,
            reader.smoc_filter_with_minimum_area(
              reader.sequential_query(&time_range),
              smoc_u32_depth,
              smoc_u32_ranges,
              smoc_u64_depth,
              smoc_u64_ranges,
              minimum_sky_fraction,
            ),
            &time_range,
            smoc_u32_ranges,
            smoc_u64_ranges,
            print_cgi,
            true,
            print_time,
            &output_time_fmt,
            print_smoc,
          )
        } else {
          write_result(
            writer,
            reader.smoc_filter_with_minimum_area(
              reader.rtree_query(&time_range),
              smoc_u32_depth,
              smoc_u32_ranges,
              smoc_u64_depth,
              smoc_u64_ranges,
              minimum_sky_fraction,
            ),
            &time_range,
            smoc_u32_ranges,
            smoc_u64_ranges,
            print_cgi,
            true,
            print_time,
            &output_time_fmt,
            print_smoc,
          )
        }?;
      }
      Self::STMoc {
        path,
        minimal_sky_fraction: None,
      } => {
        let mut print_header = true;
        for (tmoc, smoc_u64) in open_stmoc(path)? {
          let smoc_u32 = to_u32_possibly_degrading(&smoc_u64);
          let smoc_u32_ranges = &smoc_u32.moc_ranges().0;
          let smoc_u64_ranges = &smoc_u64.moc_ranges().0;
          if sequential {
            for time_range in tmoc.into_range_moc_iter() {
              write_result(
                &mut writer,
                reader.smoc_filter(
                  reader.sequential_query(&time_range),
                  smoc_u32_ranges,
                  smoc_u64_ranges,
                ),
                &time_range,
                smoc_u32_ranges,
                smoc_u64_ranges,
                print_cgi,
                print_header,
                print_time,
                &output_time_fmt,
                print_smoc,
              )?;
              print_header = false;
            }
          } else {
            for time_range in tmoc.into_range_moc_iter() {
              write_result(
                &mut writer,
                reader.smoc_filter(
                  reader.rtree_query(&time_range),
                  smoc_u32_ranges,
                  smoc_u64_ranges,
                ),
                &time_range,
                smoc_u32_ranges,
                smoc_u64_ranges,
                print_cgi,
                print_header,
                print_time,
                &output_time_fmt,
                print_smoc,
              )?;
              print_header = false;
            }
          }
        }
      }
      Self::STMoc {
        path,
        minimal_sky_fraction: Some(minimum_sky_fraction),
      } => {
        let mut print_header = true;
        let stmoc_it = open_stmoc(path)?;
        if print_cgi {
          writeln!(writer, "Content-Type: text/plain\n")?;
        }
        for (tmoc, smoc_u64) in stmoc_it {
          let smoc_u32 = to_u32_possibly_degrading(&smoc_u64);
          let smoc_u32_depth = smoc_u32.depth_max();
          let smoc_u32_ranges = &smoc_u32.moc_ranges().0;
          let smoc_u64_depth = smoc_u64.depth_max();
          let smoc_u64_ranges = &smoc_u64.moc_ranges().0;
          if sequential {
            for time_range in tmoc.into_range_moc_iter() {
              write_result(
                &mut writer,
                reader.smoc_filter_with_minimum_area(
                  reader.sequential_query(&time_range),
                  smoc_u32_depth,
                  smoc_u32_ranges,
                  smoc_u64_depth,
                  smoc_u64_ranges,
                  minimum_sky_fraction,
                ),
                &time_range,
                smoc_u32_ranges,
                smoc_u64_ranges,
                false,
                print_header,
                print_time,
                &output_time_fmt,
                print_smoc,
              )?;
              print_header = false;
            }
          } else {
            for time_range in tmoc.into_range_moc_iter() {
              write_result(
                &mut writer,
                reader.smoc_filter_with_minimum_area(
                  reader.rtree_query(&time_range),
                  smoc_u32_depth,
                  smoc_u32_ranges,
                  smoc_u64_depth,
                  smoc_u64_ranges,
                  minimum_sky_fraction,
                ),
                &time_range,
                smoc_u32_ranges,
                smoc_u64_ranges,
                false,
                print_header,
                print_time,
                &output_time_fmt,
                print_smoc,
              )?;
              print_header = false;
            }
          }
        }
      }
    };
    Ok(())
  }
}

pub fn open_stmoc(
  path: PathBuf,
) -> Result<
  Box<dyn Iterator<Item = (RangeMOC<u64, Time<u64>>, RangeMOC<u64, Hpx<u64>>)>>,
  Box<dyn Error>,
> {
  File::open(path)
    .map_err(|e| FitsError::Io(e).into())
    .map(BufReader::new)
    .and_then(load_stmoc)
}

pub fn load_stmoc<R: Read + 'static>(
  reader: BufReader<R>,
) -> Result<
  Box<dyn Iterator<Item = (RangeMOC<u64, Time<u64>>, RangeMOC<u64, Hpx<u64>>)>>,
  Box<dyn Error>,
> {
  match from_fits_ivoa(reader)? {
    MocIdxType::U64(moc) => match moc {
      MocQtyType::TimeHpx(moc) => match moc {
        STMocType::V2(moc) => Ok(Box::new(read_stmoc(moc))),
        STMocType::PreV2(moc) => Ok(Box::new(read_stmoc(moc))),
      },
      _ => Err(String::from("2D-TMOC dies not seem to be a ST-MOC!").into()),
    },
    _ => Err(String::from("ST-MOC data type different from u64!").into()),
  }
}

pub fn read_stmoc<I, J, K, L>(
  stmoc: L,
) -> impl Iterator<Item = (RangeMOC<u64, Time<u64>>, RangeMOC<u64, Hpx<u64>>)>
where
  I: RangeMOCIterator<u64, Qty = Time<u64>>,
  J: RangeMOCIterator<u64, Qty = Hpx<u64>>,
  K: RangeMOC2ElemIt<u64, Time<u64>, u64, Hpx<u64>, It1 = I, It2 = J>,
  L: RangeMOC2Iterator<u64, Time<u64>, I, u64, Hpx<u64>, J, K>,
{
  stmoc.map(|st_iters| {
    let (t_ranges_it, s_ranges_it) = st_iters.range_mocs_it();
    let tmoc = t_ranges_it.into_range_moc();
    let smoc = s_ranges_it.into_range_moc();
    (tmoc, smoc)
  })
}

pub fn write_result<'a, W, I>(
  mut writer: W,
  it: I,
  trange: &Range<u64>,
  smoc_u32: &Ranges<u32>,
  smoc_u64: &Ranges<u64>,
  print_cgi: bool,
  print_header: bool,
  print_time: bool,
  output_time_fmt: &TimeType,
  print_smoc: bool,
) -> Result<(), Box<dyn Error>>
where
  W: Write,
  I: Iterator<Item = (LeafEntry<'a>, &'a [u8])>,
{
  if print_cgi {
    writeln!(writer, "Content-Type: text/plain\n")?;
  }

  match ((print_time as u8) << 1) | (print_smoc as u8) {
    2 => {
      // print id + time intersection
      write_leaves(
        writer,
        print_header,
        output_time_fmt,
        it.map(move |(leaf, _)| {
          (
            leaf.id(),
            trange.start.max(leaf.tmin())..trange.end.min(leaf.tmax()),
          )
        }),
      )
    }
    3 => {
      // print id + time intersection + smoc
      write_leaves_with_smoc(
        writer,
        print_header,
        output_time_fmt,
        it.map(move |(leaf, smoc_bytes)| {
          let depth = leaf.smoc_depth();
          let mut ascii_buff = Vec::<u8>::with_capacity(100);
          if depth <= Hpx::<u32>::MAX_DEPTH {
            let ranges: BorrowedRanges<'_, u32> = BorrowedRanges::from(smoc_bytes);
            let inter = ranges.intersection(&smoc_u32.into());
            let smoc = RangeMOC::<u32, Hpx<u32>>::new(depth, inter.into());
            to_ascii_ivoa(
              smoc.into_range_moc_iter().cells().cellranges(),
              &None,
              false,
              &mut ascii_buff,
            )
            .unwrap();
          } else {
            let ranges: BorrowedRanges<'_, u64> = BorrowedRanges::from(smoc_bytes);
            let inter = ranges.intersection(&smoc_u64.into());
            let smoc = RangeMOC::<u64, Hpx<u64>>::new(depth, inter.into());
            to_ascii_ivoa(
              smoc.into_range_moc_iter().cells().cellranges(),
              &None,
              false,
              &mut ascii_buff,
            )
            .unwrap();
          };
          (
            leaf.id(),
            trange.start.max(leaf.tmin())..trange.end.min(leaf.tmax()),
            ascii_buff,
          )
        }),
      )
    }
    _ => {
      // print id only
      if print_header {
        writeln!(&mut writer, "id")?;
      }
      for (leaf, _) in it {
        writeln!(&mut writer, "{}", leaf.id())?;
      }
      Ok(())
    }
  }
}

pub fn to_u32_possibly_degrading(moc64: &RangeMOC<u64, Hpx<u64>>) -> RangeMOC<u32, Hpx<u32>> {
  if moc64.depth_max() <= Hpx::<u32>::MAX_DEPTH {
    convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>((&moc64).into_range_moc_iter()).into_range_moc()
  } else {
    let moc64tmp = moc64.degraded(Hpx::<u32>::MAX_DEPTH);
    convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>((&moc64tmp).into_range_moc_iter())
      .into_range_moc()
  }
}
