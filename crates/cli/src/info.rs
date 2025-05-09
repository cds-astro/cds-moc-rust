use std::{error::Error, fs::File, io::BufReader, path::PathBuf};

use structopt::StructOpt;

use moclib::{
  deser::fits::{MocIdxType, MocQtyType, MocType, STMocType},
  idx::Idx,
  moc::{range::RangeMocIter, CellMOCIntoIterator, CellMOCIterator, RangeMOCIterator},
  moc2d::{range::RangeMOC2Elem, RangeMOC2Iterator},
  qty::MocQty,
};

use super::input::from_fits_file;

#[derive(StructOpt, Debug)]
pub struct Info {
  #[structopt(parse(from_os_str))]
  /// Path of the FITS file containing a MOC
  file: PathBuf,
}

impl Info {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    print_info(from_fits_file(self.file)?)
  }
}

fn print_info(moc: MocIdxType<BufReader<File>>) -> Result<(), Box<dyn Error>> {
  match moc {
    // MocIdxType::U8(moc) => print_info_qty("u8", moc),
    MocIdxType::U16(moc) => print_info_qty("u16", moc),
    MocIdxType::U32(moc) => print_info_qty("u32", moc),
    MocIdxType::U64(moc) => print_info_qty("u64", moc),
    // MocIdxType::U128(moc) => print_info_qty("u128", moc),
  }
}

fn print_info_qty<T: Idx>(
  idx_type: &str,
  moc: MocQtyType<T, BufReader<File>>,
) -> Result<(), Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(moc) => print_moc_info_type(idx_type, "SPACE", moc),
    MocQtyType::Time(moc) => print_moc_info_type(idx_type, "TIME", moc),
    MocQtyType::Freq(moc) => print_moc_info_type(idx_type, "FREQUENCY", moc),
    MocQtyType::TimeHpx(moc) => print_moc2_info_type(idx_type, "TIME-SPACE", moc),
    MocQtyType::FreqHpx(moc) => print_moc2_info(idx_type, "FREQUENCY-SPACE", moc),
  }
}

fn print_moc_info_type<T: Idx, Q: MocQty<T>>(
  idx_type: &str,
  qty_type: &str,
  moc: MocType<T, Q, BufReader<File>>,
) -> Result<(), Box<dyn Error>> {
  match moc {
    MocType::Ranges(moc) => print_moc_info(idx_type, qty_type, moc),
    MocType::Cells(moc) => print_moc_info(idx_type, qty_type, moc.into_cell_moc_iter().ranges()),
  }
}

fn print_moc_info<T: Idx, Q: MocQty<T>, R: RangeMOCIterator<T, Qty = Q>>(
  idx_type: &str,
  qty_type: &str,
  moc: R,
) -> Result<(), Box<dyn Error>> {
  let depth = moc.depth_max();
  let coverage = moc.coverage_percentage();
  println!("MOC type: {}", qty_type);
  println!("MOC index type: {}", idx_type);
  println!("MOC depth: {}", depth);
  println!("MOC coverage: {:13.9} %", coverage * 100_f64);
  Ok(())
}

fn print_moc2_info_type<T: Idx>(
  idx_type: &str,
  qty_type: &str,
  moc2: STMocType<T, BufReader<File>>,
) -> Result<(), Box<dyn Error>> {
  match moc2 {
    STMocType::V2(moc) => print_moc2_info(idx_type, qty_type, moc),
    STMocType::PreV2(moc) => print_moc2_info(idx_type, qty_type, moc),
  }
}

fn print_moc2_info<T: Idx, Q1: MocQty<T>, Q2: MocQty<T>, R>(
  idx_type: &str,
  qty_type: &str,
  moc2: R,
) -> Result<(), Box<dyn Error>>
where
  T: Idx,
  R: RangeMOC2Iterator<
    T,
    Q1,
    RangeMocIter<T, Q1>,
    T,
    Q2,
    RangeMocIter<T, Q2>,
    RangeMOC2Elem<T, Q1, T, Q2>,
  >,
{
  let name_lowercase_dim1 = Q1::NAME.to_lowercase().to_string();
  let name_lowercase_dim2 = Q2::NAME.to_lowercase().to_string();
  let prefix_uppercase_dim1 = Q1::PREFIX.to_uppercase().to_string();
  let prefix_uppercase_dim2 = Q2::PREFIX.to_uppercase().to_string();
  let max_depth_dim1 = moc2.depth_max_1();
  let max_depth_dim2 = moc2.depth_max_2();
  let (n_elems, n_1, n_2) = moc2.stats();
  println!("MOC type: {}", qty_type);
  println!("MOC index type: {}", idx_type);
  println!("MOC {} depth: {}", name_lowercase_dim1, max_depth_dim1);
  println!("MOC {} depth: {}", name_lowercase_dim2, max_depth_dim2);
  println!(
    "MOC number of ({}-MOC, {}-MOC) tuples: {}",
    prefix_uppercase_dim1, prefix_uppercase_dim2, n_elems
  );
  println!(
    "Tot number of {}-MOC ranges: {}",
    prefix_uppercase_dim1, n_1
  );
  println!(
    "Tot Number of {}-MOC ranges: {}",
    prefix_uppercase_dim2, n_2
  );
  Ok(())
}
