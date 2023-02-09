//! MOC storage, protected from concurrent access.
//! The purpose is to be used this common storage in MOCWasm, MOCSet and MOCGui 
//! to store MOC in memory on the Rust side.
//!
//! # Note
//! Internally we use a [slab](https://crates.io/crates/slab) with concurrent access protected
//! by a [RwLock](https://doc.rust-lang.org/std/sync/struct.RwLock.html).
//! We may have used [sharded-slab](https://crates.io/crates/sharded-slab) but the current version
//! (v0.11.4) is still experimental according to the README, and the development does not seem to be
//! very active.


use std::{
  fs::{self, File},
  ops::Range,
  path::Path,
  io::{Cursor, BufRead, BufReader},
};

#[cfg(not(target_arch = "wasm32"))]
use rayon::iter::{
  ParallelIterator, IndexedParallelIterator,
  IntoParallelIterator, IntoParallelRefMutIterator,
};

use crate::{
  idx::Idx,
  qty::{MocQty, Hpx, Time, Frequency},
  elem::valuedcell::valued_cells_to_moc_with_opt,
  elemset::range::HpxRanges,
  moc::{
    CellMOCIterator, CellMOCIntoIterator,
    RangeMOCIterator,
    CellOrCellRangeMOCIterator, CellOrCellRangeMOCIntoIterator,
    range::RangeMOC,
  },
  moc2d::{
    RangeMOC2Iterator, RangeMOC2IntoIterator,
    CellMOC2IntoIterator,
    CellOrCellRangeMOC2IntoIterator,
  },
  hpxranges2d::TimeSpaceMoc,
  deser::{
    ascii::{from_ascii_ivoa, moc2d_from_ascii_ivoa},
    json::{from_json_aladin, cellmoc2d_from_json_aladin},
    img::to_img_default,
    fits::{
      from_fits_ivoa, MocIdxType,
      multiordermap::from_fits_multiordermap,
      skymap::from_fits_skymap,
    },
  },
};

pub mod common;
mod load;
mod store;
mod op1;
mod op2;
mod opn;

use self::{
  common::{
    PI, HALF_PI, MocQType,
    InternalMoc, SMOC, TMOC, FMOC, STMOC,
    check_depth, lon_deg2rad, lat_deg2rad,
  },
  load::{
    from_fits_gen, from_fits_u64, 
    smoc_from_fits_gen, tmoc_from_fits_gen, fmoc_from_fits_gen,
    stmoc_from_fits_u64
  },
  op1::{
    Op1, Op1MultiRes,
    op1_count_split, op1_1st_axis_min, op1_1st_axis_max,
    op1_flatten_to_moc_depth, op1_flatten_to_depth,
    op1_moc_barycenter, op1_moc_largest_distance_from_coo_to_moc_vertices
  },
  op2::Op2,
  opn::OpN,
};


/// Number of microseconds in a 24h day.
const JD_TO_USEC: f64 = (24_u64 * 60 * 60 * 1_000_000) as f64;

static GLOBAL_STORE: U64MocStore = U64MocStore;

pub struct U64MocStore;

// TODO: add methods
// Filters
// * returning the list of MOCs intersecting/containing a MOC
//    - input: array of moc indices
//    - output: array of boolean
// * filter on st-moc: Input = iter of ((lon, lat), jd)
// * fill holes

impl U64MocStore {
  
  pub fn get_global_store() -> &'static Self { &GLOBAL_STORE }

  pub fn insert_smoc(&self, moc: SMOC) -> Result<usize, String> {
    store::add(moc)
  }

  pub fn insert_tmoc(&self, moc: TMOC) -> Result<usize, String> {
    store::add(moc)
  }

  pub fn insert_fmoc(&self, moc: FMOC) -> Result<usize, String> {
    store::add(moc)
  }

  pub fn insert_stmoc(&self, moc: STMOC) -> Result<usize, String> {
    store::add(moc)
  }

  pub fn new_empty_smoc(&self, depth: u8) -> Result<usize, String> {
   let moc = RangeMOC::<u64, Hpx::<u64>>::new_empty(depth);
    store::add(moc)
  }

  pub fn new_empty_tmoc(&self, depth: u8) -> Result<usize, String> {
    let moc = RangeMOC::<u64, Time::<u64>>::new_empty(depth);
    store::add(moc)
  }

  pub fn new_empty_fmoc(&self, depth: u8) -> Result<usize, String> {
    let moc = RangeMOC::<u64, Frequency::<u64>>::new_empty(depth);
    store::add(moc)
  }

  /*pub fn insert_stmoc(&self, moc: STMOC) -> Result<usize, String> {
    store::add(moc)
  }*/

  /// Remove from the store the MOC at the given index.
  pub fn drop(&self, index: usize) -> Result<(), String> {
    store::drop(index).map(|_| ())
  }

  pub fn drop_smoc(&self, index: usize) -> Result<SMOC, String> {
    store::drop(index)
      .and_then(|moc| match moc {
        InternalMoc::Space(moc) => Ok(moc),
        _ => Err(String::from("MOC at the given index is not a S-MOC"))
      })
  }
  
  pub fn drop_tmoc(&self, index: usize) -> Result<TMOC, String> {
    store::drop(index)
      .and_then(|moc| match moc {
        InternalMoc::Time(moc) => Ok(moc),
        _ => Err(String::from("MOC at the given index is not a T-MOC"))
      })
  }
  
  pub fn drop_fmoc(&self, index: usize) -> Result<FMOC, String> {
    store::drop(index)
      .and_then(|moc| match moc {
        InternalMoc::Frequency(moc) => Ok(moc),
        _ => Err(String::from("MOC at the given index is not a F-MOC"))
      })
  }
  
  pub fn drop_stmoc(&self, index: usize) -> Result<STMOC, String> {
    store::drop(index)
      .and_then(|moc| match moc {
        InternalMoc::TimeSpace(moc) => Ok(moc),
        _ => Err(String::from("MOC at the given index is not a ST-MOC"))
      })
  }

  pub fn get_1st_axis_min(&self, index: usize) -> Result<Option<u64>, String> {
    op1_1st_axis_min(index)
  }

  pub fn get_1st_axis_max(&self, index: usize) -> Result<Option<u64>, String> {
    op1_1st_axis_max(index)
  }

  //////////////////
  // Get MOC info //
  
  pub fn get_qty_type(&self, index: usize) -> Result<MocQType, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_qty_type)
  }
  
  pub fn get_smoc_depth(&self, index: usize) -> Result<u8, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_smoc_depth)
  }

  pub fn get_smoc_copy(&self, index: usize) -> Result<SMOC, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_smoc_copy)
  }

  pub fn get_tmoc_depth(&self, index: usize) -> Result<u8, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_tmoc_depth)
  }

  pub fn get_fmoc_depth(&self, index: usize) -> Result<u8, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_fmoc_depth)
  }

  pub fn get_stmoc_depths(&self, index: usize) -> Result<(u8, u8), String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_stmoc_time_and_space_depths)
  }

  pub fn is_empty(&self, index: usize) -> Result<bool, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::is_empty)
  }

  pub fn get_n_ranges(&self, index: usize) -> Result<u32, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_n_ranges)
  }

  pub fn get_ranges_sum(&self, index: usize) -> Result<u64, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_ranges_sum)
  }
  
  pub fn get_coverage_percentage(&self, index: usize) -> Result<f64, String> {
    store::exec_on_one_readonly_moc(
      index, 
      |internal_moc| internal_moc.get_coverage_percentage().ok_or_else(|| String::from("No coverage available for this type of MOC"))
    )
  }
  
  pub fn eq(&self, left_index: usize, right_index: usize) -> Result<bool, String> {
    store::exec_on_two_readonly_mocs(
      left_index,
      right_index,
      |l, r| Ok(l == r)
    )
  }
  
  pub fn to_uniq_hpx(&self, index: usize) -> Result<Vec<u64>, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_uniq_hpx)
  }

  pub fn to_uniq_gen(&self, index: usize) -> Result<Vec<u64>, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_uniq_gen)
  }

  pub fn to_uniq_zorder(&self, index: usize) -> Result<Vec<u64>, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_uniq_zorder)
  }

  pub fn to_ranges(&self, index: usize) -> Result<Vec<Range<u64>>, String> {
    store::exec_on_one_readonly_moc(index, InternalMoc::get_ranges)
  }
  
  ///////////////////////
  // LOAD EXISTING MOC //

  // - from fits //

  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_from_fits_file<P: AsRef<Path>>(&self, source: P) -> Result<usize, String> {
    let file = File::open(&source).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    self.load_from_fits(reader)
  }
  
  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_from_fits_buff(&self, content: &[u8]) -> Result<usize, String> {
    self.load_from_fits(Cursor::new(content))
  }

  pub fn load_from_fits<R: BufRead>(&self, reader: R) -> Result<usize, String> {
    from_fits_ivoa(reader)
      .map_err(|e| e.to_string())
      .and_then(
        |moc| match moc {
          MocIdxType::U16(moc) => from_fits_gen(moc),
          MocIdxType::U32(moc) => from_fits_gen(moc),
          MocIdxType::U64(moc) => from_fits_u64(moc),
        }.map_err(|e| e.to_string())
      )
      .and_then(store::add)
  }


  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_smoc_from_fits_file<P: AsRef<Path>>(&self, source: P) -> Result<usize, String> {
    let file = File::open(&source).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    self.load_smoc_from_fits(reader)
  }

  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_smoc_from_fits_buff(&self, content: &[u8]) -> Result<usize, String> {
    self.load_smoc_from_fits(Cursor::new(content))
  }

  pub fn load_smoc_from_fits<R: BufRead>(&self, reader: R) -> Result<usize, String> {
    from_fits_ivoa(reader)
      .map_err(|e| e.to_string())
      .and_then(
        |moc| match moc {
          MocIdxType::U16(moc) => smoc_from_fits_gen(moc),
          MocIdxType::U32(moc) => smoc_from_fits_gen(moc),
          MocIdxType::U64(moc) => smoc_from_fits_gen(moc),
        }.map_err(|e| e.to_string())
      )
      .and_then(store::add)
  }

  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_tmoc_from_fits_file<P: AsRef<Path>>(&self, source: P) -> Result<usize, String> {
    let file = File::open(&source).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    self.load_tmoc_from_fits(reader)
  }

  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_tmoc_from_fits_buff(&self, content: &[u8]) -> Result<usize, String> {
    self.load_tmoc_from_fits(Cursor::new(content))
  }

  pub fn load_tmoc_from_fits<R: BufRead>(&self, reader: R) -> Result<usize, String> {
    from_fits_ivoa(reader)
      .map_err(|e| e.to_string())
      .and_then(
        |moc| match moc {
          MocIdxType::U16(moc) => tmoc_from_fits_gen(moc),
          MocIdxType::U32(moc) => tmoc_from_fits_gen(moc),
          MocIdxType::U64(moc) => tmoc_from_fits_gen(moc),
        }.map_err(|e| e.to_string())
      )
      .and_then(store::add)
  }


  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_fmoc_from_fits_file<P: AsRef<Path>>(&self, source: P) -> Result<usize, String> {
    let file = File::open(&source).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    self.load_fmoc_from_fits(reader)
  }

  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_fmoc_from_fits_buff(&self, content: &[u8]) -> Result<usize, String> {
    self.load_fmoc_from_fits(Cursor::new(content))
  }

  pub fn load_fmoc_from_fits<R: BufRead>(&self, reader: R) -> Result<usize, String> {
    from_fits_ivoa(reader)
      .map_err(|e| e.to_string())
      .and_then(
        |moc| match moc {
          MocIdxType::U16(moc) => fmoc_from_fits_gen(moc),
          MocIdxType::U32(moc) => fmoc_from_fits_gen(moc),
          MocIdxType::U64(moc) => fmoc_from_fits_gen(moc),
        }.map_err(|e| e.to_string())
      )
      .and_then(store::add)
  }

  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_stmoc_from_fits_file<P: AsRef<Path>>(&self, source: P) -> Result<usize, String> {
    let file = File::open(&source).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    self.load_stmoc_from_fits(reader)
  }

  /// Load a MOC from the pre-loaded content of a FITS file, and put it in the store 
  ///
  /// # Output
  /// - The index in the storage
  pub fn load_stmoc_from_fits_buff(&self, content: &[u8]) -> Result<usize, String> {
    self.load_stmoc_from_fits(Cursor::new(content))
  }

  pub fn load_stmoc_from_fits<R: BufRead>(&self, reader: R) -> Result<usize, String> {
    from_fits_ivoa(reader)
      .map_err(|e| e.to_string())
      .and_then(
        |moc| match moc {
          MocIdxType::U16(_) => Err(String::from("Only u64 ST-MOCs are supported").into()),
          MocIdxType::U32(_) => Err(String::from("Only u64 ST-MOCs are supported").into()),
          MocIdxType::U64(moc) => stmoc_from_fits_u64(moc),
        }.map_err(|e| e.to_string())
      )
      .and_then(store::add)
  }


  /// Create o S-MOC from a FITS multi-order map plus other parameters.
  /// # Args
  /// * `path`: path of the fits file
  /// * `from_threshold`: Cumulative value at which we start putting cells in he MOC (often = 0).
  /// * `to_threshold`: Cumulative value at which we stop putting cells in the MOC.
  /// * `asc`: Compute cumulative value from ascending density values instead of descending (often = false).
  /// * `not_strict`: Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
  /// * `split`: Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
  /// * `revese_recursive_descent`: Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
  pub fn from_multiordermap_fits_file<P: AsRef<Path>>(
    &self,
    path: P,
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
  ) -> Result<usize, String> {
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    from_fits_multiordermap(
      reader,
      from_threshold,
      to_threshold,
      asc,
      !not_strict,
      split,
      revese_recursive_descent,
    ).map_err(|e| e.to_string())
      .and_then(store::add)
  }
  
  /// Create o S-MOC from a FITS multi-order map plus other parameters.
  /// # Args
  /// * `data`: binary content of the fits file
  /// * `from_threshold`: Cumulative value at which we start putting cells in he MOC (often = 0).
  /// * `to_threshold`: Cumulative value at which we stop putting cells in the MOC.
  /// * `asc`: Compute cumulative value from ascending density values instead of descending (often = false).
  /// * `not_strict`: Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
  /// * `split`: Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
  /// * `revese_recursive_descent`: Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
  pub fn from_multiordermap_fits_file_content(
    &self,
    data: &[u8],
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
  ) -> Result<usize, String> {
    from_fits_multiordermap(
      BufReader::new(Cursor::new(data)),
      from_threshold,
      to_threshold,
      asc,
      !not_strict,
      split,
      revese_recursive_descent,
    ).map_err(|e| e.to_string())
      .and_then(store::add)
  }

  /// Create o S-MOC from a FITS skymap plus other parameters.
  /// # Args
  /// * `path`: path of the fits file
  /// * `skip_values_le`: skip cells associated to values lower or equal to the given value 
  /// * `from_threshold`: Cumulative value at which we start putting cells in he MOC (often = 0).
  /// * `to_threshold`: Cumulative value at which we stop putting cells in the MOC.
  /// * `asc`: Compute cumulative value from ascending density values instead of descending (often = false).
  /// * `not_strict`: Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
  /// * `split`: Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
  /// * `revese_recursive_descent`: Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
  pub fn from_skymap_fits_file<P: AsRef<Path>>(
    &self,
    path: P,
    skip_values_le: f64,
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
  ) -> Result<usize, String> {
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    from_fits_skymap(
      reader,
      skip_values_le,
      from_threshold,
      to_threshold,
      asc,
      !not_strict,
      split,
      revese_recursive_descent,
    ).map_err(|e| e.to_string())
      .and_then(store::add)
  }
  
  /// Create o S-MOC from a FITS skymap plus other parameters.
  /// # Args
  /// * `data`: binary content of the fits file
  /// * `skip_values_le`: skip cells associated to values lower or equal to the given value 
  /// * `from_threshold`: Cumulative value at which we start putting cells in he MOC (often = 0).
  /// * `to_threshold`: Cumulative value at which we stop putting cells in the MOC.
  /// * `asc`: Compute cumulative value from ascending density values instead of descending (often = false).
  /// * `not_strict`: Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
  /// * `split`: Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
  /// * `revese_recursive_descent`: Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
  pub fn from_skymap_fits_file_content(
    &self,
    data: &[u8],
    skip_values_le: f64,
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
  ) -> Result<usize, String> {
    from_fits_skymap(
      BufReader::new(Cursor::new(data)),
      skip_values_le,
      from_threshold,
      to_threshold,
      asc,
      !not_strict,
      split,
      revese_recursive_descent,
    ).map_err(|e| e.to_string())
      .and_then(store::add)
  }

  // - from ascii  //

  pub fn load_smoc_from_ascii_file<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
    fs::read_to_string(path)
      .map_err(|e| e.to_string())
      .and_then(|s| self.load_smoc_from_ascii(&s))
  }

  pub fn load_tmoc_from_ascii_file<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
    fs::read_to_string(path)
      .map_err(|e| e.to_string())
      .and_then(|s| self.load_tmoc_from_ascii(&s))
  }

  pub fn load_fmoc_from_ascii_file<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
    fs::read_to_string(path)
      .map_err(|e| e.to_string())
      .and_then(|s| self.load_fmoc_from_ascii(&s))
  }

  pub fn load_stmoc_from_ascii_file<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
    fs::read_to_string(path)
      .map_err(|e| e.to_string())
      .and_then(|s| self.load_stmoc_from_ascii(&s))
  }

  
  pub fn load_smoc_from_ascii(&self, content: &str) -> Result<usize, String> {
    from_ascii_ivoa::<u64, Hpx::<u64>>(content)
      .map_err(|e| e.to_string())
      .and_then(|cellcellranges| {
        let moc = cellcellranges.into_cellcellrange_moc_iter().ranges().into_range_moc();
        store::add(moc)
      })
  }

  pub fn load_tmoc_from_ascii(&self, content: &str) -> Result<usize, String> {
    from_ascii_ivoa::<u64, Time::<u64>>(content)
      .map_err(|e| e.to_string())
      .and_then(|cellcellranges| {
        let moc = cellcellranges.into_cellcellrange_moc_iter().ranges().into_range_moc();
        store::add(moc)
      })
  }

  pub fn load_fmoc_from_ascii(&self, content: &str) -> Result<usize, String> {
    from_ascii_ivoa::<u64, Frequency::<u64>>(content)
      .map_err(|e| e.to_string())
      .and_then(|cellcellranges| {
        let moc = cellcellranges.into_cellcellrange_moc_iter().ranges().into_range_moc();
        store::add(moc)
      })
  }

  pub fn load_stmoc_from_ascii(&self, content: &str) -> Result<usize, String> {
    moc2d_from_ascii_ivoa::<u64, Time::<u64>, u64, Hpx::<u64>>(content)
      .map_err(|e| e.to_string())
      .and_then(|cellrange2| {
        let moc2 = cellrange2.into_cellcellrange_moc2_iter().into_range_moc2_iter().into_range_moc2();
        store::add(moc2)
      })
  }

  // - from json //

  pub fn load_smoc_from_json_file<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
    fs::read_to_string(path)
      .map_err(|e| e.to_string())
      .and_then(|s| self.load_smoc_from_json(&s))
  }

  pub fn load_tmoc_from_json_file<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
    fs::read_to_string(path)
      .map_err(|e| e.to_string())
      .and_then(|s| self.load_tmoc_from_json(&s))
  }

  pub fn load_fmoc_from_json_file<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
    fs::read_to_string(path)
      .map_err(|e| e.to_string())
      .and_then(|s| self.load_fmoc_from_json(&s))
  }

  pub fn load_stmoc_from_json_file<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
    fs::read_to_string(path)
      .map_err(|e| e.to_string())
      .and_then(|s| self.load_stmoc_from_json(&s))
  }
  
  
  pub fn load_smoc_from_json(&self, content: &str) -> Result<usize, String> {
    from_json_aladin::<u64, Hpx::<u64>>(content)
      .map_err(|e| e.to_string())
      .and_then(|cellrange2| {
        let moc = cellrange2.into_cell_moc_iter().ranges().into_range_moc();
        store::add(moc)
      })
  }

  pub fn load_tmoc_from_json(&self, content: &str) -> Result<usize, String> {
    from_json_aladin::<u64, Time::<u64>>(content)
      .map_err(|e| e.to_string())
      .and_then(|cells| {
        let moc = cells.into_cell_moc_iter().ranges().into_range_moc();
        store::add(moc)
      })
  }

  pub fn load_fmoc_from_json(&self, content: &str) -> Result<usize, String> {
    from_json_aladin::<u64, Frequency::<u64>>(content)
      .map_err(|e| e.to_string())
      .and_then(|cells| {
        let moc = cells.into_cell_moc_iter().ranges().into_range_moc();
        store::add(moc)
      })
  }

  pub fn load_stmoc_from_json(&self, content: &str) -> Result<usize, String> {
    cellmoc2d_from_json_aladin::<u64, Time::<u64>, u64, Hpx::<u64>>(content)
      .map_err(|e| e.to_string())
      .and_then(|cell2| {
        let moc2 = cell2.into_cell_moc2_iter().into_range_moc2_iter().into_range_moc2();
        store::add(moc2)
      })
  }

  ///////////////////////
  // SAVE EXISTING MOC //

  /// # Params
  /// * `smoc`: the Spatial MOC to be print;
  /// * `img_y_size`: the `Y` number of pixels in the image, the image size will be `(2*Y, Y)`;
  pub fn to_png(
    &self,
    moc_index: usize,
    img_y_size: u16,
  ) -> Result<Box<[u8]>, String> {
    let xsize = (img_y_size << 1) as usize;
    let ysize = img_y_size as usize;
    let op = move |moc: &InternalMoc| match moc {
      InternalMoc::Space(smoc) => {
        let data = to_img_default(smoc, (xsize as u16, ysize as u16), None, None);
        let mut buff = Vec::<u8>::with_capacity(1024 + xsize * ysize);
        let mut encoder = png::Encoder::new(&mut buff, xsize as u32, ysize as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()
          .map_err(|e| e.to_string())
          .and_then(move |mut writer|
            writer.write_image_data(&data)
              .map_err(|e| e.to_string())
          ).map(move |_| buff.into_boxed_slice())
      }
      _ => Err(String::from("Can't make a PNG for a MOC different from a S-MOC")),
    };
    store::exec_on_one_readonly_moc(moc_index, op)
  }

  /// Returns an RGBA array (each pixel is made of 4 successive u8: RGBA) using the Mollweide projection.
  pub fn to_image(
    &self,
    moc_index: usize,
    img_y_size: u16,
  ) -> Result<Box<[u8]>, String> {
    let xsize = (img_y_size << 1) as usize;
    let ysize = img_y_size as usize;
    let op = move |moc: &InternalMoc| match moc {
      InternalMoc::Space(smoc) => {
        Ok(
          to_img_default(smoc, (xsize as u16, ysize as u16), None, None)
            .into_boxed_slice()
        )
      }
      _ => Err(String::from("Can't make an image for a MOC different from a S-MOC")),
    };
    store::exec_on_one_readonly_moc(moc_index, op)
  }

  /// Returns the ASCII serialization of the given MOC.
  /// # Args
  ///
  pub fn to_ascii_str(&self, moc_index: usize, fold: Option<usize>) -> Result<String, String> {
    // from_str creates a copy :o/
    store::exec_on_one_readonly_moc(
      moc_index,
      move |moc| moc.to_ascii_str(fold),
    )
  }

  /// Write the ASCII serialization of the given MOC in the given path.
  /// # Args
  ///
  pub fn to_ascii_file<P: AsRef<Path>>(&self, moc_index: usize, destination: P, fold: Option<usize>) -> Result<(), String> {
    // from_str creates a copy :o/
    store::exec_on_one_readonly_moc(
      moc_index,
      move |moc| moc.to_ascii_file(destination, fold),
    )
  }

  // Instead of returning a String, we should probably return a map of (depth, array of indices) values :o/
  /// Returns the JSON serialization of the given MOC.
  /// # Args
  ///
  pub fn to_json_str(&self, moc_index: usize, fold: Option<usize>) -> Result<String, String> {
    store::exec_on_one_readonly_moc(
      moc_index,
      move |moc| moc.to_json_str(fold),
    )
  }

  /// Write the KSON serialization of the given MOC in the given path.
  /// # Args
  ///
  pub fn to_json_file<P: AsRef<Path>>(&self, moc_index: usize, destination: P, fold: Option<usize>) -> Result<(), String> {
    store::exec_on_one_readonly_moc(
      moc_index,
      move |moc| moc.to_json_file(destination, fold),
    )
  }

  /// Returns in memory the FITS serialization of the MOC of given `name`.
  /// # Args
  /// * `name`: name of the MOC in the internal store
  /// * `force_v1_compatibility`: for S-MOCs, force compatibility with Version 1 of the MOC standard. 
  pub fn to_fits_buff(&self, moc_index: usize, force_v1_compatibility: Option<bool>) -> Result<Box<[u8]>, String> {
    store::exec_on_one_readonly_moc(
      moc_index,
      move |moc| moc.to_fits_buff(force_v1_compatibility.unwrap_or(false)),
    )
  }

  /// Returns in memory the FITS serialization of the MOC of given `name` in the given path.
  /// # Args
  /// * `name`: name of the MOC in the internal store
  /// * `force_v1_compatibility`: for S-MOCs, force compatibility with Version 1 of the MOC standard. 
  pub fn to_fits_file<P: AsRef<Path>>(&self, moc_index: usize, destination: P, force_v1_compatibility: Option<bool>) -> Result<(), String> {
    store::exec_on_one_readonly_moc(
      moc_index,
      move |moc| moc.to_fits_file(destination, force_v1_compatibility.unwrap_or(false)),
    )
  }

  

  //////////////////
  // MOC CREATION //

  
  // * S-MOC CREATION //


  pub fn from_hpx_cells<T: Idx, I>(
    &self,
    depth: u8,
    cells_it: I,
    buf_capacity: Option<usize>
  ) -> Result<usize, String>
    where
      I: Iterator<Item=(u8, T)>
  {
    let it = cells_it.map(|(depth, idx)| (depth, idx.to_u64()));
    let moc: RangeMOC<u64, Hpx::<u64>> = RangeMOC::from_cells(depth, it, buf_capacity);
    store::add(moc)
  }
  
  pub fn from_hpx_ranges<T: Idx, I>(&self,
    depth: u8,
    ranges_it: I,
    buf_capacity: Option<usize>
  ) -> Result<usize, String>
   where
      I: Iterator<Item=Range<T>>
  {
    let it = ranges_it.map(|range| T::to_u64_idx(range.start)..T::to_u64_idx(range.end));
    let moc: RangeMOC<u64, Hpx::<u64>> = RangeMOC::from_maxdepth_ranges(depth, it, buf_capacity);
    store::add(moc)
  }


  /// Create and store a MOC from the given cone.
  ///
  /// # Input
  /// * `lon_deg` the longitude of the center of the cone, in degrees
  /// * `lat_deg` the latitude of the center of the cone, in degrees
  /// * `radius_deg` the radius of the cone, in degrees
  /// * `depth`: the MOC depth
  /// * `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_cone(
    &self,
    lon_deg: f64,
    lat_deg: f64,
    radius_deg: f64,
    depth: u8,
    delta_depth: u8,
  ) -> Result<usize, String> {
    check_depth::<Hpx<u64>>(depth)?;
    let lon = lon_deg2rad(lon_deg)?;
    let lat = lat_deg2rad(lat_deg)?;
    let r = radius_deg.to_radians();
    if (0.0..=PI).contains(&r) {
      let dd = delta_depth.min(Hpx::<u64>::MAX_DEPTH - depth);
      let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_cone(lon, lat, r, depth, dd);
      store::add(moc)
    } else {
      Err(String::from("Cone radius must be in [0, pi["))
    }
  }

  /// Create and store a MOC from the given ring.
  ///
  /// # Input
  /// * `lon_deg` the longitude of the center of the ring, in degrees
  /// * `lat_deg` the latitude of the center of the ring, in degrees
  /// * `internal_radius_deg` the internal radius of the ring, in degrees
  /// * `external_radius_deg` the external radius of the ring, in degrees
  /// * `depth`: the MOC depth
  /// * `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_ring(
    &self,
    lon_deg: f64,
    lat_deg: f64,
    internal_radius_deg: f64,
    external_radius_deg: f64,
    depth: u8,
    delta_depth: u8,
  ) -> Result<usize, String> {
    check_depth::<Hpx<u64>>(depth)?;
    let lon = lon_deg2rad(lon_deg)?;
    let lat = lat_deg2rad(lat_deg)?;
    let r_int = internal_radius_deg.to_radians();
    let r_ext = external_radius_deg.to_radians();
    if r_int <= 0.0 || PI <= r_int {
      Err(String::from("Internal radius must be in ]0, pi["))
    } else if r_ext <= 0.0 || PI <= r_ext {
      Err(String::from("External radius must be in ]0, pi["))
    } else if r_ext < r_int {
      Err(String::from("External radius must be larger than the internal radius"))
    } else {
      let dd = delta_depth.min(Hpx::<u64>::MAX_DEPTH - depth);
      let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_ring(lon, lat, r_int, r_ext, depth, dd);
      store::add(moc)
    }
  }

  /// Create and store a MOC from the given elliptical cone.
  ///
  /// # Input
  /// * `lon_deg` the longitude of the center of the elliptical cone, in degrees
  /// * `lat_deg` the latitude of the center of the elliptical cone, in degrees
  /// * `a_deg` the semi-major axis of the elliptical cone, in degrees
  /// * `b_deg` the semi-minor axis of the elliptical cone, in degrees
  /// * `pa_deg` the position angle (i.e. the angle between the north and the semi-major axis, east-of-north), in degrees
  /// * `depth`: the MOC depth
  /// * `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_elliptical_cone(
    &self,
    lon_deg: f64,
    lat_deg: f64,
    a_deg: f64,
    b_deg: f64,
    pa_deg: f64,
    depth: u8,
    delta_depth: u8,
  ) -> Result<usize, String> {
    check_depth::<Hpx<u64>>(depth)?;
    let lon = lon_deg2rad(lon_deg)?;
    let lat = lat_deg2rad(lat_deg)?;
    let a = a_deg.to_radians();
    let b = b_deg.to_radians();
    let pa = pa_deg.to_radians();
    if a <= 0.0 || HALF_PI <= a {
      Err(String::from("Semi-major axis must be in ]0, pi/2]"))
    } else if b <= 0.0 || a <= b {
      Err(String::from("Semi-minor axis must be in ]0, a["))
    } else if pa < 0.0 || HALF_PI <= pa {
      Err(String::from("Position angle must be in [0, pi["))
    } else {
      let dd = delta_depth.min(Hpx::<u64>::MAX_DEPTH - depth);
      let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_elliptical_cone(lon, lat, a, b, pa, depth, dd);
      store::add(moc)
    }
  }

  /// Create and store a MOC from the given zone.
  ///
  /// # Input
  /// * `lon_deg_min` the longitude of the bottom left corner, in degrees
  /// * `lat_deg_min` the latitude of the bottom left corner, in degrees
  /// * `lon_deg_max` the longitude of the upper left corner, in degrees
  /// * `lat_deg_max` the latitude of the upper left corner, in degrees
  /// * `depth`: the MOC depth
  ///
  /// # Output
  /// - The index in the storage
  ///
  /// # Remark
  /// - If `lon_min > lon_max` then we consider that the zone crosses the primary meridian.
  /// - The north pole is included only if `lon_min == 0 && lat_max == pi/2`
  pub fn from_zone(
    &self,
    lon_deg_min: f64,
    lat_deg_min: f64,
    lon_deg_max: f64,
    lat_deg_max: f64,
    depth: u8,
  ) -> Result<usize, String> {
    check_depth::<Hpx<u64>>(depth)?;
    let lon_min = lon_deg2rad(lon_deg_min)?;
    let lat_min = lat_deg2rad(lat_deg_min)?;
    let lon_max = lon_deg2rad(lon_deg_max)?;
    let lat_max = lat_deg2rad(lat_deg_max)?;
    let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_zone(lon_min, lat_min, lon_max, lat_max, depth);
    store::add(moc)
  }

  /// Create and store a MOC from the given box.
  ///
  /// # Input
  /// * `lon_deg` the longitude of the center of the box, in degrees
  /// * `lat_deg` the latitude of the center of the box, in degrees
  /// * `a_deg` the semi-major axis of the box (half the box width), in degrees
  /// * `b_deg` the semi-minor axis of the box (half the box height), in degrees
  /// * `pa_deg` the position angle (i.e. the angle between the north and the semi-major axis, east-of-north), in radians
  /// * `depth`: the MOC depth
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_box(
    &self,
    lon_deg: f64,
    lat_deg: f64,
    a_deg: f64,
    b_deg: f64,
    pa_deg: f64,
    depth: u8,
  ) -> Result<usize, String> {
    check_depth::<Hpx<u64>>(depth)?;
    let lon = lon_deg2rad(lon_deg)?;
    let lat = lat_deg2rad(lat_deg)?;
    let a = a_deg.to_radians();
    let b = b_deg.to_radians();
    let pa = pa_deg.to_radians();
    if a <= 0.0 || HALF_PI <= a {
      Err(String::from("Semi-major axis must be in ]0, pi/2]"))
    } else if b <= 0.0 || a < b {
      Err(String::from("Semi-minor axis must be in ]0, a["))
    } else if pa < 0.0 || PI <= pa {
      Err(String::from("Position angle must be in [0, pi["))
    } else {
      let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_box(lon, lat, a, b, pa, depth);
      store::add(moc)
    }
  }

  /// Create and store a new MOC from the given polygon vertices.
  ///
  /// # Params
  /// * `vertices`: vertices coordinates, in degrees
  /// * `complement`: reverse the default inside/outside of the polygon
  /// * `depth`: MOC maximum depth in `[0, 29]`
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_polygon<T>(
    &self,
    vertices_it: T,
    complement: bool,
    depth: u8,
  ) -> Result<usize, String>
    where
      T: Iterator<Item=(f64, f64)>
  {
    check_depth::<Hpx<u64>>(depth)?;
    let vertices = vertices_it.map(
      |(lon_deg, lat_deg)| {
        let lon = lon_deg2rad(lon_deg)?;
        let lat = lat_deg2rad(lat_deg)?;
        Ok((lon, lat))
      }
    ).collect::<Result<Vec<(f64, f64)>, String>>()?;
    let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_polygon(&vertices, complement, depth);
    store::add(moc)
  }


  /// Create and store a new MOC from the given list of coordinates (assumed to be equatorial)
  /// # Params
  /// * `depth`: MOC maximum depth in `[0, 29]`
  /// * `coos_deg`: list of coordinates in degrees
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_coo<T>(&self, depth: u8, coos_deg: T) -> Result<usize, String>
    where
      T: Iterator<Item=(f64, f64)>
  {
    check_depth::<Hpx<u64>>(depth)?;
    let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_coos(
      depth,
      coos_deg.filter_map(|(lon_deg, lat_deg)| {
        let lon = lon_deg2rad(lon_deg);
        let lat = lat_deg2rad(lat_deg);
        match (lon, lat) {
          (Ok(lon), Ok(lat)) => Some((lon, lat)),
          _ => None,
        }
      }),
      None,
    );
    store::add(moc)
  }

  /// Create and store a new MOC from the given list of cone centers and radii
  /// Adapted for a large number of small cones (a few cells each).
  ///
  /// # Params
  /// * `depth`: MOC maximum depth in `[0, 29]`
  /// * `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).  
  /// * `coos_and_radius_deg`: list of coordinates and radii in degrees `((lon, lat), rad)`
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_small_cones<T>(
    &self,
    depth: u8,
    delta_depth: u8,
    coos_and_radius_deg: T,
  ) -> Result<usize, String>
    where
      T: Iterator<Item=((f64, f64), f64)>
  {
    check_depth::<Hpx<u64>>(depth)?;
    let dd = delta_depth.min(Hpx::<u64>::MAX_DEPTH - depth);
    let coos_rad = coos_and_radius_deg.filter_map(|((lon_deg, lat_deg), radius_deg)| {
      let lon = lon_deg2rad(lon_deg);
      let lat = lat_deg2rad(lat_deg);
      match (lon, lat) {
        (Ok(lon), Ok(lat)) => Some((lon, lat, radius_deg.to_radians())),
        _ => None,
      }
    });
    let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_small_cones(depth, dd, coos_rad, None);
    store::add(moc)
  }

  /// Create and store a new MOC from the given list of cone centers and radii
  /// Adapted for a reasonable number of possibly large cones.
  ///
  /// # Params
  /// * `depth`: MOC maximum depth in `[0, 29]`
  /// * `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).  
  /// * `coos_and_radius_deg`: list of coordinates and radii in degrees `((lon, lat), rad)`
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_large_cones<T>(
    &self,
    depth: u8,
    delta_depth: u8,
    coos_and_radius_deg: T,
  ) -> Result<usize, String>
    where
      T: Iterator<Item=((f64, f64), f64)>
  {
    check_depth::<Hpx<u64>>(depth)?;
    let dd = delta_depth.min(Hpx::<u64>::MAX_DEPTH - depth);
    let coos_rad = coos_and_radius_deg.filter_map(|((lon_deg, lat_deg), radius_deg)| {
      let lon = lon_deg2rad(lon_deg);
      let lat = lat_deg2rad(lat_deg);
      match (lon, lat) {
        (Ok(lon), Ok(lat)) => Some((lon, lat, radius_deg.to_radians())),
        _ => None,
      }
    });
    let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_large_cones(depth, dd, coos_rad);
    store::add(moc)
  }


  /// Create a new S-MOC from the given lists of UNIQ and Values.
  /// # Params
  /// * `name`: the name to be given to the MOC
  /// * `depth`: S-MOC maximum depth in `[0, 29]`, Must be >= largest input cells depth.
  /// * `density`: Input values are densities, i.e. they are not proportional to the area of their associated cells.
  /// * `from_threshold`: Cumulative value at which we start putting cells in he MOC (often = 0).
  /// * `to_threshold`: Cumulative value at which we stop putting cells in the MOC.
  /// * `asc`: Compute cumulative value from ascending density values instead of descending (often = false).
  /// * `not_strict`: Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
  /// * `split`: Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
  /// * `revese_recursive_descent`: Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
  /// * `uniqs`: array of uniq HEALPix cells
  /// * `values`: array of values associated to the HEALPix cells
  pub fn from_valued_cells<T>(
    &self,
    depth: u8,
    density: bool,
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
    uniq_vals: T,
  ) -> Result<usize, String>
    where
      T: Iterator<Item=(u64, f64)>
  {
    if to_threshold < from_threshold {
      return Err(String::from("`cumul_from` has to be < to `cumul_to`."))
    }
    let area_per_cell = (PI / 3.0) / (1_u64 << (depth << 1) as u32) as f64;  // = 4pi / (12*4^depth)
    let ranges: HpxRanges<u64> = if density {
      valued_cells_to_moc_with_opt::<u64, f64>(
        depth,
        uniq_vals.map(|(uniq, dens)| {
          let (cdepth, _ipix) = Hpx::<u64>::from_uniq_hpx(uniq);
          if cdepth > depth {
            Err(format!("Too deep cell depth. Expected: <= {}; Actual: {}", depth, cdepth))
          } else {
            let n_sub_cells = (1_u64 << (((depth - cdepth) << 1) as u32)) as f64;
            Ok((uniq, dens * n_sub_cells * area_per_cell, dens))
          }
        }).collect::<Result<_, String>>()?,
        from_threshold, to_threshold, asc, !not_strict, !split, revese_recursive_descent,
      )
    } else {
      valued_cells_to_moc_with_opt::<u64, f64>(
        depth,
        uniq_vals.map(|(uniq, val)| {
          let (cdepth, _ipix) = Hpx::<u64>::from_uniq_hpx(uniq);
          if cdepth > depth {
            Err(format!("Too deep cell depth. Expected: <= {}; Actual: {}", depth, cdepth))
          } else {
            let n_sub_cells = (1_u64 << (((depth - cdepth) << 1) as u32)) as f64;
            Ok((uniq, val, val / (n_sub_cells * area_per_cell)))
          }
        }).collect::<Result<_, String>>()?,
        from_threshold, to_threshold, asc, !not_strict, !split, revese_recursive_descent,
      )
    };
    let moc = RangeMOC::new(depth, ranges);
    store::add(moc)
  }


  // - SMOC MutliOrder


  // - SMOC SkyMaps


  // * T-MOC CREATION //

  pub fn from_microsec_since_jd0<T>(&self, depth: u8, microsec_since_jd0_it: T) -> Result<usize, String>
    where
      T: Iterator<Item=u64>
  {
    check_depth::<Time<u64>>(depth)?;
    let moc = RangeMOC::<u64, Time<u64>>::from_microsec_since_jd0(
      depth, microsec_since_jd0_it, None,
    );
    store::add(moc)
  }
    
  /// Create a new T-MOC from the given list of decimal Julian Days (JD) times.
  /// # Params
  /// * `name`: the name to be given to the MOC
  /// * `depth`: T-MOC maximum depth in `[0, 61]`
  /// * `jd`: array of decimal JD time (`f64`)
  /// # WARNING
  /// Using decimal Julian Days stored on `f64`, the precision does not reach the microsecond
  /// since JD=0.
  /// In Javascript, there is no `u64` type (integers are stored on the mantissa of 
  /// a double -- a `f64` --, which is made of 52 bits).
  /// The other approach is to use a couple of `f64`: one for the integer part of the JD, the
  /// other for the fractional part of the JD.
  /// We will add such a method later if required by users.
  pub fn from_decimal_jd_values<T>(&self, depth: u8, jd: T) -> Result<usize, String>
    where
      T: Iterator<Item=f64>
  {
    self.from_microsec_since_jd0(depth, jd.map(|jd| (jd * JD_TO_USEC) as u64))
  }

  pub fn from_microsec_ranges_since_jd0<T>(&self, depth: u8, microsec_ranges_since_jd0_it: T) -> Result<usize, String>
    where
      T: Iterator<Item=Range<u64>>
  {
    check_depth::<Time<u64>>(depth)?;
    let moc = RangeMOC::<u64, Time<u64>>::from_microsec_ranges_since_jd0(
      depth, microsec_ranges_since_jd0_it, None,
    );
    store::add(moc)
  }
  
  pub fn from_decimal_jd_ranges<T>(&self, depth: u8, jd_ranges: T) -> Result<usize, String>
    where
      T: Iterator<Item=Range<f64>>
  {
    self.from_microsec_ranges_since_jd0(
      depth,
      jd_ranges.map(|Range { start: jd_min, end: jd_max }| (jd_min * JD_TO_USEC) as u64..(jd_max * JD_TO_USEC) as u64),
    )
  }

  // * F-MOC CREATION //

  /// Create an store a new F-MOC from the given list of frequencies (Hz).
  ///
  /// # Input
  /// * `depth`: F-MOC maximum depth in `[0, 59]`
  /// * `freq`: iterator on frequencies, in Hz (`f64`)
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_hz_values<T>(&self, depth: u8, freq: T) -> Result<usize, String>
    where
      T: Iterator<Item=f64>
  {
    check_depth::<Frequency<u64>>(depth)?;
    let moc = RangeMOC::<u64, Frequency<u64>>::from_freq_in_hz(depth, freq, None);
    store::add(moc)
  }

  /// Create and store a new F-MOC from the given list of frequencies (Hz) ranges.
  ///
  /// # Input
  /// * `depth`: F-MOC maximum depth in `[0, 59]`
  /// * `freq_ranges`: iterator on frequencies ranges, in Hz (`f64`)
  ///
  /// # Output
  /// - The index in the storage
  pub fn from_hz_ranges<T>(&self, depth: u8, freq_ranges: T) -> Result<usize, String>
    where
      T: Iterator<Item=Range<f64>>
  {
    check_depth::<Frequency<u64>>(depth)?;
    let moc = RangeMOC::<u64, Frequency<u64>>::from_freq_ranges_in_hz(depth, freq_ranges, None);
    store::add(moc)
  }

  // * ST-MOC CREATION //

  /// Create a abd store a new ST-MOC from a list of sky coordinates and times.
  ///
  /// # Arguments
  ///
  /// * `times` - The times expressed in jd coded on doubles (=> not precise to the microsecond).
  /// * `lon` - The longitudes of the sky coordinates, in radians.
  /// * `lat` - The latitudes of the sky coordinates, in radians.
  /// * `dt` - The depth along the time (i.e. `T`) axis.
  /// * `ds` - The depth at which HEALPix cell indices
  ///   will be computed.
  ///
  /// # Output
  /// - The index in the storage
  pub fn create_from_times_positions_approx(
    &self,
    times: Vec<f64>,
    lon: Vec<f64>,
    lat: Vec<f64>,
    time_depth: u8,
    space_depth: u8,
  ) -> Result<usize, String> {
    let times = jd2mas_approx(times);
    self.create_from_times_positions(times, lon, lat, time_depth, space_depth)
  }

  /// Create a abd store a new ST-MOC from a list of sky coordinates and times.
  ///
  /// # Arguments
  ///
  /// * `times` - The times expressed in microsecond since jd=0.
  /// * `lon` - The longitudes of the sky coordinates, in radians.
  /// * `lat` - The latitudes of the sky coordinates, in radians.
  /// * `time_depth` - The depth along the time (i.e. `T`) axis.
  /// * `space_depth` - The depth at which HEALPix cell indices
  ///   will be computed.
  ///
  /// # Output
  /// - The index in the storage
  pub fn create_from_times_positions(
    &self,
    times: Vec<u64>,
    lon: Vec<f64>,
    lat: Vec<f64>,
    time_depth: u8,
    space_depth: u8,
  ) -> Result<usize, String> {
    if time_depth > Time::<u64>::MAX_DEPTH {
      Err(format!("Time depth must be in [0, {}]", Time::<u64>::MAX_DEPTH))
    } else if times.len() != lon.len() {
      Err(format!("Times and longitudes do not have the same size: {} != {}", times.len(), lon.len()))
    } else {
      lonlat2hash(space_depth, lon, lat)
        .map(|ipix| TimeSpaceMoc::<u64, u64>::create_from_times_positions(
            times, ipix, time_depth, space_depth,
        )).and_then(|moc| store::add(moc.time_space_iter(time_depth, space_depth).into_range_moc2()))
    }
  }

  /// Create a time-spatial coverage (2D) from a list of sky coordinates
  /// and ranges of times.
  ///
  /// # Arguments
  ///
  /// * ``times_start`` - The starting times expressed in jd.
  /// * ``times_end`` - The ending times expressed in jd.
  /// * ``lon`` - The longitudes of the sky coordinates.
  /// * ``lat`` - The latitudes of the sky coordinates.
  /// * ``dt`` - The depth along the time (i.e. `T`) axis.
  /// * ``ds`` - The depth at which HEALPix cell indices
  ///   will be computed.
  ///
  /// # Precondition
  ///
  /// * ``lon`` and ``lat`` are expressed in radians.
  /// They are valid because they come from
  /// `astropy.units.Quantity` objects.
  /// * ``times`` are expressed in jd and are coming
  /// from `astropy.time.Time` objects.
  ///
  /// # Errors
  ///
  /// If the number of longitudes, latitudes and times do not match.
  ///
  pub fn create_from_time_ranges_positions_approx(
    &self,
    times_start: Vec<f64>,
    times_end: Vec<f64>,
    time_depth: u8,
    lon: Vec<f64>,
    lat: Vec<f64>,
    space_depth: u8,
  ) -> Result<usize, String> {
    let times_start = jd2mas_approx(times_start);
    let times_end = jd2mas_approx(times_end);
    self.create_from_time_ranges_positions(times_start, times_end, time_depth, lon, lat, space_depth)
  }

  /// Create a time-spatial coverage (2D) from a list of sky coordinates
  /// and ranges of times.
  ///
  /// # Arguments
  ///
  /// * ``times_start`` - The starting times expressed in microseconds since jd=0.
  /// * ``times_end`` - The ending times expressed in microseconds since jd=0.
  /// * ``lon`` - The longitudes of the sky coordinates.
  /// * ``lat`` - The latitudes of the sky coordinates.
  /// * ``dt`` - The depth along the time (i.e. `T`) axis.
  /// * ``ds`` - The depth at which HEALPix cell indices
  ///   will be computed.
  ///
  /// # Precondition
  ///
  /// * ``lon`` and ``lat`` are expressed in radians.
  /// They are valid because they come from
  /// `astropy.units.Quantity` objects.
  /// * ``times`` are expressed in jd and are coming
  /// from `astropy.time.Time` objects.
  ///
  /// # Errors
  ///
  /// If the number of longitudes, latitudes and times do not match.
  pub fn create_from_time_ranges_positions(
    &self,
    times_start: Vec<u64>,
    times_end: Vec<u64>,
    time_depth: u8,
    lon: Vec<f64>,
    lat: Vec<f64>,
    space_depth: u8,
  ) -> Result<usize, String> {
    if times_start.len() != lon.len() {
      Err(format!("Times and coos do not have the same size: {} != {}.", times_start.len(), lon.len()))
    } else {
      let ipix = lonlat2hash(space_depth, lon, lat)?;
      let times = times2hash(time_depth, times_start, times_end)?;
      let moc = TimeSpaceMoc::<u64, u64>::create_from_time_ranges_positions(
        times, ipix, time_depth, space_depth
      );
      store::add(moc.time_space_iter(time_depth, space_depth).into_range_moc2())
    }
  }


  /// Create a time-spatial coverage (2D) from a list of cones
  /// and time ranges.
  ///
  /// # Arguments
  ///
  /// * ``times_start`` - The starting times expressed in jd.
  /// * ``times_end`` - The ending times expressed in jd.
  /// * ``lon`` - The longitudes of the sky coordinates.
  /// * ``lat`` - The latitudes of the sky coordinates.
  /// * ``radius`` - The radiuses of the cones.
  /// * ``dt`` - The depth along the time (i.e. `T`) axis.
  /// * ``ds`` - The depth at which HEALPix cell indices
  ///   will be computed.
  ///
  pub fn from_time_ranges_spatial_coverages_approx(
    &self,
    times_start: Vec<f64>,
    times_end: Vec<f64>,
    time_depth: u8,
    spatial_coverages: Vec<HpxRanges<u64>>,
    space_depth: u8,
  ) -> Result<usize, String> {
    let times_start = jd2mas_approx(times_start);
    let times_end = jd2mas_approx(times_end);
    self.from_time_ranges_spatial_coverages(
        times_start, times_end, time_depth, spatial_coverages, space_depth
    )
  }

  /// Create a time-spatial coverage (2D) from a list of cones
  /// and time ranges.
  ///
  /// # Arguments
  ///
  /// * ``times_start`` - The starting times expressed in microseconds since jd=0.
  /// * ``times_end`` - The ending times expressed in  microseconds since jd=0.
  /// * ``dt`` - The depth along the time (i.e. `T`) axis.
  /// 
  /// * ``ds`` - The depth at which HEALPix cell indices
  ///   will be computed.
  ///
  /// # Precondition
  ///
  /// * ``lon`` and ``lat`` are expressed in radians.
  /// They are valid because they come from
  /// `astropy.units.Quantity` objects.
  /// * ``times`` are expressed in jd and are coming
  /// from `astropy.time.Time` objects.
  ///
  /// # Errors
  ///
  /// If the number of longitudes, latitudes and times do not match.
  pub fn from_time_ranges_spatial_coverages(
    &self,
    times_start: Vec<u64>,
    times_end: Vec<u64>,
    time_depth: u8,
    spatial_coverages: Vec<HpxRanges<u64>>,
    space_depth: u8,
  ) -> Result<usize, String> {
    let times = times2hash(time_depth, times_start, times_end)?;
    let moc = TimeSpaceMoc::<u64, u64>::create_from_time_ranges_spatial_coverage(
      times, spatial_coverages, time_depth,
    );
    store::add(moc.time_space_iter(time_depth, space_depth).into_range_moc2())
  }

  /// Create a time-spatial coverage (2D) from a list of cones
  /// and time ranges.
  ///
  /// # Arguments
  ///
  /// * ``times_start`` - The starting times expressed in jd.
  /// * ``times_end`` - The ending times expressed in jd.
  /// * ``lon`` - The longitudes of the sky coordinates.
  /// * ``lat`` - The latitudes of the sky coordinates.
  /// * ``radius`` - The radiuses of the cones.
  /// * ``dt`` - The depth along the time (i.e. `T`) axis.
  /// * ``ds`` - The depth at which HEALPix cell indices
  ///   will be computed.
  ///
  pub fn from_time_ranges_spatial_coverages_in_store_approx(
    &self,
    times_start: Vec<f64>,
    times_end: Vec<f64>,
    time_depth: u8,
    spatial_coverages: Vec<usize>,
    // space_depth: u8,
  ) -> Result<usize, String> {
    let times_start = jd2mas_approx(times_start);
    let times_end = jd2mas_approx(times_end);
    self.from_time_ranges_spatial_coverages_in_store(
      times_start, times_end, time_depth, spatial_coverages//, space_depth
    )
  }
  
  /// Create a time-spatial coverage (2D) from a list of cones
  /// and time ranges.
  ///
  /// # Arguments
  ///
  /// * ``times_start`` - The starting times expressed in microseconds since jd=0.
  /// * ``times_end`` - The ending times expressed in  microseconds since jd=0.
  /// * ``lon`` - The longitudes of the sky coordinates.
  /// * ``lat`` - The latitudes of the sky coordinates.
  /// * ``radius`` - The radiuses of the cones.
  /// * ``dt`` - The depth along the time (i.e. `T`) axis.
  /// * ``ds`` - The depth at which HEALPix cell indices
  ///   will be computed.
  ///
  /// # Precondition
  ///
  /// * ``lon`` and ``lat`` are expressed in radians.
  /// They are valid because they come from
  /// `astropy.units.Quantity` objects.
  /// * ``times`` are expressed in jd and are coming
  /// from `astropy.time.Time` objects.
  ///
  /// # Errors
  ///
  /// If the number of longitudes, latitudes and times do not match.
  pub fn from_time_ranges_spatial_coverages_in_store(
    &self,
    times_start: Vec<u64>,
    times_end: Vec<u64>,
    time_depth: u8,
    spatial_coverage_indices: Vec<usize>,
    // space_depth: u8,
  ) -> Result<usize, String> {
    let times = times2hash(time_depth, times_start, times_end)?;
    let space_depth = spatial_coverage_indices.iter()
      .filter_map(|index| self.get_smoc_depth(*index).ok())
      .max()
      .unwrap_or(0);
    let spatial_coverages: Vec<HpxRanges<u64>> = spatial_coverage_indices.into_iter().map(
      |index| self.get_smoc_copy(index).map(|moc| moc.into_moc_ranges())
      // |index| self.degrade(index, space_depth).map(|moc| moc.into_moc_ranges())
    ).collect::<Result<_, _>>()?;
    let moc = TimeSpaceMoc::<u64, u64>::create_from_time_ranges_spatial_coverage(
      times, spatial_coverages, time_depth,
    );
    store::add(moc.time_space_iter(time_depth, space_depth).into_range_moc2())
  }


  /////////////////////////
  // OPERATIONS ON 1 MOC //

  // return a hierachical view (Json like) for display?
  // (not necessary if display made from rust code too)

  
  pub fn barycenter(&self, index: usize) -> Result<(f64, f64), String> {
    op1_moc_barycenter(index)
  }

  pub fn largest_distance_from_coo_to_moc_vertices(&self, index: usize, lon: f64, lat: f64) -> Result<f64, String> {
    op1_moc_largest_distance_from_coo_to_moc_vertices(index, lon, lat)
  }
  
  pub fn not(&self, index: usize) -> Result<usize, String> {
    self.complement(index)
  }
  pub fn complement(&self, index: usize) -> Result<usize, String> {
    Op1::Complement.exec(index)
  }


  pub fn flatten_to_moc_depth(&self, index: usize) -> Result<Vec<u64>, String> {
    op1_flatten_to_moc_depth(index)
  }

  pub fn flatten_to_depth(&self, index: usize, depth: u8) -> Result<Vec<u64>, String> {
    op1_flatten_to_depth(index, depth)
  }

  /// Split the given disjoint S-MOC int joint S-MOCs.
  /// Split "direct", i.e. we consider 2 neighboring cells to be the same only if the share an edge.
  /// WARNING: may create a lot of new MOCs, exec `splitCount` first!!
  pub fn split(&self, index: usize) -> Result<Vec<usize>, String> {
    Op1MultiRes::Split.exec(index)
  }

  /// Count the number of joint S-MOC splitting ("direct") the given disjoint S-MOC.
  pub fn split_count(&self, index: usize) -> Result<u32, String> {
    op1_count_split(index, false)
  }

  /// Split the given disjoint S-MOC int joint S-MOCs.
  /// Split "indirect", i.e. we consider 2 neighboring cells to be the same if the share an edge
  /// or a vertex.
  /// WARNING: may create a lot of new MOCs, exec `splitIndirectCount` first!!
  pub fn split_indirect(&self, index: usize) -> Result<Vec<usize>, String> {
    Op1MultiRes::SplitIndirect.exec(index)
  }

  /// Count the number of joint S-MOC splitting ("direct") the given disjoint S-MOC.
  pub fn split_indirect_count(&self, index: usize) -> Result<u32, String> {
    op1_count_split(index, true)
  }


  pub fn degrade(&self, index: usize, new_depth: u8) -> Result<usize, String> {
    Op1::Degrade { new_depth }.exec(index)
  }

  pub fn extend(&self, index: usize) -> Result<usize, String> {
    Op1::Extend.exec(index)
  }

  pub fn contract(&self, index: usize) -> Result<usize, String> {
    Op1::Contract.exec(index)
  }

  pub fn ext_border(&self, index: usize) -> Result<usize, String> {
    Op1::ExtBorder.exec(index)
  }

  pub fn int_border(&self, index: usize) -> Result<usize, String> {
    Op1::IntBorder.exec(index)
  }

  ////////////////////////////////////////////////////
  // LOGICAL OPERATIONS BETWEEN 2 MOCs of same type //

  pub fn or(&self, left_index: usize, right_index: usize) -> Result<usize, String> {
    self.union(left_index, right_index)
  }
  pub fn union(&self, left_index: usize, right_index: usize) -> Result<usize, String> {
    Op2::Union.exec(left_index, right_index)
  }

  pub fn and(&self, left_index: usize, right_index: usize) -> Result<usize, String> {
    self.intersection(left_index, right_index)
  }
  pub fn intersection(&self, left_index: usize, right_index: usize) -> Result<usize, String> {
    Op2::Intersection.exec(left_index, right_index)
  }

  pub fn xor(&self, left_index: usize, right_index: usize) -> Result<usize, String> {
    self.symmetric_difference(left_index, right_index)
  }
  pub fn symmetric_difference(&self, left_index: usize, right_index: usize) -> Result<usize, String> {
    Op2::SymmetricDifference.exec(left_index, right_index)
  }

  pub fn minus(&self, left_index: usize, right_index: usize) -> Result<usize, String> {
    Op2::Minus.exec(left_index, right_index)
  }
  pub fn difference(&self, left_index: usize, right_index: usize) -> Result<usize, String> {
    self.minus(left_index, right_index)
  }

  /////////////////////////////////////////////////////
  // LOGICAL OPERATIONS BETWEEN >2 MOCs of same type //

  pub fn multi_union(&self, indices: &[usize]) -> Result<usize, String> {
    OpN::Union.exec(indices)
  }

  pub fn multi_intersection(&self, indices: &[usize]) -> Result<usize, String> {
    OpN::Intersection.exec(indices)
  }
  
  pub fn multi_symmetric_difference(&self, indices: &[usize]) -> Result<usize, String> {
    OpN::SymmetricDifference.exec(indices)
  }
  
  ////////////////////////
  // ST-MOC projections //

  /// Returns the union of the S-MOCs associated to T-MOCs intersecting the given T-MOC.
  /// Left: T-MOC, right: ST-MOC, result: S-MOC.
  pub fn time_fold(&self, time_moc_index: usize, st_moc_index: usize) -> Result<usize, String> {
    Op2::TFold.exec(time_moc_index, st_moc_index)
  }

  /// Returns the union of the T-MOCs associated to S-MOCs intersecting the given S-MOC. 
  /// Left: S-MOC, right: ST-MOC, result: T-MOC.
  pub fn space_fold(&self, space_moc_index: usize, st_moc_index: usize) -> Result<usize, String> {
    Op2::SFold.exec(space_moc_index, st_moc_index)
  }

  ///////////////////////
  // FILTER OPERATIONS //

  //////////////////////////////////////////////////////
  // Filter/Contains (returning an array of boolean?) //

  /// Returns an array (of boolean or u8 or ...) telling if the pairs of coordinates
  /// in the input slice are in (true=1) or out of (false=0) the S-MOC.
  /// # Args
  /// * `moc_index`: index of the S-MOC to be used for filtering
  /// * `coos_deg`: iterator on coordinates in degrees `[lon_1, lat_1, lon_2, lat_2, ..., lon_n, lat_n]`
  /// # Remarks
  /// * the size of the returned array is the same as the number of elements on the input iterator.
  /// * we do not return an iterator to avoid chaining with possibly costly operations
  ///   while keeping a read lock on the store.
  /// * similarly, be carefull not to use an input Iterator based on costly operations...
  pub fn filter_pos<T, F, R>(&self, moc_index: usize, coos_deg: T, fn_bool: F) -> Result<Vec<R>, String>
    where
      T: Iterator<Item=(f64, f64)>,
      F: Fn(bool) -> R
  {
    let filter = |moc: &InternalMoc| match moc {
      InternalMoc::Space(moc) => {
        let depth = moc.depth_max();
        let layer = healpix::nested::get(depth);
        let shift = Hpx::<u64>::shift_from_depth_max(depth) as u32;
        Ok(
          coos_deg.map(|(lon_deg, lat_deg)| {
            let lon = lon_deg2rad(lon_deg);
            let lat = lat_deg2rad(lat_deg);
            match (lon, lat) {
              (Ok(lon), Ok(lat)) => {
                let icell = layer.hash(lon, lat) << shift;
                fn_bool(moc.contains_val(&icell))
              }
              _ => fn_bool(false),
            }
          }).collect::<Vec<R>>()
        )
      }
      _ => Err(String::from("Can't filter coos on a MOC different from a S-MOC")),
    };
    store::exec_on_one_readonly_moc(moc_index, filter)
  }

  /// Returns an array (of boolean or u8 or ...) telling if the time (in Julian Days)
  /// in the input array are in (true=1) or out of (false=0) the T-MOC of given name.
  /// # Args
  /// * `moc_index`: index of the S-MOC to be used for filtering
  /// * `jds`: iterator on decimal JD time (`f64`)
  /// # Remarks
  /// * the size of the returned array is the same as the number of elements on the input iterator.
  /// * we do not return an iterator to avoid chaining with possibly costly operations
  ///   while keeping a read lock on the store.
  /// * similarly, be careful not to use an input Iterator based on costly operations...
  pub fn filter_time_approx<T, F, R>(&self, moc_index: usize, jds_it: T, fn_bool: F) -> Result<Vec<R>, String>
    where
      T: Iterator<Item=f64>,
      F: Fn(bool) -> R
  {
    self.filter_time(
      moc_index, 
      jds_it.map(|jd| (jd * JD_TO_USEC) as u64),
      fn_bool
    )
  }

  /// Returns an array (of boolean or u8 or ...) telling if the time (in Julian Days)
  /// in the input array are in (true=1) or out of (false=0) the T-MOC of given name.
  /// # Args
  /// * `moc_index`: index of the S-MOC to be used for filtering
  /// * `jds`: iterator of times, in microsec since JD=0
  /// # Remarks
  /// * the size of the returned array is the same as the number of elements on the input iterator.
  /// * we do not return an iterator to avoid chaining with possibly costly operations
  ///   while keeping a read lock on the store.
  /// * similarly, be carefull not to use an input Iterator based on costly operations...
  pub fn filter_time<T, F, R>(&self, moc_index: usize, usec_it: T, fn_bool: F) -> Result<Vec<R>, String>
    where
      T: Iterator<Item=u64>,
      F: Fn(bool) -> R
  {
    let filter = move |moc: &InternalMoc| match moc {
      InternalMoc::Time(moc) => Ok(usec_it.map(|usec| fn_bool(moc.contains_val(&usec))).collect::<Vec<R>>()),
      _ => Err(String::from("Can't filter time on a MOC different from a T-MOC")),
    };
    store::exec_on_one_readonly_moc(moc_index, filter)
  }


  /// Returns an array (of boolean or u8 or ...) telling if the pairs of coordinates
  /// in the input slice are in (true=1) or out of (false=0) the S-MOC.
  /// # Args
  /// * `moc_index`: index of the S-MOC to be used for filtering
  /// * `coos_deg`: list of coordinates in degrees `[lon_1, lat_1, lon_2, lat_2, ..., lon_n, lat_n]`
  /// # Remarks
  /// * the size of the returned array is the same as the number of elements on the input iterator.
  /// * we do not return an iterator to avoid chaining with possibly costly operations
  ///   while keeping a read lock on the store.
  /// * similarly, be carefull not to use an input Iterator based on costly operations...
  pub fn filter_timepos_approx<T, F, R>(&self, moc_index: usize, jd_pos_it: T, fn_bool: F) -> Result<Vec<R>, String>
    where
      T: Iterator<Item=(f64, (f64, f64))>,
      F: Fn(bool) -> R
  {
    self.filter_timepos(
      moc_index,
      jd_pos_it.map(|(jd, pos)| {
        let usec = (jd * JD_TO_USEC) as u64;
        (usec, pos)
      }),
      fn_bool
    )
  }


  /// Returns an array (of boolean or u8 or ...) telling if the pairs of coordinates
  /// in the input slice are in (true=1) or out of (false=0) the S-MOC.
  /// # Args
  /// * `moc_index`: index of the S-MOC to be used for filtering
  /// * `usec_pos_it`: iterator on tuples made of a time, in microsec since JD=0, and coordinates 
  ///                  in degrees `(lon, lat)`
  /// # Remarks
  /// * the size of the returned array is the same as the number of elements on the input iterator.
  /// * we do not return an iterator to avoid chaining with possibly costly operations
  ///   while keeping a read lock on the store.
  /// * similarly, be carefull not to use an input Iterator based on costly operations...
  pub fn filter_timepos<T, F, R>(&self, moc_index: usize, usec_pos_it: T, fn_bool: F) -> Result<Vec<R>, String>
    where
      T: Iterator<Item=(u64, (f64, f64))>,
      F: Fn(bool) -> R
  {
    let layer = healpix::nested::get(Hpx::<u64>::MAX_DEPTH);
    let filter = move |moc: &InternalMoc| match moc {
      InternalMoc::TimeSpace(stmoc) => Ok(
        usec_pos_it.map(|(usec, (lon, lat))| {
            let idx = layer.hash(lon, lat);
            fn_bool(stmoc.contains_val(&usec, &idx))
          }).collect::<Vec<R>>()
      ),
      _ => Err(String::from("Can't filter time on a MOC different from a T-MOC")),
    };
    store::exec_on_one_readonly_moc(moc_index, filter)
  }

}


fn jd2mas_approx(times: Vec<f64>) -> Vec<u64> {
  let jd2mas = |t: f64| (t * 86400000000_f64).floor() as u64;
  #[cfg(not(target_arch = "wasm32"))]
  {
    times.into_par_iter()
      .map(jd2mas)
      .collect::<Vec<_>>()
  }
  #[cfg(target_arch = "wasm32")]
  {
    times.into_iter()
      .map(jd2mas)
      .collect::<Vec<_>>()
  }
}


fn lonlat2hash(depth: u8, lon: Vec<f64>, lat: Vec<f64>) -> Result<Vec<u64>, String> {
  if depth > Hpx::<u64>::MAX_DEPTH {
    Err(format!("Space depth must be in [0, {}]", Hpx::<u64>::MAX_DEPTH))
  } else if lon.len() != lat.len() {
    Err(format!("Longitudes and latitudes do not have the same size: {} != {}", lon.len(), lat.len()))
  } else {
    let mut ipix = vec![0; lon.len()];
    let layer = healpix::nested::get(depth);
    #[cfg(not(target_arch = "wasm32"))]
    ipix.par_iter_mut()
      .zip_eq(lon.into_par_iter().zip_eq(lat.into_par_iter()))
      .for_each(|(p, (l, b))| {
        *p = layer.hash(l, b);
      });
    #[cfg(target_arch = "wasm32")]
    ipix.iter_mut()
      .zip(lon.into_iter().zip(lat.into_iter()))
      .for_each(|(p, (l, b))| {
        *p = layer.hash(l, b);
      });
    Ok(ipix)
  }
}

fn times2hash(depth: u8, times_start: Vec<u64>, times_end: Vec<u64>) -> Result<Vec<Range<u64>>, String> {
  if depth > Time::<u64>::MAX_DEPTH {
    Err(format!("Time depth must be in [0, {}]", Hpx::<u64>::MAX_DEPTH))
  } else if times_start.len() != times_end.len() {
    Err(format!("Times start and end do not have the same size: {} != {}", times_start.len(), times_end.len()))
  } else {
    let mut times = vec![0..0; times_start.len()];
    #[cfg(not(target_arch = "wasm32"))]
    times.par_iter_mut()
      .zip_eq(times_start.into_par_iter().zip_eq(times_end.into_par_iter()))
      .for_each(|(t, (t1, t2))| {
        *t = t1..t2;
      });
    #[cfg(target_arch = "wasm32")]
    times.iter_mut()
      .zip(times_start.into_iter().zip(times_end.into_iter()))
      .for_each(|(t, (t1, t2))| {
        *t = t1..t2;
      });
    Ok(times)
  }
}


// See maybe https://github.com/mikaelmello/inquire
//   to build an interactive prompt ?
