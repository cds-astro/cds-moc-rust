
use std::error::Error;
use std::path::PathBuf;

use clap::Parser;

use crate::{StatusFlag, MocSetFileWriter};

#[derive(Debug, Parser)]
/// Change the status flag of the given MOCs identifiers (valid, deprecated, removed)
pub struct ChangeStatus {
  #[clap(parse(from_os_str))]
  /// The moc-set to be updated.
  file: PathBuf,
  /// Identifier of the MOC we want to modify the flag.
  id: u64,
  /// New status flag to be set.
  new_status: StatusFlag,
}

impl ChangeStatus {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let mut moc_set = MocSetFileWriter::new(self.file)?;
    moc_set.chg_status(self.id, self.new_status)?;
    moc_set.release()?;
    Ok(())
  }
}
