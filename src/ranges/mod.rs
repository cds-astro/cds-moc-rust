//! Very generic ranges operations

use std::cmp;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::mem;
use std::ops::{Index, Range};
use std::ptr::slice_from_raw_parts;
use std::slice::Iter;

use num::{Integer, One, PrimInt, Zero};

#[cfg(not(target_arch = "wasm32"))]
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

use crate::{idx::Idx, utils};

pub mod ranges2d;

/// Generic operations on a set of Sorted and Non-Overlapping ranges.
/// SNO = Sorted Non-Overlapping
pub trait SNORanges<'a, T: Idx>: Sized {
  type OwnedRanges: SNORanges<'a, T>;

  type Iter: Iterator<Item = &'a Range<T>>;
  #[cfg(not(target_arch = "wasm32"))]
  type ParIter: ParallelIterator<Item = &'a Range<T>>;

  fn is_empty(&self) -> bool;

  /// The iterator **MUST** return sorted and non-overlapping ranges
  fn iter(&'a self) -> Self::Iter;

  #[cfg(not(target_arch = "wasm32"))]
  fn par_iter(&'a self) -> Self::ParIter;

  fn intersects_range(&self, x: &Range<T>) -> bool;
  #[cfg(not(target_arch = "wasm32"))]
  fn par_intersects_range(&'a self, x: &Range<T>) -> bool {
    self
      .par_iter()
      .map(|r| !(x.start >= r.end || x.end <= r.start))
      .any(|a| a)
  }

  fn contains_val(&self, x: &T) -> bool;

  fn contains_range(&self, x: &Range<T>) -> bool;
  #[cfg(not(target_arch = "wasm32"))]
  fn par_contains_range(&'a self, x: &Range<T>) -> bool {
    self
      .par_iter()
      .map(|r| x.start >= r.start && x.end <= r.end)
      .any(|a| a)
  }

  fn contains(&self, rhs: &'a Self) -> bool {
    // TODO: implement a more efficient algo, avoiding to re-explore the sub-part of self
    // already explored (since rhs ranges are also ordered!)
    for range in rhs.iter() {
      if !self.contains_range(range) {
        return false;
      }
    }
    true
  }

  /// Returns the sum of the ranges lengths
  fn range_sum(&'a self) -> T {
    let mut sum = T::zero();
    for Range { start, end } in self.iter() {
      sum += *end - *start;
    }
    sum
  }

  fn intersects(&self, rhs: &Self) -> bool;

  fn merge(&self, other: &Self, op: impl Fn(bool, bool) -> bool) -> Self::OwnedRanges;

  fn union(&self, other: &Self) -> Self::OwnedRanges;
  /*fn union(&self, other: &Self) -> Self {
      self.merge(other, |a, b| a || b)
  }*/

  fn intersection(&self, other: &Self) -> Self::OwnedRanges;
  /*fn intersection(&self, other: &Self) -> Self {
      self.merge(other, |a, b| a && b)
  }*/

  //  = minus
  // != (symmtric difference = xor)
  fn difference(&self, other: &Self) -> Self::OwnedRanges {
    self.merge(other, |a, b| a && !b)
  }

  /// Returns the complement assuming that the possible values are
  /// in `[0, upper_bound_exclusive[`.
  fn complement_with_upper_bound(&self, upper_bound_exclusive: T) -> Self::OwnedRanges;

  /// Performs a logical OR between all the bounds of all ranges,
  /// and returns the number of trailing zeros.
  fn trailing_zeros(&'a self) -> u8 {
    let all_vals_or: T = self
      .iter()
      .fold(Zero::zero(), |res, x| res | x.start | x.end);
    all_vals_or.trailing_zeros() as u8
  }

  /// Performs a logical OR between all the bounds of all ranges,
  /// and returns the number of trailing zeros.
  #[cfg(not(target_arch = "wasm32"))]
  fn par_trailing_zeros(&'a self) -> u8 {
    let all_vals_or: T = self
      .par_iter()
      .fold(T::zero, |res, x| res | x.start | x.end)
      .reduce(T::zero, |a, b| a | b);
    all_vals_or.trailing_zeros() as u8
  }
}

/// Generic Range operations
/// We use a `Boxed array` instead of `Vec` because we use possibly a lot of them in
/// 2D-MOCs, so in may because not negligible to spare the capacity info.
#[derive(Clone, Debug)]
pub struct Ranges<T: Idx>(pub Box<[Range<T>]>);

impl<T: Idx> Default for Ranges<T> {
  fn default() -> Self {
    Ranges(Default::default())
  }
}

impl<T: Idx> Ranges<T> {
  /// Assumes (without checking!) that the input vector of range is already sorted and do not
  /// contains overlapping (or consecutive) ranges
  pub fn new_unchecked(data: Vec<Range<T>>) -> Self {
    Ranges(data.into_boxed_slice())
  }

  /// Assumes (without checking!) that the input vector of range is already sorted **BUT**
  /// may contains overlapping (or consecutive) ranges.
  pub fn new_from_sorted(data: Vec<Range<T>>) -> Self {
    Self::new_unchecked(MergeOverlappingRangesIter::new(data.iter(), None).collect::<Vec<_>>())
  }

  /// Internally sorts the input vector and ensures there is no overlapping (or consecutive) ranges.
  pub fn new_from(mut data: Vec<Range<T>>) -> Self {
    #[cfg(not(target_arch = "wasm32"))]
    data.par_sort_unstable_by(|left, right| left.start.cmp(&right.start));
    #[cfg(target_arch = "wasm32")]
    (&mut data).sort_unstable_by(|left, right| left.start.cmp(&right.start));
    Self::new_from_sorted(data)
  }

  pub fn as_bytes(&self) -> &[u8] {
    let offset = self.0.as_ptr().align_offset(mem::align_of::<u8>());
    // I suppose that align with u8 is never a problem.
    assert_eq!(offset, 0);
    unsafe {
      std::slice::from_raw_parts(
        self.0.as_ptr() as *const u8,
        (self.0.len() << 1) * std::mem::size_of::<T>(),
      )
    }
  }

  /*/// Make the `Ranges<T>` consistent
  ///
  /// # Info
  ///
  /// By construction, the data are sorted so that it is possible (see the new
  /// method definition above) to merge the overlapping ranges.
  pub fn make_consistent(mut self) -> Self {
      self.0 = MergeOverlappingRangesIter::new(self.iter(), None).collect::<Vec<_>>();
      self
  }*/
}

impl<T: Idx> PartialEq for Ranges<T> {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl<T: Idx> Index<usize> for Ranges<T> {
  type Output = Range<T>;

  fn index(&self, index: usize) -> &Range<T> {
    &self.0[index]
  }
}

impl<'a, T: Idx> SNORanges<'a, T> for Ranges<T> {
  type OwnedRanges = Self;

  type Iter = Iter<'a, Range<T>>;
  #[cfg(not(target_arch = "wasm32"))]
  type ParIter = rayon::slice::Iter<'a, Range<T>>;

  fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  fn iter(&'a self) -> Self::Iter {
    self.0.iter()
  }

  #[cfg(not(target_arch = "wasm32"))]
  fn par_iter(&'a self) -> Self::ParIter {
    self.0.par_iter()
  }

  fn intersects_range(&self, x: &Range<T>) -> bool {
    BorrowedRanges(&self.0).intersects_range(x)
  }

  fn contains_val(&self, x: &T) -> bool {
    BorrowedRanges(&self.0).contains_val(x)
  }

  fn contains_range(&self, x: &Range<T>) -> bool {
    BorrowedRanges(&self.0).contains_range(x)
  }

  fn intersects(&self, rhs: &Self) -> bool {
    BorrowedRanges(&self.0).intersects(&BorrowedRanges(&rhs.0))
  }

  fn union(&self, other: &Self) -> Self {
    BorrowedRanges(&self.0).union(&BorrowedRanges(&other.0))
  }

  fn intersection(&self, other: &Self) -> Self {
    BorrowedRanges(&self.0).intersection(&BorrowedRanges(&other.0))
  }

  #[allow(clippy::many_single_char_names)]
  fn merge(&self, other: &Self, op: impl Fn(bool, bool) -> bool) -> Self {
    BorrowedRanges(&self.0).merge(&BorrowedRanges(&other.0), op)
  }

  fn complement_with_upper_bound(&self, upper_bound_exclusive: T) -> Self {
    BorrowedRanges(&self.0).complement_with_upper_bound(upper_bound_exclusive)
  }
}

/// To perform operations on e.g. a buffer without having to own the ranges
pub struct BorrowedRanges<'a, T: Idx>(pub &'a [Range<T>]);

impl<'a, T: Idx> From<&'a Ranges<T>> for BorrowedRanges<'a, T> {
  fn from(ranges: &'a Ranges<T>) -> Self {
    BorrowedRanges(&ranges.0)
  }
}

impl<'a, T: Idx> From<&'a [u8]> for BorrowedRanges<'a, T> {
  fn from(blob: &'a [u8]) -> Self {
    let offset = blob.as_ptr().align_offset(mem::align_of::<Range<T>>());
    if offset != 0 {
      panic!("Not aligned array!!");
    }
    let len = blob.len() / std::mem::size_of::<Range<T>>();
    debug_assert_eq!(blob.len(), len * std::mem::size_of::<Range<T>>());
    BorrowedRanges(unsafe { &*slice_from_raw_parts(blob.as_ptr() as *const Range<T>, len) })
  }
}

impl<'a, T: Idx> BorrowedRanges<'a, T> {
  pub fn as_bytes(&self) -> &[u8] {
    let offset = self.0.as_ptr().align_offset(mem::align_of::<u8>());
    // I suppose that align with u8 is never a problem.
    assert_eq!(offset, 0);
    unsafe {
      std::slice::from_raw_parts(
        self.0.as_ptr() as *const u8,
        (self.0.len() << 1) * std::mem::size_of::<T>(),
      )
    }
  }
}

impl<'a, T: Idx> SNORanges<'a, T> for BorrowedRanges<'a, T> {
  type OwnedRanges = Ranges<T>;

  type Iter = Iter<'a, Range<T>>;
  #[cfg(not(target_arch = "wasm32"))]
  type ParIter = rayon::slice::Iter<'a, Range<T>>;

  fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  fn iter(&'a self) -> Self::Iter {
    self.0.iter()
  }

  #[cfg(not(target_arch = "wasm32"))]
  fn par_iter(&'a self) -> Self::ParIter {
    self.0.par_iter()
  }

  // The MOC **MUST BE** consistent!
  fn intersects_range(&self, x: &Range<T>) -> bool {
    let len = self.0.len();
    // Quick rejection test
    if len == 0 || x.end <= self.0[0].start || self.0[len - 1].end <= x.start {
      return false;
    }
    let len = len << 1;
    let ptr = self.0.as_ptr();
    // It is ok because (see test test_alignment()):
    // * T is a primitive type in practice, and size_of(T) << 1 == size_of(Range<T>)
    // * the alignment of Range<T> is the same as the alignment of T (see test_alignment)
    // But: https://rust-lang.github.io/unsafe-code-guidelines/layout/structs-and-tuples.html
    let result: &[T] = unsafe { &*slice_from_raw_parts(ptr as *const T, len) };
    match result.binary_search(&x.start) {
      Ok(i) => {
        i & 1 == 0 // even index => x.start = range.start => Ok
                  || (i + 1 < len && x.end > result[i + 1]) // else (odd index) => upper bound => Check x.end > next range.start
      }
      Err(i) => {
        i & 1 == 1 // index is odd => x.start inside the range
                  || (i < len && x.end > result[i]) // else (even index) => Check x.end > range.start
      }
    }
  }

  // The MOC **MUST BE** consistent!
  fn contains_val(&self, x: &T) -> bool {
    let len = self.0.len();
    // Quick rejection test
    if len == 0 || *x < self.0[0].start || self.0[len - 1].end <= *x {
      return false;
    }
    let len = len << 1;
    let ptr = self.0.as_ptr();
    // It is ok because (see test test_alignment()):
    // * T is a primitive type in practice, and size_of(T) << 1 == size_of(Range<T>)
    // * the alignment of Range<T> is the same as the alignment of T (see test_alignment)
    // But: https://rust-lang.github.io/unsafe-code-guidelines/layout/structs-and-tuples.html
    let result: &[T] = unsafe { &*slice_from_raw_parts(ptr as *const T, len) };
    // Performs a binary search in it
    match result.binary_search(x) {
      Ok(i) => i & 1 == 0,  // index must be even (lower bound of a range)
      Err(i) => i & 1 == 1, // index must be odd (inside a range)
    }
  }

  // The MOC **MUST BE** consistent!
  fn contains_range(&self, x: &Range<T>) -> bool {
    let len = self.0.len();
    // Quick rejection test
    if len == 0 || x.end <= self.0[0].start || self.0[len - 1].end <= x.start {
      return false;
    }
    let len = len << 1;
    let ptr = self.0.as_ptr();
    // It is ok because (see test test_alignment()):
    // * T is a primitive type in practice, and size_of(T) << 1 == size_of(Range<T>)
    // * the alignment of Range<T> is the same as the alignment of T (see test_alignment)
    // But: https://rust-lang.github.io/unsafe-code-guidelines/layout/structs-and-tuples.html
    let result: &[T] = unsafe { &*slice_from_raw_parts(ptr as *const T, len) };
    // Performs a binary search in it
    match result.binary_search(&x.start) {
      Ok(i) => i & 1 == 0 && x.end <= result[i | 1], // index must be even (lower bound of a range)
      Err(i) => i & 1 == 1 && x.end <= result[i],    // index must be odd (inside a range)
    }
  }

  fn intersects(&self, rhs: &Self) -> bool {
    // Quickly adaptedd from "intersection", we may find a better option
    let l = &self.0;
    let r = &rhs.0;
    // Quick rejection test
    if l.is_empty()
      || r.is_empty()
      || l[0].start >= r[r.len() - 1].end
      || l[l.len() - 1].end <= r[0].start
    {
      return false;
    }
    // Use binary search to find the starting indices
    let (il, ir) = match l[0].start.cmp(&r[0].start) {
      Ordering::Less => {
        let il = match l.binary_search_by(|l_range| l_range.start.cmp(&r[0].start)) {
          Ok(i) => i,
          Err(i) => i - 1,
        };
        (il, 0)
      }
      Ordering::Greater => {
        let ir = match r.binary_search_by(|r_range| r_range.start.cmp(&l[0].start)) {
          Ok(i) => i,
          Err(i) => i - 1,
        };
        (0, ir)
      }
      Ordering::Equal => (0, 0),
    };
    // Now simple sequential algo
    let mut left_it = l[il..].iter();
    let mut right_it = r[ir..].iter();
    let mut left = left_it.next();
    let mut right = right_it.next();
    while let (Some(el), Some(er)) = (&left, &right) {
      if el.end <= er.start {
        // |--l--| |--r--|
        left = left_it.next();
        while let Some(el) = &left {
          if el.end <= er.start {
            left = left_it.next();
          } else {
            break;
          }
        }
      } else if er.end <= el.start {
        // |--r--| |--l--|
        right = right_it.next();
        while let Some(er) = &right {
          if er.end <= el.start {
            right = right_it.next();
          } else {
            break;
          }
        }
      } else {
        return true;
      }
    }
    false
  }

  #[allow(clippy::many_single_char_names)]
  fn merge(&self, other: &Self, op: impl Fn(bool, bool) -> bool) -> Self::OwnedRanges {
    let l = &self.0;
    let ll = l.len() << 1;

    let r = &other.0;
    let rl = r.len() << 1;

    let mut i = 0;
    let mut j = 0;

    let mut result = Vec::with_capacity(ll + rl);

    while i < ll || j < rl {
      let (in_l, in_r, c) = if i == ll {
        let rv = if j & 0x1 != 0 {
          r[j >> 1].end
        } else {
          r[j >> 1].start
        };

        let on_rising_edge_t2 = (j & 0x1) == 0;

        let in_l = false;
        let in_r = on_rising_edge_t2;

        j += 1;

        (in_l, in_r, rv)
      } else if j == rl {
        let lv = if i & 0x1 != 0 {
          l[i >> 1].end
        } else {
          l[i >> 1].start
        };

        let on_rising_edge_t1 = (i & 0x1) == 0;

        let in_l = on_rising_edge_t1;
        let in_r = false;

        i += 1;

        (in_l, in_r, lv)
      } else {
        let lv = if i & 0x1 != 0 {
          l[i >> 1].end
        } else {
          l[i >> 1].start
        };
        let rv = if j & 0x1 != 0 {
          r[j >> 1].end
        } else {
          r[j >> 1].start
        };

        let on_rising_edge_t1 = (i & 0x1) == 0;
        let on_rising_edge_t2 = (j & 0x1) == 0;

        let c = cmp::min(lv, rv);

        let in_l = (on_rising_edge_t1 && c == lv) | (!on_rising_edge_t1 && c < lv);
        let in_r = (on_rising_edge_t2 && c == rv) | (!on_rising_edge_t2 && c < rv);

        if c == lv {
          i += 1;
        }
        if c == rv {
          j += 1;
        }

        (in_l, in_r, c)
      };

      let closed = (result.len() & 0x1) == 0;

      let add = !(closed ^ op(in_l, in_r));
      if add {
        result.push(c);
      }
    }
    result.shrink_to_fit();
    Ranges::new_unchecked(utils::unflatten(&mut result))
  }

  fn union(&self, other: &Self) -> Self::OwnedRanges {
    let l = self.0;
    let r = other.0;
    // Deal with cases in which one of the two ranges (or both) is empty
    if l.is_empty() {
      return Ranges(r.to_vec().into_boxed_slice());
    } else if r.is_empty() {
      return Ranges(l.to_vec().into_boxed_slice());
    }
    // Trivial case (simple concatenation)
    // - unwrap is safe here, because we test l.is_empty() and r.ie_empty() before
    if l.last().unwrap().end < r.first().unwrap().start {
      return Ranges::new_unchecked(l.iter().cloned().chain(r.iter().cloned()).collect());
    } else if r.last().unwrap().end < l.first().unwrap().start {
      return Ranges::new_unchecked(r.iter().cloned().chain(l.iter().cloned()).collect());
    }
    // Init result
    let mut res = Vec::with_capacity(l.len() + r.len());
    // Else, Use binary search to find the starting indices (and starts with a simple array copy)
    let (il, ir) = if l[0].end < r[0].start {
      let il = match l.binary_search_by(|l_range| l_range.end.cmp(&r[0].start)) {
        Ok(i) => i,
        Err(i) => i,
      };
      res.extend_from_slice(&l[..il]);
      (il, 0)
    } else if l[0].start > r[0].end {
      let ir = match r.binary_search_by(|r_range| r_range.end.cmp(&l[0].start)) {
        Ok(i) => i,
        Err(i) => i,
      };
      res.extend_from_slice(&r[..ir]);
      (0, ir)
    } else {
      (0, 0)
    };
    // Now regular work
    let mut left_it = l[il..].iter();
    let mut right_it = r[ir..].iter();
    let mut left = left_it.next().cloned();
    let mut right = right_it.next().cloned();

    fn consume_while_end_lower_than<'a, T, I>(it: &mut I, to: T) -> Option<&'a Range<T>>
    where
      T: Idx,
      I: Iterator<Item = &'a Range<T>>,
    {
      let mut curr = it.next();
      while let Some(c) = &curr {
        if c.end > to {
          break;
        } else {
          curr = it.next();
        }
      }
      curr
    }

    loop {
      match (&mut left, &mut right) {
        (Some(ref mut l), Some(ref mut r)) => {
          if l.end < r.start {
            // L--L R--R
            res.push(mem::replace(&mut left, left_it.next().cloned()).unwrap());
          } else if r.end < l.start {
            // R--R L--L
            res.push(mem::replace(&mut right, right_it.next().cloned()).unwrap());
          } else if l.end <= r.end {
            //    R--L--L--R
            if l.start < r.start {
              // or L--R--L--R
              r.start = l.start;
            }
            left = consume_while_end_lower_than(&mut left_it, r.end).cloned();
          } else {
            //    L--R--R--L
            if r.start < l.start {
              // or R--L--R--L
              l.start = r.start;
            }
            right = consume_while_end_lower_than(&mut right_it, l.end).cloned();
          }
        }
        (Some(ref l), None) => {
          res.push(l.clone());
          for l in left_it {
            res.push(l.clone());
          }
          break;
        }
        (None, Some(ref r)) => {
          res.push(r.clone());
          for r in right_it {
            res.push(r.clone());
          }
          break;
        }
        (None, None) => break,
      };
    }
    res.shrink_to_fit();
    Ranges::new_unchecked(res)
  }

  fn intersection(&self, other: &Self) -> Self::OwnedRanges {
    // utils.flatten()/unfallten()
    let l = &self.0;
    let r = &other.0;
    // Quick rejection test
    if l.is_empty()
      || r.is_empty()
      || l[0].start >= r[r.len() - 1].end
      || l[l.len() - 1].end <= r[0].start
    {
      return Ranges(Default::default());
    }
    // Use binary search to find the starting indices
    let (il, ir) = match l[0].start.cmp(&r[0].start) {
      Ordering::Less => {
        let il = match l.binary_search_by(|l_range| l_range.start.cmp(&r[0].start)) {
          Ok(i) => i,
          Err(i) => i - 1,
        };
        (il, 0)
      }
      Ordering::Greater => {
        let ir = match r.binary_search_by(|r_range| r_range.start.cmp(&l[0].start)) {
          Ok(i) => i,
          Err(i) => i - 1,
        };
        (0, ir)
      }
      Ordering::Equal => (0, 0),
    };
    // Now simple sequential algo
    let mut res = Vec::with_capacity(l.len() + r.len() - (il + ir));
    let mut left_it = l[il..].iter();
    let mut right_it = r[ir..].iter();
    let mut left = left_it.next();
    let mut right = right_it.next();
    while let (Some(el), Some(er)) = (&left, &right) {
      if el.end <= er.start {
        // |--l--| |--r--|
        left = left_it.next();
        while let Some(el) = &left {
          if el.end <= er.start {
            left = left_it.next();
          } else {
            break;
          }
        }
      } else if er.end <= el.start {
        // |--r--| |--l--|
        right = right_it.next();
        while let Some(er) = &right {
          if er.end <= el.start {
            right = right_it.next();
          } else {
            break;
          }
        }
      } else {
        let from = el.start.max(er.start);
        match el.end.cmp(&er.end) {
          Ordering::Less => {
            res.push(from..el.end);
            left = left_it.next();
          }
          Ordering::Greater => {
            res.push(from..er.end);
            right = right_it.next();
          }
          Ordering::Equal => {
            res.push(from..el.end);
            left = left_it.next();
            right = right_it.next();
          }
        }
      }
    }
    res.shrink_to_fit();
    Ranges::new_unchecked(res)
  }

  fn complement_with_upper_bound(&self, upper_bound_exclusive: T) -> Self::OwnedRanges {
    let ranges = &self.0;
    let mut result = Vec::<Range<T>>::with_capacity(ranges.len() + 1);

    if self.is_empty() {
      result.push(T::zero()..upper_bound_exclusive);
    } else {
      let mut s = 0;
      let mut last = if ranges[0].start == T::zero() {
        s = 1;
        ranges[0].end
      } else {
        T::zero()
      };

      result = ranges
        .iter()
        .skip(s)
        .map(|range| {
          let r = last..range.start;
          last = range.end;
          r
        })
        .collect::<Vec<_>>();

      if last < upper_bound_exclusive {
        result.push(last..upper_bound_exclusive);
      }
    }
    Ranges::new_unchecked(result)
  }
}

#[derive(Debug)]
pub struct MergeOverlappingRangesIter<'a, T>
where
  T: Integer,
{
  last: Option<Range<T>>,
  ranges: Iter<'a, Range<T>>,
  split_ranges: VecDeque<Range<T>>,
  shift: Option<u32>,
}

impl<'a, T> MergeOverlappingRangesIter<'a, T>
where
  T: Integer + PrimInt,
{
  pub fn new(
    mut ranges: Iter<'a, Range<T>>,
    shift: Option<u32>,
  ) -> MergeOverlappingRangesIter<'a, T> {
    let last = ranges.next().cloned();
    let split_ranges = VecDeque::<Range<T>>::new();
    MergeOverlappingRangesIter {
      last,
      ranges,
      split_ranges,
      shift,
    }
  }

  fn split_range(&self, range: Range<T>) -> VecDeque<Range<T>> {
    let mut ranges = VecDeque::<Range<T>>::new();
    match self.shift {
      None => {
        ranges.push_back(range);
      }
      Some(shift) => {
        let mut mask: T = One::one();
        mask = mask.unsigned_shl(shift) - One::one();

        if range.end - range.start < mask {
          ranges.push_back(range);
        } else {
          let offset = range.start & mask;
          let mut s = range.start;
          if offset > Zero::zero() {
            s = (range.start - offset) + mask + One::one();
            ranges.push_back(range.start..s);
          }

          while s + mask + One::one() < range.end {
            let next = s + mask + One::one();
            ranges.push_back(s..next);
            s = next;
          }

          ranges.push_back(s..range.end);
        }
      }
    }
    ranges
  }
}

impl<'a, T> Iterator for MergeOverlappingRangesIter<'a, T>
where
  T: Integer + PrimInt,
{
  type Item = Range<T>;

  fn next(&mut self) -> Option<Self::Item> {
    if !self.split_ranges.is_empty() {
      return self.split_ranges.pop_front();
    }

    while let Some(curr) = self.ranges.next() {
      let prev = self.last.as_mut().unwrap();
      if curr.start <= prev.end {
        prev.end = cmp::max(curr.end, prev.end);
      } else {
        let range = self.last.clone();
        self.last = Some(curr.clone());

        self.split_ranges = self.split_range(range.unwrap());
        return self.split_ranges.pop_front();
      }
    }

    if self.last.is_some() {
      let range = self.last.clone();
      self.last = None;

      self.split_ranges = self.split_range(range.unwrap());
      self.split_ranges.pop_front()
    } else {
      None
    }
  }
}

#[cfg(test)]
mod tests {

  use std::mem::{align_of, size_of};
  use std::ops::Range;

  use num::PrimInt;
  use rand::Rng;

  use crate::qty::{Hpx, MocQty};
  use crate::ranges::{Ranges, SNORanges};

  #[test]
  fn test_alignment() {
    // Ensure alignement for unsafe code in contains
    assert_eq!(align_of::<u32>(), align_of::<Range<u32>>());
    assert_eq!(align_of::<u64>(), align_of::<Range<u64>>());
    assert_eq!(align_of::<i32>(), align_of::<Range<i32>>());
    assert_eq!(align_of::<i64>(), align_of::<Range<i64>>());

    assert_eq!(size_of::<u32>() << 1, size_of::<Range<u32>>());
    assert_eq!(size_of::<u64>() << 1, size_of::<Range<u64>>());
    assert_eq!(size_of::<i32>() << 1, size_of::<Range<i32>>());
    assert_eq!(size_of::<i64>() << 1, size_of::<Range<i64>>());
  }

  #[test]
  fn merge_range() {
    fn assert_merge(a: Vec<Range<u64>>, expected: Vec<Range<u64>>) {
      let ranges = Ranges::<u64>::new_from(a);
      let expected_ranges = Ranges::<u64>::new_unchecked(expected);

      assert_eq!(ranges, expected_ranges);
    }

    assert_merge(vec![12..17, 3..5, 5..7, 6..8], vec![3..8, 12..17]);
    assert_merge(vec![0..1, 2..5], vec![0..1, 2..5]);
    assert_merge(vec![], vec![]);
    assert_merge(vec![0..6, 7..9, 8..13], vec![0..6, 7..13]);
  }

  #[test]
  fn test_union() {
    fn assert_union(a: Vec<Range<u64>>, b: Vec<Range<u64>>, expected: Vec<Range<u64>>) {
      let a = Ranges::<u64>::new_from(a);
      let b = Ranges::<u64>::new_from(b);

      let expected_ranges = Ranges::<u64>::new_unchecked(expected);
      let ranges = a.union(&b);
      assert_eq!(ranges, expected_ranges);
    }

    assert_union(
      vec![12..17, 3..5, 5..7, 6..8],
      vec![0..1, 2..5],
      vec![0..1, 2..8, 12..17],
    );
    assert_union(
      vec![12..17, 3..5, 5..7, 6..8],
      vec![12..17, 3..5, 5..7, 6..8],
      vec![3..8, 12..17],
    );
    assert_union(vec![], vec![], vec![]);
    assert_union(vec![12..17], vec![], vec![12..17]);
    assert_union(vec![], vec![12..17], vec![12..17]);
    assert_union(vec![0..1, 2..3, 4..5], vec![1..22], vec![0..22]);
    assert_union(vec![0..10], vec![15..22], vec![0..10, 15..22]);
  }

  #[test]
  fn test_intersection() {
    fn assert_intersection(a: Vec<Range<u64>>, b: Vec<Range<u64>>, expected: Vec<Range<u64>>) {
      let a = Ranges::<u64>::new_from(a);
      let b = Ranges::<u64>::new_from(b);

      let expected_ranges = Ranges::<u64>::new_unchecked(expected);
      let ranges = a.intersection(&b);
      assert_eq!(ranges, expected_ranges);
    }

    assert_intersection(vec![12..17, 3..5, 5..7, 6..8], vec![0..1, 2..5], vec![3..5]);
    assert_intersection(vec![], vec![0..1, 2..5], vec![]);
    assert_intersection(vec![], vec![], vec![]);
    assert_intersection(vec![2..6], vec![0..3, 4..8], vec![2..3, 4..6]);
    assert_intersection(vec![2..6], vec![2..6, 7..8], vec![2..6]);
    assert_intersection(vec![10..11], vec![10..11], vec![10..11]);
  }

  #[test]
  fn test_difference() {
    fn assert_difference(a: Vec<Range<u64>>, b: Vec<Range<u64>>, expected: Vec<Range<u64>>) {
      let a = Ranges::<u64>::new_from(a);
      let b = Ranges::<u64>::new_from(b);

      let expected_ranges = Ranges::<u64>::new_unchecked(expected);
      let ranges = a.difference(&b);
      assert_eq!(ranges, expected_ranges);
    }

    assert_difference(vec![0..20], vec![5..7], vec![0..5, 7..20]);
    assert_difference(vec![0..20], vec![0..20], vec![]);
    assert_difference(vec![0..20], vec![], vec![0..20]);
    assert_difference(vec![], vec![0..5], vec![]);
    assert_difference(vec![0..20], vec![19..22], vec![0..19]);
    assert_difference(vec![0..20], vec![25..27], vec![0..20]);
    assert_difference(
      vec![0..20],
      vec![1..2, 3..4, 5..6],
      vec![0..1, 2..3, 4..5, 6..20],
    );
  }

  #[test]
  fn test_uniq_decompose() {
    macro_rules! uniq_to_pix_depth {
      ($t:ty, $size:expr) => {
        let mut rng = rand::thread_rng();

        (0..$size).for_each(|_| {
          let depth = rng.gen_range(Range {
            start: 0,
            end: Hpx::<$t>::MAX_DEPTH,
          });

          let npix = 12 * 4.pow(depth as u32);
          let pix = rng.gen_range(Range {
            start: 0,
            end: npix,
          });

          let uniq = 4 * 4.pow(depth as u32) + pix;
          assert_eq!(Hpx::<$t>::from_uniq_hpx(uniq), (depth, pix));
        });
      };
    }

    uniq_to_pix_depth!(u128, 10000);
    uniq_to_pix_depth!(u64, 10000);
    uniq_to_pix_depth!(u32, 10000);
    uniq_to_pix_depth!(u8, 10000);
  }

  /*use test::Bencher;

  #[bench]
  fn bench_uniq_to_depth_pix(b: &mut Bencher) {
      let mut rng = rand::thread_rng();
      let n = test::black_box(100000);

      let uniq: Vec<u64> = (0..n)
          .map(|_| {
              let depth = rng.gen_range(0, 30);

              let npix = 12 * 4.pow(depth);
              let pix = rng.gen_range(0, npix);

              let u = 4 * 4.pow(depth) + pix;
              u
          })
          .collect();

      b.iter(|| {
          uniq.iter()
              .fold(0, |a, b| a + (u64::pix_depth(*b).0 as u64))
      });
  }*/
}
