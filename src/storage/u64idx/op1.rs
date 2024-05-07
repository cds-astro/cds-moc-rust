use std::{
  fs::File,
  io::{BufReader, Cursor},
  path::Path,
};

use crate::{
  deser::fits::multiordermap::sum_from_fits_multiordermap,
  moc::{CellMOCIntoIterator, CellMOCIterator, RangeMOCIntoIterator, RangeMOCIterator},
  mom::{HpxMOMIterator, HpxMomIter},
  qty::Hpx,
};

use super::{
  common::{InternalMoc, FMOC, SMOC, STMOC, TMOC},
  store,
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
      Op1::Extend => Ok(moc.expanded()),
      Op1::Contract => Ok(moc.contracted()),
      Op1::ExtBorder => Err(String::from(
        "External border not implemented (yet) for T-MOCs.",
      )),
      Op1::IntBorder => Err(String::from(
        "Internal border not implemented (yet) for T-MOCs.",
      )),
    }
  }
  fn perform_op_on_fmoc(self, moc: &FMOC) -> Result<FMOC, String> {
    match self {
      Op1::Complement => Ok(moc.not()),
      Op1::Degrade { new_depth } => Ok(moc.degraded(new_depth)),
      Op1::Extend => Ok(moc.expanded()),
      Op1::Contract => Ok(moc.contracted()),
      Op1::ExtBorder => Err(String::from(
        "External border not implemented (yet) for F-MOCs.",
      )),
      Op1::IntBorder => Err(String::from(
        "Internal border not implemented (yet) for F-MOCs.",
      )),
    }
  }
  fn perform_op_on_stmoc(self, _moc: &STMOC) -> Result<STMOC, String> {
    match self {
      Op1::Complement => Err(String::from(
        "Complement not implemented (yet) for ST-MOCs.",
      )),
      Op1::Degrade { new_depth: _ } => {
        Err(String::from("Degrade not implemented (yet) for ST-MOCs."))
      }
      Op1::Extend => Err(String::from(
        "Extend border not implemented (yet) for ST-MOCs.",
      )),
      Op1::Contract => Err(String::from(
        "Contract border not implemented (yet) for ST-MOCs.",
      )),
      Op1::ExtBorder => Err(String::from(
        "External border not implemented (yet) for ST-MOCs.",
      )),
      Op1::IntBorder => Err(String::from(
        "Internal border not implemented (yet) for ST-MOCs.",
      )),
    }
  }

  /// Performs the given operation on the given MOC and store the resulting MOC in the store,
  /// returning its index.
  pub(crate) fn exec(&self, index: usize) -> Result<usize, String> {
    store::op1(index, move |moc| match moc {
      InternalMoc::Space(m) => self.perform_op_on_smoc(m).map(InternalMoc::Space),
      InternalMoc::Time(m) => self.perform_op_on_tmoc(m).map(InternalMoc::Time),
      InternalMoc::Frequency(m) => self.perform_op_on_fmoc(m).map(InternalMoc::Frequency),
      InternalMoc::TimeSpace(m) => self.perform_op_on_stmoc(m).map(InternalMoc::TimeSpace),
    })
  }
}

#[derive(Copy, Clone)]
pub(crate) enum Op1MultiRes {
  Split,
  SplitIndirect,
}

impl Op1MultiRes {
  fn perform_op_on_smoc(self, moc: &SMOC) -> Result<Vec<InternalMoc>, String> {
    Ok(
      match self {
        Op1MultiRes::Split => moc.split_into_joint_mocs(false),
        Op1MultiRes::SplitIndirect => moc.split_into_joint_mocs(true),
      }
      .drain(..)
      .map(|cell_moc| {
        cell_moc
          .into_cell_moc_iter()
          .ranges()
          .into_range_moc()
          .into()
      })
      .collect(),
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
    store::op1_multi_res(index, move |moc| match moc {
      InternalMoc::Space(m) => self.perform_op_on_smoc(m),
      InternalMoc::Time(m) => self.perform_op_on_tmoc(m),
      InternalMoc::Frequency(m) => self.perform_op_on_fmoc(m),
      InternalMoc::TimeSpace(m) => self.perform_op_on_stmoc(m),
    })
  }
}

/// Returns the barycenter of the given MOC, (lon, lat) in radians.
pub(crate) fn op1_moc_barycenter(index: usize) -> Result<(f64, f64), String> {
  store::exec_on_one_readonly_moc(index, move |moc| match moc {
    InternalMoc::Space(moc) => Ok(moc.into_range_moc_iter().cells().mean_center()),
    InternalMoc::Time(_) => Err(String::from("Barycenter not implemented for T-MOCs.")),
    InternalMoc::Frequency(_) => Err(String::from("Barycenter not implemented for F-MOCs.")),
    InternalMoc::TimeSpace(_) => Err(String::from("Barycenter not implemented for ST-MOCs.")),
  })
}

/// Returns the largest distance (in radians) from the given point to a MOC vertex (lon, lat) in radians.
pub(crate) fn op1_moc_largest_distance_from_coo_to_moc_vertices(
  index: usize,
  lon: f64,
  lat: f64,
) -> Result<f64, String> {
  store::exec_on_one_readonly_moc(index, move |moc| match moc {
    InternalMoc::Space(moc) => Ok(
      moc
        .into_range_moc_iter()
        .cells()
        .max_distance_from(lon, lat),
    ),
    InternalMoc::Time(_) => Err(String::from("Barycenter not implemented for T-MOCs.")),
    InternalMoc::Frequency(_) => Err(String::from("Barycenter not implemented for F-MOCs.")),
    InternalMoc::TimeSpace(_) => Err(String::from("Barycenter not implemented for ST-MOCs.")),
  })
}

/// Returns all the cells at the moc depth
pub(crate) fn op1_flatten_to_moc_depth(index: usize) -> Result<Vec<u64>, String> {
  store::exec_on_one_readonly_moc(index, move |moc| match moc {
    InternalMoc::Space(m) => Ok(m.flatten_to_fixed_depth_cells().collect()),
    InternalMoc::Time(m) => Ok(m.flatten_to_fixed_depth_cells().collect()),
    InternalMoc::Frequency(m) => Ok(m.flatten_to_fixed_depth_cells().collect()),
    InternalMoc::TimeSpace(_) => Err(String::from(
      "Flatten to MOC depth not implemented for ST-MOCs.",
    )),
  })
}

/// Returns all the cells at the given depth (possibly degrading before flattening)
pub(crate) fn op1_flatten_to_depth(index: usize, depth: u8) -> Result<Vec<u64>, String> {
  store::exec_on_one_readonly_moc(index, move |moc| match moc {
    InternalMoc::Space(m) => Ok(
      m.into_range_moc_iter()
        .degrade(depth)
        .flatten_to_fixed_depth_cells()
        .collect(),
    ),
    InternalMoc::Time(m) => Ok(
      m.into_range_moc_iter()
        .degrade(depth)
        .flatten_to_fixed_depth_cells()
        .collect(),
    ),
    InternalMoc::Frequency(m) => Ok(
      m.into_range_moc_iter()
        .degrade(depth)
        .flatten_to_fixed_depth_cells()
        .collect(),
    ),
    InternalMoc::TimeSpace(_) => Err(String::from(
      "Flatten to depth not implemented for ST-MOCs.",
    )),
  })
}

pub(crate) fn op1_count_split(index: usize, indirect_neigh: bool) -> Result<u32, String> {
  store::exec_on_one_readonly_moc(index, move |moc| match moc {
    InternalMoc::Space(m) => Ok(m.split_into_joint_mocs(indirect_neigh).len() as u32),
    InternalMoc::Time(_) => Err(String::from("Split not implemented for T-MOCs.")),
    InternalMoc::Frequency(_) => Err(String::from("Split not implemented for F-MOCs.")),
    InternalMoc::TimeSpace(_) => Err(String::from("Split not implemented for ST-MOCs.")),
  })
}

pub(crate) fn op1_1st_axis_min(index: usize) -> Result<Option<u64>, String> {
  store::exec_on_one_readonly_moc(index, move |moc| {
    Ok(match moc {
      InternalMoc::Space(moc) => moc.first_index(),
      InternalMoc::Time(moc) => moc.first_index(),
      InternalMoc::Frequency(moc) => moc.first_index(),
      InternalMoc::TimeSpace(stmoc) => stmoc.min_index_left(),
    })
  })
}

pub(crate) fn op1_1st_axis_max(index: usize) -> Result<Option<u64>, String> {
  store::exec_on_one_readonly_moc(index, move |moc| {
    Ok(match moc {
      InternalMoc::Space(moc) => moc.last_index(),
      InternalMoc::Time(moc) => moc.last_index(),
      InternalMoc::Frequency(moc) => moc.last_index(),
      InternalMoc::TimeSpace(stmoc) => stmoc.max_index_left(),
    })
  })
}

pub(crate) fn op1_mom_sum<I>(index: usize, it: I) -> Result<f64, String>
where
  I: Sized + Iterator<Item = (u64, f64)>,
{
  store::exec_on_one_readonly_moc(index, move |moc| match moc {
    InternalMoc::Space(moc) => {
      let mom_it = HpxMomIter::<u64, Hpx<u64>, f64, _>::new(it);
      Ok(mom_it.sum_values_in_hpxmoc(&moc))
    }
    InternalMoc::Time(_) => Err(String::from("MOM Sum not implemented for T-MOCs.")),
    InternalMoc::Frequency(_) => Err(String::from("MOM Sum not implemented for F-MOCs.")),
    InternalMoc::TimeSpace(_) => Err(String::from("MOM Sum not implemented for ST-MOCs.")),
  })
}

pub(crate) fn op1_mom_sum_from_path<P: AsRef<Path>>(
  index: usize,
  mom_path: P,
) -> Result<f64, String> {
  store::exec_on_one_readonly_moc(index, move |moc| match moc {
    InternalMoc::Space(moc) => {
      let file = File::open(&mom_path).map_err(|e| e.to_string())?;
      let reader = BufReader::new(file);
      sum_from_fits_multiordermap(reader, &moc).map_err(|e| e.to_string())
    }
    InternalMoc::Time(_) => Err(String::from("MOM Sum not implemented for T-MOCs.")),
    InternalMoc::Frequency(_) => Err(String::from("MOM Sum not implemented for F-MOCs.")),
    InternalMoc::TimeSpace(_) => Err(String::from("MOM Sum not implemented for ST-MOCs.")),
  })
}

pub(crate) fn op1_mom_sum_from_data(index: usize, mom_data: &[u8]) -> Result<f64, String> {
  store::exec_on_one_readonly_moc(index, move |moc| match moc {
    InternalMoc::Space(moc) => {
      sum_from_fits_multiordermap(BufReader::new(Cursor::new(mom_data)), &moc)
        .map_err(|e| e.to_string())
    }
    InternalMoc::Time(_) => Err(String::from("MOM Sum not implemented for T-MOCs.")),
    InternalMoc::Frequency(_) => Err(String::from("MOM Sum not implemented for F-MOCs.")),
    InternalMoc::TimeSpace(_) => Err(String::from("MOM Sum not implemented for ST-MOCs.")),
  })
}
