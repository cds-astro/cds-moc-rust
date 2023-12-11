use std::{
  error::Error,
  fs::{self, File, OpenOptions},
  io::{self, BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write},
  path::PathBuf,
};

use byteorder::{LittleEndian, WriteBytesExt};
use clap::Parser;
use memmap::MmapMut;

use moclib::{
  deser::fits::{MocIdxType, MocQtyType},
  moc::{
    range::{
      op::convert::{convert, convert_from_u64},
      RangeMOC,
    },
    RangeMOCIntoIterator, RangeMOCIterator,
  },
  qty::{Hpx, MocQty},
};

use crate::{append_moc, check_id, from_fits_file, MocSetFileIOHelper, StatusFlag};

#[derive(Debug, Parser)]
/// Make a new mocset
pub struct Make {
  #[clap(short = 'l', long = "moc-list", value_name = "FILE")]
  /// Input file containing the 'moc_id moc_path' list (default: read from stdin)
  /// 'moc_id' must be a positive integer smaller than 281_474_976_710_655 (can be stored on 6 bytes).
  /// Use a negative value to flag as deprecated.
  moc_list_path: Option<PathBuf>,
  #[clap(short = 'd', long = "delimiter", default_value = " ")]
  /// Delimiter used to separate the moc identifier from the moc path
  separator: char,
  #[clap(short = 'n', long = "n128", default_value = "1")]
  /// n x 128 - 1 = number of MOCs that can be stored in this moc set
  n128: u64,
  #[clap(value_name = "FILE")]
  /// Output file, storing the MOC set.
  output: PathBuf,
}

impl Make {
  fn read_moc_list(&self) -> String {
    match &self.moc_list_path {
      None => {
        let mut buffer = String::new();
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        handle
          .read_to_string(&mut buffer)
          .expect("Error reading data from stdin");
        buffer
      }
      Some(path) => {
        fs::read_to_string(path).unwrap_or_else(|_| panic!("Unable to read file {:?}", path))
      }
    }
  }

  fn parse_moc_list(&self, moc_list_content: String) -> Vec<(i64, PathBuf)> {
    moc_list_content
      .lines()
      .enumerate()
      .filter_map(|(i, line)| match line.trim().split_once(self.separator) {
        None => {
          eprintln!(
            "Line {} ignored. No delimiter '{}' found in {}.",
            i, self.separator, line
          );
          None
        }
        Some((id, path)) => match id.trim().parse::<i64>() {
          Ok(id) => Some((id, PathBuf::from(path.trim()))),
          Err(e) => {
            eprintln!(
              "Line {} ignored. Error parsing identifier '{}': {}",
              i, id, e
            );
            None
          }
        },
      })
      .collect()
  }

  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    // Prepare file struct helper
    let mocset_helper = MocSetFileIOHelper::new(self.n128);
    // Read and parse moc list
    let moc_list = self.parse_moc_list(self.read_moc_list());
    // - ensure its size is ok
    if moc_list.len() > mocset_helper.n_mocs_max() {
      return Err(
        String::from(
          "MOC list size larger that reserved size, please increase value of option '-k'.",
        )
        .into(),
      );
    }
    // - ensure there is no ID duplicates (and that all IDs are valid)
    let mut sorted_id = moc_list
      .iter()
      .map(|(id_and_flag, _)| check_id((*id_and_flag).unsigned_abs())) // take abs since negative = 'deprecated'
      .collect::<Result<Vec<u64>, _>>()?;
    sorted_id.sort_unstable();
    sorted_id.dedup();
    if sorted_id.len() != moc_list.len() {
      return Err(String::from("Non uniq MOC ID detected").into());
    }

    // Open file
    // - with `create_new`, fails if file already exists
    let file = OpenOptions::new()
      .read(true)
      .write(true)
      .create_new(true)
      .open(&self.output)?;
    // Prepare to write
    // - header
    let header_len = mocset_helper.header_byte_size() as u64;
    file.set_len(header_len)?;
    let mut header_mmap = unsafe { MmapMut::map_mut(&file)? };
    MocSetFileIOHelper::write_n128(&mut header_mmap, self.n128)?;
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
    // Read MOC files
    for (id_and_flag, path) in moc_list {
      let (flag, id) = if id_and_flag < 0 {
        (StatusFlag::Deprecated, -id_and_flag as u64)
      } else {
        (StatusFlag::Valid, id_and_flag as u64)
      };
      // Read MOC file
      // TODO/WARNING: part of code duplicated with 'append.rs' => to be clean (e.g. passing a closure)!
      from_byte = match from_fits_file(path.clone()) {
        Err(e) => {
          eprintln!("MOC id: {}; path: {:?}; ignored. Cause: {:?}", id, path, e);
          from_byte
        }
        Ok(MocIdxType::<BufReader<File>>::U16(MocQtyType::<u16, BufReader<File>>::Hpx(moc))) => {
          let moc: RangeMOC<u16, Hpx<u16>> = moc.collect();
          // We convert to u32 because of alignment of u8 slices converted to slice of range of u64.
          // Wth u32, no alignment problems since with use Range<u32>, i.e. multiples of 64 bits.
          // It is not the case with Range<u16> since they are multiple of 32 bits only.
          let moc: RangeMOC<u32, Hpx<u32>> = convert(moc.into_range_moc_iter()).into_range_moc();
          append_moc(
            flag,
            id,
            moc,
            from_byte,
            &mut meta,
            &mut index,
            &mut data_writer,
          )?
        }
        Ok(MocIdxType::<BufReader<File>>::U32(MocQtyType::<u32, BufReader<File>>::Hpx(moc))) => {
          let moc: RangeMOC<u32, Hpx<u32>> = moc.collect();
          append_moc(
            flag,
            id,
            moc,
            from_byte,
            &mut meta,
            &mut index,
            &mut data_writer,
          )?
        }
        Ok(MocIdxType::<BufReader<File>>::U64(MocQtyType::<u64, BufReader<File>>::Hpx(moc))) => {
          let moc: RangeMOC<u64, Hpx<u64>> = moc.collect();
          if moc.depth_max() <= Hpx::<u32>::MAX_DEPTH {
            // We convert to save space
            let moc: RangeMOC<u32, Hpx<u32>> =
              convert_from_u64(moc.into_range_moc_iter()).into_range_moc();
            append_moc(
              flag,
              id,
              moc,
              from_byte,
              &mut meta,
              &mut index,
              &mut data_writer,
            )?
          } else {
            append_moc(
              flag,
              id,
              moc,
              from_byte,
              &mut meta,
              &mut index,
              &mut data_writer,
            )?
          }
        }
        _ => {
          eprintln!(
            "MOC id: {}; path: {:?}; ignored. Cause: MOC type not supported.",
            id, path
          );
          from_byte
        }
      }
    }
    data_writer.flush()?;
    header_mmap.flush()?;
    Ok(())
  }
}
