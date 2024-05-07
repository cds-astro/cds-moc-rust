use std::{
  error::Error,
  fs::File,
  io::{self, BufRead, BufReader, Write},
  ops::Range,
  path::PathBuf,
  str::FromStr,
};

use structopt::StructOpt;

use moclib::{
  deser::{
    ascii::{from_ascii_ivoa, from_ascii_stream},
    fits::{from_fits_ivoa, MocIdxType, MocQtyType, MocType as RMocType},
    json::from_json_aladin,
  },
  idx::Idx,
  moc::{
    CellMOCIntoIterator, CellMOCIterator, CellOrCellRangeMOCIntoIterator,
    CellOrCellRangeMOCIterator, RangeMOCIterator,
  },
  qty::{Frequency, Time},
};

use super::{input::InputFormat, N_MICROSEC_IN_DAY};

#[derive(Debug)]
pub enum MocType {
  TMOC,
  FMOC,
}
impl FromStr for MocType {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "tmoc" => Ok(MocType::TMOC),
      "fmoc" => Ok(MocType::FMOC),
      _ => Err(format!(
        "Unrecognized moc type. Actual: '{}'. Expected: 'tmoc' or 'fmoc'",
        s
      )),
    }
  }
}

#[derive(StructOpt, Debug)]
pub struct HumanPrint {
  #[structopt(parse(from_os_str))]
  /// Path of the input MOC file (or stdin if equals "-")
  input: PathBuf,
  #[structopt(short = "t", long = "type")]
  /// Input MOC type ('tmoc' or 'fmoc') required for 'ascii', 'json' ans 'stream' inputs; ignored for 'fits'
  moc_type: Option<MocType>,
  #[structopt(short = "f", long = "format")]
  /// Format of the input MOC ('ascii', 'json', 'fits' or 'stream') [default: guess from the file extension]
  input_fmt: Option<InputFormat>,
  #[structopt(short, long)]
  /// Do not print header lines
  no_header: bool,
}

impl HumanPrint {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let path = self.input;
    if path == PathBuf::from("-") {
      if let Some(input_fmt) = self.input_fmt {
        let stdin = std::io::stdin();
        exec(stdin.lock(), input_fmt, self.moc_type, !self.no_header)
      } else {
        Err(
          String::from(
            "Using stdin, the MOC format ('ascii', 'json', ...) must be provided, see options.",
          )
          .into(),
        )
      }
    } else {
      let input_fmt = match self.input_fmt {
        Some(input_fmt) => Ok(input_fmt),
        None => InputFormat::from_extension(&path),
      }?;
      let f = File::open(path)?;
      exec(BufReader::new(f), input_fmt, self.moc_type, !self.no_header)
    }
  }
}

pub fn exec<R: BufRead>(
  mut input: R,
  input_fmt: InputFormat,
  moc_type: Option<MocType>,
  print_header: bool,
) -> Result<(), Box<dyn Error>> {
  match (moc_type, input_fmt) {
    // TMOC
    (Some(MocType::TMOC), InputFormat::Ascii) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cellcellranges = from_ascii_ivoa::<u64, Time<u64>>(&input_str)?;
      print_tmoc(
        print_header,
        cellcellranges.into_cellcellrange_moc_iter().ranges(),
      )
    }
    (Some(MocType::TMOC), InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cells = from_json_aladin::<u64, Time<u64>>(&input_str)?;
      print_tmoc(print_header, cells.into_cell_moc_iter().ranges())
    }
    (Some(MocType::TMOC), InputFormat::Stream) => {
      let cellrange_it = from_ascii_stream::<u64, Time<u64>, _>(input)?;
      print_tmoc(print_header, cellrange_it.ranges())
    }
    // FMOC
    (Some(MocType::FMOC), InputFormat::Ascii) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cellcellranges = from_ascii_ivoa::<u64, Frequency<u64>>(&input_str)?;
      print_fmoc(
        print_header,
        cellcellranges.into_cellcellrange_moc_iter().ranges(),
      )
    }
    (Some(MocType::FMOC), InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cells = from_json_aladin::<u64, Frequency<u64>>(&input_str)?;
      print_fmoc(print_header, cells.into_cell_moc_iter().ranges())
    }
    (Some(MocType::FMOC), InputFormat::Stream) => {
      let cellrange_it = from_ascii_stream::<u64, Frequency<u64>, _>(input)?;
      print_fmoc(print_header, cellrange_it.ranges())
    }
    // FITS file (SMOC or TMOC or FMOC, or ST-MOC)
    (_, InputFormat::Fits) => {
      let fits_res = from_fits_ivoa(input)?;
      match fits_res {
        MocIdxType::U16(moc) => match moc {
          MocQtyType::Time(moc) => match moc {
            RMocType::Ranges(moc) => print_tmoc(print_header, moc),
            RMocType::Cells(moc) => print_tmoc(print_header, moc.into_cell_moc_iter().ranges()),
          },
          MocQtyType::Freq(moc) => match moc {
            RMocType::Ranges(moc) => print_fmoc(print_header, moc),
            RMocType::Cells(moc) => print_fmoc(print_header, moc.into_cell_moc_iter().ranges()),
          },
          _ => Err(String::from("Only Time or Frequency MOC supported for human printing").into()),
        },
        MocIdxType::U32(moc) => match moc {
          MocQtyType::Time(moc) => match moc {
            RMocType::Ranges(moc) => print_tmoc(print_header, moc),
            RMocType::Cells(moc) => print_tmoc(print_header, moc.into_cell_moc_iter().ranges()),
          },
          MocQtyType::Freq(moc) => match moc {
            RMocType::Ranges(moc) => print_fmoc(print_header, moc),
            RMocType::Cells(moc) => print_fmoc(print_header, moc.into_cell_moc_iter().ranges()),
          },
          _ => Err(String::from("Only Time or Frequency MOC supported for human printing").into()),
        },
        MocIdxType::U64(moc) => match moc {
          MocQtyType::Time(moc) => match moc {
            RMocType::Ranges(moc) => print_tmoc(print_header, moc),
            RMocType::Cells(moc) => print_tmoc(print_header, moc.into_cell_moc_iter().ranges()),
          },
          MocQtyType::Freq(moc) => match moc {
            RMocType::Ranges(moc) => print_fmoc(print_header, moc),
            RMocType::Cells(moc) => print_fmoc(print_header, moc.into_cell_moc_iter().ranges()),
          },
          _ => Err(String::from("Only Time or Frequency MOC supported for human printing").into()),
        },
      }
    }
    // MOC type required
    _ => Err(String::from("Input MOC type must be specified.").into()),
  }
}

pub fn print_tmoc<T, I>(print_header: bool, it: I) -> Result<(), Box<dyn Error>>
where
  T: Idx,
  I: RangeMOCIterator<T, Qty = Time<T>>,
{
  const N_MICROSEC_IN_DAY_U64: u64 = 86400000000;
  let stdout = io::stdout();
  let mut buff = stdout.lock();
  if print_header {
    writeln!(&mut buff, "# Time ranges in JD")?;
    writeln!(&mut buff, "from_inclusive,to_exclusive")?;
  }
  for Range { start, end } in it {
    let start = start.to_u64_idx();
    let end = end.to_u64_idx();
    writeln!(
      &mut buff,
      "{}.{},{}.{}",
      start / N_MICROSEC_IN_DAY_U64,
      (((start % N_MICROSEC_IN_DAY_U64) as f64 / N_MICROSEC_IN_DAY) * 1e+17) as u64,
      end / N_MICROSEC_IN_DAY_U64,
      (((end % N_MICROSEC_IN_DAY_U64) as f64 / N_MICROSEC_IN_DAY) * 1e+17) as u64,
    )?;
  }
  Ok(())
}

pub fn print_fmoc<T, I>(print_header: bool, it: I) -> Result<(), Box<dyn Error>>
where
  T: Idx,
  I: RangeMOCIterator<T, Qty = Frequency<T>>,
{
  let stdout = io::stdout();
  let mut buff = stdout.lock();
  if print_header {
    writeln!(&mut buff, "# Frequency ranges in Hz")?;
    writeln!(&mut buff, "from_inclusive,to_exclusive")?;
  }
  for Range { start, end } in it {
    writeln!(
      &mut buff,
      "{:e},{:e}",
      Frequency::<T>::hash2freq(start),
      Frequency::<T>::hash2freq(end)
    )?;
  }
  Ok(())
}
