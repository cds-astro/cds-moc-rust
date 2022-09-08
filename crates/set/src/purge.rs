
use std::{
  io::{
    Cursor,
    Seek, SeekFrom,
    Write, BufWriter
  },
  fs::{self, OpenOptions},
  path::PathBuf,
  error::Error
};

use clap::Parser;
use byteorder::{LittleEndian, WriteBytesExt};
use memmap::MmapMut;

use moclib::qty::{MocQty, Hpx};

use crate::{
  append_moc_bytes, StatusFlag, MocSetFileReader, MocSetFileWriter, MocSetFileIOHelper
};

#[derive(Debug, Parser)]
/// Purge the mocset removing physically the MOCs flagged as 'removed'
pub struct Purge {
  #[clap(parse(from_os_str))]
  /// The moc-set file to be purge.
  file: PathBuf,
  #[clap(short = 'n', long = "n128")]
  /// n x 128 - 1 = number of MOCs that can be stored in this purged moc set, 
  /// if larger than the current value. 
  n128: Option<u64>,
}

impl Purge {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    // read and write in a new file
    // get an iterator of (meta, data), perform operation similar to mk.rs
    // - write lock
    // - copy in a temporary file
    // - mv files
    // - remove old file
    // - remove lock
    
    // Acquire a write lock
    let moc_set_writer = MocSetFileWriter::new(self.file.clone())?;
    
    // Create and acquire a temp file
    let mut tmp_file = self.file.clone();
    assert!(
      tmp_file.set_extension(
        tmp_file.extension().map(|e| format!("{:?}.tmp", e)).unwrap_or_else(|| String::from(".tmp"))
      )
    );
    // TODO/WARNING: part of code duplicated from 'mk.rs' and 'append.rs' => to be clean (e.g. passing a closure)!
    // Prepare file struct helper
    let n128 = self.n128.unwrap_or(1).max(moc_set_writer.n128());
    let mocset_helper = MocSetFileIOHelper::new(n128);
    // Open file
    // - with `create_new`, fails if file already exists
    let file = OpenOptions::new().read(true).write(true).create_new(true).open(&tmp_file)?;
    // Prepare to write
    // - header
    let header_len = mocset_helper.header_byte_size() as u64;
    file.set_len(header_len)?;
    let mut header_mmap = unsafe { MmapMut::map_mut(&file)? };
    MocSetFileIOHelper::write_n128(&mut header_mmap, n128)?;
    let (meta, index) = header_mmap.split_at_mut(mocset_helper.index_first_byte_inclusive());
    let (_, meta) = meta.split_at_mut(MocSetFileIOHelper::meta_first_byte_inclusive());
    let mut from_byte = header_len;
    let mut index = Cursor::new(index);
    let mut meta = Cursor::new(meta);
    index.write_u64::<LittleEndian>(from_byte)?; // Starting byte of the data part
    // - data part
    let mut file_data = file.try_clone()?;
    file_data.seek(SeekFrom::Start(from_byte))?;
    let mut data_writer = BufWriter::new(file_data);
    // TODO/WARNING: part of the code duplicated from 'list.rs' => to be clean (e.g. passing a closure)!
    let moc_set_reader = MocSetFileReader::new(self.file.clone())?;
    let meta_it = moc_set_reader.meta().into_iter();
    let bytes_it = moc_set_reader.index().into_iter();
    for (flg_depth_id, byte_range) in meta_it.zip(bytes_it) {
      let id = flg_depth_id.identifier();
      let status = flg_depth_id.status();
      let depth = flg_depth_id.depth();
      debug_assert!(StatusFlag::Void < StatusFlag::Removed);
      debug_assert!(StatusFlag::Removed < StatusFlag::Deprecated);
      debug_assert!(StatusFlag::Deprecated < StatusFlag::Valid);
      if status > StatusFlag::Removed {
        if depth <= Hpx::<u32>::MAX_DEPTH {
          let ranges = moc_set_reader.ranges::<u32>(byte_range);
          from_byte = append_moc_bytes(status, id, depth, ranges.as_bytes(), from_byte, &mut meta, &mut index, &mut data_writer)?;
        } else {
          let ranges = moc_set_reader.ranges::<u64>(byte_range);
          from_byte = append_moc_bytes(status, id, depth, ranges.as_bytes(), from_byte, &mut meta, &mut index, &mut data_writer)?;
        }
      }
    }
    data_writer.flush()?;
    header_mmap.flush()?;

    // Move the new file into the old file. This is an atomic operation so if it succeed
    // then we are sure that everything is ok.
    fs::rename(tmp_file, self.file)?;
    
    // Release the write lock
    moc_set_writer.release()?;
    Ok(())
  }
}
