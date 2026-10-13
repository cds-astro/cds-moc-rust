use std::{
  collections::HashMap,
  env,
  error::Error,
  io::{BufReader, Cursor, Error as IoError, ErrorKind, Read, Write, stdin, stdout},
  str::FromStr,
};

use clap::Parser;
use http::{method::Method, status::StatusCode as Status};

use moclib::moc::RangeMOCIntoIterator;

use stmoc_pp::{
  info::Info,
  list::List,
  make::Make,
  query::{Query, QueryType, load_stmoc, to_u32_possibly_degrading, write_result},
  rtree::STMocPPFile,
  time::TimeType,
};

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None, allow_negative_numbers = true)]
/// Create and query a ST-MOC precursor, i.e. a list of time (range, S-MOC) couples.
enum Args {
  #[clap(name = "make")]
  Make(Make),
  #[clap(name = "info")]
  Info(Info),
  #[clap(name = "list")]
  List(List),
  #[clap(name = "query")]
  Query(Query),
}

impl Args {
  fn exec(self) -> Result<(), Box<dyn Error>> {
    match self {
      Args::Make(make) => make.exec(),
      Args::Info(info) => info.exec(),
      Args::List(list) => list.exec(),
      Args::Query(query) => query.exec(),
    }
  }
}

fn exec_cli() -> Result<(), Box<dyn Error>> {
  env_logger::init();
  let args = Args::parse();
  match args.exec() {
    Ok(()) => Ok(()),
    Err(e) => match e.downcast::<IoError>() {
      Ok(err) if err.kind() == ErrorKind::BrokenPipe => Ok(()),
      Ok(err) => {
        eprintln!("Error: {}", &err);
        Err(err.into())
      }
      Err(err) => {
        eprintln!("Error: {}", &err);
        Err(err.into())
      }
    },
  }
}

// 2 query modes:
// * GET:  ppfile=NAME&tfrom=TIME&tto=TIME&tfmt=TIME_FMT&skyfrac_min=F64&smoc=SMOC
// * POST: ppfile=NAME&skyfrac_min POST=ST-MOC

/// Retrieve CGI arguments from the given map of environment variables.
#[cfg(feature = "cgi")]
fn exec_cgi(env_vars: HashMap<String, String>) -> Result<(), Box<dyn Error>> {
  match &env_vars["REQUEST_METHOD"].parse::<Method>() {
    Ok(Method::GET) => exec_cgi_get(env_vars).map_err(|e| {
      let error_msg = format!("Error executing a GET query: {}.", e);
      println!("Status: {}\n\n{}", Status::INTERNAL_SERVER_ERROR, error_msg);
      error_msg.into()
    }),
    Ok(Method::POST) => exec_cgi_post(env_vars).map_err(|e| {
      let error_msg = format!("Error executing a POST query: {}.", e);
      println!("Status: {}\n\n{}", Status::INTERNAL_SERVER_ERROR, error_msg);
      error_msg.into()
    }),
    Ok(method) => {
      let error_msg = format!("Method {} not supported.", method);
      println!("Status: {}\n\n{}", Status::NOT_IMPLEMENTED, error_msg);
      Err(error_msg.into())
    }
    Err(_) => {
      let error_msg = String::from("Invalid method.");
      println!("Status: {}\n\n{}", Status::BAD_REQUEST, error_msg);
      Err(error_msg.into())
    }
  }
}

/// Exec a GET query, args are:
/// * `dataset`: name of the STMOC-PP file (must be in `cgi-bin/data/NAME.stmpp.bin`
/// * `tstart`: starting time (inclusive)
/// * `tend`: endding time (exclusive)
/// * `tfmt`: format of starting and ending time (`jd`, `mjd`, `usec`, `isorfc`, `isosimple`
/// * `skyfrac_min`: sky fraction in `[0.0, 1.0]`
/// * `smoc`: ASCII S-MOC
#[cfg(feature = "cgi")]
fn exec_cgi_get(env_vars: HashMap<String, String>) -> Result<(), Box<dyn Error>> {
  let kv = query_string_2_keyval_map(&env_vars["QUERY_STRING"]);
  let time_fmt = kv
    .get("tfmt")
    .ok_or_else(|| String::from("Get parameter 'tfmt' not found!"))
    .and_then(|val| TimeType::from_str(val))?;
  let time_start: String = kv
    .get("tstart")
    .map(|s| String::from(*s))
    .ok_or_else(|| String::from("Get parameter 'tstart' not found!"))?;
  let time_end: String = kv
    .get("tend")
    .map(|s| String::from(*s))
    .ok_or_else(|| String::from("Get parameter 'tend' not found!"))?;
  let name: &str = kv
    .get("dataset")
    .ok_or_else(|| String::from("Get parameter 'dataset' not found!"))?;
  let minimal_sky_fraction: f64 = kv
    .get("skyfrac_min")
    .ok_or_else(|| String::from("Get parameter 'skyfrac_min' not found!"))
    .and_then(|val| {
      val
        .parse::<f64>()
        .map_err(|_| format!("Unable to parse skyfraction value '{}'", val))
    })?;
  let smoc_ascii: String = kv
    .get("smoc")
    .map(|s| String::from(*s))
    .ok_or_else(|| String::from("Get parameter 'smoc' not found!"))?;
  // Create query parameters
  let q = QueryType::Simple {
    time_fmt,
    time_start,
    time_end,
    smoc_ascii,
    minimal_sky_fraction: Some(minimal_sky_fraction),
  };
  // Open stmoc pp file
  if !name.chars().all(|c| c.is_ascii_alphanumeric()) {
    return Err(
      format!(
        "Dataset name '{}' contains non alphanumeric characters.",
        name
      )
      .into(),
    );
  }
  let stmpp = STMocPPFile::new(format!("stmocppdata/{}.bin", name))?;
  // Exec query
  q.exec(true, stmpp, false, false, TimeType::MJD, false)
}

/// Exec a POST query, args are:
/// * `dataset`: name of the STMOC-PP file (must be in `cgi-bin/data/NAME.stmpp.bin`
/// * `skyfrac_min`: sky fraction in `[0.0, 1.0]`
/// * then stdin must be of type ` application/fits` and contains the bytes of a ST-MOC
#[cfg(feature = "cgi")]
fn exec_cgi_post(env_vars: HashMap<String, String>) -> Result<(), Box<dyn Error>> {
  // Parse GET parameters
  let kv = query_string_2_keyval_map(&env_vars["QUERY_STRING"]);
  let name: &str = kv
    .get("dataset")
    .ok_or_else(|| String::from("Get parameter 'dataset' not found!"))?;
  let minimal_sky_fraction: f64 = kv
    .get("skyfrac_min")
    .ok_or_else(|| String::from("Get parameter 'skyfrac_min' not found!"))
    .and_then(|val| {
      val
        .parse::<f64>()
        .map_err(|_| format!("Unable to parse skyfraction value '{}'", val))
    })?;
  // Read POST file
  let content_length: usize = env::var("CONTENT_LENGTH")?.parse()?;
  // eprintln!("CONTENT_LENGTH: {}", content_length);
  let stdin = stdin();
  let mut input_bytes = Vec::new();
  stdin
    .lock()
    .take(content_length as u64)
    .read_to_end(&mut input_bytes)?;

  // Open stmoc pp file
  if !name.chars().all(|c| c.is_ascii_alphanumeric()) {
    return Err(
      format!(
        "Dataset name '{}' contains non alphanumeric characters.",
        name
      )
      .into(),
    );
  }
  let stmpp = STMocPPFile::new(format!("stmocppdata/{}.bin", name))?;
  let reader = stmpp.get_reader();
  let mut writer = stdout().lock();

  let stmoc_it = load_stmoc(BufReader::new(Cursor::new(input_bytes)))?;
  writeln!(&mut writer, "Content-Type: text/plain\n\nid")?;

  for (tmoc, smoc_u64) in stmoc_it {
    let smoc_u32 = to_u32_possibly_degrading(&smoc_u64);
    let smoc_u32_depth = smoc_u32.depth_max();
    let smoc_u32_ranges = &smoc_u32.moc_ranges().0;
    let smoc_u64_depth = smoc_u64.depth_max();
    let smoc_u64_ranges = &smoc_u64.moc_ranges().0;
    for time_range in tmoc.into_range_moc_iter() {
      write_result(
        &mut writer,
        reader.smoc_filter_with_minimum_area(
          reader.rtree_query(&time_range),
          smoc_u32_depth,
          smoc_u32_ranges,
          smoc_u64_depth,
          smoc_u64_ranges,
          minimal_sky_fraction,
        ),
        &time_range,
        smoc_u32_ranges,
        smoc_u64_ranges,
        false,
        false,
        false,
        &TimeType::MJD,
        false,
      )?;
      false;
    }
  }
  Ok(())
}

#[cfg(feature = "cgi")]
fn query_string_2_keyval_map(query_string: &str) -> HashMap<&str, &str> {
  query_string
    .split('&')
    .map(|param| match param.split_once('=') {
      None => (param, ""),
      Some((k, v)) => (k, v),
    })
    .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
  #[cfg(feature = "cgi")]
  {
    let env_vars: HashMap<String, String> = env::vars().collect();
    if env_vars.get("REQUEST_METHOD").is_none() {
      exec_cli()
    } else {
      exec_cgi(env_vars)
    }
  }
  #[cfg(not(feature = "cgi"))]
  exec_cli()
}
