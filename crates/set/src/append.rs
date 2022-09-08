
use std::{
  fs::File,
  io::BufReader,
  path::PathBuf,
  error::Error
};

use clap::Parser;

use moclib::{
  qty::{MocQty, Hpx},
  moc::{
    RangeMOCIterator, RangeMOCIntoIterator,
    range::{
      RangeMOC,
      op::convert::{convert, convert_from_u64}
    }
  },
  deser::fits::{
    MocIdxType, MocQtyType
  }
};

use crate::{
  StatusFlag,
  from_fits_file,
  MocSetFileWriter
};

#[derive(Debug, Parser)]
/// Append the given MOCs to an existing mocset
pub struct Append {
  #[clap(parse(from_os_str))]
  /// The moc-set to be updated.
  file: PathBuf,
  /// Identifier of the MOC we want to append.
  /// 'moc_id' must be a positive integer smaller than 281_474_976_710_655 (can be stored on 6 bytes).
  /// Use a negative value to flag as deprecated. 
  id: i64,
  /// Path of the MOC to be append
  moc_path: PathBuf,
}

impl Append {

  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    let (id, flag) = if self.id < 0 {
      (-self.id as u64, StatusFlag::Deprecated)
    } else {
      (self.id as u64, StatusFlag::Valid)
    };
    let moc = from_fits_file(self.moc_path.clone())?;
    let mut moc_set = MocSetFileWriter::new(self.file)?;
    // TODO/WARNING: part of code duplicated with 'mk.rs' => to be clean (e.g. passing a closure)! 
    let result = match moc {
      MocIdxType::<BufReader<File>>::U16(MocQtyType::<u16, BufReader<File>>::Hpx(moc)) => {
        let moc: RangeMOC<u16, Hpx::<u16>> = moc.collect();
        // We convert to u32 because of alignment of u8 slices converted to slice of range of u64.
        // Wth u32, no alignment problems since with use Range<u32>, i.e. multiples of 64 bits.
        // It is not the case with Range<u16> since they are multiple of 32 bits only.
        let moc: RangeMOC<u32, Hpx::<u32>> = convert(moc.into_range_moc_iter()).into_range_moc();
        moc_set.append_moc(flag, id, moc)
      },
      MocIdxType::<BufReader<File>>::U32(MocQtyType::<u32, BufReader<File>>::Hpx(moc)) => {
        let moc: RangeMOC<u32, Hpx::<u32>> = moc.collect();
        moc_set.append_moc(flag, id, moc)
      },
      MocIdxType::<BufReader<File>>::U64(MocQtyType::<u64, BufReader<File>>::Hpx(moc)) => {
        let moc: RangeMOC<u64, Hpx::<u64>> = moc.collect();
        if moc.depth_max() <= Hpx::<u32>::MAX_DEPTH {
          // We convert to save space
          let moc: RangeMOC<u32, Hpx::<u32>> = convert_from_u64(moc.into_range_moc_iter()).into_range_moc();
          moc_set.append_moc(flag, id, moc)
        } else {
          moc_set.append_moc(flag, id, moc)
        }
      },
      _ => Err(format!("MOC id: {}; path: {:?}: MOC type not supported.", id, self.moc_path).into()),
    };
    moc_set.release()?;
    result
  }
}
