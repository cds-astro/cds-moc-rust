use crate::moc::{RangeMOCIterator, CellMOCIterator, CellMOCIntoIterator};

use super::{
  store,
  common::{SMOC, TMOC, FMOC, STMOC, InternalMoc},
};

// Other operations {
//   View,
//   Coverage
//   ...,
//  AutoDetectCenterAndRadius
// } (No point in maing a enum since result types are different for each operation

#[derive(Copy, Clone)]
pub(crate) enum Op1 {
  Complement,
  Degrade { new_depth: u8 },
  Extend,
  Contract,
  ExtBorder,
  IntBorder,
  // Fill holes
}

impl Op1 {
  fn perform_op_on_smoc(self, moc: &SMOC) -> Result<SMOC, String> {
    match self {
      Op1::Complement => Ok(moc.not()),
      Op1::Degrade { new_depth } => Ok(moc.degraded(new_depth)),
      Op1::Extend => Ok(moc.expanded()),
      Op1::Contract => Ok(moc.contracted()),
      Op1::ExtBorder => Ok(moc.external_border()),
      Op1::IntBorder => Ok(moc.internal_border()),
    }
  }
  fn perform_op_on_tmoc(self, moc: &TMOC) -> Result<TMOC, String> {
    match self {
      Op1::Complement => Ok(moc.not()),
      Op1::Degrade { new_depth } => Ok(moc.degraded(new_depth)),
      Op1::Extend => Err(String::from("Extend border not implemented (yet) for T-MOCs.")),
      Op1::Contract => Err(String::from("Contract border not implemented (yet) for T-MOCs.")),
      Op1::ExtBorder => Err(String::from("External border not implemented (yet) for T-MOCs.")),
      Op1::IntBorder => Err(String::from("Internal border not implemented (yet) for T-MOCs.")),
    }
  }
  fn perform_op_on_fmoc(self, moc: &FMOC) -> Result<FMOC, String> {
    match self {
      Op1::Complement => Ok(moc.not()),
      Op1::Degrade { new_depth } => Ok(moc.degraded(new_depth)),
      Op1::Extend => Err(String::from("Extend border not implemented (yet) for F-MOCs.")),
      Op1::Contract => Err(String::from("Contract border not implemented (yet) for F-MOCs.")),
      Op1::ExtBorder => Err(String::from("External border not implemented (yet) for F-MOCs.")),
      Op1::IntBorder => Err(String::from("Internal border not implemented (yet) for F-MOCs.")),
    }
  }
  fn perform_op_on_stmoc(self, _moc: &STMOC) -> Result<STMOC, String> {
    match self {
      Op1::Complement => Err(String::from("Complement not implemented (yet) for ST-MOCs.")),
      Op1::Degrade { new_depth: _ } => Err(String::from("Degrade not implemented (yet) for ST-MOCs.")),
      Op1::Extend => Err(String::from("Extend border not implemented (yet) for ST-MOCs.")),
      Op1::Contract => Err(String::from("Contract border not implemented (yet) for ST-MOCs.")),
      Op1::ExtBorder => Err(String::from("External border not implemented (yet) for ST-MOCs.")),
      Op1::IntBorder => Err(String::from("Internal border not implemented (yet) for ST-MOCs.")),
    }
  }

  /// Performs the given operation on the given MOC and store the resulting MOC in the store, 
  /// returning its index.
  pub(crate) fn exec(&self, index: usize) -> Result<usize, String> {
    store::op1(
      index,
      move |moc| match moc {
        InternalMoc::Space(m) => self.perform_op_on_smoc(m).map(InternalMoc::Space),
        InternalMoc::Time(m) => self.perform_op_on_tmoc(m).map(InternalMoc::Time),
        InternalMoc::Frequency(m) => self.perform_op_on_fmoc(m).map(InternalMoc::Frequency),
        InternalMoc::TimeSpace(m) => self.perform_op_on_stmoc(m).map(InternalMoc::TimeSpace),
      },
    )
  }
}

#[derive(Copy, Clone)]
pub(crate) enum Op1MultiRes {
  Split,
  SplitIndirect,
}

impl Op1MultiRes {

  fn perform_op_on_smoc(self, moc: &SMOC) -> Result<Vec<InternalMoc>, String> {
    Ok(match self {
      Op1MultiRes::Split => moc.split_into_joint_mocs(false),
      Op1MultiRes::SplitIndirect =>  moc.split_into_joint_mocs(true),
    }.drain(..)
     .map(|cell_moc| cell_moc.into_cell_moc_iter().ranges().into_range_moc().into())
     .collect()
    )
  }
  fn perform_op_on_tmoc(self, _moc: &TMOC) -> Result<Vec<InternalMoc>, String> {
    Err(String::from("Split not implemented for T-MOCs."))
  }
  fn perform_op_on_fmoc(self, _moc: &FMOC) -> Result<Vec<InternalMoc>, String> {
    Err(String::from("Split not implemented for F-MOCs."))
  }
  fn perform_op_on_stmoc(self, _moc: &STMOC) -> Result<Vec<InternalMoc>, String> {
    Err(String::from("Split not implemented for ST-MOCs."))
  }

  /// Performs the given operation on the given MOC and store the resulting MOC in the store, 
  /// returning its index.
  pub(crate) fn exec(&self, index: usize) -> Result<Vec<usize>, String> {
    store::op1_multi_res(
      index,
      move |moc| match moc {
        InternalMoc::Space(m) => self.perform_op_on_smoc(m),
        InternalMoc::Time(m) => self.perform_op_on_tmoc(m),
        InternalMoc::Frequency(m) => self.perform_op_on_fmoc(m),
        InternalMoc::TimeSpace(m) => self.perform_op_on_stmoc(m),
      },
    )
  }
}

pub(crate) fn op1_count_split(index: usize, indirect_neigh: bool) -> Result<u32, String> {
  store::exec_on_one_readonly_moc(
    index,
    move |moc| match moc {
      InternalMoc::Space(m) => Ok(m.split_into_joint_mocs(indirect_neigh).len() as u32),
      InternalMoc::Time(_) => Err(String::from("Split not implemented for T-MOCs.")),
      InternalMoc::Frequency(_) => Err(String::from("Split not implemented for F-MOCs.")),
      InternalMoc::TimeSpace(_) => Err(String::from("Split not implemented for ST-MOCs.")),
    },
  )
}

pub(crate) fn op1_stmoc_tmin(index: usize) -> Result<Option<u64>, String> {
  store::exec_on_one_readonly_moc(
    index,
    move |moc| match moc {
      InternalMoc::Space(_) => Err(String::from("Tmin not implemented for S-MOCs.")),
      InternalMoc::Time(_) => Err(String::from("Tmin not implemented for T-MOCs.")),
      InternalMoc::Frequency(_) =>  Err(String::from("Tmin not implemented for F-MOCs.")),
      InternalMoc::TimeSpace(stmoc) => Ok(stmoc.min_index_left()),
    },
  )
}

pub(crate) fn op1_stmoc_tmax(index: usize) -> Result<Option<u64>, String> {
  store::exec_on_one_readonly_moc(
    index,
    move |moc| match moc {
      InternalMoc::Space(_) => Err(String::from("Tmin not implemented for S-MOCs.")),
      InternalMoc::Time(_) => Err(String::from("Tmin not implemented for T-MOCs.")),
      InternalMoc::Frequency(_) =>  Err(String::from("Tmin not implemented for F-MOCs.")),
      InternalMoc::TimeSpace(stmoc) => Ok(stmoc.max_index_left()),
    },
  )
}
