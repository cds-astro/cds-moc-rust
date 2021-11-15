
use std::cmp::Ordering;
use std::marker::PhantomData;

use crate::idx::Idx;
use crate::qty::{MocQty, Hpx};
use crate::elem::range::MocRange;

/// A MOC cell, i.e. an index at a given depth.
/// Without attached quantities, we do not know the shift from one depth to another, and
/// so we cannot define a absolute order.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Cell<T: Idx> {
    pub depth: u8,
    pub idx: T,
}
impl<T: Idx> Cell<T> {
    pub fn new(depth: u8, idx: T) -> Self {
        Cell {depth, idx}
    }
    /// Comparison independent from the hierarchy, i.e. like a deepest level comparison.
    pub fn flat_cmp<Q: MocQty<T>>(&self, other: &Self) -> Ordering {
        match self.depth.cmp(&other.depth) {
            Ordering::Equal => self.idx.cmp(&other.idx),
            Ordering::Less => self.idx.unsigned_shl(Q::shift(other.depth - self.depth) as u32).cmp(&other.idx),
            Ordering::Greater => self.idx.cmp(&other.idx.unsigned_shl(Q::shift(self.depth - other.depth) as u32)),
        }
    }

    /// From HEALPix standard uniq numbering (natural order follow the hierarchy, from low to high resolution)
    pub fn from_uniq_hpx(uniq: T) -> Self {
        let (depth, idx) = Hpx::<T>::from_uniq_hpx(uniq);
        Self { depth, idx }
    }
    /// To HEALPix standard uniq numbering (natural order follow the hierarchy, from low to high resolution)
    pub fn uniq_hpx(&self) -> T {
        Hpx::<T>::uniq_hpx(self.depth, self.idx)
    }

    /// From generic uniq numbering (natural order follow the hierarchy, from low to high resolution)
    pub fn from_uniq<Q: MocQty<T>>(uniq: T) -> Self {
        let (depth, idx) = Q::from_uniq_gen(uniq);
        Self { depth, idx }
    }
    /// To generic uniq numbering (natural order follow the hierarchy, from low to high resolution)
    pub fn uniq<Q: MocQty<T>>(&self) -> T {
        Q::to_uniq_gen(self.depth, self.idx)
    }

    /// From generic uniq numbering (natural order follow the z-order curve, mixing resolutions)
    pub fn from_zuniq<Q: MocQty<T>>(zuniq: T) -> Self {
        let (depth, idx) = Q::from_zuniq(zuniq);
        Self { depth, idx }
    }
    /// To generic uniq numbering (natural order follow the z-order curve, mixing resolutions)
    pub fn zuniq<Q: MocQty<T>>(&self) -> T {
        Q::to_zuniq(self.depth, self.idx)
    }

    pub fn overlap<Q: MocQty<T>>(&self, other: &Self) -> bool {
        let range1: MocRange::<T, Q> = self.into();
        let range2: MocRange::<T, Q> = other.into();
        !(range1.0.end <= range2.0.start || range2.0.end <= range1.0.start)
    }
}
impl<T: Idx, Q: MocQty<T>> From<MocCell<T, Q>> for Cell<T> {
    fn from(cell: MocCell<T, Q>) -> Self {
        cell.0
    }
}
impl<T: Idx, Q: MocQty<T>> From<&MocCell<T, Q>> for Cell<T> {
    fn from(cell: &MocCell<T, Q>) -> Self {
        cell.0
    }
}

/// The order we define corresponds to the order of the lower bound of the cell at the deepest depth.
#[repr(transparent)] // To be able to transmute Vec<Cell<T>> into Vec<MocCell<T, _>>
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MocCell<T: Idx, Q: MocQty<T>>(pub Cell<T>, PhantomData<Q>);

impl<T: Idx, Q: MocQty<T>> Ord for MocCell<T, Q> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.0.depth.cmp(&other.0.depth) {
            Ordering::Equal => self.0.idx.cmp(&other.0.idx),
            Ordering::Less => self.0.idx.unsigned_shl(Q::shift(other.0.depth - self.0.depth) as u32).cmp(&other.0.idx),
            Ordering::Greater => self.0.idx.cmp(&other.0.idx.unsigned_shl(Q::shift(self.0.depth - other.0.depth) as u32)),
        }
    }
}
impl<T: Idx, Q: MocQty<T>> PartialOrd for MocCell<T, Q> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Idx, Q: MocQty<T>> From<Cell<T>> for MocCell<T, Q> {
    fn from(cell: Cell<T>) -> Self {
        Self(cell, PhantomData)
    }
}
