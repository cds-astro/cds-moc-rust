
use std::error::Error;

use structopt::StructOpt;

use moclib::idx::Idx;
use moclib::qty::{MocQty, Hpx, Time};

#[derive(StructOpt, Debug)]
pub enum Constants {
  #[structopt(name = "space")]
  /// Provides iformation on HEALPix (used in Space MOCs)
  Space,
  #[structopt(name = "time")]
  /// Provides iformation on HEALPix (used in Space MOCs)
  Time,
}

impl Constants {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    match self {
      Constants::Space => print_hpx_info(),
      Constants::Time => print_time_info(),
    }
    Ok(())
  }
}

fn print_hpx_info() {
  const ARCMIN: f64 = 60.0;
  const ARCSEC: f64 = 3_600.0;
  const MAS: f64 = 3_600_000.0;
  const UMAS: f64 = 3_600_000_000.0;
  println!("Space MOCs");
  println!();
  println!("Index types:");
  println!("- u16 (short), depth max = {:2}", Hpx::<u16>::MAX_DEPTH);
  println!("- u32   (int), depth max = {:2}", Hpx::<u32>::MAX_DEPTH);
  println!("- u64  (long), depth max = {:2}", Hpx::<u64>::MAX_DEPTH);
  println!();
  println!("Layers info:");
  println!("{:>5} {:>10} {:>20} {:>12}",   "depth", "nside", "ncells", "cellSize");
  for depth in 0..=Hpx::<u64>::MAX_DEPTH {
    let ncells = Hpx::<u64>::n_cells(depth as u8);
    let area_rad2 = (4.0 * std::f64::consts::PI) / (ncells as f64);
    let side_deg = area_rad2.sqrt().to_degrees();
    if side_deg < 1.0 / MAS {
      println!("   {:2} {:10} {:20} {:8.4} μas", depth, 1_u64 << depth, ncells, side_deg * UMAS);
    } else if side_deg < 1.0 / ARCSEC {
      println!("   {:2} {:10} {:20} {:8.4} mas", depth, 1_u64 << depth, ncells, side_deg * MAS);
    } else if side_deg < 1.0 / ARCMIN {
      println!("   {:2} {:10} {:20} {:8.4} ″  ", depth, 1_u64 << depth, ncells, side_deg * ARCSEC);
    } else if side_deg < 1.0 {
      println!("   {:2} {:10} {:20} {:8.4} ′  ", depth, 1_u64 << depth, ncells, side_deg * ARCMIN);
    } else {
      println!("   {:2} {:10} {:20} {:8.4} °  ", depth, 1_u64 << depth, ncells, side_deg);
    }
  }
}

fn print_time_info() {
  println!("Time MOCs");
  println!();
  println!("Index types:");
  println!("- u16 (short), depth max = {}", Time::<u16>::MAX_DEPTH);
  println!("- u32   (int), depth max = {}", Time::<u32>::MAX_DEPTH);
  println!("- u64  (long), depth max = {}", Time::<u64>::MAX_DEPTH);
  println!();
  println!("Layers info:");
  println!("{:>5} {:>20}",  "depth", "ncells");
  for depth in 0..=Time::<u64>::MAX_DEPTH {
    println!("   {:2} {:20}", depth, Time::<u64>::n_cells(depth as u8));
  }
}
