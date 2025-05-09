use std::{marker::PhantomData, ops::Range};

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

use crate::{
  elemset::range::MocRanges,
  idx::Idx,
  moc::{range::RangeMocIter, RangeMOCIntoIterator, RangeMOCIterator},
  moc2d::{range::RangeMOC2Elem, RangeMOC2ElemIt, RangeMOC2Iterator},
  qty::MocQty,
  ranges::{
    ranges2d::{Ranges2D, SNORanges2D},
    Ranges,
  },
};

// Declaration of the ST-MOC and FS-MOC types made in hpxranges2d

#[derive(Debug)]
pub struct Moc2DRanges<TT, T, ST, S>
where
  TT: Idx,
  T: MocQty<TT>,
  ST: Idx,
  S: MocQty<ST>,
{
  pub ranges2d: Ranges2D<TT, ST>,
  _t_type: PhantomData<T>,
  _s_type: PhantomData<S>,
}

impl<'a, TT, T, ST, S> SNORanges2D<'a, TT, ST> for Moc2DRanges<TT, T, ST, S>
where
  TT: Idx,
  T: MocQty<TT>,
  ST: Idx,
  S: MocQty<ST>,
{
  fn make_consistent(mut self) -> Self {
    self.ranges2d = self.ranges2d.make_consistent();
    self
  }

  fn is_empty(&self) -> bool {
    self.ranges2d.is_empty()
  }

  fn contains(&self, time: TT, range: &Range<ST>) -> bool {
    self.ranges2d.contains(time, range)
  }

  fn union(&self, other: &Self) -> Self {
    self.ranges2d.union(&other.ranges2d).into()
  }

  fn intersection(&self, other: &Self) -> Self {
    self.ranges2d.intersection(&other.ranges2d).into()
  }

  fn difference(&self, other: &Self) -> Self {
    self.ranges2d.difference(&other.ranges2d).into()
  }
}

impl<TT, T, ST, S> From<Ranges2D<TT, ST>> for Moc2DRanges<TT, T, ST, S>
where
  TT: Idx,
  T: MocQty<TT>,
  ST: Idx,
  S: MocQty<ST>,
{
  fn from(ranges2d: Ranges2D<TT, ST>) -> Self {
    Moc2DRanges {
      ranges2d,
      _t_type: PhantomData,
      _s_type: PhantomData,
    }
  }
}

/*impl <TT, T, ST, S> From<Moc2DRanges<TT, T, ST, S>> for Ranges2D<TT, ST>
  where
    TT: Idx,
    T: MocQty<TT>,
    ST: Idx,
    S: MocQty<ST> {
  fn from(mocranges2d: Moc2DRanges<TT, T, ST, S>) -> Self {
    mocranges2d.ranges2d
  }
}*/

impl<TT, T, ST, S> Moc2DRanges<TT, T, ST, S>
where
  TT: Idx,
  T: MocQty<TT>,
  ST: Idx,
  S: MocQty<ST>,
{
  /// Creates a 2D coverage
  ///
  /// # Arguments
  ///
  /// * `t` - A set of ranges constituing the first dimension. This stores
  ///   usually quantities such as times, redshifts or proper motions.
  /// * `s` - A set of 1D coverage constituing the second dimension. This stores
  ///   usually space informations such as HEALPix cell indices under the nested format.
  ///
  /// # Precondition
  ///
  /// ``t`` and ``s`` must have the same length.
  pub fn new(t: Vec<Range<TT>>, s: Vec<Ranges<ST>>) -> Self {
    Self {
      ranges2d: Ranges2D::new(t, s),
      _t_type: PhantomData,
      _s_type: PhantomData,
    }
  }

  pub fn from_ranges_it<I>(it: I) -> Self
  where
    I: RangeMOC2Iterator<
      TT,
      T,
      RangeMocIter<TT, T>,
      ST,
      S,
      RangeMocIter<ST, S>,
      RangeMOC2Elem<TT, T, ST, S>,
    >,
  {
    let mut f = Vec::<Range<TT>>::new(); // 'f' for 'first'
    let mut s = Vec::<Ranges<ST>>::new(); // 's' for 'second'
    for elem in it {
      let (moc_f, moc_s) = elem.mocs();
      /* Simpler but we want to avoid the copy of the secondary_moc for the last first_range
      for range_f in moc_f.into_range_moc_iter() {
          f.push(range_f);
          s.push(moc_s.moc_ranges().ranges().clone())
      }*/
      let mut it = moc_f.into_range_moc_iter().peekable();
      while it.peek().is_some() {
        let range_t = it.next().unwrap();
        f.push(range_t);
        s.push(moc_s.moc_ranges().ranges().clone())
      }
      if let Some(range_t) = it.next() {
        f.push(range_t);
        s.push(moc_s.into_moc_ranges().into_ranges())
      }
    }
    Self::new(f, s)
  }

  pub fn from_ranges_it_gen<I, J, K, L>(it: L) -> Self
  where
    I: RangeMOCIterator<TT, Qty = T>,
    J: RangeMOCIterator<ST, Qty = S>,
    K: RangeMOC2ElemIt<TT, T, ST, S, It1 = I, It2 = J>,
    L: RangeMOC2Iterator<TT, T, I, ST, S, J, K>,
  {
    let mut f = Vec::<Range<TT>>::new(); // 'f' for 'first'
    let mut s = Vec::<Ranges<ST>>::new(); // 's' for 'second'
    for elem in it {
      let (moc_f_it, moc_s_it) = elem.range_mocs_it();
      let moc_f = moc_f_it.into_range_moc();
      let moc_s = moc_s_it.into_range_moc();
      /* Simpler but we want to avoid the copy of the s_moc for the last f_range
      for range_f in moc_f.into_range_moc_iter() {
          t.push(range_f);
          s.push(moc_s.moc_ranges().ranges().clone())
      }*/
      let mut it = moc_f.into_range_moc_iter().peekable();
      while it.peek().is_some() {
        let range_f = it.next().unwrap();
        f.push(range_f);
        s.push(moc_s.moc_ranges().ranges().clone())
      }
      if let Some(range_f) = it.next() {
        f.push(range_f);
        s.push(moc_s.into_moc_ranges().into_ranges())
      }
    }
    Self::new(f, s)
  }

  /// Compute the smallest possible depth of the coverage
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
  #[cfg(not(target_arch = "wasm32"))]
  pub fn compute_min_depth(&self) -> (u8, u8) {
    let y = self
      .ranges2d
      .y
      .par_iter()
      // Compute the depths of the Ranges<S>
      .map(|ranges| MocRanges::<ST, S>::compute_min_depth_gen(ranges))
      // Get the max of these depths
      .max()
      // If there are no ranges, the max depth
      // along the second dimension is set to 0
      .unwrap_or(0);

    // The computation is very light (logical OR), so I wonder about the the cost (overhead)
    // of the parallelization here (except for very large MOCs)...
    let x = T::compute_min_depth(
      self
        .ranges2d
        .x
        .par_iter()
        // Perform a logical 'or' between (upper and lower bounds of) all indices of the first dimension
        // then look at the trailing zeros (in the compute_min_depth method)
        .fold_with(TT::zero(), |acc, range| acc | range.start | range.end)
        .reduce(TT::zero, |a, b| a | b),
    );

    (x, y)
  }

  /// Compute the smallest possible depth of the coverage
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
  #[cfg(target_arch = "wasm32")]
  pub fn compute_min_depth(&self) -> (u8, u8) {
    let y = self
      .ranges2d
      .y
      .iter()
      // Compute the depths of the Ranges<S>
      .map(|ranges| MocRanges::<ST, S>::compute_min_depth_gen(ranges))
      // Get the max of these depths
      .max()
      // If there are no ranges, the max depth
      // along the second dimension is set to 0
      .unwrap_or(0);

    // The computation is very light (logical OR), so I wonder about the the cost (overhead)
    // of the parallelization here (except for very large MOCs)...
    let x = T::compute_min_depth(
      self
        .ranges2d
        .x
        .iter()
        // Perform a logical 'or' between (upper and lower bounds of) all indices of the first dimension
        // then look at the trailing zeros (in the compute_min_depth method)
        .fold(TT::zero(), |acc, range| acc | range.start | range.end),
    );

    (x, y)
  }
}

impl<TT, T, ST, S> PartialEq for Moc2DRanges<TT, T, ST, S>
where
  TT: Idx,
  T: MocQty<TT>,
  ST: Idx,
  S: MocQty<ST>,
{
  fn eq(&self, other: &Self) -> bool {
    self.ranges2d.eq(&other.ranges2d)
  }
}

#[cfg(test)]
mod tests {
  use std::ops::Range;

  use crate::hpxranges2d::{HpxRanges2D, TimeSpaceMoc};
  use crate::idx::Idx;
  use crate::mocranges2d::Moc2DRanges;
  use crate::qty::{Hpx, Time};
  use crate::ranges::ranges2d::SNORanges2D;
  use crate::ranges::Ranges;

  // Tests are the same as (a sub-part of) range2d test.
  // So  we basically test with the decorator (algo are already tested in rande2d).

  type TimeSpaceRanges<T, S> = Moc2DRanges<T, Time<T>, S, Hpx<S>>;

  fn time_space_moc<T: Idx, S: Idx>(
    ranges: Moc2DRanges<T, Time<T>, S, Hpx<S>>,
  ) -> TimeSpaceMoc<T, S> {
    HpxRanges2D(ranges)
  }

  fn new_time_space_moc<T: Idx, S: Idx>(t: Vec<Range<T>>, s: Vec<Ranges<S>>) -> TimeSpaceMoc<T, S> {
    time_space_moc(TimeSpaceRanges::<T, S>::new(t, s).make_consistent())
  }

  #[test]
  fn merge_overlapping_ranges() {
    let t: Vec<Range<u64>> = vec![0..15, 0..15, 15..30, 30..45, 15..30];
    let s = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage = new_time_space_moc(t, s);

    let t_expect = vec![0..15, 15..45];
    let s_expect = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage_expect = new_time_space_moc(t_expect, s_expect);
    assert_eq!(coverage, coverage_expect);
  }

  // Overlapping time ranges configuration:
  // xxxxxxxxxxx
  // xxxx-------
  #[test]
  fn remove_different_length_time_ranges() {
    let t: Vec<Range<u64>> = vec![0..7, 0..30];
    let s = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage = new_time_space_moc(t, s);

    let t_expect = vec![0..7, 7..30];
    let s_expect = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..21]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage_expect = new_time_space_moc(t_expect, s_expect);
    assert_eq!(coverage, coverage_expect);
  }

  // Overlapping time ranges configuration:
  // xxxxxxxxxxx
  // ----xxxx---
  #[test]
  fn remove_different_length_time_ranges2() {
    let t: Vec<Range<u64>> = vec![0..30, 2..10];
    let s = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage = new_time_space_moc(t, s);

    let t_expect = vec![0..2, 2..10, 10..30];
    let s_expect = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![0..4, 5..21]),
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
    ];
    let coverage_expect = new_time_space_moc(t_expect, s_expect);
    assert_eq!(coverage, coverage_expect);
  }

  // Overlapping time ranges configuration:
  // xxxxxxx----
  // ----xxxxxxx
  #[test]
  fn remove_different_length_time_ranges3() {
    let t: Vec<Range<u64>> = vec![0..5, 2..10];
    let s = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage = new_time_space_moc(t, s);

    let t_expect = vec![0..2, 2..5, 5..10];
    let s_expect = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![0..4, 5..21]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage_expect = new_time_space_moc(t_expect, s_expect);
    assert_eq!(coverage, coverage_expect);
  }

  // Overlapping time ranges configuration:
  // xxxxxxxxxxx
  // ----xxxxxxx
  #[test]
  fn remove_different_length_time_ranges4() {
    let t: Vec<Range<u64>> = vec![0..30, 10..30];
    let s = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage = new_time_space_moc(t, s);

    let t_expect = vec![0..10, 10..30];
    let s_expect = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![0..4, 5..21]),
    ];
    let coverage_expect = new_time_space_moc(t_expect, s_expect);
    assert_eq!(coverage, coverage_expect);
  }
  // No overlapping time ranges
  // xxxxxx----
  // ------xxxx
  #[test]
  fn remove_different_length_time_ranges5() {
    let t: Vec<Range<u64>> = vec![0..5, 5..20];
    let s = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage = new_time_space_moc(t, s);

    let t_expect = vec![0..5, 5..20];
    let s_expect = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage_expect = new_time_space_moc(t_expect, s_expect);
    assert_eq!(coverage, coverage_expect);
  }

  #[test]
  fn merge_overlapping_ranges_2() {
    let t: Vec<Range<u64>> = vec![0..15, 0..15, 15..30, 30..45, 15..30];
    let s = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![0..4]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
      Ranges::<u64>::new_unchecked(vec![16..21, 25..26]),
    ];
    let coverage = new_time_space_moc(t, s);

    let t_expect = vec![0..15, 15..30, 30..45];
    let s_expect = vec![
      Ranges::<u64>::new_unchecked(vec![0..4, 5..16, 17..18]),
      Ranges::<u64>::new_unchecked(vec![0..4, 16..21, 25..26]),
      Ranges::<u64>::new_unchecked(vec![16..21]),
    ];
    let coverage_expect = new_time_space_moc(t_expect, s_expect);
    assert_eq!(coverage, coverage_expect);
  }
}
