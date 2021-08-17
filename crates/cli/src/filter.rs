use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::error::Error;
use std::marker::Send;

use num::PrimInt;
use rayon::prelude::*;
use structopt::StructOpt;

use moclib::idx::Idx;
use moclib::qty::{MocQty, Hpx, Time};
use moclib::moc::range::RangeMOC;
use moclib::deser::fits::{MocIdxType, MocQtyType};

use super::InputTime;
use super::input::from_fits_file;

#[derive(StructOpt, Debug)]
pub struct CsvArgs {
  #[structopt(parse(from_os_str))]
  /// Path of the input CSV file to be filtered (or stdin if equals "-" or empty)
  input_csv: Option<PathBuf>,
  #[structopt(short = "h", long)]
  /// The input file contains a header line (the first non-commented line)
  has_header: bool,
  #[structopt(short = "d", long, default_value = ",")]
  /// Use the provided separator
  delimiter: char,
}
impl CsvArgs {
  
  fn posfilter_input_dispatch<T: Idx>(
    &self,
    pos_filter: &PositionFilter,
    moc: RangeMOC<T, Hpx<T>>
  ) -> Result<(), Box<dyn Error>> {
    let path = self.input_csv.clone().unwrap_or_else(|| PathBuf::from("-"));
    if path ==  PathBuf::from("-") {
      let stdin = std::io::stdin();
      pos_filter.filter_from(BufReader::new(stdin), moc)
    } else {
      let f = File::open(path)?;
      pos_filter.filter_from(BufReader::new(f), moc)
    }
  }

  fn timefilter_input_dispatch<T: Idx>(
    &self,
    time_filter: &TimeFilter,
    moc: RangeMOC<T, Time<T>>
  ) -> Result<(), Box<dyn Error>> {
    let path = self.input_csv.clone().unwrap_or_else(|| PathBuf::from("-"));
    if path ==  PathBuf::from("-") {
      let stdin = std::io::stdin();
      time_filter.filter_from(BufReader::new(stdin), moc)
    } else {
      let f = File::open(path)?;
      time_filter.filter_from(BufReader::new(f), moc)
    }
  }
}

#[derive(StructOpt, Debug)]
pub enum Filter {
  /// Filter a file containing equatorial coordinates using a Space MOC
  Position(PositionFilter), // hpx sort (flag to filter on streaming mode?)
  /// Filter a file containing a time using a Time MOC (NOT YET IMPLEMETNED)
  Time(TimeFilter),
  // SpaceTime(SpaceTime) // TODO
}

impl Filter {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    match self {
      Filter::Position(pos) => pos.exec(),
      Filter::Time(time) => time.exec(),
    }
  }
}

#[derive(StructOpt, Debug)]
pub struct PositionFilter {
  #[structopt(parse(from_os_str))]
  /// Path of the input MOC file
  input_moc: PathBuf,
  #[structopt(flatten)]
  csv_args: CsvArgs,
  #[structopt(short = "l", long, default_value = "0")]
  /// Column name (or index starting at 0) of the decimal degrees longitude field
  lon: String,
  #[structopt(short = "b", long, default_value = "1")]
  /// Column name (or index starting at 0) of the decimal degrees latitude field
  lat: String,
  #[structopt(long = "--n-threads")]
  /// Use multithreading with the given number of threads
  n_threads: Option<u16>,
  #[structopt(long = "--chunk-size", default_value = "200000")]
  /// Number of rows to be processed in parallel (only with multi-threading on)
  chunk_size: u32,
}
impl PositionFilter {
  pub fn exec(&self) -> Result<(), Box<dyn Error>> {
    match from_fits_file(self.input_moc.clone())? {
      MocIdxType::U16(moc) =>
        match moc {
          MocQtyType::Hpx(moc) => self.filter(moc.collect()),
          _ => Err(String::from("Input MOC must be a Spatial MOC.").into()),
        },
      MocIdxType::U32(moc) =>
        match moc {
          MocQtyType::Hpx(moc) => self.filter(moc.collect()),
          _ => Err(String::from("Input MOC must be a Spatial MOC.").into()),
        },
      MocIdxType::U64(moc) =>
        match moc {
          MocQtyType::Hpx(moc) => self.filter(moc.collect()),
          _ => Err(String::from("Input MOC must be a Spatial MOC.").into()),
        },
    }
  }

  fn filter<T: Idx>(&self, moc: RangeMOC<T, Hpx<T>>) -> Result<(), Box<dyn Error>> {
    self.csv_args.posfilter_input_dispatch(self, moc)
  }
  fn filter_from<T: Idx, R: BufRead + Send>(&self, reader: R, moc: RangeMOC<T, Hpx<T>>) -> Result<(), Box<dyn Error>> {
    let sep = self.csv_args.delimiter;
    let mut it = reader.lines().peekable();
    // Consume and echo starting comments
    while let Some(Ok(line)) = it.peek() {
      // If Err instead of Ok, it will be catch later
      if line.starts_with('#') || line.is_empty() {
        println!("{}", line);
        it.next(); // Simply consume the iterator element (with is a comment line)
      } else {
        break;
      }
    }
    // Deal with header line (if any) and pos column indices
    let (ilon, ilat) = if self.csv_args.has_header {
      if let Some(line) = it.next().transpose()? {
        println!("{}", line);
        let col_names: Vec<&str> = line.split(sep).collect();
        let ilon = col_names.iter().position(|name| name == &self.lon);
        let ilat = col_names.iter().position(|name| name == &self.lat);
        if let (Some(ilon), Some(ilat)) = (ilon, ilat) {
          (ilon, ilat)
        } else {
          (self.lon.parse::<usize>()?, self.lat.parse::<usize>()?)
        }
      } else {
        // iterator already depleted, so we can return rubbish
        (0, 1)
      }
    } else {
      (self.lon.parse::<usize>()?, self.lat.parse::<usize>()?)
    };
    // We can start the job
    let layer = healpix::nested::get(moc.depth_max());
    // WARNING: THIS WILL NOT WORK IF MOC CONTAINS DEPTH > 29!! 
    let shift = Hpx::<u64>::shift_from_depth_max(moc.depth_max()) as u32;
    match self.n_threads {
      None | Some(1) => {
        if ilon < ilat {
          let olat = ilat - ilon - 1;
          for line in it {
            let line = line?;
            let mut split_it = line.split(sep);
            let lon = split_it.nth(ilon).map(|s| s.parse::<f64>());
            let lat = split_it.nth(olat).map(|s| s.parse::<f64>());
            if let (Some(Ok(lon)), Some(Ok(lat))) = (lon, lat) {
              let icell = T::from_u64_idx(layer.hash(lon.to_radians(), lat.to_radians()).unsigned_shl(shift));
              if moc.contains_val(&icell) {
                println!("{}", line);
              }
            }
          }
        } else {
          // Yeah, code repetition to put a if out of a for loop...
          let olon = ilon - ilat - 1;
          for line in it {
            let line = line?;
            let mut split_it = line.split(sep);
            let lat = split_it.nth(ilat).map(|s| s.parse::<f64>());
            let lon = split_it.nth(olon).map(|s| s.parse::<f64>());
            if let (Some(Ok(lon)), Some(Ok(lat))) = (lon, lat) {
              let icell = T::from_u64_idx(layer.hash(lon.to_radians(), lat.to_radians()).unsigned_shl(shift));
              if moc.contains_val(&icell) {
                println!("{}", line);
              }
            }
          }
        }
      },
      Some(nthread) => {
        rayon::ThreadPoolBuilder::new().num_threads(nthread as usize).build_global().unwrap();
        let chunk_size = self.chunk_size as usize;
        let mut input: Vec<Result<String, _>> = (&mut it).take(chunk_size).collect();
        let mut output: Vec<String> = Default::default();
        while !input.is_empty() {
          let (next_output, ((), next_input)) = rayon::join(
            || if ilon < ilat {
              let olat = ilat - ilon - 1;
              input.par_iter()
                .filter_map(|res| res.as_ref().ok())
                .filter_map(|line| {
                  let mut split_it = line.split(sep);
                  let lon = split_it.nth(ilon).map(|s| s.parse::<f64>());
                  let lat = split_it.nth(olat).map(|s| s.parse::<f64>());
                  if let (Some(Ok(lon)), Some(Ok(lat))) = (lon, lat) {
                    let icell = T::from_u64_idx(layer.hash(lon.to_radians(), lat.to_radians()).unsigned_shl(shift));
                    if moc.contains_val(&icell) {
                      Some(line.clone())
                    } else {
                      None
                    }
                  } else {
                    None
                  }
                })
                .collect()
            } else {
              // Yeah, code repetition to put a if out of a for loop...
              let olon = ilon - ilat - 1;
              input.par_iter()
                .filter_map(|res| res.as_ref().ok())
                .filter_map(|line| {
                  let mut split_it = line.split(sep);
                  let lat = split_it.nth(ilat).map(|s| s.parse::<f64>());
                  let lon = split_it.nth(olon).map(|s| s.parse::<f64>());
                  if let (Some(Ok(lon)), Some(Ok(lat))) = (lon, lat) {
                    let icell = T::from_u64_idx(layer.hash(lon.to_radians(), lat.to_radians()).unsigned_shl(shift));
                    if moc.contains_val(&icell) {
                      Some(line.clone())
                    } else {
                      None
                    }
                  } else {
                    None
                  }
                })
                .collect()
            },
            || rayon::join(
              || for line in output { println!("{}", line); }, // write output
              || (&mut it).take(chunk_size).collect(),        // read new chunk
            ),
          );
          input = next_input;
          output = next_output;
        }
        for line in output { println!("{}", line); } // write output
      },
    };
    Ok(())
  }
}


#[derive(StructOpt, Debug)]
pub struct TimeFilter {
  #[structopt(parse(from_os_str))]
  /// Path of the input MOC file
  input_moc: PathBuf,
  #[structopt(flatten)]
  csv_args: CsvArgs,
  #[structopt(short = "t", long, default_value = "0")]
  /// Column name (or index starting at 0) of the MJD time field
  time: String,
  #[structopt(long = "time-type", default_value = "jd")]
  /// Time type: 'jd' (julian date), 'mjd' (modified julian date) or 'usec' (microsec since JD=0)
  time_type: InputTime,
}
impl TimeFilter {
  pub fn exec(&self) -> Result<(), Box<dyn Error>> {
    match from_fits_file(self.input_moc.clone())? {
      MocIdxType::U16(moc) =>
        match moc {
          MocQtyType::Time(moc) => self.filter(moc.collect()),
          _ => Err(String::from("Input MOC must be a Time MOC.").into()),
        },
      MocIdxType::U32(moc) =>
        match moc {
          MocQtyType::Time(moc) => self.filter(moc.collect()),
          _ => Err(String::from("Input MOC must be a Time MOC.").into()),
        },
      MocIdxType::U64(moc) =>
        match moc {
          MocQtyType::Time(moc) => self.filter(moc.collect()),
          _ => Err(String::from("Input MOC must be a Time MOC.").into()),
        },
    }
  }

  fn filter<T: Idx>(&self, moc: RangeMOC<T, Time<T>>) -> Result<(), Box<dyn Error>> {
    self.csv_args.timefilter_input_dispatch(self, moc)
  }
  fn filter_from<T: Idx, R: BufRead + Send>(&self, reader: R, moc: RangeMOC<T, Time<T>>) -> Result<(), Box<dyn Error>> {
    let sep = self.csv_args.delimiter;
    let mut it = reader.lines().peekable();
    // Consume and echo starting comments
    while let Some(Ok(line)) = it.peek() {
      // If Err instead of Ok, it will be catch later
      if line.starts_with('#') || line.is_empty() {
        println!("{}", line);
        it.next(); // Simply consume the iterator element (with is a comment line)
      } else {
        break;
      }
    }
    // Deal with header line (if any) and pos column indices
    let itime = if self.csv_args.has_header {
      if let Some(line) = it.next().transpose()? {
        println!("{}", line);
        let col_names: Vec<&str> = line.split(sep).collect();
        let itime = col_names.iter().position(|name| name == &self.time);
        if let Some(itime) = itime {
          itime
        } else {
          self.time.parse::<usize>()?
        }
      } else {
        // iterator already depleted, so we can return rubbish
        0
      }
    } else {
      self.time.parse::<usize>()?
    };
    // We can start the job
    // WARNING: THIS WILL NOT WORK IF MOC CONTAINS DEPTH > 29!! 
    let shift = Time::<u64>::shift_from_depth_max(moc.depth_max()) as u32;
    for line in it {
      let line = line?;
      let mut split_it = line.split(sep);
      let icell = split_it.nth(itime)
        .and_then(|s| self.time_type.parse(s).ok())
        .map(|tcell| T::from_u64_idx(tcell).unsigned_shl(shift));
      if let Some(icell) = icell {
        if moc.contains_val(&icell) {
          println!("{}", line);
        }
      }
    }
    Ok(())
  }
}

