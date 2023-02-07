//! Builder in which we add ranges at the maximum depth.

use std::ops::Range;

use crate::idx::Idx;
use crate::qty::MocQty;
use crate::moc::{range::RangeMOC, RangeMOCIntoIterator};
use crate::moc::range::op::{
  or::or,
  merge::merge_sorted
};

pub struct RangeMocBuilder<T: Idx, Q: MocQty<T>> {
  depth: u8,
  //one_at_new_depth: T,
  rm_bits_mask: T,
  bits_to_be_rm_mask: T,
  buff: Vec<Range<T>>,
  sorted: bool,
  moc: Option<RangeMOC<T, Q>>,
}

impl<T: Idx, Q: MocQty<T>> RangeMocBuilder<T, Q> {

  pub fn new(depth: u8, buf_capacity: Option<usize>) -> Self {
    let shift = Q::shift_from_depth_max(depth) as u32;
    //let one_at_new_depth = T::one().unsigned_shl(shift);
    let rm_bits_mask = (!T::zero()).unsigned_shl(shift);
    let bits_to_be_rm_mask = !rm_bits_mask;
    RangeMocBuilder {
      depth,
      //one_at_new_depth,
      rm_bits_mask,
      bits_to_be_rm_mask,
      buff: Vec::with_capacity(buf_capacity.unwrap_or(100_000)),
      sorted: true,
      moc: None
    }
  }

  pub fn from(buf_capacity: Option<usize>, moc: RangeMOC<T, Q>) -> Self {
    let shift = Q::shift_from_depth_max(moc.depth_max()) as u32;
    //let one_at_new_depth = T::one().unsigned_shl(shift);
    let rm_bits_mask = (!T::zero()).unsigned_shl(shift);
    let bits_to_be_rm_mask = !rm_bits_mask;
    RangeMocBuilder {
      depth: moc.depth_max(),
      //one_at_new_depth,
      rm_bits_mask,
      bits_to_be_rm_mask,
      buff: Vec::with_capacity(buf_capacity.unwrap_or(100_000)),
      sorted: true,
      moc: Some(moc)
    }
  }

  pub fn into_moc(mut self) -> RangeMOC<T, Q> {
    self.drain_buffer();
    let depth = self.depth;
    self.moc.unwrap_or_else(|| RangeMOC::new(depth, Default::default()))
  }

  pub fn push(&mut self, mut new_range: Range<T>) {
    // Degrade to the input depth to ensure consistency
    use super::super::range::op::degrade::degrade_range;
    degrade_range(&mut new_range, /*self.one_at_new_depth,*/ self.rm_bits_mask, self.bits_to_be_rm_mask);
    if let Some(Range { start, end }) = self.buff.last_mut() {
      if new_range.end < *start || *end < new_range.start {
        // both ranges do not overlap
        self.sorted &= *end < new_range.start;
        self.buff.push(new_range);
      } else {
        // merge overlaping ranges
        if new_range.start < *start {
          self.sorted = false; // we could try to look a previous ranges to merge them...
          *start = new_range.start;
        }
        if *end < new_range.end {
          *end = new_range.end
        }
      }
    } else {
      self.buff.push(new_range);
    }
    if self.buff.len() == self.buff.capacity() {
      self.drain_buffer();
    }
  }

  fn drain_buffer(&mut self) {
    if !self.sorted {
      // Sort without removing duplicates
      self.buff.sort_unstable_by(|a, b| a.start.cmp(&b.start));
    }
    let new_moc_it = merge_sorted(self.depth, self.buff.drain(..));
    self.sorted = true;
    let merged_moc = if let Some(prev_moc) = &self.moc {
      let or = or(prev_moc.into_range_moc_iter(), new_moc_it);
      RangeMOC::new(self.depth, or.collect())
    } else {
      RangeMOC::new(self.depth, new_moc_it.collect())
    };
    self.moc.replace(merged_moc);
  }

  /*fn buff_to_moc(&mut self) -> RangeMOC<T, Q> {
    self.drain_buffer();
    self.moc.take().unwrap_or_else(|| RangeMOC::new(self.depth, Default::default()))
  }*/

}

