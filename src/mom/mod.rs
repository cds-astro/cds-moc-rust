//! Multi-Ordered healpix Map (MOM)
//! Here we assume that a MOM a simply a set of of `(key, value)` pairs in which:
//! * the `key` is a UNIQ NESTED HEALPix cell number
//! * all the `keys` in the set are non-overlapping
//! * we assume nothing on the `key` ordering
//! * the `value` (e.g. a probability) is proportional to the cell area, i.e. we can split
//!   a `(key, value)` at the order N+1 into the 4 `(key, value)` pairs:
//!     + `(key << 2 + 0, value / 4)`
//!     + `(key << 2 + 1, value / 4)`
//!     + `(key << 2 + 2, value / 4)`
//!     + `(key << 2 + 3, value / 4)`

use std::{
  f64::{self, consts::FRAC_PI_3},
  marker::PhantomData,
  ops::{AddAssign, Mul},
};

use num::Num;

use crate::{
  idx::Idx,
  moc::range::RangeMOC,
  qty::{Hpx, MocQty},
};

// 'static mean that Idx does not contains any reference
pub trait Value<T: Idx>:
  'static
  + Num
  + PartialOrd
  + Mul<f64, Output = Self>
  + AddAssign
  + Copy
  + Send
  + Sync
  + std::fmt::Debug
{
}

impl<T: Idx> Value<T> for f64 {}

/*
trait MOMIterOp {
  pub fn sum_values_in_moc
}*/

pub trait MOMIterator<T: Idx, Q: MocQty<T>, V: Value<T>>: Sized + Iterator<Item = (T, V)> {
  fn sum_values_in_moc(self, moc: &RangeMOC<T, Q>) -> V {
    let mut sum = V::zero();
    for (zuniq, value) in self {
      let (depth, ipix) = Q::from_zuniq(zuniq);
      let cell_fraction = moc.cell_fraction(depth, ipix);
      sum += value * cell_fraction;
    }
    sum
  }
}

pub trait HpxMOMIterator<T: Idx, V: Value<T>>: MOMIterator<T, Hpx<T>, V> {
  fn sum_values_in_hpxmoc(self, moc: &RangeMOC<T, Hpx<T>>) -> V {
    let mut sum = V::zero();
    for (hpx_uniq, value) in self {
      let (depth, ipix) = Hpx::<T>::from_uniq_hpx(hpx_uniq);
      let cell_fraction = moc.cell_fraction(depth, ipix);
      sum += value * cell_fraction;
    }
    sum
  }

  fn retain_values_with_weights_in_hpxmoc(
    self,
    moc: &RangeMOC<T, Hpx<T>>,
  ) -> HpxMOMFilter<'_, T, V, Self> {
    HpxMOMFilter::new(self, moc)
  }
}

pub struct HpxMomIter<T: Idx, Q: MocQty<T>, V: Value<T>, I: Sized + Iterator<Item = (T, V)>> {
  it: I,
  _phantom: PhantomData<Q>,
}
impl<T: Idx, Q: MocQty<T>, V: Value<T>, I: Sized + Iterator<Item = (T, V)>> HpxMomIter<T, Q, V, I> {
  pub fn new(it: I) -> Self {
    Self {
      it,
      _phantom: PhantomData,
    }
  }
}

impl<T: Idx, Q: MocQty<T>, V: Value<T>, I: Sized + Iterator<Item = (T, V)>> Iterator
  for HpxMomIter<T, Q, V, I>
{
  type Item = (T, V);

  fn next(&mut self) -> Option<Self::Item> {
    self.it.next()
  }
}

impl<T: Idx, Q: MocQty<T>, V: Value<T>, I: Sized + Iterator<Item = (T, V)>>
  MOMIterator<T, Hpx<T>, V> for HpxMomIter<T, Q, V, I>
{
}

impl<T: Idx, Q: MocQty<T>, V: Value<T>, I: Sized + Iterator<Item = (T, V)>> HpxMOMIterator<T, V>
  for HpxMomIter<T, Q, V, I>
{
}

/// Filter cell which are in a given MOC and Map to return
/// a value together with the sky area it covers.
pub struct HpxMOMFilter<'a, T, V, I>
where
  T: Idx,
  V: Value<T>,
  I: HpxMOMIterator<T, V>,
{
  it: I,
  moc: &'a RangeMOC<T, Hpx<T>>,
  _phantom: PhantomData<V>,
}

impl<'a, T, V, I> HpxMOMFilter<'a, T, V, I>
where
  T: Idx,
  V: Value<T>,
  I: HpxMOMIterator<T, V>,
{
  pub fn new(it: I, moc: &'a RangeMOC<T, Hpx<T>>) -> Self {
    Self {
      it,
      moc,
      _phantom: PhantomData,
    }
  }
}

impl<'a, T, V, I> Iterator for HpxMOMFilter<'a, T, V, I>
where
  T: Idx,
  V: Value<T>,
  I: HpxMOMIterator<T, V>,
{
  type Item = (V, f64); // (value, weight)

  fn next(&mut self) -> Option<Self::Item> {
    while let Some((uniq, val)) = self.it.next() {
      let (depth, ipix) = Hpx::<T>::from_uniq_hpx(uniq);
      let cell_fraction = self.moc.cell_fraction(depth, ipix);
      if cell_fraction > 0.0 {
        let cell_area = FRAC_PI_3 / (1_u64 << (depth << 1) as u32) as f64;
        return Some((val, cell_area * cell_fraction));
      }
    }
    None
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (0, self.it.size_hint().1)
  }
}
