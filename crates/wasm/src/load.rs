
use std::io::Cursor;
use std::error::Error;

use moclib::idx::Idx;
use moclib::qty::{Hpx, Time, Frequency};
use moclib::moc::{CellMOCIterator, CellMOCIntoIterator, RangeMOCIterator};
use moclib::moc::range::op::convert::convert_to_u64;
use moclib::moc2d::HasTwoMaxDepth;
use moclib::moc2d::range::RangeMOC2;
use moclib::deser::fits::{MocQtyType, MocType, STMocType};

use super::common::{SMOC, TMOC, FMOC, STMOC, InternalMoc};

/// Returns an `InternalMoc` from fits reading result.
/// WARNING: do not use to get a ST-MOC (we so far assume that ST-MOCs are on u64 only).
pub(crate) fn from_fits_gen<T: Idx>(moc: MocQtyType<T, Cursor<&[u8]>>)
                         -> Result<InternalMoc, Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(moc) => from_fits_hpx(moc),
    MocQtyType::Time(moc) => from_fits_time(moc),
    MocQtyType::Freq(moc) => from_fits_freq(moc),
    MocQtyType::TimeHpx(_) => Err(String::from("Only u64 ST-MOCs supported").into()),
  }
}

/// Returns an `InternalMoc` from fits reading result, knowing the index type is u64.
/// Remark: to be used for ST-MOC (we so far assume that ST-MOCs are on u64 only).
pub(crate)  fn from_fits_u64(moc: MocQtyType<u64, Cursor<&[u8]>>)
                 -> Result<InternalMoc, Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(moc) => from_fits_hpx(moc),
    MocQtyType::Time(moc) => from_fits_time(moc),
    MocQtyType::Freq(moc) => from_fits_freq(moc),
    MocQtyType::TimeHpx(moc2) => from_fits_spacetime(moc2),
  }
}

fn from_fits_hpx<T: Idx>(
  moc: MocType<T, Hpx<T>, Cursor<&[u8]>>
) -> Result<InternalMoc, Box<dyn Error>>
{
  let moc: SMOC = match moc {
    MocType::Ranges(moc) => convert_to_u64::<T, Hpx<T>, _, Hpx<u64>>(moc).into_range_moc(),
    MocType::Cells(moc) => convert_to_u64::<T, Hpx<T>, _, Hpx<u64>>(
      moc.into_cell_moc_iter().ranges()
    ).into_range_moc(),
  };
  Ok(InternalMoc::Space(moc))
}

fn from_fits_time<T: Idx>(
  moc: MocType<T, Time<T>, Cursor<&[u8]>>
) -> Result<InternalMoc, Box<dyn Error>>
{
  let moc: TMOC = match moc {
    MocType::Ranges(moc) => convert_to_u64::<T, Time<T>, _, Time<u64>>(moc).into_range_moc(),
    MocType::Cells(moc) => convert_to_u64::<T, Time<T>, _, Time<u64>>(
      moc.into_cell_moc_iter().ranges()
    ).into_range_moc(),
  };
  Ok(InternalMoc::Time(moc))
}

fn from_fits_freq<T: Idx>(
  moc: MocType<T, Frequency<T>, Cursor<&[u8]>>
) -> Result<InternalMoc, Box<dyn Error>>
{
  let moc: FMOC = match moc {
    MocType::Ranges(moc) => convert_to_u64::<T, Frequency<T>, _, Frequency<u64>>(moc).into_range_moc(),
    MocType::Cells(moc) => convert_to_u64::<T, Frequency<T>, _, Frequency<u64>>(
      moc.into_cell_moc_iter().ranges()
    ).into_range_moc(),
  };
  Ok(InternalMoc::Frequency(moc))
}

fn from_fits_spacetime(
  moc2: STMocType<u64, Cursor<&[u8]>>
) -> Result<InternalMoc, Box<dyn Error>>
{
  // TimeSpaceMoc::<u64, u64>::from_ranges_it(it)
  let moc2: STMOC = match moc2 {
    STMocType::V2(moc2) => {
      let depth_max_1 = moc2.depth_max_1();
      let depth_max_2 = moc2.depth_max_2();
      RangeMOC2::new(depth_max_1, depth_max_2, moc2.collect())
    },
    STMocType::PreV2(moc2) => {
      let depth_max_1 = moc2.depth_max_1();
      let depth_max_2 = moc2.depth_max_2();
      RangeMOC2::new(depth_max_1, depth_max_2, moc2.collect())
    }
  };
  Ok(InternalMoc::TimeSpace(moc2))
}
