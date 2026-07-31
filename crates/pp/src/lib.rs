extern crate moclib;

use std::{
  cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
  io::{Error as IoError, Read, Write},
  mem,
};

pub mod extsort;
pub mod info;
pub mod list;
pub mod make;
pub mod query;
pub mod rtree;
pub mod time;

use extsort::ExtSortEntry;
use time::TimeType;

#[derive(Debug)]
/// An entry, as read from the CSV input file, and sorted.
pub struct RawEntry {
  /// Product (image, ...) identifier
  pub id: u32,
  /// Time range lower bound (inclusive), in microsec since JD=0
  pub tmin: u64,
  /// Time range upper bound (exclusive), in microsec since JD=0
  pub tmax: u64,
  /// SMOC descriptor (can be a path, an ASCII MOC or a STC-S)
  pub smoc: String,
}
impl RawEntry {
  fn new(id: u32, tmin: u64, tmax: u64, smoc: String) -> Self {
    Self {
      id,
      tmin,
      tmax,
      smoc,
    }
  }
  /// Returns the value to be used in a comparison.
  /// We use the center of the time range (but, for comparison, no need to divide by 2).
  fn val_for_cmp(&self) -> u64 {
    self.tmin + self.tmax
  }
  /// Build an entry from a line of the input file.
  /// # Params
  /// * `iline`: index of the line in the input (to write debug message)
  /// * `line`: the content of the line
  /// * `separator`: the field separator
  /// * `time`: the object telling how to interpret the time and to cinvert it (if neeeded) in microsec since JD=0
  pub(crate) fn parse(
    i_line: usize,
    line: &str,
    separator: char,
    time: &TimeType,
  ) -> Result<Self, String> {
    let elems: Vec<&str> = line.split(separator).collect();
    if elems.len() != 4 {
      Err(format!(
        "Error at line {}: line ignored. Wrong number of elements. Expected: 4. Actual: {}. Line: {}",
        i_line,
        elems.len(),
        line
      ))
    } else {
      let id = elems[0].parse::<u32>();
      let tmin = time.parse(elems[1]);
      let tmax = time.parse(elems[2]);
      let smoc = elems[3];
      match (id, tmin, tmax) {
        (Ok(id), Ok(tmin), Ok(tmax)) => Ok(Self::new(id, tmin, tmax, String::from(smoc))),
        (Err(_), _, _) | (_, Err(_), _) | (_, _, Err(_)) => Err(format!(
          "Error at line {}: line ignored. Unable to parse id, tmin or tmax. Line: {}.",
          i_line, line
        )),
      }
    }
  }
}
impl PartialEq<Self> for RawEntry {
  fn eq(&self, other: &Self) -> bool {
    self.val_for_cmp().eq(&other.val_for_cmp())
  }
}
impl PartialOrd<Self> for RawEntry {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.val_for_cmp().partial_cmp(&other.val_for_cmp())
  }
}
impl Eq for RawEntry {}
impl Ord for RawEntry {
  fn cmp(&self, other: &Self) -> Ordering {
    self.val_for_cmp().cmp(&other.val_for_cmp())
  }
}
impl ExtSortEntry for RawEntry {
  fn write<W: Write>(self, writer: &mut W) -> Result<usize, IoError> {
    let smoc_len = self.smoc.len() as u32;
    writer
      .write_all(&self.id.to_le_bytes())
      .and_then(|()| writer.write_all(&self.tmin.to_le_bytes()))
      .and_then(|()| writer.write_all(&self.tmax.to_le_bytes()))
      .and_then(|()| writer.write_all(&smoc_len.to_le_bytes()))
      .and_then(|()| writer.write_all(&self.smoc.as_bytes()))
      .map(|()| ((size_of::<u32>() + size_of::<u64>()) << 1) + smoc_len as usize)
  }

  fn read<R: Read>(reader: &mut R) -> Result<Self, IoError> {
    let mut id_bytes = [0_u8; size_of::<u32>()];
    let mut tmin_bytes = [0_u8; size_of::<u64>()];
    let mut tmax_bytes = [0_u8; size_of::<u64>()];
    let mut smoc_len_bytes = [0_u8; size_of::<u32>()];
    reader
      .read_exact(&mut id_bytes)
      .and_then(|()| reader.read_exact(&mut tmin_bytes))
      .and_then(|()| reader.read_exact(&mut tmax_bytes))
      .and_then(|()| reader.read_exact(&mut smoc_len_bytes))
      .and_then(|()| {
        let smoc_len = u32::from_le_bytes(smoc_len_bytes) as usize;
        let mut smoc_bytes = vec![0_u8; smoc_len];
        reader.read_exact(&mut smoc_bytes).map(|()| {
          Self::new(
            u32::from_le_bytes(id_bytes),
            u64::from_le_bytes(tmin_bytes),
            u64::from_le_bytes(tmax_bytes),
            unsafe { String::from_utf8_unchecked(smoc_bytes) },
          )
        })
      })
  }
}

pub(crate) fn slice_u64_to_u8(data: &[u64]) -> &[u8] {
  let offset = data.as_ptr().align_offset(mem::align_of::<u8>());
  // I suppose that align with u8 is never a problem.
  assert_eq!(offset, 0);
  unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * size_of::<u64>()) }
}

pub(crate) fn slice_u8_to_u64data(byte: &[u8]) -> &[u64] {
  let offset = byte.as_ptr().align_offset(mem::align_of::<u64>());
  // I suppose that align with u64 for u8 shold not be a problem (we use 64 everywhere so...)
  assert_eq!(offset, 0);
  unsafe { std::slice::from_raw_parts(byte.as_ptr() as *const u64, byte.len() / size_of::<u64>()) }
}
