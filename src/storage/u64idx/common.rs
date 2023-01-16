//! We are in a web page, so we load the full MOCs in memory (no streaming).
//! We also take the simple approach to work only on u64 indices
//! (possibly converting when reading or witting).

use crate::{
  qty::{MocQty, Hpx, Time, Frequency},
  moc::{
    RangeMOCIterator, RangeMOCIntoIterator, CellHpxMOCIterator,
    CellMOCIterator,
    CellOrCellRangeMOCIterator,
    range::RangeMOC,
  },
  moc2d::{
    HasTwoMaxDepth,
    RangeMOC2IntoIterator,
    CellMOC2Iterator, CellMOC2IntoIterator,
    CellOrCellRangeMOC2Iterator, CellOrCellRangeMOC2IntoIterator,
    range::RangeMOC2,
  },
  deser::fits::ranges2d_to_fits_ivoa,
};

pub(crate) const HALF_PI: f64 = 0.5 * std::f64::consts::PI;
pub(crate) const PI: f64 = std::f64::consts::PI;
pub(crate) const TWICE_PI: f64 = 2.0 * std::f64::consts::PI;

/// Convenient type for Space-MOCs
pub(crate) type SMOC = RangeMOC<u64, Hpx<u64>>;
/// Convenient type for Time-MOCs
pub(crate) type TMOC = RangeMOC<u64, Time<u64>>;
/// Convenient type for Frequency-MOCs
pub(crate) type FMOC = RangeMOC<u64, Frequency<u64>>;
/// Convenient type for SpaceTime-MOCs
pub(crate) type STMOC = RangeMOC2<u64, Time<u64>, u64, Hpx<u64>>;


pub(crate) enum InternalMoc {
  Space(SMOC),
  Time(TMOC),
  Frequency(FMOC),
  TimeSpace(STMOC),
}

impl From<SMOC> for InternalMoc {
  fn from(value: SMOC) -> Self {
    InternalMoc::Space(value)
  }
}

impl From<TMOC> for InternalMoc {
  fn from(value: TMOC) -> Self {
    InternalMoc::Time(value)
  }
}

impl From<FMOC> for InternalMoc {
  fn from(value: FMOC) -> Self {
    InternalMoc::Frequency(value)
  }
}

impl From<STMOC> for InternalMoc {
  fn from(value: STMOC) -> Self {
    InternalMoc::TimeSpace(value)
  }
}


impl InternalMoc {
  pub(crate) fn get_space_time_depths(&self) -> (Option<u8>, Option<u8>) {
    match self {
      InternalMoc::Space(moc) => (Some(moc.depth_max()), None),
      InternalMoc::Time(moc) => (None, Some(moc.depth_max())),
      InternalMoc::Frequency(_) => (None, None),
      InternalMoc::TimeSpace(moc2) => (Some(moc2.depth_max_2()), Some(moc2.depth_max_1())),
    }
  }

  pub(crate) fn get_nranges(&self) -> u32 {
    match self {
      InternalMoc::Space(moc) => moc.len() as u32,
      InternalMoc::Time(moc) => moc.len() as u32,
      InternalMoc::Frequency(moc) => moc.len() as u32,
      InternalMoc::TimeSpace(moc2) => moc2.compute_n_ranges() as u32,
    }
  }

  pub(crate) fn get_coverage_percentage(&self) -> Option<f64> {
    match self {
      InternalMoc::Space(moc) => Some(moc.coverage_percentage() * 100.0),
      InternalMoc::Time(moc) => Some(moc.coverage_percentage() * 100.0),
      InternalMoc::Frequency(moc) => Some(moc.coverage_percentage() * 100.0),
      InternalMoc::TimeSpace(_) => None,
    }
  }

  pub(crate) fn to_ascii(&self, fold: Option<usize>) -> Result<String, String> {
    let mut buf: Vec<u8> = Default::default();
    // Uses unsafe [unchecked_unwrap_ok](https://docs.rs/unreachable/1.0.0/unreachable/trait.UncheckedResultExt.html)
    // for wasm size optimisation.
    // We do it because no I/O error can occurs since we are writing in memory.
    match self {
      InternalMoc::Space(moc) =>
        moc.into_range_moc_iter()
          .cells()
          .cellranges()
          .to_ascii_ivoa(fold, false, &mut buf)
          .map_err(|e| e.to_string()),
      InternalMoc::Time(moc) =>
        moc.into_range_moc_iter()
          .cells()
          .cellranges()
          .to_ascii_ivoa(fold, false, &mut buf)
          .map_err(|e| e.to_string()),
      InternalMoc::Frequency(moc) =>
        moc.into_range_moc_iter()
          .cells()
          .cellranges()
          .to_ascii_ivoa(fold, false, &mut buf)
          .map_err(|e| e.to_string()),
      InternalMoc::TimeSpace(moc) =>
        moc.into_range_moc2_iter()
          .into_cellcellrange_moc2_iter()
          .to_ascii_ivoa(fold, false, &mut buf)
          .map_err(|e| e.to_string()),
    }.map(move |()| unsafe {
      String::from_utf8_unchecked(buf)
    })
  }

  pub(crate) fn to_json(&self, fold: Option<usize>) -> Result<String, String> {
    let mut buf: Vec<u8> = Default::default();
    // Uses unsafe [unchecked_unwrap_ok](https://docs.rs/unreachable/1.0.0/unreachable/trait.UncheckedResultExt.html)
    // for wasm size optimisation.
    // We do it because no I/O error can occurs since we are writing in memory.
    match self {
      InternalMoc::Space(moc) =>
        moc.into_range_moc_iter()
          .cells()
          .to_json_aladin(fold, &mut buf),
      InternalMoc::Time(moc) =>
        moc.into_range_moc_iter()
          .cells()
          .to_json_aladin(fold, &mut buf),
      InternalMoc::Frequency(moc) =>
        moc.into_range_moc_iter()
          .cells()
          .to_json_aladin(fold, &mut buf),
      InternalMoc::TimeSpace(moc) =>
        moc.into_range_moc2_iter()
          .into_cell_moc2_iter()
          .to_json_aladin(&fold, &mut buf),
    }.map(move |()| unsafe {
      String::from_utf8_unchecked(buf)
    }).map_err(|e| e.to_string())
  }

  /// # Params
  /// * `force_v1_compatibility`: set to `true` to save a S-MOC using NUNIQ (to be compatible with 
  ///    MOC standard v1).
  pub(crate) fn to_fits(&self, force_v1_compatibility: bool) -> Result<Box<[u8]>, String> {
    let mut buf: Vec<u8> = Default::default();
    // Uses unsafe [unchecked_unwrap_ok](https://docs.rs/unreachable/1.0.0/unreachable/trait.UncheckedResultExt.html)
    // for wasm size optimisation.
    // We do it because no I/O error can occurs since we are writing in memory.
    match self {
      InternalMoc::Space(moc) =>
        if force_v1_compatibility {
          moc.into_range_moc_iter()
            .cells()
            .hpx_cells_to_fits_ivoa(None, None, &mut buf)
        } else {
          moc.into_range_moc_iter()
            .to_fits_ivoa(None, None, &mut buf)
        },
      InternalMoc::Time(moc) =>
        moc.into_range_moc_iter()
          .to_fits_ivoa(None, None, &mut buf),
      InternalMoc::Frequency(moc) =>
        moc.into_range_moc_iter()
          .to_fits_ivoa(None, None, &mut buf),
      InternalMoc::TimeSpace(moc) =>
        ranges2d_to_fits_ivoa(moc.into_range_moc2_iter(), None, None, &mut buf),
    }.map(|()| buf.into_boxed_slice())
      .map_err(|e| e.to_string())
  }
}

pub(crate) fn check_depth<Q: MocQty<u64>>(depth: u8) -> Result<(), String> {
  if depth > Q::MAX_DEPTH {
    Err(format!("Wrong depth. Actual: {}. Expected: max {}", depth, Q::MAX_DEPTH))
  } else {
    Ok(())
  }
}

pub(crate) fn lon_deg2rad(lon_deg: f64) -> Result<f64, String> {
  let lon = lon_deg.to_radians();
  if lon < 0.0 || TWICE_PI <= lon {
    Err(String::from("Longitude must be in [0, 2pi["))
  } else {
    Ok(lon)
  }
}

pub(crate) fn lat_deg2rad(lat_deg: f64) -> Result<f64, String> {
  let lat = lat_deg.to_radians();
  if lat < -HALF_PI || HALF_PI <= lat {
    Err(String::from("Latitude must be in [-pi/2, pi/2]"))
  } else {
    Ok(lat)
  }
}

