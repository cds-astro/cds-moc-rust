
use crate::idx::Idx;
use crate::qty::MocQty;
use crate::moc::{
  RangeMOC, RangeMOCIterator, or,
  range::MocRanges,
};

/// Performs a logical `OR` between the input iterators of ranges.
/// The operation is made by chunks of 8 iterators.
/// # Warning
/// This is more suited for a few large MOCs that a lot of very small MOCs
pub fn kway_or<T, Q, I1, I2>(
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
        moc.or(&or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), or(i7, i8))).into_range_moc()),
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), Some(i7), None) =>
        return moc.or(&or(or(or(i1, i2), or(i3, i4)), or(or(i5, i6), i7)).into_range_moc()),
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), Some(i6), None, _) =>
        return moc.or(&or(or(or(i1, i2), or(i3, i4)), or(i5, i6)).into_range_moc()),
      (Some(i1), Some(i2), Some(i3), Some(i4), Some(i5), None, _, _) =>
        return moc.or(&or(or(or(i1, i2), or(i3, i4)), i5).into_range_moc()),
      (Some(i1), Some(i2), Some(i3), Some(i4), None, _, _, _) =>
        return moc.or(&or(or(i1, i2), or(i3, i4)).into_range_moc()),
      (Some(i1), Some(i2), Some(i3), None, _, _, _, _) =>
        return moc.or(&or(or(i1, i2), i3).into_range_moc()),
      (Some(i1), Some(i2), None, _, _, _, _, _) =>
        return moc.or(&or(i1, i2).into_range_moc()),
      (Some(i1), None, _, _, _, _, _, _) =>
        return moc.or(&i1.into_range_moc()),
      (None, _, _, _, _, _, _, _) =>
        return moc,
    };
  }
}