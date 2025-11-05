use std::{cmp::Ordering, ops::Range, slice, vec::IntoIter};

use crate::qty::Frequency;
use crate::{
  idx::Idx,
  moc::{
    adapters::CellMOCIteratorFromRanges,
    range::{RangeMOC, RangeMocIter, RangeRefMocIter},
    NonOverlapping, RangeMOCIntoIterator, RangeMOCIterator, ZSorted,
  },
  moc2d::{
    builder::{
      maxdepths_cell::FixedDepth2DMocBuilder,
      maxdepths_ranges_cells::RangesAndFixedDepthCells2DMocBuilder,
    },
    CellMOC2ElemIt, CellMOC2IntoIterator, CellMOC2Iterator, HasTwoMaxDepth, MOC2Properties,
    RangeMOC2ElemIt, RangeMOC2IntoIterator, RangeMOC2Iterator,
  },
  qty::{Hpx, MocQty, Time},
};

pub mod op;

/// One element of a MOC2 made of Range elements
/// # Info
/// This implementation is not the most memory efficient since it is based on a couple of MOCs,
/// and the `max_depth` of each MOC is thus stored in each element).
/// As an alternative, we could store `MocRanges` insead of `RangeMOC`.
/// We could also replace `Vec<T>` by `Box<[T]>` in the base type `Ranges<T>`
/// TODO later if we run into memory consumption troubles.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeMOC2Elem<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> {
  moc_l: RangeMOC<T, Q>, // or (do not contains the depth): MocRanges<T, Q>,
  moc_r: RangeMOC<U, R>, // or (do not contains the depth): MocRanges<U, R>,
}
impl<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> RangeMOC2Elem<T, Q, U, R> {
  pub fn new(moc_l: RangeMOC<T, Q>, moc_r: RangeMOC<U, R>) -> Self {
    Self { moc_l, moc_r }
  }

  /// Returns the number of ranges in the first dimension
  pub fn n_ranges_1(&self) -> usize {
    self.moc_l.len()
  }
  /// Returns the number of ranges in the second dimension
  pub fn n_ranges_2(&self) -> usize {
    self.moc_r.len()
  }
  /// Returns the number of ranges in both quantities
  pub fn n_ranges(&self) -> u64 {
    self.n_ranges_1() as u64 + self.n_ranges_2() as u64
  }

  pub fn mocs(self) -> (RangeMOC<T, Q>, RangeMOC<U, R>) {
    (self.moc_l, self.moc_r)
  }

  pub fn eq_without_depth(&self, rhs: &Self) -> bool {
    self.moc_l.eq_without_depth(&rhs.moc_l) && self.moc_r.eq_without_depth(&rhs.moc_r)
  }

  pub fn first_index_left(&self) -> Option<T> {
    self.moc_l.first_index()
  }

  pub fn last_index_left(&self) -> Option<T> {
    self.moc_l.last_index()
  }

  /// The values are the values at Q::MAX_DEPTH and R::MAX_DEPTH respectively
  pub fn contains_val(&self, val_left: &T, val_right: &U) -> bool {
    self.moc_l.contains_val(val_left) && self.moc_r.contains_val(val_right)
  }
}
/*impl<T, Q, U, R> PartialEq for RangeMOC2Elem<T, Q, U, R> {
  where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>

  fn eq(&self, other: &Self) -> bool {
    self.depth_max == other.depth_max
      && self.ranges.eq(&other.ranges)
  }
}*/

impl<T, Q, U, R> RangeMOC2ElemIt<T, Q, U, R> for RangeMOC2Elem<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  type It1 = RangeMocIter<T, Q>;
  type It2 = RangeMocIter<U, R>;
  fn range_mocs_it(self) -> (Self::It1, Self::It2) {
    (
      self.moc_l.into_range_moc_iter(),
      self.moc_r.into_range_moc_iter(),
    )
  }
}
impl<'a, T, Q, U, R> RangeMOC2ElemIt<T, Q, U, R> for &'a RangeMOC2Elem<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  type It1 = RangeRefMocIter<'a, T, Q>;
  type It2 = RangeRefMocIter<'a, U, R>;
  fn range_mocs_it(self) -> (Self::It1, Self::It2) {
    (
      (&self.moc_l).into_range_moc_iter(),
      (&self.moc_r).into_range_moc_iter(),
    )
  }
}

impl<T, Q, U, R> CellMOC2ElemIt<T, Q, U, R> for RangeMOC2Elem<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  type It1 = CellMOCIteratorFromRanges<T, Q, RangeMocIter<T, Q>>;
  type It2 = CellMOCIteratorFromRanges<U, R, RangeMocIter<U, R>>;
  fn cell_mocs_it(self) -> (Self::It1, Self::It2) {
    (
      self.moc_l.into_range_moc_iter().cells(),
      self.moc_r.into_range_moc_iter().cells(),
    )
  }
}
impl<'a, T, Q, U, R> CellMOC2ElemIt<T, Q, U, R> for &'a RangeMOC2Elem<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  type It1 = CellMOCIteratorFromRanges<T, Q, RangeRefMocIter<'a, T, Q>>;
  type It2 = CellMOCIteratorFromRanges<U, R, RangeRefMocIter<'a, U, R>>;
  fn cell_mocs_it(self) -> (Self::It1, Self::It2) {
    (
      (&self.moc_l).into_range_moc_iter().cells(),
      (&self.moc_r).into_range_moc_iter().cells(),
    )
  }
}

/// A MOC2 made of Range elements
#[derive(Debug, Clone, PartialEq)]
pub struct RangeMOC2<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> {
  depth_max_l: u8,
  depth_max_r: u8, // not in vmoc. Really useful?
  elems: Vec<RangeMOC2Elem<T, Q, U, R>>,
}
impl<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> RangeMOC2<T, Q, U, R> {
  pub fn new(depth_max_l: u8, depth_max_r: u8, elems: Vec<RangeMOC2Elem<T, Q, U, R>>) -> Self {
    Self {
      depth_max_l: depth_max_l.min(Q::MAX_DEPTH),
      depth_max_r: depth_max_r.min(R::MAX_DEPTH),
      elems,
    }
  }

  pub fn is_empty(&self) -> bool {
    self.elems.is_empty()
  }

  pub fn min_index_left(&self) -> Option<T> {
    self.elems.first().and_then(|elem| elem.first_index_left())
  }

  pub fn max_index_left(&self) -> Option<T> {
    self.elems.last().and_then(|elem| elem.last_index_left())
  }

  pub fn global_range_left(&self) -> Option<Range<T>> {
    match (self.min_index_left(), self.max_index_left()) {
      (Some(min), Some(max)) => Some(min..max),
      _ => None,
    }
  }

  pub fn eq_without_depth(&self, rhs: &Self) -> bool {
    if self.elems.len() != rhs.elems.len() {
      return false;
    } else {
      for (l, r) in self.elems.iter().zip(rhs.elems.iter()) {
        if !l.eq_without_depth(r) {
          return false;
        }
      }
    }
    true
  }

  /// From a list of cells in both dim 1 and dim 2.
  pub fn from_fixed_depth_cells<I: Iterator<Item = (T, U)>>(
    depth_1: u8,
    depth_2: u8,
    cells_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    let mut builder = FixedDepth2DMocBuilder::new(depth_1, depth_2, buf_capacity);
    for (cell_1, cell_2) in cells_it {
      builder.push(cell_1, cell_2);
    }
    builder.into_moc()
  }

  /// From al list of tuple containing both a range in dim 1 and a cell in dim 2.
  pub fn from_ranges_and_fixed_depth_cells<I: Iterator<Item = (Range<T>, U)>>(
    depth_1: u8,
    depth_2: u8,
    cells_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    let mut builder = RangesAndFixedDepthCells2DMocBuilder::new(depth_1, depth_2, buf_capacity);
    for (range_1, cell_2) in cells_it {
      builder.push(range_1, cell_2);
    }
    builder.into_moc()
  }

  /// The total number of ranges in both dimensions
  pub fn compute_n_ranges(&self) -> u64 {
    self.elems.iter().map(|e| e.n_ranges()).sum()
  }

  /// The values are the values at Q::MAX_DEPTH and R::MAX_DEPTH respectively
  pub fn contains_val(&self, val_left: &T, val_right: &U) -> bool {
    let comp_res = self.elems.binary_search_by(|elem| {
      match (elem.moc_l.first_index(), elem.moc_l.last_index()) {
        (Some(start), Some(end)) => {
          if *val_left < start {
            Ordering::Greater
          } else if *val_left >= end {
            Ordering::Less
          } else {
            Ordering::Equal
          }
        }
        _ => Ordering::Greater,
      }
    });
    match comp_res {
      Ok(i) => self.elems[i].contains_val(val_left, val_right),
      _ => false,
    }
  }

  /// So far the internal code resort to the code to perform operation on iterator.
  /// TODO: make a more performan code based on the particular RangeMOC2 type?
  pub fn into_or(self, rhs: RangeMOC2<T, Q, U, R>) -> RangeMOC2<T, Q, U, R> {
    op::or::or(self.into_range_moc2_iter(), rhs.into_range_moc2_iter()).into_range_moc2()
  }
  /// So far the internal code resort to the code to perform operation on iterator.
  /// TODO: make a more performant code based on the particular RangeMOC2 type?
  pub fn or(&self, rhs: &RangeMOC2<T, Q, U, R>) -> RangeMOC2<T, Q, U, R> {
    op::or::or(self.into_range_moc2_iter(), rhs.into_range_moc2_iter()).into_range_moc2()
  }
}

impl RangeMOC2<u64, Time<u64>, u64, Hpx<u64>> {
  pub fn new_empty(depth_time: u8, depth_hpx: u8) -> Self {
    Self::new(depth_time, depth_hpx, Default::default())
  }

  pub fn from_time_and_coos<I: Iterator<Item = (u64, f64, f64)>>(
    depth_time: u8,
    depth_hpx: u8,
    val_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    let shift = Time::<u64>::shift_from_depth_max(depth_time);
    let layer = healpix::nested::get(depth_hpx);
    Self::from_fixed_depth_cells(
      depth_time,
      depth_hpx,
      val_it.map(move |(us_since_jd0, lon_rad, lat_rad)| {
        (us_since_jd0 >> shift, layer.hash(lon_rad, lat_rad))
      }),
      buf_capacity,
    )
  }
}

impl RangeMOC2<u64, Frequency<u64>, u64, Hpx<u64>> {
  pub fn new_empty(depth_freq: u8, depth_hpx: u8) -> Self {
    Self::new(depth_freq, depth_hpx, Default::default())
  }

  /// Frequency in Hz
  /// (Lon, Lat) in radians
  pub fn from_freq_in_hz_and_coos<I: Iterator<Item = (f64, (f64, f64))>>(
    depth_freq: u8,
    depth_hpx: u8,
    val_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    let shift = Frequency::<u64>::shift_from_depth_max(depth_freq);
    let layer = healpix::nested::get(depth_hpx);
    Self::from_fixed_depth_cells(
      depth_freq,
      depth_hpx,
      val_it.map(move |(freq_hz, (lon_rad, lat_rad))| {
        (
          Frequency::<u64>::freq2hash(freq_hz) >> shift,
          layer.hash(lon_rad, lat_rad),
        )
      }),
      buf_capacity,
    )
  }

  /// Frequency in Hz
  /// (Lon, Lat) in radians
  pub fn from_freqranges_in_hz_and_coos<I: Iterator<Item = (Range<f64>, (f64, f64))>>(
    depth_freq: u8,
    depth_hpx: u8,
    val_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    let layer = healpix::nested::get(depth_hpx);
    Self::from_ranges_and_fixed_depth_cells(
      depth_freq,
      depth_hpx,
      val_it.map(move |(freq_hz_range, (lon_rad, lat_rad))| {
        (
          Frequency::<u64>::freq2hash(freq_hz_range.start)
            ..Frequency::<u64>::freq2hash_accept_freqmax(freq_hz_range.end),
          layer.hash(lon_rad, lat_rad),
        )
      }),
      buf_capacity,
    )
  }
}

impl<T, Q, U, R> HasTwoMaxDepth for RangeMOC2<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  fn depth_max_1(&self) -> u8 {
    self.depth_max_l
  }
  fn depth_max_2(&self) -> u8 {
    self.depth_max_r
  }
}
impl<T, Q, U, R> ZSorted for RangeMOC2<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}
impl<T, Q, U, R> NonOverlapping for RangeMOC2<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}
impl<T, Q, U, R> MOC2Properties for RangeMOC2<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}

/// Iterator taking the ownership of a MOC2 made of Range elements
pub struct RangeMoc2Iter<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  depth_max_l: u8,
  depth_max_r: u8,
  iter: IntoIter<RangeMOC2Elem<T, Q, U, R>>,
}
impl<T, Q, U, R> HasTwoMaxDepth for RangeMoc2Iter<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  fn depth_max_1(&self) -> u8 {
    self.depth_max_l
  }
  fn depth_max_2(&self) -> u8 {
    self.depth_max_r
  }
}
impl<T, Q, U, R> ZSorted for RangeMoc2Iter<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}
impl<T, Q, U, R> NonOverlapping for RangeMoc2Iter<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}
impl<T, Q, U, R> MOC2Properties for RangeMoc2Iter<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}
impl<T, Q, U, R> Iterator for RangeMoc2Iter<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  type Item = RangeMOC2Elem<T, Q, U, R>;
  fn next(&mut self) -> Option<Self::Item> {
    self.iter.next()
  }
}

impl<T, Q, U, R>
  RangeMOC2Iterator<T, Q, RangeMocIter<T, Q>, U, R, RangeMocIter<U, R>, RangeMOC2Elem<T, Q, U, R>>
  for RangeMoc2Iter<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}

impl<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>>
  RangeMOC2IntoIterator<
    T,
    Q,
    RangeMocIter<T, Q>,
    U,
    R,
    RangeMocIter<U, R>,
    RangeMOC2Elem<T, Q, U, R>,
  > for RangeMOC2<T, Q, U, R>
{
  type IntoRangeMOC2Iter = RangeMoc2Iter<T, Q, U, R>;

  fn into_range_moc2_iter(self) -> Self::IntoRangeMOC2Iter {
    RangeMoc2Iter {
      depth_max_l: self.depth_max_l,
      depth_max_r: self.depth_max_r,
      iter: self.elems.into_iter(),
    }
  }
}

impl<T, Q, U, R>
  CellMOC2Iterator<
    T,
    Q,
    CellMOCIteratorFromRanges<T, Q, RangeMocIter<T, Q>>,
    U,
    R,
    CellMOCIteratorFromRanges<U, R, RangeMocIter<U, R>>,
    RangeMOC2Elem<T, Q, U, R>,
  > for RangeMoc2Iter<T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}

impl<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>>
  CellMOC2IntoIterator<
    T,
    Q,
    CellMOCIteratorFromRanges<T, Q, RangeMocIter<T, Q>>,
    U,
    R,
    CellMOCIteratorFromRanges<U, R, RangeMocIter<U, R>>,
    RangeMOC2Elem<T, Q, U, R>,
  > for RangeMOC2<T, Q, U, R>
{
  type IntoCellMOC2Iter = RangeMoc2Iter<T, Q, U, R>;

  fn into_cell_moc2_iter(self) -> Self::IntoCellMOC2Iter {
    RangeMoc2Iter {
      depth_max_l: self.depth_max_l,
      depth_max_r: self.depth_max_r,
      iter: self.elems.into_iter(),
    }
  }
}

/*impl<T, Q, U, R> CellMOC2Iterator<
  T, Q, CellMOCIteratorFromRanges<T, Q, RangeMocIter<T, Q>>,
  U, R, CellMOCIteratorFromRanges<U, R, RangeMocIter<U, R>>,
  RangeMOC2Elem<T, Q, U, R>
> for RangeMoc2Iter<T, Q, U, R>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U> {}

impl<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>>
CellMOC2IntoIterator<
  T, Q, CellMOCIteratorFromRanges<T, Q, RangeMocIter<T, Q>>,
  U, U, CellMOCIteratorFromRanges<U, R, RangeMocIter<U, R>>,
  RangeMOC2ElemIt<
    T, Q, U, R,
    It1=CellMOCIteratorFromRanges<T, Q, RangeMocIter<T, Q>>,
    It2=CellMOCIteratorFromRanges<U, R, RangeMocIter<U, R>>
  >
> for RangeMOC2<T, Q, U, R> {
  type IntoCellMOC2Iter = RangeMoc2Iter<T, Q, U, R>;

  fn into_cell_moc2_iter(self) -> Self::IntoCellMOC2Iter {
    RangeMoc2Iter {
      depth_max_l: self.depth_max_l,
      depth_max_r: self.depth_max_r,
      iter: self.elems.into_iter()
    }
  }
}*/

/// Iterator borrowing a MOC2 made of Range elements
pub struct RangeRefMoc2Iter<'a, T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  depth_max_l: u8,
  depth_max_r: u8,
  iter: slice::Iter<'a, RangeMOC2Elem<T, Q, U, R>>,
}
impl<'a, T, Q, U, R> HasTwoMaxDepth for RangeRefMoc2Iter<'a, T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  fn depth_max_1(&self) -> u8 {
    self.depth_max_l
  }
  fn depth_max_2(&self) -> u8 {
    self.depth_max_r
  }
}
impl<'a, T, Q, U, R> ZSorted for RangeRefMoc2Iter<'a, T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}
impl<'a, T, Q, U, R> NonOverlapping for RangeRefMoc2Iter<'a, T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}
impl<'a, T, Q, U, R> MOC2Properties for RangeRefMoc2Iter<'a, T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}
impl<'a, T, Q, U, R> Iterator for RangeRefMoc2Iter<'a, T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
  type Item = &'a RangeMOC2Elem<T, Q, U, R>;
  fn next(&mut self) -> Option<Self::Item> {
    self.iter.next()
  }
}

impl<'a, T, Q, U, R>
  RangeMOC2Iterator<
    T,
    Q,
    RangeRefMocIter<'a, T, Q>,
    U,
    R,
    RangeRefMocIter<'a, U, R>,
    &'a RangeMOC2Elem<T, Q, U, R>,
  > for RangeRefMoc2Iter<'a, T, Q, U, R>
where
  T: Idx,
  Q: MocQty<T>,
  U: Idx,
  R: MocQty<U>,
{
}

impl<'a, T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>>
  RangeMOC2IntoIterator<
    T,
    Q,
    RangeRefMocIter<'a, T, Q>,
    U,
    R,
    RangeRefMocIter<'a, U, R>,
    &'a RangeMOC2Elem<T, Q, U, R>,
  > for &'a RangeMOC2<T, Q, U, R>
{
  type IntoRangeMOC2Iter = RangeRefMoc2Iter<'a, T, Q, U, R>;
  fn into_range_moc2_iter(self) -> Self::IntoRangeMOC2Iter {
    RangeRefMoc2Iter {
      depth_max_l: self.depth_max_l,
      depth_max_r: self.depth_max_r,
      iter: self.elems.iter(),
    }
  }
}

#[cfg(test)]
mod tests {

  use super::*;

  use crate::{
    deser::ascii::moc2d_from_ascii_ivoa,
    moc2d::{CellOrCellRangeMOC2IntoIterator, RangeMOC2IntoIterator},
    qty::{Frequency, Hpx},
  };

  #[test]
  fn test_from_freq_in_hz_and_coos() {
    let deg1 = 1.0_f64.to_radians();
    let deg2 = 2.0_f64.to_radians();
    let elems = [
      (0.01_f64, (0.0, 0.0)),
      (2.0_f64, (deg1, deg1)),
      (300.0_f64, (deg2, deg2)),
    ];
    let moc2 = RangeMOC2::<u64, Frequency<u64>, u64, Hpx<u64>>::from_freq_in_hz_and_coos(
      20,
      12,
      elems.into_iter(),
      None,
    );

    let moc2ascii = moc2d_from_ascii_ivoa::<u64, Frequency<u64>, u64, Hpx<u64>>(
      r#"
f20/599186 
s12/79691776 
f20/685357 
s12/79697029 
f20/766850 
s12/79712788 
f20/ s12/
"#,
    )
    .map(|cellrange2| {
      cellrange2
        .into_cellcellrange_moc2_iter()
        .into_range_moc2_iter()
        .into_range_moc2()
    })
    .unwrap();
    assert_eq!(moc2, moc2ascii);

    /*use crate::moc2d::CellOrCellRangeMOC2Iterator;
    moc2
      .into_range_moc2_iter()
      .into_cellcellrange_moc2_iter()
      .to_ascii_ivoa(Some(80), false, std::io::stdout())
      .map_err(|e| e.to_string())
      .unwrap();*/
  }
}
