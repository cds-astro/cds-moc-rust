
use std::{
  str,
  fs::File,
  error::Error,
  path::PathBuf,
  io::{self, BufWriter}
};

use clap::Parser;

use moclib::{
  idx::Idx,
  qty::{MocQty, Hpx},
  moc::{
    RangeMOCIterator, CellMOCIterator,
    range::{
      RangeRefMocIter,
      op::convert::convert_to_u64
    }
  },
  deser::{
    fits::{self, ranges_to_fits_ivoa},
    json::to_json_aladin,
    ascii::to_ascii_ivoa,
  }
};

use crate::{StatusFlag, MocSetFileReader};

#[derive(Debug, Parser)]
/// Extracts a MOC from the given moc-set.
pub struct Extract {
  #[clap(parse(from_os_str))]
  /// The moc-set to be read.
  file: PathBuf,
  /// Identifier of the MOC to be extracted
  id: u64,
  #[clap(subcommand)]
  /// Export format
  output: OutputFormat,
}

impl Extract {

  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let moc_set_reader = MocSetFileReader::new(self.file)?;
    let meta_it = moc_set_reader.meta().into_iter();
    let bytes_it = moc_set_reader.index().into_iter();
    for (flg_depth_id, byte_range) in meta_it.zip(bytes_it) {
      let id = flg_depth_id.identifier();
      let status = flg_depth_id.status();
      if id == self.id && (status == StatusFlag::Valid || status == StatusFlag::Deprecated) {
        let depth = flg_depth_id.depth();
        if depth <= Hpx::<u32>::MAX_DEPTH {
          let borrowed_ranges = moc_set_reader.ranges::<u32>(byte_range);
          let it = RangeRefMocIter::<u32, Hpx<u32>>::from_borrowed_ranges_unsafe(depth, borrowed_ranges);
          return self.output.write_smoc_possibly_converting_to_u64(it);
        } else {
          let borrowed_ranges = moc_set_reader.ranges::<u64>(byte_range);
          let it= RangeRefMocIter::<u64, Hpx<u64>>::from_borrowed_ranges_unsafe(depth, borrowed_ranges);
          return self.output.write_smoc_possibly_converting_to_u64(it);
        }
      } 
    }
    Ok(())
  }
}

#[derive(Clone, Debug, Parser)]
pub enum OutputFormat {
  #[clap(name = "ascii")]
  /// Output an ASCII MOC (VO compatible)
  Ascii {
    #[clap(short = 'w', long = "fold")]
    /// Width of a cheep fold formatting
    fold: Option<usize>,
    #[clap(short = 'l', long = "range-len")]
    /// Use range len instead of range end (not VO compatible)
    range_len: bool,
    /// Path of the output file (stdout if empty)
    opt_file: Option<PathBuf>,
  },
  #[clap(name = "json")]
  /// Output a JSON MOC (Aladin compatible)
  Json {
    #[clap(short = 'w', long = "--fold")]
    /// Width of a cheep fold formatting
    fold: Option<usize>,
    /// Path of the output file (stdout if empty)
    opt_file: Option<PathBuf>,
  },
  #[clap(name = "fits")]
  /// Output a FITS MOC (VO compatible)
  Fits {
    #[clap(short = 'f', long = "force-u64")]
    /// Force indices to be stored on u64 (ignored after operations involving 2 MOCs)
    force_u64: bool,
    #[clap(short = 'i', long = "moc-id")]
    /// MOC ID to be written in the FITS header
    moc_id: Option<String>,
    #[clap(short = 'y', long = "moc-type")]
    /// MOC Type to be written in the FITS header (IMAGE or CATALOG)
    moc_type: Option<fits::keywords::MocType>,
    /// Path of the output file
    file: PathBuf
  },
}

impl OutputFormat {
  
  pub fn is_fits_forced_to_u64(&self) -> bool {
    matches!(self, OutputFormat::Fits { force_u64: true, .. })
  }
  
  pub fn write_smoc_possibly_converting_to_u64<T: Idx, I>(self, it: I) -> Result<(), Box<dyn Error>>
    where
      I: RangeMOCIterator<T, Qty=Hpx<T>>
  {
    if self.is_fits_forced_to_u64() {
      self.write_moc(convert_to_u64::<T, Hpx<T>, _, Hpx<u64>>(it))
    } else {
      self.write_moc(it)
    }
  }
  
  pub fn write_moc<T, Q, I>(self, it: I) -> Result<(), Box<dyn Error>>
    where
      T: Idx,
      Q: MocQty<T>,
      I: RangeMOCIterator<T, Qty=Q>
  {
    match self {
      OutputFormat::Ascii { fold, range_len, opt_file: None } => {
        let stdout = io::stdout();
        to_ascii_ivoa(it.cells().cellranges(), &fold, range_len, stdout.lock()).map_err(|e| e.into())
      },
      OutputFormat::Ascii { fold, range_len, opt_file: Some(path) } => {
        let file = File::create(path)?;
        to_ascii_ivoa(it.cells().cellranges(), &fold, range_len, BufWriter::new(file)).map_err(|e| e.into())
      },
      OutputFormat::Json { fold, opt_file: None } => {
        let stdout = io::stdout();
        to_json_aladin(it.cells(), &fold, "", stdout.lock()).map_err(|e| e.into())
      },
      OutputFormat::Json { fold, opt_file: Some(path) } => {
        let file = File::create(path)?;
        to_json_aladin(it.cells(), &fold, "", BufWriter::new(file)).map_err(|e| e.into())
      },
      OutputFormat::Fits { force_u64: _, moc_id, moc_type, file } => {
        let file = File::create(file)?;
        let writer = BufWriter::new(file);
        ranges_to_fits_ivoa(it, moc_id, moc_type, writer).map_err(|e| e.into())
      }
    }
  }
}
