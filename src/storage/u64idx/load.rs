use std::{error::Error, io::BufRead};

use crate::deser::fits::RangeMoc2DIterFromFits;
use crate::{
  deser::fits::{MocQtyType, MocType, STMocType},
  idx::Idx,
  moc::{
    range::op::convert::convert_to_u64, CellMOCIntoIterator, CellMOCIterator, RangeMOCIterator,
  },
  moc2d::{range::RangeMOC2, HasTwoMaxDepth},
  qty::{Frequency, Hpx, Time},
};

use super::common::{InternalMoc, FMOC, SMOC, STMOC, TMOC};

/// Returns an `InternalMoc` from fits reading result.
/// WARNING: do not use to get a ST-MOC (we so far assume that ST-MOCs are on u64 only).
pub(crate) fn from_fits_gen<T: Idx, R: BufRead>(
  moc: MocQtyType<T, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(moc) => from_fits_hpx(moc),
    MocQtyType::Time(moc) => from_fits_time(moc),
    MocQtyType::Freq(moc) => from_fits_freq(moc),
    MocQtyType::TimeHpx(_) => Err(String::from("Only u64 ST-MOCs supported").into()),
    MocQtyType::FreqHpx(_) => Err(String::from("Only u64 SF-MOCs supported").into()),
  }
}

pub(crate) fn smoc_from_fits_gen<T: Idx, R: BufRead>(
  moc: MocQtyType<T, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(moc) => from_fits_hpx(moc),
    MocQtyType::Time(_) => {
      Err(String::from("Wrong MOC type. Expected: S-MOCs. Actual: T-MOC").into())
    }
    MocQtyType::Freq(_) => {
      Err(String::from("Wrong MOC type. Expected: S-MOCs. Actual: F-MOC").into())
    }
    MocQtyType::TimeHpx(_) => {
      Err(String::from("Wrong MOC type. Expected: S-MOCs. Actual: ST-MOC").into())
    }
    MocQtyType::FreqHpx(_) => {
      Err(String::from("Wrong MOC type. Expected: S-MOCs. Actual: SF-MOC").into())
    }
  }
}

pub(crate) fn tmoc_from_fits_gen<T: Idx, R: BufRead>(
  moc: MocQtyType<T, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(_) => {
      Err(String::from("Wrong MOC type. Expected: T-MOCs. Actual: S-MOC").into())
    }
    MocQtyType::Time(moc) => from_fits_time(moc),
    MocQtyType::Freq(_) => {
      Err(String::from("Wrong MOC type. Expected: T-MOCs. Actual: F-MOC").into())
    }
    MocQtyType::TimeHpx(_) => {
      Err(String::from("Wrong MOC type. Expected: T-MOCs. Actual: ST-MOC").into())
    }
    MocQtyType::FreqHpx(_) => {
      Err(String::from("Wrong MOC type. Expected: T-MOCs. Actual: SF-MOC").into())
    }
  }
}

pub(crate) fn fmoc_from_fits_gen<T: Idx, R: BufRead>(
  moc: MocQtyType<T, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(_) => {
      Err(String::from("Wrong MOC type. Expected: F-MOCs. Actual: S-MOC").into())
    }
    MocQtyType::Time(_) => {
      Err(String::from("Wrong MOC type. Expected: F-MOCs. Actual: T-MOC").into())
    }
    MocQtyType::Freq(moc) => from_fits_freq(moc),
    MocQtyType::TimeHpx(_) => {
      Err(String::from("Wrong MOC type. Expected: F-MOCs. Actual: ST-MOC").into())
    }
    MocQtyType::FreqHpx(_) => {
      Err(String::from("Wrong MOC type. Expected: F-MOCs. Actual: ST-MOC").into())
    }
  }
}

pub(crate) fn stmoc_from_fits_u64<R: BufRead>(
  moc: MocQtyType<u64, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(_) => {
      Err(String::from("Wrong MOC type. Expected: ST-MOCs. Actual: S-MOC").into())
    }
    MocQtyType::Time(_) => {
      Err(String::from("Wrong MOC type. Expected: ST-MOCs. Actual: T-MOC").into())
    }
    MocQtyType::Freq(_) => {
      Err(String::from("Wrong MOC type. Expected: ST-MOCs. Actual: T-MOC").into())
    }
    MocQtyType::TimeHpx(moc2) => from_fits_spacetime(moc2),
    MocQtyType::FreqHpx(_) => {
      Err(String::from("Wrong MOC type. Expected: ST-MOCs. Actual: SF-MOC").into())
    }
  }
}

pub(crate) fn sfmoc_from_fits_u64<R: BufRead>(
  moc: MocQtyType<u64, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(_) => {
      Err(String::from("Wrong MOC type. Expected: ST-MOCs. Actual: S-MOC").into())
    }
    MocQtyType::Time(_) => {
      Err(String::from("Wrong MOC type. Expected: ST-MOCs. Actual: T-MOC").into())
    }
    MocQtyType::Freq(_) => {
      Err(String::from("Wrong MOC type. Expected: ST-MOCs. Actual: T-MOC").into())
    }
    MocQtyType::TimeHpx(_) => {
      Err(String::from("Wrong MOC type. Expected: SF-MOCs. Actual: ST-MOC").into())
    }
    MocQtyType::FreqHpx(moc2) => from_fits_spacefreq(moc2),
  }
}

/// Returns an `InternalMoc` from fits reading result, knowing the index type is u64.
/// Remark: to be used for ST-MOC (we so far assume that ST-MOCs are on u64 only).
pub(crate) fn from_fits_u64<R: BufRead>(
  moc: MocQtyType<u64, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(moc) => from_fits_hpx(moc),
    MocQtyType::Time(moc) => from_fits_time(moc),
    MocQtyType::Freq(moc) => from_fits_freq(moc),
    MocQtyType::TimeHpx(moc2) => from_fits_spacetime(moc2),
    MocQtyType::FreqHpx(moc2) => from_fits_spacefreq(moc2),
  }
}

fn from_fits_hpx<T: Idx, R: BufRead>(
  moc: MocType<T, Hpx<T>, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  let moc: SMOC = match moc {
    MocType::Ranges(moc) => convert_to_u64::<T, Hpx<T>, _, Hpx<u64>>(moc).into_range_moc(),
    MocType::Cells(moc) => {
      convert_to_u64::<T, Hpx<T>, _, Hpx<u64>>(moc.into_cell_moc_iter().ranges()).into_range_moc()
    }
  };
  Ok(InternalMoc::Space(moc))
}

fn from_fits_time<T: Idx, R: BufRead>(
  moc: MocType<T, Time<T>, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  let moc: TMOC = match moc {
    MocType::Ranges(moc) => convert_to_u64::<T, Time<T>, _, Time<u64>>(moc).into_range_moc(),
    MocType::Cells(moc) => {
      convert_to_u64::<T, Time<T>, _, Time<u64>>(moc.into_cell_moc_iter().ranges()).into_range_moc()
    }
  };
  Ok(InternalMoc::Time(moc))
}

fn from_fits_freq<T: Idx, R: BufRead>(
  moc: MocType<T, Frequency<T>, R>,
) -> Result<InternalMoc, Box<dyn Error>> {
  let moc: FMOC = match moc {
    MocType::Ranges(moc) => {
      convert_to_u64::<T, Frequency<T>, _, Frequency<u64>>(moc).into_range_moc()
    }
    MocType::Cells(moc) => {
      convert_to_u64::<T, Frequency<T>, _, Frequency<u64>>(moc.into_cell_moc_iter().ranges())
        .into_range_moc()
    }
  };
  Ok(InternalMoc::Frequency(moc))
}

fn from_fits_spacetime<R: BufRead>(moc2: STMocType<u64, R>) -> Result<InternalMoc, Box<dyn Error>> {
  // TimeSpaceMoc::<u64, u64>::from_ranges_it(it)
  let moc2: STMOC = match moc2 {
    STMocType::V2(moc2) => {
      let depth_max_1 = moc2.depth_max_1();
      let depth_max_2 = moc2.depth_max_2();
      RangeMOC2::new(depth_max_1, depth_max_2, moc2.collect())
    }
    STMocType::PreV2(moc2) => {
      let depth_max_1 = moc2.depth_max_1();
      let depth_max_2 = moc2.depth_max_2();
      RangeMOC2::new(depth_max_1, depth_max_2, moc2.collect())
    }
  };
  Ok(InternalMoc::TimeSpace(moc2))
}

fn from_fits_spacefreq<R: BufRead>(
  moc2: RangeMoc2DIterFromFits<u64, R, Frequency<u64>, Hpx<u64>>,
) -> Result<InternalMoc, Box<dyn Error>> {
  let depth_max_1 = moc2.depth_max_1();
  let depth_max_2 = moc2.depth_max_2();
  let moc2 = RangeMOC2::new(depth_max_1, depth_max_2, moc2.collect());
  Ok(InternalMoc::FreqSpace(moc2))
}
