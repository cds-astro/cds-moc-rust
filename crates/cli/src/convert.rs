use std::{
  error::Error,
  fs::File,
  io::{BufRead, BufReader},
  path::PathBuf,
  str::FromStr,
};

use structopt::StructOpt;

use moclib::{
  deser::{
    ascii::{from_ascii_ivoa, from_ascii_stream, moc2d_from_ascii_ivoa},
    fits::{from_fits_ivoa, MocIdxType, MocQtyType, MocType as RMocType, STMocType},
    json::{cellmoc2d_from_json_aladin, from_json_aladin},
  },
  moc::{
    CellMOCIntoIterator, CellMOCIterator, CellOrCellRangeMOCIntoIterator,
    CellOrCellRangeMOCIterator,
  },
  moc2d::{CellMOC2IntoIterator, CellOrCellRangeMOC2IntoIterator, RangeMOC2IntoIterator},
  qty::{Frequency, Hpx, Time},
};

use super::{input::InputFormat, output::OutputFormat};

#[derive(Debug)]
pub enum MocType {
  SMOC,
  TMOC,
  FMOC,
  STMOC,
  SFMOC,
}
impl FromStr for MocType {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "moc" | "smoc" => Ok(MocType::SMOC),
      "tmoc" => Ok(MocType::TMOC),
      "fmoc" => Ok(MocType::FMOC),
      "stmoc" => Ok(MocType::STMOC),
      "sfmoc" => Ok(MocType::SFMOC),
      _ => Err(format!(
        "Unrecognized moc type. Actual: '{}'. Expected: 'moc (or smoc), 'tmoc', 'fmoc', 'stmoc' or 'sfmoc'",
        s
      )),
    }
  }
}

#[derive(StructOpt, Debug)]
pub struct Convert {
  #[structopt(parse(from_os_str))]
  /// Path of the input MOC file (or stdin if equals "-")
  input: PathBuf,
  #[structopt(short = "t", long = "type")]
  /// Input MOC type ('smoc', 'tmoc', 'fmoc' or 'stmoc') required for 'ascii', 'json' ans 'stream' inputs; ignored for 'fits'
  moc_type: Option<MocType>,
  #[structopt(short = "f", long = "format")]
  /// Format of the input MOC ('ascii', 'json', 'fits' or 'stream') [default: guess from the file extension]
  input_fmt: Option<InputFormat>,
  #[structopt(subcommand)]
  output: OutputFormat,
}

impl Convert {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    //let path = self.input.unwrap_or_else(|| PathBuf::from("-"));
    let path = self.input;
    if path == PathBuf::from("-") {
      if let Some(input_fmt) = self.input_fmt {
        let stdin = std::io::stdin();
        exec(stdin.lock(), input_fmt, self.moc_type, self.output)
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
      exec(BufReader::new(f), input_fmt, self.moc_type, self.output)
    }
  }
}

pub fn exec<R: BufRead>(
  mut input: R,
  input_fmt: InputFormat,
  moc_type: Option<MocType>,
  output: OutputFormat,
) -> Result<(), Box<dyn Error>> {
  match (moc_type, input_fmt) {
    // SMOC
    (Some(MocType::SMOC), InputFormat::Ascii) => {
      let mut input_str = Default::default();
      input.read_to_string(&mut input_str)?;
      let cellcellranges = from_ascii_ivoa::<u64, Hpx<u64>>(&input_str)?;
      output.write_smoc_possibly_auto_converting_from_u64(
        cellcellranges.into_cellcellrange_moc_iter().ranges(),
      )
    }
    (Some(MocType::SMOC), InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cells = from_json_aladin::<u64, Hpx<u64>>(&input_str)?;
      output.write_smoc_possibly_auto_converting_from_u64(cells.into_cell_moc_iter().ranges())
    }
    (Some(MocType::SMOC), InputFormat::Stream) => {
      let cellrange_it = from_ascii_stream::<u64, Hpx<u64>, _>(input)?;
      output.write_smoc_possibly_auto_converting_from_u64(cellrange_it.ranges())
    }
    // TMOC
    (Some(MocType::TMOC), InputFormat::Ascii) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cellcellranges = from_ascii_ivoa::<u64, Time<u64>>(&input_str)?;
      output.write_tmoc_possibly_auto_converting_from_u64(
        cellcellranges.into_cellcellrange_moc_iter().ranges(),
      )
    }
    (Some(MocType::TMOC), InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cells = from_json_aladin::<u64, Time<u64>>(&input_str)?;
      output.write_tmoc_possibly_auto_converting_from_u64(cells.into_cell_moc_iter().ranges())
    }
    (Some(MocType::TMOC), InputFormat::Stream) => {
      let cellrange_it = from_ascii_stream::<u64, Time<u64>, _>(input)?;
      output.write_tmoc_possibly_auto_converting_from_u64(cellrange_it.ranges())
    }
    // FMOC
    (Some(MocType::FMOC), InputFormat::Ascii) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cellcellranges = from_ascii_ivoa::<u64, Frequency<u64>>(&input_str)?;
      output.write_fmoc_possibly_auto_converting_from_u64(
        cellcellranges.into_cellcellrange_moc_iter().ranges(),
      )
    }
    (Some(MocType::FMOC), InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cells = from_json_aladin::<u64, Frequency<u64>>(&input_str)?;
      output.write_fmoc_possibly_auto_converting_from_u64(cells.into_cell_moc_iter().ranges())
    }
    (Some(MocType::FMOC), InputFormat::Stream) => {
      let cellrange_it = from_ascii_stream::<u64, Frequency<u64>, _>(input)?;
      output.write_fmoc_possibly_auto_converting_from_u64(cellrange_it.ranges())
    }
    // ST-MOC
    (Some(MocType::STMOC), InputFormat::Ascii) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cellrange2 = moc2d_from_ascii_ivoa::<u64, Time<u64>, u64, Hpx<u64>>(&input_str)?;
      output.write_stmoc(
        cellrange2
          .into_cellcellrange_moc2_iter()
          .into_range_moc2_iter(),
      )
    }
    (Some(MocType::STMOC), InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cell2 = cellmoc2d_from_json_aladin::<u64, Time<u64>, u64, Hpx<u64>>(&input_str)?;
      output.write_stmoc(cell2.into_cell_moc2_iter().into_range_moc2_iter())
    }
    (Some(MocType::STMOC), InputFormat::Stream) => {
      Err(String::from("No stream format for ST-MOCs yet.").into())
    }
    // SF-MOC
    (Some(MocType::SFMOC), InputFormat::Ascii) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cellrange2 = moc2d_from_ascii_ivoa::<u64, Frequency<u64>, u64, Hpx<u64>>(&input_str)?;
      output.write_sfmoc(
        cellrange2
          .into_cellcellrange_moc2_iter()
          .into_range_moc2_iter(),
      )
    }
    (Some(MocType::SFMOC), InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cell2 = cellmoc2d_from_json_aladin::<u64, Frequency<u64>, u64, Hpx<u64>>(&input_str)?;
      output.write_sfmoc(cell2.into_cell_moc2_iter().into_range_moc2_iter())
    }
    (Some(MocType::SFMOC), InputFormat::Stream) => {
      Err(String::from("No stream format for SF-MOCs yet.").into())
    }
    // FITS file (SMOC or TMOC or FMOC, or ST-MOC)
    (_, InputFormat::Fits) => {
      let fits_res = from_fits_ivoa(input)?;
      match fits_res {
        MocIdxType::U16(moc) => match moc {
          MocQtyType::Hpx(moc) => match moc {
            RMocType::Ranges(moc) => output.write_smoc_possibly_converting_to_u64(moc),
            RMocType::Cells(moc) => {
              output.write_smoc_possibly_converting_to_u64(moc.into_cell_moc_iter().ranges())
            }
          },
          MocQtyType::Time(moc) => match moc {
            RMocType::Ranges(moc) => output.write_tmoc_possibly_converting_to_u64(moc),
            RMocType::Cells(moc) => {
              output.write_tmoc_possibly_converting_to_u64(moc.into_cell_moc_iter().ranges())
            }
          },
          MocQtyType::Freq(moc) => match moc {
            RMocType::Ranges(moc) => output.write_fmoc_possibly_converting_to_u64(moc),
            RMocType::Cells(moc) => {
              output.write_fmoc_possibly_converting_to_u64(moc.into_cell_moc_iter().ranges())
            }
          },
          MocQtyType::TimeHpx(moc) => match moc {
            STMocType::V2(moc) => output.write_stmoc(moc),
            STMocType::PreV2(moc) => output.write_stmoc(moc),
          },
          MocQtyType::FreqHpx(moc) => output.write_sfmoc(moc),
        },
        MocIdxType::U32(moc) => match moc {
          MocQtyType::Hpx(moc) => match moc {
            RMocType::Ranges(moc) => output.write_smoc_possibly_converting_to_u64(moc),
            RMocType::Cells(moc) => {
              output.write_smoc_possibly_converting_to_u64(moc.into_cell_moc_iter().ranges())
            }
          },
          MocQtyType::Time(moc) => match moc {
            RMocType::Ranges(moc) => output.write_tmoc_possibly_converting_to_u64(moc),
            RMocType::Cells(moc) => {
              output.write_tmoc_possibly_converting_to_u64(moc.into_cell_moc_iter().ranges())
            }
          },
          MocQtyType::Freq(moc) => match moc {
            RMocType::Ranges(moc) => output.write_fmoc_possibly_converting_to_u64(moc),
            RMocType::Cells(moc) => {
              output.write_fmoc_possibly_converting_to_u64(moc.into_cell_moc_iter().ranges())
            }
          },
          MocQtyType::TimeHpx(moc) => match moc {
            STMocType::V2(moc) => output.write_stmoc(moc),
            STMocType::PreV2(moc) => output.write_stmoc(moc),
          },
          MocQtyType::FreqHpx(moc) => output.write_sfmoc(moc),
        },
        MocIdxType::U64(moc) => match moc {
          MocQtyType::Hpx(moc) => match moc {
            RMocType::Ranges(moc) => output.write_smoc_possibly_converting_to_u64(moc),
            RMocType::Cells(moc) => {
              output.write_smoc_possibly_converting_to_u64(moc.into_cell_moc_iter().ranges())
            }
          },
          MocQtyType::Time(moc) => match moc {
            RMocType::Ranges(moc) => output.write_tmoc_possibly_converting_to_u64(moc),
            RMocType::Cells(moc) => {
              output.write_tmoc_possibly_converting_to_u64(moc.into_cell_moc_iter().ranges())
            }
          },
          MocQtyType::Freq(moc) => match moc {
            RMocType::Ranges(moc) => output.write_fmoc_possibly_converting_to_u64(moc),
            RMocType::Cells(moc) => {
              output.write_fmoc_possibly_converting_to_u64(moc.into_cell_moc_iter().ranges())
            }
          },
          MocQtyType::TimeHpx(moc) => match moc {
            STMocType::V2(moc) => output.write_stmoc(moc),
            STMocType::PreV2(moc) => output.write_stmoc(moc),
          },
          MocQtyType::FreqHpx(moc) => output.write_sfmoc(moc),
        },
      }
    }
    // MOC type required
    _ => Err(String::from("Input MOC type must be specified.").into()),
  }
}

/*
IN THIS PREVIOUS CODE, WE LOADED DATA (FROM JSON OR ASCII) WITH A GIVEN DATATYPE u16, u32 or u64.
NOW, WE ALWAYS LOAD IN u64 AND CONVERT IF NECESSARY
impl Convert {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    //let path = self.input.unwrap_or_else(|| PathBuf::from("-"));
    let path = self.input;
    if path ==  PathBuf::from("-") {
      if let Some(input_fmt) = self.input_fmt {
        let stdin = std::io::stdin();
        match self.idx_type {
          DataType::U16 => exec::<u16, _>(stdin.lock(), input_fmt, self.moc_type, self.output),
          DataType::U32 => exec::<u32, _>(stdin.lock(), input_fmt, self.moc_type, self.output),
          DataType::U64 => exec::<u64, _>(stdin.lock(), input_fmt, self.moc_type, self.output),
        }
      } else {
        Err(String::from("Using stdin, the MOC format ('ascii', 'json', ...) must be provided, see options.").into())
      }
    } else {
      let input_fmt = match self.input_fmt {
        Some(input_fmt) => Ok(input_fmt),
        None => fmt_from_extension(&path),
      }?;
      let f = File::open(path)?;
      match self.idx_type {
        DataType::U16 => exec::<u16, _>(BufReader::new(f), input_fmt, self.moc_type, self.output),
        DataType::U32 => exec::<u32, _>(BufReader::new(f), input_fmt, self.moc_type, self.output),
        DataType::U64 => exec::<u64, _>(BufReader::new(f), input_fmt, self.moc_type, self.output),
      }
    }
  }
}

pub fn exec<T: Idx, R: BufRead>(
  mut input: R,
  input_fmt: InputFormat,
  moc_type: MocType,
  output: PathBuf)
  -> Result<(), Box<dyn Error>>
{
  let file = File::create(output)?;
  let writer = BufWriter::new(file);
  match (moc_type,  input_fmt) {
    // SMOC
    (MocType::SMOC, InputFormat::Ascii) => {
      let mut input_str = Default::default();
      input.read_to_string(&mut input_str)?;
      let cellcellranges = from_ascii_ivoa::<T, Hpx::<T>>(&input_str)?;
      ranges_to_fits_ivoa(cellcellranges.into_cellcellrange_moc_iter().ranges(), None, None, writer)
        .map_err(|e| e.into())
    },
    (MocType::SMOC, InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cells = from_json_aladin::<T, Hpx::<T>>(&input_str)?;
      ranges_to_fits_ivoa(cells.into_cell_moc_iter().ranges(), None, None, writer)
        .map_err(|e| e.into())
    },
    (MocType::SMOC, InputFormat::Fits) => {
      from_fits_ivoa(input)?.to_fits_ivoa(writer).map_err(|e| e.into())
    },
    (MocType::SMOC, InputFormat::Stream) => {
      let cellrange_it = from_ascii_stream::<T, Hpx::<T>, _>(input)?;
      ranges_to_fits_ivoa(cellrange_it.ranges(), None, None, writer)
        .map_err(|e| e.into())
    },

    // TMOC
    (MocType::TMOC, InputFormat::Ascii) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cellcellranges = from_ascii_ivoa::<T, Time::<T>>(&input_str)?;
      ranges_to_fits_ivoa(cellcellranges.into_cellcellrange_moc_iter().ranges(), None, None, writer)
        .map_err(|e| e.into())
    },
    (MocType::TMOC, InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cells = from_json_aladin::<T, Time::<T>>(&input_str)?;
      ranges_to_fits_ivoa(cells.into_cell_moc_iter().ranges(), None, None, writer)
        .map_err(|e| e.into())
    },
    (MocType::TMOC, InputFormat::Fits) => {
      from_fits_ivoa(input)?.to_fits_ivoa(writer).map_err(|e| e.into())
    },
    (MocType::TMOC, InputFormat::Stream) => {
      let cellrange_it = from_ascii_stream::<T, Time::<T>, _>(input)?;
      ranges_to_fits_ivoa(cellrange_it.ranges(), None, None, writer)
        .map_err(|e| e.into())
    },

    // ST-MOC
    (MocType::STMOC, InputFormat::Ascii) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cellrange2 = moc2d_from_ascii_ivoa::<T, Time::<T>, T, Hpx::<T>>(&input_str)?;
      ranges2d_to_fits_ivoa(cellrange2.into_cellcellrange_moc2_iter().into_range_moc2_iter(), None, None, writer)
        .map_err(|e| e.into())
    },
    (MocType::STMOC, InputFormat::Json) => {
      let mut input_str = String::new();
      input.read_to_string(&mut input_str)?;
      let cell2 = cellmoc2d_from_json_aladin::<T, Time::<T>, T, Hpx::<T>>(&input_str)?;
      ranges2d_to_fits_ivoa(cell2.into_cell_moc2_iter().into_range_moc2_iter(), None, None, writer)
        .map_err(|e| e.into())
    },
    (MocType::STMOC, InputFormat::Fits) => {
      from_fits_ivoa(input)?.to_fits_ivoa(writer).map_err(|e| e.into())
    },
    (MocType::STMOC, InputFormat::Stream) => {
      Err(String::from("No stream format for ST-MOCs yet.").into())
    },
  }
}
*/
