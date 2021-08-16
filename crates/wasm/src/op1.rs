
use wasm_bindgen::JsValue;

use super::store;
use super::common::{SMOC, TMOC, STMOC, InternalMoc};

#[derive(Copy, Clone)]
pub(crate) enum Op1 {
  Complement,
  Degrade { new_depth: u8 },
  Extend,
  Contract,
  ExtBorder,
  IntBorder,
}
impl Op1 {
  fn perform_op_on_smoc(self, moc: &SMOC) -> Result<SMOC, String> {
    Ok(
      match self {
        Op1::Complement => moc.not(),
        Op1::Degrade { new_depth } => moc.degraded(new_depth),
        Op1::Extend => moc.expanded(),
        Op1::Contract => moc.contracted(),
        Op1::ExtBorder => moc.external_border(),
        Op1::IntBorder => moc.internal_border(),
      }
    )
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
  fn perform_op_on_stmoc(self, _moc: &STMOC) -> Result<STMOC, String> {
    match self {
      Op1::Complement => Err(String::from("Complement not implemented (yet) for ST-MOCs.")),
      Op1::Degrade { new_depth: _ } => Err(String::from("Degrade not implemented (yet) for ST-MOCs.")),
      Op1::Extend =>  Err(String::from("Extend border not implemented (yet) for ST-MOCs.")),
      Op1::Contract =>  Err(String::from("Contract border not implemented (yet) for ST-MOCs.")),
      Op1::ExtBorder =>  Err(String::from("External border not implemented (yet) for ST-MOCs.")),
      Op1::IntBorder => Err(String::from("Internal border not implemented (yet) for ST-MOCs.")),
    }
  }
}

/// Performs the given operation on the given MOC and store the resulting MOC in the store.
pub(crate) fn op1(name: &str, op: Op1, res_name: &str) -> Result<(), JsValue> {
  store::op1(
    name,
    move |moc| match moc {
      InternalMoc::Space(m) => op.perform_op_on_smoc(m).map(InternalMoc::Space),
      InternalMoc::Time(m) => op.perform_op_on_tmoc(m).map(InternalMoc::Time),
      InternalMoc::TimeSpace(m) => op.perform_op_on_stmoc(m).map(InternalMoc::TimeSpace),
    },
    res_name
  )
}
