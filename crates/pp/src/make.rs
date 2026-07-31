use std::{
  error::Error,
  fs::{File, OpenOptions},
  io::{BufRead, BufReader, BufWriter, Cursor, Error as IoError, Seek, SeekFrom, Write, stdin},
  marker::Send,
  path::PathBuf,
  str::FromStr,
};

use clap::Parser;
use memmap::MmapMut;

use moclib::{
  deser::{ascii::from_ascii_ivoa, stcs::stcs2moc},
  moc::{
    CellOrCellRangeMOCIntoIterator, CellOrCellRangeMOCIterator, RangeMOCIntoIterator,
    RangeMOCIterator,
    range::{RangeMOC, op::convert::convert_from_u64},
  },
  qty::{Hpx, MocQty},
};

use crate::{
  RawEntry,
  extsort::ExtSortBuilder,
  rtree::{Header, LEAF_ENTRY_BYTE_SIZE, LeafEntry, RTreeLayout, SubTreeWriter},
  slice_u64_to_u8,
  time::TimeType,
};

#[derive(Debug, Parser)]
/// Make a new ST-MOC PP file.
pub struct Make {
  #[clap(short = 'i', long = "input", value_name = "FILE")]
  /// Input file containing the 'id,tmin,tmax,moc_path' list (default: read from stdin)
  /// 'id' must be a positive integer that can be stored on 4 bytes.
  input: Option<PathBuf>,
  #[clap(
    short = 'd',
    long = "delimiter",
    value_name = "CHAR",
    default_value = " "
  )]
  /// Delimiter used in the input file
  separator: char,
  #[structopt(long = "time-type", default_value = "jd")]
  /// Input time format: 'jd' (julian date), 'mjd' (modified julian date), 'usec' (microsec since JD=0),
  /// 'isorfc' (Gregorian date-time, Rfc3339, WARNING: no conversion to TCB),
  /// or 'isosimple' (Gregorian date, 'YYYY-MM-DDTHH:MM:SS' WARNING: no conversion to TCB)
  time: TimeType,
  #[structopt(long = "smoc-type", default_value = "path")]
  /// S-MOC format in the input file: 'path' (path to a FITS file), 'ascii' (inlined ASCII MOC) or 'stcs(DEPTH)' (compute from STC-S at given DEPTH).
  smoc: InputSMOC,
  #[clap(short = 'p', long = "page-size", default_value = "4")]
  /// Size of the implicit R-Tree page, in kB
  page_size_kb: u8,
  #[clap(short = 'c', long = "chunk-size", default_value = "500000")]
  /// External sort param: #rows in a chunk (a temporary file).
  extsort_chunk_size: usize,
  #[clap(short = 'k', long = "k-way", default_value = "7")]
  /// External sort param: 'k' value of the external k-way merge sort
  extsort_kway: usize,
  #[clap(short = 't', long = "tmp-dir", default_value = ".ext_sort_tmp")]
  /// External sort param: temp directory
  extsort_tmp_dir: PathBuf,
  #[clap(value_name = "FILE")]
  /// Output file, storing the ST-MOC PP.
  output: PathBuf,
}

impl Make {
  pub fn exec(self) -> Result<(), Box<dyn Error>> {
    match &self.input {
      Some(path) => File::open(path)
        .map_err(|e| e.into())
        .and_then(|file| self.exec_from_reader(BufReader::new(file))),
      None => self.exec_from_reader(BufReader::new(stdin())),
    }
  }

  pub fn exec_from_reader<R: BufRead + Send + 'static>(
    self,
    reader: R,
  ) -> Result<(), Box<dyn Error>> {
    let separator = self.separator;
    let time = self.time.clone();
    self.exec_from_iterator(reader.lines().enumerate().map(move |(i, line_res)| {
      line_res.and_then(|line| {
        RawEntry::parse(i, line.as_str(), separator, &time).map_err(IoError::other)
      })
    }))
  }

  pub fn exec_from_iterator<I>(self, it: I) -> Result<(), Box<dyn Error>>
  where
    I: Iterator<Item = Result<RawEntry, IoError>> + Send + 'static,
  {
    ExtSortBuilder::new()
      .set_chunk_size(self.extsort_chunk_size)
      .set_kway(self.extsort_kway)
      .set_tmp_dir(self.extsort_tmp_dir.clone())
      .build()
      .external_sort(it)
      .and_then(|(n_entries, sorted_it)| {
        self.exec_from_sorted_iterator(n_entries as u32, sorted_it)
      })
  }

  pub fn exec_from_sorted_iterator<I>(
    self,
    n_entries: u32,
    sorted_it: I,
  ) -> Result<(), Box<dyn Error>>
  where
    I: Iterator<Item = RawEntry>,
  {
    // Get the iterator with actual S-MOCs
    let it_with_smoc = sorted_it.map(move |raw_entry| {
      self
        .smoc
        .build_moc(&raw_entry.smoc)
        .map(|smoc| (raw_entry, smoc))
    });

    // Compute the file structure
    let rtree_layout = RTreeLayout::from(n_entries, self.page_size_kb);
    let node_first_byte = rtree_layout.first_node_byte_offset();
    let leaf_first_byte = rtree_layout.first_leaf_byte_offset();
    let mut smoc_first_byte = rtree_layout.first_smoc_byte_offset();

    // Create the file
    let file = OpenOptions::new()
      .read(true)
      .write(true)
      .create(true)
      .truncate(true)
      .open(&self.output)?;
    file.set_len(smoc_first_byte)?;
    // * create a memory map from file start to end of leaves
    let mut mmap = unsafe { MmapMut::map_mut(&file)? };
    // * create buffer to write Nodes
    let (header_bytes, rem_bytes) = mmap.split_at_mut(node_first_byte as usize);
    let (nodes_bytes, rem_bytes) =
      rem_bytes.split_at_mut((leaf_first_byte - node_first_byte) as usize);
    let (leaves_bytes, _zero) =
      rem_bytes.split_at_mut((smoc_first_byte - leaf_first_byte) as usize);
    let node_writer = rtree_layout.get_nodes_writer(nodes_bytes);
    // * create the buffer to write leaves
    let mut leaf_buff = Cursor::new(leaves_bytes);
    // * create a buffer writer to write S-MOCs
    let mut file_smoc_part = file.try_clone()?;
    file_smoc_part.seek(SeekFrom::Start(smoc_first_byte))?;
    let mut smoc_buff = BufWriter::new(file_smoc_part);

    let mut depth_max = 0;
    // Write leaves and S-MOCs for all entries
    for res in it_with_smoc {
      let (raw_entry, smoc) = res?;
      let depth = smoc.depth_max();
      if depth_max < depth {
        depth_max = depth;
      }
      if depth <= Hpx::<u32>::MAX_DEPTH {
        // We convert to save space
        let smoc: RangeMOC<u32, Hpx<u32>> =
          convert_from_u64(smoc.into_range_moc_iter()).into_range_moc();
        let moc_ranges = smoc.into_moc_ranges().into_ranges();
        let moc_bytes = moc_ranges.as_bytes();
        let leaf_bytes = LeafEntry::to_bytes(
          raw_entry.id,
          raw_entry.tmin,
          raw_entry.tmax,
          depth,
          moc_bytes.len() as u32,
          smoc_first_byte,
        );
        leaf_buff
          .write_all(slice_u64_to_u8(&leaf_bytes[..]))
          .and_then(|()| smoc_buff.write_all(&moc_bytes))?;
        smoc_first_byte += moc_bytes.len() as u64;
      } else {
        let moc_ranges = smoc.into_moc_ranges().into_ranges();
        let moc_bytes = moc_ranges.as_bytes();
        let leaf_bytes = LeafEntry::to_bytes(
          raw_entry.id,
          raw_entry.tmin,
          raw_entry.tmax,
          depth,
          moc_bytes.len() as u32,
          smoc_first_byte,
        );
        leaf_buff
          .write_all(slice_u64_to_u8(&leaf_bytes[..]))
          .and_then(|()| smoc_buff.write_all(&moc_bytes))?;
        smoc_first_byte += moc_bytes.len() as u64;
      };
    }
    smoc_buff.flush().and_then(|()| leaf_buff.flush())?;

    // Compute and write the implicit RTree structure
    let leaves_bytes = leaf_buff.into_inner();
    let mut trange_it = leaves_bytes
      .chunks(rtree_layout.page_byte_size() as usize)
      .map(|leaf_page_bytes| {
        leaf_page_bytes
          .chunks(LEAF_ENTRY_BYTE_SIZE as usize)
          .map(|leaf_bytes| {
            let leaf_entry = LeafEntry::new(leaf_bytes);
            leaf_entry.tmin()..leaf_entry.tmax()
          })
          .reduce(|mut acc, curr| {
            if curr.start < acc.start {
              acc.start = curr.start;
            }
            if curr.end > acc.end {
              acc.end = curr.end;
            }
            acc
          })
          .unwrap()
      });
    // Warning: the node_writer must flush as it writes data
    node_writer.write(&mut trange_it);
    // First flush, then write header, then re-flush!
    let mut head_buff = Cursor::new(header_bytes);
    head_buff
      .write_all(&Header::to_bytes(
        1,
        self.page_size_kb,
        depth_max,
        n_entries,
        leaf_first_byte as u32,
        smoc_first_byte,
      ))
      .and_then(|()| head_buff.flush())
      .map_err(|e| e.into())
  }
}

#[derive(Clone, Debug)]
pub enum InputSMOC {
  /// Path to a FITS file
  Path,
  /// Inline ASCII MOC
  Ascii,
  /// Inline STC-S region
  StcS {
    /// Depth of the MOC to be computed from the STC-S region
    depth: u8,
  },
}

impl InputSMOC {
  pub fn build_moc(&self, moc: &str) -> Result<RangeMOC<u64, Hpx<u64>>, Box<dyn Error>> {
    match self {
      Self::Path => RangeMOC::<u64, Hpx<u64>>::from_fits_file(moc),
      Self::Ascii => from_ascii_ivoa::<u64, Hpx<u64>>(moc)
        .map(|ccr| ccr.into_cellcellrange_moc_iter().ranges().into_range_moc())
        .map_err(|e| e.into()),
      Self::StcS { depth } => stcs2moc(*depth, None, moc).map_err(|e| e.into()),
    }
  }
}

impl FromStr for InputSMOC {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.as_bytes() {
      [b'p', b'a', b't', b'h'] => Ok(InputSMOC::Path),
      [b'a', b's', b'c', b'i', b'i'] => Ok(InputSMOC::Ascii),
      [b's', b't', b'c', b's', b'(', nbr @ .., b')'] => unsafe { str::from_utf8_unchecked(nbr) }
        .parse::<u8>()
        .map(|depth| Self::StcS { depth })
        .map_err(|e| format!("Unable to parse depth in option 'stcs(depth)': {:?}", e)),
      _ => Err(format!(
        "Unrecognized time type. Actual: '{}'. Expected: 'path', 'ascii', 'stcs(DEPTH)'",
        s
      )),
    }
  }
}
