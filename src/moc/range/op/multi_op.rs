
use crate::idx::Idx;
use crate::qty::MocQty;
use crate::moc::{RangeMOC, RangeMOCIterator, range::MocRanges};

pub fn kway_or<'a, T, Q>(
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
      kway_or(Box::new(KWay4 { cur_moc, it }))
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

pub fn kway_or_it<'a, T, Q, I>(
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
      kway_or(Box::new(KWay4 { cur_moc, it }))
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

/*
pub fn kway_or<'a, T, Q>(
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
      kway_or(Box::new(KWay8 { cur_moc, it }))
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

/// Performs a logical `OR` between the input iterators of ranges.
/// The operation is made by chunks of 8 iterators.
/// # Warning
/// This is more suited for a few large MOCs that a lot of very small MOCs
/// # Info
/// Nor really a kway-merge. MOCs are merged 8 by 8 but then each result is 
/// iterativly merge with the previous result (instread of merging by 8 then by 8, ...)
pub fn kway_or_it<T, Q, I1, I2>(
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
