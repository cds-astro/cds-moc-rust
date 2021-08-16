
use std::str::FromStr;
use std::error::Error;

pub mod input;
pub mod info;
pub mod constants;
pub mod convert;
pub mod from;
pub mod op;
pub mod filter;
pub mod output;

#[derive(Debug)]
pub enum InputTime {
  /// Julian Date, in decimal degrees
  JD,
  /// Modified Julian Date, in decimal degrees
  MJD,
  /// Number of microseconds since JD=0, unsigned values
  MicroSecSinceJD0,
}

impl InputTime {
  pub fn parse(&self, value: &str) -> Result<u64, Box<dyn Error>> {
    match self {
      InputTime::JD =>
        value.parse::<f64>()
          .map(|v| (v * N_MICROSEC_IN_DAY) as u64)
          .map_err(|e| e.into()),
      InputTime::MJD =>
        value.parse::<f64>()
          .map(|v| (mjd2jd(v) * N_MICROSEC_IN_DAY) as u64)
          .map_err(|e| e.into()),
      InputTime::MicroSecSinceJD0 =>
        value.parse::<u64>()
          .map_err(|e| e.into()),
    }
  }
}

impl FromStr for InputTime {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "jd" => Ok(InputTime::JD),
      "mjd" => Ok(InputTime::MJD),
      "usec" => Ok(InputTime::MicroSecSinceJD0),
      _ => Err(format!("Unrecognized time type. Actual: '{}'. Expected: 'jd', 'mjd' or 'usec'", s)),
    }
  }
}

/// Modified Julian Date origin (MJD=0) in Julian Date.
/// According to [wikipedia](https://en.wikipedia.org/wiki/Julian_day),
/// "the MJD has a starting point of midnight on November 17, 1858, and is computed by:"
/// > MJD = JD - 2400000.5
/// Citing [this](http://www.madore.org/~david/misc/time.html): "A calendar day starts at midnight,
///  when the modified Julian date is an integer (thus, each calendar day can be associated
/// precisely one modified Julian date".
const MJD_ORIGIN_IN_JD: f64 = 2400000.5;

/// Number of microseconds in a day
const N_MICROSEC_IN_DAY: f64 = 86400000000_f64;

/// Converts a Modified Julian Date into a Julian Date.
/// # Input
/// * `mjd`: modified julian date = modified julian days + fraction of day
/// # Algorithm
/// > JD = MJD + 2400000.5
fn mjd2jd(mjd: f64) -> f64 {
  mjd + MJD_ORIGIN_IN_JD
}
