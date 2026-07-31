use std::{error::Error, io::Write, io::stdout, ops::Range, path::PathBuf};

use clap::Parser;

use moclib::{
  deser::ascii::to_ascii_ivoa,
  moc::{
    CellMOCIterator, RangeMOCIntoIterator, RangeMOCIterator, range::borrowed::BorrowedRangeMOC,
  },
  qty::{Hpx, MocQty},
  ranges::BorrowedRanges,
};

use crate::{
  rtree::{LeafEntry, STMocPPFile, STMocPPFileReader},
  time::TimeType,
};

#[derive(Debug, Parser)]
/// List all entries of a ST-MOC PP file.
pub struct List {
  #[clap(value_name = "FILE")]
  /// The stmoc-pp file to be queried.
  file: PathBuf,
  #[structopt(short, long = "time-fmt", default_value = "jd")]
  /// Output time format: 'jd' (julian date), 'mjd' (modified julian date), 'usec' (microsec since JD=0),
  /// 'isorfc' (Gregorian date-time, Rfc3339, WARNING: no conversion to TCB),
  /// or 'isosimple' (Gregorian date, 'YYYY-MM-DDTHH:MM:SS' WARNING: no conversion to TCB)
  time: TimeType,
  #[clap(subcommand)]
  /// List mode.
  mode: ListType,
}
impl List {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let stmpp = STMocPPFile::new(self.file)?;
    let reader = stmpp.get_reader();
    self.mode.exec(reader, self.time)
  }
}

#[derive(Debug, Parser)]
pub enum ListType {
  #[clap(name = "nodes")]
  /// List all RTree nodes
  Nodes,
  #[clap(name = "entries")]
  /// List all entries
  Entries {
    #[clap(short = 's', long)]
    /// Add a column containing the inlined S-MOC
    print_smoc: bool,
  },
}
impl ListType {
  pub fn exec(self, reader: STMocPPFileReader, time_fmt: TimeType) -> Result<(), Box<dyn Error>> {
    let mut writer = stdout().lock();
    match self {
      Self::Nodes => reader
        .print_nodes(&mut writer, &time_fmt)
        .map_err(|e| e.into()),
      Self::Entries { print_smoc } => {
        if !print_smoc {
          write_leaves(
            writer,
            true,
            &time_fmt,
            reader
              .leaves()
              .map(|leaf| (leaf.id(), leaf.tmin()..leaf.tmax())),
          )
        } else {
          write_leaves_with_smoc(
            writer,
            true,
            &time_fmt,
            add_ascii_smoc(reader.leaves(), &reader)
              .map(|(leaf, ascii)| (leaf.id(), leaf.tmin()..leaf.tmax(), ascii)),
          )
        }
      }
    }
  }
}

pub fn add_ascii_smoc<'a, I>(
  it: I,
  reader: &'a STMocPPFileReader,
) -> impl Iterator<Item = (LeafEntry<'a>, Vec<u8>)>
where
  I: Iterator<Item = LeafEntry<'a>>,
{
  it.map(move |leaf| {
    let depth = leaf.smoc_depth();
    let bstart = leaf.smoc_starting_byte_offset() as usize;
    let bend = bstart + leaf.smoc_byte_length() as usize;
    let smoc_bytes = reader.get_smoc_bytes(bstart..bend);
    let mut ascii_buff = Vec::<u8>::with_capacity(100);
    if depth <= Hpx::<u32>::MAX_DEPTH {
      let ranges: BorrowedRanges<'_, u32> = BorrowedRanges::from(smoc_bytes);
      let smoc = BorrowedRangeMOC::<u32, Hpx<u32>>::new(depth, ranges.into());
      to_ascii_ivoa(
        smoc.into_range_moc_iter().cells().cellranges(),
        &None,
        false,
        &mut ascii_buff,
      )
      .unwrap();
    } else {
      let ranges: BorrowedRanges<'_, u64> = BorrowedRanges::from(smoc_bytes);
      let smoc = BorrowedRangeMOC::<u64, Hpx<u64>>::new(depth, ranges.into());
      to_ascii_ivoa(
        smoc.into_range_moc_iter().cells().cellranges(),
        &None,
        false,
        &mut ascii_buff,
      )
      .unwrap();
    };
    (leaf, ascii_buff)
  })
}

/// # Params
/// The input iterator iterateson:
/// * leaf ID
/// * a Time Range
pub fn write_leaves<W, I>(
  mut writer: W,
  with_header: bool,
  time_fmt: &TimeType,
  it: I,
) -> Result<(), Box<dyn Error>>
where
  W: Write,
  I: Iterator<Item = (u32, Range<u64>)>,
{
  if with_header {
    writeln!(&mut writer, "id,tmin,tmax")?;
  }
  for (id, trange) in it {
    write!(&mut writer, "{}", id)
      // leaf.tmin()..leaf.tmax()
      .and_then(|()| time_fmt.write_time_range(&mut writer, trange))
      .and_then(|()| writeln!(&mut writer))?;
  }
  Ok(())
}

/// # Params
/// The input iterator iterateson:
/// * leaf ID
/// * a Time Range
/// * the row byte of the ASCII serialization of a S-MOC.
pub fn write_leaves_with_smoc<W, I>(
  mut writer: W,
  with_header: bool,
  time_fmt: &TimeType,
  it: I,
) -> Result<(), Box<dyn Error>>
where
  W: Write,
  I: Iterator<Item = (u32, Range<u64>, Vec<u8>)>,
{
  if with_header {
    writeln!(&mut writer, "id,tmin,tmax,smoc")?;
  }
  for (id, trange, ascii_bytes) in it {
    write!(&mut writer, "{}", id)
      .and_then(|()| time_fmt.write_time_range(&mut writer, trange))
      .and_then(|()| {
        writeln!(
          &mut writer,
          ",{}",
          String::from_utf8_lossy(ascii_bytes.as_slice())
        )
      })?;
  }
  Ok(())
}
