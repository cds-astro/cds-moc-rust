
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


// TODO: replace the ugly copy/paste/modify by a macro taking the operator as a parameter

pub fn kway_and<'a, T, Q>(
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
          self.cur_moc.replace(( i1.and(&i2) ).and( &i3.and(&i4) )),
        (Some(i1), Some(i2), Some(i3), None) =>
          self.cur_moc.replace(( i1.and(&i2) ).and( &i3 )),
        (Some(i1), Some(i2), None, _) =>
          self.cur_moc.replace(i1.and(&i2)),
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
      let cur_moc = Some((i1.and(&i2)).and(&i3.and(&i4)));
      kway_and(Box::new(KWay4 { cur_moc, it }))
    },
    (Some(i1), Some(i2), Some(i3), None) =>
      ( i1.and(&i2) ).and( &i3 ),
    (Some(i1), Some(i2), None, _) =>
      i1.and(&i2),
    (Some(i1), None, _, _) =>
      i1,
    (None, _, _, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

pub fn kway_and_it<'a, T, Q, I>(
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
          self.cur_moc.replace(( i1.and(i2) ).and( i3.and(i4) ).into_range_moc()),
        (Some(i1), Some(i2), Some(i3), None) =>
          self.cur_moc.replace(( i1.and(i2) ).and(i3).into_range_moc()),
        (Some(i1), Some(i2), None, _) =>
          self.cur_moc.replace(i1.and(i2).into_range_moc()),
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
      let cur_moc = Some((i1.and(i2)).and(i3.and(i4)).into_range_moc());
      kway_and(Box::new(KWay4 { cur_moc, it }))
    },
    (Some(i1), Some(i2), Some(i3), None) =>
      ( i1.and(i2) ).and( i3 ).into_range_moc(),
    (Some(i1), Some(i2), None, _) =>
      i1.and(i2).into_range_moc(),
    (Some(i1), None, _, _) =>
      i1.into_range_moc(),
    (None, _, _, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

pub fn kway_xor<'a, T, Q>(
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
          self.cur_moc.replace(( i1.xor(&i2) ).xor( &i3.xor(&i4) )),
        (Some(i1), Some(i2), Some(i3), None) =>
          self.cur_moc.replace(( i1.xor(&i2) ).xor( &i3 )),
        (Some(i1), Some(i2), None, _) =>
          self.cur_moc.replace(i1.xor(&i2)),
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
      let cur_moc = Some((i1.xor(&i2)).xor(&i3.xor(&i4)));
      kway_xor(Box::new(KWay4 { cur_moc, it }))
    },
    (Some(i1), Some(i2), Some(i3), None) =>
      ( i1.xor(&i2) ).xor( &i3 ),
    (Some(i1), Some(i2), None, _) =>
      i1.xor(&i2),
    (Some(i1), None, _, _) =>
      i1,
    (None, _, _, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

pub fn kway_xor_it<'a, T, Q, I>(
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
          self.cur_moc.replace(( i1.xor(i2) ).xor( i3.xor(i4) ).into_range_moc()),
        (Some(i1), Some(i2), Some(i3), None) =>
          self.cur_moc.replace(( i1.xor(i2) ).xor(i3).into_range_moc()),
        (Some(i1), Some(i2), None, _) =>
          self.cur_moc.replace(i1.xor(i2).into_range_moc()),
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
      let cur_moc = Some((i1.xor(i2)).xor(i3.xor(i4)).into_range_moc());
      kway_xor(Box::new(KWay4 { cur_moc, it }))
    },
    (Some(i1), Some(i2), Some(i3), None) =>
      ( i1.xor(i2) ).xor( i3 ).into_range_moc(),
    (Some(i1), Some(i2), None, _) =>
      i1.xor(i2).into_range_moc(),
    (Some(i1), None, _, _) =>
      i1.into_range_moc(),
    (None, _, _, _) =>
      RangeMOC::new(0, MocRanges::default())
  }
}

