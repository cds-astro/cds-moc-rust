//! We are in a web page, so we load the full MOCs in memory (no streaming).
//! We also take the simple approach to work only on u64 indices
//! (possibly converting when reading or witting).

use std::{
  fs::File,
  io::{BufWriter, Write},
  ops::Range,
  path::Path,
};

use crate::{
  deser::fits::ranges2d_to_fits_ivoa,
  moc::{
    range::RangeMOC, CellHpxMOCIterator, CellMOCIterator, CellOrCellRangeMOCIterator,
    RangeMOCIntoIterator, RangeMOCIterator,
  },
  moc2d::{
    range::RangeMOC2, CellMOC2IntoIterator, CellMOC2Iterator, CellOrCellRangeMOC2IntoIterator,
    CellOrCellRangeMOC2Iterator, HasTwoMaxDepth, RangeMOC2IntoIterator,
  },
  qty::{Frequency, Hpx, MocQty, Time},
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

#[derive(Copy, Clone)]
pub enum MocQType {
  Space,
  Time,
  Frequency,
  TimeSpace,
}

#[derive(Debug)]
pub(crate) enum InternalMoc {
  Space(SMOC),
  Time(TMOC),
  Frequency(FMOC),
  TimeSpace(STMOC),
}

impl PartialEq for InternalMoc {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (InternalMoc::Space(l), InternalMoc::Space(r)) => r == l,
      (InternalMoc::Time(l), InternalMoc::Time(r)) => r == l,
      (InternalMoc::Frequency(l), InternalMoc::Frequency(r)) => r == l,
      (InternalMoc::TimeSpace(l), InternalMoc::TimeSpace(r)) => r == l,
      _ => false,
    }
  }
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
  pub(crate) fn get_qty_type(&self) -> Result<MocQType, String> {
    match self {
      InternalMoc::Space(_) => Ok(MocQType::Space),
      InternalMoc::Time(_) => Ok(MocQType::Time),
      InternalMoc::Frequency(_) => Ok(MocQType::Frequency),
      InternalMoc::TimeSpace(_) => Ok(MocQType::TimeSpace),
    }
  }

  pub(crate) fn get_smoc_depth(&self) -> Result<u8, String> {
    match self {
      InternalMoc::Space(moc) => Ok(moc.depth_max()),
      InternalMoc::Time(_) => Err(String::from(
        "Wrong MOC type. Expected: Space. Actual: Time",
      )),
      InternalMoc::Frequency(_) => Err(String::from(
        "Wrong MOC type. Expected: Space. Actual: Frequency",
      )),
      InternalMoc::TimeSpace(_) => Err(String::from(
        "Wrong MOC type. Expected: Space. Actual: Space-Time",
      )),
    }
  }

  pub(crate) fn get_smoc_copy(&self) -> Result<SMOC, String> {
    match self {
      InternalMoc::Space(moc) => Ok(moc.clone()),
      InternalMoc::Time(_) => Err(String::from(
        "Wrong MOC type. Expected: Space. Actual: Time",
      )),
      InternalMoc::Frequency(_) => Err(String::from(
        "Wrong MOC type. Expected: Space. Actual: Frequency",
      )),
      InternalMoc::TimeSpace(_) => Err(String::from(
        "Wrong MOC type. Expected: Space. Actual: Space-Time",
      )),
    }
  }

  pub(crate) fn get_tmoc_depth(&self) -> Result<u8, String> {
    match self {
      InternalMoc::Space(_) => Err(String::from(
        "Wrong MOC type. Expected: Time. Actual: Space",
      )),
      InternalMoc::Time(moc) => Ok(moc.depth_max()),
      InternalMoc::Frequency(_) => Err(String::from(
        "Wrong MOC type. Expected: Time. Actual: Frequency",
      )),
      InternalMoc::TimeSpace(_) => Err(String::from(
        "Wrong MOC type. Expected: Time. Actual: Space-Time",
      )),
    }
  }

  pub(crate) fn get_fmoc_depth(&self) -> Result<u8, String> {
    match self {
      InternalMoc::Space(_) => Err(String::from(
        "Wrong MOC type. Expected: Frequency. Actual: Space",
      )),
      InternalMoc::Time(_) => Err(String::from(
        "Wrong MOC type. Expected: Frequency. Actual: Time",
      )),
      InternalMoc::Frequency(moc) => Ok(moc.depth_max()),
      InternalMoc::TimeSpace(_) => Err(String::from(
        "Wrong MOC type. Expected: Frequency. Actual: Space-Time",
      )),
    }
  }

  pub(crate) fn get_stmoc_time_and_space_depths(&self) -> Result<(u8, u8), String> {
    match self {
      InternalMoc::Space(_) => Err(String::from(
        "Wrong MOC type. Expected: Space-Time. Actual: Space",
      )),
      InternalMoc::Time(_) => Err(String::from(
        "Wrong MOC type. Expected: Space-Time. Actual: Time",
      )),
      InternalMoc::Frequency(_) => Err(String::from(
        "Wrong MOC type. Expected: Space-Time. Actual: Frequency",
      )),
      InternalMoc::TimeSpace(moc2) => Ok((moc2.depth_max_1(), moc2.depth_max_2())),
    }
  }

  pub(crate) fn is_empty(&self) -> Result<bool, String> {
    Ok(match self {
      InternalMoc::Space(moc) => moc.is_empty(),
      InternalMoc::Time(moc) => moc.is_empty(),
      InternalMoc::Frequency(moc) => moc.is_empty(),
      InternalMoc::TimeSpace(moc) => moc.is_empty(),
    })
  }

  pub(crate) fn get_n_ranges(&self) -> Result<u32, String> {
    Ok(match self {
      InternalMoc::Space(moc) => moc.len() as u32,
      InternalMoc::Time(moc) => moc.len() as u32,
      InternalMoc::Frequency(moc) => moc.len() as u32,
      InternalMoc::TimeSpace(moc2) => moc2.compute_n_ranges() as u32,
    })
  }

  pub(crate) fn get_ranges_sum(&self) -> Result<u64, String> {
    match self {
      InternalMoc::Space(moc) => Ok(moc.range_sum()),
      InternalMoc::Time(moc) => Ok(moc.range_sum()),
      InternalMoc::Frequency(moc) => Ok(moc.range_sum()),
      InternalMoc::TimeSpace(_) => Err(String::from("Range sum not implemented for ST-MOC")),
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

  pub(crate) fn get_uniq_hpx(&self) -> Result<Vec<u64>, String> {
    match self {
      InternalMoc::Space(moc) => Ok(
        moc
          .into_range_moc_iter()
          .cells()
          .map(|cell| cell.uniq_hpx())
          .collect(),
      ),
      InternalMoc::Time(_) => Err(String::from("Uniq HPX not possible with Time MOC")),
      InternalMoc::Frequency(_) => Err(String::from("Uniq HPX not possible with Frequency MOC")),
      InternalMoc::TimeSpace(_) => Err(String::from("Uniq HPX not possible with Time-Space MOCs")),
    }
  }

  pub(crate) fn get_uniq_gen(&self) -> Result<Vec<u64>, String> {
    match self {
      InternalMoc::Space(moc) => Ok(
        moc
          .into_range_moc_iter()
          .cells()
          .map(|cell| cell.uniq::<Hpx<u64>>())
          .collect(),
      ),
      InternalMoc::Time(moc) => Ok(
        moc
          .into_range_moc_iter()
          .cells()
          .map(|cell| cell.uniq::<Time<u64>>())
          .collect(),
      ),
      InternalMoc::Frequency(moc) => Ok(
        moc
          .into_range_moc_iter()
          .cells()
          .map(|cell| cell.uniq::<Frequency<u64>>())
          .collect(),
      ),
      InternalMoc::TimeSpace(_) => Err(String::from("Uniq gen not possible with Time-Space MOCs")),
    }
  }

  pub(crate) fn get_uniq_zorder(&self) -> Result<Vec<u64>, String> {
    match self {
      InternalMoc::Space(moc) => Ok(
        moc
          .into_range_moc_iter()
          .cells()
          .map(|cell| cell.zuniq::<Hpx<u64>>())
          .collect(),
      ),
      InternalMoc::Time(moc) => Ok(
        moc
          .into_range_moc_iter()
          .cells()
          .map(|cell| cell.zuniq::<Time<u64>>())
          .collect(),
      ),
      InternalMoc::Frequency(moc) => Ok(
        moc
          .into_range_moc_iter()
          .cells()
          .map(|cell| cell.zuniq::<Frequency<u64>>())
          .collect(),
      ),
      InternalMoc::TimeSpace(_) => Err(String::from(
        "Uniq Zorder not possible with Time-Space MOCs",
      )),
    }
  }

  pub(crate) fn get_ranges(&self) -> Result<Vec<Range<u64>>, String> {
    match self {
      InternalMoc::Space(moc) => Ok(moc.into_range_moc_iter().collect()),
      InternalMoc::Time(moc) => Ok(moc.into_range_moc_iter().collect()),
      InternalMoc::Frequency(moc) => Ok(moc.into_range_moc_iter().collect()),
      InternalMoc::TimeSpace(_) => Err(String::from("Get ranges not possible for Time-Space MOCs")),
    }
  }

  pub(crate) fn get_hz_ranges(&self) -> Result<Vec<Range<f64>>, String> {
    match self {
      InternalMoc::Frequency(moc) => Ok(
        moc
          .into_range_moc_iter()
          .map(|Range { start, end }| {
            Frequency::<u64>::hash2freq(start)..Frequency::<u64>::hash2freq(end)
          })
          .collect(),
      ),
      _ => Err(String::from("Get Hz ranges only available for F-MOCs")),
    }
  }

  pub(crate) fn to_ascii<W>(&self, fold: Option<usize>, writer: W) -> Result<(), String>
  where
    W: Write,
  {
    match self {
      InternalMoc::Space(moc) => moc
        .into_range_moc_iter()
        .cells()
        .cellranges()
        .to_ascii_ivoa(fold, false, writer)
        .map_err(|e| e.to_string()),
      InternalMoc::Time(moc) => moc
        .into_range_moc_iter()
        .cells()
        .cellranges()
        .to_ascii_ivoa(fold, false, writer)
        .map_err(|e| e.to_string()),
      InternalMoc::Frequency(moc) => moc
        .into_range_moc_iter()
        .cells()
        .cellranges()
        .to_ascii_ivoa(fold, false, writer)
        .map_err(|e| e.to_string()),
      InternalMoc::TimeSpace(moc) => moc
        .into_range_moc2_iter()
        .into_cellcellrange_moc2_iter()
        .to_ascii_ivoa(fold, false, writer)
        .map_err(|e| e.to_string()),
    }
  }

  pub(crate) fn to_ascii_str(&self, fold: Option<usize>) -> Result<String, String> {
    let mut buf: Vec<u8> = Default::default();
    self
      .to_ascii(fold, &mut buf)
      .map(move |()| unsafe { String::from_utf8_unchecked(buf) })
  }

  pub(crate) fn to_ascii_file<P: AsRef<Path>>(
    &self,
    destination: P,
    fold: Option<usize>,
  ) -> Result<(), String> {
    let file = File::create(destination).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    self.to_ascii(fold, writer)
  }

  pub(crate) fn to_json<W>(&self, fold: Option<usize>, writer: W) -> Result<(), String>
  where
    W: Write,
  {
    match self {
      InternalMoc::Space(moc) => moc
        .into_range_moc_iter()
        .cells()
        .to_json_aladin(fold, writer),
      InternalMoc::Time(moc) => moc
        .into_range_moc_iter()
        .cells()
        .to_json_aladin(fold, writer),
      InternalMoc::Frequency(moc) => moc
        .into_range_moc_iter()
        .cells()
        .to_json_aladin(fold, writer),
      InternalMoc::TimeSpace(moc) => moc
        .into_range_moc2_iter()
        .into_cell_moc2_iter()
        .to_json_aladin(&fold, writer),
    }
    .map_err(|e| e.to_string())
  }

  pub(crate) fn to_json_str(&self, fold: Option<usize>) -> Result<String, String> {
    let mut buf: Vec<u8> = Default::default();
    self
      .to_json(fold, &mut buf)
      .map(move |()| unsafe { String::from_utf8_unchecked(buf) })
  }

  pub(crate) fn to_json_file<P: AsRef<Path>>(
    &self,
    destination: P,
    fold: Option<usize>,
  ) -> Result<(), String> {
    let file = File::create(destination).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    self.to_json(fold, writer)
  }

  /// # Params
  /// * `force_v1_compatibility`: set to `true` to save a S-MOC using NUNIQ (to be compatible with
  ///    MOC standard v1).
  pub(crate) fn to_fits<W>(&self, force_v1_compatibility: bool, writer: W) -> Result<(), String>
  where
    W: Write,
  {
    match self {
      InternalMoc::Space(moc) => {
        if force_v1_compatibility {
          moc
            .into_range_moc_iter()
            .cells()
            .hpx_cells_to_fits_ivoa(None, None, writer)
        } else {
          moc.into_range_moc_iter().to_fits_ivoa(None, None, writer)
        }
      }
      InternalMoc::Time(moc) => moc.into_range_moc_iter().to_fits_ivoa(None, None, writer),
      InternalMoc::Frequency(moc) => moc.into_range_moc_iter().to_fits_ivoa(None, None, writer),
      InternalMoc::TimeSpace(moc) => {
        ranges2d_to_fits_ivoa(moc.into_range_moc2_iter(), None, None, writer)
      }
    }
    .map_err(|e| e.to_string())
  }

  /// # Params
  /// * `force_v1_compatibility`: set to `true` to save a S-MOC using NUNIQ (to be compatible with
  ///    MOC standard v1).
  pub(crate) fn to_fits_buff(&self, force_v1_compatibility: bool) -> Result<Box<[u8]>, String> {
    let mut buf: Vec<u8> = Default::default();
    self
      .to_fits(force_v1_compatibility, &mut buf)
      .map(|()| buf.into_boxed_slice())
  }

  /// # Params
  /// * `force_v1_compatibility`: set to `true` to save a S-MOC using NUNIQ (to be compatible with
  ///    MOC standard v1).
  pub(crate) fn to_fits_file<P: AsRef<Path>>(
    &self,
    destination: P,
    force_v1_compatibility: bool,
  ) -> Result<(), String> {
    let file = File::create(destination).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    self.to_fits(force_v1_compatibility, writer)
  }
}

pub(crate) fn check_depth<Q: MocQty<u64>>(depth: u8) -> Result<(), String> {
  if depth > Q::MAX_DEPTH {
    Err(format!(
      "Wrong depth. Actual: {}. Expected: max {}",
      depth,
      Q::MAX_DEPTH
    ))
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

pub(crate) fn lon_deg2rad_relaxed(lon_deg: f64) -> Result<f64, String> {
  let lon = lon_deg.to_radians();
  if lon < 0.0 || TWICE_PI < lon {
    Err(String::from("Longitude must be in [0, 2pi]"))
  } else {
    Ok(lon)
  }
}

pub(crate) fn lat_deg2rad(lat_deg: f64) -> Result<f64, String> {
  let lat = lat_deg.to_radians();
  if lat < -HALF_PI || HALF_PI < lat {
    Err(String::from("Latitude must be in [-pi/2, pi/2]"))
  } else {
    Ok(lat)
  }
}
