
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::error::Error;

use structopt::StructOpt;

use moclib::idx::Idx;
use moclib::qty::{MocQty, Hpx, Time, Frequency};
use moclib::elemset::range::MocRanges;
use moclib::moc::{
  RangeMOCIterator, RangeMOCIntoIterator,
  CellMOCIterator, CellMOCIntoIterator,
  range::{RangeMOC, RangeMocIter}
};
use moclib::deser::fits::{
  MocIdxType, MocQtyType, MocType,
  STMocType,
  from_fits_ivoa,
};
use moclib::moc2d::{
  RangeMOC2Iterator,
  range::RangeMOC2Elem
};
use moclib::hpxranges2d::TimeSpaceMoc;

use crate::input::ReducedInputFormat;
use crate::output::OutputFormat;


#[derive(StructOpt, Debug)]
pub enum Op {

  #[structopt(name = "complement")]
  /// Performs a logical 'NOT' on the input MOC (= MOC complement)
  Complement(Op1Args),
  #[structopt(name = "degrade")]
  /// Degrade the input MOC (= MOC complement)
  Degrade {
    /// The new target depth (must be smaller than the input MOC depth).
    new_depth: u8,
    #[structopt(flatten)]
    op: Op1Args
  },

  #[structopt(name = "split")]
  /// Split the disjoint parts of the MOC into distinct MOCs, SMOC only.
  /// WARNING: this may create a lot of files, use first option `--count`.
  Split {
    #[structopt(short = "-i", long = "--8neigh")]
    /// Account for indirect neighbours (8-neigh) instead of direct neighbours (4-neigh) only.
    indirect_neigh: bool,
    #[structopt(short = "-c", long = "--count")]
    /// Only prints the number of disjoint MOCs (security before really executing the task)
    count: bool,
    #[structopt(flatten)]
    op: Op1Args
  },

  #[structopt(name = "extend")]
  /// Add an extra border of cells having the MOC depth, SMOC only
  Extend(Op1Args),
  #[structopt(name = "contract")]
  /// Remove an the internal border made of cells having the MOC depth, SMOC only
  Contract(Op1Args),
  #[structopt(name = "extborder")]
  /// Returns the MOC external border (made of cell of depth the MOC depth), SMOC only
  ExtBorder(Op1Args),
  #[structopt(name = "intborder")]
  /// Returns the MOC internal border (made of cell of depth the MOC depth), SMOC only
  IntBorder(Op1Args),

  #[structopt(name = "inter")]
  /// Performs a logical 'AND' between 2 MOCs (= MOC intersection)
  Intersection(Op2Args),
  #[structopt(name = "union")]
  /// Performs a logical 'OR' between 2 MOCs (= MOC union)
  Union(Op2Args),
  #[structopt(name = "diff")]
  /// Performs a logical 'XOR' between 2 MOCs (= MOC difference)
  Difference(Op2Args),
  #[structopt(name = "minus")]
  /// Performs the logical operation 'AND(left, NOT(right))' between 2 MOCs (= left minus right)
  Minus(Op2Args),
  // MultiUnion OpNArgs (only FITS files?)
  
  
  #[structopt(name = "tfold")]
  /// Returns the union of the S-MOCs associated to T-MOCs intersecting the given T-MOC. Left: T-MOC, right: ST-MOC, res: S-MOC.
  TimeFold(Op2Args),
  #[structopt(name = "sfold")]
  /// Returns the union of the T-MOCs associated to S-MOCs intersecting the given S-MOC. Left: S-MOC, right: ST-MOC, res: T-MOC.
  SpaceFold(Op2Args),

  
  
  // Add (?):
  // * moc contains (exit code=0 + output="true", else exit code=1 + output="false")
  // * moc overlaps
}

impl Op {

  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    match self {
      Op::Complement(op) => op.exec(Op1::Complement),
      Op::Degrade { new_depth, op } => op.exec(Op1::Degrade { new_depth }),
      Op::Split{ indirect_neigh, count, op } => op.exec(Op1::Split { indirect_neigh, count }),
      Op::Extend(op) => op.exec(Op1::Extend),
      Op::Contract(op) => op.exec(Op1::Contract),
      Op::ExtBorder(op) => op.exec(Op1::ExtBorder),
      Op::IntBorder(op) => op.exec(Op1::IntBorder),
      Op::Intersection(op) => op.exec(Op2::Intersection),
      Op::Union(op) => op.exec(Op2::Union),
      Op::Difference(op) => op.exec(Op2::Difference),
      Op::Minus(op) => op.exec(Op2::Minus),
      Op::TimeFold(op) => op.exec(Op2::TimeFold),
      Op::SpaceFold(op) => op.exec(Op2::SpaceFold),
    }
  }
}

#[derive(StructOpt, Debug)]
pub struct Op1Args {
  #[structopt(parse(from_os_str))]
  /// Input MOC file
  input: PathBuf,
  #[structopt(short = "f", long = "input-fmt", default_value = "fits")]
  /// Format of the input MOC file: 'fits' or 'stream' (stream no yet implemented)
  input_fmt: ReducedInputFormat,
  #[structopt(subcommand)]
  output: OutputFormat,
}
impl Op1Args {

  pub fn exec(self, op1: Op1) -> Result<(), Box<dyn Error>> {
    let file = File::open(self.input)?;
    let reader = BufReader::new(file);
    match self.input_fmt {
      ReducedInputFormat::Fits => {
        let moc = from_fits_ivoa(reader)?;
        op1_exec_on_fits(op1, moc, self.output)
      },
      ReducedInputFormat::Stream => {
        todo!() // Stream or mix Fits/Stream
      },
    }
  }
}

fn op1_exec_on_fits(op1: Op1, moc: MocIdxType<BufReader<File>>, output: OutputFormat) -> Result<(), Box<dyn Error>> {
  match moc {
    // MocIdxType::U8(moc) => op1_exec_on_fits_qty(op1, moc, output),
    MocIdxType::U16(moc) => op1_exec_on_fits_qty(op1, moc, output),
    MocIdxType::U32(moc) => op1_exec_on_fits_qty(op1, moc, output),
    MocIdxType::U64(moc) => op1_exec_on_fits_qty(op1, moc, output),
    // MocIdxType::U128(moc) => op1_exec_on_fits_qty(op1, moc, output),
  }
}

fn op1_exec_on_fits_qty<T: Idx>(op1: Op1, moc: MocQtyType<T, BufReader<File>>, output: OutputFormat) -> Result<(), Box<dyn Error>> {
  match moc {
    MocQtyType::Hpx(moc) => op1_exec_on_fits_hpx(op1, moc, output),
    MocQtyType::Time(moc) => op1_exec_on_fits_time(op1, moc, output),
    MocQtyType::TimeHpx(moc) => op1_exec_on_fits_timehpx(op1, moc, output),
    MocQtyType::Freq(moc) => op1_exec_on_fits_freq(op1, moc, output),
  }
}

fn op1_exec_on_fits_hpx<T: Idx>(op1: Op1, moc: MocType<T, Hpx<T>, BufReader<File>>, output: OutputFormat) -> Result<(), Box<dyn Error>> {
  match moc {
    MocType::Ranges(moc) => op1.perform_op_on_srangemoc_iter(moc, output),
    MocType::Cells(moc) =>  op1.perform_op_on_srangemoc_iter(moc.into_cell_moc_iter().ranges(), output),
  }
}

fn op1_exec_on_fits_time<T: Idx>(op1: Op1, moc: MocType<T, Time<T>, BufReader<File>>, output: OutputFormat) -> Result<(), Box<dyn Error>> {
  match moc {
    MocType::Ranges(moc) => op1.perform_op_on_trangemoc_iter(moc, output),
    MocType::Cells(moc) =>  {
      // supposedly unreachable since TMOC supposed to be stored on ranges
      op1.perform_op_on_trangemoc_iter(moc.into_cell_moc_iter().ranges(), output)
    },
  }
}

fn op1_exec_on_fits_timehpx<T: Idx>(
  op1: Op1, 
  moc: STMocType<T, BufReader<File>>, 
  output: OutputFormat
) -> Result<(), Box<dyn Error>> {
  match moc {
    STMocType::V2(stmoc) =>  op1.perform_op_on_strangemoc_iter(stmoc, output),
    STMocType::PreV2(stmoc) => op1.perform_op_on_strangemoc_iter(stmoc, output),
  }
 
}

fn op1_exec_on_fits_freq<T: Idx>(op1: Op1, moc: MocType<T, Frequency<T>, BufReader<File>>, output: OutputFormat) -> Result<(), Box<dyn Error>> {
  match moc {
    MocType::Ranges(moc) => op1.perform_op_on_frangemoc_iter(moc, output),
    MocType::Cells(moc) =>  {
      // supposedly unreachable since TMOC supposed to be stored on ranges
      op1.perform_op_on_frangemoc_iter(moc.into_cell_moc_iter().ranges(), output)
    },
  }
}


pub enum Op1 {
  Complement,
  Degrade { new_depth: u8 },
  Split { indirect_neigh: bool, count: bool },
  Extend,
  Contract,
  ExtBorder,
  IntBorder,
}
impl Op1 {

  /*
  When loading from FITS, if the stored as Ranges, we get an iterator so this is useless:
  pub fn perform_op_on_srangemoc<T: Idx>(self, moc: RangeMOC<T, Hpx<T>>, output: OutputFormat)
    -> Result<(), Box<dyn Error>>
  {
    match self {
      Op1::Complement => output.write_moc(moc.complement().into_range_moc_iter()),
      Op1::Degrade { new_depth } => output.write_moc(moc.degraded(new_depth).into_range_moc_iter()),
      Op1::Extend => output.write_moc(moc.expanded_iter()),
      Op1::Contract => output.write_moc(moc.contracted_iter()),
      Op1::ExtBorder => output.write_moc(moc.external_border_iter()),
      Op1::IntBorder => output.write_moc(moc.internal_border_iter()),
    }
  }*/

  pub fn perform_op_on_srangemoc_iter<T, R>(self, moc_it: R, out: OutputFormat) -> Result<(), Box<dyn Error>>
    where
      T: Idx,
      R: RangeMOCIterator<T, Qty=Hpx<T>>
  {
    match self {
      Op1::Complement => out.write_smoc_possibly_converting_to_u64(moc_it.not()),
      Op1::Degrade  { new_depth } =>  out.write_smoc_possibly_converting_to_u64(moc_it.degrade(new_depth)), // out.write_smoc_converting(moc_it.degrade(new_depth)),
      Op1::Split { indirect_neigh, count }=> {
        let mocs = moc_it.into_range_moc().split_into_joint_mocs(indirect_neigh);
        if count {
          println!("{}", mocs.len());
        } else {
          for (num, cell_moc) in mocs.into_iter().enumerate() {
            let nout = out.clone_with_number(num);
            nout.write_smoc_from_cells_possibly_converting_to_u64(cell_moc.into_cell_moc_iter())?;
          }
        }
        Ok(())
      },
      Op1::Extend => out.write_smoc_possibly_converting_to_u64(moc_it.into_range_moc().expanded_iter()),
      Op1::Contract => out.write_smoc_possibly_converting_to_u64(moc_it.into_range_moc().contracted_iter()),
      Op1::ExtBorder => out.write_smoc_possibly_converting_to_u64(moc_it.into_range_moc().external_border_iter()),
      Op1::IntBorder => out.write_smoc_possibly_converting_to_u64(moc_it.into_range_moc().internal_border_iter()),
    }
  }

  pub fn perform_op_on_trangemoc_iter<T, R>(self, moc_it: R, out: OutputFormat) -> Result<(), Box<dyn Error>>
    where
      T: Idx,
      R: RangeMOCIterator<T, Qty=Time<T>>
  {
    match self {
      Op1::Complement => out.write_tmoc_possibly_converting_to_u64(moc_it.not()),
      Op1::Degrade  { new_depth } => out.write_tmoc_possibly_converting_to_u64(moc_it.degrade(new_depth)), // out.write_tmoc_converting(moc_it.degrade(new_depth)),
      Op1::Split { .. } => Err(String::from("No 'split' operation on T-MOCs.").into()),
      Op1::Extend => Err(String::from("No 'extend' operation on T-MOCs.").into()),
      Op1::Contract => Err(String::from("No 'contract' operation on T-MOCs.").into()),
      Op1::ExtBorder => Err(String::from("No 'extborder' operation on T-MOCs.").into()),
      Op1::IntBorder => Err(String::from("No 'intborder' operation on T-MOCs.").into()),
    }
  }

  pub fn perform_op_on_frangemoc_iter<T, R>(self, moc_it: R, out: OutputFormat) -> Result<(), Box<dyn Error>>
    where
      T: Idx,
      R: RangeMOCIterator<T, Qty=Frequency<T>>
  {
    match self {
      Op1::Complement => out.write_fmoc_possibly_converting_to_u64(moc_it.not()),
      Op1::Degrade  { new_depth } => out.write_fmoc_possibly_converting_to_u64(moc_it.degrade(new_depth)), // out.write_tmoc_converting(moc_it.degrade(new_depth)),
      Op1::Split { .. } => Err(String::from("No 'split' operation on T-MOCs.").into()),
      Op1::Extend => Err(String::from("No 'extend' operation on T-MOCs.").into()),
      Op1::Contract => Err(String::from("No 'contract' operation on T-MOCs.").into()),
      Op1::ExtBorder => Err(String::from("No 'extborder' operation on T-MOCs.").into()),
      Op1::IntBorder => Err(String::from("No 'intborder' operation on T-MOCs.").into()),
    }
  }
  
  pub fn perform_op_on_strangemoc_iter<T, R>(
    self,
    /*moc2: HpxRanges2D<T, Time<T>, T>*/_moc_it: R,
    _out: OutputFormat
  ) -> Result<(), Box<dyn Error>>
    where
      T: Idx,
      R: RangeMOC2Iterator<
        T, Time::<T>, RangeMocIter<T, Time::<T>>,
        T, Hpx::<T>, RangeMocIter<T, Hpx::<T>>,
        RangeMOC2Elem<T, Time::<T>, T, Hpx::<T>>
      >
  {
    //let moc2:  HpxRanges2D<T, Time<T>, T> = HpxRanges2D::from_ranges_it(moc_it);
    match self {
      Op1::Complement => todo!(), // Not yet implemented on ST-MOC!! Do we add time ranges with full space S-MOCs?
      Op1::Degrade  { .. } => todo!(), // Not yet implemented on ST-MOC!! Degrade on T, on S, on both (take two paramaeters)?
      Op1::Split { .. } => Err(String::from("No 'split' operation on ST-MOCs.").into()),
      Op1::Extend => Err(String::from("No 'extend' operation on ST-MOCs.").into()),
      Op1::Contract => Err(String::from("No 'contract' operation on ST-MOCs.").into()),
      Op1::ExtBorder => Err(String::from("No 'extborder' operation on ST-MOCs.").into()),
      Op1::IntBorder => Err(String::from("No 'intborder' operation on ST-MOCs.").into()),
    }
  }
}


#[derive(StructOpt, Debug)]
pub struct Op2Args {
  #[structopt(parse(from_os_str))]
  /// Left MOC file
  left_input: PathBuf,
  #[structopt(short = "l", long = "left-fmt", default_value = "fits")]
  /// Format of the left MOC: 'fits' or 'stream' (stream no yet implemented)
  left_fmt: ReducedInputFormat,
  #[structopt(parse(from_os_str))]
  /// Right MOC file
  right_input: PathBuf,
  #[structopt(short = "r", long = "right-fmt", default_value = "fits")]
  /// Format of the right MOC: 'fits' or 'stream'  (stream no yet implemented)
  right_fmt: ReducedInputFormat,
  #[structopt(subcommand)]
  output: OutputFormat,
}
impl Op2Args {

  pub fn exec(self, op2: Op2) -> Result<(), Box<dyn Error>> {
    let left_file = File::open(self.left_input)?;
    let left_reader = BufReader::new(left_file);
    let right_file = File::open(self.right_input)?;
    let right_reader = BufReader::new(right_file);
    match (self.left_fmt, self.right_fmt) {
      (ReducedInputFormat::Fits, ReducedInputFormat::Fits) => {
        let left_moc = from_fits_ivoa(left_reader)?;
        let right_moc = from_fits_ivoa(right_reader)?;
        op2_exec_on_fits(op2, left_moc, right_moc, self.output)
      },
      _ => {
        todo!() // Stream or mix Fits/Stream
      },
    }
  }
}

fn op2_exec_on_fits(
  op2: Op2,
  left_moc: MocIdxType<BufReader<File>>,
  right_moc: MocIdxType<BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>>
{
  match (left_moc, right_moc)  {
    (MocIdxType::U64(lmoc), MocIdxType::U64(rmoc)) => op2_exec_on_fits_qty(op2, lmoc, rmoc, output),
    (MocIdxType::U32(lmoc), MocIdxType::U32(rmoc)) => op2_exec_on_fits_qty(op2, lmoc, rmoc, output),
    (MocIdxType::U16(lmoc), MocIdxType::U16(rmoc)) => op2_exec_on_fits_qty(op2, lmoc, rmoc, output),
    //(MocIdxType::U128(lmoc), MocIdxType::U128(rmoc)) => op2_exec_on_fits_qty(op2, lmoc, rmoc, output),
    // convert on the fly, ask for manual conversion?
    //(MocIdxType::U128(lmoc), MocIdxType::U16(rmoc)) => op2_exec_on_fits_qty_with_rconv(op2, lmoc, rmoc, output),
    //(MocIdxType::U128(lmoc), MocIdxType::U32(rmoc)) => op2_exec_on_fits_qty_with_rconv(op2, lmoc, rmoc, output),
    //(MocIdxType::U128(lmoc), MocIdxType::U64(rmoc)) => op2_exec_on_fits_qty_with_rconv(op2, lmoc, rmoc, output),
    
    //(MocIdxType::U16(lmoc), MocIdxType::U128(rmoc)) => op2_exec_on_fits_qty_with_lconv(op2, lmoc, rmoc, output),
    //(MocIdxType::U32(lmoc), MocIdxType::U128(rmoc)) => op2_exec_on_fits_qty_with_lconv(op2, lmoc, rmoc, output),
    //(MocIdxType::U64(lmoc), MocIdxType::U128(rmoc)) => op2_exec_on_fits_qty_with_lconv(op2, lmoc, rmoc, output),

    (MocIdxType::U64(lmoc), MocIdxType::U16(rmoc)) => op2_exec_on_fits_qty_with_rconv(op2, lmoc, rmoc, output),
    (MocIdxType::U64(lmoc), MocIdxType::U32(rmoc)) => op2_exec_on_fits_qty_with_rconv(op2, lmoc, rmoc, output),
    
    (MocIdxType::U16(lmoc), MocIdxType::U64(rmoc)) => op2_exec_on_fits_qty_with_lconv(op2, lmoc,rmoc, output),
    (MocIdxType::U32(lmoc), MocIdxType::U64(rmoc)) => op2_exec_on_fits_qty_with_lconv(op2, lmoc,rmoc, output),

    (MocIdxType::U32(lmoc), MocIdxType::U16(rmoc)) => op2_exec_on_fits_qty_with_rconv(op2, lmoc, rmoc, output),
    (MocIdxType::U16(lmoc), MocIdxType::U32(rmoc)) => op2_exec_on_fits_qty_with_lconv(op2, lmoc,rmoc, output),
    
  }
}

fn op2_exec_on_fits_qty<T: Idx>(
  op2: Op2,
  left_moc: MocQtyType<T, BufReader<File>>,
  right_moc: MocQtyType<T, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>>
{
  match (left_moc, right_moc) {
    (MocQtyType::Hpx(left_moc),  MocQtyType::Hpx(right_moc)) => op2_exec_on_fits_moc(op2, left_moc, right_moc, output),
    (MocQtyType::Time(left_moc), MocQtyType::Time(right_moc)) => op2_exec_on_fits_moc(op2, left_moc, right_moc, output),
    (MocQtyType::TimeHpx(left_moc), MocQtyType::TimeHpx(right_moc)) => op2_exec_on_fits_moc2(op2, left_moc, right_moc, output),
    (MocQtyType::Hpx(left_moc), MocQtyType::TimeHpx(right_moc)) => op2_exec_on_fits_smoc_stmoc(op2, left_moc, right_moc, output),
    (MocQtyType::TimeHpx(_), MocQtyType::Hpx(_)) => Err(String::from("Incompatible MOCs. Left: ST-MOC. Right: S-MOC.").into()),
    (MocQtyType::Time(left_moc), MocQtyType::TimeHpx(right_moc)) => op2_exec_on_fits_tmoc_stmoc(op2, left_moc, right_moc, output),
    (MocQtyType::TimeHpx(_), MocQtyType::Time(_)) => Err(String::from("Incompatible MOCs. Left: ST-MOC. Right: T-MOC.").into()),
    (MocQtyType::Hpx(_), MocQtyType::Time(_)) => Err(String::from("Incompatible MOCs. Left: S-MOC. Right: T-MOC.").into()),
    (MocQtyType::Time(_), MocQtyType::Hpx(_)) => Err(String::from("Incompatible MOCs. Left: T-MOC. Right: S-MOC.").into()),
    (MocQtyType::Freq(_), MocQtyType::TimeHpx(_)) => Err(String::from("Incompatible MOCs. Left: F-MOC. Right: ST-MOC.").into()),
    (MocQtyType::TimeHpx(_), MocQtyType::Freq(_)) => Err(String::from("Incompatible MOCs. Left: ST-MOC. Right: F-MOC.").into()),
    (MocQtyType::Freq(left_moc), MocQtyType::Freq(right_moc)) => op2_exec_on_fits_moc(op2, left_moc, right_moc, output),
    (MocQtyType::Hpx(_), MocQtyType::Freq(_)) => Err(String::from("Incompatible MOCs. Left: S-MOC. Right: F-MOC.").into()),
    (MocQtyType::Freq(_), MocQtyType::Hpx(_)) => Err(String::from("Incompatible MOCs. Left: F-MOC. Right: S-MOC.").into()),
    (MocQtyType::Time(_), MocQtyType::Freq(_)) => Err(String::from("Incompatible MOCs. Left: T-MOC. Right: F-MOC.").into()),
    (MocQtyType::Freq(_), MocQtyType::Time(_)) => Err(String::from("Incompatible MOCs. Left: F-MOC. Right: T-MOC.").into()),
  }
}
fn op2_exec_on_fits_moc<T: Idx, Q: MocQty<T>>(
  op2: Op2,
  left_moc: MocType<T, Q, BufReader<File>>,
  right_moc: MocType<T, Q, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>> {
  match (left_moc, right_moc) {
    (MocType::Ranges(left_moc), MocType::Ranges(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc,
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), MocType::Cells(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc.into_cell_moc_iter().ranges(),
        right_moc.into_cell_moc_iter().ranges(),
        output
      ),
    (MocType::Ranges(left_moc), MocType::Cells(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc,
        right_moc.into_cell_moc_iter().ranges(),
        output
      ),
    (MocType::Cells(left_moc), MocType::Ranges(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc.into_cell_moc_iter().ranges(),
        right_moc,
        output
      ),
  }
}

fn op2_exec_on_fits_qty_with_lconv<TL: Idx, TR: Idx + From<TL>>(
  op2: Op2,
  left_moc: MocQtyType<TL, BufReader<File>>,
  right_moc: MocQtyType<TR, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>>
{
  match (left_moc, right_moc) {
    (MocQtyType::Hpx(left_moc),  MocQtyType::Hpx(right_moc)) => op2_exec_on_fits_moc_lconv(op2, left_moc, right_moc, output),
    (MocQtyType::Time(left_moc), MocQtyType::Time(right_moc)) => op2_exec_on_fits_moc_lconv(op2, left_moc, right_moc, output),
    (MocQtyType::TimeHpx(_), MocQtyType::TimeHpx(_)) => Err(String::from("Unable to convert a ST-MOCs datatype so far.").into()),
    (MocQtyType::Hpx(left_moc), MocQtyType::TimeHpx(right_moc)) => op2_exec_on_fits_smoc_stmoc_lconv(op2, left_moc, right_moc, output),
    (MocQtyType::TimeHpx(_), MocQtyType::Hpx(_)) =>  Err(String::from("Incompatible MOCs. Left: ST-MOC. Right: S-MOC.").into()),
    (MocQtyType::Time(left_moc), MocQtyType::TimeHpx(right_moc)) => op2_exec_on_fits_tmoc_stmoc_lconv(op2, left_moc, right_moc, output),
    (MocQtyType::TimeHpx(_), MocQtyType::Time(_)) =>  Err(String::from("Incompatible MOCs. Left: ST-MOC. Right: T-MOC.").into()),
    (MocQtyType::Hpx(_), MocQtyType::Time(_)) => Err(String::from("Incompatible MOCs. Left: S-MOC. Right: T-MOC.").into()),
    (MocQtyType::Time(_), MocQtyType::Hpx(_)) => Err(String::from("Incompatible MOCs. Left: T-MOC. Right: S-MOC.").into()),
    (MocQtyType::Freq(_), MocQtyType::TimeHpx(_)) => Err(String::from("Incompatible MOCs. Left: F-MOC. Right: ST-MOC.").into()),
    (MocQtyType::TimeHpx(_), MocQtyType::Freq(_)) => Err(String::from("Incompatible MOCs. Left: ST-MOC. Right: F-MOC.").into()),
    (MocQtyType::Freq(left_moc), MocQtyType::Freq(right_moc)) => op2_exec_on_fits_moc_lconv(op2, left_moc, right_moc, output),
    (MocQtyType::Hpx(_), MocQtyType::Freq(_)) => Err(String::from("Incompatible MOCs. Left: S-MOC. Right: F-MOC.").into()),
    (MocQtyType::Freq(_), MocQtyType::Hpx(_)) => Err(String::from("Incompatible MOCs. Left: F-MOC. Right: S-MOC.").into()),
    (MocQtyType::Time(_), MocQtyType::Freq(_)) => Err(String::from("Incompatible MOCs. Left: T-MOC. Right: F-MOC.").into()),
    (MocQtyType::Freq(_), MocQtyType::Time(_)) => Err(String::from("Incompatible MOCs. Left: F-MOC. Right: T-MOC.").into()),
  }
}

fn op2_exec_on_fits_moc_lconv<TL: Idx, QL: MocQty<TL>, TR: Idx + From<TL>, QR: MocQty<TR>>(
  op2: Op2,
  left_moc: MocType<TL, QL, BufReader<File>>,
  right_moc: MocType<TR, QR, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>> {
  match (left_moc, right_moc) {
    (MocType::Ranges(left_moc), MocType::Ranges(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc.convert::<TR, QR>(),
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), MocType::Cells(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc.into_cell_moc_iter().ranges().convert::<TR, QR>(),
        right_moc.into_cell_moc_iter().ranges(),
        output
      ),
    (MocType::Ranges(left_moc), MocType::Cells(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc.convert::<TR, QR>(),
        right_moc.into_cell_moc_iter().ranges(),
        output
      ),
    (MocType::Cells(left_moc), MocType::Ranges(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc.into_cell_moc_iter().ranges().convert::<TR, QR>(),
        right_moc,
        output
      ),
  }
}

fn op2_exec_on_fits_qty_with_rconv<TL: Idx + From<TR>, TR: Idx>(
  op2: Op2,
  left_moc: MocQtyType<TL, BufReader<File>>,
  right_moc: MocQtyType<TR, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>>
{
  match (left_moc, right_moc) {
    (MocQtyType::Hpx(left_moc),  MocQtyType::Hpx(right_moc)) => op2_exec_on_fits_moc_rconv(op2, left_moc, right_moc, output),
    (MocQtyType::Time(left_moc), MocQtyType::Time(right_moc)) => op2_exec_on_fits_moc_rconv(op2, left_moc, right_moc, output),
    (MocQtyType::TimeHpx(_), MocQtyType::TimeHpx(_)) =>  Err(String::from("Unable to convert a ST-MOCs datatype so far.").into()),
    (MocQtyType::Hpx(_), MocQtyType::TimeHpx(_)) => Err(String::from("Unable to convert a ST-MOCs datatype so far.").into()),
    (MocQtyType::TimeHpx(_), MocQtyType::Hpx(_)) =>  Err(String::from("Incompatible MOCs. Left: ST-MOC. Right: S-MOC.").into()),
    (MocQtyType::Time(_), MocQtyType::TimeHpx(_)) => Err(String::from("Unable to convert a ST-MOCs datatype so far.").into()),
    (MocQtyType::TimeHpx(_), MocQtyType::Time(_)) =>Err(String::from("Incompatible MOCs. Left: ST-MOC. Right: T-MOC.").into()),
    (MocQtyType::Hpx(_), MocQtyType::Time(_)) => Err(String::from("Incompatible MOCs. Left: S-MOC. Right: T-MOC.").into()),
    (MocQtyType::Time(_), MocQtyType::Hpx(_)) => Err(String::from("Incompatible MOCs. Left: T-MOC. Right: S-MOC.").into()),
    (MocQtyType::Freq(_), MocQtyType::TimeHpx(_)) => Err(String::from("Incompatible MOCs. Left: F-MOC. Right: ST-MOC.").into()),
    (MocQtyType::TimeHpx(_), MocQtyType::Freq(_)) => Err(String::from("Incompatible MOCs. Left: ST-MOC. Right: F-MOC.").into()),
    (MocQtyType::Freq(left_moc), MocQtyType::Freq(right_moc)) => op2_exec_on_fits_moc_rconv(op2, left_moc, right_moc, output),
    (MocQtyType::Hpx(_), MocQtyType::Freq(_)) => Err(String::from("Incompatible MOCs. Left: S-MOC. Right: F-MOC.").into()),
    (MocQtyType::Freq(_), MocQtyType::Hpx(_)) => Err(String::from("Incompatible MOCs. Left: F-MOC. Right: S-MOC.").into()),
    (MocQtyType::Time(_), MocQtyType::Freq(_)) => Err(String::from("Incompatible MOCs. Left: T-MOC. Right: F-MOC.").into()),
    (MocQtyType::Freq(_), MocQtyType::Time(_)) => Err(String::from("Incompatible MOCs. Left: F-MOC. Right: T-MOC.").into()),
  }
}
fn op2_exec_on_fits_moc_rconv<TL: Idx + From<TR>, QL: MocQty<TL>, TR: Idx, QR: MocQty<TR>>(
  op2: Op2,
  left_moc: MocType<TL, QL, BufReader<File>>,
  right_moc: MocType<TR, QR, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>> {
  match (left_moc, right_moc) {
    (MocType::Ranges(left_moc), MocType::Ranges(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc,
        right_moc.convert::<TL, QL>(),
        output
      ),
    (MocType::Cells(left_moc), MocType::Cells(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc.into_cell_moc_iter().ranges(),
        right_moc.into_cell_moc_iter().ranges().convert::<TL, QL>(),
        output
      ),
    (MocType::Ranges(left_moc), MocType::Cells(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc,
        right_moc.into_cell_moc_iter().ranges().convert::<TL, QL>(),
        output
      ),
    (MocType::Cells(left_moc), MocType::Ranges(right_moc)) =>
      op2.perform_op_on_rangemoc_iter(
        left_moc.into_cell_moc_iter().ranges(),
        right_moc.convert::<TL, QL>(),
        output
      ),
  }
}


fn op2_exec_on_fits_moc2<T: Idx>(
  op2: Op2,
  left_stmoc: STMocType<T, BufReader<File>>,
  right_stmoc: STMocType<T, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>> {
  match (left_stmoc, right_stmoc) {
    (STMocType::V2(left_stmoc), STMocType::V2(right_stmoc))       => op2.perform_op_on_strangemoc_iter(left_stmoc, right_stmoc, output),
    (STMocType::PreV2(left_stmoc), STMocType::PreV2(right_stmoc)) => op2.perform_op_on_strangemoc_iter(left_stmoc, right_stmoc, output),
    (STMocType::V2(left_stmoc), STMocType::PreV2(right_stmoc))    => op2.perform_op_on_strangemoc_iter(left_stmoc, right_stmoc, output),
    (STMocType::PreV2(left_stmoc), STMocType::V2(right_stmoc))    => op2.perform_op_on_strangemoc_iter(left_stmoc, right_stmoc, output),
  }
}


fn op2_exec_on_fits_smoc_stmoc<T: Idx>(
  op2: Op2,
  left_moc: MocType<T, Hpx<T>, BufReader<File>>,
  right_moc: STMocType<T, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>> {
  match (left_moc, right_moc) {
    (MocType::Ranges(left_moc), STMocType::V2(right_moc)) =>
      op2.perform_op_on_srangemoc_vs_strangemoc_iter(
        left_moc,
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), STMocType::V2(right_moc)) =>
      op2.perform_op_on_srangemoc_vs_strangemoc_iter(
        left_moc.into_cell_moc_iter().ranges(),
        right_moc,
        output
      ),
    (MocType::Ranges(left_moc), STMocType::PreV2(right_moc)) =>
      op2.perform_op_on_srangemoc_vs_strangemoc_iter(
        left_moc,
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), STMocType::PreV2(right_moc)) =>
      op2.perform_op_on_srangemoc_vs_strangemoc_iter(
        left_moc.into_cell_moc_iter().ranges(),
        right_moc,
        output
      ),
  }
}

fn op2_exec_on_fits_tmoc_stmoc<T: Idx>(
  op2: Op2,
  left_moc: MocType<T, Time<T>, BufReader<File>>,
  right_moc: STMocType<T, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>> {
  match (left_moc, right_moc) {
    (MocType::Ranges(left_moc), STMocType::V2(right_moc)) =>
      op2.perform_op_on_trangemoc_vs_strangemoc_iter(
        left_moc,
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), STMocType::V2(right_moc)) =>
      op2.perform_op_on_trangemoc_vs_strangemoc_iter(
        left_moc.into_cell_moc_iter().ranges(),
        right_moc,
        output
      ),
    (MocType::Ranges(left_moc), STMocType::PreV2(right_moc)) =>
      op2.perform_op_on_trangemoc_vs_strangemoc_iter(
        left_moc,
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), STMocType::PreV2(right_moc)) =>
      op2.perform_op_on_trangemoc_vs_strangemoc_iter(
        left_moc.into_cell_moc_iter().ranges(),
        right_moc,
        output
      ),
  }
}

fn op2_exec_on_fits_smoc_stmoc_lconv<TL: Idx, TR: Idx + From<TL>>(
  op2: Op2,
  left_moc: MocType<TL, Hpx<TL>, BufReader<File>>,
  right_moc: STMocType<TR, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>> {
  match (left_moc, right_moc) {
    (MocType::Ranges(left_moc), STMocType::V2(right_moc)) =>
      op2.perform_op_on_srangemoc_vs_strangemoc_iter(
        left_moc.convert::<TR, Hpx<TR>>(),
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), STMocType::V2(right_moc)) =>
      op2.perform_op_on_srangemoc_vs_strangemoc_iter(
        left_moc.into_cell_moc_iter().ranges().convert::<TR, Hpx<TR>>(),
        right_moc,
        output
      ),
    (MocType::Ranges(left_moc), STMocType::PreV2(right_moc)) =>
      op2.perform_op_on_srangemoc_vs_strangemoc_iter(
        left_moc.convert::<TR, Hpx<TR>>(),
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), STMocType::PreV2(right_moc)) =>
      op2.perform_op_on_srangemoc_vs_strangemoc_iter(
        left_moc.into_cell_moc_iter().ranges().convert::<TR, Hpx<TR>>(),
        right_moc,
        output
      ),
  }
}

fn op2_exec_on_fits_tmoc_stmoc_lconv<TL: Idx, TR: Idx + From<TL>>(
  op2: Op2,
  left_moc: MocType<TL, Time<TL>, BufReader<File>>,
  right_moc: STMocType<TR, BufReader<File>>,
  output: OutputFormat
) -> Result<(), Box<dyn Error>> {
  match (left_moc, right_moc) {
    (MocType::Ranges(left_moc), STMocType::V2(right_moc)) =>
      op2.perform_op_on_trangemoc_vs_strangemoc_iter(
        left_moc.convert::<TR, Time<TR>>(),
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), STMocType::V2(right_moc)) =>
      op2.perform_op_on_trangemoc_vs_strangemoc_iter(
        left_moc.into_cell_moc_iter().ranges().convert::<TR, Time<TR>>(),
        right_moc,
        output
      ),
    (MocType::Ranges(left_moc), STMocType::PreV2(right_moc)) =>
      op2.perform_op_on_trangemoc_vs_strangemoc_iter(
        left_moc.convert::<TR, Time<TR>>(),
        right_moc,
        output
      ),
    (MocType::Cells(left_moc), STMocType::PreV2(right_moc)) =>
      op2.perform_op_on_trangemoc_vs_strangemoc_iter(
        left_moc.into_cell_moc_iter().ranges().convert::<TR, Time<TR>>(),
        right_moc,
        output
      ),
  }
}


pub enum Op2 {
  Intersection,
  Union,
  Difference,
  Minus,
  // ST-MOC scpecific
  TimeFold,
  SpaceFold,
}
impl Op2 {
  
  pub fn perform_op_on_rangemoc_iter<T, Q, L, R>(
    self,
    left_moc_it: L,
    right_moc_it: R,
    output: OutputFormat
  ) -> Result<(), Box<dyn Error>>
    where
      T: Idx,
      Q: MocQty<T>,
      L: RangeMOCIterator<T, Qty=Q>,
      R: RangeMOCIterator<T, Qty=Q>
  {
    match self {
      Op2::Intersection => output.write_moc(left_moc_it.and(right_moc_it)),
      Op2::Union => output.write_moc(left_moc_it.or(right_moc_it)),
      Op2::Difference => output.write_moc(left_moc_it.xor(right_moc_it)),
      Op2::Minus => output.write_moc(left_moc_it.minus(right_moc_it)),
      Op2::TimeFold | Op2::SpaceFold => Err(String::from("Operation must involve a ST-MOC").into()),
    }
  }

  /*pub fn perform_op_on_srangemoc_iter<T, L, R>(
    self,
    left_moc_it: L,
    right_moc_it: R,
    output: OutputFormat
  ) -> Result<(), Box<dyn Error>>
    where
      T: Idx,
      L: RangeMOCIterator<T, Qty=Hpx<T>>,
      R: RangeMOCIterator<T, Qty=Hpx<T>>
  {
    match self {
      Op2::Intersection => output.write_smoc_possibly_converting_to_u64(left_moc_it.and(right_moc_it)),
      Op2::Union => output.write_smoc_possibly_converting_to_u64(left_moc_it.or(right_moc_it)),
      Op2::Difference => output.write_smoc_possibly_converting_to_u64(left_moc_it.xor(right_moc_it)),
      Op2::Minus => output.write_smoc_possibly_converting_to_u64(left_moc_it.minus(right_moc_it)),
    }
  }

  pub fn perform_op_on_trangemoc_iter<T, L, R>(
    self,
    left_moc_it: L,
    right_moc_it: R,
    output: OutputFormat
  ) -> Result<(), Box<dyn Error>>
    where
      T: Idx,
      L: RangeMOCIterator<T, Qty=Time<T>>,
      R: RangeMOCIterator<T, Qty=Time<T>>
  {
    match self {
      Op2::Intersection => output.write_tmoc_possibly_converting_to_u64(left_moc_it.and(right_moc_it)),
      Op2::Union => output.write_tmoc_possibly_converting_to_u64(left_moc_it.or(right_moc_it)),
      Op2::Difference => output.write_tmoc_possibly_converting_to_u64(left_moc_it.xor(right_moc_it)),
      Op2::Minus => output.write_tmoc_possibly_converting_to_u64(left_moc_it.minus(right_moc_it)),
    }
  }*/

  fn perform_op_on_trangemoc_vs_strangemoc_iter<T: Idx, L, R>(
    self,
    left_moc: L,
    right_stmoc: R,
    output: OutputFormat
  ) -> Result<(), Box<dyn Error>>
    where
      L: RangeMOCIterator<T, Qty=Time<T>>,
      R: RangeMOC2Iterator<
        T, Time::<T>, RangeMocIter<T, Time::<T>>,
        T, Hpx::<T>, RangeMocIter<T, Hpx::<T>>,
        RangeMOC2Elem<T, Time::<T>, T, Hpx::<T>>
      >
  {
    let hpx_depth = right_stmoc.depth_max_2();
    let tmoc: MocRanges::<T, Time<T>> = MocRanges::new_from(left_moc.collect());
    let stmoc = TimeSpaceMoc::from_ranges_it(right_stmoc);
    match self {
      Op2::TimeFold => {
        let sranges: MocRanges<T, Hpx<T>> = TimeSpaceMoc::project_on_second_dim(&tmoc, &stmoc);
        let smoc_res = RangeMOC::new(hpx_depth, sranges);
        output.write_smoc_possibly_converting_to_u64(smoc_res.into_range_moc_iter())
      },
      _ => Err(String::from("Operation between T-MOC and  ST-MOC can only be 'tfold'").into()),
    }
  }

  fn perform_op_on_srangemoc_vs_strangemoc_iter<T: Idx, L, R>(
    self,
    left_moc: L,
    right_stmoc: R,
    output: OutputFormat
  ) -> Result<(), Box<dyn Error>>
    where
      L: RangeMOCIterator<T, Qty=Hpx<T>>,
      R: RangeMOC2Iterator<
        T, Time::<T>, RangeMocIter<T, Time::<T>>,
        T, Hpx::<T>, RangeMocIter<T, Hpx::<T>>,
        RangeMOC2Elem<T, Time::<T>, T, Hpx::<T>>
      >
  {
    // Operations on iterator to be written!!
    // In the meantime, use hpxranges2d (via TimeSpaceMoc)
    let time_depth = right_stmoc.depth_max_1();
    let smoc: MocRanges::<T, Hpx<T>> = MocRanges::new_from(left_moc.collect());
    let stmoc = TimeSpaceMoc::from_ranges_it(right_stmoc);
    match self {
      Op2::SpaceFold => {
        let tranges: MocRanges<T, Time<T>> = TimeSpaceMoc::project_on_first_dim(&smoc, &stmoc);
        let tmoc_res = RangeMOC::new(time_depth, tranges);
        output.write_tmoc_possibly_converting_to_u64(tmoc_res.into_range_moc_iter())
      },
      _ => Err(String::from("Operation between T-MOC and  ST-MOC can only be 'sfold'").into()),
    }
  }
  

  fn perform_op_on_strangemoc_iter<T: Idx, L, R>(
    self,
    left_stmoc: L, // HpxRanges2D<T, Time<T>, T>,
    right_stmoc: R, // HpxRanges2D<T, Time<T>, T>,
    output: OutputFormat
  ) -> Result<(), Box<dyn Error>> 
    where
      L: RangeMOC2Iterator<
        T, Time::<T>, RangeMocIter<T, Time::<T>>,
        T, Hpx::<T>, RangeMocIter<T, Hpx::<T>>,
        RangeMOC2Elem<T, Time::<T>, T, Hpx::<T>>
      >,
      R: RangeMOC2Iterator<
        T, Time::<T>, RangeMocIter<T, Time::<T>>,
        T, Hpx::<T>, RangeMocIter<T, Hpx::<T>>,
        RangeMOC2Elem<T, Time::<T>, T, Hpx::<T>>
      >
  {
    // Operations on iterator to be written!!
    // In the meantime, use hpxranges2d (via TimeSpaceMoc)
   /* let (time_depth_1, hpx_depth_1) = (left_stmoc.depth_max_1(), left_stmoc.depth_max_2());
    let (time_depth_2, hpx_depth_2) = (right_stmoc.depth_max_1(), right_stmoc.depth_max_2());
    let left_stmoc = TimeSpaceMoc::from_ranges_it(left_stmoc);
    let right_stmoc = TimeSpaceMoc::from_ranges_it(right_stmoc);
    let result = match self {
      Op2::Intersection => left_stmoc.intersection(&right_stmoc),
      Op2::Union => left_stmoc.union(&right_stmoc),
      Op2::Difference => return Err(String::from("Difference (or xor) not implemented yet for ST-MOCs.").into()), // todo!()
      Op2::Minus => left_stmoc.difference(&right_stmoc), // warning method name is misleading
      Op2::TimeFold | Op2::SpaceFold => return Err(String::from("Operation must involve either a S-MOC or a T-MOC").into()),
    };
    output.write_stmoc(
      result.time_space_iter(time_depth_1.max(time_depth_2), hpx_depth_1.max(hpx_depth_2))
    )*/
    match self {
      Op2::Intersection => {
        let (time_depth_1, hpx_depth_1) = (left_stmoc.depth_max_1(), left_stmoc.depth_max_2());
        let (time_depth_2, hpx_depth_2) = (right_stmoc.depth_max_1(), right_stmoc.depth_max_2());
        let left_stmoc = TimeSpaceMoc::from_ranges_it(left_stmoc);
        let right_stmoc = TimeSpaceMoc::from_ranges_it(right_stmoc);
        output.write_stmoc(
          left_stmoc.intersection(&right_stmoc).time_space_iter(time_depth_1.max(time_depth_2), hpx_depth_1.max(hpx_depth_2))
        )
      },
      Op2::Union => output.write_stmoc(left_stmoc.or(right_stmoc)),
      Op2::Difference => Err(String::from("Difference (or xor) not implemented yet for ST-MOCs.").into()), // todo!()
      Op2::Minus => {
        let (time_depth_1, hpx_depth_1) = (left_stmoc.depth_max_1(), left_stmoc.depth_max_2());
        let (time_depth_2, hpx_depth_2) = (right_stmoc.depth_max_1(), right_stmoc.depth_max_2());
        let left_stmoc = TimeSpaceMoc::from_ranges_it(left_stmoc);
        let right_stmoc = TimeSpaceMoc::from_ranges_it(right_stmoc);
        output.write_stmoc(
          left_stmoc.difference(&right_stmoc).time_space_iter(time_depth_1.max(time_depth_2), hpx_depth_1.max(hpx_depth_2))
        )
      }, // warning method name is misleading
      Op2::TimeFold | Op2::SpaceFold => Err(String::from("Operation must involve either a S-MOC or a T-MOC").into()),
    }
  }

}


#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use crate::op::{Op1Args, Op};
  use crate::input::ReducedInputFormat;
  use crate::output::OutputFormat;
  use std::fs;

  // Yes, I could have mad a single function with different parameters...

  #[test]
  fn test_split_bayestar() {
    let from = Op::Split {
      indirect_neigh: false,
      count: false,//  true,
      op: Op1Args {
        input: PathBuf::from("test/resources/MOC_0.9_bayestar.multiorder.fits"),
        input_fmt: ReducedInputFormat::Fits,
        output: /*OutputFormat::Fits {
          force_u64: true,
          moc_id: None,
          moc_type: None,
          file: "test/resources/MOC_0.9_bayestar.multiorder.split.fits".into(),
        }*/OutputFormat::Ascii {
          fold: Some(80),
          range_len: false,
          opt_file: Some("test/resources/Bayestar.multiorder.actual.ascii".into()),
        }
      }
    };
    from.exec().unwrap();
    // Check results
    for i in 0..9 {
      let expected = format!("test/resources/Bayestar.multiorder.expected.{}.ascii", i);
      let actual = format!("test/resources/Bayestar.multiorder.actual.{}.ascii", i);
      let actual = fs::read_to_string(actual).unwrap();
      let expected = fs::read_to_string(expected).unwrap();
      assert_eq!(actual, expected);
    }
  }
}