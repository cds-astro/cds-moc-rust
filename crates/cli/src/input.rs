use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::{self, FromStr};

use moclib::deser::fits::{from_fits_ivoa, MocIdxType};

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
      _ => Err(format!(
        "Unrecognized format '{}'. Expected: 'fits' or 'stream'",
        s
      )),
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
      "ascii" => Ok(Self::Ascii),
      "json" => Ok(Self::Json),
      "fits" => Ok(Self::Fits),
      "stream" => Ok(Self::Stream),
      _ => Err(format!(
        "Unrecognized format '{}'. Expected: 'ascii, 'json', 'fits' or 'stream'",
        s
      )),
    }
  }
}
impl InputFormat {
  /// Guess the file format from the given path extension.
  pub fn from_extension(path: &Path) -> Result<Self, String> {
    match path.extension().and_then(|e| e.to_str()) {
      Some("fits") => Ok(Self::Fits),
      Some("json") => Ok(Self::Json),
      Some("ascii") | Some("txt") => Ok(Self::Ascii),
      Some("stream") => Ok(Self::Stream),
      _ => Err(String::from(
        "Unable to guess the MOC format from the file extension, see options.",
      )),
    }
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
  let file = File::open(path)?;
  let reader = BufReader::new(file);
  from_fits_ivoa(reader).map_err(|e| e.into())
}
