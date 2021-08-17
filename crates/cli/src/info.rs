
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::error::Error;

use structopt::StructOpt;

use moclib::idx::Idx;
use moclib::qty::{MocQty, Hpx, Time};
use moclib::deser::fits::{MocIdxType, MocQtyType, MocType, STMocType};
use moclib::moc::{
  RangeMOCIterator,
  CellMOCIterator, CellMOCIntoIterator,
  range::RangeMocIter
};
use moclib::moc2d::{
  RangeMOC2Iterator,
  range::RangeMOC2Elem
};

use super::input::from_fits_file;

#[derive(StructOpt, Debug)]
pub struct Info {
  #[structopt(parse(from_os_str))]
  /// Path of the FITS file containing a MOC
  file: PathBuf
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

fn print_info_qty<T: Idx>(idx_type: &str, moc: MocQtyType<T, BufReader<File>>) -> Result<(), Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(moc) => print_moc_info_type(idx_type, "SPACE", moc),
    MocQtyType::Time(moc) => print_moc_info_type(idx_type, "TIME", moc),
    MocQtyType::TimeHpx(moc) => print_moc2_info_type(idx_type, "TIME-SPACE", moc),
  }
}

fn print_moc_info_type<T: Idx, Q: MocQty<T>>(idx_type: &str, qty_type: &str, moc: MocType<T, Q, BufReader<File>>) -> Result<(), Box<dyn Error>> {
  match moc {
    MocType::Ranges(moc) => print_moc_info(idx_type, qty_type, moc),
    MocType::Cells(moc) =>  print_moc_info(idx_type, qty_type, moc.into_cell_moc_iter().ranges()),
  }
}

fn print_moc_info<T: Idx, Q: MocQty<T>, R: RangeMOCIterator<T, Qty=Q>>(idx_type: &str, qty_type: &str, moc: R) -> Result<(), Box<dyn Error>> {
  let depth = moc.depth_max();
  let coverage = moc.coverage_percentage();
  println!("MOC type: {}", qty_type);
  println!("MOC index type: {}", idx_type);
  println!("MOC depth: {}", depth);
  println!("MOC coverage: {:13.9} %", coverage * 100_f64);
  Ok(())
}

fn print_moc2_info_type<T: Idx>(idx_type: &str, qty_type: &str, moc2: STMocType<T, BufReader<File>>) -> Result<(), Box<dyn Error>> {
  match moc2 {
    STMocType::V2(moc) => print_moc2_info(idx_type, qty_type, moc),
    STMocType::PreV2(moc) => print_moc2_info(idx_type, qty_type, moc),
  }
}

fn print_moc2_info<T: Idx, R>(idx_type: &str, qty_type: &str, moc2: R) -> Result<(), Box<dyn Error>>
  where
    T: Idx,
    R: RangeMOC2Iterator<
      T, Time::<T>, RangeMocIter<T, Time::<T>>,
      T, Hpx::<T>, RangeMocIter<T, Hpx::<T>>,
       RangeMOC2Elem<T, Time::<T>, T, Hpx::<T>>
    >
{
  let max_depth_hpx = moc2.depth_max_1();
  let max_depth_time = moc2.depth_max_2();
  println!("MOC type: {}", qty_type);
  println!("MOC index type: {}", idx_type);
  println!("MOC hpx  depth: {}", max_depth_hpx);
  println!("MOC time depth: {}", max_depth_time);
  // println!("MOC number of (T-MOC, S-MOC) tuples: {}", ??);
  Ok(())
}
