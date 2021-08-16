
use std::fs::File;
use std::io::BufReader;
use std::str::{self, FromStr};
use std::path::PathBuf;
use std::error::Error;

use structopt::StructOpt;

use moclib::deser::fits::{MocIdxType, from_fits_ivoa};

#[derive(Debug)]
pub enum ReducedInputFormat {
  Fits,
  Stream,
}
impl FromStr for ReducedInputFormat {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "fits" => Ok(ReducedInputFormat::Fits),
      "stream" => Ok(ReducedInputFormat::Stream),
      _ => Err(format!("Unrecognized format '{}'. Expected: 'fits' or 'stream'", s)),
    }
  }
}

#[derive(Debug)]
pub enum InputFormat {
  Ascii,
  Json,
  Fits,
  Stream,
}
impl FromStr for InputFormat {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "ascii" => Ok(InputFormat::Ascii),
      "json" => Ok(InputFormat::Json),
      "fits" => Ok(InputFormat::Fits),
      "stream" => Ok(InputFormat::Stream),
      _ => Err(format!("Unrecognized format '{}'. Expected: 'ascii, 'json', 'fits' or 'stream'", s)),
    }
  }
}

/// Guess the file format from the extension.
pub fn fmt_from_extension(path: &PathBuf) -> Result<InputFormat, String> {
  match path.extension().and_then(|e| e.to_str()) {
    Some("fits") => Ok(InputFormat::Fits),
    Some("json") => Ok(InputFormat::Json),
    Some("ascii") | Some("txt") => Ok(InputFormat::Ascii),
    Some("stream") => Ok(InputFormat::Stream),
    _ => Err(String::from("Unable to guess the MOC format from the file extension, see options.")),
  }
}

#[derive(Debug)]
pub enum DataType {
  // U8,
  U16,
  U32,
  U64,
  // U128
}
impl FromStr for DataType {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "u16" | "short" => Ok(DataType::U16),
      "u32" | "int" => Ok(DataType::U32),
      "u64" | "long" => Ok(DataType::U64),
      _ => Err(format!("Unrecognized moc type. Actual: '{}'. Expected: 'u16' (or 'short'), 'u32' (or 'int') or 'u64' (or 'long')", s)),
    }
  }
}
impl DataType {
  pub fn type_from_loaded_fits(input: &MocIdxType<BufReader<File>>) -> Self {
    match input {
      MocIdxType::U16(_) => DataType::U16,
      MocIdxType::U32(_) => DataType::U32,
      MocIdxType::U64(_) => DataType::U64,
    }
  }
}

pub fn from_fits_file(path: PathBuf) -> Result<MocIdxType<BufReader<File>>, Box<dyn Error>> {
  let file = File::open(&path)?;
  let reader = BufReader::new(file);
  from_fits_ivoa(reader).map_err(|e| e.into())
}
