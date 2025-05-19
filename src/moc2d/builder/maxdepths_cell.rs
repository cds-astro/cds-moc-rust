//! Builder in which we add (Time, Hpx) cells at the MOC maximum given depths.

use std::cmp::Ordering;

use crate::{
  moc::{builder::fixed_depth::FixedDepthMocBuilder, range::RangeMOC},
  moc2d::{Idx, RangeMOC2, RangeMOC2Elem},
  qty::MocQty,
};

pub struct FixedDepth2DMocBuilder<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> {
  depth_1: u8,
  depth_2: u8,
  buff: Vec<(T, U)>,
  sorted: bool,
  moc: Option<RangeMOC2<T, Q, U, R>>,
}

impl<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> FixedDepth2DMocBuilder<T, Q, U, R> {
  pub fn new(depth_1: u8, depth_2: u8, buf_capacity: Option<usize>) -> Self {
    Self {
      depth_1,
      depth_2,
      buff: Vec::with_capacity(buf_capacity.unwrap_or(100_000)),
      sorted: true,
      moc: None,
    }
  }

  pub fn push(&mut self, idx_1: T, idx_2: U) {
    if let Some((h1, h2)) = self.buff.last() {
      if *h1 == idx_1 && *h2 == idx_2 {
        return;
      } else if self.sorted && *h1 > idx_1 {
        self.sorted = false;
      }
    }
    self.buff.push((idx_1, idx_2));
    if self.buff.len() == self.buff.capacity() {
      self.drain_buffer();
    }
  }

  pub fn into_moc(mut self) -> RangeMOC2<T, Q, U, R> {
    self.drain_buffer();
    let depth_1 = self.depth_1;
    let depth_2 = self.depth_2;
    self
      .moc
      .unwrap_or_else(|| RangeMOC2::new(depth_1, depth_2, Default::default()))
  }

  fn drain_buffer(&mut self) {
    if !self.sorted {
      // Sort on the first dim
      self
        .buff
        .sort_unstable_by(|(h1_a, _), (h1_b, _)| h1_a.cmp(h1_b));
    }
    let new_moc = self.buff_to_moc();
    self.clear_buff();
    let merged_moc = if let Some(prev_moc) = &self.moc {
      prev_moc.or(&new_moc)
    } else {
      new_moc
    };
    self.moc.replace(merged_moc);
  }

  fn buff_to_moc(&self) -> RangeMOC2<T, Q, U, R> {
    // Entering here, the buffer ( buff: Vec<(T, U)> ) is sorted on T
    // Build first Vec<T, RangeMOC<U, R>>
    // Then merge successive T having the same  RangeMOC<U, R>
    let mut range_mocs: Vec<RangeMOC2Elem<T, Q, U, R>> = Vec::with_capacity(self.buff.len());

    // We assume here that the buffer is ordered, but may contains duplicates
    let mut it = self.buff.iter();
    if let Some((from_1, from_2)) = it.next() {
      let mut from_1 = *from_1;
      let from_2 = *from_2;
      let mut moc_builder_1 = FixedDepthMocBuilder::<T, Q>::new(self.depth_1, Some(64));
      moc_builder_1.push(from_1);
      let mut moc_builder_2 = FixedDepthMocBuilder::<U, R>::new(self.depth_2, Some(1000));
      moc_builder_2.push(from_2);
      let mut prev_moc_2: Option<RangeMOC<U, R>> = None;
      for (curr_1, curr_2) in it {
        match from_1.cmp(curr_1) {
          Ordering::Equal => moc_builder_2.push(*curr_2), //  No change in time => update space
          Ordering::Less => {
            // There is a change in time, build moc_2
            let moc_2 = moc_builder_2.into_moc();
            debug_assert!(!moc_2.is_empty());
            // Check whether or not the moc_2 is the same as the on in the current moc_1_builder
            if let Some(p_moc_2) = prev_moc_2.as_ref() {
              if !moc_2.eq(p_moc_2) {
                // if not create a new entry
                let p_moc_1 = moc_builder_1.into_moc();
                let p_moc_2 = prev_moc_2.replace(moc_2).unwrap();
                debug_assert!(!p_moc_1.is_empty());
                debug_assert!(!p_moc_2.is_empty());
                range_mocs.push(RangeMOC2Elem::new(p_moc_1, p_moc_2));
                moc_builder_1 = FixedDepthMocBuilder::<T, Q>::new(self.depth_1, Some(64));
                moc_builder_1.push(from_1);
              } else {
                moc_builder_1.push(from_1);
              }
            } else {
              // First loop iteration, simply set prev_moc_2
              // in such a case, the current moc_2 equals the prev_moc_2
              // current moc_builder_1 contains the current moc_1 associated to prev_moc_2
              prev_moc_2 = Some(moc_2);
              // No need to push from_1, it is already in the builder
            }
            // Update tmp variables
            moc_builder_2 = FixedDepthMocBuilder::<U, R>::new(self.depth_2, Some(1000));
            moc_builder_2.push(*curr_2);
            from_1 = *curr_1;
          }
          Ordering::Greater => unreachable!(), // self.buff supposed to be sorted!
        }
      }
      let moc_2 = moc_builder_2.into_moc();
      if let Some(p_moc_2) = prev_moc_2.as_ref() {
        if !moc_2.eq(p_moc_2) {
          // if not create a new entry
          let p_moc_1 = moc_builder_1.into_moc();
          let p_moc_2 = prev_moc_2.replace(moc_2).unwrap();
          debug_assert!(!p_moc_1.is_empty());
          debug_assert!(!p_moc_2.is_empty());
          range_mocs.push(RangeMOC2Elem::new(p_moc_1, p_moc_2));
          moc_builder_1 = FixedDepthMocBuilder::<T, Q>::new(self.depth_1, Some(64));
          moc_builder_1.push(from_1);
          let moc_1 = moc_builder_1.into_moc();
          range_mocs.push(RangeMOC2Elem::new(moc_1, prev_moc_2.unwrap()));
        } else {
          moc_builder_1.push(from_1);
          let moc_1 = moc_builder_1.into_moc();
          debug_assert!(!moc_1.is_empty());
          debug_assert!(!moc_2.is_empty());
          range_mocs.push(RangeMOC2Elem::new(moc_1, moc_2));
        }
      } else {
        let moc_1 = moc_builder_1.into_moc();
        debug_assert!(!moc_1.is_empty());
        debug_assert!(!moc_2.is_empty());
        range_mocs.push(RangeMOC2Elem::new(moc_1, moc_2));
      }
    }
    RangeMOC2::new(self.depth_1, self.depth_2, range_mocs)
  }

  fn clear_buff(&mut self) {
    self.sorted = true;
    self.buff.clear();
  }
}

#[cfg(test)]
mod tests {
  use std::io;

  use super::FixedDepth2DMocBuilder;
  use crate::{
    moc2d::{CellOrCellRangeMOC2IntoIterator, CellOrCellRangeMOC2Iterator, RangeMOC2IntoIterator},
    qty::{Frequency, Hpx},
  };

  #[test]
  fn test_build2dmoc_fixeddepth() {
    let mut builder =
      FixedDepth2DMocBuilder::<u64, Frequency<u64>, u64, Hpx<u64>>::new(10, 11, None);
    builder.push(1, 1);
    builder.push(2, 3);
    builder.push(4, 6);
    let moc2d = builder.into_moc();
    assert_eq!(moc2d.compute_n_ranges(), 6);
    /*moc2d
    .into_range_moc2_iter()
    .into_cellcellrange_moc2_iter()
    .to_ascii_ivoa(Some(80), false, io::stdout())
    .map_err(|e| e.to_string())
    .unwrap();*/
  }
}
