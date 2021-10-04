
use core::ops::Range;
use std::cmp::Ordering;
use std::marker::PhantomData;

use crate::idx::Idx;
use crate::qty::MocQty;
use crate::ranges::Ranges;
use crate::moc2d::{
  HasTwoMaxDepth, ZSorted, NonOverlapping, MOC2Properties,
  RangeMOC2Elem, RangeMOC2ElemIt,
  RangeMOCIterator, RangeMOC2Iterator,
  range::{RangeMOC, RangeMocIter}
};


// Iterator => possibility to perform operations on very large ST-MOC with a tiny 
//             memory footprint
// So far the code is much more complex that I wanted.
// One reason, besides the fact that we are working on iterator, is that I wnated to avoid:
// - having to compute several time the same moc_2 union
// - post process to merge moc_1 havong a same moc_2
// I should review this code an try to remove duplicated parts


/// Performs a logical `OR` between the two input iterators of ranges2.
/// WARNING: the purpose is not to perform the fastest possible operation on MOC2, but
///   a trade-off between complexity and code readability (a very relative notion)
///   working on iterator.
///   For best performances, one would probably re-implement 'or' directly on RangeMOC2.
pub fn or<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>(
  left_it: L1,
  right_it: L2
) -> OrRange2Iter<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>
  where
    T: Idx,       // Type of the 1st quantity (e.g. u32 or u64)
    Q: MocQty<T>, // First quantity type, e.g Time
    U: Idx,       // Type of the 2nd quantity (e.g. u32 or u64)
    R: MocQty<U>, // Second quantity type, e.g Space (we use Hpx for HEALPix)
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    L1: RangeMOC2Iterator<T, Q, I1, U, R, J1, K1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>,
    L2: RangeMOC2Iterator<T, Q, I2, U, R, J2, K2>,
{
  OrRange2Iter::new(left_it, right_it)
}

struct MOC2Elem<T, Q, U, R, I>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I: RangeMOCIterator<T, Qty=Q>,
{
  moc_1_head: Option<Range<T>>,
  moc_1_it: I,
  moc_2: RangeMOC<U, R>,
  _t: PhantomData<T>,
  _q: PhantomData<Q>,
}

impl<T, Q, U, R, I> MOC2Elem<T, Q, U, R, I>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I: RangeMOCIterator<T, Qty=Q>
{
  fn new(mut moc_1_it: I, moc_2: RangeMOC<U, R>) -> Self {
    let moc_1_head = moc_1_it.next();
    //let moc_1_depth = moc_1_it.depth_max();
    MOC2Elem {
      moc_1_head, 
      //moc_1_depth,
      moc_1_it, //: moc_1_it.peekable(), 
      moc_2,
      _t: PhantomData,
      _q: PhantomData
    }
  }
  
  fn depth_1(&self) -> u8 {
    self.moc_1_it.depth_max()
  }
  
  fn next_1(&mut self) -> Option<Range<T>> {
    std::mem::replace(&mut self.moc_1_head, self.moc_1_it.next())
  }
  
  fn moc1_it_is_depleted(&self) -> bool {
    self.moc_1_head.is_none()
  }
  
  fn to_range_moc2_elem(mut self) -> RangeMOC2Elem<T, Q, U, R> {
    let depth1 = self.moc_1_it.depth_max();
    let init_capacity = self.moc_1_it.size_hint().1.map(|up| 1 + up).unwrap_or(64);
    let mut ranges1: Vec<Range<T>> = Vec::with_capacity(init_capacity);
    if let Some(v) = self.moc_1_head {
      ranges1.push(v);
      for v in self.moc_1_it {
        ranges1.push(v);
      }
    } else {
      debug_assert!(self.moc_1_it.next().is_none());
    }
    let moc1 = RangeMOC::new(depth1, Ranges::new_unchecked(ranges1.into()).into());
    RangeMOC2Elem::new(moc1, self.moc_2)
  }

  fn push_all(&mut self, buff: &mut Vec<Range<T>>) {
    buff.push(std::mem::replace(&mut self.moc_1_head, None).unwrap());
    for range in &mut self.moc_1_it {
      buff.push(range);
    }
  }
  
  fn push_while_moc_1_range_end_le_skip_first_test(&mut self, to: T, buff: &mut Vec<Range<T>>) {
    debug_assert!(self.moc_1_head.as_ref().unwrap().end <= to);
    buff.push(self.next_1().unwrap());
    self.push_while_moc_1_range_end_le(to, buff);
  }

  fn push_while_moc_1_range_end_le(&mut self, to: T, buff: &mut Vec<Range<T>>) {
    while let Some(Range{ start: _, end}) = &self.moc_1_head {
      if *end <= to {
        buff.push(self.next_1().unwrap());
      } else {
        break;
      }
    }
  }
  
  fn consume_moc_1_till_range_end_le(&mut self, to: T) {
    debug_assert!(self.moc_1_head.as_ref().unwrap().end <= to);
    let mut curr = self.moc_1_it.next();
    while let Some(Range{ start: _, end}) = &curr {
      if *end <= to {
        curr = self.moc_1_it.next();
      } else {
        break;
      }
    }
    self.moc_1_head = curr;
  }
}

#[derive(Debug, Clone)]
enum MOCOrg {
  Left,  // From left MOC2Elem
  Right, // From right MOC2Elem 
  Both,  // From left and right (same MOC)
  Union, // Union of both MOCs
}

#[derive(Debug, Clone)]
enum MOCUnionType {
  LeftContainsRight,
  RightContainsLeft,
  NoOneContainsTheOther,
}

#[derive(Debug, Clone)]
struct MOCUnion<U: Idx, R: MocQty<U>> {
  utype: MOCUnionType,
  moc_2: RangeMOC<U, R>,
}
impl<U: Idx, R: MocQty<U>> MOCUnion<U, R> {
  fn new(utype: MOCUnionType, moc_2: RangeMOC<U, R>) -> Self {
    MOCUnion { utype, moc_2  }
  }
  fn from(left: &RangeMOC<U, R>, right: &RangeMOC<U, R>) -> Self {
    let union = left.or(&right);
    MOCUnion::new(
      if union.eq_without_depth(left) {
        MOCUnionType::LeftContainsRight
      } else if union.eq_without_depth(right) {
        MOCUnionType::RightContainsLeft
      } else { // no moc included in the other one
        MOCUnionType::NoOneContainsTheOther
      },
      union
    )
  }
}

#[derive(Debug, Clone)]
struct MOC2ElemBuilder<T, Q, U, R> 
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
{
  moc_1_depth: u8,
  moc_1_elems: Vec<Range<T>>,
  moc_2: RangeMOC<U, R>,
  moc_2_org: MOCOrg,
  _q: PhantomData<Q>,
}

impl<T, Q, U, R> MOC2ElemBuilder<T, Q, U, R>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
{
  
  fn new(
    moc_1_depth: u8,
    moc_1_elems: Vec<Range<T>>,
    moc_2: RangeMOC<U, R>,
    moc_2_org: MOCOrg
  ) -> Self {
    MOC2ElemBuilder {
      moc_1_depth,
      moc_1_elems,
      moc_2,
      moc_2_org,
      _q: PhantomData
    }
  }
  
  fn push(&mut self, elem: Range<T>) {
    debug_assert!(elem.start < elem.end);
    self.moc_1_elems.push(elem)
  }
  
  fn to_moc2_elem(self) -> RangeMOC2Elem<T, Q, U, R> {
    debug_assert!(self.moc_1_elems.len() > 0);
    let moc_1 = RangeMOC::new(self.moc_1_depth, Ranges::new_unchecked(self.moc_1_elems).into());
    RangeMOC2Elem::new(moc_1, self.moc_2)
  }
}


/// Performs an `OR` operation between two iterators of ranges on-the-fly, while iterating.
pub struct OrRange2Iter<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    L1: RangeMOC2Iterator<T, Q, I1, U, R, J1, K1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>,
    L2: RangeMOC2Iterator<T, Q, I2, U, R, J2, K2>,
{
  depth_max_l: u8,
  depth_max_r: u8,
  // last: Option<Range<T>>,
  curr_moc2_left: Option<MOC2Elem<T, Q, U, R, I1>>,
  curr_moc2_right: Option<MOC2Elem<T, Q, U, R, I2>>,
  /* curr_moc_1_left: I1,
  curr_moc_1_right: I2,
  curr_moc_2_left: RangeMOC<U, R>,  // <=> Left  quantity
  curr_moc_2_right: RangeMOC<U, R>, // <=> Right quantity */
  
  moc2_builder: Option<MOC2ElemBuilder<T, Q, U, R>>, // Current MOC_1 builder
  // Union of curr_moc_2_left.moc_2 and curr_moc_2_right.moc_2 to be used as a cache
  curr_moc_2_union: Option<MOCUnion<U, R>>,
  
  left_it: L1,
  right_it: L2,
  // left: Option<Range<T>>,
  // right: Option<Range<T>>,
  _k1: PhantomData<K1>,
  _k2: PhantomData<K2>,
}

impl<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2> HasTwoMaxDepth for OrRange2Iter<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    L1: RangeMOC2Iterator<T, Q, I1, U, R, J1, K1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>,
    L2: RangeMOC2Iterator<T, Q, I2, U, R, J2, K2>,
{
  fn depth_max_1(&self) -> u8 {
    self.depth_max_l
  }
  fn depth_max_2(&self) -> u8 {
    self.depth_max_r
  }
}
impl<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2> ZSorted for OrRange2Iter<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    L1: RangeMOC2Iterator<T, Q, I1, U, R, J1, K1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>,
    L2: RangeMOC2Iterator<T, Q, I2, U, R, J2, K2> { }
impl<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2> NonOverlapping for OrRange2Iter<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    L1: RangeMOC2Iterator<T, Q, I1, U, R, J1, K1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>,
    L2: RangeMOC2Iterator<T, Q, I2, U, R, J2, K2> { }
impl<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2> MOC2Properties for OrRange2Iter<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    L1: RangeMOC2Iterator<T, Q, I1, U, R, J1, K1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>,
    L2: RangeMOC2Iterator<T, Q, I2, U, R, J2, K2> { }



// same algo when merging quantities is needed!!
// L--L L--L L--L | L----L
//   R----R |  R---R

impl<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2> OrRange2Iter<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    L1: RangeMOC2Iterator<T, Q, I1, U, R, J1, K1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>,
    L2: RangeMOC2Iterator<T, Q, I2, U, R, J2, K2>,
{
  fn new(mut left_it: L1, mut right_it: L2) -> Self {
    let depth_max_l = u8::max(left_it.depth_max_1(), right_it.depth_max_1());
    let depth_max_r = u8::max(left_it.depth_max_2(), right_it.depth_max_2());
    let curr_moc2_left = left_it.next().map(|e| {
      let (moc_1, moc_2) = e.range_mocs_it();
      MOC2Elem::new(moc_1, moc_2.into_range_moc())
    });
    let curr_moc2_right = right_it.next().map(|e| {
      let (moc_1, moc_2) = e.range_mocs_it();
      MOC2Elem::new(moc_1, moc_2.into_range_moc())
    });
    OrRange2Iter {
      depth_max_l,
      depth_max_r,
      curr_moc2_left,
      curr_moc2_right,
      moc2_builder: None,
      curr_moc_2_union: None,
      left_it,
      right_it,
      //left,
      //right,
      _k1: PhantomData,
      _k2: PhantomData,
    }
  }
  
  fn next_left(&mut self) -> Option<MOC2Elem<T, Q, U, R, I1>> {
    self.curr_moc_2_union = None;
    std::mem::replace(
      &mut self.curr_moc2_left,
      self.left_it.next().map(|e| {
        let (moc_1, moc_2) = e.range_mocs_it();
        MOC2Elem::new(moc_1, moc_2.into_range_moc())
      })
    )
  }

  fn next_right(&mut self) -> Option<MOC2Elem<T, Q, U, R, I2>> {
    self.curr_moc_2_union = None;
    std::mem::replace(
      &mut self.curr_moc2_right,
      self.right_it.next().map(|e| {
        let (moc_1, moc_2) = e.range_mocs_it();
        MOC2Elem::new(moc_1, moc_2.into_range_moc())
      })
    )
  }
}


impl<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2> Iterator for OrRange2Iter<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    L1: RangeMOC2Iterator<T, Q, I1, U, R, J1, K1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>,
    L2: RangeMOC2Iterator<T, Q, I2, U, R, J2, K2>,
{
  type Item = RangeMOC2Elem<T, Q, U, R>;
  
  fn next(&mut self) -> Option<Self::Item> {
    #[derive(Debug)]
    enum WasDepleted {
      Left,
      Right,
      None,
    }
    let mut was_depleted = WasDepleted::None;
    loop {
      match (self.curr_moc2_left.as_mut(), self.curr_moc2_right.as_mut()) {
        (None, None) => {
          return self.moc2_builder.take().map(|moc2_builder| moc2_builder.to_moc2_elem())
        },
        (Some(curr_moc2_left), None) => {
          match self.moc2_builder.take() {
            None => {
              // No more element in the right MOC2, simply iterate over the left MOC2
              let prev_moc2_left = self.next_left().unwrap();
              return Some(prev_moc2_left.to_range_moc2_elem());
            },
            Some(mut moc2_builder) => {
              // use moc2_builder_ref.moc_2_org to possibly avoid the MOC inclusion test?
              if curr_moc2_left.moc_2 == moc2_builder.moc_2 {
                curr_moc2_left.push_all(&mut moc2_builder.moc_1_elems);
                self.next_left().unwrap();
                // The next left_moc_2 will be different, so return instead of break.
              } 
              return Some(moc2_builder.to_moc2_elem());
            }
          }
        },
        (None, Some(curr_moc2_right)) => {
          match self.moc2_builder.take() {
            None => {
              // No more element in the left MOC2, simply iterate over the right MOC2
              let prev_moc2_right = self.next_right().unwrap();
              return Some(prev_moc2_right.to_range_moc2_elem());
            },
            Some(mut moc2_builder) => {
              // use moc2_builder_ref.moc_2_org to possibly avoid the MOC inclusion test?
              if curr_moc2_right.moc_2 == moc2_builder.moc_2 {
                curr_moc2_right.push_all(&mut moc2_builder.moc_1_elems);
                self.next_right().unwrap();
                // The next right_moc_2 will be different, so return instead of break.
              }
              return Some(moc2_builder.to_moc2_elem());
            }
          }
        },
        (Some(curr_moc2_left), Some(curr_moc2_right)) => {
          loop {
            if curr_moc2_left.moc1_it_is_depleted() {
              self.next_left().unwrap();
              was_depleted = WasDepleted::Left;
              break;
            } else if curr_moc2_right.moc1_it_is_depleted() {
              self.next_right().unwrap();
              was_depleted = WasDepleted::Right;
              break;
            } else if let Some(moc2_builder) = self.moc2_builder.as_mut() { //  MOC2Elem started in a previous loop iteration
              match moc2_builder.moc_2_org {
                MOCOrg::Both => {
                  // We stopped iterating because one of the MOC2Elem was empty.
                  // We known that the new moc_2 is different from
                  // (but may be contained in) the current moc2_builder.moc_2
                  let l: &mut Range<T> = curr_moc2_left.moc_1_head.as_mut().unwrap();
                  let r: &mut Range<T> = curr_moc2_right.moc_1_head.as_mut().unwrap();
                  match was_depleted {
                    WasDepleted::Left => {
                      // Check if right moc_1 head overlap left moc_1 head 
                      // AND left moc_2 is included in 
                      // If not, return prev MOC2Elem
                      // else...
                      if r.end <= l.start { // R--RL--L (no overlap)
                        curr_moc2_right.push_while_moc_1_range_end_le_skip_first_test(l.start, &mut moc2_builder.moc_1_elems);
                        if curr_moc2_right.moc1_it_is_depleted() {
                          return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                        } // else continue the loop
                      } else if l.start <= r.start { // L--xx  (L--LR--R or L--R--xx )
                        return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                      } else { // R--L--xx
                        debug_assert!(r.start < l.start);
                        // Get the cached moc_2 union or compute it
                        let moc_2_union = match self.curr_moc_2_union.as_ref() {
                          Some(moc_2_union) => moc_2_union.clone(),
                          None => {
                            let moc_2_union = MOCUnion::from(&curr_moc2_left.moc_2, &curr_moc2_right.moc_2);
                            self.curr_moc_2_union = Some(moc_2_union.clone());
                            moc_2_union
                          },
                        };
                        if matches!(moc_2_union.utype, MOCUnionType::RightContainsLeft) { // left moc_2 included in right_moc_2
                          perform_union_while_left_included_in_right(curr_moc2_left, curr_moc2_right, &mut moc2_builder.moc_1_elems);
                          if !curr_moc2_left.moc1_it_is_depleted() {
                            return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                          } else {
                            moc2_builder.moc_2_org = MOCOrg::Right;
                            // check if nex_left_moc_1_head intersects  current right_moc_1_head 
                            // AND next_left_moc_2 also included in current right_moc_2
                          }
                        } else {
                          let range = r.start..l.start;
                          r.start = l.start;
                          moc2_builder.push(range);
                          return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                        }
                      }
                    },
                    WasDepleted::Right => {
                      if l.end <= r.start { // L--LR--R (no overlap)
                        curr_moc2_left.push_while_moc_1_range_end_le_skip_first_test(r.start, &mut moc2_builder.moc_1_elems);
                        if curr_moc2_left.moc1_it_is_depleted() {
                          return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                        }  // else continue the loop 
                      } else if r.start <= l.start { // R--xx  (R--RL--L or R--L--xx )
                        return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                      } else { // L--R--xx
                        debug_assert!(l.start < r.start);
                        // Get the cached moc_2 union or compute it
                        let moc_2_union = match self.curr_moc_2_union.as_ref() {
                          Some(moc_2_union) => moc_2_union.clone(),
                          None => {
                            let moc_2_union = MOCUnion::from(&curr_moc2_left.moc_2, &curr_moc2_right.moc_2);
                            self.curr_moc_2_union = Some(moc_2_union.clone());
                            moc_2_union
                          },
                        };
                        if matches!(moc_2_union.utype, MOCUnionType::LeftContainsRight) { // right moc_2 included in left_moc_2
                          perform_union_while_right_included_in_left(curr_moc2_left, curr_moc2_right, &mut moc2_builder.moc_1_elems);
                          if !curr_moc2_right.moc1_it_is_depleted() {
                            return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                          } else {
                            moc2_builder.moc_2_org = MOCOrg::Left;
                            // check if nex_right_moc_1_head intersects  current left_moc_1_head 
                            // AND next_right_moc_2 also included in current left_moc_2
                          }
                        } else {
                          let range = l.start..r.start;
                          l.start = r.start;
                          moc2_builder.push(range);
                          return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                        }
                      }
                    },
                    WasDepleted::None => unreachable!(),
                  }
                },
                MOCOrg::Left => {
                  match was_depleted {
                    WasDepleted::Left => unreachable!(),
                    WasDepleted::Right => {
                      // MOC left wasn't depleted, but new MOC right to be evaluated
                      // => see if new right_moc_2 equals left_moc_2 
                      //    or if range1 overlaps and union if in left

                      // Check whether or not it equals the current moc_2
                      debug_assert!(moc2_builder.moc_2.eq_without_depth(&curr_moc2_left.moc_2));
                      if curr_moc2_left.moc_2 == curr_moc2_right.moc_2 {
                        // Rare case, but to be checked
                        perform_union_till_first_end(curr_moc2_left, curr_moc2_right, &mut moc2_builder.moc_1_elems);
                        moc2_builder.moc_2_org = MOCOrg::Both;
                      } else {
                        // Check for overlap
                        let l: &mut Range<T> = curr_moc2_left.moc_1_head.as_mut().unwrap();
                        let r: &mut Range<T> = curr_moc2_right.moc_1_head.as_mut().unwrap();
                        if l.end <= r.start || r.end <= l.start { // L--L R--R or R--R L--L => no overlap
                          // No overlap and we already checked that both moc_2 are different
                          return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                        } else {
                          // Ranges overlap, need to compute the union and check whether right_moc_2
                          //   included or not in left_moc_2
                          // The union is supposed to be empty (new right element)
                          assert!(self.curr_moc_2_union.is_none());
                          // Compute the union
                          let moc_2_union = MOCUnion::from(&curr_moc2_left.moc_2, &curr_moc2_right.moc_2);
                          if r.start < l.start { // R--L--xx
                            return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                          } else { // L--R--xx
                            match moc_2_union.utype {
                              MOCUnionType::LeftContainsRight => {
                                perform_union_while_right_included_in_left(curr_moc2_left, curr_moc2_right, &mut moc2_builder.moc_1_elems);
                                if curr_moc2_right.moc1_it_is_depleted() {
                                  // need to check if nex_right_moc_1_head intersects  current left_moc_1_head 
                                  // AND next_right_moc_2 also included in current left_moc_2
                                  self.next_right().unwrap();
                                  debug_assert!(matches!(was_depleted, WasDepleted::Right));
                                  break;
                                } else {
                                  return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                                } // else
                              },
                              _ => {
                                let range = l.start..r.start;
                                l.start = r.start;
                                moc2_builder.moc_1_elems.push(range);
                                return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                              },
                            }
                          }
                        }
                      }
                    },
                    WasDepleted::None => {
                      // We know that both moc_2 are different (else branch MOCOrg::Both)
                      debug_assert!(curr_moc2_left.moc_2 != curr_moc2_right.moc_2);
                      let l: &mut Range<T> = curr_moc2_left.moc_1_head.as_mut().unwrap();
                      let r: &mut Range<T> = curr_moc2_right.moc_1_head.as_mut().unwrap();
                      debug_assert!(l.end > r.start); // No case L--LR--R
                      if r.start <= l.start { // R--x
                        return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                      } else { // L--R--xx
                        // Check whether Left included in Right or Right included in left
                        // Get the cached moc_2 union or compute it
                        let moc_2_union = match self.curr_moc_2_union.as_ref() {
                          Some(moc_2_union) => moc_2_union.clone(),
                          None => {
                            let moc_2_union = MOCUnion::from(&curr_moc2_left.moc_2, &curr_moc2_right.moc_2);
                            self.curr_moc_2_union = Some(moc_2_union.clone());
                            moc_2_union
                          },
                        };
                        match moc_2_union.utype {
                          MOCUnionType::LeftContainsRight => {
                            perform_union_while_right_included_in_left(curr_moc2_left, curr_moc2_right, &mut moc2_builder.moc_1_elems);
                            if !curr_moc2_right.moc1_it_is_depleted() {
                              return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                            } // else 
                            // check if nex_right_moc_1_head intersects  current left_moc_1_head 
                            // AND next_right_moc_2 also included in current left_moc_2
                          },
                          _ => {
                            let range = l.start..r.start;
                            l.start = r.start;
                            moc2_builder.moc_1_elems.push(range);
                            return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                          },
                        }
                      }
                    },
                  }
                },
                MOCOrg::Right => {
                  match was_depleted {
                    WasDepleted::Left => {
                      // MOC right wasn't depleted, but new MOC left to be evaluated
                      // => see if new left_moc_2 equals right_moc_2 
                      //    or if range1 overlaps and union if in right

                      // Check whether or not it equals the current moc_2
                      debug_assert!(moc2_builder.moc_2 == curr_moc2_right.moc_2);
                      if curr_moc2_left.moc_2 == curr_moc2_right.moc_2 {
                        // Rare case, but to be checked
                        perform_union_till_first_end(curr_moc2_left, curr_moc2_right, &mut moc2_builder.moc_1_elems);
                        moc2_builder.moc_2_org = MOCOrg::Both;
                      } else {
                        // Check for overlap
                        let l: &mut Range<T> = curr_moc2_left.moc_1_head.as_mut().unwrap();
                        let r: &mut Range<T> = curr_moc2_right.moc_1_head.as_mut().unwrap();
                        if l.end <= r.start || r.end <= l.start { // L--L R--R or R--R L--L => no overlap
                          // No overlap and we already checked that both moc_2 are different
                          return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                        } else {
                          // Ranges overlap, need to compute the union and check whether right_moc_2
                          //   included or not in left_moc_2
                          // The union is supposed to be empty (new right element)
                          assert!(self.curr_moc_2_union.is_none());
                          // Compute the union
                          let moc_2_union = MOCUnion::from(&curr_moc2_left.moc_2, &curr_moc2_right.moc_2);
                          if l.start < r.start { // L--R--xx
                            return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                          } else { // R--L--xx
                            /*let range = r.start..l.start;
                            r.start = l.start;
                            moc2_builder.moc_1_elems.push(range);
                            return self.moc2_builder.take().map(|b| b.to_moc2_elem());*/
                            match moc_2_union.utype {
                              MOCUnionType::RightContainsLeft => {
                                perform_union_while_left_included_in_right(curr_moc2_left, curr_moc2_right, &mut moc2_builder.moc_1_elems);
                                if !curr_moc2_left.moc1_it_is_depleted() {
                                  return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                                } // else
                                // check if nex_left_moc_1_head intersects  current right_moc_1_head 
                                // AND next_left_moc_2 also included in current right_moc_2
                              },
                              _ => {
                                let range = r.start..l.start;
                                r.start = l.start;
                                moc2_builder.moc_1_elems.push(range);
                                return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                              },
                            }
                          }
                        }
                      }
                    },
                    WasDepleted::Right =>  unreachable!(),
                    WasDepleted::None => {
                      // We know that both moc_2 are different (else branch MOCOrg::Both)
                      debug_assert!(curr_moc2_left.moc_2 != curr_moc2_right.moc_2);
                      let l: &mut Range<T> = curr_moc2_left.moc_1_head.as_mut().unwrap();
                      let r: &mut Range<T> = curr_moc2_right.moc_1_head.as_mut().unwrap();
                      debug_assert!(r.end > l.start); // No case R--RL--L
                      if l.start <= r.start { // L--x
                        return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                      } else { // R--L--xx
                        // Check whether Left included in Right or Right included in left
                        // Get the cached moc_2 union or compute it
                        let moc_2_union = match self.curr_moc_2_union.as_ref() {
                          Some(moc_2_union) => moc_2_union.clone(),
                          None => {
                            let moc_2_union = MOCUnion::from(&curr_moc2_left.moc_2, &curr_moc2_right.moc_2);
                            self.curr_moc_2_union = Some(moc_2_union.clone());
                            moc_2_union
                          },
                        };
                        match moc_2_union.utype {
                          MOCUnionType::RightContainsLeft => {
                            perform_union_while_left_included_in_right(curr_moc2_left, curr_moc2_right, &mut moc2_builder.moc_1_elems);
                            if !curr_moc2_left.moc1_it_is_depleted() {
                              return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                            } // else 
                            // check if nex_left_moc_1_head intersects  current right_moc_1_head 
                            // AND next_left_moc_2 also included in current right_moc_2
                          },
                          _ => {
                            let range = r.start..l.start;
                            r.start = l.start;
                            moc2_builder.moc_1_elems.push(range);
                            return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                          },
                        }
                      }
                    },
                  }
                },
                MOCOrg::Union => {
                  // Both were depleted, we need to check whether or not the next elem to be added
                  // has the same moc_2
                  let l: &mut Range<T> = curr_moc2_left.moc_1_head.as_mut().unwrap();
                  let r: &mut Range<T> = curr_moc2_right.moc_1_head.as_mut().unwrap();
                  match l.start.cmp(&r.start) {
                    Ordering::Less => { // L--xx
                      if moc2_builder.moc_2 == curr_moc2_left.moc_2 {
                        curr_moc2_left.push_while_moc_1_range_end_le(r.start, &mut moc2_builder.moc_1_elems);
                        moc2_builder.moc_2_org = MOCOrg::Left;
                        was_depleted = WasDepleted::Right;
                      } else {
                        return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                      }
                    },
                    Ordering::Greater => { // R--
                      //curr_moc2_left.moc_2 == curr_moc2_right.moc_2
                      if moc2_builder.moc_2 == curr_moc2_right.moc_2 {
                        curr_moc2_right.push_while_moc_1_range_end_le(l.start, &mut moc2_builder.moc_1_elems);
                        moc2_builder.moc_2_org = MOCOrg::Right;
                        was_depleted = WasDepleted::Left;
                      } else {
                        return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                      }
                    },
                    Ordering::Equal => {
                      assert!(self.curr_moc_2_union.is_none());
                      let moc_2_union = MOCUnion::from(&curr_moc2_left.moc_2, &curr_moc2_right.moc_2);
                      // Check whether or not it equals the current moc_2
                      if moc2_builder.moc_2.eq_without_depth(&moc_2_union.moc_2)  {
                        self.curr_moc_2_union = Some(moc_2_union);
                        perform_union_while_equal_ranges(curr_moc2_left, curr_moc2_right, &mut moc2_builder.moc_1_elems);
                        if !curr_moc2_left.moc1_it_is_depleted() || !curr_moc2_right.moc1_it_is_depleted() {
                          return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                        } // else iterate
                      } else {
                        self.curr_moc_2_union = Some(moc_2_union);
                        return self.moc2_builder.take().map(|b| b.to_moc2_elem());
                      }
                    },
                  }
                },
              }
            } else { // No MOC2Elem started yet
              was_depleted = WasDepleted::None;
              debug_assert!(self.moc2_builder.is_none());
              let depth_1 = u8::max(curr_moc2_left.moc_1_it.depth_max(), curr_moc2_right.moc_1_it.depth_max());
              if curr_moc2_left.moc_2 == curr_moc2_right.moc_2 {
                let mut moc_1_buff: Vec<Range<T>> = Default::default();
                perform_union_till_first_end(curr_moc2_left, curr_moc2_right, &mut moc_1_buff);
                // We could have avoided the moc_2 copy by retrieving the one of the
                // depleted MOC2Elem.moc_1_it. But the algo become more complex 
                // (due to Rust ownership/borrowing rules).
                let moc_2 = curr_moc2_left.moc_2.clone(); // ESSAYER QUAND MEME DE METRE ICI LE (Some, None), (None, Some)
                // We loop to check whether the MOC with remaining moc_1 elements:
                // - overlaps the new MOC2Elem moc_1
                // - AND have a moc_2 containing the new MOC2Elem moc_2
                // At this point, moc_1_buff may be empty but will be filled in the loop
                self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2, MOCOrg::Both));
              } else {
                let l: &mut Range<T> = curr_moc2_left.moc_1_head.as_mut().unwrap();
                let r: &mut Range<T> = curr_moc2_right.moc_1_head.as_mut().unwrap();
                if l.end <= r.start {        // L--L R--R => Non overlapping (easy)
                  let mut moc_1_buff: Vec<Range<T>> = Default::default();
                  curr_moc2_left.push_while_moc_1_range_end_le_skip_first_test(r.start, &mut moc_1_buff);
                  if curr_moc2_left.moc1_it_is_depleted() {
                    // No overlapping with right moc_1, next left moc_2 will be different so
                    // we can return a new MOC2Elem
                    let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(moc_1_buff).into());
                    let prev_left = self.next_left().unwrap();
                    return Some(RangeMOC2Elem::new(moc_1, prev_left.moc_2));
                  } else {
                    debug_assert!(curr_moc2_left.moc_1_head.as_ref().unwrap().end > r.start);
                    // We loop to check whether:
                    // - R-xx => return the current MOC
                    // - L--R--xx with R MOC2 included in L MOC2 (=> merge)
                    // - L--R--xx with R MOC2 NOT included in L MOC2 (=> add L--R to current MOC and return it)
                    debug_assert!(self.moc2_builder.is_none() && moc_1_buff.len() > 0);
                    let moc_2 = curr_moc2_left.moc_2.clone();
                    //  WasDepleted can be Right or None;
                    self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2, MOCOrg::Left));
                  }
                } else if r.end <= l.start { // R--R L--L => Non overlapping (easy)
                  let mut moc_1_buff: Vec<Range<T>> = Default::default();
                  curr_moc2_right.push_while_moc_1_range_end_le(l.start, &mut moc_1_buff);
                  if curr_moc2_right.moc1_it_is_depleted() {
                    // No overlapping with right moc_1, next left moc_2 will be different so
                    // we can return a new MOC2Elem
                    let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(moc_1_buff).into());
                    let prev_right = self.next_right().unwrap();
                    return Some(RangeMOC2Elem::new(moc_1, prev_right.moc_2));
                  } else {
                    // We loop to check whether:
                    // - L-xx => return the current MOC
                    // - R--L--xx with L MOC2 included in R MOC2 (=> merge)
                    // - R--L--xx with L MOC2 NOT included in R MOC2 (=> add R--L to current MOC and return it)
                    debug_assert!(self.moc2_builder.is_none() && moc_1_buff.len() > 0);
                    let moc_2 = curr_moc2_right.moc_2.clone();
                    //  WasDepleted can be Left or None;
                    self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2, MOCOrg::Right));
                  }
                } else {
                  // Get the cached moc_2 union or compute it
                  let moc_2_union = match self.curr_moc_2_union.as_ref() {
                    Some(moc_2_union) => moc_2_union.clone(),
                    None => {
                      let moc_2_union = MOCUnion::from(&curr_moc2_left.moc_2, &curr_moc2_right.moc_2);
                      self.curr_moc_2_union = Some(moc_2_union.clone());
                      moc_2_union
                    },
                  };
                  // Perform the union
                  if l.start == r.start { // LR--xx Perform the union !!!
                    match l.end.cmp(&r.end) {
                      Ordering::Less => { // LR--L--R
                        match moc_2_union.utype {
                          MOCUnionType::RightContainsLeft => {
                            let mut moc_1_buff: Vec<Range<T>> = Default::default();
                            perform_union_while_left_included_in_right(curr_moc2_left, curr_moc2_right, &mut moc_1_buff);
                            debug_assert!(self.moc2_builder.is_none());
                            if !curr_moc2_left.moc1_it_is_depleted() {
                              let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(moc_1_buff).into());
                              return Some(RangeMOC2Elem::new(moc_1, moc_2_union.moc_2));
                            } else if moc_1_buff.len() > 0 {
                              // check if nex_left_moc_1_head intersects  current right_moc_1_head 
                              // AND next_left_moc_2 also included in current right_moc_2
                              self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2_union.moc_2, MOCOrg::Right));
                            }
                          },
                          _ => {
                            r.start = l.end;
                            let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(vec![curr_moc2_left.next_1().unwrap()]).into());
                            return Some(RangeMOC2Elem::new(moc_1, moc_2_union.moc_2));
                          },
                        }
                      },
                      Ordering::Equal => {  // LR--RL
                        match moc_2_union.utype {
                          MOCUnionType::RightContainsLeft => {
                            let mut moc_1_buff: Vec<Range<T>> = Default::default();
                            perform_union_while_left_included_in_right(curr_moc2_left, curr_moc2_right, &mut moc_1_buff);
                            debug_assert!(self.moc2_builder.is_none());
                            if !curr_moc2_left.moc1_it_is_depleted() {
                              let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(moc_1_buff).into());
                              return Some(RangeMOC2Elem::new(moc_1, moc_2_union.moc_2));
                            } else if moc_1_buff.len() > 0 {
                              // check if nex_left_moc_1_head intersects  current right_moc_1_head 
                              // AND next_left_moc_2 also included in current right_moc_2
                              self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2_union.moc_2, MOCOrg::Right));
                            }
                          },
                          MOCUnionType::LeftContainsRight => {
                            let mut moc_1_buff: Vec<Range<T>> = Default::default();
                            perform_union_while_right_included_in_left(curr_moc2_left, curr_moc2_right, &mut moc_1_buff);
                            debug_assert!(self.moc2_builder.is_none());
                            if !curr_moc2_right.moc1_it_is_depleted() {
                              let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(moc_1_buff).into());
                              return Some(RangeMOC2Elem::new(moc_1, moc_2_union.moc_2));
                            } else if moc_1_buff.len() > 0 {
                              // check if nex_right_moc_1_head intersects  current left_moc_1_head 
                              // AND next_right_moc_2 also included in current left_moc_2
                              self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2_union.moc_2, MOCOrg::Left));
                            }
                          },
                          MOCUnionType::NoOneContainsTheOther => {
                            let mut moc_1_buff: Vec<Range<T>> = Default::default();
                            perform_union_while_equal_ranges_skip_first_check(curr_moc2_left, curr_moc2_right, &mut moc_1_buff);
                            debug_assert!(self.moc2_builder.is_none());
                            if curr_moc2_left.moc1_it_is_depleted() && curr_moc2_right.moc1_it_is_depleted() {
                              // Check whether the next MOC equals the union or not
                              debug_assert!(moc_1_buff.len() > 0);
                              self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2_union.moc_2, MOCOrg::Union));
                              self.next_left();
                              self.next_right();
                              break;
                            } else if moc_1_buff.len() > 0 {
                              let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(moc_1_buff).into());
                              return Some(RangeMOC2Elem::new(moc_1, moc_2_union.moc_2));
                            }
                          }
                        }
                      },
                      Ordering::Greater => { // LR--R--L
                        match moc_2_union.utype {
                          MOCUnionType::LeftContainsRight => {
                            let mut moc_1_buff: Vec<Range<T>> = Default::default();
                            perform_union_while_right_included_in_left(curr_moc2_left, curr_moc2_right, &mut moc_1_buff);
                            debug_assert!(self.moc2_builder.is_none());
                            if !curr_moc2_right.moc1_it_is_depleted() {
                              let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(moc_1_buff).into());
                              return Some(RangeMOC2Elem::new(moc_1, moc_2_union.moc_2));
                            } else if moc_1_buff.len() > 0 {
                              // check if nex_right_moc_1_head intersects  current left_moc_1_head 
                              // AND next_right_moc_2 also included in current left_moc_2
                              self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2_union.moc_2, MOCOrg::Left));
                            }
                          },
                          _ => {
                            l.start = r.end;
                            let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(vec![curr_moc2_right.next_1().unwrap()]).into());
                            return Some(RangeMOC2Elem::new(moc_1, moc_2_union.moc_2));
                          },
                        }
                      },
                    }
                  } else if l.start < r.start { // L--Rxx
                    match moc_2_union.utype {
                      MOCUnionType::LeftContainsRight => {
                        let mut moc_1_buff: Vec<Range<T>> = Default::default();
                        perform_union_while_right_included_in_left(curr_moc2_left, curr_moc2_right, &mut moc_1_buff);
                        debug_assert!(self.moc2_builder.is_none());
                        if !curr_moc2_right.moc1_it_is_depleted() {
                          let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(moc_1_buff).into());
                          return Some(RangeMOC2Elem::new(moc_1, moc_2_union.moc_2));
                        } else if moc_1_buff.len() > 0 {
                          // check if nex_right_moc_1_head intersects  current left_moc_1_head 
                          // AND next_right_moc_2 also included in current left_moc_2
                          self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2_union.moc_2, MOCOrg::Left));
                        }
                      },
                      _ => {
                        if l.end == r.start {  //    L--RL--xx
                          let moc_1 = RangeMOC::new(curr_moc2_left.depth_1(), Ranges::new_unchecked(vec![curr_moc2_left.next_1().unwrap()]).into());
                          return Some(RangeMOC2Elem::new(moc_1, curr_moc2_left.moc_2.clone()));
                        } else {               // or  L--R--xx
                          let range = l.start..r.start;
                          l.start = r.start;
                          let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(vec![range]).into());
                          return Some(RangeMOC2Elem::new(moc_1, curr_moc2_left.moc_2.clone()));
                        }
                      },
                    }
                  } else {
                    debug_assert!(l.start > r.start);
                    match moc_2_union.utype {
                      MOCUnionType::RightContainsLeft => {
                        let mut moc_1_buff: Vec<Range<T>> = Default::default();
                        perform_union_while_left_included_in_right(curr_moc2_left, curr_moc2_right, &mut moc_1_buff);
                        debug_assert!(self.moc2_builder.is_none());
                        if !curr_moc2_left.moc1_it_is_depleted() {
                          let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(moc_1_buff).into());
                          return Some(RangeMOC2Elem::new(moc_1, moc_2_union.moc_2));
                        } else if moc_1_buff.len() > 0 {
                          // check if nex_left_moc_1_head intersects  current right_moc_1_head 
                          // AND next_left_moc_2 also included in current right_moc_2
                          self.moc2_builder = Some(MOC2ElemBuilder::new(depth_1, moc_1_buff, moc_2_union.moc_2, MOCOrg::Right));
                        }
                      },
                      _ => {
                        if l.end == r.end {   //  R--LR--xx 
                          let moc_1 = RangeMOC::new(curr_moc2_right.depth_1(), Ranges::new_unchecked(vec![curr_moc2_right.next_1().unwrap()]).into());
                          return Some(RangeMOC2Elem::new(moc_1, curr_moc2_right.moc_2.clone()));
                        } else {             //    R--L--xx        (None, None) => return None,
                          let range = r.start..l.start;
                          r.start = l.start;
                          let moc_1 = RangeMOC::new(depth_1, Ranges::new_unchecked(vec![range]).into());
                          return Some(RangeMOC2Elem::new(moc_1, curr_moc2_right.moc_2.clone()));
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}

fn perform_union_till_first_end<T, Q, U, R, I1, I2>(
  moc2_elem_l: &mut MOC2Elem<T, Q, U, R, I1>, 
  moc2_elem_r:  &mut MOC2Elem<T, Q, U, R, I2>,
  moc_1_ranges_collector: &mut Vec<Range<T>>
) 
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: RangeMOCIterator<T, Qty=Q>,
{
  while let (Some(ref mut l), Some(ref mut r)) = (&mut moc2_elem_l.moc_1_head, &mut moc2_elem_r.moc_1_head) {
    if l.end < r.start {        // L--L R--R
      moc_1_ranges_collector.push(moc2_elem_l.next_1().unwrap());
    } else if r.end < l.start { // R--R L--L
      moc_1_ranges_collector.push(moc2_elem_r.next_1().unwrap());
    } else if l.end <= r.end {  //    R--L--L--R
      if l.start < r.start {    // or L--R--L--R
        r.start = l.start;
      }
      moc2_elem_l.consume_moc_1_till_range_end_le(r.end);
    } else {                    //    L--R--R--L
      if r.start < l.start {    // or R--L--R--L
        l.start = r.start;
      }
      moc2_elem_r.consume_moc_1_till_range_end_le(l.end);
    }
  }
}


fn perform_union_while_left_included_in_right<T, Q, U, R, I1, I2>(
  moc2_elem_l: &mut MOC2Elem<T, Q, U, R, I1>,
  moc2_elem_r:  &mut MOC2Elem<T, Q, U, R, I2>,
  moc_1_ranges_collector: &mut Vec<Range<T>>
)
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: RangeMOCIterator<T, Qty=Q>,
{
  while let (Some(ref mut l), Some(ref mut r)) = (&mut moc2_elem_l.moc_1_head, &mut moc2_elem_r.moc_1_head) {
    if l.start < r.start {        // L--xx
      break;
    } else if r.end <= l.start { // R--RL--L
      moc_1_ranges_collector.push(moc2_elem_r.next_1().unwrap());
    } else if r.start <= l.start { 
      if r.end < l.end { // R--L--R--L
        l.start = r.end;
        moc_1_ranges_collector.push(moc2_elem_r.next_1().unwrap());
        break;
      } else {           // R--L--L--R
        moc2_elem_l.consume_moc_1_till_range_end_le(r.end);
        // moc_1_ranges_collector.push(moc2_elem_r.next_1().unwrap());
      }
    }
  }
}

fn perform_union_while_right_included_in_left<T, Q, U, R, I1, I2>(
  moc2_elem_l: &mut MOC2Elem<T, Q, U, R, I1>,
  moc2_elem_r:  &mut MOC2Elem<T, Q, U, R, I2>,
  moc_1_ranges_collector: &mut Vec<Range<T>>
)
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: RangeMOCIterator<T, Qty=Q>,
{
  perform_union_while_left_included_in_right(moc2_elem_r, moc2_elem_l, moc_1_ranges_collector)
}

fn perform_union_while_equal_ranges<T, Q, U, R, I1, I2>(
  moc2_elem_l: &mut MOC2Elem<T, Q, U, R, I1>,
  moc2_elem_r:  &mut MOC2Elem<T, Q, U, R, I2>,
  moc_1_ranges_collector: &mut Vec<Range<T>>
)
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: RangeMOCIterator<T, Qty=Q>,
{
  while let (Some(ref mut l), Some(ref mut r)) = (&mut moc2_elem_l.moc_1_head, &mut moc2_elem_r.moc_1_head) {
    if l.start == r.start {
      match l.end.cmp(&r.end) {
        Ordering::Less => {
          r.start = l.end;
          moc_1_ranges_collector.push(moc2_elem_l.next_1().unwrap());
          break;
        },
        Ordering::Greater => {
          l.start = r.end;
          moc_1_ranges_collector.push(moc2_elem_r.next_1().unwrap());
          break;
        },
        Ordering::Equal => {
          moc2_elem_l.next_1().unwrap();
          moc_1_ranges_collector.push(moc2_elem_r.next_1().unwrap());
        },
      }
    } else {
      break;
    }
  }
}

fn perform_union_while_equal_ranges_skip_first_check<T, Q, U, R, I1, I2>(
  moc2_elem_l: &mut MOC2Elem<T, Q, U, R, I1>,
  moc2_elem_r:  &mut MOC2Elem<T, Q, U, R, I2>,
  moc_1_ranges_collector: &mut Vec<Range<T>>
)
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: RangeMOCIterator<T, Qty=Q>,
{
  perform_union_while_equal_ranges(moc2_elem_l, moc2_elem_r, moc_1_ranges_collector)
}

impl<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2> RangeMOC2Iterator<
  T, Q, RangeMocIter<T, Q>,
  U, R, RangeMocIter<U, R>, 
  RangeMOC2Elem<T, Q, U, R>
>
for OrRange2Iter<T, Q, U, R, I1, J1, K1, L1, I2, J2, K2, L2>
  where
    T: Idx,
    Q: MocQty<T>,
    U: Idx,
    R: MocQty<U>,
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    L1: RangeMOC2Iterator<T, Q, I1, U, R, J1, K1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>,
    L2: RangeMOC2Iterator<T, Q, I2, U, R, J2, K2> { }
