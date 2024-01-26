use std::error::Error;

use structopt::StructOpt;

use moclib::qty::{Frequency, Hpx, MocQty, Time};

#[derive(StructOpt, Debug)]
pub enum Constants {
  #[structopt(name = "space")]
  /// Provides information on HEALPix (used in Space MOCs)
  Space,
  #[structopt(name = "time")]
  /// Provides information on time MOCs
  Time,
  #[structopt(name = "freq")]
  /// Provides information on frequency MOCs
  Frequency,
}

impl Constants {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    match self {
      Constants::Space => print_hpx_info(),
      Constants::Time => print_time_info(),
      Constants::Frequency => print_freq_info(),
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
  println!(
    "{:>5} {:>10} {:>20} {:>12}",
    "depth", "nside", "ncells", "cellSize"
  );
  for depth in 0..=Hpx::<u64>::MAX_DEPTH {
    let ncells = Hpx::<u64>::n_cells(depth as u8);
    let area_rad2 = (4.0 * std::f64::consts::PI) / (ncells as f64);
    let side_deg = area_rad2.sqrt().to_degrees();
    if side_deg < 1.0 / MAS {
      println!(
        "   {:2} {:10} {:20} {:8.4} μas",
        depth,
        1_u64 << depth,
        ncells,
        side_deg * UMAS
      );
    } else if side_deg < 1.0 / ARCSEC {
      println!(
        "   {:2} {:10} {:20} {:8.4} mas",
        depth,
        1_u64 << depth,
        ncells,
        side_deg * MAS
      );
    } else if side_deg < 1.0 / ARCMIN {
      println!(
        "   {:2} {:10} {:20} {:8.4} ″  ",
        depth,
        1_u64 << depth,
        ncells,
        side_deg * ARCSEC
      );
    } else if side_deg < 1.0 {
      println!(
        "   {:2} {:10} {:20} {:8.4} ′  ",
        depth,
        1_u64 << depth,
        ncells,
        side_deg * ARCMIN
      );
    } else {
      println!(
        "   {:2} {:10} {:20} {:8.4} °  ",
        depth,
        1_u64 << depth,
        ncells,
        side_deg
      );
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
  println!("{:>5} {:>20}", "depth", "ncells");
  for depth in 0..=Time::<u64>::MAX_DEPTH {
    println!("   {:2} {:20}", depth, Time::<u64>::n_cells(depth as u8));
  }
}

fn print_freq_info() {
  println!("Frequency MOCs");
  println!();
  println!("Index types:");
  println!("- u16 (short), depth max = {}", Frequency::<u16>::MAX_DEPTH);
  println!("- u32   (int), depth max = {}", Frequency::<u32>::MAX_DEPTH);
  println!("- u64  (long), depth max = {}", Frequency::<u64>::MAX_DEPTH);
  println!();
  println!("Layers info:");
  println!("{:>5} {:>20}", "depth", "ncells");
  for depth in 0..=Frequency::<u64>::MAX_DEPTH {
    println!(
      "   {:2} {:20}",
      depth,
      Frequency::<u64>::n_cells(depth as u8)
    );
  }
}
