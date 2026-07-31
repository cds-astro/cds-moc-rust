use std::{
  error::Error,
  io::{Error as IoError, Write},
  ops::Range,
  str::FromStr,
};

use time::{
  Date, OffsetDateTime, PrimitiveDateTime, Time,
  format_description::well_known::{Iso8601, Rfc3339},
};

const DATE_TIME_FMT: Rfc3339 = Rfc3339;

#[derive(Clone, Debug)]
pub enum TimeType {
  /// Julian Date, in decimal degrees
  JD,
  /// Modified Julian Date, in decimal degrees
  MJD,
  /// Number of microseconds since JD=0, unsigned values
  MicroSecSinceJD0,
  /// ISO time in Gregorian, following RFC3339, i.e. YYYY-MM-DDTHH:MM:SS.SSZ+... (no conversion from UT to TCB)
  IsoRfc,
  /// ISO time in Gregorian, simple format: YYYY-MM-DDTHH:MM:SS (no conversion from UT to TCB)
  IsoSimple,
}

impl TimeType {
  pub fn parse(&self, value: &str) -> Result<u64, Box<dyn Error>> {
    match self {
      Self::JD => value
        .parse::<f64>()
        .map(|v| (v * N_MICROSEC_IN_DAY_F64) as u64)
        .map_err(|e| e.into()),
      Self::MJD => value
        .parse::<f64>()
        .map(|v| (mjd2jd(v) * N_MICROSEC_IN_DAY_F64) as u64)
        .map_err(|e| e.into()),
      Self::MicroSecSinceJD0 => value.parse::<u64>().map_err(|e| e.into()),
      Self::IsoRfc => OffsetDateTime::parse(value, &DATE_TIME_FMT)
        .map(|date_time| {
          let year = date_time.year() as i16;
          let month: u8 = date_time.month().into();
          let day = date_time.day();
          let (h, m, s, u) = date_time.to_hms_micro();
          let jd = gregorian2jd(year, month, day) as i64 * 86400000000_i64;
          let jday_frac =
            u as i64 + (hms2jday_fract(h, m, s as f64) * N_MICROSEC_IN_DAY_F64) as i64;
          (jd + jday_frac) as u64
        })
        .map_err(|e| e.into()),
      Self::IsoSimple => PrimitiveDateTime::parse(value, &Iso8601::DATE_TIME)
        .map(|date_time| {
          let year = date_time.year() as i16;
          let month: u8 = date_time.month().into();
          let day = date_time.day();
          let (h, m, s, u) = date_time.as_hms_micro();
          let jd = gregorian2jd(year, month, day) as i64 * 86400000000_i64;
          let jday_frac =
            u as i64 + (hms2jday_fract(h, m, s as f64) * N_MICROSEC_IN_DAY_F64) as i64;
          (jd + jday_frac) as u64
        })
        .map_err(|e| e.into()),
    }
  }
  pub fn write_time_range<W: Write>(
    &self,
    writer: &mut W,
    microsec_since_jd0: Range<u64>,
  ) -> Result<(), IoError> {
    let Range { start, end } = microsec_since_jd0;
    match self {
      Self::JD => {
        write!(writer, ",{},{}", mus2jd(start), mus2jd(end))
      }
      Self::MJD => {
        write!(writer, ",{},{}", jd2mjd(mus2jd(start)), jd2mjd(mus2jd(end)))
      }
      Self::MicroSecSinceJD0 => write!(writer, ",{},{}", start, end),
      Self::IsoRfc => {
        let jd = mus2jd(start) as i32;
        let (h, m, s, mus) = mus2hmsmu(start % N_MICROSEC_IN_DAY_U64);
        let dstart = OffsetDateTime::new_utc(
          Date::from_julian_day(jd).unwrap(),
          Time::from_hms_micro(h, m, s, mus).unwrap(),
        );

        let jd = mus2jd(end) as i32;
        let (h, m, s, mus) = mus2hmsmu(end % N_MICROSEC_IN_DAY_U64);
        let dend = OffsetDateTime::new_utc(
          Date::from_julian_day(jd).unwrap(),
          Time::from_hms_micro(h, m, s, mus).unwrap(),
        );

        write!(
          writer,
          ",{},{}",
          dstart.format(&DATE_TIME_FMT).unwrap(),
          dend.format(&DATE_TIME_FMT).unwrap()
        )
      }
      Self::IsoSimple => {
        let jd = mus2jd(start) as i32;
        let (h, m, s, mus) = mus2hmsmu(start % N_MICROSEC_IN_DAY_U64);
        let dstart = PrimitiveDateTime::new(
          Date::from_julian_day(jd).unwrap(),
          Time::from_hms_micro(h, m, s, mus).unwrap(),
        );

        let jd = mus2jd(end) as i32;
        let (h, m, s, mus) = mus2hmsmu(end % N_MICROSEC_IN_DAY_U64);
        let dend = PrimitiveDateTime::new(
          Date::from_julian_day(jd).unwrap(),
          Time::from_hms_micro(h, m, s, mus).unwrap(),
        );

        write!(
          writer,
          ",{},{}",
          dstart.format(&Iso8601::DATE_TIME).unwrap(),
          dend.format(&Iso8601::DATE_TIME).unwrap()
        )
      }
    }
  }
}

impl FromStr for TimeType {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "jd" => Ok(TimeType::JD),
      "mjd" => Ok(TimeType::MJD),
      "usec" => Ok(TimeType::MicroSecSinceJD0),
      "isorfc" => Ok(TimeType::IsoRfc),
      "isosimple" => Ok(TimeType::IsoSimple),
      _ => Err(format!(
        "Unrecognized time type. Actual: '{}'. Expected: 'jd', 'mjd', 'usec', 'isorfc', 'isosimple'",
        s
      )),
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
const N_MICROSEC_IN_DAY_F64: f64 = 86400000000_f64;
/// Number of microseconds in a day
const N_MICROSEC_IN_DAY_U64: u64 = 86400000000_u64;

fn mus2jd(mus: u64) -> f64 {
  (mus / N_MICROSEC_IN_DAY_U64) as f64
    + ((mus % N_MICROSEC_IN_DAY_U64) as f64) / N_MICROSEC_IN_DAY_F64
}

fn mus2hmsmu(mus: u64) -> (u8, u8, u8, u32) {
  const HOURS_IN_MICROSEC: u64 = 3600_000_000_u64; // 60 * 60 * 1000000
  const MIN_IN_MICROSEC: u64 = 60_000_000_u64; // 60 * 1000000
  const SEC_IN_MICROSEC: u64 = 1_000_000_u64; // 1000000
  assert!(mus < 24 * HOURS_IN_MICROSEC);
  let mus = mus % N_MICROSEC_IN_DAY_U64;
  let h = (mus / HOURS_IN_MICROSEC) as u8;
  let mus = mus % HOURS_IN_MICROSEC;
  let m = (mus / MIN_IN_MICROSEC) as u8;
  let mus = mus % MIN_IN_MICROSEC;
  let s = (mus / SEC_IN_MICROSEC) as u8;
  let mus = (mus % SEC_IN_MICROSEC) as u32;
  (h, m, s, mus)
}

/// Converts a Modified Julian Date into a Julian Date.
/// # Input
/// * `mjd`: modified julian date = modified julian days + fraction of day
/// # Algorithm
/// > JD = MJD + 2400000.5
fn mjd2jd(mjd: f64) -> f64 {
  mjd + MJD_ORIGIN_IN_JD
}

/// Converts a Julian Date into a Modified Julian Date.
/// # Input
/// * `jd`: julian date = julian days + fraction of day
/// # Algorithm
/// > MJD = JD - 2400000.5
fn jd2mjd(jd: f64) -> f64 {
  jd - MJD_ORIGIN_IN_JD
}

/// Transforms a time given in `hh:mm::ss.s` in a fraction of julian day (day of 86400 seconds).
/// We recall that Julian days start at 12h00 so that the result can be negative.
/// # Returns
/// * `jday_fract` in `[-0.5, 0.5[`.
/// # Example
/// ```rust
/// use stmoc_pp::time::hms2jday_fract;
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
/// use stmoc_pp::time::hms2day_fract;
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
  ((e + (153 * f + 2) / 5) as u32, g)
}
