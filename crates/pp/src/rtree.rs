//! This module contains the code of the persistent data structure, containing the
//! 1-dimensional RTree.

use std::{
  fs::File,
  io::{Cursor, Error as IoError, Write},
  mem::size_of,
  ops::{Deref, Range},
  path::Path,
};

use memmap::{Mmap, MmapOptions};

use crate::{slice_u8_to_u64data, time::TimeType};

use moclib::{
  moc::range::RangeMOC,
  qty::{Hpx, MocQty},
  ranges::{BorrowedRanges, Ranges, SNORanges},
};

/// Number of bytes in the header of the file.
pub const HEADER_BYTE_SIZE: u64 = 32;

/// Starting bytes of teh file.
pub const MAGIC_BYTES: &'static [u8; 13] = b"ST-MOC PP Ver";

/// Size, in byte, of en entry in a node of the (implicit) R-Tree.
/// See the `NodeEntry` structure.
pub const NODE_ENTRY_BYTE_SIZE: u32 = 2 * size_of::<u64>() as u32;

/// Size, in bytes, of an entry in a leaf of the (implicit) R-tree.
/// See the `LeafEntry` structure.
pub const LEAF_ENTRY_BYTE_SIZE: u32 = 4 * size_of::<u64>() as u32;

pub struct Header<'a>(&'a [u8; 32]);
impl<'a> Header<'a> {
  /// # Params
  /// * `version`: version number (so far, only '1')
  /// * `page_size_kb`: size of a page, in kB
  /// * `smoc_depth_max`: largest depth of a S-MOC in the file
  /// * `n_entries`: total number of `(Time range, S-MOC)` entries in the file
  /// * `first_leaf_entry_byte_offset`: index of the first byte of the first leaf Time range
  /// * `file_size`: total file size, in bytes
  pub fn to_bytes(
    version: u8,
    page_size_kb: u8,
    smoc_depth_max: u8,
    n_entries: u32,
    first_leaf_entry_byte_offset: u32,
    file_size: u64,
  ) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    (&mut bytes[0..13]).write(MAGIC_BYTES).unwrap();
    bytes[13] = version;
    bytes[14] = page_size_kb;
    bytes[15] = smoc_depth_max;
    (&mut bytes[16..20])
      .write(&n_entries.to_le_bytes())
      .unwrap();
    (&mut bytes[20..24])
      .write(&first_leaf_entry_byte_offset.to_le_bytes())
      .unwrap();
    (&mut bytes[24..32])
      .write(&file_size.to_le_bytes())
      .unwrap();
    bytes
  }
  pub fn are_magic_bytes_ok(&self) -> bool {
    &self.0[0..13] == MAGIC_BYTES
  }

  pub fn version(&self) -> u8 {
    self.0[13]
  }
  pub fn page_size(&self) -> u8 {
    self.0[14]
  }
  pub fn smoc_depth_max(&self) -> u8 {
    self.0[15]
  }
  pub fn n_entries(&self) -> u32 {
    u32::from_le_bytes((&self.0[16..20]).try_into().unwrap())
  }
  pub fn first_leaf_entry_byte_offset(&self) -> u32 {
    u32::from_le_bytes((&self.0[20..24]).try_into().unwrap())
  }
  pub fn file_size(&self) -> u64 {
    u64::from_le_bytes((&self.0[24..32]).try_into().unwrap())
  }
}

/// Represent a layer (made of pages at a same depth) in the implicit R-Tree
pub struct PageLayer {
  /// Number of pages the layer contains
  pub n_pages: u32,
  /// Number of elements (nodes or leaves) the rightmost page contains (cannot = 0).
  pub n_elems_in_rightmost_page: u32,
}
impl PageLayer {
  pub fn new(n_pages: u32, n_elems_in_rightmost_page: u32) -> Self {
    Self {
      n_pages,
      n_elems_in_rightmost_page,
    }
  }
  pub fn n_full_pages(&self) -> u32 {
    debug_assert!(
      (self.n_elems_in_rightmost_page == 0 && ((self.n_elems_in_rightmost_page != 0) as u32) == 0)
        || (self.n_elems_in_rightmost_page != 0
          && ((self.n_elems_in_rightmost_page != 0) as u32) == 1)
    );
    self.n_pages - ((self.n_elems_in_rightmost_page != 0) as u32)
  }
  pub fn n_entries(&self, n_entries_per_page: u32) -> u32 {
    self.n_full_pages() * n_entries_per_page + self.n_elems_in_rightmost_page
  }
  pub fn byte_size(&self, n_entries_per_page: u32, elem_byte_size: u32) -> u64 {
    self.n_entries(n_entries_per_page) as u64 * elem_byte_size as u64
  }
}

/// The structure of the R-Tree is an implicit structure, with all page (node or leaf)
/// full, except the rightmost one.
/// The structure is "recursive", i.e. all sub-nodes of a node are stored after a node, before the
/// next node:
/// `[root_page][ [l1_sub_tree], [l1_sub_tree], ...,  [l1_subtree] ]`
/// in which
/// `[l1_sub_tree] = [l1_page][[l2_sub_tree], [l2_sub_tree], ...[l2_sub_tree]]`
/// The R-Tree is globabl organized as follow:
/// `[[root_page][sub_trees]][leave values]`
pub struct RTreeLayout {
  /// Number of entries in the leaves.
  n_entries: u32,
  /// Size of a page, in kB.
  page_size_kb: u8,
  /// Layers, computed from `n_entries` and `page_size_kb`.
  layers: Vec<PageLayer>,
}
impl RTreeLayout {
  pub fn from(n_entries: u32, page_size_kb: u8) -> Self {
    let mut layers = Vec::<PageLayer>::new();
    // Structure of the layer containing leaves
    let n_entries_per_page = Self::n_leaf_entries_per_page(page_size_kb);
    let mut n_pages = n_entries.div_euclid(n_entries_per_page);
    let mut n_entries_rightmost_page = n_entries.rem_euclid(n_entries_per_page);
    if n_entries_rightmost_page != 0 {
      n_pages += 1;
    } else {
      n_entries_rightmost_page = n_entries_per_page;
    }
    layers.push(PageLayer::new(n_pages, n_entries_rightmost_page));
    // Structure of the layers containing the nodes
    let n_entries_per_page = Self::n_node_entries_per_page(page_size_kb);
    while n_pages > 1 {
      let n_entries = n_pages;
      n_pages = n_entries.div_euclid(n_entries_per_page);
      let mut n_entries_rightmost_page = n_entries.rem_euclid(n_entries_per_page);
      if n_entries_rightmost_page != 0 {
        n_pages += 1;
      } else {
        n_entries_rightmost_page = n_entries_per_page;
      }
      layers.push(PageLayer::new(n_pages, n_entries_rightmost_page));
    }
    // Reverse the order to put the root first and the leave at last index
    layers.reverse();
    Self {
      n_entries,
      page_size_kb,
      layers,
    }
  }

  /// Total number of `(T-Range, S-MOC)` tuples in the structure.
  pub fn n_leaves_entries(&self) -> u32 {
    self.n_entries
  }

  pub fn layers(&self) -> &[PageLayer] {
    &self.layers[..]
  }

  pub fn page_byte_size(&self) -> u32 {
    (self.page_size_kb as u32) << 10
  }

  /// * `dest_bytes`: must contains exactly `self.nodes_byte_size()`
  pub fn get_nodes_writer<'a>(&'a self, dest_bytes: &'a mut [u8]) -> RightmostSubTreeWriter<'a> {
    assert_eq!(self.nodes_byte_size(), dest_bytes.len() as u64);
    let n_regular_node_entries = Self::n_node_entries_per_page(self.page_size_kb);
    RightmostSubTreeWriter::new(
      n_regular_node_entries,
      &self.layers[..self.layers.len() - 1],
      dest_bytes,
    )
  }

  /// * `node_bytes`: must contains exactly `self.nodes_byte_size()`
  pub fn get_nodes_reader<'a>(&'a self, node_bytes: &'a [u8]) -> RightmostSubTreeReader<'a> {
    assert_eq!(self.nodes_byte_size(), node_bytes.len() as u64);
    let n_regular_node_entries = Self::n_node_entries_per_page(self.page_size_kb);
    RightmostSubTreeReader::new(
      n_regular_node_entries,
      &self.layers[..self.layers.len() - 1],
      node_bytes,
      0,
    )
  }

  pub fn n_layers(&self) -> u8 {
    assert!(self.layers.len() <= 255);
    self.layers.len() as u8
  }
  pub fn n_node_entries_per_page(page_size_kb: u8) -> u32 {
    // First convert from kB to B
    debug_assert_eq!(1024, 1 << 10);
    ((page_size_kb as u32) << 10) / NODE_ENTRY_BYTE_SIZE
  }
  pub fn n_leaf_entries_per_page(page_size_kb: u8) -> u32 {
    // First convert from kB to B
    debug_assert_eq!(1024, 1 << 10);
    ((page_size_kb as u32) << 10) / LEAF_ENTRY_BYTE_SIZE
  }

  /// Returns the size of all nodes of implicit RTree structure, ignoring the leaves.
  pub fn nodes_byte_size(&self) -> u64 {
    let n_node_entries_per_page = Self::n_node_entries_per_page(self.page_size_kb);
    let depth = self.layers.len();
    self
      .layers
      .iter()
      .take(depth - 1) // do not take into account the leaves
      .map(|page_layer| page_layer.n_entries(n_node_entries_per_page))
      .sum::<u32>() as u64
      * NODE_ENTRY_BYTE_SIZE as u64
  }
  /// Returns the size of the leaves of the implicit RTree structure, ignoring the nodes.
  pub fn leaves_byte_size(&self) -> u64 {
    let n_leaf_entries_per_page = self.n_leaf_entries_per_regular_page();
    self
      .layers
      .iter()
      .last() // take into account only the leaves
      .map(|page_layer| page_layer.n_entries(n_leaf_entries_per_page))
      .unwrap() as u64
      * LEAF_ENTRY_BYTE_SIZE as u64
  }

  /// Returns the number of leaf entries in a "regular" (i.e. full) page.
  pub fn n_leaf_entries_per_regular_page(&self) -> u32 {
    Self::n_leaf_entries_per_page(self.page_size_kb)
  }

  /// Returns the index, in the file, of the first byte of the Node parts.
  pub fn first_node_byte_offset(&self) -> u64 {
    HEADER_BYTE_SIZE
  }
  /// Returns the index, in the file, of the first byte of the Leaf parts.
  pub fn first_leaf_byte_offset(&self) -> u64 {
    self.first_node_byte_offset() + self.nodes_byte_size()
  }
  /// Returns the index, in the file, of the first byte of the S-MOCs parts.
  pub fn first_smoc_byte_offset(&self) -> u64 {
    self.first_leaf_byte_offset() + self.leaves_byte_size()
  }

  /// Returns the number of node entries a node page and its related sub-node pages contains.
  /// or `n_levels = 1`, returns the `n_node_entries_per_page`.
  pub fn n_nodes_in_regular_sub_tree(&self, n_levels: u8) -> u32 {
    Self::n_nodes_in_regular_sub_tree_for(
      Self::n_node_entries_per_page(self.page_size_kb),
      n_levels,
    )
  }

  /// Returns the size, in bytes, of a sub-tree of given level (at least 1 for a single node page).
  pub fn regular_sub_tree_bytes_size(&self, n_levels: u8) -> usize {
    Self::regular_sub_tree_bytes_size_for(
      Self::n_node_entries_per_page(self.page_size_kb),
      n_levels,
    )
  }

  /// Returns the size, in bytes, of a sub-tree of given level (at least 1 for a single node page).
  pub fn n_nodes_in_regular_sub_tree_for(n_node_entries_per_page: u32, n_levels: u8) -> u32 {
    assert!(n_levels > 0);
    let k = n_node_entries_per_page as u64;
    // \sum_{i=1}^{n_sub} n_node_entries^i = ...
    // \sum_{i=1}^n = k^n = k + k^2 + ... + k^n = S
    // k S = k^2 + k^3 + ... + k^{n+1} = S + k(k^n - 1)
    // => k S = S + k(k^n - 1)
    // => S (k-1) = k(k^n - 1)
    // => S = k(k^n-1) / (k-1)
    ((k.pow(n_levels as u32 + 1) - k) / (k - 1)) as u32
  }

  /// Returns the number of nodes in the deepest layer of a regular subtree of given
  /// number of levels (at least 1).
  pub fn n_nodes_in_regular_sub_tree_deepest_layer(
    n_node_entries_per_page: u32,
    n_levels: u8,
  ) -> u32 {
    assert!(n_levels > 0);
    n_node_entries_per_page.pow(n_levels as u32)
  }

  pub fn regular_sub_tree_bytes_size_for(n_node_entries_per_page: u32, n_levels: u8) -> usize {
    (Self::n_nodes_in_regular_sub_tree_for(n_node_entries_per_page, n_levels) as u64
      * NODE_ENTRY_BYTE_SIZE as u64) as usize
  }
}

/// Represents the information stored n the bytes of a Node entry.
pub struct NodeEntry<'a>(&'a [u64; 2]);
impl<'a> NodeEntry<'a> {
  pub fn new(bytes: &'a [u8]) -> Self {
    Self(slice_u8_to_u64data(bytes).try_into().unwrap())
  }
  pub fn to_bytes(tmin: u64, tmax: u64) -> [u64; 2] {
    /*let mut bytes = [0u64; 2];
    bytes[0] = tmin;
    bytes[1] = tmax;
    bytes*/
    [tmin, tmax]
  }
  /// Time range lower bound (inclusive), in microsec since JD=0
  pub fn tmin(&self) -> u64 {
    self.0[0]
  }
  /// Time range upper bound (exclusive), in microsec since JD=0
  pub fn tmax(&self) -> u64 {
    self.0[1]
  }
  /// Check whether this time range intersects or not the given time range.
  pub fn intersects(&self, time_range: &Range<u64>) -> bool {
    self.tmin() < time_range.end && time_range.start < self.tmax()
  }
}

/// Represents the information stored n the bytes of a Leaf entry.
pub struct LeafEntry<'a>(&'a [u64; 4]);
impl<'a> LeafEntry<'a> {
  pub fn new(bytes: &'a [u8]) -> Self {
    Self(slice_u8_to_u64data(bytes).try_into().unwrap())
  }
  pub fn to_bytes(
    id: u32,
    tmin: u64,
    tmax: u64,
    depth: u8,
    byte_length: u32,
    byte_offset: u64,
  ) -> [u64; 4] {
    let mut bytes = [0u64; 4];
    bytes[0] = tmin;
    bytes[1] = tmax;
    assert!(byte_offset < 1 << 56);
    bytes[2] = ((depth as u64) << 56) | byte_offset;
    bytes[3] = ((byte_length as u64) << 32) | (id as u64);
    bytes
  }
  /// Time range lower bound (inclusive), in microsec since JD=0
  pub fn tmin(&self) -> u64 {
    self.0[0]
  }
  /// Time range upper bound (exclusive), in microsec since JD=0
  pub fn tmax(&self) -> u64 {
    self.0[1]
  }
  /// Depth of the SMOC associated to this entry
  pub fn smoc_depth(&self) -> u8 {
    (self.0[2] >> 56) as u8
  }
  /// Byte offset, from the beginning of the file, of the first element of the SMOC
  pub fn smoc_starting_byte_offset(&self) -> u64 {
    self.0[2] & 0x00FFFFFFFFFFFFFF
  }
  /// Length, in byte on the disk, of the SMOC
  pub fn smoc_byte_length(&self) -> u32 {
    (self.0[3] >> 32) as u32
  }
  /// Unique identifier of the SMOC
  pub fn id(&self) -> u32 {
    (self.0[3] & 0x00000000FFFFFFFF) as u32
  }

  /// Check whether this time range intersects or not the given time range.
  pub fn intersects(&self, time_range: &Range<u64>) -> bool {
    self.tmin() < time_range.end && time_range.start < self.tmax()
  }
}

// STMocPPWriter
// * input: (page_size_kb: u8, n_entries: u32, sorted_input_file_iterator)
//     ,

// STMocPPWReader
// * from memmmap (starts reading the header)
// * Separate: header (32 bytes), Node bytes, Leaf bytes, SMOC bytes

pub trait SubTreeWriter {
  /// Iterates on the deepest node layer entries and write the sub-tree from the input iterator.
  /// Once the sub-tree is full, returns the time range the sub-tree covers
  /// (the iterator is fully consumed, except for the rightmost sub-tree).
  fn write<I>(self, deepest_layer_entries: &mut I) -> Range<u64>
  where
    I: Iterator<Item = Range<u64>>;
}

/// Implicit RTree Node write for the regular part of the tree.
/// Here, the given input iterator **is not supposed** to become
/// depleted (it should arise in the non-regular part of the tree, i.e. in the rightmost node).
pub struct RegularSubTreeWriter<'a> {
  /// Number of level the sub-tree contains (0 if no sub-level)
  n_sub_levels: u8,
  /// Constant value: number of entries in each node.
  n_node_entries: u32,
  /// Cursor to write the entries of this page
  entries_writer: Cursor<&'a mut [u8]>,
  // /// The current sub-tree being populated
  // current_sub_tree: Option<Box<RegularSubTreeWriter<'a>>>,
  /// Bytes reserved for the entry sub-trees
  remaining_storage: &'a mut [u8],
}
impl<'a> RegularSubTreeWriter<'a> {
  /// # Params
  /// * `n_node_entries`: number of entries in a node page (1 node = 1 page)
  /// * `n_sub_levels`: number of level of the sub-tree entries of this sub-tree (0 if contains leaf nodes)
  /// * `dest_bytes`: the bytes in which the full sub-tree (with its sub-trees) will be written (des_bytes probably backed by a mmmap).
  pub fn new(n_sub_levels: u8, n_node_entries: u32, dest_bytes: &'a mut [u8]) -> Self {
    let page_byte_size = n_node_entries * NODE_ENTRY_BYTE_SIZE;
    let (local_bytes, remaining_storage) = dest_bytes.split_at_mut(page_byte_size as usize);
    let entries_writer = Cursor::new(local_bytes);
    Self {
      n_sub_levels,
      n_node_entries,
      entries_writer,
      remaining_storage,
    }
  }
}
impl<'a> SubTreeWriter for RegularSubTreeWriter<'a> {
  fn write<I>(mut self, deepest_layer_entries: &mut I) -> Range<u64>
  where
    I: Iterator<Item = Range<u64>>,
  {
    if self.n_sub_levels == 0 {
      assert!(self.remaining_storage.is_empty());
      let mut writer = self.entries_writer;
      let mut min = u64::MAX;
      let mut max = 0;
      for (_, Range { start, end }) in (0..self.n_node_entries)
        .into_iter()
        .zip(deepest_layer_entries)
      {
        if start < min {
          min = start;
        }
        if end > max {
          max = end;
        }
        writer
          .write_all(&start.to_le_bytes())
          .and_then(|()| writer.write_all(&end.to_le_bytes()))
          .expect("Unexpected error!");
      }
      min..max
    } else {
      let sub_tree_byte_size =
        RTreeLayout::regular_sub_tree_bytes_size_for(self.n_node_entries, self.n_sub_levels);
      let mut writer = self.entries_writer;
      let mut min = u64::MAX;
      let mut max = 0;
      for _ in 0..self.n_node_entries {
        let (current_sub_tree_bytes, remaining_storage) =
          self.remaining_storage.split_at_mut(sub_tree_byte_size);
        self.remaining_storage = remaining_storage;
        let Range { start, end } = RegularSubTreeWriter::new(
          self.n_sub_levels - 1,
          self.n_node_entries,
          current_sub_tree_bytes,
        )
        .write(deepest_layer_entries);
        if start < min {
          min = start;
        }
        if end > max {
          max = end;
        }
        writer
          .write_all(&start.to_le_bytes())
          .and_then(|()| writer.write_all(&end.to_le_bytes()))
          .expect("Unexpected error!");
      }
      min..max
    }
  }
}

/// Implicit RTree Node writer for the rihgtmost (irregular) part of the tree.
/// Here, the given input iterator **is not supposed** to become
/// depleted (it should arise in the non-regular part of the tree, i.e. in the rightmost node).
pub struct RightmostSubTreeWriter<'a> {
  /// Sub tree layers, without the leaves.
  sub_node_layers: &'a [PageLayer],
  /// Number of entries this (rightmost) node/page contains.
  n_node_entries: u32,
  /// Constant value: number of entries in each "regular" node.
  n_regular_node_entries: u32,
  /// Cursor to write the entries of this page
  entries_writer: Cursor<&'a mut [u8]>,
  /// Bytes reserved for the entry sub-trees
  remaining_storage: &'a mut [u8],
}
impl<'a> RightmostSubTreeWriter<'a> {
  fn new(
    n_regular_node_entries: u32,
    node_layers: &'a [PageLayer],
    dest_bytes: &'a mut [u8],
  ) -> Self {
    assert!(!node_layers.is_empty());
    let (curr_layer, sub_node_layers) = node_layers.split_at(1);
    let n_node_entries = curr_layer[0].n_elems_in_rightmost_page;
    let page_byte_size = (n_node_entries * NODE_ENTRY_BYTE_SIZE) as usize;
    let (local_bytes, remaining_storage) = dest_bytes.split_at_mut(page_byte_size);
    let entries_writer = Cursor::new(local_bytes);
    Self {
      sub_node_layers,
      n_node_entries,
      n_regular_node_entries,
      entries_writer,
      remaining_storage,
    }
  }
}

impl<'a> SubTreeWriter for RightmostSubTreeWriter<'a> {
  fn write<I>(mut self, deepest_layer_entries: &mut I) -> Range<u64>
  where
    I: Iterator<Item = Range<u64>>,
  {
    let n_sub_levels = self.sub_node_layers.len() as u8;
    if n_sub_levels == 0 {
      let mut writer = self.entries_writer;
      let mut min = u64::MAX;
      let mut max = 0;
      for (_, Range { start, end }) in (0..self.n_node_entries)
        .into_iter()
        .zip(deepest_layer_entries)
      {
        if start < min {
          min = start;
        }
        if end > max {
          max = end;
        }
        writer
          .write_all(&start.to_le_bytes())
          .and_then(|()| writer.write_all(&end.to_le_bytes()))
          .expect("Unexpected error!");
      }
      writer.flush().expect("Error flushing data!");
      min..max
    } else {
      let sub_tree_byte_size =
        RTreeLayout::regular_sub_tree_bytes_size_for(self.n_regular_node_entries, n_sub_levels);
      let mut writer = self.entries_writer;
      let mut min = u64::MAX;
      let mut max = 0;
      // All node except the rightmost one
      for _ in 0..self.n_node_entries - 1 {
        let (current_sub_tree_bytes, remaining_storage) =
          self.remaining_storage.split_at_mut(sub_tree_byte_size);
        self.remaining_storage = remaining_storage;
        let Range { start, end } = RegularSubTreeWriter::new(
          n_sub_levels - 1,
          self.n_regular_node_entries,
          current_sub_tree_bytes,
        )
        .write(deepest_layer_entries);
        if start < min {
          min = start;
        }
        if end > max {
          max = end;
        }
        writer
          .write_all(&start.to_le_bytes())
          .and_then(|()| writer.write_all(&end.to_le_bytes()))
          .expect("Unexpected error!");
      }
      // Deal with the rightmost node
      let Range { start, end } = RightmostSubTreeWriter::new(
        self.n_regular_node_entries,
        self.sub_node_layers,
        self.remaining_storage,
      )
      .write(deepest_layer_entries);
      if start < min {
        min = start;
      }
      if end > max {
        max = end;
      }
      writer
        .write_all(&start.to_le_bytes())
        .and_then(|()| writer.write_all(&end.to_le_bytes()))
        .expect("Unexpected error!");
      // Return result
      writer.flush().expect("Error flushing data!");
      min..max
    }
  }
}

pub struct STMocPPFile {
  mmap: Mmap,
  version: u8,
  page_size_kb: u8,
  smoc_depth_max: u8,
  n_entries: u32,
  first_leaf_entry_byte_offset: u32,
  file_size: u64,
  layout: RTreeLayout,
}
impl STMocPPFile {
  pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, IoError> {
    let file = File::open(path.as_ref())?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let file_size = mmap.deref().len() as u64;
    if file_size < HEADER_BYTE_SIZE {
      return Err(IoError::other(format!(
        "File '{:?}' is too small!",
        path.as_ref()
      )));
    }
    // Read header
    let header = Header(&mmap[..HEADER_BYTE_SIZE as usize].try_into().unwrap());
    // * check magic byte
    if !header.are_magic_bytes_ok() {
      return Err(IoError::other(format!(
        "File '{:?}' does not seem to be a ST-MOC PP file (wrong magic key)!",
        path.as_ref()
      )));
    }
    // * get and check version
    let version = header.version();
    if version != 1 {
      return Err(IoError::other(format!(
        "Wrong file version. Expected: 1. Actual: {}.",
        version
      )));
    }
    // * check file length
    let expected_file_size = header.file_size();
    if file_size != expected_file_size {
      return Err(IoError::other(format!(
        "Wrong file length. Expected: {}. Actual: {}.",
        expected_file_size, file_size
      )));
    }
    // * Get useful info
    let n_entries = header.n_entries();
    let page_size_kb = header.page_size();
    let smoc_depth_max = header.smoc_depth_max();
    let first_leaf_entry_byte_offset = header.first_leaf_entry_byte_offset();
    let layout = RTreeLayout::from(n_entries, page_size_kb);
    assert_eq!(
      layout.first_leaf_byte_offset(),
      first_leaf_entry_byte_offset as u64
    );
    Ok(Self {
      mmap,
      version,
      page_size_kb,
      smoc_depth_max,
      n_entries,
      first_leaf_entry_byte_offset,
      file_size,
      layout,
    })
  }

  pub fn version(&self) -> u8 {
    self.version
  }

  pub fn page_size_kb(&self) -> u8 {
    self.page_size_kb
  }

  pub fn smoc_depth_max(&self) -> u8 {
    self.smoc_depth_max
  }

  pub fn n_entries(&self) -> u32 {
    self.n_entries
  }

  pub fn file_size(&self) -> u64 {
    self.file_size
  }

  pub fn layout(&self) -> &RTreeLayout {
    &self.layout
  }

  pub fn get_reader(&self) -> STMocPPFileReader<'_> {
    let bytes = &self.mmap[..];
    let (bytes, smocs_bytes) = bytes.split_at(self.layout.first_smoc_byte_offset() as usize);
    let (bytes, leaves_bytes) = bytes.split_at(self.first_leaf_entry_byte_offset as usize);
    let (header, nodes_bytes) = bytes.split_at(self.layout.first_node_byte_offset() as usize);
    assert_eq!(header.len(), HEADER_BYTE_SIZE as usize);
    STMocPPFileReader::new(&self.layout, nodes_bytes, leaves_bytes, smocs_bytes)
  }
}

pub struct STMocPPFileReader<'a> {
  layout: &'a RTreeLayout,
  nodes_bytes: &'a [u8],
  leaves_bytes: &'a [u8],
  smocs_bytes: &'a [u8],
}
impl<'a> STMocPPFileReader<'a> {
  fn new(
    layout: &'a RTreeLayout,
    nodes_bytes: &'a [u8],
    leaves_bytes: &'a [u8],
    smocs_bytes: &'a [u8],
  ) -> Self {
    Self {
      layout,
      nodes_bytes,
      leaves_bytes,
      smocs_bytes,
    }
  }

  /// Bytes from the beginning of the file
  pub fn get_smoc_bytes(&self, bytes: Range<usize>) -> &[u8] {
    let first_smoc_byte_offset = self.layout.first_smoc_byte_offset() as usize;
    let bytes = bytes.start - first_smoc_byte_offset..bytes.end - first_smoc_byte_offset;
    &self.smocs_bytes[bytes]
  }

  /// Iterates on all leaves, without resorting to the RTree structure.
  /// # Info
  /// Useful for sequential scan, i.e. mainly for debug.
  pub fn leaves(&self) -> impl Iterator<Item = LeafEntry<'_>> {
    self
      .leaves_bytes
      .chunks(LEAF_ENTRY_BYTE_SIZE as usize)
      .map(move |bytes| LeafEntry::new(bytes))
  }

  /// Returns all leave intersecting the given time range sequentially,
  /// i.e. without resorting to the RTRee structure.
  pub fn sequential_query(&self, trange: &Range<u64>) -> impl Iterator<Item = LeafEntry<'_>> {
    self.leaves().filter(move |leaf| leaf.intersects(trange))
  }

  /// Returns all leave intersecting the given time range, resorting to the RTree structure.
  pub fn rtree_query(&'a self, trange: &'a Range<u64>) -> impl Iterator<Item = LeafEntry<'a>> + 'a {
    // Box<dyn Iterator<Item = LeafEntry<'_>> + '_> {
    let reader = self.layout.get_nodes_reader(self.nodes_bytes);
    // create a visitor
    // * from node index, go read leaves
    let leave_page_bytes_size = self.layout.page_byte_size() as usize;
    let box_dyn_it = reader.deepest_nodes_index(trange);
    box_dyn_it
      .map(move |i_node| {
        let from = i_node as usize * leave_page_bytes_size;
        // The last page can be smaller (hence the 'min')
        let to = (from + leave_page_bytes_size).min(self.leaves_bytes.len());
        (&self.leaves_bytes[from..to])
          .chunks(LEAF_ENTRY_BYTE_SIZE as usize)
          .filter_map(move |bytes| {
            let leaf = LeafEntry::new(bytes);
            if leaf.intersects(trange) {
              Some(leaf)
            } else {
              None
            }
          })
      })
      .flatten()
  }

  pub fn print_nodes<W: Write>(&self, writer: &mut W, time_fmt: &TimeType) -> Result<(), IoError> {
    let reader = self.layout.get_nodes_reader(self.nodes_bytes);
    reader.print_nodes(0, 0, writer, time_fmt)
  }

  /// Filter returning only entries having SMOCs intersecting the given SMOC.
  /// (Returns the SMOC bytes)
  pub fn smoc_filter<I>(
    &self,
    it: I,
    smoc_u32: &Ranges<u32>,
    smoc_u64: &Ranges<u64>,
  ) -> impl Iterator<Item = (LeafEntry<'_>, &'_ [u8])>
  where
    I: Iterator<Item = LeafEntry<'a>>,
  {
    let first_smoc_byte_offset = self.layout.first_smoc_byte_offset() as usize;
    // let smoc32: Ranges<u32> = smoc_u32.into_moc_ranges().into_ranges();
    // let smoc64: Ranges<u64> = smoc_u64.into_moc_ranges().into_ranges();
    it.filter_map(move |leaf| {
      let depth = leaf.smoc_depth();
      let bstart = leaf.smoc_starting_byte_offset() as usize - first_smoc_byte_offset;
      let bend = bstart + leaf.smoc_byte_length() as usize;
      let smoc_bytes = &self.smocs_bytes[bstart..bend];
      let intersects = if depth <= Hpx::<u32>::MAX_DEPTH {
        let ranges: BorrowedRanges<'_, u32> = BorrowedRanges::from(smoc_bytes);
        // ranges.intersection(&(&smoc32).into())
        ranges.intersects(&(smoc_u32).into())
      } else {
        let ranges: BorrowedRanges<'_, u64> = BorrowedRanges::from(smoc_bytes);
        // ranges.intersection(&(&smoc64).into())
        ranges.intersects(&(smoc_u64).into())
      };
      if intersects {
        Some((leaf, smoc_bytes))
      } else {
        None
      }
    })
  }

  /// Filter returning only entries having SMOCs whose intersection with the given SMOC covers
  /// the minimum given area (in sky fraction).
  /// (Returns the SMOC bytes)
  pub fn smoc_filter_with_minimum_area<I>(
    &self,
    it: I,
    smoc_u32_depth: u8,
    smoc_u32_ranges: &Ranges<u32>,
    smoc_u64_depth: u8,
    smoc_u64_ranges: &Ranges<u64>,
    minimum_sky_fraction: f64,
  ) -> impl Iterator<Item = (LeafEntry<'_>, &'_ [u8])>
  where
    I: Iterator<Item = LeafEntry<'a>>,
  {
    let first_smoc_byte_offset = self.layout.first_smoc_byte_offset() as usize;
    it.filter_map(move |leaf| {
      let depth = leaf.smoc_depth();
      let bstart = leaf.smoc_starting_byte_offset() as usize - first_smoc_byte_offset;
      let bend = bstart + leaf.smoc_byte_length() as usize;
      let smoc_bytes = &self.smocs_bytes[bstart..bend];
      let intersection_sky_fraction = if depth <= Hpx::<u32>::MAX_DEPTH {
        let ranges: BorrowedRanges<'_, u32> = BorrowedRanges::from(smoc_bytes);
        let intersection_depth = smoc_u32_depth.max(depth);
        let intersection_ranges = ranges.intersection(&(smoc_u32_ranges).into());
        let intersection_moc =
          RangeMOC::<u32, Hpx<u32>>::new(intersection_depth, intersection_ranges.into());
        intersection_moc.coverage_percentage()
      } else {
        let ranges: BorrowedRanges<'_, u64> = BorrowedRanges::from(smoc_bytes);
        let intersection_depth = smoc_u64_depth.max(depth);
        let intersection_ranges = ranges.intersection(&(smoc_u64_ranges).into());
        let intersection_moc =
          RangeMOC::<u64, Hpx<u64>>::new(intersection_depth, intersection_ranges.into());
        intersection_moc.coverage_percentage()
      };
      if intersection_sky_fraction > 0.0 && intersection_sky_fraction >= minimum_sky_fraction {
        Some((leaf, smoc_bytes))
      } else {
        None
      }
    })
  }
}

/*
// Like writer: pass de bytes of sub-trees
pub trait SubTreeReader {
  /// Iterates on the deepest node layer entries and write the sub-tree from the input iterator.
  /// Once the sub-tree is full, returns the time range the sub-tree covers
  /// (the iterator is fully consumed, except for the rightmost sub-tree).
  fn read<I>(self, deepest_layer_entries: &mut I) -> Range<u64>
  where
    I: Iterator<Item = Range<u64>>;
}*/

/// Implicit RTree Node reader for the rightmost (the irregular) part of the tree.
/// Here, the given input iterator **is not supposed** to become
/// depleted (it should arise in the non-regular part of the tree, i.e. in the rightmost node).
pub struct RightmostSubTreeReader<'a> {
  /// Sub tree layers, without the leaves.
  sub_node_layers: &'a [PageLayer],
  /// Number of entries this (rightmost) node/page contains.
  n_node_entries: u32,
  /// Constant value: number of entries in each "regular" node.
  n_regular_node_entries: u32,
  /// Bytes of this node entries
  entries_bytes: &'a [u8],
  /// Bytes reserved for the sub-trees
  remaining_storage: &'a [u8],

  /// Number of node entries (all layer) on the left part of this sub-tree.
  n_leftmost_nodes: u32,
}

impl<'a> RightmostSubTreeReader<'a> {
  fn new(
    n_regular_node_entries: u32,
    node_layers: &'a [PageLayer],
    node_bytes: &'a [u8],
    n_leftmost_nodes: u32,
  ) -> Self {
    assert!(!node_layers.is_empty());
    let (curr_layer, sub_node_layers) = node_layers.split_at(1);
    let n_node_entries = curr_layer[0].n_elems_in_rightmost_page;
    let page_byte_size = (n_node_entries * NODE_ENTRY_BYTE_SIZE) as usize;
    let (entries_bytes, remaining_storage) = node_bytes.split_at(page_byte_size);
    Self {
      sub_node_layers,
      n_node_entries,
      n_regular_node_entries,
      entries_bytes,
      remaining_storage,
      n_leftmost_nodes,
    }
  }

  pub fn n_entries(&self) -> usize {
    self.entries_bytes.len() / (NODE_ENTRY_BYTE_SIZE as usize)
  }

  pub fn nodes(&self) -> impl Iterator<Item = NodeEntry<'_>> {
    self
      .entries_bytes
      .chunks(NODE_ENTRY_BYTE_SIZE as usize)
      .map(move |bytes| NodeEntry::new(bytes))
  }

  pub fn into_nodes(self) -> impl Iterator<Item = NodeEntry<'a>> {
    self
      .entries_bytes
      .chunks(NODE_ENTRY_BYTE_SIZE as usize)
      .map(move |bytes| NodeEntry::new(bytes))
  }

  /// Returns an iterator on nodes index (at the deepest layer) intersecting the given time range.
  pub fn deepest_nodes_index(self, trange: &'a Range<u64>) -> Box<dyn Iterator<Item = u32> + 'a> {
    let n_sub_levels = self.sub_node_layers.len() as u8;
    if n_sub_levels == 0 {
      assert!(self.remaining_storage.len() == 0);
      let n_leftmost_nodes = self.n_leftmost_nodes;
      Box::new(self.into_nodes().enumerate().filter_map(move |(i, node)| {
        if node.intersects(&trange) {
          Some(n_leftmost_nodes + i as u32)
        } else {
          None
        }
      }))
    } else {
      // make a single iterator but separate
      // * regular case
      // * rightmos case
      let sub_node_layers = self.sub_node_layers;
      let n_node_entries = self.n_node_entries as usize;
      let n_regular_node_entries = self.n_regular_node_entries;
      let n_leftmost_nodes = self.n_leftmost_nodes;
      let remaining_storage = self.remaining_storage;
      // regular sub-tree
      let sub_tree_byte_size =
        RTreeLayout::regular_sub_tree_bytes_size_for(n_regular_node_entries, n_sub_levels);
      let n_nodes_in_subtree_deepest_layer = RTreeLayout::n_nodes_in_regular_sub_tree_deepest_layer(
        n_regular_node_entries,
        n_sub_levels,
      );
      Box::new(
        self
          .into_nodes()
          .enumerate()
          .filter_map(move |(i, node)| {
            if node.intersects(&trange) {
              let from_byte = i * sub_tree_byte_size;
              let i_leftmost_nodes = n_leftmost_nodes + i as u32 * n_nodes_in_subtree_deepest_layer;
              if i < n_node_entries - 1 {
                // regular sub-tree
                let reader = RegularSubTreeReader::new(
                  n_sub_levels - 1,
                  n_regular_node_entries,
                  &remaining_storage[from_byte..(i + 1) * sub_tree_byte_size],
                  i_leftmost_nodes,
                );
                Some(reader.deepest_nodes_index(&trange))
              } else {
                // righmost sub-tree
                let reader = RightmostSubTreeReader::new(
                  n_regular_node_entries,
                  sub_node_layers,
                  &remaining_storage[from_byte..],
                  i_leftmost_nodes,
                );
                Some(reader.deepest_nodes_index(&trange))
              }
            } else {
              None
            }
          })
          .flatten(),
      )
    }
  }

  pub fn print_nodes<W: Write>(
    &self,
    level: u8,
    inode: u32,
    writer: &mut W,
    time_fmt: &TimeType,
  ) -> Result<(), IoError> {
    let n_sub_levels = self.sub_node_layers.len() as u8;
    if n_sub_levels == 0 {
      // Write the page entries
      writeln!(
        writer,
        "# Level: {} (deepest nodes); Page index: {}",
        level, inode
      )?;
      writeln!(writer, "inode_in_layer,tmin,tmax")?;
      for (i, node) in self.nodes().enumerate() {
        write!(writer, "{}", self.n_leftmost_nodes + i as u32)
          .and_then(|()| time_fmt.write_time_range(writer, node.tmin()..node.tmax()))
          .and_then(|()| writeln!(writer))?;
      }
    } else {
      // Write the page entries
      writeln!(writer, "# Level: {}; Page index: {}", level, inode)?;
      writeln!(writer, "inode_in_layer,tmin,tmax")?;
      let inode = inode * self.n_node_entries;
      for (i, node) in self.nodes().enumerate() {
        write!(writer, "{}", inode + i as u32)
          .and_then(|()| time_fmt.write_time_range(writer, node.tmin()..node.tmax()))
          .and_then(|()| writeln!(writer))?;
      }
      // Write sub-trees entries
      let sub_tree_byte_size =
        RTreeLayout::regular_sub_tree_bytes_size_for(self.n_regular_node_entries, n_sub_levels);
      let n_nodes_in_subtree_deepest_layer = RTreeLayout::n_nodes_in_regular_sub_tree_deepest_layer(
        self.n_regular_node_entries,
        n_sub_levels,
      );
      for i in 0..self.n_entries() {
        let from_byte = i * sub_tree_byte_size;
        let i_leftmost_nodes = self.n_leftmost_nodes + i as u32 * n_nodes_in_subtree_deepest_layer;
        if (i as u32) < self.n_node_entries - 1 {
          // regular sub-tree
          let reader = RegularSubTreeReader::new(
            n_sub_levels - 1,
            self.n_regular_node_entries,
            &self.remaining_storage[from_byte..(i + 1) * sub_tree_byte_size],
            i_leftmost_nodes,
          );
          reader.print_nodes(
            level + 1,
            inode * self.n_node_entries + i as u32,
            writer,
            time_fmt,
          )?;
        } else {
          // righmost sub-tree
          let reader = RightmostSubTreeReader::new(
            self.n_regular_node_entries,
            self.sub_node_layers,
            &self.remaining_storage[from_byte..],
            i_leftmost_nodes,
          );
          reader.print_nodes(
            level + 1,
            inode * self.n_node_entries + i as u32,
            writer,
            time_fmt,
          )?;
        }
      }
    }
    Ok(())
  }
}

/// Implicit RTree Node reader for the regular part of the tree.
pub struct RegularSubTreeReader<'a> {
  /// Number of level the sub-tree contains (0 if no sub-level)
  n_sub_levels: u8,
  /// Constant value: number of entries in each node.
  n_node_entries: u32,
  /// Bytes of this node entries
  entries_bytes: &'a [u8],
  /// Bytes reserved for the entry sub-trees
  remaining_storage: &'a [u8],

  /// Number of node entries (all layer) on the left part of this sub-tree.
  n_leftmost_nodes: u32,
}
impl<'a> RegularSubTreeReader<'a> {
  /// # Params
  /// * `n_node_entries`: number of entries in a node page (1 node = 1 page)
  /// * `n_sub_levels`: number of level of the sub-tree entries of this sub-tree (0 if contains leaf nodes)
  /// * `dest_bytes`: the bytes in which the full sub-tree (with its sub-trees) will be written (des_bytes probably backed by a mmmap).
  pub fn new(
    n_sub_levels: u8,
    n_node_entries: u32,
    bytes: &'a [u8],
    n_leftmost_nodes: u32,
  ) -> Self {
    let page_byte_size = n_node_entries * NODE_ENTRY_BYTE_SIZE;
    let (entries_bytes, remaining_storage) = bytes.split_at(page_byte_size as usize);
    Self {
      n_sub_levels,
      n_node_entries,
      entries_bytes,
      remaining_storage,
      n_leftmost_nodes,
    }
  }

  pub fn n_entries(&self) -> usize {
    assert_eq!(
      self.n_node_entries as usize,
      self.entries_bytes.len() / NODE_ENTRY_BYTE_SIZE as usize
    );
    self.n_node_entries as usize
  }

  pub fn nodes(&self) -> impl Iterator<Item = NodeEntry<'_>> {
    self
      .entries_bytes
      .chunks(NODE_ENTRY_BYTE_SIZE as usize)
      .map(move |bytes| NodeEntry::new(bytes))
  }

  pub fn into_nodes(self) -> impl Iterator<Item = NodeEntry<'a>> {
    self
      .entries_bytes
      .chunks(NODE_ENTRY_BYTE_SIZE as usize)
      .map(move |bytes| NodeEntry::new(bytes))
  }

  /// Returns an iterator on nodes index (at the deepest layer) intersecting the given time range.
  pub fn deepest_nodes_index(self, trange: &'a Range<u64>) -> Box<dyn Iterator<Item = u32> + 'a> {
    if self.n_sub_levels == 0 {
      let n_leftmost_nodes = self.n_leftmost_nodes;
      Box::new(self.into_nodes().enumerate().filter_map(move |(i, node)| {
        if node.intersects(&trange) {
          Some(n_leftmost_nodes + i as u32)
        } else {
          None
        }
      }))
    } else {
      let n_leftmost_nodes = self.n_leftmost_nodes;
      let n_node_entries = self.n_node_entries;
      let n_sub_levels = self.n_sub_levels;
      let remaining_storage = self.remaining_storage;
      let sub_tree_byte_size =
        RTreeLayout::regular_sub_tree_bytes_size_for(n_node_entries, n_sub_levels);
      let n_nodes_in_subtree_deepest_layer =
        RTreeLayout::n_nodes_in_regular_sub_tree_deepest_layer(n_node_entries, n_sub_levels);
      Box::new(
        self
          .into_nodes()
          .enumerate()
          .filter_map(move |(i, node)| {
            if node.intersects(&trange) {
              let reader = Self::new(
                n_sub_levels - 1,
                n_node_entries,
                &remaining_storage[i * sub_tree_byte_size..(i + 1) * sub_tree_byte_size],
                n_leftmost_nodes + i as u32 * n_nodes_in_subtree_deepest_layer,
              );
              Some(reader.deepest_nodes_index(&trange))
            } else {
              None
            }
          })
          .flatten(),
      )
    }
  }

  pub fn print_nodes<W: Write>(
    &self,
    level: u8,
    inode: u32,
    writer: &mut W,
    time_fmt: &TimeType,
  ) -> Result<(), IoError> {
    if self.n_sub_levels == 0 {
      // Write the page entries
      writeln!(
        writer,
        "# Level: {} (deepest nodes); Page index: {}",
        level, inode
      )?;
      writeln!(writer, "inode_in_layer,tmin,tmax")?;
      for (i, node) in self.nodes().enumerate() {
        write!(writer, "{}", self.n_leftmost_nodes + i as u32)
          .and_then(|()| time_fmt.write_time_range(writer, node.tmin()..node.tmax()))
          .and_then(|()| writeln!(writer))?;
      }
    } else {
      // Write the page entries
      writeln!(writer, "# Level: {}; Page index: {}", level, inode)?;
      writeln!(writer, "inode_in_layer,tmin,tmax")?;
      let inode = inode * self.n_node_entries;
      for (i, node) in self.nodes().enumerate() {
        write!(writer, "{}", inode + i as u32)
          .and_then(|()| time_fmt.write_time_range(writer, node.tmin()..node.tmax()))
          .and_then(|()| writeln!(writer))?;
      }
      // Write sub-trees entries
      let sub_tree_byte_size =
        RTreeLayout::regular_sub_tree_bytes_size_for(self.n_node_entries, self.n_sub_levels);
      let n_nodes_in_subtree_deepest_layer = RTreeLayout::n_nodes_in_regular_sub_tree_deepest_layer(
        self.n_node_entries,
        self.n_sub_levels,
      );
      for i in 0..self.n_entries() {
        let from_byte = i * sub_tree_byte_size;
        let i_leftmost_nodes = self.n_leftmost_nodes + i as u32 * n_nodes_in_subtree_deepest_layer;
        // regular sub-tree
        let reader = RegularSubTreeReader::new(
          self.n_sub_levels - 1,
          self.n_node_entries,
          &self.remaining_storage[from_byte..(i + 1) * sub_tree_byte_size],
          i_leftmost_nodes,
        );
        reader.print_nodes(
          level + 1,
          inode * self.n_node_entries + i as u32,
          writer,
          time_fmt,
        )?;
      }
    }
    Ok(())
  }

  /*pub fn query<V: Visitor>(&self, trange: &Range<u64>, visitor: &mut V) {
    if self.n_sub_levels == 0 {
      for (i, node) in self.nodes().enumerate() {
        if node.intersects(&trange) {
          visitor.visit(self.n_leftmost_nodes + i);
        }
      }
    } else {
      for (i, node) in self.nodes().enumerate() {
        if node.intersects(&trange) {
          let reader = todo!(); //CREATE SU READER
          reader.query(trange, visitor);
        }
      }
    }
  }*/
}

/*
trait Visitor {
  fn visit(&mut self, deepest_layer_node_index: u64);
}
*/
