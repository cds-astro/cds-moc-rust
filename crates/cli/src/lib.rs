
use std::str::FromStr;
use std::error::Error;

use time::PrimitiveDateTime;
use time::format_description::{self, well_known::Rfc3339};
// use chrono::prelude::*;

pub mod input;
pub mod info;
pub mod constants;
pub mod convert;
pub mod from;
pub mod op;
pub mod filter;
pub mod output;
pub mod hprint;

// See https://www.ivoa.net/rdf/timescale/2019-03-15/timescale.html

const DATE_TIME_FMT: Rfc3339 = Rfc3339;

#[derive(Debug)]
pub enum InputTime {
  /// Julian Date, in decimal degrees
  JD,
  /// Modified Julian Date, in decimal degrees
  MJD,
  /// Number of microseconds since JD=0, unsigned values
  MicroSecSinceJD0,
  /// ISO time in Gregorian, following RFC3339, i.e. YYYY-MM-DDTHH:MM:SS.SSZ+... (no conversion from UT to TCB)
  IsoRfc,
  /// ISO time in Gregorian, simple format: YYYY-MM-DDTHH:MM:SS (no conversion from UT to TCB)
  IsoSimple
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
      InputTime::IsoRfc => 
        PrimitiveDateTime::parse(value, &DATE_TIME_FMT)
          .map(|date_time| {
            let year = date_time.year() as i16;
            let month: u8 = date_time.month().into();
            let day = date_time.day();
            let (h, m, s, u) = date_time.as_hms_micro();
            let jd = gregorian2jd(year, month, day) as i64 * 86400000000_i64;
            let jday_frac = u as i64 + (hms2jday_fract(h, m, s as f64) * N_MICROSEC_IN_DAY) as i64;
            (jd + jday_frac) as u64
          }).map_err(|e| e.into()),
      InputTime::IsoSimple => {
        // value.parse::<DateTime<Utc>>()
        let format = format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]").unwrap();
        PrimitiveDateTime::parse(value, &format)
          .map(|date_time| {
            let year = date_time.year() as i16;
            let month: u8 = date_time.month().into();
            let day = date_time.day();
            let (h, m, s, u) = date_time.as_hms_micro();
            let jd = gregorian2jd(year, month, day) as i64 * 86400000000_i64;
            let jday_frac = u as i64 + (hms2jday_fract(h, m, s as f64) * N_MICROSEC_IN_DAY) as i64;
            (jd + jday_frac) as u64
          }).map_err(|e| e.into())
      }
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
      "isorfc" => Ok(InputTime::IsoRfc),
      "isosimple" => Ok(InputTime::IsoSimple),
      _ => Err(format!("Unrecognized time type. Actual: '{}'. Expected: 'jd', 'mjd', 'usec', 'isorfc', 'isosimple'", s)),
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

/// Transforms a time given in `hh:mm::ss.s` in a fraction of julian day (day of 86400 seconds).
/// We recall that Julian days start at 12h00 so that the result can be negative.
/// # Returns
/// * `jday_fract` in `[-0.5, 0.5[`.
/// # Example
/// ```rust
/// use moc_cli::{hms2jday_fract};
/// assert_eq!(hms2jday_fract(0, 0, 0.0), -0.5);
/// assert_eq!(hms2jday_fract(12, 0, 0.0), 0.0);
/// ```
pub fn hms2jday_fract(hours: u8, minutes: u8, seconds: f64) -> f64 {
  hms2day_fract(hours, minutes, seconds) - 0.5
}

/// Transforms a time given in `hh:mm::ss.s` in a fraction of SI day (i.e. day of 86400 seconds).
/// # Returns
/// * `day_fract` in `[0, 1[`.
/// # Example
/// ```rust
/// use moc_cli::{hms2day_fract};
/// assert_eq!(hms2day_fract(12, 0, 0.0), 0.5);
/// ```
pub fn hms2day_fract(hours: u8, minutes: u8, seconds: f64) -> f64 {
  assert!(hours < 24);
  assert!(minutes < 60);
  assert!((0.0..60.0).contains(&seconds)); // <=60 for leap seconds?
  hours as f64 / 24_f64 + minutes as f64 / 1440_f64 + seconds / 86400_f64
}

pub fn gregorian2jd(year: i16, month: u8, day: u8) -> i32 {
  let (j, g) = calendar2f(year, month, day);
  j as i32 - ((3 * ((g as i32 + 184) / 100)) >> 2) + 38
}

// Sub-routine common to julian and gregorian calendar conversion
// See Richards, "15.11.3 Interconverting Dates and Julian Day Numbers", algorithm 3
fn calendar2f(y: i16, m: u8, d: u8) -> (u32, i16) {
  let h = m as i16 - 2;
  let g = y + 4716 - (12 - h) / 12;
  let f = ((h + 11) % 12) as i32;
  let e = ((1461 * g as i32) >> 2) + d as i32 - 1402;
  ((e + (153 * f + 2) / 5) as u32, g as i16)
}
