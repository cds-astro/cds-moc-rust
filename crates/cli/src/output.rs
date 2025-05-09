use std::{
  error::Error,
  fs::File,
  io::{self, BufWriter},
  path::PathBuf,
};

use structopt::StructOpt;

use moclib::{
  deser::{
    ascii::{moc2d_to_ascii_ivoa, to_ascii_ivoa, to_ascii_stream},
    fits::{self, ranges_sf_to_fits_ivoa, ranges_st_to_fits_ivoa, ranges_to_fits_ivoa},
    json::{cellmoc2d_to_json_aladin, to_json_aladin},
  },
  idx::Idx,
  moc::{
    range::op::convert::{convert_from_u64, convert_to_u64},
    CellHpxMOCIterator, CellMOCIterator, RangeMOCIterator,
  },
  moc2d::{
    CellMOC2IntoIterator, CellOrCellRangeMOC2IntoIterator, RangeMOC2ElemIt, RangeMOC2Iterator,
  },
  qty::{Frequency, Hpx, MocQty, Time},
};

#[derive(StructOpt, Clone, Debug)]
pub enum OutputFormat {
  #[structopt(name = "ascii")]
  /// Output an ASCII MOC (VO compatible)
  Ascii {
    #[structopt(short = "-w", long = "--fold")]
    /// Width of a cheep fold formatting
    fold: Option<usize>,
    #[structopt(short = "-l", long = "--range-len")]
    /// Use range len instead of range end (not VO compatible)
    range_len: bool,
    /// Path of the output file (stdout if empty)
    opt_file: Option<PathBuf>,
  },
  #[structopt(name = "json")]
  /// Output a JSON MOC (Aladin compatible)
  Json {
    #[structopt(short = "-w", long = "--fold")]
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
    #[structopt(short = "-p", long = "--force-v1")]
    /// Force compatibility with MOC v1.0 (i.e. save NUNIQ instead of Ranges; ignored if MOC is not a S-MOC)
    force_v1: bool,
    #[structopt(short = "-i", long = "--moc-id")]
    /// MOC ID to be written in the FITS header
    moc_id: Option<String>,
    #[structopt(short = "-y", long = "--moc-type")]
    /// MOC Type to be written in the FITS header (IMAGE or CATALOG)
    moc_type: Option<fits::keywords::MocType>,
    /// Path of the output file
    file: PathBuf,
  },
  #[structopt(name = "stream")]
  /// Output a streamed MOC (not yet implemented!)
  Stream,
}

impl OutputFormat {
  /// Clone this output format, providing a number to possibly change the name
  pub fn clone_with_number(&self, num: usize) -> Self {
    let mut new = self.clone();
    match &mut new {
      OutputFormat::Ascii {
        opt_file: Some(path),
        ..
      } => add_number_before_extension(num, path),
      OutputFormat::Json {
        opt_file: Some(path),
        ..
      } => add_number_before_extension(num, path),
      OutputFormat::Fits { file, .. } => add_number_before_extension(num, file),
      _ => {}
    };
    new
  }

  pub fn is_fits(&self) -> bool {
    matches!(self, OutputFormat::Fits { .. })
  }

  pub fn is_fits_forced_to_v1_std(&self) -> bool {
    matches!(self, OutputFormat::Fits { force_v1: true, .. })
  }

  pub fn is_fits_forced_to_u64(&self) -> bool {
    matches!(
      self,
      OutputFormat::Fits {
        force_u64: true,
        ..
      }
    )
  }

  pub fn is_fits_not_forced_to_u64(&self) -> bool {
    matches!(
      self,
      OutputFormat::Fits {
        force_u64: false,
        ..
      }
    )
  }

  pub fn write_smoc_possibly_auto_converting_from_u64<I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    I: RangeMOCIterator<u64, Qty = Hpx<u64>>,
  {
    let depth = it.depth_max();
    if self.is_fits_not_forced_to_u64() && depth <= Hpx::<u32>::MAX_DEPTH {
      if depth <= Hpx::<u16>::MAX_DEPTH {
        if self.is_fits_forced_to_v1_std() {
          self.write_smoc_fits_v1(convert_from_u64::<Hpx<u64>, u16, Hpx<u16>, _>(it))
        } else {
          self.write_moc(convert_from_u64::<Hpx<u64>, u16, Hpx<u16>, _>(it))
        }
      } else {
        assert!(depth <= Hpx::<u32>::MAX_DEPTH);
        if self.is_fits_forced_to_v1_std() {
          self.write_smoc_fits_v1(convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>(it))
        } else {
          self.write_moc(convert_from_u64::<Hpx<u64>, u32, Hpx<u32>, _>(it))
        }
      }
    } else if self.is_fits_forced_to_v1_std() {
      self.write_smoc_fits_v1(it)
    } else {
      self.write_moc(it)
    }
  }

  pub fn write_smoc_possibly_converting_to_u64<T: Idx, I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    I: RangeMOCIterator<T, Qty = Hpx<T>>,
  {
    if self.is_fits_forced_to_u64() {
      if self.is_fits_forced_to_v1_std() {
        self.write_smoc_fits_v1(convert_to_u64::<T, Hpx<T>, _, Hpx<u64>>(it))
      } else {
        self.write_moc(convert_to_u64::<T, Hpx<T>, _, Hpx<u64>>(it))
      }
    } else if self.is_fits_forced_to_v1_std() {
      self.write_smoc_fits_v1(it)
    } else {
      self.write_moc(it)
    }
  }

  pub fn write_smoc_from_cells_possibly_converting_to_u64<T: Idx, I>(
    self,
    it: I,
  ) -> Result<(), Box<dyn Error>>
  where
    I: CellMOCIterator<T, Qty = Hpx<T>>,
  {
    if self.is_fits_forced_to_u64() {
      if self.is_fits_forced_to_v1_std() {
        self.write_smoc_fits_v1_from_cells(it)
      } else {
        self.write_moc(convert_to_u64::<T, Hpx<T>, _, Hpx<u64>>(it.ranges()))
      }
    } else if self.is_fits_forced_to_v1_std() {
      self.write_smoc_fits_v1_from_cells(it)
    } else {
      self.write_moc_from_cells(it)
    }
  }

  pub fn write_tmoc_possibly_auto_converting_from_u64<I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    I: RangeMOCIterator<u64, Qty = Time<u64>>,
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
    I: RangeMOCIterator<T, Qty = Time<T>>,
  {
    if self.is_fits_forced_to_u64() {
      self.write_moc(convert_to_u64::<T, Time<T>, _, Time<u64>>(it))
    } else {
      self.write_moc(it)
    }
  }

  pub fn write_fmoc_possibly_auto_converting_from_u64<I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    I: RangeMOCIterator<u64, Qty = Frequency<u64>>,
  {
    if self.is_fits_not_forced_to_u64() {
      let depth = it.depth_max();
      if depth <= Time::<u16>::MAX_DEPTH {
        self.write_moc(convert_from_u64::<Frequency<u64>, u16, Frequency<u16>, _>(
          it,
        ))
      } else if depth <= Time::<u32>::MAX_DEPTH {
        self.write_moc(convert_from_u64::<Frequency<u64>, u32, Frequency<u32>, _>(
          it,
        ))
      } else {
        self.write_moc(it)
      }
    } else {
      self.write_moc(it)
    }
  }

  pub fn write_fmoc_possibly_converting_to_u64<T: Idx, I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    I: RangeMOCIterator<T, Qty = Frequency<T>>,
  {
    if self.is_fits_forced_to_u64() {
      self.write_moc(convert_to_u64::<T, Frequency<T>, _, Frequency<u64>>(it))
    } else {
      self.write_moc(it)
    }
  }

  pub fn write_moc<T, Q, I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    T: Idx,
    Q: MocQty<T>,
    I: RangeMOCIterator<T, Qty = Q>,
  {
    match self {
      OutputFormat::Ascii {
        fold,
        range_len,
        opt_file: None,
      } => {
        let stdout = io::stdout();
        to_ascii_ivoa(it.cells().cellranges(), &fold, range_len, stdout.lock())
          .map_err(|e| e.into())
      }
      OutputFormat::Ascii {
        fold,
        range_len,
        opt_file: Some(path),
      } => {
        let file = File::create(path)?;
        to_ascii_ivoa(
          it.cells().cellranges(),
          &fold,
          range_len,
          BufWriter::new(file),
        )
        .map_err(|e| e.into())
      }
      OutputFormat::Json {
        fold,
        opt_file: None,
      } => {
        let stdout = io::stdout();
        to_json_aladin(it.cells(), &fold, "", stdout.lock()).map_err(|e| e.into())
      }
      OutputFormat::Json {
        fold,
        opt_file: Some(path),
      } => {
        let file = File::create(path)?;
        to_json_aladin(it.cells(), &fold, "", BufWriter::new(file)).map_err(|e| e.into())
      }
      OutputFormat::Fits {
        force_u64: _,
        force_v1: _,
        moc_id,
        moc_type,
        file,
      } => {
        // Here I don't know how to convert the generic qty MocQty<T> into MocQty<u64>...
        let file = File::create(file)?;
        ranges_to_fits_ivoa(it, moc_id, moc_type, BufWriter::new(file)).map_err(|e| e.into())
      }
      OutputFormat::Stream => {
        let stdout = io::stdout();
        to_ascii_stream(it.cells().cellranges(), true, stdout.lock()).map_err(|e| e.into())
      }
    }
  }

  pub fn write_smoc_fits_v1<T, I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    T: Idx,
    I: RangeMOCIterator<T, Qty = Hpx<T>>,
  {
    self.write_smoc_fits_v1_from_cells(it.cells())
  }

  pub fn write_smoc_fits_v1_from_cells<T, I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    T: Idx,
    I: CellMOCIterator<T, Qty = Hpx<T>>,
  {
    match self {
      OutputFormat::Fits {
        force_u64: _,
        force_v1: _,
        moc_id,
        moc_type,
        file,
      } => {
        // Here I don't know how to convert the generic qty MocQty<T> into MocQty<u64>...
        let file = File::create(file)?;
        it.hpx_cells_to_fits_ivoa(moc_id, moc_type, BufWriter::new(file))
          .map_err(|e| e.into())
      }
      _ => unreachable!(),
    }
  }

  pub fn write_moc_from_cells<T, Q, I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    T: Idx,
    Q: MocQty<T>,
    I: CellMOCIterator<T, Qty = Q>,
  {
    match self {
      OutputFormat::Ascii {
        fold,
        range_len,
        opt_file: None,
      } => {
        let stdout = io::stdout();
        to_ascii_ivoa(it.cellranges(), &fold, range_len, stdout.lock()).map_err(|e| e.into())
      }
      OutputFormat::Ascii {
        fold,
        range_len,
        opt_file: Some(path),
      } => {
        let file = File::create(path)?;
        to_ascii_ivoa(it.cellranges(), &fold, range_len, BufWriter::new(file)).map_err(|e| e.into())
      }
      OutputFormat::Json {
        fold,
        opt_file: None,
      } => {
        let stdout = io::stdout();
        to_json_aladin(it, &fold, "", stdout.lock()).map_err(|e| e.into())
      }
      OutputFormat::Json {
        fold,
        opt_file: Some(path),
      } => {
        let file = File::create(path)?;
        to_json_aladin(it, &fold, "", BufWriter::new(file)).map_err(|e| e.into())
      }
      OutputFormat::Fits {
        force_u64: _,
        force_v1: _,
        moc_id,
        moc_type,
        file,
      } => {
        // Here I don't know how to convert the generic qty MocQty<T> into MocQty<u64>...
        let file = File::create(file)?;
        ranges_to_fits_ivoa(it.ranges(), moc_id, moc_type, BufWriter::new(file))
          .map_err(|e| e.into())
      }
      OutputFormat::Stream => {
        let stdout = io::stdout();
        to_ascii_stream(it.cellranges(), true, stdout.lock()).map_err(|e| e.into())
      }
    }
  }

  pub fn write_stmoc<T, I, J, K, L>(self, stmoc: L) -> Result<(), Box<dyn Error>>
  where
    T: Idx,
    I: RangeMOCIterator<T, Qty = Time<T>>,
    J: RangeMOCIterator<T, Qty = Hpx<T>>,
    K: RangeMOC2ElemIt<T, I::Qty, T, J::Qty, It1 = I, It2 = J>,
    L: RangeMOC2Iterator<T, I::Qty, I, T, J::Qty, J, K>,
  {
    // In case of ascii or json inputs, we perform useless conversions:
    //            cell -> range -> cell
    //   cellcellrange -> range -> cellcellrange
    // We could make 2 other `write_stmoc` methods (taking different iterators) to avoid this
    match self {
      OutputFormat::Ascii {
        fold,
        range_len,
        opt_file: None,
      } => {
        let stdout = io::stdout();
        moc2d_to_ascii_ivoa(
          stmoc.into_cellcellrange_moc2_iter(),
          &fold,
          range_len,
          stdout.lock(),
        )
        .map_err(|e| e.into())
      }
      OutputFormat::Ascii {
        fold,
        range_len,
        opt_file: Some(path),
      } => {
        let file = File::create(path)?;
        moc2d_to_ascii_ivoa(
          stmoc.into_cellcellrange_moc2_iter(),
          &fold,
          range_len,
          BufWriter::new(file),
        )
        .map_err(|e| e.into())
      }
      OutputFormat::Json {
        fold,
        opt_file: None,
      } => {
        let stdout = io::stdout();
        cellmoc2d_to_json_aladin(stmoc.into_cell_moc2_iter(), &fold, stdout.lock())
          .map_err(|e| e.into())
      }
      OutputFormat::Json {
        fold,
        opt_file: Some(path),
      } => {
        let file = File::create(path)?;
        cellmoc2d_to_json_aladin(stmoc.into_cell_moc2_iter(), &fold, BufWriter::new(file))
          .map_err(|e| e.into())
      }
      OutputFormat::Fits {
        force_u64: _,
        force_v1: _,
        moc_id,
        moc_type,
        file,
      } => {
        // TODO handle the forced to u64??
        let file = File::create(file)?;
        ranges_st_to_fits_ivoa(stmoc, moc_id, moc_type, BufWriter::new(file)).map_err(|e| e.into())
      }
      OutputFormat::Stream => {
        // let stdout = io::stdout();
        Err(String::from("No stream format for ST-MOCs yet.").into())
      }
    }
  }

  pub fn write_sfmoc<T, I, J, K, L>(self, sfmoc: L) -> Result<(), Box<dyn Error>>
  where
    T: Idx,
    I: RangeMOCIterator<T, Qty = Frequency<T>>,
    J: RangeMOCIterator<T, Qty = Hpx<T>>,
    K: RangeMOC2ElemIt<T, I::Qty, T, J::Qty, It1 = I, It2 = J>,
    L: RangeMOC2Iterator<T, I::Qty, I, T, J::Qty, J, K>,
  {
    // In case of ascii or json inputs, we perform useless conversions:
    //            cell -> range -> cell
    //   cellcellrange -> range -> cellcellrange
    // We could make 2 other `write_stmoc` methods (taking different iterators) to avoid this
    match self {
      OutputFormat::Ascii {
        fold,
        range_len,
        opt_file: None,
      } => {
        let stdout = io::stdout();
        moc2d_to_ascii_ivoa(
          sfmoc.into_cellcellrange_moc2_iter(),
          &fold,
          range_len,
          stdout.lock(),
        )
        .map_err(|e| e.into())
      }
      OutputFormat::Ascii {
        fold,
        range_len,
        opt_file: Some(path),
      } => {
        let file = File::create(path)?;
        moc2d_to_ascii_ivoa(
          sfmoc.into_cellcellrange_moc2_iter(),
          &fold,
          range_len,
          BufWriter::new(file),
        )
        .map_err(|e| e.into())
      }
      OutputFormat::Json {
        fold,
        opt_file: None,
      } => {
        let stdout = io::stdout();
        cellmoc2d_to_json_aladin(sfmoc.into_cell_moc2_iter(), &fold, stdout.lock())
          .map_err(|e| e.into())
      }
      OutputFormat::Json {
        fold,
        opt_file: Some(path),
      } => {
        let file = File::create(path)?;
        cellmoc2d_to_json_aladin(sfmoc.into_cell_moc2_iter(), &fold, BufWriter::new(file))
          .map_err(|e| e.into())
      }
      OutputFormat::Fits {
        force_u64: _,
        force_v1: _,
        moc_id,
        moc_type,
        file,
      } => {
        // TODO handle the forced to u64??
        let file = File::create(file)?;
        ranges_sf_to_fits_ivoa(sfmoc, moc_id, moc_type, BufWriter::new(file)).map_err(|e| e.into())
      }
      OutputFormat::Stream => {
        // let stdout = io::stdout();
        Err(String::from("No stream format for SF-MOCs yet.").into())
      }
    }
  }
}

fn add_number_before_extension(num: usize, path: &mut PathBuf) {
  match path.extension().and_then(|s| s.to_str()).map(String::from) {
    Some(ext) => path.set_extension(format!("{}.{}", num, ext)),
    None => path.set_extension(format!("{}", num)),
  };
}
