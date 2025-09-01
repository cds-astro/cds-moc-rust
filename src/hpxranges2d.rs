use num::One;
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use std::marker::PhantomData;
use std::{convert::From, iter::Peekable, ops::Range, slice};

use crate::{
  elemset::range::{HpxRanges, MocRanges},
  idx::Idx,
  moc::{
    range::{RangeMOC, RangeMocIter},
    CellMOCIntoIterator, CellMOCIterator, CellOrCellRangeMOCIntoIterator,
    CellOrCellRangeMOCIterator, NonOverlapping, RangeMOCIterator, ZSorted,
  },
  moc2d::{
    cell::CellMoc2Iter, cellcellrange::CellOrCellRangeMoc2Iter, range::RangeMOC2Elem,
    HasTwoMaxDepth, MOC2Properties, RangeMOC2ElemIt, RangeMOC2Iterator,
  },
  mocranges2d::Moc2DRanges,
  qty::{Frequency, Hpx, MocQty, Time},
  ranges::{
    ranges2d::{Ranges2D, SNORanges2D},
    Ranges, SNORanges,
  },
};

/// Declaration of the ST-MOC type
pub type TimeSpaceMoc<T, S> = HpxRanges2D<T, Time<T>, S>;

/// Declaration of the SF-MOC type
pub type FreqSpaceMoc<T, S> = HpxRanges2D<T, Frequency<T>, S>;

// Just to be able to define specific methods on this struct
#[derive(Debug)]
pub struct HpxRanges2D<TT: Idx, T: MocQty<TT>, ST: Idx>(pub Moc2DRanges<TT, T, ST, Hpx<ST>>);

impl<TT: Idx> TimeSpaceMoc<TT, TT> {
  pub fn time_space_iter(
    &self,
    depth_max_t: u8,
    depth_max_s: u8,
  ) -> RangeMOC2IteratorAdaptor<'_, TT, Time<TT>> {
    RangeMOC2IteratorAdaptor {
      depth_max_f: depth_max_t,
      depth_max_s,
      it_t: self.0.ranges2d.x.iter().peekable(),
      it_s: self.0.ranges2d.y.iter().peekable(),
      _phantom: PhantomData,
    }
  }

  /// Returns the smallest time value in the 2DMOC
  pub fn t_min(&self) -> Result<TT, &'static str> {
    self.first_dim_min()
  }
  /// Returns the largest time value in the 2DMOC
  pub fn t_max(&self) -> Result<TT, &'static str> {
    self.first_dim_max()
  }

  /// Returns the T-MOC of the ST-MOC elements fully covered by the given S-MOC ranges.
  pub fn time_ranges_covered_by(&self, smoc: &HpxRanges<TT>) -> MocRanges<TT, Time<TT>> {
    Self::project_on_first_dim(smoc, &self)
  }

  /// Returns the S-MOC of the ST-MOC elements intersected by the given T-MOC ranges.
  pub fn spatial_ranges_intersected_by(&self, tmoc: &MocRanges<TT, Time<TT>>) -> HpxRanges<TT> {
    Self::project_on_second_dim(tmoc, self)
  }

  // build_global_time_moc(&self)
  // build_global_smoc(&self)

  pub fn from_ranges_it<I>(it: I) -> Self
  where
    I: RangeMOC2Iterator<
      TT,
      Time<TT>,
      RangeMocIter<TT, Time<TT>>,
      TT,
      Hpx<TT>,
      RangeMocIter<TT, Hpx<TT>>,
      RangeMOC2Elem<TT, Time<TT>, TT, Hpx<TT>>,
    >,
  {
    Self(Moc2DRanges::<TT, Time<TT>, TT, Hpx<TT>>::from_ranges_it(it))
  }

  pub fn from_ranges_it_gen<I, J, K, L>(it: L) -> Self
  where
    I: RangeMOCIterator<TT, Qty = Time<TT>>,
    J: RangeMOCIterator<TT, Qty = Hpx<TT>>,
    K: RangeMOC2ElemIt<TT, Time<TT>, TT, Hpx<TT>, It1 = I, It2 = J>,
    L: RangeMOC2Iterator<TT, Time<TT>, I, TT, Hpx<TT>, J, K>,
  {
    Self(Moc2DRanges::<TT, Time<TT>, TT, Hpx<TT>>::from_ranges_it_gen(it))
  }

  pub fn create_from_times_positions(
    times: Vec<TT>,
    positions: Vec<TT>,
    time_deph: u8,
    hpx_depth: u8,
  ) -> Self {
    Self::create_from_values_and_position(times, positions, time_deph, hpx_depth)
  }

  pub fn create_from_time_ranges_positions(
    time_ranges: Vec<Range<TT>>,
    positions: Vec<TT>,
    time_deph: u8,
    hpx_depth: u8,
  ) -> Self {
    Self::create_from_ranges_and_position(time_ranges, positions, time_deph, hpx_depth)
  }

  pub fn create_from_time_ranges_spatial_coverage(
    times_ranges: Vec<Range<TT>>,
    spatial_ranges: Vec<HpxRanges<TT>>,
    time_deph: u8,
  ) -> Self {
    Self::create_from_ranges_and_spatial_coverage(times_ranges, spatial_ranges, time_deph)
  }
}

// Just to be able to define specific methods on this struct
impl<TT: Idx> FreqSpaceMoc<TT, TT> {
  pub fn freq_space_iter(
    &self,
    depth_max_f: u8,
    depth_max_s: u8,
  ) -> RangeMOC2IteratorAdaptor<'_, TT, Frequency<TT>> {
    RangeMOC2IteratorAdaptor {
      depth_max_f,
      depth_max_s,
      it_t: self.0.ranges2d.x.iter().peekable(),
      it_s: self.0.ranges2d.y.iter().peekable(),
      _phantom: PhantomData,
    }
  }

  /// Returns the smallest frequency in the 2DMOC
  pub fn f_min(&self) -> Result<TT, &'static str> {
    self.first_dim_min()
  }
  /// Returns the largest frequency in the 2DMOC
  pub fn f_max(&self) -> Result<TT, &'static str> {
    self.first_dim_max()
  }

  /// Returns the F-MOC of the SF-MOC elements fully covered by the given S-MOC ranges.
  pub fn freq_ranges_covered_by(&self, smoc: &HpxRanges<TT>) -> MocRanges<TT, Frequency<TT>> {
    Self::project_on_first_dim(smoc, &self)
  }

  /// Returns the S-MOC of the SF-MOC elements intersected by the given F-MOC ranges.
  pub fn spatial_ranges_intersected_by(
    &self,
    fmoc: &MocRanges<TT, Frequency<TT>>,
  ) -> HpxRanges<TT> {
    Self::project_on_second_dim(fmoc, self)
  }

  pub fn from_ranges_it<I>(it: I) -> Self
  where
    I: RangeMOC2Iterator<
      TT,
      Frequency<TT>,
      RangeMocIter<TT, Frequency<TT>>,
      TT,
      Hpx<TT>,
      RangeMocIter<TT, Hpx<TT>>,
      RangeMOC2Elem<TT, Frequency<TT>, TT, Hpx<TT>>,
    >,
  {
    Self(Moc2DRanges::<TT, Frequency<TT>, TT, Hpx<TT>>::from_ranges_it(it))
  }

  pub fn from_ranges_it_gen<I, J, K, L>(it: L) -> Self
  where
    I: RangeMOCIterator<TT, Qty = Frequency<TT>>,
    J: RangeMOCIterator<TT, Qty = Hpx<TT>>,
    K: RangeMOC2ElemIt<TT, Frequency<TT>, TT, Hpx<TT>, It1 = I, It2 = J>,
    L: RangeMOC2Iterator<TT, Frequency<TT>, I, TT, Hpx<TT>, J, K>,
  {
    Self(Moc2DRanges::<TT, Frequency<TT>, TT, Hpx<TT>>::from_ranges_it_gen(it))
  }

  pub fn create_from_freq_positions(
    freqs: Vec<TT>,
    positions: Vec<TT>,
    freq_deph: u8,
    hpx_depth: u8,
  ) -> Self {
    Self::create_from_values_and_position(freqs, positions, freq_deph, hpx_depth)
  }

  pub fn create_from_freq_ranges_positions(
    freq_ranges: Vec<Range<TT>>,
    positions: Vec<TT>,
    freq_deph: u8,
    hpx_depth: u8,
  ) -> Self {
    Self::create_from_ranges_and_position(freq_ranges, positions, freq_deph, hpx_depth)
  }

  pub fn create_from_freq_ranges_spatial_coverage(
    freq_ranges: Vec<Range<TT>>,
    spatial_ranges: Vec<HpxRanges<TT>>,
    freq_deph: u8,
  ) -> Self {
    Self::create_from_ranges_and_spatial_coverage(freq_ranges, spatial_ranges, freq_deph)
  }
}

impl<TT, T, ST> Default for HpxRanges2D<TT, T, ST>
where
  TT: Idx,
  T: MocQty<TT>,
  ST: Idx,
{
  /// Create a new empty `HpxRanges2D<T, S>`
  fn default() -> HpxRanges2D<TT, T, ST> {
    let ranges = Moc2DRanges::new(vec![], vec![]);
    HpxRanges2D(ranges)
  }
}

impl<TT, T, ST> HpxRanges2D<TT, T, ST>
where
  TT: Idx,
  T: MocQty<TT>,
  ST: Idx,
{
  /// Create a Quantity/Space 2D coverage
  ///
  /// # Arguments
  ///
  /// * `x` - A set of values expressed that will be converted to
  ///   ranges and degraded at the depth ``d1``.
  ///   This quantity axe may refer to a time (expressed in µs), a redshift etc...
  ///   This will define the first dimension of the coverage.
  /// * `y` - A set of spatial HEALPix cell indices at the depth ``d2``.
  ///   This will define the second dimension of the coverage.
  /// * `d1` - The depth of the coverage along its 1st dimension.
  /// * `d2` - The depth of the coverage along its 2nd dimension.
  ///
  /// The resulted 2D coverage will be of depth (``d1``, ``d2``)
  ///
  /// # Precondition
  ///
  /// - `d1` must be valid (within `[0, <T>::MAXDEPTH]`)
  /// - `d2` must be valid (within `[0, <S>::MAXDEPTH]`)
  /// - `x` and `y` must have the same size.
  #[cfg(not(target_arch = "wasm32"))]
  pub fn create_from_values_and_position(
    x: Vec<TT>,
    y: Vec<ST>,
    d1: u8,
    d2: u8,
  ) -> HpxRanges2D<TT, T, ST> {
    let s1 = T::shift_from_depth_max(d1); // ((Self::<T>::MAX_DEPTH - d1) << 1) as u32;
    let mut off1: TT = One::one();
    off1 = off1.unsigned_shl(s1 as u32) - One::one();

    let mut m1: TT = One::one();
    m1 = m1.checked_mul(&!off1).unwrap();

    let x = x
      .into_par_iter()
      .map(|r| {
        let a: TT = r & m1;
        let b: TT = r
          .checked_add(&One::one())
          .unwrap()
          .checked_add(&off1)
          .unwrap()
          & m1;
        a..b
      })
      .collect::<Vec<_>>();

    // More generic: Hpx::<ST>::shift_from_depth_max(d2)
    let s2 = ((Hpx::<ST>::MAX_DEPTH - d2) << 1) as u32;
    let y = y
      .into_par_iter()
      .map(|r| {
        let a = r.unsigned_shl(s2);
        let b = r.checked_add(&One::one()).unwrap().unsigned_shl(s2);
        // We do not want a min_depth along the 2nd dimension
        // making sure that the created Ranges<ST> is valid.
        Ranges::<ST>::new_unchecked(vec![a..b])
      })
      .collect::<Vec<_>>();

    let ranges = Ranges2D::<TT, ST>::new(x, y).make_consistent();

    HpxRanges2D(ranges.into())
  }

  /// Create a Quantity/Space 2D coverage
  ///
  /// # Arguments
  ///
  /// * `x` - A set of values expressed that will be converted to
  ///   ranges and degraded at the depth ``d1``.
  ///   This quantity axe may refer to a time (expressed in µs), a redshift etc...
  ///   This will define the first dimension of the coverage.
  /// * `y` - A set of spatial HEALPix cell indices at the depth ``d2``.
  ///   This will define the second dimension of the coverage.
  /// * `d1` - The depth of the coverage along its 1st dimension.
  /// * `d2` - The depth of the coverage along its 2nd dimension.
  ///
  /// The resulted 2D coverage will be of depth (``d1``, ``d2``)
  ///
  /// # Precondition
  ///
  /// - `d1` must be valid (within `[0, <T>::MAXDEPTH]`)
  /// - `d2` must be valid (within `[0, <S>::MAXDEPTH]`)
  /// - `x` and `y` must have the same size.
  #[cfg(target_arch = "wasm32")]
  pub fn create_from_values_and_position(
    x: Vec<TT>,
    y: Vec<ST>,
    d1: u8,
    d2: u8,
  ) -> HpxRanges2D<TT, T, ST> {
    let s1 = T::shift_from_depth_max(d1); // ((Self::<T>::MAX_DEPTH - d1) << 1) as u32;
    let mut off1: TT = One::one();
    off1 = off1.unsigned_shl(s1 as u32) - One::one();

    let mut m1: TT = One::one();
    m1 = m1.checked_mul(&!off1).unwrap();

    let x = x
      .into_iter()
      .map(|r| {
        let a: TT = r & m1;
        let b: TT = r
          .checked_add(&One::one())
          .unwrap()
          .checked_add(&off1)
          .unwrap()
          & m1;
        a..b
      })
      .collect::<Vec<_>>();

    // More generic: Hpx::<ST>::shift_from_depth_max(d2)
    let s2 = ((Hpx::<ST>::MAX_DEPTH - d2) << 1) as u32;
    let y = y
      .into_iter()
      .map(|r| {
        let a = r.unsigned_shl(s2);
        let b = r.checked_add(&One::one()).unwrap().unsigned_shl(s2);
        // We do not want a min_depth along the 2nd dimension
        // making sure that the created Ranges<ST> is valid.
        Ranges::<ST>::new_unchecked(vec![a..b])
      })
      .collect::<Vec<_>>();

    let ranges = Ranges2D::<TT, ST>::new(x, y).make_consistent();

    HpxRanges2D(ranges.into())
  }

  /// Create a Quantity/Space 2D coverage
  ///
  /// # Arguments
  ///
  /// * `x` - A set of quantity ranges that will be degraded to the depth ``d1``.
  ///   This quantity axe may refer to a time (expressed in µs), a redshift etc...
  ///   This will define the first dimension of the coverage.
  /// * `y` - A set of spatial HEALPix cell indices at the depth ``d2``.
  ///   This will define the second dimension of the coverage.
  /// * `d2` - The depth of the coverage along its 2nd dimension.
  ///
  /// The resulted 2D coverage will be of depth (``d1``, ``d2``)
  ///
  /// # Precondition
  ///
  /// - `d2` must be valid (within `[0, <S>::MAXDEPTH]`)
  /// - `x` and `y` must have the same size.
  /// - `x` must contain `[a..b]` ranges where `b > a`
  #[cfg(not(target_arch = "wasm32"))]
  pub fn create_from_ranges_and_position(
    x: Vec<Range<TT>>,
    y: Vec<ST>,
    d1: u8,
    d2: u8,
  ) -> HpxRanges2D<TT, T, ST> {
    let s1 = T::shift_from_depth_max(d1);
    let mut off1: TT = One::one();
    off1 = off1.unsigned_shl(s1 as u32) - One::one();

    let mut m1: TT = One::one();
    m1 = m1.checked_mul(&!off1).unwrap();

    let x = x
      .into_par_iter()
      .filter_map(|r| {
        let a: TT = r.start & m1;
        let b: TT = r.end.checked_add(&off1).unwrap() & m1;
        if b > a {
          Some(a..b)
        } else {
          None
        }
      })
      .collect::<Vec<_>>();

    // More generic: Hpx::<ST>::shift_from_depth_max(d2)
    let s2 = ((Hpx::<ST>::MAX_DEPTH - d2) << 1) as u32;
    let y = y
      .into_par_iter()
      .map(|r| {
        let a = r.unsigned_shl(s2);
        let b = r.checked_add(&One::one()).unwrap().unsigned_shl(s2);
        // We do not want a min_depth along the 2nd dimension
        // making sure that the created Ranges<S> is valid.
        Ranges::<ST>::new_unchecked(vec![a..b])
      })
      .collect::<Vec<_>>();

    let ranges = Moc2DRanges::<TT, T, ST, Hpx<ST>>::new(x, y).make_consistent();

    HpxRanges2D(ranges)
  }

  /// Create a Quantity/Space 2D coverage
  ///
  /// # Arguments
  ///
  /// * `x` - A set of quantity ranges that will be degraded to the depth ``d1``.
  ///   This quantity axe may refer to a time (expressed in µs), a redshift etc...
  ///   This will define the first dimension of the coverage.
  /// * `y` - A set of spatial HEALPix cell indices at the depth ``d2``.
  ///   This will define the second dimension of the coverage.
  /// * `d2` - The depth of the coverage along its 2nd dimension.
  ///
  /// The resulted 2D coverage will be of depth (``d1``, ``d2``)
  ///
  /// # Precondition
  ///
  /// - `d2` must be valid (within `[0, <S>::MAXDEPTH]`)
  /// - `x` and `y` must have the same size.
  /// - `x` must contain `[a..b]` ranges where `b > a`.
  #[cfg(target_arch = "wasm32")]
  pub fn create_from_ranges_and_position(
    x: Vec<Range<TT>>,
    y: Vec<ST>,
    d1: u8,
    d2: u8,
  ) -> HpxRanges2D<TT, T, ST> {
    let s1 = T::shift_from_depth_max(d1);
    let mut off1: TT = One::one();
    off1 = off1.unsigned_shl(s1 as u32) - One::one();

    let mut m1: TT = One::one();
    m1 = m1.checked_mul(&!off1).unwrap();

    let x = x
      .into_iter()
      .filter_map(|r| {
        let a: TT = r.start & m1;
        let b: TT = r.end.checked_add(&off1).unwrap() & m1;
        if b > a {
          Some(a..b)
        } else {
          None
        }
      })
      .collect::<Vec<_>>();

    // More generic: Hpx::<ST>::shift_from_depth_max(d2)
    let s2 = ((Hpx::<ST>::MAX_DEPTH - d2) << 1) as u32;
    let y = y
      .into_iter()
      .map(|r| {
        let a = r.unsigned_shl(s2);
        let b = r.checked_add(&One::one()).unwrap().unsigned_shl(s2);
        // We do not want a min_depth along the 2nd dimension
        // making sure that the created Ranges<S> is valid.
        Ranges::<ST>::new_unchecked(vec![a..b])
      })
      .collect::<Vec<_>>();

    let ranges = Moc2DRanges::<TT, T, ST, Hpx<ST>>::new(x, y).make_consistent();

    HpxRanges2D(ranges)
  }

  /// Create a Quantity/Space 2D coverage
  ///
  /// # Arguments
  ///
  /// * `x` - A set of quantity ranges that will be degraded to the depth ``d1``.
  ///   This quantity axe may refer to a time (expressed in µs), a redshift etc...
  ///   This will define the first dimension of the coverage.
  /// * `y` - A set of spatial HEALPix cell indices at the depth ``d2``.
  ///   This will define the second dimension of the coverage.
  /// * `d2` - The depth of the coverage along its 2nd dimension.
  ///
  /// The resulted 2D coverage will be of depth (``d1``, ``d2``)
  ///
  /// # Precondition
  ///
  /// - `d2` must be valid (within `[0, <S>::MAXDEPTH]`)
  /// - `x` and `y` must have the same size.
  /// - `x` must contain `[a..b]` ranges where `b > a`.
  pub fn create_from_ranges_and_spatial_coverage(
    x: Vec<Range<TT>>,
    y: Vec<HpxRanges<ST>>,
    d1: u8,
  ) -> HpxRanges2D<TT, T, ST> {
    let s1 = T::shift_from_depth_max(d1) as u32;
    let mut off1: TT = One::one();
    off1 = off1.unsigned_shl(s1) - One::one();

    let mut m1: TT = One::one();
    m1 = m1.checked_mul(&!off1).unwrap();

    let x = x
      .into_iter()
      .filter_map(|r| {
        let a: TT = r.start & m1;
        let b: TT = r.end.checked_add(&off1).unwrap() & m1;
        if b > a {
          Some(a..b)
        } else {
          None
        }
      })
      .collect::<Vec<_>>();

    let y = y.into_iter().map(|r| r.0).collect::<Vec<_>>();

    let ranges = Moc2DRanges::<TT, T, ST, Hpx<ST>>::new(x, y).make_consistent();

    HpxRanges2D(ranges)
  }

  /// Returns the union of the ranges along the `S` axis for which their
  /// `T` ranges intersect ``x``
  ///
  /// # Arguments
  ///
  /// * ``x``- The set of ranges along the `T` axis.
  /// * ``coverage`` - The input coverage
  ///
  /// # Algorithm
  ///
  /// This method checks for all the `T` axis ranges of ``coverage`` that
  /// lie into the range set ``x``.
  ///
  /// It then performs the union of the `S` axis ranges corresponding to the
  /// matching ranges along the `T` axis.
  pub fn project_on_second_dim(
    x: &MocRanges<TT, T>,
    coverage: &HpxRanges2D<TT, T, ST>,
  ) -> HpxRanges<ST> {
    let coverage = &coverage.0.ranges2d;

    #[cfg(not(target_arch = "wasm32"))]
    let ranges = coverage
      .x
      .par_iter()
      .zip_eq(coverage.y.par_iter())
      // Filter the time ranges to keep only those
      // that intersects with ``x``
      .filter_map(|(t, s)| {
        if x.intersects_range(t) {
          Some(s.clone())
        } else {
          None
        }
      })
      // Compute the union of all the 2nd
      // dim ranges that have been kept
      .reduce(Ranges::<ST>::default, |s1, s2| s1.union(&s2));

    #[cfg(target_arch = "wasm32")]
    let ranges = coverage
      .x
      .iter()
      .zip(coverage.y.iter())
      .filter_map(|(t, s)| {
        if x.intersects_range(t) {
          Some(s.clone())
        } else {
          None
        }
      })
      // Compute the union of all the 2nd
      // dim ranges that have been kept
      .reduce(|s1, s2| s1.union(&s2))
      .unwrap_or(Default::default());

    ranges.into()
  }

  /// Returns the union of the ranges along the `T` axis for which their
  /// `S` ranges is contained in ``y``
  ///
  /// # Arguments
  ///
  /// * ``y``- The set of ranges along the `S` axis.
  /// * ``coverage`` - The input coverage.
  ///
  /// # Algorithm
  ///
  /// This method checks for all the `S` axis ranges of ``coverage`` that
  /// lie into the range set ``y``.
  ///
  /// It then performs the union of the `T` axis ranges corresponding to the
  /// matching ranges along the `S` axis.
  pub fn project_on_first_dim(
    y: &HpxRanges<ST>,
    coverage: &HpxRanges2D<TT, T, ST>,
  ) -> MocRanges<TT, T> {
    let coverage = &coverage.0.ranges2d;
    #[cfg(not(target_arch = "wasm32"))]
    let it = coverage.x.par_iter().zip_eq(coverage.y.par_iter());
    #[cfg(target_arch = "wasm32")]
    let it = coverage.x.iter().zip(coverage.y.iter());
    let t_ranges = it
      // Filter the time ranges to keep only those
      // that lie into ``x``
      .filter_map(|(t, s)| {
        for r in s.iter() {
          if !y.contains_range(r) {
            return None;
          }
        }
        // The matching 1st dim ranges matching
        // are cloned. We do not want
        // to consume the Range2D
        Some(t.clone())
      })
      .collect::<Vec<_>>();
    // TODO: debug_assert: check is sorted!!
    MocRanges::<TT, T>::new_from_sorted(t_ranges)
  }

  /*/// Returns the union of the ranges along the `T` axis for which their
  /// `S` ranges intersect ``y``
  ///
  /// # Arguments
  ///
  /// * ``y``- The set of ranges along the `S` axis.
  /// * ``coverage`` - The input coverage.
  ///
  /// # Algorithm
  ///
  /// This method checks for all the `S` axis ranges of ``coverage`` that
  /// lie into the range set ``y``.
  ///
  /// It then performs the union of the `T` axis ranges corresponding to the
  /// matching ranges along the `S` axis.
  pub fn project_on_first_dim_v2(
      y: &HpxRanges<ST>,
      coverage: &HpxRanges2D<TT, T, ST>,
  ) -> MocRanges<TT, T> {
      let coverage = &coverage.0.ranges2d;
      let t_ranges = coverage.x.par_iter()
        .zip_eq(coverage.y.par_iter())
        // Filter the time ranges to keep only those
        // that lie into ``x``
        .filter_map(|(t, s)| {
            for r in s.iter() {
                if !y.contains(r) {
                    return None;
                }
            }
            // The matching 1st dim ranges matching
            // are cloned. We do not want
            // to consume the Range2D
            Some(t.clone())
        })
        .collect::<Vec<_>>();
      // TODO: debug_assert: check is sorted!!
      MocRanges::<TT, T>::new_from_sorted(t_ranges)
  }*/

  /// Compute the depth of the coverage
  ///
  /// # Returns
  ///
  /// A tuple containing two values:
  ///
  /// * The maximum depth along the `T` axis
  /// * The maximum depth along the `S` axis
  ///
  /// # Info
  ///
  /// If the `NestedRanges2D<T, S>` is empty, the depth returned
  /// is set to (0, 0)
  pub fn compute_min_depth(&self) -> (u8, u8) {
    self.0.compute_min_depth()
  }

  /// Returns the minimum value along the `T` dimension
  ///
  /// # Errors
  ///
  /// When the `NestedRanges2D<T, S>` is empty.
  pub fn first_dim_min(&self) -> Result<TT, &'static str> {
    if self.0.ranges2d.is_empty() {
      Err("The coverage is empty")
    } else {
      Ok(self.0.ranges2d.x[0].start)
    }
  }

  /// Returns the maximum value along the `T` dimension
  ///
  /// # Errors
  ///
  /// When the `NestedRanges2D<T, S>` is empty.
  pub fn first_dim_max(&self) -> Result<TT, &'static str> {
    if self.0.is_empty() {
      Err("The coverage is empty")
    } else {
      Ok(self.0.ranges2d.x.last().unwrap().end)
    }
  }

  /// Performs the union between two `NestedRanges2D<T, S>`
  ///
  /// # Arguments
  ///
  /// * ``other`` - The other `NestedRanges2D<T, S>` to
  ///   perform the union with.
  pub fn union(&self, other: &Self) -> Self {
    let ranges = self.0.union(&other.0);
    HpxRanges2D(ranges)
  }

  /// Performs the intersection between two `NestedRanges2D<T, S>`
  ///
  /// # Arguments
  ///
  /// * ``other`` - The other `NestedRanges2D<T, S>` to
  ///   perform the intersection with.
  pub fn intersection(&self, other: &Self) -> Self {
    let ranges = self.0.intersection(&other.0);
    HpxRanges2D(ranges)
  }

  /// Performs the difference between two `NestedRanges2D<T, S>`
  ///
  /// # Arguments
  ///
  /// * ``other`` - The other `NestedRanges2D<T, S>` to
  ///   perform the difference with.
  pub fn difference(&self, other: &Self) -> Self {
    let ranges = self.0.difference(&other.0);
    HpxRanges2D(ranges)
  }

  /// Check whether a `NestedRanges2D<T, S>` has data in
  /// a (time, ra, dec) tuple.
  ///
  /// # Arguments
  ///
  /// * ``time`` - The time of the tuple
  /// * ``range`` - The position that has been converted to a nested range
  pub fn contains(&self, time: TT, range: &Range<ST>) -> bool {
    self.0.contains(time, range)
  }

  /// Check whether a `NestedRanges2D<T, S>` is empty
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

impl<TT, T, ST> PartialEq for HpxRanges2D<TT, T, ST>
where
  TT: Idx,
  T: MocQty<TT>,
  ST: Idx,
{
  fn eq(&self, other: &Self) -> bool {
    self.0.eq(&other.0)
  }
}

impl<TT, T, ST> Eq for HpxRanges2D<TT, T, ST>
where
  TT: Idx,
  T: MocQty<TT>,
  ST: Idx,
{
}

impl From<CellOrCellRangeMoc2Iter<u64, Time<u64>, u64, Hpx<u64>>>
  for HpxRanges2D<u64, Time<u64>, u64>
{
  fn from(it: CellOrCellRangeMoc2Iter<u64, Time<u64>, u64, Hpx<u64>>) -> Self {
    let (_, upp) = it.size_hint();
    let ub = upp.unwrap_or(100);
    let mut t: Vec<Range<u64>> = Vec::with_capacity(ub);
    let mut s: Vec<Ranges<u64>> = Vec::with_capacity(ub);
    for elem in it {
      let (moc_t, moc_s) = elem.mocs();
      /* Simpler but we want to avoid the copy of the s_moc for the last t_range
      for range_t in moc_t.into_cellcellrange_moc_iter().ranges().peekable() {
          t.push(range_t);
          s.push(moc_s.moc_ranges().ranges().clone())
      }*/
      let sranges = Ranges::new_unchecked(moc_s.into_cellcellrange_moc_iter().ranges().collect());
      let mut it = moc_t.into_cellcellrange_moc_iter().ranges().peekable();
      while it.peek().is_some() {
        let range_t = it.next().unwrap();
        t.push(range_t);
        s.push(sranges.clone())
      }
      if let Some(range_t) = it.next() {
        t.push(range_t);
        s.push(sranges)
      }
    }
    HpxRanges2D(Moc2DRanges::<u64, Time<u64>, u64, Hpx<u64>>::new(t, s))
  }
}

impl From<CellMoc2Iter<u64, Time<u64>, u64, Hpx<u64>>> for HpxRanges2D<u64, Time<u64>, u64> {
  fn from(it: CellMoc2Iter<u64, Time<u64>, u64, Hpx<u64>>) -> Self {
    let (_, upp) = it.size_hint();
    let ub = upp.unwrap_or(100);
    let mut t: Vec<Range<u64>> = Vec::with_capacity(ub);
    let mut s: Vec<Ranges<u64>> = Vec::with_capacity(ub);
    for elem in it {
      let (moc_t, moc_s) = elem.mocs();
      /* Simpler but we want to avoid the copy of the s_moc for the last t_range
      for range_t in moc_t.into_cell_moc_iter().ranges().peekable() {
          t.push(range_t);
          s.push(moc_s.moc_ranges().ranges().clone())
      }*/
      let sranges = Ranges::new_unchecked(moc_s.into_cell_moc_iter().ranges().collect());
      let mut it = moc_t.into_cell_moc_iter().ranges().peekable();
      while it.peek().is_some() {
        let range_t = it.next().unwrap();
        t.push(range_t);
        s.push(sranges.clone())
      }
      if let Some(range_t) = it.next() {
        t.push(range_t);
        s.push(sranges)
      }
    }
    HpxRanges2D(Moc2DRanges::<u64, Time<u64>, u64, Hpx<u64>>::new(t, s))
  }
}

// Adaptor to write FITs
pub struct RangeMOC2IteratorAdaptor<'a, T: Idx, Q1: MocQty<T>> {
  depth_max_f: u8,
  depth_max_s: u8,
  it_t: Peekable<slice::Iter<'a, Range<T>>>,
  it_s: Peekable<slice::Iter<'a, Ranges<T>>>,
  _phantom: PhantomData<Q1>,
}
impl<'a, T: Idx, Q1: MocQty<T>> HasTwoMaxDepth for RangeMOC2IteratorAdaptor<'a, T, Q1> {
  fn depth_max_1(&self) -> u8 {
    self.depth_max_f
  }
  fn depth_max_2(&self) -> u8 {
    self.depth_max_s
  }
}
impl<'a, T: Idx, Q1: MocQty<T>> ZSorted for RangeMOC2IteratorAdaptor<'a, T, Q1> {}
impl<'a, T: Idx, Q1: MocQty<T>> NonOverlapping for RangeMOC2IteratorAdaptor<'a, T, Q1> {}
impl<'a, T: Idx, Q1: MocQty<T>> MOC2Properties for RangeMOC2IteratorAdaptor<'a, T, Q1> {}
impl<'a, T: Idx, Q1: MocQty<T>> Iterator for RangeMOC2IteratorAdaptor<'a, T, Q1> {
  type Item = RangeMOC2Elem<T, Q1, T, Hpx<T>>;
  fn next(&mut self) -> Option<Self::Item> {
    if let (Some(t_range), Some(s_ranges)) = (self.it_t.next(), self.it_s.next()) {
      let mut t = vec![t_range.clone()];
      while let Some(next_s_ranges) = self.it_s.peek() {
        if next_s_ranges == &s_ranges {
          t.push(self.it_t.next().unwrap().clone());
          self.it_s.next().unwrap();
        } else {
          break;
        }
      }
      Some(RangeMOC2Elem::new(
        RangeMOC::new(self.depth_max_f, Ranges::new_unchecked(t).into()),
        RangeMOC::new(self.depth_max_s, s_ranges.clone().into()),
      ))
    } else {
      None
    }
  }
}
impl<'a, T: Idx, Q1: MocQty<T>>
  RangeMOC2Iterator<
    T,
    Q1,
    RangeMocIter<T, Q1>,
    T,
    Hpx<T>,
    RangeMocIter<T, Hpx<T>>,
    RangeMOC2Elem<T, Q1, T, Hpx<T>>,
  > for RangeMOC2IteratorAdaptor<'a, T, Q1>
{
}
