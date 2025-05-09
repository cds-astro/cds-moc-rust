//! Module containing operations between n MOCs generating a MOC.

use crate::moc::{
  range::op::multi_op::{kway_and_it, kway_or_it, kway_xor_it},
  RangeMOCIntoIterator,
};

use super::{
  common::{InternalMoc, FMOC, SMOC, TMOC},
  store,
};

#[derive(Copy, Clone)]
pub(crate) enum OpN {
  Intersection,
  Union,
  SymmetricDifference,
}

impl OpN {
  fn perform_op_on_smoc(self, mocs: Vec<&SMOC>) -> Result<SMOC, String> {
    let it = Box::new(mocs.iter().map(|moc_ref| moc_ref.into_range_moc_iter()));
    match self {
      OpN::Intersection => Ok(kway_and_it(it)),
      OpN::Union => Ok(kway_or_it(it)),
      OpN::SymmetricDifference => Ok(kway_xor_it(it)),
    }
  }

  fn perform_op_on_tmoc(self, mocs: Vec<&TMOC>) -> Result<TMOC, String> {
    let it = Box::new(mocs.iter().map(|moc_ref| moc_ref.into_range_moc_iter()));
    match self {
      OpN::Intersection => Ok(kway_and_it(it)),
      OpN::Union => Ok(kway_or_it(it)),
      OpN::SymmetricDifference => Ok(kway_xor_it(it)),
    }
  }

  fn perform_op_on_fmoc(self, mocs: Vec<&FMOC>) -> Result<FMOC, String> {
    let it = Box::new(mocs.iter().map(|moc_ref| moc_ref.into_range_moc_iter()));
    match self {
      OpN::Intersection => Ok(kway_and_it(it)),
      OpN::Union => Ok(kway_or_it(it)),
      OpN::SymmetricDifference => Ok(kway_xor_it(it)),
    }
  }

  /// Performs the given operation on the given MOCs and store the resulting MOC in the store.
  pub(crate) fn exec(&self, indices: &[usize]) -> Result<usize, String> {
    store::opn(indices, move |mocs| match mocs.first() {
      Some(InternalMoc::Space(_)) => {
        let mocs: Vec<&SMOC> = mocs
          .iter()
          .map(|moc| {
            if let InternalMoc::Space(moc) = moc {
              Ok(moc)
            } else {
              Err(String::from(
                "Multi operations must apply on a same MOC type",
              ))
            }
          })
          .collect::<Result<_, _>>()?;
        self.perform_op_on_smoc(mocs).map(InternalMoc::Space)
      }
      Some(InternalMoc::Time(_)) => {
        let mocs: Vec<&TMOC> = mocs
          .iter()
          .map(|moc| {
            if let InternalMoc::Time(moc) = moc {
              Ok(moc)
            } else {
              Err(String::from(
                "Multi operations must apply on a same MOC type",
              ))
            }
          })
          .collect::<Result<_, _>>()?;
        self.perform_op_on_tmoc(mocs).map(InternalMoc::Time)
      }
      Some(InternalMoc::Frequency(_)) => {
        let mocs: Vec<&FMOC> = mocs
          .iter()
          .map(|moc| {
            if let InternalMoc::Frequency(moc) = moc {
              Ok(moc)
            } else {
              Err(String::from(
                "Multi operations must apply on a same MOC type",
              ))
            }
          })
          .collect::<Result<_, _>>()?;
        self.perform_op_on_fmoc(mocs).map(InternalMoc::Frequency)
      }
      Some(InternalMoc::TimeSpace(_)) => Err(String::from("No opN operations for ST-MOCs")),
      Some(InternalMoc::FreqSpace(_)) => Err(String::from("No opN operations for SF-MOCs")),
      None => Err(String::from("Empty MOC list")),
    })
  }
}
