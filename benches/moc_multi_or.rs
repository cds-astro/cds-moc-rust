
use std::fs;
use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};

use moc::{
  idx::Idx,
  qty::{Hpx, MocQty},
  elemset::range::MocRanges,
  moc::{
    RangeMOCIterator, RangeMOCIntoIterator,
    range::{RangeMOC, op::or::or},
  }
};

pub fn multi_or_naive<'a, T, Q>(
  it: Box<dyn Iterator<Item=RangeMOC<T, Q>> + 'a>
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
{
  it.fold(RangeMOC::new(0, MocRanges::default()), |acc, cur| acc.or(&cur))
}

pub fn multi_or_it<T, Q, I1, I2>(
  mut it_of_its: I2,
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: Iterator<Item=I1>,
{
  let mut moc = match (
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next())
  {
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), Some(i8)) =>
      or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), or(i7, i8))).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), None) =>
      return or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), i7)).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), None, _) =>
      return or(or(or(i1, i2), or(i3, i4)), or(i5, i6)).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), None, _, _) =>
      return or(or(or(i1, i2), or(i3, i4)), i5).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), None, _, _, _) =>
      return or(or(i1, i2), or(i3, i4)).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
      return or(or(i1, i2), i3).into_range_moc(),
    (Some(i1), Some(i2), None, _, _, _, _, _) =>
      return or(i1, i2).into_range_moc(),
    (Some(i1), None, _, _, _, _, _, _) =>
      return i1.into_range_moc(),
    (None, _, _, _, _, _, _, _) =>
      return RangeMOC::new(0, MocRanges::default()),
  };
  loop {
    moc = match (
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next())
    {
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), Some(i8)) =>
        or(moc.into_range_moc_iter(), or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), or(i7, i8)))).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), None) =>
        return or(moc.into_range_moc_iter(),or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), i7))).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), None, _) =>
        return or(moc.into_range_moc_iter(), or(or(or(i1, i2), or(i3, i4)), or(i5, i6))).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), None, _, _) =>
        return or(moc.into_range_moc_iter(), or(or(or(i1, i2), or(i3, i4)), i5)).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), Some(i4), None, _, _, _) =>
        return or(moc.into_range_moc_iter(), or(or(i1, i2), or(i3, i4))).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
        return or(moc.into_range_moc_iter(), or(or(i1, i2), i3)).into_range_moc(),
      (Some(i1), Some(i2), None, _, _, _, _, _) =>
        return or(moc.into_range_moc_iter(), or(i1, i2)).into_range_moc(),
      (Some(i1), None, _, _, _, _, _, _) =>
        return or(moc.into_range_moc_iter(), i1).into_range_moc(),
      (None, _, _, _, _, _, _, _) =>
        return moc,
    };
  }
}

pub fn multi_or_it_boxdyn<'a, T, Q, I>(
  mut it_of_its: Box<dyn Iterator<Item=I> + 'a>
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
    I: RangeMOCIterator<T, Qty=Q>,
{
  let mut moc = match (
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next())
  {
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), Some(i8)) =>
      or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), or(i7, i8))).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), None) =>
      return or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), i7)).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), None, _) =>
      return or(or(or(i1, i2), or(i3, i4)), or(i5, i6)).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), None, _, _) =>
      return or(or(or(i1, i2), or(i3, i4)), i5).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), None, _, _, _) =>
      return or(or(i1, i2), or(i3, i4)).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
      return or(or(i1, i2), i3).into_range_moc(),
    (Some(i1), Some(i2), None, _, _, _, _, _) =>
      return or(i1, i2).into_range_moc(),
    (Some(i1), None, _, _, _, _, _, _) =>
      return i1.into_range_moc(),
    (None, _, _, _, _, _, _, _) =>
      return RangeMOC::new(0, MocRanges::default()),
  };
  loop {
    moc = match (
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next())
    {
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), Some(i8)) =>
        or(moc.into_range_moc_iter(), or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), or(i7, i8)))).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), None) =>
        return or(moc.into_range_moc_iter(),or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), i7))).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), None, _) =>
        return or(moc.into_range_moc_iter(), or(or(or(i1, i2), or(i3, i4)), or(i5, i6))).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), None, _, _) =>
        return or(moc.into_range_moc_iter(), or(or(or(i1, i2), or(i3, i4)), i5)).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), Some(i4), None, _, _, _) =>
        return or(moc.into_range_moc_iter(), or(or(i1, i2), or(i3, i4))).into_range_moc(),
      (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
        return or(moc.into_range_moc_iter(), or(or(i1, i2), i3)).into_range_moc(),
      (Some(i1), Some(i2), None, _, _, _, _, _) =>
        return or(moc.into_range_moc_iter(), or(i1, i2)).into_range_moc(),
      (Some(i1), None, _, _, _, _, _, _) =>
        return or(moc.into_range_moc_iter(), i1).into_range_moc(),
      (None, _, _, _, _, _, _, _) =>
        return moc,
    };
  }
}


pub fn kway8_or<'a, T, Q>(
  mut it: Box<dyn Iterator<Item=RangeMOC<T, Q>> + 'a>
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
{
  struct KWay8<'a, T1, Q1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
  {
    cur_moc: Option<RangeMOC<T1, Q1>>,
    it: Box<dyn Iterator<Item=RangeMOC<T1, Q1>> + 'a>
  }
  impl<'a, T1, Q1> Iterator for KWay8<'a, T1, Q1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
  {
    type Item = RangeMOC<T1, Q1>;
    fn next(&mut self) -> Option<Self::Item> {
      match (
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next())
      {
        (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), Some(i8)) =>
          self.cur_moc.replace((  ( i1.or(&i2) ).or( &i3.or(&i4) )  ).or(  &( i5.or(&i6) ).or( &i7.or(&i8) )  )),
        (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), None) =>
          self.cur_moc.replace((  ( i1.or(&i2) ).or( &i3.or(&i4) )  ).or(  &( i5.or(&i6) ).or( &i7 )  )),
        (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), None, _) =>
          self.cur_moc.replace((  ( i1.or(&i2) ).or( &i3.or(&i4) )  ).or(  &i5.or(&i6)  )),
        (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), None, _, _) =>
          self.cur_moc.replace((  ( i1.or(&i2) ).or( &i3.or(&i4) )  ).or(  &i5  )),
        (Some(i1), Some(i2), Some(i3), Some(i4), None, _, _, _) =>
          self.cur_moc.replace(( i1.or(&i2) ).or( &i3.or(&i4) )),
        (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
          self.cur_moc.replace(( i1.or(&i2) ).or( &i3 )),
        (Some(i1), Some(i2), None, _, _, _, _, _) =>
          self.cur_moc.replace(i1.or(&i2)),
        (Some(i1), None, _, _, _, _, _, _) =>
          self.cur_moc.replace(i1),
        (None, _, _, _, _, _, _, _) =>
          self.cur_moc.take()
      }
    }
  }
  match (
    it.next(),
    it.next(),
    it.next(),
    it.next(),
    it.next(),
    it.next(),
    it.next(),
    it.next())
  {
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), Some(i8)) => {
      let cur_moc = Some((  ( i1.or(&i2) ).or( &i3.or(&i4) )  ).or(  &( i5.or(&i6) ).or( &i7.or(&i8) )  ));
      kway8_or(Box::new(KWay8 { cur_moc, it }))
    },
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), None) =>
      (  ( i1.or(&i2) ).or( &i3.or(&i4) )  ).or(  &( i5.or(&i6) ).or( &i7 )  ),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), None, _) =>
      (  ( i1.or(&i2) ).or( &i3.or(&i4) )  ).or(  &i5.or(&i6)  ),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), None, _, _) =>
      (  ( i1.or(&i2) ).or( &i3.or(&i4) )  ).or(  &i5  ),
    (Some(i1), Some(i2), Some(i3), Some(i4), None, _, _, _) =>
      ( i1.or(&i2) ).or( &i3.or(&i4) ),
    (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
      ( i1.or(&i2) ).or( &i3 ),
    (Some(i1), Some(i2), None, _, _, _, _, _) =>
      i1.or(&i2),
    (Some(i1), None, _, _, _, _, _, _) =>
      i1,
    (None, _, _, _, _, _, _, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

pub fn kway4_or<'a, T, Q>(
  mut it: Box<dyn Iterator<Item=RangeMOC<T, Q>> + 'a>
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
{
  struct KWay4<'a, T1, Q1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
  {
    cur_moc: Option<RangeMOC<T1, Q1>>,
    it: Box<dyn Iterator<Item=RangeMOC<T1, Q1>> + 'a>
  }
  impl<'a, T1, Q1> Iterator for KWay4<'a, T1, Q1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
  {
    type Item = RangeMOC<T1, Q1>;
    fn next(&mut self) -> Option<Self::Item> {
      match (
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next())
      {
        (Some(i1), Some(i2), Some(i3), Some(i4)) =>
          self.cur_moc.replace(( i1.or(&i2) ).or( &i3.or(&i4) )),
        (Some(i1), Some(i2), Some(i3), None) =>
          self.cur_moc.replace(( i1.or(&i2) ).or( &i3 )),
        (Some(i1), Some(i2), None, _) =>
          self.cur_moc.replace(i1.or(&i2)),
        (Some(i1), None, _, _) =>
          self.cur_moc.replace(i1),
        (None, _, _, _) =>
          self.cur_moc.take()
      }
    }
  }
  match (
    it.next(),
    it.next(),
    it.next(),
    it.next())
  {
    (Some(i1), Some(i2), Some(i3), Some(i4)) => {
      let cur_moc = Some((i1.or(&i2)).or(&i3.or(&i4)));
      kway4_or(Box::new(KWay4 { cur_moc, it }))
    },
    (Some(i1), Some(i2), Some(i3), None) =>
      ( i1.or(&i2) ).or( &i3 ),
    (Some(i1), Some(i2), None, _) =>
      i1.or(&i2),
    (Some(i1), None, _, _) =>
      i1,
    (None, _, _, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

pub fn kway2_or<'a, T, Q>(
  mut it: Box<dyn Iterator<Item=RangeMOC<T, Q>> + 'a>
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
{
  struct KWay2<'a, T1, Q1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
  {
    cur_moc: Option<RangeMOC<T1, Q1>>,
    it: Box<dyn Iterator<Item=RangeMOC<T1, Q1>> + 'a>
  }
  impl<'a, T1, Q1> Iterator for KWay2<'a, T1, Q1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
  {
    type Item = RangeMOC<T1, Q1>;
    fn next(&mut self) -> Option<Self::Item> {
      match (
        self.it.next(),
        self.it.next())
      {
        (Some(i1), Some(i2)) =>
          self.cur_moc.replace(i1.or(&i2)),
        (Some(i1), None) =>
          self.cur_moc.replace(i1),
        (None, _) =>
          self.cur_moc.take()
      }
    }
  }
  match (
    it.next(),
    it.next())
  {
    (Some(i1), Some(i2)) => {
      let cur_moc = Some(i1.or(&i2));
      kway2_or(Box::new(KWay2 { cur_moc, it }))
    },
    (Some(i1), None) =>
      i1,
    (None, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}


pub fn kway8_or_it<'a, T, Q, I>(
  mut it: Box<dyn Iterator<Item=I> + 'a>
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
    I: RangeMOCIterator<T, Qty=Q>,
{
  struct KWay8<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
      I1: RangeMOCIterator<T1, Qty=Q1>,
  {
    cur_moc: Option<RangeMOC<T1, Q1>>,
    it: Box<dyn Iterator<Item=I1> + 'a>
  }
  impl<'a, T1, Q1, I1> Iterator for KWay8<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
      I1: RangeMOCIterator<T1, Qty=Q1>,
  {
    type Item = RangeMOC<T1, Q1>;
    fn next(&mut self) -> Option<Self::Item> {
      match (
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next())
      {
        (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), Some(i8)) =>
          self.cur_moc.replace((  ( i1.or(i2) ).or( i3.or(i4) )  ).or(  ( i5.or(i6) ).or( i7.or(i8) )  ).into_range_moc()),
        (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), None) =>
          self.cur_moc.replace((  ( i1.or(i2) ).or( i3.or(i4) )  ).or(  ( i5.or(i6) ).or( i7 )  ).into_range_moc()),
        (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), None, _) =>
          self.cur_moc.replace((  ( i1.or(i2) ).or( i3.or(i4) )  ).or(  i5.or(i6)  ).into_range_moc()),
        (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), None, _, _) =>
          self.cur_moc.replace((  ( i1.or(i2) ).or( i3.or(i4) )  ).or(  i5  ).into_range_moc()),
        (Some(i1), Some(i2), Some(i3), Some(i4), None, _, _, _) =>
          self.cur_moc.replace(( i1.or(i2) ).or( i3.or(i4) ).into_range_moc()),
        (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
          self.cur_moc.replace(( i1.or(i2) ).or(i3).into_range_moc()),
        (Some(i1), Some(i2), None, _, _, _, _, _) =>
          self.cur_moc.replace(i1.or(i2).into_range_moc()),
        (Some(i1), None, _, _, _, _, _, _) =>
          self.cur_moc.replace(i1.into_range_moc()),
        (None, _, _, _, _, _, _, _) =>
          self.cur_moc.take()
      }
    }
  }
  match (
    it.next(),
    it.next(),
    it.next(),
    it.next(),
    it.next(),
    it.next(),
    it.next(),
    it.next())
  {
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), Some(i8)) => {
      let cur_moc = Some((  ( i1.or(i2) ).or( i3.or(i4) )  ).or(  ( i5.or(i6) ).or( i7.or(i8) )  ).into_range_moc());
      kway8_or(Box::new(KWay8 { cur_moc, it }))
    },
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), None) =>
      (  ( i1.or(i2) ).or( i3.or(i4) )  ).or(  ( i5.or(i6) ).or( i7 )  ).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), None, _) =>
      (  ( i1.or(i2) ).or( i3.or(i4) )  ).or(  i5.or(i6)  ).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), None, _, _) =>
      (  ( i1.or(i2) ).or( i3.or(i4) )  ).or(  i5  ).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), Some(i4), None, _, _, _) =>
      ( i1.or(i2) ).or( i3.or(i4) ).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
      ( i1.or(i2) ).or( i3 ).into_range_moc(),
    (Some(i1), Some(i2), None, _, _, _, _, _) =>
      i1.or(i2).into_range_moc(),
    (Some(i1), None, _, _, _, _, _, _) =>
      i1.into_range_moc(),
    (None, _, _, _, _, _, _, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

pub fn kway4_or_it<'a, T, Q, I>(
  mut it: Box<dyn Iterator<Item=I> + 'a>
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
    I: RangeMOCIterator<T, Qty=Q>,
{
  struct KWay4<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
      I1: RangeMOCIterator<T1, Qty=Q1>,
  {
    cur_moc: Option<RangeMOC<T1, Q1>>,
    it: Box<dyn Iterator<Item=I1> + 'a>
  }
  impl<'a, T1, Q1, I1> Iterator for KWay4<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
      I1: RangeMOCIterator<T1, Qty=Q1>,
  {
    type Item = RangeMOC<T1, Q1>;
    fn next(&mut self) -> Option<Self::Item> {
      match (
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next())
      {
        (Some(i1), Some(i2), Some(i3), Some(i4)) =>
          self.cur_moc.replace(( i1.or(i2) ).or( i3.or(i4) ).into_range_moc()),
        (Some(i1), Some(i2), Some(i3), None) =>
          self.cur_moc.replace(( i1.or(i2) ).or(i3).into_range_moc()),
        (Some(i1), Some(i2), None, _) =>
          self.cur_moc.replace(i1.or(i2).into_range_moc()),
        (Some(i1), None, _, _) =>
          self.cur_moc.replace(i1.into_range_moc()),
        (None, _, _, _) =>
          self.cur_moc.take()
      }
    }
  }
  match (
    it.next(),
    it.next(),
    it.next(),
    it.next())
  {
    (Some(i1), Some(i2), Some(i3), Some(i4)) => {
      let cur_moc = Some((i1.or(i2)).or(i3.or(i4)).into_range_moc());
      kway4_or(Box::new(KWay4 { cur_moc, it }))
    },
    (Some(i1), Some(i2), Some(i3), None) =>
      ( i1.or(i2) ).or( i3 ).into_range_moc(),
    (Some(i1), Some(i2), None, _) =>
      i1.or(i2).into_range_moc(),
    (Some(i1), None, _, _) =>
      i1.into_range_moc(),
    (None, _, _, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

pub fn kway2_or_it<'a, T, Q, I>(
  mut it: Box<dyn Iterator<Item=I> + 'a>
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
    I: RangeMOCIterator<T, Qty=Q>,
{
  struct KWay2<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
      I1: RangeMOCIterator<T1, Qty=Q1>,
  {
    cur_moc: Option<RangeMOC<T1, Q1>>,
    it: Box<dyn Iterator<Item=I1> + 'a>
  }
  impl<'a, T1, Q1, I1> Iterator for KWay2<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
      I1: RangeMOCIterator<T1, Qty=Q1>,
  {
    type Item = RangeMOC<T1, Q1>;
    fn next(&mut self) -> Option<Self::Item> {
      match (
        self.it.next(),
        self.it.next(), )
      {
        (Some(i1), Some(i2)) =>
          self.cur_moc.replace(i1.or(i2).into_range_moc()),
        (Some(i1), None) =>
          self.cur_moc.replace(i1.into_range_moc()),
        (None, _) =>
          self.cur_moc.take()
      }
    }
  }
  match (
    it.next(),
    it.next())
  {
    (Some(i1), Some(i2)) => {
      let cur_moc = Some(i1.or(i2).into_range_moc());
      kway2_or(Box::new(KWay2 { cur_moc, it }))
    },
    (Some(i1), None) =>
      i1.into_range_moc(),
    (None, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

/*
pub fn kway4_or_it_v2<T, Q, I1, I2>(
  mut it_of_its: I2,
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: Iterator<Item=I1>,
{
  struct KWay4<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
      I1: Iterator<Item=Box<dyn RangeMOCIterator<T1, Qty=Q1> + 'a>>,
  {
    cur_moc: Option<RangeMOC<T1, Q1>>,
    it: Box<dyn Iterator<Item=RangeMOC<T1, Q1>> + 'a>
  }
  impl<'a, T1, Q1, I1> Iterator for KWay4<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
  {
    type Item = RangeMOC<T1, Q1>;
    fn next(&mut self) -> Option<Self::Item> {
      match (
        self.it.next(),
        self.it.next(),
        self.it.next(),
        self.it.next())
      {
        (Some(i1), Some(i2), Some(i3), Some(i4)) =>
          self.cur_moc.replace(( i1.or(&i2) ).or( &i3.or(&i4) )),
        (Some(i1), Some(i2), Some(i3), None) =>
          self.cur_moc.replace(( i1.or(&i2) ).or( &i3 )),
        (Some(i1), Some(i2), None, _) =>
          self.cur_moc.replace(i1.or(&i2)),
        (Some(i1), None, _, _) =>
          self.cur_moc.replace(i1),
        (None, _, _, _) =>
          self.cur_moc.take()
      }
    }
  }
  match (
    it.next(),
    it.next(),
    it.next(),
    it.next())
  {
    (Some(i1), Some(i2), Some(i3), Some(i4)) => {
      let cur_moc = Some(  ( i1.or(&i2) ).or( &i3.or(&i4) )  );
      kway4_or_it_boxdyn_v2(KWay4 { cur_moc, it })
    },
    (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
      ( i1.or(&i2) ).or( &i3 ),
    (Some(i1), Some(i2), None, _, _, _, _, _) =>
      i1.or(&i2),
    (Some(i1), None, _, _, _, _, _, _) =>
      i1,
    (None, _, _, _, _, _, _, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

pub fn kway2_or_it_v2<T, Q, I1, I2>(
  mut it_of_its: I2,
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: Iterator<Item=I1>,
{
  struct KWay2<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
      I1: Iterator<Item=Box<dyn RangeMOCIterator<T1, Qty=Q1> + 'a>>,
  {
    cur_moc: Option<RangeMOC<T1, Q1>>,
    it: Box<dyn Iterator<Item=RangeMOC<T1, Q1>> + 'a>
  }
  impl<'a, T1, Q1, I1> Iterator for KWay2<'a, T1, Q1, I1>
    where
      T1: Idx,
      Q1: MocQty<T1>,
  {
    type Item = RangeMOC<T1, Q1>;
    fn next(&mut self) -> Option<Self::Item> {
      match (self.it.next(), self.it.next())
      {
        (Some(i1), Some(i2)) =>
          self.cur_moc.replace(i1.or(&i2)),
        (Some(i1), None) =>
          self.cur_moc.replace(i1),
        (None, _) =>
          self.cur_moc.take()
      }
    }
  }
  match (
    it.next(),
    it.next())
  {
    (Some(i1), Some(i2)) => {
      let cur_moc = Some(i1.or(&i2));
      kway2_or_it_boxdyn_v2(KWay2 { cur_moc, it })
    },
    (Some(i1), None) =>
      i1,
    (None, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}


pub fn kway4_or_it<T, Q, I1, I2>(
  mut it_of_its: I2,
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: Iterator<Item=I1>,
{
  let mut moc = match (
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next(),
    it_of_its.next())
  {
    (Some(i1), Some(i2), Some(i3), Some(i4)) =>
      or(or(i1, i2), or(i3, i4)).into_range_moc(),
    (Some(i1), Some(i2), Some(i3), None) =>
      return or(or(i1, i2), i3).into_range_moc(),
    (Some(i1), Some(i2), None, _) =>
      return or(i1, i2).into_range_moc(),
    (Some(i1), None, _, _) =>
      return i1.into_range_moc(),
    (None, _, _, _) =>
      return RangeMOC::new(0, MocRanges::default()),
  };
  loop {
    moc = match (
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next(),
      it_of_its.next())
    {
      (Some(i1), Some(i2), Some(i3), Some(i4)) =>
        moc.or(&or(or(i1, i2), or(i3, i4)).into_range_moc()),
      (Some(i1), Some(i2), Some(i3), None) =>
        return moc.or(&or(or(i1, i2), i3).into_range_moc()),
      (Some(i1), Some(i2), None, _) =>
        return moc.or(&or(i1, i2).into_range_moc()),
      (Some(i1), None, _, _) =>
        return moc.or(&i1.into_range_moc()),
      (None, _, _, _) =>
        return moc,
    };
  }
}

pub fn kway2_or_it<T, Q, I1, I2>(
  mut it_of_its: I2,
) -> RangeMOC<T, Q>
  where
    T: Idx,
    Q: MocQty<T>,
    I1: RangeMOCIterator<T, Qty=Q>,
    I2: Iterator<Item=I1>,
{
  let mut moc = match (
    it_of_its.next(),
    it_of_its.next())
  {
    (Some(i1), Some(i2)) =>
      or(i1, i2).into_range_moc(),
    (Some(i1), None) =>
      return i1.into_range_moc(),
    (None, _) =>
      return RangeMOC::new(0, MocRanges::default()),
  };
  loop {
    moc = match (
      it_of_its.next(),
      it_of_its.next())
    {
      (Some(i1), Some(i2)) =>
        moc.or(&or(i1, i2).into_range_moc()),
      (Some(i1), None) =>
        return moc.or(&i1.into_range_moc()),
      (None, _) =>
        return moc,
    };
  }
}
*/
/// polygon_list.txt

fn create_cones_mocs() -> Vec<RangeMOC<u64, Hpx<u64>>> {
  let filename = "xmmdr11_obs_center.17arcmin.csv";
  let path_buf = PathBuf::from(format!("resources/{}", filename));
  //let path_buf = PathBuf::from(format!("../resources/{}", filename));
  // let file = File::open(&path_buf1).or_else(|_| File::open(&path_buf2)).unwrap();
  fs::read_to_string(path_buf)
    .unwrap()
    .lines()
    .map(|line| {
      let fields: Vec<&str> = line.split(',').collect();
      let lon_deg = fields[0].parse::<f64>().unwrap();
      let lat_deg = fields[1].parse::<f64>().unwrap();
      let radius = fields[2].parse::<f64>().unwrap();
      let lon = lon_deg.to_radians();
      let lat = lat_deg.to_radians();
      let radius = radius.to_radians();
      RangeMOC::from_cone(lon, lat, radius, 12, 2)
    }).collect()
}

fn bench_multi_or(c: &mut Criterion) {
  // https://bheisler.github.io/criterion.rs/book/user_guide/comparing_functions.html
  let mut group = c.benchmark_group("multi_or");
  let mocs = create_cones_mocs();
  group.sample_size(10);
  group.bench_function("multi_or_naive",
      |b| b.iter(|| multi_or_naive(Box::new(mocs.iter().cloned())))
  );
  group.bench_function("multi_or_it",
      |b| b.iter(|| multi_or_it(mocs.iter().map(|moc| moc.into_range_moc_iter())))
   );
  group.bench_function("multi_or_it_boxdyn",
      |b| b.iter(|| multi_or_it_boxdyn(Box::new(mocs.iter().map(|moc| moc.into_range_moc_iter()))))
  );
  group.bench_function("kway8_or",
      |b| b.iter(|| kway8_or(Box::new(mocs.iter().cloned())))
  );
  group.bench_function("kway4_or",
                       |b| b.iter(|| kway4_or(Box::new(mocs.iter().cloned())))
  );
  group.bench_function("kway2_or",
                       |b| b.iter(|| kway2_or(Box::new(mocs.iter().cloned())))
  );
  group.bench_function("kway2_or_it",
                       |b| b.iter(|| kway2_or_it(Box::new(mocs.iter().map(|moc| moc.into_range_moc_iter()))))
  );
  group.bench_function("kway4_or_it",
                       |b| b.iter(|| kway4_or_it(Box::new(mocs.iter().map(|moc| moc.into_range_moc_iter()))))
  );
  group.bench_function("kway8_or_it",
                       |b| b.iter(|| kway8_or_it(Box::new(mocs.iter().map(|moc| moc.into_range_moc_iter()))))
  );
  /*
  group.bench_function("kway4_or_it_boxdyn_v2",
                       |b| b.iter(|| kway4_or_it_boxdyn_v2(Box::new(mocs.iter().map(|moc| moc.into_range_moc_iter()))))
  );
  group.bench_function("kway2_or_it_boxdyn_v2",
                       |b| b.iter(|| kway2_or_it_boxdyn_v2(Box::new(mocs.iter().map(|moc| moc.into_range_moc_iter()))))
  );*/
  group.finish();
}

criterion_group!(benches, bench_multi_or);
criterion_main!(benches);