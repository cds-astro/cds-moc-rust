use std::ops::Range;

use crate::idx::Idx;
use crate::moc::{HasMaxDepth, MOCProperties, NonOverlapping, RangeMOCIterator, ZSorted};
use crate::qty::MocQty;

/// Performs a logical `OR` between the two input iterators of ranges.
pub fn overlapped_by<T, Q, I1, I2>(left_it: I1, right_it: I2) -> OverlapRangeIter<T, Q, I1, I2>
where
  T: Idx,
  Q: MocQty<T>,
  I1: RangeMOCIterator<T, Qty = Q>,
  I2: RangeMOCIterator<T, Qty = Q>,
{
  OverlapRangeIter::new(left_it, right_it)
}

/// Returns in a on-the-fly manner the ranges of `left_it` that overlap those in I2.
pub struct OverlapRangeIter<T, Q, I1, I2>
where
  T: Idx,
  Q: MocQty<T>,
  I1: RangeMOCIterator<T, Qty = Q>,
  I2: RangeMOCIterator<T, Qty = Q>,
{
  left_it: I1,
  right_it: I2,
  left: Option<Range<T>>,
  right: Option<Range<T>>,
}

impl<T, Q, I1, I2> OverlapRangeIter<T, Q, I1, I2>
where
  T: Idx,
  Q: MocQty<T>,
  I1: RangeMOCIterator<T, Qty = Q>,
  I2: RangeMOCIterator<T, Qty = Q>,
{
  fn new(mut left_it: I1, mut right_it: I2) -> OverlapRangeIter<T, Q, I1, I2> {
    let left = left_it.next();
    let right = right_it.next();

    // Quick rejection tests
    if let (Some(up_left), Some(low_right)) = (left_it.peek_last(), &right) {
      if up_left.end <= low_right.start {
        return OverlapRangeIter {
          left_it,
          right_it,
          left: None,
          right: None,
        };
      }
    }
    if let (Some(low_left), Some(up_right)) = (&left, right_it.peek_last()) {
      if up_right.end <= low_left.start {
        return OverlapRangeIter {
          left_it,
          right_it,
          left: None,
          right: None,
        };
      }
    }
    // Normal behaviour
    OverlapRangeIter {
      left_it,
      right_it,
      left,
      right,
    }
  }
}

impl<T, Q, I1, I2> HasMaxDepth for OverlapRangeIter<T, Q, I1, I2>
where
  T: Idx,
  Q: MocQty<T>,
  I1: RangeMOCIterator<T, Qty = Q>,
  I2: RangeMOCIterator<T, Qty = Q>,
{
  fn depth_max(&self) -> u8 {
    self.left_it.depth_max()
  }
}

impl<T, Q, I1, I2> ZSorted for OverlapRangeIter<T, Q, I1, I2>
where
  T: Idx,
  Q: MocQty<T>,
  I1: RangeMOCIterator<T, Qty = Q>,
  I2: RangeMOCIterator<T, Qty = Q>,
{
}
impl<T, Q, I1, I2> NonOverlapping for OverlapRangeIter<T, Q, I1, I2>
where
  T: Idx,
  Q: MocQty<T>,
  I1: RangeMOCIterator<T, Qty = Q>,
  I2: RangeMOCIterator<T, Qty = Q>,
{
}

impl<T, Q, I1, I2> MOCProperties for OverlapRangeIter<T, Q, I1, I2>
where
  T: Idx,
  Q: MocQty<T>,
  I1: RangeMOCIterator<T, Qty = Q>,
  I2: RangeMOCIterator<T, Qty = Q>,
{
}

impl<T, Q, I1, I2> Iterator for OverlapRangeIter<T, Q, I1, I2>
where
  T: Idx,
  Q: MocQty<T>,
  I1: RangeMOCIterator<T, Qty = Q>,
  I2: RangeMOCIterator<T, Qty = Q>,
{
  type Item = Range<T>;

  fn next(&mut self) -> Option<Self::Item> {
    while let (Some(l), Some(r)) = (&self.left, &self.right) {
      if l.end <= r.start {
        // |--l--| |--r--|
        self.left = self.left_it.next();
        while let Some(l) = &self.left {
          if l.end <= r.start {
            self.left = self.left_it.next();
          } else {
            break;
          }
        }
      } else if r.end <= l.start {
        // |--r--| |--l--|
        self.right = self.right_it.next();
        while let Some(r) = &self.right {
          if r.end <= l.start {
            self.right = self.right_it.next();
          } else {
            break;
          }
        }
      } else {
        // Overlapping case between l and r
        // TODO: I had to clone it because I return Range<T> but here this iterator could just return a &Range<T>
        let range = l.clone();
        self.left = self.left_it.next();
        // We return that range from left_it, it should give us all the ranges from left_it
        // that are overlapping with right_it
        return Some(range);
      }
    }
    None
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.left_it.size_hint()
  }
}

impl<T, Q, I1, I2> RangeMOCIterator<T> for OverlapRangeIter<T, Q, I1, I2>
where
  T: Idx,
  Q: MocQty<T>,
  I1: RangeMOCIterator<T, Qty = Q>,
  I2: RangeMOCIterator<T, Qty = Q>,
{
  type Qty = Q;

  fn peek_last(&self) -> Option<&Range<T>> {
    // We could have considered the case in which the upper bound is the same for both inputs
    None
  }
}

#[cfg(test)]
mod tests {
  use std::fs::File;
  use std::io::BufReader;
  use std::path::PathBuf;

  use crate::deser::fits::{from_fits_ivoa, MocIdxType, MocQtyType, MocType};
  use crate::moc::range::op::overlap::overlapped_by;
  use crate::moc::range::RangeMOC;
  use crate::moc::{CellMOCIntoIterator, CellMOCIterator, HasMaxDepth, RangeMOCIntoIterator};
  use crate::qty::Hpx;

  fn load_moc(filename: &str) -> RangeMOC<u32, Hpx<u32>> {
    let path_buf1 = PathBuf::from(format!("resources/{}", filename));
    let path_buf2 = PathBuf::from(format!("../resources/{}", filename));
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    match from_fits_ivoa(reader) {
      Ok(MocIdxType::U32(MocQtyType::Hpx(MocType::Ranges(moc)))) => {
        let moc = RangeMOC::new(moc.depth_max(), moc.collect());
        moc
      }
      Ok(MocIdxType::U32(MocQtyType::Hpx(MocType::Cells(moc)))) => {
        let moc = RangeMOC::new(moc.depth_max(), moc.into_cell_moc_iter().ranges().collect());
        moc
      }
      _ => unreachable!(),
    }
  }

  fn load_mocs() -> (RangeMOC<u32, Hpx<u32>>, RangeMOC<u32, Hpx<u32>>) {
    let sdss = load_moc("V_147_sdss12.moc.fits");
    let other = load_moc("CDS-I-125A-catalog_MOC.fits");
    (sdss, other)
  }

  // we could also perform the operation without having first collected the iteartor we obtain from
  // the FITS file
  #[test]
  fn overlap_it() {
    let (moc_l, moc_r) = load_mocs();
    let overlap_it = overlapped_by(
      (&moc_l).into_range_moc_iter(),
      (&moc_r).into_range_moc_iter(),
    );
    let overlap = RangeMOC::<u32, Hpx<u32>>::new(overlap_it.depth_max(), overlap_it.collect());

    assert_eq!(
      overlap.moc_ranges().0 .0.len() <= moc_l.moc_ranges().0 .0.len(),
      true
    );
  }
}
