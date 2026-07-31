use std::{
  error::Error,
  io::{Write, stdout},
  path::PathBuf,
};

use clap::Parser;

use crate::rtree::{RTreeLayout, STMocPPFile};

#[derive(Debug, Parser)]
/// Get information on a ST-MOC PP file.
pub struct Info {
  #[clap(value_name = "FILE")]
  /// The stmoc-pp file.
  file: PathBuf,
}

impl Info {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let stmpp = STMocPPFile::new(self.file)?;
    let mut writer = stdout().lock();
    writeln!(&mut writer, "ST-MOC PP")?;
    writeln!(&mut writer, "* Version: {}", stmpp.version())?;
    writeln!(&mut writer, "* Page size (kB): {}", stmpp.page_size_kb())?;
    writeln!(
      &mut writer,
      "* SMOC maximum depth: {}",
      stmpp.smoc_depth_max()
    )?;
    writeln!(&mut writer, "* Number of entries: {}", stmpp.n_entries())?;
    writeln!(&mut writer, "* File size (B): {}", stmpp.file_size())?;
    let layout = stmpp.layout();
    writeln!(&mut writer, "* Layout:")?;
    let n_node_entries_per_page = RTreeLayout::n_node_entries_per_page(stmpp.page_size_kb());
    let n_leaf_entries_per_page = RTreeLayout::n_leaf_entries_per_page(stmpp.page_size_kb());
    writeln!(
      &mut writer,
      "  + node entries/page: {}",
      n_node_entries_per_page
    )?;
    writeln!(
      &mut writer,
      "  + leaf entries/page: {}",
      n_leaf_entries_per_page
    )?;
    let layers = layout.layers();
    let n = layers.len();
    for (i, layer) in layers.iter().enumerate() {
      let n_entries = if i == n - 1 {
        layer.n_entries(n_leaf_entries_per_page)
      } else {
        layer.n_entries(n_node_entries_per_page)
      };
      writeln!(&mut writer, "  + layer {}: {} entries", i, n_entries)?;
      writeln!(&mut writer, "    - number of pages: {}", layer.n_pages)?;
      writeln!(
        &mut writer,
        "    - number of element in rightmost page: {}",
        layer.n_elems_in_rightmost_page
      )?;
    }
    Ok(())
  }
}
