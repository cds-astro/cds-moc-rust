//! Module containing operations between 2 MOCs generating a MOC.

use crate::{
  qty::{Hpx, Time},
  elemset::range::MocRanges,
  moc::range::RangeMOC,
  hpxranges2d::TimeSpaceMoc,
  moc2d::{
    HasTwoMaxDepth, RangeMOC2IntoIterator,
    range::RangeMOC2
  }
};

use super::{
  store,
  common::{SMOC, TMOC, FMOC, STMOC, InternalMoc}
};

#[derive(Copy, Clone)]
pub(crate) enum Op2 {
  Intersection,
  Union,
  Difference,
  Minus,
  TFold,
  SFold,
}

impl Op2 {
  
  fn perform_op_on_smoc(self, left: &SMOC, right: &SMOC) -> Result<SMOC, String> {
    match self {
      Op2::Intersection => Ok(left.and(right)),
      Op2::Union => Ok(left.or(right)),
      Op2::Difference => Ok(left.xor(right)),
      Op2::Minus => Ok(left.minus(right)),
      Op2::TFold => Err(String::from("TimeFold operation not available on 2 S-MOCs.")),
      Op2::SFold => Err(String::from("SpaceFold operation not available on 2 S-MOCs.")),
    }
  }
  
  fn perform_op_on_tmoc(self, left: &TMOC, right: &TMOC) -> Result<TMOC, String> {
    match self {
      Op2::Intersection => Ok(left.and(right)),
      Op2::Union => Ok(left.or(right)),
      Op2::Difference => Ok(left.xor(right)),
      Op2::Minus => Ok(left.minus(right)),
      Op2::TFold => Err(String::from("TimeFold operation not available on 2 T-MOCs.")),
      Op2::SFold => Err(String::from("SpaceFold operation not available on 2 T-MOCs.")),
    }
  }

  fn perform_op_on_fmoc(self, left: &FMOC, right: &FMOC) -> Result<FMOC, String> {
    match self {
      Op2::Intersection => Ok(left.and(right)),
      Op2::Union => Ok(left.or(right)),
      Op2::Difference => Ok(left.xor(right)),
      Op2::Minus => Ok(left.minus(right)),
      Op2::TFold => Err(String::from("TimeFold operation not available on 2 F-MOCs.")),
      Op2::SFold => Err(String::from("SpaceFold operation not available on 2 F-MOCs.")),
    }
  }
  
  fn perform_op_on_stmoc(self, left: &STMOC, right: &STMOC) -> Result<STMOC, String> {
    let (time_depth_l, hpx_depth_l) = (left.depth_max_1(), left.depth_max_2());
    let (time_depth_r, hpx_depth_r) = (right.depth_max_1(), right.depth_max_2());
    // Here we loose time by performing a conversion!! (TODO implement operations on RangeMOC2!)
    let left = TimeSpaceMoc::from_ranges_it_gen(left.into_range_moc2_iter());
    let right = TimeSpaceMoc::from_ranges_it_gen(right.into_range_moc2_iter());
    let result = match self {
      Op2::Intersection => left.intersection(&right),
      Op2::Union => left.union(&right),
      Op2::Difference => left.difference(&right),
      Op2::Minus => return Err(String::from("Minus not implemented yet for ST-MOCs.")),
      Op2::TFold => return Err(String::from("TimeFold operation not available on 2 ST-MOCs.")),
      Op2::SFold => return Err(String::from("SpaceFold operation not available on 2 ST-MOCs.")),
    };
    let time_depth = time_depth_l.max(time_depth_r);
    let space_depth = hpx_depth_l.max(hpx_depth_r);
    Ok(RangeMOC2::new(time_depth, space_depth, result.time_space_iter(time_depth, space_depth).collect()))
  }
  
  fn perform_space_fold(self, left: &SMOC, right: &STMOC) -> Result<TMOC, String> {
    if !matches!(self, Op2::SFold) {
      Err(String::from("Operation SpaceFold expected on S-MOC vs ST-MOC."))
    } else {
      let time_depth = right.depth_max_1();
      // Here we loose time by performing a conversion!! (TODO implement operations on RangeMOC2!)
      let stmoc = TimeSpaceMoc::from_ranges_it_gen(right.into_range_moc2_iter());
      let tranges: MocRanges<u64, Time<u64>> = TimeSpaceMoc::project_on_first_dim(left.moc_ranges(), &stmoc);
      Ok(RangeMOC::new(time_depth, tranges))
    }
  }
  
  fn perform_time_fold(self, left: &TMOC, right: &STMOC) -> Result<SMOC, String> {
    if !matches!(self, Op2::TFold) {
      Err(String::from("Operation TimeFold expected on T-MOC vs ST-MOC."))
    } else {
      let hpx_depth = right.depth_max_2();
      // Here we loose time by performing a conversion!! (TODO implement operations on RangeMOC2!)
      let stmoc = TimeSpaceMoc::from_ranges_it_gen(right.into_range_moc2_iter());
      let sranges: MocRanges<u64, Hpx<u64>> = TimeSpaceMoc::project_on_second_dim(left.moc_ranges(), &stmoc);
      Ok(RangeMOC::new(hpx_depth, sranges))
    }
  }

  /// Performs the given operation on the given MOCs and store the resulting MOC in the store.
  pub(crate) fn exec(&self, left_index: usize, right_index: usize) -> Result<usize, String> {
    store::op2(
      left_index,
      right_index,
      move |left, right| match (left, right) {
        (InternalMoc::Space(l), InternalMoc::Space(r)) => self.perform_op_on_smoc(l, r).map(InternalMoc::Space),
        (InternalMoc::Time(l), InternalMoc::Time(r)) => self.perform_op_on_tmoc(l, r).map(InternalMoc::Time),
        (InternalMoc::Frequency(l), InternalMoc::Frequency(r)) => self.perform_op_on_fmoc(l, r).map(InternalMoc::Frequency),
        (InternalMoc::TimeSpace(l), InternalMoc::TimeSpace(r)) => self.perform_op_on_stmoc(l, r).map(InternalMoc::TimeSpace),
        (InternalMoc::Space(l), InternalMoc::TimeSpace(r)) => self.perform_space_fold(l, r).map(InternalMoc::Time),
        (InternalMoc::Time(l), InternalMoc::TimeSpace(r)) => self.perform_time_fold(l, r).map(InternalMoc::Space),
        _ => Err(String::from("Both type of both MOCs must be the same, except in fold operations")),
      },
    )
  }
  
}

