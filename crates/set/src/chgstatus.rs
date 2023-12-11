use std::{error::Error, path::PathBuf};

use clap::Parser;

use crate::{MocSetFileWriter, StatusFlag};

#[derive(Debug, Parser)]
/// Change the status flag of the given MOCs identifiers (valid, deprecated, removed)
pub struct ChangeStatus {
  #[clap(value_name = "FILE")]
  /// The moc-set to be updated.
  file: PathBuf,
  /// New status flag to be set.
  new_status: StatusFlag,
  #[clap(value_delimiter = ',')]
  /// Comma separated list of identifiers of the MOCs we want to modify the flag.
  ids: Vec<u64>,
}

impl ChangeStatus {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let mut moc_set = MocSetFileWriter::new(self.file)?;
    // moc_set.chg_status(self.id, self.new_status)?;
    let id_status_map = self.ids.iter().map(|id| (*id, self.new_status)).collect();
    let res = moc_set
      .chg_multi_status(id_status_map)
      .map_err(|e| e.into());
    moc_set.release()?;
    res
  }
}

//  chg_multi_status(&mut self, mut target: HashMap<u64, StatusFlag>)
