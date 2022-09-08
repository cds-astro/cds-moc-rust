
use std::mem::size_of;
use std::error::Error;
use std::path::PathBuf;

use clap::Parser;

use moclib::qty::{MocQty, Hpx};

use crate::MocSetFileReader;

#[derive(Debug, Parser)]
/// Provide the list of the MOCs in the mocset and the associated flags
pub struct List {
  #[clap(parse(from_os_str))]
  /// The moc-set to be read.
  file: PathBuf,
  // list of id
  // deprecated only
  // rm only
  // ...
  #[clap(short = 'r', long = "ranges")]
  /// Print byte ranges instead of byte_size
  ranges: bool,
}

impl List {
  
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let moc_set_reader = MocSetFileReader::new(self.file)?;
    let meta_it = moc_set_reader.meta().into_iter();
    let bytes_it = moc_set_reader.index().into_iter();
    if self.ranges {
      println!("id,status,depth,n_ranges,byte_start,byte_end");
      for (flg_depth_id, byte_range) in meta_it.zip(bytes_it) {
        let id = flg_depth_id.identifier();
        let status = flg_depth_id.status();
        let depth = flg_depth_id.depth();
        let byte_size = byte_range.end - byte_range.start;
        let elem_byte_size = if depth <= Hpx::<u32>::MAX_DEPTH {
          size_of::<u32>()
        } else {
          size_of::<u64>()
        };
        let n_ranges = byte_size / (elem_byte_size << 1); // x2 since 1 range = 2 elems
        println!("{},{},{},{},{},{}", id, status.str_value(), depth, n_ranges, byte_range.start, byte_range.end);
      }
    } else {
      println!("id,status,depth,n_ranges,byte_size");
      for (flg_depth_id, byte_range) in meta_it.zip(bytes_it) {
        let id = flg_depth_id.identifier();
        let status = flg_depth_id.status();
        let depth = flg_depth_id.depth();
        let byte_size = byte_range.end - byte_range.start;
        let elem_byte_size = if depth <= Hpx::<u32>::MAX_DEPTH {
          size_of::<u32>()
        } else {
          size_of::<u64>()
        };
        let n_ranges = byte_size / (elem_byte_size << 1); // x2 since 1 range = 2 elems
        println!("{},{},{},{},{}", id, status.str_value(), depth, n_ranges, byte_size);
      }
    }
    Ok(())
  }
}
