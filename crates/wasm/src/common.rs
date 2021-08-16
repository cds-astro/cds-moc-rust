//! We are in a web page, so we load the full MOCs in memory (no streaming).
//! We also take the simple approach to work only on u64 indices
//! (possibly converting when reading or witting).

use unreachable::UncheckedResultExt;

use wasm_bindgen::JsValue;

use moclib::qty::{Hpx, Time};
use moclib::moc::{
  RangeMOCIterator, RangeMOCIntoIterator,
  CellMOCIterator,
  CellOrCellRangeMOCIterator,
  range::RangeMOC
};
use moclib::moc2d::{
  HasTwoMaxDepth,
  RangeMOC2IntoIterator,
  CellMOC2Iterator, CellMOC2IntoIterator,
  CellOrCellRangeMOC2Iterator, CellOrCellRangeMOC2IntoIterator,
  range::RangeMOC2
};
use moclib::deser::fits::ranges2d_to_fits_ivoa;


use super::MocQType;

pub(crate) const HALF_PI: f64 = 0.5 * std::f64::consts::PI;
pub(crate) const PI: f64 = std::f64::consts::PI;
pub(crate) const TWICE_PI: f64 = 2.0 * std::f64::consts::PI;

/// Convenient type for Space-MOCs
pub(crate) type SMOC = RangeMOC<u64, Hpx<u64>>;
/// Convenient type for Time-MOCs
pub(crate) type TMOC = RangeMOC<u64, Time<u64>>;
/// Convenient type for SpaceTime-MOCs
pub(crate) type STMOC = RangeMOC2<u64, Time<u64>, u64, Hpx<u64>>;


pub(crate) enum InternalMoc {
  Space(SMOC),
  Time(TMOC),
  TimeSpace(STMOC),
}

impl InternalMoc {

  pub(crate) fn get_qty_type(&self) -> MocQType {
    match self {
      InternalMoc::Space(_) => MocQType::Space,
      InternalMoc::Time(_) => MocQType::Time,
      InternalMoc::TimeSpace(_) => MocQType::SpaceTime,
    }
  }

  pub(crate) fn get_space_time_depths(&self) -> (Option<u8>, Option<u8>) {
    match self {
      InternalMoc::Space(moc) => (Some(moc.depth_max()), None),
      InternalMoc::Time(moc) => (None, Some(moc.depth_max())),
      InternalMoc::TimeSpace(moc2) => (Some(moc2.depth_max_2()), Some(moc2.depth_max_1())),
    }
  }

  pub(crate) fn get_nranges(&self) -> u32 {
    match self {
      InternalMoc::Space(moc) => moc.len() as u32,
      InternalMoc::Time(moc) => moc.len() as u32,
      InternalMoc::TimeSpace(moc2) => moc2.compute_n_ranges() as u32,
    }
  }

  pub(crate) fn get_coverage_percentage(&self) -> Option<f64> {
    match self {
      InternalMoc::Space(moc) => Some(moc.coverage_percentage() * 100.0),
      InternalMoc::Time(moc) => Some(moc.coverage_percentage() * 100.0),
      InternalMoc::TimeSpace(_) => None,
    }
  }
  
  pub(crate) fn to_ascii(&self, fold: Option<usize>) -> String {
    let mut buf: Vec<u8> = Default::default();
    // Uses unsafe [unchecked_unwrap_ok](https://docs.rs/unreachable/1.0.0/unreachable/trait.UncheckedResultExt.html)
    // for wasm size optimisation.
    // We do it because no I/O error can occurs since we are writing in memory.
    unsafe {
      match self {
        InternalMoc::Space(moc) =>
          moc.into_range_moc_iter()
            .cells()
            .cellranges()
            .to_ascii_ivoa(fold, false, &mut buf)
            .unchecked_unwrap_ok(),
        InternalMoc::Time(moc) =>
          moc.into_range_moc_iter()
            .cells()
            .cellranges()
            .to_ascii_ivoa(fold, false, &mut buf)
            .unchecked_unwrap_ok(),
        InternalMoc::TimeSpace(moc) =>
          moc.into_range_moc2_iter()
            .into_cellcellrange_moc2_iter()
            .to_ascii_ivoa(fold, false, &mut buf)
            .unchecked_unwrap_ok(),
      }
    }
    unsafe {
      String::from_utf8_unchecked(buf)
    }
  }
  
  pub(crate) fn to_json(&self, fold: Option<usize>) -> String {
    let mut buf: Vec<u8> = Default::default();
    // Uses unsafe [unchecked_unwrap_ok](https://docs.rs/unreachable/1.0.0/unreachable/trait.UncheckedResultExt.html)
    // for wasm size optimisation.
    // We do it because no I/O error can occurs since we are writing in memory.
    unsafe {
      match self {
        InternalMoc::Space(moc) =>
          moc.into_range_moc_iter()
            .cells()
            .to_json_aladin(fold, &mut buf)
            .unchecked_unwrap_ok(),
        InternalMoc::Time(moc) =>
          moc.into_range_moc_iter()
            .cells()
            .to_json_aladin(fold, &mut buf)
            .unchecked_unwrap_ok(),
        InternalMoc::TimeSpace(moc) =>
          moc.into_range_moc2_iter()
            .into_cell_moc2_iter()
            .to_json_aladin(&fold, &mut buf)
            .unchecked_unwrap_ok(),
      }
    }
    unsafe {
      String::from_utf8_unchecked(buf)
    }
  }
  
  pub(crate) fn to_fits(&self) -> Box<[u8]> {
    let mut buf: Vec<u8> = Default::default();
    // Uses unsafe [unchecked_unwrap_ok](https://docs.rs/unreachable/1.0.0/unreachable/trait.UncheckedResultExt.html)
    // for wasm size optimisation.
    // We do it because no I/O error can occurs since we are writing in memory.
    unsafe {
      match self {
        InternalMoc::Space(moc) =>
          moc.into_range_moc_iter()
            .to_fits_ivoa(None, None, &mut buf)
            .unchecked_unwrap_ok(),
        InternalMoc::Time(moc) =>
          moc.into_range_moc_iter()
            .to_fits_ivoa(None, None, &mut buf)
            .unchecked_unwrap_ok(),
        InternalMoc::TimeSpace(moc) =>
          ranges2d_to_fits_ivoa(moc.into_range_moc2_iter(), None, None, &mut buf)
            .unchecked_unwrap_ok(),
      }
    }
    buf.into_boxed_slice()
  }
}

pub(crate) fn lon_deg2rad(lon_deg: f64) -> Result<f64, JsValue> {
  let lon = lon_deg.to_radians();
  if lon < 0.0 || TWICE_PI <= lon {
    Err(JsValue::from_str("Longitude must be in [0, 2pi["))
  } else {
    Ok(lon)
  }
}

pub(crate) fn lat_deg2rad(lat_deg: f64) -> Result<f64, JsValue> {
  let lat  = lat_deg.to_radians();
  if lat < -HALF_PI || HALF_PI <= lat {
    Err(JsValue::from_str("Latitude must be in [-pi/2, pi/2]"))
  } else {
    Ok(lat)
  }
}

