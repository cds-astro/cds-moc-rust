use std::cmp::Ordering;

use crate::idx::Idx;
use crate::qty::MocQty;

use super::cell::{Cell, MocCell};
use super::cellrange::{CellRange, MocCellRange};
use super::range::MocRange;

/// The motivation for this enum is the ASCII serialization which looks like:
/// > 3/3 10 4/16-18 22 5/19-20 17/222 28/123456789 29/
/// Mixing single cells and cells range.
/// This is usefull for Qty having a DIM > 1, because at DIM = 1 a cell is only divided in 2
/// (so we get a super cell instead of a range).
/// This is not mempry efficient (size_of_u8 + 2 x size_of T + extra tag bytes) and should be
/// used in intermediary representations only (with ASCII file which are not supposed
/// to be very large).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellOrCellRange<T: Idx> {
  Cell(Cell<T>),
  CellRange(CellRange<T>),
}
impl<T: Idx> CellOrCellRange<T> {
  fn get_depth_idx_low(&self) -> (u8, &T) {
    match &self {
      CellOrCellRange::Cell(Cell { depth, idx }) => (*depth, idx),
      CellOrCellRange::CellRange(CellRange { depth, range }) => (*depth, &range.start),
    }
  }
  /// Comparison independent from the hierarchy, i.e. like a deepest level comparison.
  pub fn flat_cmp<Q: MocQty<T>>(&self, other: &Self) -> Ordering {
    let (d1, i_l) = self.get_depth_idx_low();
    let (d2, i_r) = other.get_depth_idx_low();
    match d1.cmp(&d2) {
      Ordering::Equal => i_l.cmp(i_r),
      Ordering::Less => i_l.unsigned_shl(Q::shift(d2 - d1) as u32).cmp(i_r),
      Ordering::Greater => i_l.cmp(&i_r.unsigned_shl(Q::shift(d1 - d2) as u32)),
    }
  }

  pub fn overlap<Q: MocQty<T>>(&self, other: &Self) -> bool {
    let range1: MocRange<T, Q> = self.into();
    let range2: MocRange<T, Q> = other.into();
    !(range1.0.end <= range2.0.start || range2.0.end <= range1.0.start)
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MocCellOrCellRange<T: Idx, Q: MocQty<T>> {
  MocCell(MocCell<T, Q>),
  MocCellRange(MocCellRange<T, Q>),
}
