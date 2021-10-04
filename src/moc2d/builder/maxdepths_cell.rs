//! Builder in which we add (Time, Hpx) cells at the MOC maximum given depths.

use std::cmp::Ordering;

use crate::moc2d::Idx;
use crate::qty::MocQty;
use crate::moc::{
  range::RangeMOC,
  builder::fixed_depth::FixedDepthMocBuilder
};
use crate::moc2d::{RangeMOC2Elem, RangeMOC2};


pub struct FixedDepthSTMocBuilder<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> {
  depth_1: u8,
  depth_2: u8,
  buff: Vec<(T, U)>,
  sorted: bool,
  moc: Option<RangeMOC2<T, Q, U, R>>,
}

// sort buff on T.
// Build MOC on each T
// Merge consecutive T (with possible holes) IF same U-MOC (=> (T-RANGE, U-MOC))

// SUccessice UNION

impl<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> FixedDepthSTMocBuilder<T, Q, U, R> {
  pub fn new(depth_1: u8, depth_2: u8, buf_capacity: Option<usize>) -> Self {
    FixedDepthSTMocBuilder {
      depth_1,
      depth_2,
      buff: Vec::with_capacity(buf_capacity.unwrap_or(10_000)),
      sorted: true,
      moc: None
    }
  }

  pub fn push(&mut self, idx_1: T, idx_2: U) {
    if let Some((h1, h2)) = self.buff.last() {
      if *h1 == idx_1 && *h2 == idx_2  {
        return;
      } else if self.sorted && *h1 >= idx_1 && *h2 > idx_2 {
        self.sorted = false;
      }
    }
    self.buff.push((idx_1, idx_2));
    if self.buff.len() == self.buff.capacity() {
      self.drain_buffer();
    }
  }

  pub fn into_moc(mut self) -> RangeMOC2<T, Q, U, R> {
    (&mut self).drain_buffer();
    let depth_1 = self.depth_1;
    let depth_2 = self.depth_2;
    self.moc.unwrap_or_else(|| RangeMOC2::new(depth_1, depth_2, Default::default()))
  }

  fn drain_buffer(&mut self) {
    if !self.sorted {
      // Sort on the firs dim
      self.buff.sort_unstable_by(|(h1_a, _), (h1_b, _)| h1_a.cmp(h1_b));
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
    if let Some((from, from_2)) = it.next() {
      let mut from = *from;
      let from_2 = *from_2;
      let mut moc_builder_1 = FixedDepthMocBuilder::<T, Q>::new(self.depth_1, Some(64));
      // moc_builder_1.push(from);
      let mut moc_builder_2 = FixedDepthMocBuilder::<U, R>::new(self.depth_2, Some(1000));
      moc_builder_2.push(from_2);
      let mut prev_moc_2: Option<RangeMOC<U, R>> = None;
      for (curr, curr_2) in it {
        match from.cmp(curr) {
          Ordering::Equal => moc_builder_2.push(*curr_2),
          Ordering::Less => {
            // Push the previous T value in the builder
            moc_builder_1.push(from);
            // Retrieve the MOC associated to the previous T value
            let moc_2 = moc_builder_2.into_moc();
            // Check whether or not the MOC is the same as the one associated to the previous T value
            if let Some(p_moc_2) = prev_moc_2.as_ref() {
              // - if not create a new entry
              if !moc_2.eq(p_moc_2) {
                let moc_1 = moc_builder_1.into_moc();
                range_mocs.push(RangeMOC2Elem::new(moc_1, moc_2.clone()));
                prev_moc_2 = Some(moc_2);
                moc_builder_1 = FixedDepthMocBuilder::<T, Q>::new(self.depth_1, Some(64));
                moc_builder_1.push(from);
              }
            } else {
              // First loop iteration, simply set prev_moc_2
              prev_moc_2 = Some(moc_2);
            }
            // Update tmp variables
            moc_builder_2 = FixedDepthMocBuilder::<U, R>::new(self.depth_2, Some(1000));
            moc_builder_2.push(*curr_2);
            from = *curr;
          },
          Ordering::Greater => unreachable!(), // self.buff supposed to be sorted!
        }
      }
      moc_builder_1.push(from);
      let moc_1 = moc_builder_1.into_moc();
      let moc_2 = moc_builder_2.into_moc();
      range_mocs.push(RangeMOC2Elem::new(moc_1, moc_2));
    }
    RangeMOC2::new(self.depth_1, self.depth_2, range_mocs)
  }

  fn clear_buff(&mut self) {
    self.sorted = true;
    self.buff.clear();
  }
  
}