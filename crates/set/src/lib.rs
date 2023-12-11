use std::{
  collections::HashMap,
  error::Error,
  fs::{self, File, OpenOptions},
  io::{self, BufReader, BufWriter, Cursor, Seek, SeekFrom, Write},
  mem::{align_of, size_of},
  ops::Range,
  path::PathBuf,
  ptr::slice_from_raw_parts,
  slice,
  str::FromStr,
};

use memmap::{Mmap, MmapMut, MmapOptions};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use moclib::{
  deser::fits::{from_fits_ivoa, MocIdxType},
  idx::Idx,
  moc::range::RangeMOC,
  qty::{Hpx, MocQty},
  ranges::BorrowedRanges,
};

pub mod append;
pub mod chgstatus;
pub mod extract;
pub mod list;
pub mod mk;
pub mod purge;
pub mod query;
pub mod union;

/// Size of one element of the meta array.
/// Each element is made of:
/// * 1 byte  storing the flags
/// * 2 byte  storing the MOC depth
/// * 6 bytes storing the identifer
/// /// ```rust
/// use moc_set::{META_ELEM_BYTE_SIZE,};
/// assert_eq!(8, META_ELEM_BYTE_SIZE_SHIFT);
/// ```
pub const META_ELEM_BYTE_SIZE: usize = size_of::<u64>();

/// Shift used to multiply a quantity by `META_ELEM_BYTE_SIZE`.
/// ```rust
/// use moc_set::{META_ELEM_BYTE_SIZE, META_ELEM_BYTE_SIZE_SHIFT};
/// assert_eq!(1_usize << META_ELEM_BYTE_SIZE_SHIFT, META_ELEM_BYTE_SIZE);
/// assert_eq!(9_usize << META_ELEM_BYTE_SIZE_SHIFT, 9_usize * META_ELEM_BYTE_SIZE);
/// ```
pub const META_ELEM_BYTE_SIZE_SHIFT: usize = 3;

/// * u64 => 8 bytes
pub const INDEX_ELEM_BYTE_SIZE: usize = size_of::<u64>();

/// Shift used to multiply a quantity by `INDEX_ELEM_BYTE_SIZE`.
/// ```rust
/// use moc_set::{INDEX_ELEM_BYTE_SIZE, INDEX_ELEM_BYTE_SIZE_SHIFT};
/// assert_eq!(1_usize << INDEX_ELEM_BYTE_SIZE_SHIFT, INDEX_ELEM_BYTE_SIZE);
/// assert_eq!(9_usize << INDEX_ELEM_BYTE_SIZE_SHIFT, 9_usize * INDEX_ELEM_BYTE_SIZE);
/// ```
pub const INDEX_ELEM_BYTE_SIZE_SHIFT: usize = 3;

/// Mask used to retrieve the identifier of a MOC on the u64 storing metadata
pub const ID_MASK: u64 = 0x0000FFFFFFFFFFFF;

pub fn check_id(id: u64) -> Result<u64, String> {
  if id > ID_MASK {
    Err(format!(
      "ID is too large. Max expected: {}; Actual: {}",
      ID_MASK, id
    ))
  } else {
    Ok(id)
  }
}

const VOID: &str = "void";
const REMOVED: &str = "removed";
const DEPRECATED: &str = "deprecated";
const VALID: &str = "valid";

/// Status Flag, stored on a u8, but using only 2 bits.
#[repr(u8)]
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Clone, Copy)]
pub enum StatusFlag {
  Void = 0b00,       // 0
  Removed = 0b01,    // 1
  Deprecated = 0b10, // 2
  Valid = 0b11,      // 3
}

impl FromStr for StatusFlag {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      VOID => Ok(StatusFlag::Void),
      REMOVED => Ok(StatusFlag::Removed),
      DEPRECATED => Ok(StatusFlag::Deprecated),
      VALID => Ok(StatusFlag::Valid),
      // Hide 'void'.
      _ => Err(format!(
        "Status string not valid: Actual: {}. Expected: {}, {} or {}",
        s, REMOVED, DEPRECATED, VALID
      )),
    }
  }
}

impl StatusFlag {
  pub fn str_value(&self) -> &str {
    match self {
      StatusFlag::Void => VOID,
      StatusFlag::Removed => REMOVED,
      StatusFlag::Deprecated => DEPRECATED,
      StatusFlag::Valid => VALID,
    }
  }
}

/// Utility struct to have methods to compose from/decompose to the flag, depth and id elements.
///
/// ```rust
/// use moc_set::{META_ELEM_BYTE_SIZE, FlagDepthId};
///
/// assert_eq!(std::mem::size_of::<FlagDepthId>(), META_ELEM_BYTE_SIZE);
/// ```
#[derive(Copy, Clone)]
pub struct FlagDepthId(u64);

impl FlagDepthId {
  fn new(flag: StatusFlag, depth: u8, identifier: u64) -> Self {
    FlagDepthId(((flag as u64) << 56) | ((depth as u64) << 48) | (identifier & ID_MASK))
  }

  fn from_raw(raw_val: u64) -> Self {
    Self(raw_val)
  }

  fn raw_value(&self) -> u64 {
    self.0
  }

  fn status(&self) -> StatusFlag {
    let val = (self.0 >> 56) as u8 & 0b11;
    match val {
      0b00 => StatusFlag::Void,
      0b01 => StatusFlag::Removed,
      0b10 => StatusFlag::Deprecated,
      0b11 => StatusFlag::Valid,
      _ => unreachable!(), // since & 0b11;
    }
  }

  fn depth(&self) -> u8 {
    (self.0 >> 48) as u8
  }

  fn identifier(&self) -> u64 {
    self.0 & ID_MASK
  }
}

/// Represents the Metadata part of the MOC set
pub struct Metadata<'a>(&'a [FlagDepthId]);

impl<'a> IntoIterator for &Metadata<'a> {
  type Item = FlagDepthId;
  type IntoIter = MetadataIter<'a>;

  fn into_iter(self) -> Self::IntoIter {
    MetadataIter(self.0.iter())
  }
}

pub struct MetadataIter<'a>(slice::Iter<'a, FlagDepthId>);

/// End the iteration at first Void encountered.
impl<'a> Iterator for MetadataIter<'a> {
  type Item = FlagDepthId;

  fn next(&mut self) -> Option<Self::Item> {
    match self.0.next() {
      Some(FlagDepthId(0)) | None => None, // should check e.status == StatusFlag::Void instead
      Some(e) => Some(*e),
    }
  }
}

///
/// `[(n_mocs + 1) / 1024, moc_0, moc_1, ..., moc_n]`
/// `[0, n_bytes_0, n_bytes_0 + n_bytes_1, ..., sum]`
pub struct CumulByteSize<'a>(&'a [u64]);

impl<'a> IntoIterator for &CumulByteSize<'a> {
  type Item = Range<usize>;
  type IntoIter = MOCByteRangeIter<'a>;

  fn into_iter(self) -> Self::IntoIter {
    let mut it = self.0.iter();
    let start = it.next().map(|v| *v as usize).unwrap_or(0);
    MOCByteRangeIter { start, it }
  }
}

pub struct MOCByteRangeIter<'a> {
  start: usize,
  it: slice::Iter<'a, u64>,
}

/// End the iteration at first Void encountered.
impl<'a> Iterator for MOCByteRangeIter<'a> {
  type Item = Range<usize>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.it.next() {
      Some(end) => {
        let end = *end as usize;
        let range = self.start..end;
        self.start = end;
        Some(range)
      }
      None => None,
    }
  }
}

// impl Iterator<Item=Range<u64> for CumulByteSize
// To be use with zip on MetadataIter to stop :)

/// The oder in which data is read is important:
/// * first: `n_moc`, it is a constant, so ok
/// * second: `meta`, because it is the last part modified by a write operation
/// * third: `index`, it is updated before `meta` and inforamtion are appended only
///    so if `meta` not yet updated, we are not going to read the added informations
pub struct MocSetFileReader {
  helper: MocSetFileIOHelper,
  mmap: Mmap,
}

impl MocSetFileReader {
  pub fn new(path: PathBuf) -> Result<Self, io::Error> {
    let file = File::open(path)?;
    let helper = MocSetFileIOHelper::from_file(&file)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    Ok(MocSetFileReader { helper, mmap })
  }

  pub fn n128(&self) -> u64 {
    self.helper.n128()
  }

  pub fn meta(&self) -> Metadata {
    let blob = &self.mmap[self.helper.meta_bytes()];
    let len = self.helper.n_mocs_max();
    assert_eq!(blob.len(), len << META_ELEM_BYTE_SIZE_SHIFT);
    // ######################################
    // # Security check before using unsafe #
    let offset = blob.as_ptr().align_offset(align_of::<FlagDepthId>());
    if offset != 0 {
      panic!("Wrong metadata alignment!");
    }
    // ######################################
    Metadata(unsafe { &*slice_from_raw_parts(blob.as_ptr() as *const FlagDepthId, len) })
  }

  pub fn index(&self) -> CumulByteSize {
    let blob = &self.mmap[self.helper.index_bytes()];
    let len = self.helper.n_mocs_max_plus_one();
    assert_eq!(blob.len(), len << INDEX_ELEM_BYTE_SIZE_SHIFT);
    // ######################################
    // # Security check before using unsafe #
    let offset = blob.as_ptr().align_offset(align_of::<u64>());
    if offset != 0 {
      panic!("Wrong metadata alignment!");
    }
    // ######################################
    CumulByteSize(unsafe { &*slice_from_raw_parts(blob.as_ptr() as *const u64, len) })
  }

  /// WARNING: calling this method is unsafe.
  /// You have to be sure of the type `T` (either `u32` or `u64`)
  /// according to the MOC depth!
  pub fn ranges<T: Idx>(&self, bytes: Range<usize>) -> BorrowedRanges<'_, T> {
    BorrowedRanges::from(&self.mmap[bytes])
  }
}

pub struct MocSetFileWriter {
  helper: MocSetFileIOHelper,
  file: File,
  lock_path: PathBuf,
  mmap: MmapMut,
}

impl MocSetFileWriter {
  pub fn new(path: PathBuf) -> Result<Self, io::Error> {
    let mut lock_path = path.clone();
    assert!(lock_path.set_extension(
      lock_path
        .extension()
        .map(|e| format!("{:?}.lock", e))
        .unwrap_or_else(|| String::from(".lock"))
    ));
    // Atomic operation: fails if the file is already created!
    // Create the lock file
    OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(&lock_path)?;
    let file = OpenOptions::new().read(true).write(true).open(&path)?;
    let helper = MocSetFileIOHelper::from_file(&file)?;
    let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
    Ok(MocSetFileWriter {
      helper,
      file,
      lock_path,
      mmap,
    })
  }

  pub fn remap(&mut self) -> Result<(), io::Error> {
    self.mmap = unsafe { MmapOptions::new().map_mut(&self.file)? };
    Ok(())
  }

  pub fn n128(&self) -> u64 {
    self.helper.n128()
  }

  pub fn chg_status(&mut self, target_id: u64, new_status: StatusFlag) -> io::Result<()> {
    assert!(
      new_status > StatusFlag::Void,
      "Status 'void' can't be set manually."
    );
    let mut cursor = Cursor::new(&mut self.mmap[self.helper.meta_bytes()]);
    loop {
      let flg_depth_id = FlagDepthId::from_raw(cursor.read_u64::<LittleEndian>()?);
      let status = flg_depth_id.status();
      if status == StatusFlag::Void {
        eprintln!("WARNING: MOC ID {} to be updated not found.", target_id);
        return Ok(());
      }
      debug_assert!(StatusFlag::Void < StatusFlag::Removed);
      debug_assert!(StatusFlag::Removed < StatusFlag::Deprecated);
      debug_assert!(StatusFlag::Deprecated < StatusFlag::Valid);
      // Once removed, we act as if the MOC were physically removed, so we ignore IDs flagged
      // as removed
      if status > StatusFlag::Removed {
        assert!(status == StatusFlag::Deprecated || status == StatusFlag::Valid);
        let id = flg_depth_id.identifier();
        if id == target_id {
          if status != new_status {
            let depth = flg_depth_id.depth();
            let new_meta_elem = FlagDepthId::new(new_status, depth, id);
            debug_assert_eq!(size_of::<FlagDepthId>(), size_of::<u64>());
            cursor.set_position(cursor.position() - size_of::<u64>() as u64);
            cursor.write_u64::<LittleEndian>(new_meta_elem.raw_value())?;
          } // else nothing to do
          self.flush_meta()?;
          return Ok(());
        }
      }
    }
  }

  pub fn chg_multi_status(&mut self, mut target: HashMap<u64, StatusFlag>) -> io::Result<()> {
    let mut cursor = Cursor::new(&mut self.mmap[self.helper.meta_bytes()]);
    loop {
      let flg_depth_id = FlagDepthId::from_raw(cursor.read_u64::<LittleEndian>()?);
      let status = flg_depth_id.status();
      if status == StatusFlag::Void {
        break;
      }
      debug_assert!(StatusFlag::Void < StatusFlag::Removed);
      debug_assert!(StatusFlag::Removed < StatusFlag::Deprecated);
      debug_assert!(StatusFlag::Deprecated < StatusFlag::Valid);
      // Once removed, we act as if the MOC were physically removed, so we ignore IDs flagged
      // as removed
      if status > StatusFlag::Removed {
        assert!(status == StatusFlag::Deprecated || status == StatusFlag::Valid);
        let id = flg_depth_id.identifier();
        if let Some(new_status) = target.get(&id) {
          assert!(
            *new_status > StatusFlag::Void,
            "Status 'void' can't be set manually."
          );
          if status != *new_status {
            let depth = flg_depth_id.depth();
            let new_meta_elem = FlagDepthId::new(*new_status, depth, id);
            debug_assert_eq!(size_of::<FlagDepthId>(), size_of::<u64>());
            cursor.set_position(cursor.position() - size_of::<u64>() as u64);
            cursor.write_u64::<LittleEndian>(new_meta_elem.raw_value())?;
          } // else nothing to do
          target.remove(&id);
        }
      }
    }
    for remaining_id in target.into_keys() {
      eprintln!("WARNING: MOC ID {} to be updated not found.", remaining_id);
    }
    self.flush_meta()?;
    Ok(())
  }

  pub fn append_moc<T: Idx>(
    &mut self,
    flag: StatusFlag,
    id: u64,
    moc: RangeMOC<T, Hpx<T>>,
  ) -> Result<(), Box<dyn Error>> {
    let file_len = self.mmap.len();
    let (header, _) = self.mmap.split_at_mut(self.helper.header_byte_size());
    let (meta, index) = header.split_at_mut(self.helper.index_first_byte_inclusive());
    let (_, meta) = meta.split_at_mut(MocSetFileIOHelper::meta_first_byte_inclusive());
    let mut index = Cursor::new(index);
    let mut meta = Cursor::new(meta);
    let n_mocs_max = self.helper.n_mocs_max();
    let mut n_mocs = 0;
    // Find the first void entry
    while n_mocs < n_mocs_max {
      let raw_flg_depth_id = meta.read_u64::<LittleEndian>()?;
      let flg_depth_id = FlagDepthId::from_raw(raw_flg_depth_id);
      debug_assert!(StatusFlag::Void < StatusFlag::Removed);
      debug_assert!(StatusFlag::Removed < StatusFlag::Deprecated);
      debug_assert!(StatusFlag::Deprecated < StatusFlag::Valid);
      if id == flg_depth_id.identifier() && flg_depth_id.status() > StatusFlag::Removed {
        return Err(format!("MOC with id {} already in the mocset file!", id).into());
      }
      let curr_index_from = index.read_u64::<LittleEndian>()?;
      if raw_flg_depth_id == 0 {
        meta.set_position(meta.position() - size_of::<u64>() as u64);
        debug_assert_eq!(curr_index_from, file_len as u64); // Except if previous error while writing
        let mut file_data = self.file.try_clone()?;
        file_data.seek(SeekFrom::Start(curr_index_from))?;
        let mut data_writer = BufWriter::new(file_data);
        let _new_size = append_moc(
          flag,
          id,
          moc,
          curr_index_from,
          &mut meta,
          &mut index,
          &mut data_writer,
        )?;
        // flush order is important to ensure data is written before index and meta at the end
        data_writer.flush()?; // flush new data
        self.flush_index()?;
        self.flush_meta()?;
        return Ok(());
      }
      n_mocs += 1;
    }
    Err(String::from("No more space available in the mocset file!").into())
  }

  pub fn flush_meta(&self) -> io::Result<()> {
    let Range { start, end } = self.helper.meta_bytes();
    self.mmap.flush_range(start, end - start)
  }
  pub fn flush_index(&self) -> io::Result<()> {
    let Range { start, end } = self.helper.index_bytes();
    self.mmap.flush_range(start, end - start)
  }

  pub fn release(self) -> io::Result<()> {
    fs::remove_file(self.lock_path)
  }
}

pub(crate) fn append_moc<T: Idx>(
  flag: StatusFlag,
  id: u64,
  moc: RangeMOC<T, Hpx<T>>,
  from_byte: u64,
  meta: &mut Cursor<&mut [u8]>,
  index: &mut Cursor<&mut [u8]>,
  data: &mut BufWriter<File>,
) -> Result<u64, io::Error> {
  let depth = moc.depth_max();
  assert!(depth <= Hpx::<T>::MAX_DEPTH);
  let moc_ranges = moc.into_moc_ranges().into_ranges();
  let moc_bytes = moc_ranges.as_bytes();
  append_moc_bytes(flag, id, depth, moc_bytes, from_byte, meta, index, data)
}

pub(crate) fn append_moc_bytes(
  flag: StatusFlag,
  id: u64,
  depth: u8,
  moc_bytes: &[u8],
  mut from_byte: u64,
  meta: &mut Cursor<&mut [u8]>,
  index: &mut Cursor<&mut [u8]>,
  data: &mut BufWriter<File>,
) -> Result<u64, io::Error> {
  let flag_depth_id = FlagDepthId::new(flag, depth, id);
  from_byte += moc_bytes.len() as u64;
  data.write_all(moc_bytes)?;
  index.write_u64::<LittleEndian>(from_byte)?;
  meta.write_u64::<LittleEndian>(flag_depth_id.raw_value())?;
  Ok(from_byte)
}

pub struct MocSetFileIOHelper {
  n128: u64,
}

impl MocSetFileIOHelper {
  pub fn from_file(file: &File) -> Result<Self, io::Error> {
    let mmap = unsafe {
      MmapOptions::new()
        .len(MocSetFileIOHelper::n128_byte_size())
        .map(file)?
    };
    let n128 = MocSetFileIOHelper::read_n128(&mmap)?;
    Ok(MocSetFileIOHelper::new(n128))
  }

  pub fn new(n128: u64) -> MocSetFileIOHelper {
    MocSetFileIOHelper { n128 }
  }

  pub fn n128(&self) -> u64 {
    self.n128
  }

  pub fn n_mocs_max_plus_one(&self) -> usize {
    debug_assert_eq!((self.n128 as usize) << 7, (self.n128 as usize) * 128);
    (self.n128 as usize) << 7
  }

  pub fn n_mocs_max(&self) -> usize {
    self.n_mocs_max_plus_one() - 1
  }

  pub fn n_index_elements(&self) -> usize {
    self.n_mocs_max_plus_one()
  }

  /// Size in bytes of the number of 128 mocs
  /// # Remark
  ///   Same size as a meta elements so that `(n128 + meta)` is an integer value of kilobytes
  pub fn n128_byte_size() -> usize {
    META_ELEM_BYTE_SIZE
  }

  /// Size in bytes of the Metadata part
  pub fn meta_byte_size(&self) -> usize {
    let res = ((self.n128 as usize) << 10) - META_ELEM_BYTE_SIZE;
    debug_assert_eq!(res, self.n_mocs_max() * META_ELEM_BYTE_SIZE);
    res
  }

  /// Size in bytes of the Index part
  pub fn index_byte_size(&self) -> usize {
    let res = (self.n128 as usize) << 10;
    debug_assert_eq!(res, self.n_index_elements() * META_ELEM_BYTE_SIZE);
    res
  }

  /// Size in bytes of the header, i.e. the meta part plus the index part.
  pub fn header_byte_size(&self) -> usize {
    let res = (self.n128 as usize) << 11;
    debug_assert_eq!(
      res,
      MocSetFileIOHelper::n128_byte_size() + self.meta_byte_size() + self.index_byte_size()
    );
    res
  }

  pub fn write_n128(mmap: &mut MmapMut, k: u64) -> io::Result<()> {
    let byte_range = 0..MocSetFileIOHelper::meta_first_byte_inclusive();
    (&mut mmap[byte_range]).write_u64::<LittleEndian>(k)
  }
  pub fn read_n128(mmap: &Mmap) -> io::Result<u64> {
    let byte_range = 0..MocSetFileIOHelper::meta_first_byte_inclusive();
    (&mmap[byte_range]).read_u64::<LittleEndian>()
  }

  pub fn write_meta(&self, mmap: &mut MmapMut, meta: Vec<u8>) -> io::Result<()> {
    let byte_range =
      MocSetFileIOHelper::meta_first_byte_inclusive()..self.meta_last_byte_exclusive();
    (&mut mmap[byte_range]).write_all(&meta)
  }

  pub fn meta_first_byte_inclusive() -> usize {
    MocSetFileIOHelper::n128_byte_size()
  }

  pub fn meta_last_byte_exclusive(&self) -> usize {
    let res = (self.n128 as usize) << 10;
    debug_assert_eq!(
      res,
      MocSetFileIOHelper::n128_byte_size() + self.meta_byte_size()
    );
    res
  }

  pub fn index_first_byte_inclusive(&self) -> usize {
    self.meta_last_byte_exclusive()
  }

  pub fn index_last_byte_exclusive(&self) -> usize {
    self.header_byte_size()
  }

  pub fn meta_bytes(&self) -> Range<usize> {
    MocSetFileIOHelper::meta_first_byte_inclusive()..self.meta_last_byte_exclusive()
  }

  pub fn index_bytes(&self) -> Range<usize> {
    self.index_first_byte_inclusive()..self.index_last_byte_exclusive()
  }
}

pub(crate) fn from_fits_file(path: PathBuf) -> Result<MocIdxType<BufReader<File>>, Box<dyn Error>> {
  let file = File::open(&path)?;
  let reader = BufReader::new(file);
  from_fits_ivoa(reader).map_err(|e| e.into())
}
