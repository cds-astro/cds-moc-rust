
use std::mem;
use std::ops::Range;
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::collections::BTreeMap;

use crate::idx::Idx;
use crate::qty::MocQty;
use crate::elemset::range::MocRanges;
use crate::moc::{
  range::RangeMOC,
};
use crate::moc2d::{
  RangeMOC2Elem, RangeMOC2,
};

// While cell_2 list is the same, add T-range.
//  else create a new range list.

// Loop looking a the first end or new start...

// Sweep line putting start and end??


enum SweepLineEvent<'a, T, U> {
  Start(&'a (Range<T>, U)),
  End  (&'a (Range<T>, U)),
}
impl<'a, T: Idx, U: Idx> Eq for SweepLineEvent<'a, T, U> { }

impl<'a, T: Idx, U: Idx> PartialEq for SweepLineEvent<'a, T, U> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (SweepLineEvent::Start((l, _)), SweepLineEvent::Start((r, _))) => l.start.eq(&r.start),
      (SweepLineEvent::End((l, _)), SweepLineEvent::End((r, _))) => l.end.eq(&r.end),
      _ => false,
    }
  }
}

impl<'a, T: Idx, U: Idx> Ord for SweepLineEvent<'a, T, U>  {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (SweepLineEvent::Start((l, _)), SweepLineEvent::Start((r, _))) => l.start.cmp(&r.start),
      (SweepLineEvent::Start((l, _)), SweepLineEvent::End((r, _))) => {
        let cmp = l.start.cmp(&r.end);
        match cmp {
          Ordering::Equal => Ordering::Greater,
          _ => cmp,
        }
      },
      (SweepLineEvent::End((l, _)), SweepLineEvent::Start((r, _))) => {
        let cmp = l.end.cmp(&r.start);
        match cmp {
          Ordering::Equal => Ordering::Less,
          _ => cmp,
        }
      },
      (SweepLineEvent::End((l, _)), SweepLineEvent::End((r, _))) => l.end.cmp(&r.end),
    }
  }
}
  
impl<'a, T: Idx, U: Idx> PartialOrd for SweepLineEvent<'a, T, U>  {
  // if start == end, we consider the End to be lower than Start (because End is exclusive while
  // start is inclusive and we need to remove ended ranges before adding starting ranges)
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(
      match (self, other) {
        (SweepLineEvent::Start((l, _)), SweepLineEvent::Start((r, _))) => l.start.cmp(&r.start),
        (SweepLineEvent::Start((l, _)), SweepLineEvent::End((r, _))) => {
          let cmp = l.start.cmp(&r.end);
          match cmp {
            Ordering::Equal => Ordering::Greater,
            _ => cmp,
          }
        },
        (SweepLineEvent::End((l, _)), SweepLineEvent::Start((r, _))) => {
          let cmp = l.end.cmp(&r.start);
          match cmp {
            Ordering::Equal => Ordering::Less,
            _ => cmp,
          }
        },
        (SweepLineEvent::End((l, _)), SweepLineEvent::End((r, _))) =>  l.end.cmp(&r.end),
      }
    )
  }
}

struct SweepLineMOC2ElemBuilder<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> {
  depth_1: u8,
  start_1: Option<T>,
  ranges_1: Vec<Range<T>>,
  depth_2: u8, // Depth of U elements
  cells_2: BTreeMap<U, u32>,
  _q: PhantomData<Q>,
  _r: PhantomData<R>,
}

impl<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> SweepLineMOC2ElemBuilder<T, Q, U, R> {
  
  fn new(depth_1: u8, depth_2: u8) -> Self {
    Self {
      depth_1,
      start_1: None,
      ranges_1: Default::default(),
      depth_2,
      cells_2: Default::default(),
      _q: PhantomData,
      _r: PhantomData,
    }
  }
  
  fn is_empty(&self) -> bool {
    self.start_1.is_none() && self.ranges_1.is_empty() && self.cells_2.is_empty()
  }
  
  fn same_start(&self, start: T) -> bool {
    self.start_1.as_ref().map(|val| *val == start).unwrap_or(false)
  }
  
  fn add(&mut self, start: T, elem: U) -> Option<RangeMOC2Elem<T, Q, U, R>> {
    match self.cells_2.get_mut(&elem) {
      Some(count) =>  {
        // The cell_2 associated to the new starting range is already in the current moc_2
        // Update the moc_2 elem count only
        *count += 1;
        assert!(*self.start_1.get_or_insert(start) <= start);
        None
      },
      None => if self.cells_2.len() == 0 {
        // The current MOC2 element is void, start a new one
        debug_assert!(self.is_empty());
        self.cells_2.insert(elem, 1);
        self.start_1.get_or_insert(start);
        None
      } else if self.same_start(start) {
        // Update the current moc_2 only
        self.cells_2.insert(elem, 1);
        None
      } else {
        // Build a new MOC2Elem since both the current ranges_1 and the current moc_2 are changing
        // - build the moc_2 leaving all elements in place
        let moc_2 = self.build_moc_2();
        // - add the new moc_2 element in the current list
        self.cells_2.insert(elem, 1);
        // - push a new ranges_1 o simply init a new one 
        if let Some(prev_start) = self.start_1.replace(start) {
          self.ranges_1.push(prev_start..start);
        }
        // - build the moc_1 draining all rang_1 elements
        let moc_1 = RangeMOC::new(
          self.depth_1, 
          MocRanges::new_unchecked(mem::replace(&mut self.ranges_1, Default::default()))
        );
        Some(RangeMOC2Elem::new(moc_1, moc_2))
      },
    }
  }
  
  fn remove(&mut self, end: T, elem: U) -> Option<RangeMOC2Elem<T, Q, U, R>> {
    assert!(!self.is_empty());
    match self.cells_2.get_mut(&elem) {
      Some(count) => {
        assert!(*count > 0);
        *count -= 1;
        if *count == 0 {
          let prev_start =  self.start_1.replace(end).unwrap(); // By construction, should not be None
          let res = if prev_start != end {
            // - push a new ranges_1
            self.ranges_1.push(prev_start..end);
            // - build the moc_1 draining all rang_1 elements
            let moc_1 = RangeMOC::new(
              self.depth_1,
              MocRanges::new_unchecked(mem::replace(&mut self.ranges_1, Default::default()))
            );
            // The list of moc_2 elements and moc_1 ranges changes, so we build a new MOC2Elem
            // - build the moc_2 leaving all elements in place
            let moc_2 = self.build_moc_2();
            Some(RangeMOC2Elem::new(moc_1, moc_2))
          } else {
            // No change in moc_1 ranges, do nothing
            None
          };
          // - remove "elem"
          self.cells_2.remove(&elem);
          if self.cells_2.is_empty() {
            // No more elements, so no more range overlapping
            self.start_1 = None;
          }
          res
        } else {
          // Another range is still open with the same U value
          None
        }
      },
      None => unreachable!(),
    }
  }
  
  fn drain(mut self) -> Option<RangeMOC2Elem<T, Q, U, R>> {
    if self.is_empty() {
      None
    } else {
      debug_assert!(self.start_1.is_none());
      let moc_1 = RangeMOC::new(
        self.depth_1, 
        MocRanges::new_unchecked(mem::replace(&mut self.ranges_1, Default::default()))
      );
      let moc_2 = self.build_moc_2();
      Some(RangeMOC2Elem::new(moc_1, moc_2))
    }
  }
  
  fn build_moc_2(&self) -> RangeMOC<U, R> { // Taken from moc::builder::fixed_depth
    let shift = R::shift_from_depth_max(self.depth_2) as u32;
    let mut ranges = Vec::with_capacity(self.cells_2.len());
    let mut it = self.cells_2.keys();
    if let Some(from) = it.next() {
      let mut from = *from;
      let mut to = from + U::one();
      for curr in it {
        match to.cmp(curr) {
          Ordering::Equal => to += U::one(),
          Ordering::Less => {
            ranges.push(from.unsigned_shl(shift)..to.unsigned_shl(shift));
            from = *curr;
            to = *curr + U::one();
          },
          Ordering::Greater => unreachable!(),
        }
      }
      ranges.push(from.unsigned_shl(shift)..to.unsigned_shl(shift));
    }
    RangeMOC::new(self.depth_2, MocRanges::new_unchecked(ranges))
  }
}

pub struct RangesAndFixedDepthCellsSTMocBuilder<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> {
  depth_1: u8,
  one_at_new_depth_1: T,
  rm_bits_mask_1: T,
  bits_to_be_rm_mask_1: T,
  depth_2: u8,
  buff: Vec<(Range<T>, U)>,
  //sorted_no_overlap: bool, // Tells if it is sorted on dim 1
  moc: Option<RangeMOC2<T, Q, U, R>>,
  _r: PhantomData<R>,
}

impl<T: Idx, Q: MocQty<T>, U: Idx, R: MocQty<U>> RangesAndFixedDepthCellsSTMocBuilder<T, Q, U, R> {
  pub fn new(depth_1: u8, depth_2: u8, buf_capacity: Option<usize>) -> Self {
    let shift = Q::shift_from_depth_max(depth_1) as u32;
    let one_at_new_depth_1 = T::one().unsigned_shl(shift);
    let rm_bits_mask_1 = (!T::zero()).unsigned_shl(shift);
    let bits_to_be_rm_mask_1 = !rm_bits_mask_1;
    RangesAndFixedDepthCellsSTMocBuilder {
      depth_1,
      one_at_new_depth_1,
      rm_bits_mask_1,
      bits_to_be_rm_mask_1,
      depth_2,
      buff: Vec::with_capacity(buf_capacity.unwrap_or(10_000)),
      //sorted_no_overlap: true,
      moc: None,
      _r: PhantomData,
    }
  }

  pub fn push(&mut self, mut range_1: Range<T>, idx_2: U) {
    use crate::moc::range::op::degrade::degrade_range;
    degrade_range(&mut range_1, self.one_at_new_depth_1, self.rm_bits_mask_1, self.bits_to_be_rm_mask_1);
    if let Some((r, h)) = self.buff.last_mut() {
      // Easy merge if needed
      if *h == idx_2 {
        if !(range_1.end < r.start || r.end < range_1.start) {
          // Intersection
          if range_1.start < r.start {
            r.start = range_1.start;
          }
          if r.end < range_1.end {
            r.end = range_1.end;
          }
          return;
        } // else no intersection
      } /*else if self.sorted && range_1.start < *r.start {
        self.sorted_no_overlap = false;
      }*/
    }
    self.buff.push((range_1, idx_2));
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
    let new_moc = self.buff_to_moc();
    self.clear_buff();
    let merged_moc = if let Some(prev_moc) = &self.moc {
      prev_moc.or(&new_moc)
    } else {
      new_moc
    };
    self.moc.replace(merged_moc);
  }

  fn buff_to_moc(&mut self) -> RangeMOC2<T, Q, U, R> {
    // Create sweep line events
    let mut events: Vec<SweepLineEvent<'_, T, U>> = Vec::with_capacity(self.buff.len() << 1);
    for e in self.buff.iter() {
      // println!("PUT {} {}, {}", e.0.start, e.0.end, e.1);
      events.push(SweepLineEvent::Start(&e));
      events.push(SweepLineEvent::End(&e));
    }
    events.sort_unstable();
    // Build MOC2
    let mut moc2_elems: Vec<RangeMOC2Elem<T, Q, U, R>> = Default::default();
    let mut builder = SweepLineMOC2ElemBuilder::new(self.depth_1, self.depth_2);
    for e in events.drain(..) {
      let opt_elem = match e {
        SweepLineEvent::Start((range_1, val_2)) => {
          // println!("Start: {} ({}), {}", range_1.start, range_1.end, val_2);
          builder.add(range_1.start, *val_2)
        },
        SweepLineEvent::End((range_1, val_2)) => {
          // println!("End: ({}) {}, {}", range_1.start, range_1.end, val_2);
          builder.remove(range_1.end, *val_2)
        },
      };
      if let Some(elem) = opt_elem {
        moc2_elems.push(elem);
      }
    }
    if let Some(elem) = builder.drain() {
      moc2_elems.push(elem);
    }
    RangeMOC2::new(self.depth_1, self.depth_2, moc2_elems)
    
    // Entering here, the buffer ( buff: Vec<(Range<T>, U)> ) is sorted on Range.start.
    // We can benefit from the fact that U elements are indices at the same depth, so either
    // they are equals or they are different (they can't partially overlap).
    // Hence 
    
   /* let sl_events: Vec<SweepLineEvent<'a>> = Default::defaut();
    let range // add (range, U) / remove (range, U) 
    // => trigger an action if adding or removing a U element changes the MOC2 (map U, count)?
    
    let mut curr_sweep_line: T = self.buff.first().start;
    let mut curr_range_1: Vec<range<T> =  Default::default(); // union
    let mut curr_range_1_stack: BinaryHeap<> = Default::default();
    let mut curr_idx_2_stack: BTreeMap<U, usize> = Default::default();
    while new_start = curr_start {
      add to curr_range_1_stack
      add to curr_idx_2_stack
    }
    comp next_start with curr_idx_2_stack
    if next_start {
      if we add a new elem => make prev MOC
      else union
    } else if curr_idx_2_stack
      rm from curr_idx_2_stack
      if remove (n == 0) => make prev MOC
      else end_curr_range union
    }
  (on repere des changements dans la liste des valeurs)
    
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
    RangeMOC2::new(self.depth_1, self.depth_2, range_mocs)*/
  }

  fn clear_buff(&mut self) {
    // self.sorted = true;
    self.buff.clear();
  }

}