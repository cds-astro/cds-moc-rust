
use std::fs::File;
use std::io::{self, BufWriter};
use std::str;
use std::path::PathBuf;
use std::error::Error;
use structopt::StructOpt;

use moclib::idx::Idx;
use moclib::qty::{MocQty, Hpx, Time};
use moclib::deser::fits;
use moclib::moc::{
  RangeMOCIterator, CellMOCIterator,
  range::op::convert::{convert_to_u64, convert_from_u64}
};
use moclib::moc2d::{
  RangeMOC2Iterator,
  RangeMOC2ElemIt,
  CellMOC2IntoIterator,
  CellOrCellRangeMOC2IntoIterator,
};
use moclib::deser::{
  fits::{ranges_to_fits_ivoa, ranges2d_to_fits_ivoa},
  json::{to_json_aladin, cellmoc2d_to_json_aladin},
  ascii::{to_ascii_ivoa, to_ascii_stream, moc2d_to_ascii_ivoa},
};

#[derive(StructOpt, Debug)]
pub enum OutputFormat {
  #[structopt(name = "ascii")]
  /// Output an ASCII MOC (VO compatible)
  Ascii {
    #[structopt(short="-w", long = "--fold")]
    /// Width of a cheep fold formatting
    fold: Option<usize>,
    #[structopt(short="-l", long = "--range-len")]
    /// Use range len instead of range end (not VO compatible)
    range_len: bool,
    /// Path of the output file (stdout if empty)
    opt_file: Option<PathBuf>,
  },
  #[structopt(name = "json")]
  /// Output a JSON MOC (Aladin compatible)
  Json {
    #[structopt(short="-w", long = "--fold")]
    /// Width of a cheep fold formatting
    fold: Option<usize>,
    /// Path of the output file (stdout if empty)
    opt_file: Option<PathBuf>,
  },
  #[structopt(name = "fits")]
  /// Output a FITS MOC (VO compatible)
  Fits {
    #[structopt(short = "-f", long = "--force-u64")]
    /// Force indices to be stored on u64 (ignored after operations involving 2 MOCs)
    force_u64: bool,
    #[structopt(short="-i", long = "--moc-id")]
    /// MOC ID to be written in the FITS header
    moc_id: Option<String>,
    #[structopt(short="-y", long = "--moc-type")]
    /// MOC Type to be written in the FITS header (IMAGE or CATALOG)
    moc_type: Option<fits::keywords::MocType>,
    /// Path of the output file
    file: PathBuf
  },
  #[structopt(name = "stream")]
  /// Output a streamed MOC
  Stream,
}

impl OutputFormat {

  pub fn is_fits(&self) -> bool {
    matches!(self, OutputFormat::Fits { .. })
  }

  pub fn is_fits_forced_to_u64(&self) -> bool {
    matches!(self, OutputFormat::Fits { force_u64: true, .. })
  }

  pub fn is_fits_not_forced_to_u64(&self) -> bool {
    matches!(self, OutputFormat::Fits { force_u64: false, .. })

  }

  pub fn write_smoc_possibly_auto_converting_from_u64<I>(self, it: I) -> Result<(), Box<dyn Error>>
    where
      I: RangeMOCIterator<u64, Qty=Hpx<u64>>
  {
    if self.is_fits_not_forced_to_u64() {
      let depth = it.depth_max();
      if depth <= Hpx::<u16>::MAX_DEPTH {
        self.write_moc(convert_from_u64::<Hpx<u64>, u16, Hpx<u16>, _>(it))
      } else if depth <= Hpx::<u32>::MAX_DEPTH {
        self.write_moc(convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>(it))
      } else {
        self.write_moc(it)
      }
    } else {
      self.write_moc(it)
    }
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
  
  pub fn write_tmoc_possibly_auto_converting_from_u64<I>(self, it: I) -> Result<(), Box<dyn Error>>
    where
      I: RangeMOCIterator<u64, Qty=Time<u64>>
  {
    if self.is_fits_not_forced_to_u64() {
      let depth = it.depth_max();
      if depth <= Time::<u16>::MAX_DEPTH {
        self.write_moc(convert_from_u64::<Time<u64>, u16, Time<u16>, _>(it))
      } else if depth <= Time::<u32>::MAX_DEPTH {
        self.write_moc(convert_from_u64::<Time<u64>, u32, Time<u32>, _>(it))
      } else {
        self.write_moc(it)
      }
    } else {
      self.write_moc(it)
    }
  }

  pub fn write_tmoc_possibly_converting_to_u64<T: Idx, I>(self, it: I) -> Result<(), Box<dyn Error>>
    where
      I: RangeMOCIterator<T, Qty=Time<T>>
  {
    if self.is_fits_forced_to_u64() {
      self.write_moc(convert_to_u64::<T, Time<T>, _, Time<u64>>(it))
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
        // Here I don't know how to convert the generic qty MocQty<T> into MocQty<u64>...
        let file = File::create(file)?;
        ranges_to_fits_ivoa(it, moc_id, moc_type, BufWriter::new(file)).map_err(|e| e.into())
      },
      OutputFormat::Stream => {
        let stdout = io::stdout();
        to_ascii_stream(it.cells().cellranges(), true, stdout.lock()).map_err(|e| e.into())
      },
    }
  }
  
  pub fn write_stmoc<T, I, J, K, L>(self, stmoc: L)
                           -> Result<(), Box<dyn Error>>
    where
      T: Idx, 
      I: RangeMOCIterator<T, Qty=Time::<T>>,
      J: RangeMOCIterator<T, Qty=Hpx::<T>>,
      K: RangeMOC2ElemIt<T, Time::<T>, T, Hpx::<T>, It1=I, It2=J>,
      L: RangeMOC2Iterator<
        T, Time::<T>, I,
        T, Hpx::<T>, J,
        K
      >
  {
    // In case of ascii or json inputs, we perform useless conversions:
    //            cell -> range -> cell
    //   cellcellrange -> range -> cellcellrange
    // We could make 2 other `write_stmoc` methods (taking different iterators) to avoid this
    match self {
      OutputFormat::Ascii { fold, range_len, opt_file: None } => {
        let stdout = io::stdout();
        moc2d_to_ascii_ivoa(stmoc.into_cellcellrange_moc2_iter(), &fold, range_len, stdout.lock()).map_err(|e| e.into())
      },
      OutputFormat::Ascii { fold, range_len, opt_file: Some(path) } => {
        let file = File::create(path)?;
        moc2d_to_ascii_ivoa(stmoc.into_cellcellrange_moc2_iter(), &fold, range_len, BufWriter::new(file)).map_err(|e| e.into())
      },
      OutputFormat::Json { fold, opt_file: None } => {
        let stdout = io::stdout();
        cellmoc2d_to_json_aladin(stmoc.into_cell_moc2_iter(), &fold, stdout.lock()).map_err(|e| e.into())
      },
      OutputFormat::Json { fold, opt_file: Some(path) } => {
        let file = File::create(path)?;
        cellmoc2d_to_json_aladin(stmoc.into_cell_moc2_iter(), &fold, BufWriter::new(file)).map_err(|e| e.into())
      },
      OutputFormat::Fits { force_u64: _, moc_id, moc_type, file } => {
        // TODO handle the forced to u64??
        let file = File::create(file)?;
        ranges2d_to_fits_ivoa(stmoc, moc_id, moc_type, BufWriter::new(file)).map_err(|e| e.into())
      },
      OutputFormat::Stream => {
        // let stdout = io::stdout();
        Err(String::from("No stream format for ST-MOCs yet.").into())
      },
    }
  }
  
}
