use std::{
  collections::HashSet,
  error::Error,
  fs::File,
  io::{BufRead, BufReader},
  num::ParseIntError,
  ops::Range,
  path::{Path, PathBuf},
  str::FromStr,
};

use clap::Parser;

use cdshealpix::{best_starting_depth, has_best_starting_depth, nested};

use moclib::{
  deser::{
    ascii::from_ascii_ivoa,
    fits::{from_fits_ivoa, MocIdxType, MocQtyType, MocType},
    json::from_json_aladin,
  },
  idx::Idx,
  moc::{
    builder::maxdepth_range::RangeMocBuilder,
    range::{
      op::convert::{convert_from_u64, convert_to_u64},
      RangeMOC,
    },
    CellMOCIntoIterator, CellMOCIterator, CellOrCellRangeMOCIntoIterator,
    CellOrCellRangeMOCIterator, RangeMOCIntoIterator, RangeMOCIterator,
  },
  qty::{Hpx, MocQty},
  ranges::{BorrowedRanges, Ranges, SNORanges},
};

use crate::{extract::OutputFormat, MocSetFileReader, StatusFlag};

const HALF_PI: f64 = 0.5 * std::f64::consts::PI;
const TWICE_PI: f64 = 2.0 * std::f64::consts::PI;

#[derive(Debug, Parser)]
/// Union of all MOCs in the moc-set matching a given region
pub struct Union {
  #[clap(value_name = "FILE")]
  /// The moc-set to be read.
  file: PathBuf,
  #[clap(short = 'd', long = "add-deprecated")]
  /// Also selects MOCs flagged as deprecated (ignored if identifiers are provided)
  include_deprecated: bool,
  /// Depth of the output MOC
  depth: u8,
  #[clap(subcommand)]
  /// Method used to select MOCs.
  method: Method,
}

#[derive(Debug, Clone)]
pub struct IdList(HashSet<u64>);

impl FromStr for IdList {
  type Err = ParseIntError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    s.split(',')
      .map(|id| id.parse::<u64>())
      .collect::<Result<HashSet<u64>, _>>()
      .map(IdList)
  }
}

#[derive(Debug, Parser)]
pub enum Method {
  #[clap(name = "ids", allow_negative_numbers = true)]
  /// Provided list of MOC IDs.
  IDs {
    #[clap(value_parser = clap::value_parser!(IdList))]
    /// Coma separated list of MOC IDs.
    ids: IdList,
    #[clap(subcommand)]
    /// Export format
    output: OutputFormat,
  },
  #[clap(name = "pos", allow_negative_numbers = true)]
  /// Single position.
  Pos {
    /// Longitude of the cone center (in degrees)
    lon_deg: f64,
    /// Latitude of the cone center (in degrees)
    lat_deg: f64,
    #[clap(subcommand)]
    /// Export format
    output: OutputFormat,
  },
  #[clap(name = "cone", allow_negative_numbers = true)]
  /// A cone, i.e. a position with a small area around (approximated by a MOC).
  Cone {
    /// Longitude of the cone center (in degrees)
    lon_deg: f64,
    /// Latitude of the cone center (in degrees)
    lat_deg: f64,
    /// Radius of the cone (in arcseconds)
    r_arcsec: f64,
    #[clap(short = 'p', long = "precision", default_value = "2")]
    /// MOC precision; 0: depth 'd' at which the cone is overlapped by 1 to max 9 cells; 1: depth 'd' + 1; n: depth 'd' + n.
    prec: u8,
    #[clap(short = 'i', long = "included")]
    /// Selects MOCs containing the whole cone MOC (instead of overlapping only)
    full: bool,
    #[clap(subcommand)]
    /// Export format
    output: OutputFormat,
  },
  #[clap(name = "moc")]
  /// The given MOC (you create a moc using moc-cli and pipe it into moc-set)
  Moc {
    #[clap(value_name = "FILE")]
    /// Path of the input MOC file (or stdin if equals "--")
    input: PathBuf,
    #[clap(short = 'f', long = "format")]
    /// Format of the input MOC ('ascii', 'json' or 'fits') [default: guess from the file extension]
    input_fmt: Option<InputFormat>,
    #[clap(short = 'i', long = "included")]
    /// Select MOCs containing the whole given MOC (instead of overlapping)
    full: bool,
    #[clap(subcommand)]
    /// Export format
    output: OutputFormat,
  },
}

#[derive(Debug, Clone)]
pub enum InputFormat {
  Ascii,
  Json,
  Fits,
}
impl FromStr for InputFormat {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "ascii" => Ok(InputFormat::Ascii),
      "json" => Ok(InputFormat::Json),
      "fits" => Ok(InputFormat::Fits),
      _ => Err(format!(
        "Unrecognized format '{}'. Expected: 'ascii, 'json' or 'fits'",
        s
      )),
    }
  }
}

/// Guess the file format from the extension.
pub fn fmt_from_extension(path: &Path) -> Result<InputFormat, String> {
  match path.extension().and_then(|e| e.to_str()) {
    Some("fits") => Ok(InputFormat::Fits),
    Some("json") => Ok(InputFormat::Json),
    Some("ascii") | Some("txt") => Ok(InputFormat::Ascii),
    _ => Err(String::from(
      "Unable to guess the MOC format from the file extension, see options.",
    )),
  }
}

impl Union {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    match self.method {
      Method::IDs { ids, output } => exec_ids(self.file, self.depth, ids.0, output),
      Method::Pos {
        lon_deg,
        lat_deg,
        output,
      } => {
        let lon = lon_deg2rad(lon_deg)?;
        let lat = lat_deg2rad(lat_deg)?;
        let idx64 = nested::hash(Hpx::<u64>::MAX_DEPTH, lon, lat);
        let idx32 = u32::from_u64_idx(idx64);
        exec_gen(
          self.file,
          self.include_deprecated,
          self.depth,
          output,
          move |ranges| ranges.contains_val(&idx32),
          move |ranges| ranges.contains_val(&idx64),
        )
      }
      Method::Cone {
        lon_deg,
        lat_deg,
        r_arcsec,
        prec,
        full,
        output,
      } => {
        let r_rad = (r_arcsec / 3600.0).to_radians();
        let depth = if !has_best_starting_depth(r_rad) {
          prec
        } else {
          (best_starting_depth(r_rad) + prec).min(Hpx::<u64>::MAX_DEPTH)
        };
        let lon = lon_deg2rad(lon_deg)?;
        let lat = lat_deg2rad(lat_deg)?;
        if r_rad <= 0.0 {
          Err(String::from("Radius must be positive").into())
        } else {
          let moc64: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_cone(lon, lat, r_rad, depth, 2);
          let moc32: RangeMOC<u32, Hpx<u32>> =
            convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>((&moc64).into_range_moc_iter())
              .into_range_moc();
          let moc32: Ranges<u32> = moc32.into_moc_ranges().into_ranges();
          let moc64: Ranges<u64> = moc64.into_moc_ranges().into_ranges();
          let moc32_ref = (&moc32).into();
          let moc64_ref = (&moc64).into();
          if full {
            exec_gen(
              self.file,
              self.include_deprecated,
              self.depth,
              output,
              move |ranges| ranges.contains(&moc32_ref),
              move |ranges| ranges.contains(&moc64_ref),
            )
          } else {
            exec_gen(
              self.file,
              self.include_deprecated,
              self.depth,
              output,
              move |ranges| ranges.intersects(&moc32_ref),
              move |ranges| ranges.intersects(&moc64_ref),
            )
          }
        }
      }
      Method::Moc {
        input,
        input_fmt,
        full,
        output,
      } => {
        let path = input;
        let (moc32, moc64) = if path == PathBuf::from("-") {
          if let Some(input_fmt) = input_fmt {
            let stdin = std::io::stdin();
            load_moc(stdin.lock(), input_fmt)
          } else {
            Err(
              String::from(
                "Using stdin, the MOC format ('ascii', 'json', ...) must be provided, see options.",
              )
              .into(),
            )
          }
        } else {
          let input_fmt = match input_fmt {
            Some(input_fmt) => Ok(input_fmt),
            None => fmt_from_extension(&path),
          }?;
          let f = File::open(path)?;
          load_moc(BufReader::new(f), input_fmt)
        }?;
        let moc32: Ranges<u32> = moc32.into_moc_ranges().into_ranges();
        let moc64: Ranges<u64> = moc64.into_moc_ranges().into_ranges();
        let moc32_ref = (&moc32).into();
        let moc64_ref = (&moc64).into();
        if full {
          exec_gen(
            self.file,
            self.include_deprecated,
            self.depth,
            output,
            move |ranges| ranges.contains(&moc32_ref),
            move |ranges| ranges.contains(&moc64_ref),
          )
        } else {
          exec_gen(
            self.file,
            self.include_deprecated,
            self.depth,
            output,
            move |ranges| ranges.intersects(&moc32_ref),
            move |ranges| ranges.intersects(&moc64_ref),
          )
        }
      }
    }
  }
}

type MocTuple = (RangeMOC<u32, Hpx<u32>>, RangeMOC<u64, Hpx<u64>>);

pub fn load_moc<R: BufRead>(
  mut input: R,
  input_fmt: InputFormat,
) -> Result<MocTuple, Box<dyn Error>> {
  match input_fmt {
    InputFormat::Ascii => {
      let mut input_str = Default::default();
      input.read_to_string(&mut input_str)?;
      let cellcellranges = from_ascii_ivoa::<u64, Hpx<u64>>(&input_str)?;
      let range_moc_u64: RangeMOC<u64, Hpx<u64>> = cellcellranges
        .into_cellcellrange_moc_iter()
        .ranges()
        .into_range_moc();
      let range_moc_u32: RangeMOC<u32, Hpx<u32>> =
        convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>((&range_moc_u64).into_range_moc_iter())
          .into_range_moc();
      Ok((range_moc_u32, range_moc_u64))
    }
    InputFormat::Json => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cells = from_json_aladin::<u64, Hpx<u64>>(&input_str)?;
      let range_moc_u64: RangeMOC<u64, Hpx<u64>> =
        cells.into_cell_moc_iter().ranges().into_range_moc();
      let range_moc_u32: RangeMOC<u32, Hpx<u32>> =
        convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>((&range_moc_u64).into_range_moc_iter())
          .into_range_moc();
      Ok((range_moc_u32, range_moc_u64))
    }
    InputFormat::Fits => {
      let fits_res = from_fits_ivoa(input)?;
      match fits_res {
        MocIdxType::U16(moc) => {
          let range_moc_u16: RangeMOC<u16, Hpx<u16>> = match moc {
            MocQtyType::Hpx(moc) => match moc {
              MocType::Ranges(moc) => Ok(moc.into_range_moc()),
              MocType::Cells(cells) => Ok(cells.into_cell_moc_iter().ranges().into_range_moc()),
            },
            _ => Err(String::from(
              "Unexpected type in FITS file MOC. Expected: MocQtyType::Hpx",
            )),
          }?;
          let range_moc_u64 =
            convert_to_u64::<u16, Hpx<u16>, _, Hpx<u64>>(range_moc_u16.into_range_moc_iter())
              .into_range_moc();
          let range_moc_u32 =
            convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>((&range_moc_u64).into_range_moc_iter())
              .into_range_moc();
          Ok((range_moc_u32, range_moc_u64))
        }
        MocIdxType::U32(moc) => {
          let range_moc_u32: RangeMOC<u32, Hpx<u32>> = match moc {
            MocQtyType::Hpx(moc) => match moc {
              MocType::Ranges(moc) => Ok(moc.into_range_moc()),
              MocType::Cells(cells) => Ok(cells.into_cell_moc_iter().ranges().into_range_moc()),
            },
            _ => Err(String::from(
              "Unexpected type in FITS file MOC. Expected: MocQtyType::Hpx",
            )),
          }?;
          let range_moc_u64 =
            convert_to_u64::<u32, Hpx<u32>, _, Hpx<u64>>((&range_moc_u32).into_range_moc_iter())
              .into_range_moc();
          Ok((range_moc_u32, range_moc_u64))
        }
        MocIdxType::U64(moc) => {
          let range_moc_u64: RangeMOC<u64, Hpx<u64>> = match moc {
            MocQtyType::Hpx(moc) => match moc {
              MocType::Ranges(moc) => Ok(moc.into_range_moc()),
              MocType::Cells(moc) => Ok(moc.into_cell_moc_iter().ranges().into_range_moc()),
            },
            _ => Err(String::from(
              "Unexpected type in FITS file MOC. Expected: MocQtyType::Hpx",
            )),
          }?;
          let range_moc_u32 =
            convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>((&range_moc_u64).into_range_moc_iter())
              .into_range_moc();
          Ok((range_moc_u32, range_moc_u64))
        }
      }
    }
  }
}

fn exec_ids(
  file: PathBuf,
  depth: u8,
  ids: HashSet<u64>,
  output: OutputFormat,
) -> Result<(), Box<dyn Error>> {
  let moc_set_reader = MocSetFileReader::new(file)?;
  let meta_it = moc_set_reader.meta().into_iter();
  let bytes_it = moc_set_reader.index().into_iter();

  let mut builder = RangeMocBuilder::<u64, Hpx<u64>>::new(depth, None);

  for (flg_depth_id, byte_range) in meta_it.zip(bytes_it) {
    let id = flg_depth_id.identifier();
    let status = flg_depth_id.status();
    if ids.contains(&id) && (status == StatusFlag::Valid || status == StatusFlag::Deprecated) {
      let depth = flg_depth_id.depth();
      if depth <= Hpx::<u32>::MAX_DEPTH {
        let ranges = moc_set_reader.ranges::<u32>(byte_range);
        for Range { start, end } in ranges.0.iter() {
          builder.push((*start).to_u64_idx()..(*end).to_u64_idx())
        }
      } else {
        let ranges = moc_set_reader.ranges::<u64>(byte_range);
        for Range { start, end } in ranges.0 {
          builder.push(*start..*end);
        }
      }
    }
  }
  let moc = builder.into_moc();
  output.write_moc(moc.into_range_moc_iter())
}

fn exec_gen<F, D>(
  file: PathBuf,
  include_deprecated: bool,
  depth: u8,
  output: OutputFormat,
  f: F,
  d: D,
) -> Result<(), Box<dyn Error>>
where
  F: Fn(&BorrowedRanges<'_, u32>) -> bool,
  D: Fn(&BorrowedRanges<'_, u64>) -> bool,
{
  let moc_set_reader = MocSetFileReader::new(file)?;
  let meta_it = moc_set_reader.meta().into_iter();
  let bytes_it = moc_set_reader.index().into_iter();

  let mut builder = RangeMocBuilder::<u64, Hpx<u64>>::new(depth, None);

  for (flg_depth_id, byte_range) in meta_it.zip(bytes_it) {
    let status = flg_depth_id.status();
    let depth = flg_depth_id.depth();
    if status == StatusFlag::Valid || (include_deprecated && status == StatusFlag::Deprecated) {
      if depth <= Hpx::<u32>::MAX_DEPTH {
        let ranges = moc_set_reader.ranges::<u32>(byte_range);
        if f(&ranges) {
          for Range { start, end } in ranges.0.iter() {
            builder.push((*start).to_u64_idx()..(*end).to_u64_idx())
          }
        }
      } else {
        let ranges = moc_set_reader.ranges::<u64>(byte_range);
        if d(&ranges) {
          for Range { start, end } in ranges.0 {
            builder.push(*start..*end);
          }
        }
      }
    }
  }
  let moc = builder.into_moc();
  output.write_moc(moc.into_range_moc_iter())
}

fn lon_deg2rad(lon_deg: f64) -> Result<f64, Box<dyn Error>> {
  let lon = lon_deg.to_radians();
  if !(0.0..TWICE_PI).contains(&lon) {
    Err(String::from("Longitude must be in [0, 2pi[").into())
  } else {
    Ok(lon)
  }
}

fn lat_deg2rad(lat_deg: f64) -> Result<f64, Box<dyn Error>> {
  let lat = lat_deg.to_radians();
  if !(-HALF_PI..HALF_PI).contains(&lat) {
    Err(String::from("Latitude must be in [-pi/2, pi/2]").into())
  } else {
    Ok(lat)
  }
}
