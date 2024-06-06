use std::{marker::PhantomData, ops::Range};

/// Re-export `Ordinal` not to be out-of-sync with cdshealpix version.
pub use healpix::compass_point::{Ordinal, OrdinalMap, OrdinalSet};

use crate::{
  elemset::range::BorrowedMocRanges,
  idx::Idx,
  moc::{range::RangeRefMocIter, HasMaxDepth, NonOverlapping, RangeMOCIntoIterator, ZSorted},
  qty::MocQty,
};

pub struct BorrowedRangeMOC<'a, T: Idx, Q: MocQty<T>> {
  depth_max: u8,
  ranges: BorrowedMocRanges<'a, T, Q>,
}

impl<'a, T: Idx, Q: MocQty<T>> BorrowedRangeMOC<'a, T, Q> {
  pub fn new(depth_max: u8, ranges: BorrowedMocRanges<'a, T, Q>) -> Self {
    Self { depth_max, ranges }
  }
}

impl<'a, T: Idx, Q: MocQty<T>> HasMaxDepth for BorrowedRangeMOC<'a, T, Q> {
  fn depth_max(&self) -> u8 {
    self.depth_max
  }
}
impl<'a, T: Idx, Q: MocQty<T>> ZSorted for BorrowedRangeMOC<'a, T, Q> {}
impl<'a, T: Idx, Q: MocQty<T>> NonOverlapping for BorrowedRangeMOC<'a, T, Q> {}

impl<'a, T: Idx, Q: MocQty<T>> RangeMOCIntoIterator<T> for BorrowedRangeMOC<'a, T, Q> {
  type Qty = Q;
  type IntoRangeMOCIter = RangeRefMocIter<'a, T, Self::Qty>;

  fn into_range_moc_iter(self) -> Self::IntoRangeMOCIter {
    let l = self.ranges.0 .0.len();
    let last: Option<Range<T>> = if l > 0 {
      Some(self.ranges.0 .0[l - 1].clone())
    } else {
      None
    };
    RangeRefMocIter {
      depth_max: self.depth_max,
      iter: self.ranges.0 .0.iter(),
      last,
      _qty: PhantomData,
    }
  }
}
