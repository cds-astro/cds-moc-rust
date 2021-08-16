
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::{self, FromStr};
use std::num::ParseFloatError;
use std::path::PathBuf;
use std::error::Error;
use std::ops::Range;

use structopt::StructOpt;

use moclib::qty::{MocQty, Hpx, Time};
use moclib::moc::RangeMOCIntoIterator;
use moclib::moc::range::RangeMOC;

use super::InputTime;
use super::output::OutputFormat;

const HALF_PI: f64 = 0.5 * std::f64::consts::PI;
const PI: f64 = std::f64::consts::PI;
const TWICE_PI: f64 = 2.0 * std::f64::consts::PI;

#[derive(Debug)]
pub struct Vertices {
  // (ra0,dec0),(ra1,dec1),...,(ran,decn)
  list: Vec<(f64, f64)>
}

impl FromStr for Vertices {
  type Err = ParseFloatError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let list: Vec<f64> = s
      .replace("(", "")
      .replace(")", "")
      .split(",")
      .map(|t| str::parse::<f64>(t.trim()))
      .collect::<Result<Vec<f64>, _>>()?;
    Ok(
      Vertices {
        list: list.iter().step_by(2).zip(list.iter().skip(1).step_by(2))
          .map(|(lon, lat)| (*lon, *lat))
          .collect()
      }
    )
  }
}

#[derive(StructOpt, Debug)]
pub enum From {
  #[structopt(name = "cone")]
  /// Create a Spatial MOC from the given cone
  Cone {
    /// Depth of the created MOC, in `[0, 29]`.
    depth: u8,
    /// Longitude of the cone center (in degrees)
    lon_deg: f64,
    /// Latitude of the cone center (in degrees)
    lat_deg: f64,
    /// Radius of the cone (in degrees)
    r_deg: f64,
    #[structopt(subcommand)]
    out: OutputFormat
  },
  #[structopt(name = "ellipse")]
  /// Create a Spatial MOC from the given elliptical cone
  EllipticalCone {
    /// Depth of the created MOC, in `[0, 29]`.
    depth: u8,
    /// Longitude of the elliptical cone center (in degrees)
    lon_deg: f64,
    /// Latitude of the elliptical cone center (in degrees)
    lat_deg: f64,
    /// Elliptical cone semi-major axis (in degrees)
    a_deg: f64,
    /// Elliptical cone semi-minor axis (in degrees)
    b_deg: f64,
    /// Elliptical cone position angle (in degrees)
    pa_deg: f64,
    #[structopt(subcommand)]
    out: OutputFormat
  },
  #[structopt(name = "zone")]
  /// Create a Spatial MOC from the given zone
  Zone {
    /// Depth of the created MOC, in `[0, 29]`.
    depth: u8,
    /// Longitude min, in degrees
    lon_deg_min: f64,
    /// Latitude min, in degrees
    lat_deg_min: f64,
    /// Longitude max, in degrees
    lon_deg_max: f64,
    /// Latitude max, in degrees
    lat_deg_max: f64,
    #[structopt(subcommand)]
    out: OutputFormat
  },
  #[structopt(name = "box")]
  /// Create a Spatial MOC from the given box
  Box { // transform into a polygon!
    /// Depth of the created MOC, in `[0, 29]`.
    depth: u8,
    /// Longitude of the box center, in degrees
    lon_deg: f64,
    /// Latitude of the box center, in degrees
    lat_deg: f64,
    /// Semi-major axis of the box, in degrees
    a_deg: f64,
    /// Semi-minor axis of the box, in degrees
    b_deg: f64,
    /// Position angle of the box, in degrees
    pa_deg: f64,
    #[structopt(subcommand)]
    out: OutputFormat
  },
  #[structopt(name = "polygon")]
  /// Create a Spatial MOC from the given polygon
  Polygon {
    /// Depth of the created MOC, in `[0, 29]`.
    depth: u8,
    /// List of vertices: "(lon,lat),(lon,lat),...,(lon,lat)" in degrees
    vertices_deg: Vertices, // (ra0,dec0),(ra1,dec1),...,(ran,decn)
    #[structopt(short = "c", long)]
    /// Gravity center of the polygon out of the polygon (in by default)
    complement: bool,
    #[structopt(subcommand)]
    out: OutputFormat
  },
  #[structopt(name = "pos")]
  /// Create a Spatial MOC from a list of positions in decimal degrees (one pair per line, longitude first, then latitude).
  Positions {
    /// Depth of the created MOC, in `[0, 29]`.
    depth: u8,
    #[structopt(parse(from_os_str))]
    /// The input file, use '-' for stdin
    input: PathBuf,
    #[structopt(short = "s", long = "separator", default_value = " ")]
    /// Separator between both coordinates (default = ' ')
    separator: String,
    #[structopt(subcommand)]
    out: OutputFormat
  },
  #[structopt(name = "timestamp")]
  /// Create a Time MOC from a list of timestamp (one per line).
  Timestamp {
    /// Depth of the created MOC, in `[0, 61]`.
    depth: u8,
    #[structopt(long = "time-type", default_value = "jd")]
    /// Time type: 'jd' (julian date), 'mjd' (modified julian date) or 'usec' (microsec since JD=0)
    time: InputTime,
    #[structopt(parse(from_os_str))]
    /// The input file, use '-' for stdin
    input: PathBuf,
    #[structopt(subcommand)]
    out: OutputFormat
  },
  #[structopt(name = "timerange")]
  /// Create a Time MOC from a list of time range (one range per line, lower bound first, then upper bound).
  Timerange {
    /// Depth of the created MOC, in `[0, 61]`.
    depth: u8,
    #[structopt(long = "time-type", default_value = "jd")]
    /// Time type: 'jd' (julian date), 'mjd' (modified julian date) or 'usec' (microsec since JD=0)
    time: InputTime,
    #[structopt(parse(from_os_str))]
    /// The input file, use '-' for stdin
    input: PathBuf,
    #[structopt(short = "s", long = "separator", default_value = " ")]
    /// Separator between time lower and upper bounds (default = ' ')
    separator: String,
    #[structopt(subcommand)]
    out: OutputFormat
  }
  // ST-MOC  todo!()
  // moc from time_pos                    (TIME,RA,DEC)
  // moc from trange_pos                  (TMIN,TMAX,RA,DEC)
}

impl From {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    // println!("From {:?}", from);
    match self {
      From::Cone {
        depth,
        lon_deg,
        lat_deg,
        r_deg,
        out
      } => {
        let lon = lon_deg2rad(lon_deg)?;
        let lat = lat_deg2rad(lat_deg)?;
        let r = r_deg.to_radians();
        if r <= 0.0 || PI <= r {
          Err(String::from("Radius must be in ]0, pi[").into())
        } else {
          let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_cone(lon, lat, r, depth, 2);
          out.write_smoc_possibly_auto_converting_from_u64(moc.into_range_moc_iter())
        }
      },
      From::EllipticalCone {
        depth,
        lon_deg,
        lat_deg,
        a_deg,
        b_deg,
        pa_deg,
        out
      } => {
        let lon = lon_deg2rad(lon_deg)?;
        let lat = lat_deg2rad(lat_deg)?;
        let a = a_deg.to_radians();
        let b = b_deg.to_radians();
        let pa = pa_deg.to_radians();
        if a <= 0.0 || HALF_PI <= a {
          Err(String::from("Semi-major axis must be in ]0, pi/2]").into())
        } else if b <= 0.0 || a <= b {
          Err(String::from("Semi-minor axis must be in ]0, a[").into())
        } else if pa <= 0.0 || HALF_PI <= pa {
          Err(String::from("Position angle must be in [0, pi[").into())
        } else {
          let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_elliptical_cone(lon, lat, a, b, pa, depth, 2);
          out.write_smoc_possibly_auto_converting_from_u64(moc.into_range_moc_iter())
        }
      },
      From::Zone {
        depth,
        lon_deg_min,
        lat_deg_min,
        lon_deg_max,
        lat_deg_max,
        out
      } => {
        let lon_min = lon_deg2rad(lon_deg_min)?;
        let lat_min = lat_deg2rad(lat_deg_min)?;
        let lon_max = lon_deg2rad(lon_deg_max)?;
        let lat_max = lat_deg2rad(lat_deg_max)?;
        let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_zone(lon_min, lat_min, lon_max, lat_max, depth);
        out.write_smoc_possibly_auto_converting_from_u64(moc.into_range_moc_iter())
      },
      From::Box {
        depth,
        lon_deg,
        lat_deg,
        a_deg,
        b_deg,
        pa_deg,
        out
      } => {
        let lon = lon_deg2rad(lon_deg)?;
        let lat = lat_deg2rad(lat_deg)?;
        let a = a_deg.to_radians();
        let b = b_deg.to_radians();
        let pa = pa_deg.to_radians();
        if a <= 0.0 || HALF_PI <= a {
          Err(String::from("Semi-major axis must be in ]0, pi/2]").into())
        } else if b <= 0.0 || a <= b {
          Err(String::from("Semi-minor axis must be in ]0, a[").into())
        } else if pa < 0.0 || PI <= pa {
          Err(String::from("Position angle must be in [0, pi[").into())
        } else {
          let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_box(lon, lat, a, b, pa, depth);
          out.write_smoc_possibly_auto_converting_from_u64(moc.into_range_moc_iter())
        }
      },
      From::Polygon {
        depth,
        vertices_deg,
        complement,
        out
      } => {
        let vertices = vertices_deg.list.iter()
          .map(|(lon_deg, lat_deg)| {
            let lon = lon_deg2rad(*lon_deg)?;
            let lat = lat_deg2rad(*lat_deg)?;
            Ok((lon, lat))
          }).collect::<Result<Vec<(f64, f64)>, Box<dyn Error>>>()?;
        let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_polygon(&vertices, complement, depth);
        out.write_smoc_possibly_auto_converting_from_u64(moc.into_range_moc_iter())
      },
      From::Positions {
        depth,
        input,
        separator,
        out
      } => {
        fn line2coos(separator: &str, line: std::io::Result<String>) -> Result<(f64, f64), Box<dyn Error>> {
          let line = line?;
          let (lon_deg, lat_deg) = line.trim()
            .split_once(separator)
            .ok_or_else(|| String::from("split on space failed."))?;
          let lon_deg = lon_deg.parse::<f64>()?;
          let lat_deg = lat_deg.parse::<f64>()?;
          let lon = lon_deg2rad(lon_deg)?;
          let lat = lat_deg2rad(lat_deg)?;
          Ok((lon, lat))
        }
        let line2pos = move |line: std::io::Result<String>| {
          match line2coos(&separator, line) {
            Ok(lonlat) => Some(lonlat),
            Err(e) => {
              eprintln!("Error reading or parsing line: {:?}", e);
              None
            }
          }
        };
        let moc: RangeMOC<u64, Hpx<u64>> = if input == PathBuf::from(r"-") {
          let stdin = std::io::stdin();
          RangeMOC::from_coos(depth, stdin.lock().lines().filter_map(line2pos), None)
        } else {
          let f = File::open(input)?;
          let reader = BufReader::new(f);
          RangeMOC::from_coos(depth, reader.lines().filter_map(line2pos), None)
        };
        out.write_smoc_possibly_auto_converting_from_u64(moc.into_range_moc_iter())
      },
      From::Timestamp {
        depth,
        time,
        input,
        out
      } => {
        let line2ts = move |line: std::io::Result<String>| {
          match line.map_err(|e| e.into()).and_then(|s| time.parse(&s)) {
            Ok(t) => Some(t),
            Err(e) => {
              eprintln!("Error reading or parsing line: {:?}", e);
              None
            }
          }
        };
        if input == PathBuf::from(r"-") {
          let stdin = std::io::stdin();
          out.write_tmoc_possibly_auto_converting_from_u64(
            RangeMOC::<u64, Time::<u64>>::from_microsec_since_jd0(
              depth, stdin.lock().lines().filter_map(line2ts), None
            ).into_range_moc_iter()
          )
        } else {
          let f = File::open(input)?;
          let reader = BufReader::new(f);
          out.write_tmoc_possibly_auto_converting_from_u64(
            RangeMOC::<u64, Time::<u64>>::from_microsec_since_jd0(
              depth, reader.lines().filter_map(line2ts), None
            ).into_range_moc_iter()
          )
        }
      },
      From::Timerange {
        depth,
        time,
        input,
        separator,
        out
      } => {
        fn line2tr(separator: &str, time: &InputTime, line: std::io::Result<String>) -> Result<Range<u64>, Box<dyn Error>> {
          let line = line?;
          let (tmin, tmax) = line.trim()
            .split_once(&separator)
            .ok_or_else(|| String::from("split on space failed."))?;
          let tmin = time.parse(&tmin)?;
          let tmax = time.parse(&tmax)?;
          Ok(tmin..tmax)
        };
        let line2trange = move |line: std::io::Result<String>| {
          match line2tr(&separator, &time, line) {
            Ok(trange) => Some(trange),
            Err(e) => {
              eprintln!("Error reading or parsing line: {:?}", e);
              None
            }
          }
        };
        if input == PathBuf::from(r"-") {
          let stdin = std::io::stdin();
          out.write_tmoc_possibly_auto_converting_from_u64(
            RangeMOC::<u64, Time::<u64>>::from_microsec_ranges_since_jd0(
              depth, stdin.lock().lines().filter_map(line2trange), None
            ).into_range_moc_iter()
          )
        } else {
          let f = File::open(input)?;
          let reader = BufReader::new(f);
          out.write_tmoc_possibly_auto_converting_from_u64(
            RangeMOC::<u64, Time::<u64>>::from_microsec_ranges_since_jd0(
              depth, reader.lines().filter_map(line2trange), None
            ).into_range_moc_iter()
          )
        }
      }
    }
  }
}

fn lon_deg2rad(lon_deg: f64) -> Result<f64, Box<dyn Error>> {
  let lon = lon_deg.to_radians();
  if lon < 0.0 || TWICE_PI <= lon {
    Err(String::from("Longitude must be in [0, 2pi[").into())
  } else {
    Ok(lon)
  }
}

fn lat_deg2rad(lat_deg: f64) -> Result<f64, Box<dyn Error>> {
  let lat  = lat_deg.to_radians();
  if lat < -HALF_PI || HALF_PI <= lat {
    Err(String::from("Latitude must be in [-pi/2, pi/2]").into())
  } else {
    Ok(lat)
  }
}
