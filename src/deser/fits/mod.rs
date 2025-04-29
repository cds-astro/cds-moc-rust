//! This module deals with MOC serialization/deserialization in the
//! [FITS standard](https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf).

use std::{
  io::{BufRead, Cursor, Write},
  marker::PhantomData,
  ops::Range,
};

use byteorder::BigEndian;
use log::warn;

use crate::{
  deser::fits::{
    common::{
      check_keyword_and_parse_uint_val, check_keyword_and_val, consume_primary_hdu,
      next_36_chunks_of_80_bytes, write_primary_hdu, write_uint_mandatory_keyword_record,
    },
    error::FitsError,
    keywords::{
      CoordSys, FitsCard, MocDim, MocId, MocKeywords, MocKeywordsMap, MocOrdF, MocOrdS, MocOrdT,
      MocOrder, MocTool, MocVers, Ordering, TForm1, TType1, TimeSys,
    },
  },
  elem::cell::Cell,
  elemset::{
    cell::{Cells, MocCells},
    range::MocRanges,
  },
  idx::Idx,
  moc::{
    cell::CellMOC,
    range::{op::convert::convert_to_u64, RangeMOC, RangeMocIter},
    CellMOCIntoIterator, CellMOCIterator, HasMaxDepth, MOCProperties, NonOverlapping,
    RangeMOCIterator, ZSorted,
  },
  moc2d::{
    range::{RangeMOC2, RangeMOC2Elem},
    HasTwoMaxDepth, MOC2Properties, RangeMOC2ElemIt, RangeMOC2IntoIterator, RangeMOC2Iterator,
  },
  qty::{Frequency, Hpx, MocQty, MocableQty, Time},
};

pub mod common;
pub mod error;
pub mod keywords;
pub mod multiordermap;
pub mod skymap;

#[derive(Debug)]
pub enum MocIdxType<R: BufRead> {
  //U8(MocQtyType<u8, R>),
  U16(MocQtyType<u16, R>),
  U32(MocQtyType<u32, R>),
  U64(MocQtyType<u64, R>),
  // U128(MocQtyType<u128, R>),
}
impl<R: BufRead> MocIdxType<R> {
  pub fn to_fits_ivoa<W: Write>(self, write: W) -> Result<(), FitsError> {
    match self {
      //MocIdxType::U8(moc_qty_type) => moc_qty_type.to_fits_ivoa(write),
      MocIdxType::U16(moc_qty_type) => moc_qty_type.to_fits_ivoa(write),
      MocIdxType::U32(moc_qty_type) => moc_qty_type.to_fits_ivoa(write),
      MocIdxType::U64(moc_qty_type) => moc_qty_type.to_fits_ivoa(write),
      // MocIdxType::U128(moc_qty_type) => moc_qty_type.to_fits_ivoa(write),
    }
  }
}

#[derive(Debug)]
pub enum MocQtyType<T: Idx, R: BufRead> {
  Hpx(MocType<T, Hpx<T>, R>),
  Time(MocType<T, Time<T>, R>),
  TimeHpx(STMocType<T, R>),
  Freq(MocType<T, Frequency<T>, R>),
}
impl<T: Idx, R: BufRead> MocQtyType<T, R> {
  pub fn to_fits_ivoa<W: Write>(self, write: W) -> Result<(), FitsError> {
    match self {
      MocQtyType::Hpx(moc_type) => moc_type.to_fits_ivoa(write),
      MocQtyType::Time(moc_type) => moc_type.to_fits_ivoa(write),
      MocQtyType::TimeHpx(moc_type) => moc_type.to_fits_ivoa(write),
      MocQtyType::Freq(moc_type) => moc_type.to_fits_ivoa(write),
    }
  }
}

#[derive(Debug)]
pub enum MocType<T: Idx, Q: MocQty<T>, R: BufRead> {
  Ranges(RangeMocIterFromFits<T, Q, R>),
  Cells(CellMOC<T, Q>),
  // GUniq(CellMOC<T, Q>), // TODO: replace by an iterator
  // RUniq(),
  // RMixed(),
}
impl<T: Idx, Q: MocQty<T>, R: BufRead> MocType<T, Q, R> {
  pub fn to_fits_ivoa<W: Write>(self, write: W) -> Result<(), FitsError> {
    match self {
      MocType::Ranges(ranges) => ranges_to_fits_ivoa(ranges, None, None, write),
      MocType::Cells(cells) => {
        ranges_to_fits_ivoa(cells.into_cell_moc_iter().ranges(), None, None, write)
      }
    }
  }
  pub fn collect(self) -> RangeMOC<T, Q> {
    match self {
      MocType::Ranges(ranges) => ranges.into_range_moc(),
      MocType::Cells(cells) => cells.into_cell_moc_iter().ranges().into_range_moc(),
    }
  }
  /// WARNING: you have to be sure that 'P' is the same qty as 'Q'!!
  pub fn collect_to_u64<P: MocQty<u64>>(self) -> RangeMOC<u64, P> {
    match self {
      MocType::Ranges(ranges) => convert_to_u64::<T, Q, _, P>(ranges).into_range_moc(),
      MocType::Cells(cells) => {
        convert_to_u64::<T, Q, _, P>(cells.into_cell_moc_iter().ranges()).into_range_moc()
      }
    }
  }
}

#[derive(Debug)]
pub enum STMocType<T: Idx, R: BufRead> {
  V2(RangeMoc2DIterFromFits<T, R>),
  PreV2(RangeMoc2DPreV2IterFromFits<T, R>),
}
impl<T: Idx, R: BufRead> STMocType<T, R> {
  pub fn to_fits_ivoa<W: Write>(self, write: W) -> Result<(), FitsError> {
    match self {
      STMocType::V2(ranges2) => ranges2d_to_fits_ivoa(ranges2, None, None, write),
      STMocType::PreV2(ranges2) => ranges2d_to_fits_ivoa(ranges2, None, None, write),
    }
  }
}

// We could have avoided to create Cell and directly write uniq, to be changed later.
// we lazily re-use the Cell iterator made to save in JSON format.
/// To be compatible with version 1.0 of the MOC standard.
pub fn hpx_cells_to_fits_ivoa<T, I, W>(
  moc_it: I,
  moc_id: Option<String>,
  moc_type: Option<keywords::MocType>,
  mut writer: W,
) -> Result<(), FitsError>
where
  T: Idx,
  I: CellMOCIterator<T, Qty = Hpx<T>>,
  W: Write,
{
  let depth_max = moc_it.depth_max();
  let moc_kw_map = build_hpx_uniq_moc_keywords(depth_max, moc_id, moc_type, PhantomData::<T>);
  let mut buffers: Vec<Cursor<Vec<u8>>> = Vec::with_capacity((depth_max + 1) as usize);
  let n_cells_guess = moc_it.size_hint().0.max(10_000);
  for d in 0..=depth_max {
    // TODO: look at MOCs to find a better relation
    let size_guess = (n_cells_guess / 4_usize.pow((depth_max - d) as u32)).max(100);
    buffers.push(Cursor::new(Vec::with_capacity(size_guess)));
  }
  let mut n_cells = 0_u64;
  for e in moc_it {
    let d = e.depth;
    if d > depth_max {
      return Err(FitsError::UnexpectedDepth(d, depth_max));
    }
    e.uniq_hpx()
      .write::<_, BigEndian>(&mut buffers[d as usize])?;
    n_cells += 1;
  }
  write_fits_header(&mut writer, T::N_BYTES, n_cells, moc_kw_map)?;
  let mut len = 0;
  for buf in &mut buffers {
    // buf.seek(SeekFrom::Start(0))?; // or buf.set_position(0); ?
    let buf_ref = buf.get_ref();
    len += buf_ref.len();
    writer.write_all(buf_ref)?;
  }
  let mod2880 = len % 2880;
  if mod2880 != 0 {
    writer.write_all(&vec![0_u8; 2880 - mod2880])?;
  }
  Ok(())
}

fn build_hpx_uniq_moc_keywords<T: Idx>(
  depth_max: u8,
  moc_id: Option<String>,
  moc_type: Option<keywords::MocType>,
  _t_type: PhantomData<T>,
) -> MocKeywordsMap {
  let mut moc_kws = MocKeywordsMap::new();
  moc_kws.insert(MocKeywords::MOCVers(MocVers::V2_0));
  moc_kws.insert(MocKeywords::MOCDim(Hpx::<T>::MOC_DIM));
  moc_kws.insert(MocKeywords::Ordering(Ordering::Nuniq));
  moc_kws.insert(MocKeywords::CoordSys(CoordSys::ICRS));
  moc_kws.insert(MocKeywords::MOCOrdS(MocOrdS { depth: depth_max }));
  moc_kws.insert(MocKeywords::MOCOrder(MocOrder { depth: depth_max })); // For compatibility with v1
  if let Some(id) = moc_id {
    moc_kws.insert(MocKeywords::MOCId(MocId { id }));
  }
  moc_kws.insert(MocKeywords::MOCTool(MocTool {
    tool: String::from("CDS MOC Rust lib"),
  }));
  if let Some(mtype) = moc_type {
    moc_kws.insert(MocKeywords::MOCType(mtype));
  }
  // BINTABLE specific
  moc_kws.insert(MocKeywords::TForm1(T::TFORM));
  moc_kws.insert(MocKeywords::TType1(TType1 {
    ttype: String::from("UNIQ"),
  }));
  moc_kws
}

fn write_fits_header<R: Write>(
  mut writer: R,
  naxis1_n_bytes: u8,
  naxis2_n_elems: u64,
  moc_kw_map: MocKeywordsMap,
) -> Result<(), FitsError> {
  write_primary_hdu(&mut writer)?;
  let mut header_block = [b' '; 2880];
  let mut it = header_block.chunks_mut(80);
  // Write BINTABLE specific keywords in the buffer
  it.next().unwrap()[0..20].copy_from_slice(b"XTENSION= 'BINTABLE'");
  it.next().unwrap()[0..30].copy_from_slice(b"BITPIX  =                    8");
  it.next().unwrap()[0..30].copy_from_slice(b"NAXIS   =                    2");
  write_uint_mandatory_keyword_record(it.next().unwrap(), b"NAXIS1  ", naxis1_n_bytes as u64);
  write_uint_mandatory_keyword_record(it.next().unwrap(), b"NAXIS2  ", naxis2_n_elems);
  it.next().unwrap()[0..30].copy_from_slice(b"PCOUNT  =                    0");
  it.next().unwrap()[0..30].copy_from_slice(b"GCOUNT  =                    1");
  it.next().unwrap()[0..30].copy_from_slice(b"TFIELDS =                    1");
  // Write MOC (+BINTABLE) specific keywords in the buffer
  moc_kw_map.write_all(&mut it)?;
  it.next().unwrap()[0..3].copy_from_slice(b"END");
  // Do write the header
  writer.write_all(&header_block[..])?;
  Ok(())
}

/*
pub fn cells_to_fits_ivoa<T, I, W>(
  moc_it: I,
  moc_id: Option<String>,
  moc_type: Option<keywords::MocType>,
  mut writer: W
) -> Result<(), FitsError>
  where
    T: Idx,
    I: CellMOCIterator<T, Qty=Hpx<T>>,
    W: Write
{
  let depth_max = moc_it.depth_max();
  let moc_kw_map = build_hpx_uniq_moc_keywords(depth_max, moc_id, moc_type, PhantomData::<T>);
  match moc_it.size_hint() {
    (len_min, Some(len_max)) if len_min == len_max =>
      hpx_cells_to_fits_ivoa_internal(len_max, moc_it, moc_kw_map, writer),
    _ => {
      // We don't know the size, so we can't stream (since the size must be known in advance in FITS)
      let ranges: Vec<Range<T>> = moc_it.collect();
      hpx_cells_to_fits_ivoa_internal(ranges.len(), ranges.into_iter(), moc_kw_map, writer)
    },
  }
}
fn cells_to_fits_internal<T, I, W>(
  n_cells: usize,
  mut range_it: I,
  moc_kw_map: MocKeywordsMap,
  mut writer: W
) -> Result<(), FitsError>
  where
    T: Idx,
    I: Iterator<Item=Cell<T>>,
    W: Write
{
  write_fits_header(T::N_BYTES, n_cells as u64, moc_kw_map)?;
  // Write data part
  for _ in 0..n_ranges {
    if let Some(Range {start, end}) = range_it.next() {
      start.write::<_, BigEndian>(&mut writer)?;
    } else {
      return Err(FitsError::PrematureEndOfData);
    }
  }
  if range_it.next().is_some() {
    Err(FitsError::RemainingData)
  } else {
    Ok(())
  }
}*/

///
/// # Info
/// * If the number of elements in the iterator is not known in advance, a collect is necessary
/// * Compatible with the MOC2.0 IVOA standard
/// * For best performances when writing in a file, use a `BufWriter` in input.
pub fn ranges_to_fits_ivoa<T, Q, I, W>(
  moc_it: I,
  moc_id: Option<String>,
  moc_type: Option<keywords::MocType>,
  writer: W,
) -> Result<(), FitsError>
where
  T: Idx,
  Q: MocQty<T>,
  I: RangeMOCIterator<T, Qty = Q>,
  W: Write,
{
  let depth_max = moc_it.depth_max();
  let moc_kw_map = build_range_moc_keywords(
    depth_max,
    moc_id,
    moc_type,
    PhantomData::<T>,
    PhantomData::<Q>,
  );
  match moc_it.size_hint() {
    (len_min, Some(len_max)) if len_min == len_max => {
      ranges_to_fits_ivoa_internal(len_max, moc_it, moc_kw_map, writer)
    }
    _ => {
      // We don't know the size, so we can't stream (since the size must be known in advance in FITS)
      // Another solution would have been to write the stream in memory (counting the number of
      // elements written), and then to copy in the writer.
      let ranges: Vec<Range<T>> = moc_it.collect();
      ranges_to_fits_ivoa_internal(ranges.len(), ranges.into_iter(), moc_kw_map, writer)
    }
  }
}

fn build_range_moc_keywords<T: Idx, Q: MocQty<T>>(
  depth_max: u8,
  moc_id: Option<String>,
  moc_type: Option<keywords::MocType>,
  // moc_version: MocVers,
  _t_type: PhantomData<T>,
  _q_type: PhantomData<Q>,
) -> MocKeywordsMap {
  let mut moc_kws = MocKeywordsMap::new();
  moc_kws.insert(MocKeywords::MOCVers(MocVers::V2_0));
  // moc_kws.insert(MocKeywords::MOCVers(moc_version));
  moc_kws.insert(MocKeywords::MOCDim(Q::MOC_DIM));
  moc_kws.insert(MocKeywords::Ordering(Ordering::Range));
  if Q::HAS_COOSYS {
    moc_kws.insert(MocKeywords::CoordSys(CoordSys::ICRS));
    moc_kws.insert(MocKeywords::MOCOrdS(MocOrdS { depth: depth_max }));
  }
  if Q::HAS_TIMESYS {
    moc_kws.insert(MocKeywords::TimeSys(TimeSys::TCB));
    moc_kws.insert(MocKeywords::MOCOrdT(MocOrdT { depth: depth_max }));
  }
  if Q::HAS_FREQSYS {
    moc_kws.insert(MocKeywords::MOCOrdF(MocOrdF { depth: depth_max }));
  }
  if let Some(id) = moc_id {
    moc_kws.insert(MocKeywords::MOCId(MocId { id }));
  }
  moc_kws.insert(MocKeywords::MOCTool(MocTool {
    tool: String::from("CDS MOC Rust lib"),
  }));
  if let Some(mtype) = moc_type {
    moc_kws.insert(MocKeywords::MOCType(mtype));
  }
  // BINTABLE specific
  moc_kws.insert(MocKeywords::TForm1(T::TFORM));
  moc_kws.insert(MocKeywords::TType1(TType1 {
    ttype: String::from("RANGE"),
  }));
  // No ttype
  moc_kws
}

fn ranges_to_fits_ivoa_internal<T, I, W>(
  n_ranges: usize,
  mut range_it: I,
  moc_kw_map: MocKeywordsMap,
  mut writer: W,
) -> Result<(), FitsError>
where
  T: Idx,
  I: Iterator<Item = Range<T>>,
  W: Write,
{
  write_fits_header(&mut writer, T::N_BYTES, (n_ranges as u64) << 1, moc_kw_map)?;
  // Write data part
  for _ in 0..n_ranges {
    if let Some(Range { start, end }) = range_it.next() {
      start.write::<_, BigEndian>(&mut writer)?;
      end.write::<_, BigEndian>(&mut writer)?;
    } else {
      return Err(FitsError::PrematureEndOfData);
    }
  }
  // Complete FITS block of 2880 bytes
  let mod2880 = ((n_ranges << 1) * T::N_BYTES as usize) % 2880;
  if mod2880 != 0 {
    writer.write_all(&vec![0_u8; 2880 - mod2880])?;
  }
  // Ensure no more data in the iterator
  if range_it.next().is_some() {
    Err(FitsError::RemainingData)
  } else {
    Ok(())
  }
}

pub fn rangemoc2d_to_fits_ivoa<T: Idx, W: Write>(
  moc: &RangeMOC2<T, Time<T>, T, Hpx<T>>,
  moc_id: Option<String>,
  moc_type: Option<keywords::MocType>,
  mut writer: W,
) -> Result<(), FitsError> {
  let depth_max_time = moc.depth_max_1();
  let depth_max_hpx = moc.depth_max_2();
  let n_ranges = moc.compute_n_ranges();
  let moc_kw_map = build_range_moc2d_keywords(
    depth_max_time,
    depth_max_hpx,
    moc_id,
    moc_type,
    PhantomData::<T>,
  );
  write_fits_header(&mut writer, T::N_BYTES, n_ranges << 1, moc_kw_map)?;
  let n_ranges_written = write_ranges2d_data(moc.into_range_moc2_iter(), writer)?;
  if n_ranges != n_ranges_written as u64 {
    Err(FitsError::UnexpectedWrittenSize)
  } else {
    Ok(())
  }
}

///
/// # Info
/// * If the number of elements in the iterator is not known in advance, a collect is necessary
/// * Compatible with the MOC2.0 IVOA standard
/// * For best performances when writing in a file, use a `BufWriter` in input.
pub fn ranges2d_to_fits_ivoa<T, I, J, K, L, W>(
  moc_it: L,
  moc_id: Option<String>,
  moc_type: Option<keywords::MocType>,
  mut writer: W,
) -> Result<(), FitsError>
where
  T: Idx,
  I: RangeMOCIterator<T, Qty = Time<T>>,
  J: RangeMOCIterator<T, Qty = Hpx<T>>,
  K: RangeMOC2ElemIt<T, Time<T>, T, Hpx<T>, It1 = I, It2 = J>,
  L: RangeMOC2Iterator<T, Time<T>, I, T, Hpx<T>, J, K>,
  W: Write,
{
  let depth_max_time = moc_it.depth_max_1();
  let depth_max_hpx = moc_it.depth_max_2();
  let moc_kw_map = build_range_moc2d_keywords(
    depth_max_time,
    depth_max_hpx,
    moc_id,
    moc_type,
    PhantomData::<T>,
  );
  let mut mem_writter: Vec<u8> = Vec::with_capacity(1024); // 1kB
  let n_ranges_written = write_ranges2d_data(moc_it, &mut mem_writter)?;
  write_fits_header(
    &mut writer,
    T::N_BYTES,
    (n_ranges_written as u64) << 1,
    moc_kw_map,
  )?;
  writer.write_all(&mem_writter)?;
  Ok(())
}

// Returns the number of elements written
fn write_ranges2d_data<T, I, J, K, L, W>(moc2_it: L, mut writer: W) -> Result<usize, FitsError>
where
  T: Idx,
  I: RangeMOCIterator<T, Qty = Time<T>>,
  J: RangeMOCIterator<T, Qty = Hpx<T>>,
  K: RangeMOC2ElemIt<T, Time<T>, T, Hpx<T>, It1 = I, It2 = J>,
  L: RangeMOC2Iterator<T, Time<T>, I, T, Hpx<T>, J, K>,
  W: Write,
{
  let mut n_ranges_written = 0_usize;
  for e in moc2_it {
    let (moc1_it, moc2_it) = e.range_mocs_it();
    // Write time ranges
    for Range { start, end } in moc1_it {
      (start | T::MSB_MASK).write::<_, BigEndian>(&mut writer)?;
      (end | T::MSB_MASK).write::<_, BigEndian>(&mut writer)?;
      n_ranges_written += 1;
    }
    // Write space ranges
    for Range { start, end } in moc2_it {
      (start).write::<_, BigEndian>(&mut writer)?;
      (end).write::<_, BigEndian>(&mut writer)?;
      n_ranges_written += 1;
    }
  }
  // Complete FITS block of 2880 bytes
  let mod2880 = ((n_ranges_written << 1) * T::N_BYTES as usize) % 2880;
  if mod2880 != 0 {
    writer.write_all(&vec![0_u8; 2880 - mod2880])?;
  }
  Ok(n_ranges_written)
}

// Only implemented for ST-MOC so far but easy to generalize.
// Same Idx type for both ime and Space.
fn build_range_moc2d_keywords<T: Idx>(
  depth_max_time: u8,
  depth_max_hpx: u8,
  moc_id: Option<String>,
  moc_type: Option<keywords::MocType>,
  _t_type: PhantomData<T>,
) -> MocKeywordsMap {
  let mut moc_kws = MocKeywordsMap::new();
  moc_kws.insert(MocKeywords::MOCVers(MocVers::V2_0));
  moc_kws.insert(MocKeywords::MOCDim(MocDim::TimeSpace)); // To be changed for generalization
  moc_kws.insert(MocKeywords::Ordering(Ordering::Range));
  moc_kws.insert(MocKeywords::CoordSys(CoordSys::ICRS));
  moc_kws.insert(MocKeywords::MOCOrdS(MocOrdS {
    depth: depth_max_hpx,
  }));
  moc_kws.insert(MocKeywords::TimeSys(TimeSys::TCB));
  moc_kws.insert(MocKeywords::MOCOrdT(MocOrdT {
    depth: depth_max_time,
  }));
  if let Some(id) = moc_id {
    moc_kws.insert(MocKeywords::MOCId(MocId { id }));
  }
  moc_kws.insert(MocKeywords::MOCTool(MocTool {
    tool: String::from("CDS MOC Rust lib"),
  }));
  if let Some(mtype) = moc_type {
    moc_kws.insert(MocKeywords::MOCType(mtype));
  }
  // BINTABLE specific
  moc_kws.insert(MocKeywords::TForm1(T::TFORM));
  // No ttype
  moc_kws
}

// FROM FITS

/// Load a MOC stored in a FITS file implementing the IVOA MOC standard.
/// # Params
/// * `reader`: the FITS file bytes reader
pub fn from_fits_ivoa<R: BufRead>(reader: R) -> Result<MocIdxType<R>, FitsError> {
  from_fits_ivoa_custom(reader, false)
}

// We do not support compressed MOCs
/// Load a MOC stored in a FITS file implementing the IVOA MOC standard, with a permissive
/// option for more flexibility (see WARNING).
///
/// # Params
/// * `reader`: the FITS file bytes reader
/// * `coosys_permissive`: if set to true, do not fail if COORDSYS != C
///   (made for Aladin Lite v3, to load MOCs associated to Galactic HiPS and to possibly
///   allow for planetary MOCs).
/// # WARNING
///   Spatial MOCs are supposed to be defined in the ICRS coordinate system only, see Tab. 3
///   of the MOC standard (https://www.ivoa.net/documents/MOC/20220317/REC-moc-2.0-20220317.pdf).
///   But, we may want to defined MOCs associated to planet coordinate systems and
///   MOC have been defined in the Galactic coordinate system in HiPS made of Galactic tiles
///   (https://www.ivoa.net/documents/HiPS/).
///   Operations between 2 MOCs defined from different coordinate systems should not be allowed,
///   so use MOCs loaded with `coosys_permissive = true` with caution (since we so far do not store
///   the coordinate system in MOC objects, so we cannot prevent operations between MOC based on
///   different coosys).
pub fn from_fits_ivoa_custom<R: BufRead>(
  mut reader: R,
  coosys_permissive: bool,
) -> Result<MocIdxType<R>, FitsError> {
  let mut header_block = [b' '; 2880];
  consume_primary_hdu(&mut reader, &mut header_block)?;
  // Read the extention HDU
  let mut it80 = next_36_chunks_of_80_bytes(&mut reader, &mut header_block)?;
  // See Table 10 and 17 in https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf
  check_keyword_and_val(it80.next().unwrap(), b"XTENSION", b"'BINTABLE'")?;
  check_keyword_and_val(it80.next().unwrap(), b"BITPIX  ", b"8")?;
  check_keyword_and_val(it80.next().unwrap(), b"NAXIS  ", b"2")?;
  let n_bytes = check_keyword_and_parse_uint_val::<u8>(it80.next().unwrap(), b"NAXIS1  ")?;
  let n_elems = check_keyword_and_parse_uint_val::<u64>(it80.next().unwrap(), b"NAXIS2 ")?;
  check_keyword_and_val(it80.next().unwrap(), b"PCOUNT  ", b"0")?;
  check_keyword_and_val(it80.next().unwrap(), b"GCOUNT  ", b"1")?;
  check_keyword_and_val(it80.next().unwrap(), b"TFIELDS ", b"1")?;
  // nbits = |BITPIX|xGCOUNTx(PCOUNT+NAXIS1xNAXIS2x...xNAXISn)
  // In our case (bitpix = 8, GCOUNT = 1, PCOUNT = 0) => nbytes = n_cells * size_of(T)
  // let data_size n_bytes as usize * n_cells as usize; // N_BYTES ok since BITPIX = 8
  // Read MOC keywords
  let mut moc_kws = MocKeywordsMap::new();
  'hr: loop {
    for kw_record in &mut it80 {
      // Parse only MOC related keywords and ignore others
      if let Some(mkw) = MocKeywords::is_moc_kw(kw_record) {
        if mkw.is_err() && common::get_keyword(kw_record) == b"COORDSYS" && coosys_permissive {
          continue;
        }
        if let Some(previous_mkw) = moc_kws.insert(mkw?) {
          // A FITS keyword MUST BE uniq (I may be more relax here, taking the last one and not complaining)
          // return Err(FitsError::MultipleKeyword(previous_mkw.keyword_str().to_string()))
          warn!(
            "Keyword '{}' found more than once in a same HDU! We use the first occurrence.",
            previous_mkw.keyword_str()
          );
          moc_kws.insert(previous_mkw);
        }
        // else keyword added without error
      } else if &kw_record[0..4] == b"END " {
        break 'hr;
      }
    }
    // Read next 2880 bytes
    it80 = next_36_chunks_of_80_bytes(&mut reader, &mut header_block)?;
  }
  // CREATE A GUNIQ => General UNIQ in which the order is the order at the maximum depth
  // (and does not depends on the depth).
  // CREATE A RUNIQ => same order as GUNIQ, but follow the multi-order map (or bmoc) numbering
  // BOTH allow for streaming (like range)!
  // => USE GUNIQ on u32 for Vizier Catalogues ;)
  // CREATE RMIXED
  // 0: runiq 1: borne inf (max depht) 2: borne sup (max depth) => very fast binary search :)
  // println!("{:?}", &moc_kws);
  match moc_kws.get::<MocVers>() {
    Some(MocKeywords::MOCVers(MocVers::V2_0)) => {
      match moc_kws.get::<MocDim>() {
        Some(MocKeywords::MOCDim(MocDim::Space)) => {
          let depth_max = match moc_kws.get::<MocOrdS>() {
            Some(MocKeywords::MOCOrdS(MocOrdS { depth })) => *depth,
            _ => match moc_kws.get::<MocOrder>() {
              Some(MocKeywords::MOCOrder(MocOrder { depth })) => {
                warn!("Keyword 'MOCORDER' deprecated in version 2.0. Use 'MOCORD_S' instead.");
                *depth
              }
              _ => return Err(FitsError::MissingKeyword(MocOrdS::keyword_string())),
            },
          };
          moc_kws.check_coordsys()?;
          match moc_kws.get::<Ordering>() {
            Some(MocKeywords::Ordering(Ordering::Nuniq)) => {
              load_s_moc_nuniq(reader, n_bytes, n_elems, depth_max, &moc_kws)
            }
            Some(MocKeywords::Ordering(Ordering::Range)) => {
              load_s_moc_range(reader, n_bytes, n_elems, depth_max, &moc_kws)
            }
            Some(MocKeywords::Ordering(Ordering::Range29)) => {
              Err(FitsError::UncompatibleKeywordContent(
                String::from("ORDERING  = 'RABGE29'"),
                String::from("MOCVERS= '2.0'"),
              ))
            }
            // ADD GUNIQ? RUNIQ? RMIXED?
            _ => Err(FitsError::MissingKeyword(Ordering::keyword_string())),
          }
        }
        Some(MocKeywords::MOCDim(MocDim::Time)) => {
          let depth_max = match moc_kws.get::<MocOrdT>() {
            Some(MocKeywords::MOCOrdT(MocOrdT { depth })) => *depth,
            _ => return Err(FitsError::MissingKeyword(MocOrder::keyword_string())),
          };
          match moc_kws.get::<Ordering>() {
            Some(MocKeywords::Ordering(Ordering::Nuniq)) => {
              Err(FitsError::UncompatibleKeywordContent(
                String::from("MOCDIM  = 'TIME'"),
                String::from("ORDERING= 'NUNIQ'"),
              ))
            }
            Some(MocKeywords::Ordering(Ordering::Range)) => {
              load_t_moc_range(reader, n_bytes, n_elems, depth_max, &moc_kws)
            }
            Some(MocKeywords::Ordering(Ordering::Range29)) => {
              Err(FitsError::UncompatibleKeywordContent(
                String::from("ORDERING  = 'RANGE29'"),
                String::from("MOCVERS= '2.0'"),
              ))
            }
            // ADD GUNIQ? RUNIQ? RMIXED?
            _ => Err(FitsError::MissingKeyword(Ordering::keyword_string())),
          }
        }
        Some(MocKeywords::MOCDim(MocDim::TimeSpace)) => {
          let depth_max_time = match moc_kws.get::<MocOrdT>() {
            Some(MocKeywords::MOCOrdT(MocOrdT { depth })) => *depth,
            _ => return Err(FitsError::MissingKeyword(MocOrdT::keyword_string())),
          };
          let depth_max_hpx = match moc_kws.get::<MocOrdS>() {
            Some(MocKeywords::MOCOrdS(MocOrdS { depth })) => *depth,
            _ => return Err(FitsError::MissingKeyword(MocOrdS::keyword_string())),
          };
          match moc_kws.get::<Ordering>() {
            Some(MocKeywords::Ordering(Ordering::Nuniq)) => {
              Err(FitsError::UncompatibleKeywordContent(
                String::from("MOCDIM  = 'TIME.SPACE'"),
                String::from("ORDERING= 'NUNIQ'"),
              ))
            }
            Some(MocKeywords::Ordering(Ordering::Range)) => load_st_moc_range(
              reader,
              n_bytes,
              n_elems,
              depth_max_time,
              depth_max_hpx,
              &moc_kws,
            ),
            Some(MocKeywords::Ordering(Ordering::Range29)) => {
              Err(FitsError::UncompatibleKeywordContent(
                String::from("ORDERING  = 'RANGE29'"),
                String::from("MOCVERS= '2.0'"),
              ))
            }
            // ADD GUNIQ? RUNIQ? RMIXED?
            _ => Err(FitsError::MissingKeyword(Ordering::keyword_string())),
          }
        }
        Some(MocKeywords::MOCDim(MocDim::Frequency)) => {
          let depth_max = match moc_kws.get::<MocOrdF>() {
            Some(MocKeywords::MOCOrdF(MocOrdF { depth })) => *depth,
            _ => return Err(FitsError::MissingKeyword(MocOrder::keyword_string())),
          };
          match moc_kws.get::<Ordering>() {
            Some(MocKeywords::Ordering(Ordering::Nuniq)) => {
              Err(FitsError::UncompatibleKeywordContent(
                String::from("MOCDIM  = 'FREQUENCY'"),
                String::from("ORDERING= 'NUNIQ'"),
              ))
            }
            Some(MocKeywords::Ordering(Ordering::Range)) => {
              load_f_moc_range(reader, n_bytes, n_elems, depth_max, &moc_kws)
            }
            Some(MocKeywords::Ordering(Ordering::Range29)) => {
              Err(FitsError::UncompatibleKeywordContent(
                String::from("ORDERING  = 'RANGE29'"),
                String::from("MOCVERS= '2.0'"),
              ))
            }
            // ADD GUNIQ? RUNIQ? RMIXED?
            _ => Err(FitsError::MissingKeyword(Ordering::keyword_string())),
          }
        }
        _ => Err(FitsError::MissingKeyword(MocDim::keyword_string())),
      }
    }
    _ => {
      // MOC v1.0 => SMOC only (or ST-MOC pre v2.0)
      let depth_max = match moc_kws.get::<MocOrder>() {
        Some(MocKeywords::MOCOrder(MocOrder { depth })) => *depth,
        _ => return Err(FitsError::MissingKeyword(MocOrder::keyword_string())),
      };
      match moc_kws.get::<Ordering>() {
        Some(MocKeywords::Ordering(Ordering::Nuniq)) => {
          load_s_moc_nuniq(reader, n_bytes, n_elems, depth_max, &moc_kws)
        }
        Some(MocKeywords::Ordering(Ordering::Range)) => {
          load_s_moc_range(reader, n_bytes, n_elems, depth_max, &moc_kws)
        }
        Some(MocKeywords::Ordering(Ordering::Range29)) => {
          let (depth_max_time, depth_max_hpx) =
            match (moc_kws.get::<MocOrdT>(), moc_kws.get::<MocOrdS>()) {
              (None, Some(MocKeywords::MOCOrdS(MocOrdS { depth }))) => (depth_max << 1, *depth),
              (Some(MocKeywords::MOCOrdT(MocOrdT { depth })), None) => ((*depth) << 1, depth_max),
              (
                Some(MocKeywords::MOCOrdT(MocOrdT { depth: tdepth })),
                Some(MocKeywords::MOCOrdS(MocOrdS { depth: sdepth })),
              ) => ((*tdepth) << 1, *sdepth),
              _ => {
                return Err(FitsError::MissingKeyword(String::from(
                  "MOCORD_1 or TORDER",
                )))
              }
            };
          load_st_moc_range29(
            reader,
            n_bytes,
            n_elems,
            depth_max_time,
            depth_max_hpx,
            &moc_kws,
          )
        }
        _ => Err(FitsError::MissingKeyword(Ordering::keyword_string())),
      }
    }
  }
}

fn load_s_moc_nuniq<R: BufRead>(
  reader: R,
  n_bytes: u8,
  n_elems: u64,
  depth_max: u8,
  moc_kws: &MocKeywordsMap,
) -> Result<MocIdxType<R>, FitsError> {
  match (moc_kws.get::<TForm1>(), n_bytes) {
    (Some(MocKeywords::TForm1(TForm1::OneI)), u16::N_BYTES) => {
      Ok(MocIdxType::U16(MocQtyType::Hpx(MocType::Cells(
        from_fits_nuniq::<u16, R>(reader, depth_max, n_elems as usize)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneJ)), u32::N_BYTES) => {
      Ok(MocIdxType::U32(MocQtyType::Hpx(MocType::Cells(
        from_fits_nuniq::<u32, R>(reader, depth_max, n_elems as usize)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneK)), u64::N_BYTES) => {
      Ok(MocIdxType::U64(MocQtyType::Hpx(MocType::Cells(
        from_fits_nuniq::<u64, R>(reader, depth_max, n_elems as usize)?,
      ))))
    }
    (Some(MocKeywords::TForm1(tform)), nb) => Err(FitsError::UncompatibleKeywordContent(
      format!("TFORM1  = {}", nb),
      tform.to_string(),
    )),
    (None, _) => Err(FitsError::MissingKeyword(TForm1::keyword_string())),
    (_, _) => unreachable!(), // Except if a bug in the code, we are sure to get a TForm1
  }
}

fn load_s_moc_range<R: BufRead>(
  reader: R,
  n_bytes: u8,
  n_elems: u64,
  depth_max: u8,
  moc_kws: &MocKeywordsMap,
) -> Result<MocIdxType<R>, FitsError> {
  match (moc_kws.get::<TForm1>(), n_bytes) {
    (Some(MocKeywords::TForm1(TForm1::OneI)), u16::N_BYTES) => {
      Ok(MocIdxType::U16(MocQtyType::Hpx(MocType::Ranges(
        from_fits_range::<u16, Hpx<u16>, R>(reader, depth_max, n_elems >> 1)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneJ)), u32::N_BYTES) => {
      Ok(MocIdxType::U32(MocQtyType::Hpx(MocType::Ranges(
        from_fits_range::<u32, Hpx<u32>, R>(reader, depth_max, n_elems >> 1)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneK)), u64::N_BYTES) => {
      Ok(MocIdxType::U64(MocQtyType::Hpx(MocType::Ranges(
        from_fits_range::<u64, Hpx<u64>, R>(reader, depth_max, n_elems >> 1)?,
      ))))
    }
    (Some(MocKeywords::TForm1(tform)), nb) => Err(FitsError::UncompatibleKeywordContent(
      format!("NAXIS1  = {}", nb),
      tform.to_string(),
    )),
    (None, _) => Err(FitsError::MissingKeyword(TForm1::keyword_string())),
    (_, _) => unreachable!(), // Except if a bug in the code, we are sure to get a TForm1
  }
}

fn load_t_moc_range<R: BufRead>(
  reader: R,
  n_bytes: u8,
  n_elems: u64,
  depth_max: u8,
  moc_kws: &MocKeywordsMap,
) -> Result<MocIdxType<R>, FitsError> {
  let n_ranges = n_elems >> 1;
  match (moc_kws.get::<TForm1>(), n_bytes) {
    (Some(MocKeywords::TForm1(TForm1::OneI)), u16::N_BYTES) => {
      Ok(MocIdxType::U16(MocQtyType::Time(MocType::Ranges(
        from_fits_range::<u16, Time<u16>, R>(reader, depth_max, n_ranges)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneJ)), u32::N_BYTES) => {
      Ok(MocIdxType::U32(MocQtyType::Time(MocType::Ranges(
        from_fits_range::<u32, Time<u32>, R>(reader, depth_max, n_ranges)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneK)), u64::N_BYTES) => {
      Ok(MocIdxType::U64(MocQtyType::Time(MocType::Ranges(
        from_fits_range::<u64, Time<u64>, R>(reader, depth_max, n_ranges)?,
      ))))
    }
    /*(Some(MocKeywords::TForm1(TForm1::OneB)), u8::N_BYTES) =>
      Ok(MocIdxType::U8(MocQtyType::Time(MocType::Ranges(
        from_fits_range::<u8, Time::<u8>, R>(reader, depth_max, n_ranges)?
      )))),
    (Some(MocKeywords::TForm1(TForm1::TwoK)), u128::N_BYTES) =>
      Ok(MocIdxType::U128(MocQtyType::Time(MocType::Ranges(
        from_fits_range::<u128, Time::<u128>, R>(reader, depth_max, n_ranges)?
      )))),*/
    (Some(MocKeywords::TForm1(tform)), nb) => Err(FitsError::UncompatibleKeywordContent(
      format!("NAXIS1  = {}", nb),
      tform.to_string(),
    )),
    (None, _) => Err(FitsError::MissingKeyword(TForm1::keyword_string())),
    _ => unreachable!(),
  }
}

fn load_f_moc_range<R: BufRead>(
  reader: R,
  n_bytes: u8,
  n_elems: u64,
  depth_max: u8,
  moc_kws: &MocKeywordsMap,
) -> Result<MocIdxType<R>, FitsError> {
  let n_ranges = n_elems >> 1;
  match (moc_kws.get::<TForm1>(), n_bytes) {
    (Some(MocKeywords::TForm1(TForm1::OneI)), u16::N_BYTES) => {
      Ok(MocIdxType::U16(MocQtyType::Freq(MocType::Ranges(
        from_fits_range::<u16, Frequency<u16>, R>(reader, depth_max, n_ranges)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneJ)), u32::N_BYTES) => {
      Ok(MocIdxType::U32(MocQtyType::Freq(MocType::Ranges(
        from_fits_range::<u32, Frequency<u32>, R>(reader, depth_max, n_ranges)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneK)), u64::N_BYTES) => {
      Ok(MocIdxType::U64(MocQtyType::Freq(MocType::Ranges(
        from_fits_range::<u64, Frequency<u64>, R>(reader, depth_max, n_ranges)?,
      ))))
    }
    /*(Some(MocKeywords::TForm1(TForm1::OneB)), u8::N_BYTES) =>
      Ok(MocIdxType::U8(MocQtyType::Time(MocType::Ranges(
        from_fits_range::<u8, Time::<u8>, R>(reader, depth_max, n_ranges)?
      )))),
    (Some(MocKeywords::TForm1(TForm1::TwoK)), u128::N_BYTES) =>
      Ok(MocIdxType::U128(MocQtyType::Time(MocType::Ranges(
        from_fits_range::<u128, Time::<u128>, R>(reader, depth_max, n_ranges)?
      )))),*/
    (Some(MocKeywords::TForm1(tform)), nb) => Err(FitsError::UncompatibleKeywordContent(
      format!("NAXIS1  = {}", nb),
      tform.to_string(),
    )),
    (None, _) => Err(FitsError::MissingKeyword(TForm1::keyword_string())),
    _ => unreachable!(),
  }
}

fn load_st_moc_range<R: BufRead>(
  reader: R,
  n_bytes: u8,
  n_elems: u64,
  depth_max_time: u8,
  depth_max_hpx: u8,
  moc_kws: &MocKeywordsMap,
) -> Result<MocIdxType<R>, FitsError> {
  let n_ranges = n_elems >> 1;
  match (moc_kws.get::<TForm1>(), n_bytes) {
    (Some(MocKeywords::TForm1(TForm1::OneI)), u16::N_BYTES) => {
      Ok(MocIdxType::U16(MocQtyType::TimeHpx(STMocType::V2(
        from_fits_range2d::<u16, _>(reader, depth_max_time, depth_max_hpx, n_ranges)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneJ)), u32::N_BYTES) => {
      Ok(MocIdxType::U32(MocQtyType::TimeHpx(STMocType::V2(
        from_fits_range2d::<u32, _>(reader, depth_max_time, depth_max_hpx, n_ranges)?,
      ))))
    }
    (Some(MocKeywords::TForm1(TForm1::OneK)), u64::N_BYTES) => {
      Ok(MocIdxType::U64(MocQtyType::TimeHpx(STMocType::V2(
        from_fits_range2d::<u64, _>(reader, depth_max_time, depth_max_hpx, n_ranges)?,
      ))))
    }
    /*(Some(MocKeywords::TForm1(TForm1::OneB)), u8::N_BYTES) =>
      Ok(MocIdxType::U8(MocQtyType::TimeHpx(STMocType::V2(
        from_fits_range2d::<u8, _>(reader, depth_max_time, depth_max_hpx, n_ranges)?
      )))),
    (Some(MocKeywords::TForm1(TForm1::TwoK)), u128::N_BYTES) =>
      Ok(MocIdxType::U128(MocQtyType::TimeHpx(STMocType::V2(
        from_fits_range2d::<u128, _>(reader, depth_max_time, depth_max_hpx, n_ranges)?
      )))),*/
    (Some(MocKeywords::TForm1(tform)), nb) => Err(FitsError::UncompatibleKeywordContent(
      format!("NAXIS1  = {}", nb),
      tform.to_string(),
    )),
    (None, _) => Err(FitsError::MissingKeyword(TForm1::keyword_string())),
    _ => unreachable!(),
  }
}

fn load_st_moc_range29<R: BufRead>(
  reader: R,
  n_bytes: u8,
  n_elems: u64,
  depth_max_time: u8,
  depth_max_hpx: u8,
  moc_kws: &MocKeywordsMap,
) -> Result<MocIdxType<R>, FitsError> {
  let n_ranges = n_elems >> 1;
  match (moc_kws.get::<TForm1>(), n_bytes) {
    (Some(MocKeywords::TForm1(TForm1::OneK)), u64::N_BYTES) => {
      Ok(MocIdxType::U64(MocQtyType::TimeHpx(STMocType::PreV2(
        from_fits_range2d_29::<_>(reader, depth_max_time, depth_max_hpx, n_ranges)?,
      ))))
    }
    (Some(MocKeywords::TForm1(tform)), nb) => Err(FitsError::UncompatibleKeywordContent(
      format!("NAXIS1  = {}", nb),
      tform.to_string(),
    )),
    (None, _) => Err(FitsError::MissingKeyword(TForm1::keyword_string())),
    _ => unreachable!(),
  }
}

/// Official HEALPix Uniq numbering.
/// The file is sorted first by depth and then by cell number.
fn from_fits_nuniq<T, R>(
  mut reader: R,
  mut depth_max: u8,
  n_elems: usize,
) -> Result<CellMOC<T, Hpx<T>>, FitsError>
where
  T: Idx,
  R: BufRead,
{
  if depth_max > Hpx::<T>::MAX_DEPTH {
    warn!(
      "Wrong depth_max {}. Reset to {}",
      depth_max,
      Hpx::<T>::MAX_DEPTH
    );
    depth_max = Hpx::<T>::MAX_DEPTH;
  }
  let mut v: Vec<Cell<T>> = Vec::with_capacity(n_elems);
  for _ in 0..n_elems {
    let uniq = T::read::<_, BigEndian>(&mut reader)?;
    if uniq > T::zero() {
      // Bug in Aladin writting extra uniq of values set to 0!!
      v.push(Cell::from_uniq_hpx(uniq));
    }
  }
  v.sort_by(|a, b| a.flat_cmp::<Hpx<T>>(b));
  Ok(CellMOC::new(
    depth_max,
    MocCells::<T, Hpx<T>>::new(Cells::new(v)),
  ))
}

/// Generic numbering using a sentinel bit.
/// The file is sorted in a way which is independent of the depth of each cell so we can return
/// an iterator.
/// TODO: replace return type by an iterator
#[allow(dead_code)]
fn from_fits_guniq<T, Q, R>(
  mut reader: R,
  depth_max: u8,
  n_elems: usize,
) -> Result<CellMOC<T, Q>, FitsError>
where
  T: Idx,
  Q: MocQty<T>,
  R: BufRead,
{
  let mut v: Vec<Cell<T>> = Vec::with_capacity(n_elems);
  for _ in 0..n_elems {
    v.push(Cell::from_uniq::<Q>(T::read::<_, BigEndian>(&mut reader)?));
  }
  // Check is_sorted (a function already exists in nighlty rust)
  // v.sort_by(|a, b| a.cmp::<Q>(b));
  Ok(CellMOC::new(
    depth_max,
    MocCells::<T, Q>::new(Cells::new(v)),
  ))
}

fn from_fits_range<T, Q, R>(
  reader: R,
  depth_max: u8,
  n_ranges: u64,
) -> Result<RangeMocIterFromFits<T, Q, R>, FitsError>
where
  T: Idx,
  Q: MocQty<T>,
  R: BufRead,
{
  Ok(RangeMocIterFromFits::new(depth_max, reader, n_ranges))
}

#[derive(Debug)]
pub struct RangeMocIterFromFits<T: Idx, Q: MocQty<T>, R: BufRead> {
  depth_max: u8,
  reader: R,
  n_elems: u64,
  _t_type: PhantomData<T>,
  _t_qty: PhantomData<Q>,
}
impl<T: Idx, Q: MocQty<T>, R: BufRead> RangeMocIterFromFits<T, Q, R> {
  fn new(depth_max: u8, reader: R, n_elems: u64) -> Self {
    Self {
      depth_max,
      reader,
      n_elems,
      _t_type: PhantomData,
      _t_qty: PhantomData,
    }
  }
}
impl<T: Idx, Q: MocQty<T>, R: BufRead> HasMaxDepth for RangeMocIterFromFits<T, Q, R> {
  fn depth_max(&self) -> u8 {
    self.depth_max
  }
}
impl<T: Idx, Q: MocQty<T>, R: BufRead> ZSorted for RangeMocIterFromFits<T, Q, R> {}
impl<T: Idx, Q: MocQty<T>, R: BufRead> NonOverlapping for RangeMocIterFromFits<T, Q, R> {}
impl<T: Idx, Q: MocQty<T>, R: BufRead> MOCProperties for RangeMocIterFromFits<T, Q, R> {}
impl<T: Idx, Q: MocQty<T>, R: BufRead> Iterator for RangeMocIterFromFits<T, Q, R> {
  type Item = Range<T>; // Would de better to return a Result<Range, FitError>...
  fn next(&mut self) -> Option<Self::Item> {
    if self.n_elems > 0_u64 {
      let from = T::read::<_, BigEndian>(&mut self.reader);
      let to = T::read::<_, BigEndian>(&mut self.reader);
      if let (Ok(start), Ok(end)) = (from, to) {
        self.n_elems -= 1;
        Some(Range { start, end })
      } else {
        // Early stop due to read error. Better to return a Result!
        None
      }
    } else {
      None
    }
  }
  // Declaring size_hint, a 'collect' can directly allocate the right number of elements
  fn size_hint(&self) -> (usize, Option<usize>) {
    if self.n_elems > usize::MAX as u64 {
      (usize::MAX, None)
    } else {
      (self.n_elems as usize, Some(self.n_elems as usize))
    }
  }
}
impl<T: Idx, Q: MocQty<T>, R: BufRead> RangeMOCIterator<T> for RangeMocIterFromFits<T, Q, R> {
  type Qty = Q;

  fn peek_last(&self) -> Option<&Range<T>> {
    None
  }
}

// st-moc read iterator

fn from_fits_range2d<T, R>(
  reader: R,
  depth_max_time: u8,
  depth_max_hpx: u8,
  n_ranges: u64,
) -> Result<RangeMoc2DIterFromFits<T, R>, FitsError>
where
  T: Idx,
  R: BufRead,
{
  Ok(RangeMoc2DIterFromFits::new(
    depth_max_time,
    depth_max_hpx,
    reader,
    n_ranges,
  ))
}

#[derive(Debug)]
pub struct RangeMoc2DIterFromFits<T: Idx, R: BufRead> {
  depth_max_time: u8,
  depth_max_hpx: u8,
  reader: R,
  n_ranges: u64,
  _t_type: PhantomData<T>,
  prev_t: Option<Range<T>>,
}

impl<T: Idx, R: BufRead> RangeMoc2DIterFromFits<T, R> {
  fn new(
    depth_max_time: u8,
    depth_max_hpx: u8,
    reader: R,
    n_ranges: u64,
  ) -> RangeMoc2DIterFromFits<T, R> {
    RangeMoc2DIterFromFits {
      depth_max_time,
      depth_max_hpx,
      reader,
      n_ranges,
      _t_type: PhantomData,
      prev_t: None,
    }
  }
}
impl<T: Idx, R: BufRead> HasTwoMaxDepth for RangeMoc2DIterFromFits<T, R> {
  fn depth_max_1(&self) -> u8 {
    self.depth_max_time
  }
  fn depth_max_2(&self) -> u8 {
    self.depth_max_hpx
  }
}
impl<T: Idx, R: BufRead> ZSorted for RangeMoc2DIterFromFits<T, R> {}
impl<T: Idx, R: BufRead> NonOverlapping for RangeMoc2DIterFromFits<T, R> {}
impl<T: Idx, R: BufRead> MOC2Properties for RangeMoc2DIterFromFits<T, R> {}
impl<T: Idx, R: BufRead> Iterator for RangeMoc2DIterFromFits<T, R> {
  type Item = RangeMOC2Elem<T, Time<T>, T, Hpx<T>>;
  fn next(&mut self) -> Option<Self::Item> {
    let mut tranges: Vec<Range<T>> = Vec::with_capacity(1000);
    if let Some(trange) = self.prev_t.take() {
      tranges.push(trange);
    }
    let mut sranges: Vec<Range<T>> = Vec::with_capacity(1000);
    while self.n_ranges > 0_u64 {
      let from = T::read::<_, BigEndian>(&mut self.reader);
      let to = T::read::<_, BigEndian>(&mut self.reader);
      if let (Ok(start), Ok(end)) = (from, to) {
        self.n_ranges -= 1;
        if start & end & T::MSB_MASK == T::MSB_MASK {
          tranges.push(Range {
            start: start & !T::MSB_MASK,
            end: end & !T::MSB_MASK,
          })
        } else {
          sranges.push(Range { start, end });
          break;
        }
      } else {
        // Early stop due to read error. Better to return a Result!
        return None;
      }
    }
    while self.n_ranges > 0_u64 {
      let from = T::read::<_, BigEndian>(&mut self.reader);
      let to = T::read::<_, BigEndian>(&mut self.reader);
      if let (Ok(start), Ok(end)) = (from, to) {
        self.n_ranges -= 1;
        if start & end & T::MSB_MASK == T::MSB_MASK {
          self.prev_t = Some(Range {
            start: start & !T::MSB_MASK,
            end: end & !T::MSB_MASK,
          });
          break;
        } else {
          sranges.push(Range { start, end });
        }
      } else {
        // Early stop due to read error. Better to return a Result!
        return None;
      }
    }
    if tranges.is_empty() && sranges.is_empty() {
      None
    } else {
      Some(RangeMOC2Elem::new(
        RangeMOC::new(self.depth_max_time, MocRanges::new_unchecked(tranges)),
        RangeMOC::new(self.depth_max_hpx, MocRanges::new_unchecked(sranges)),
      ))
    }
  }
  // No size int because should be the number of RangeMOC2Elem instead of the
  // total number of ranges...
}
impl<T: Idx, R: BufRead>
  RangeMOC2Iterator<
    T,
    Time<T>,
    RangeMocIter<T, Time<T>>,
    T,
    Hpx<T>,
    RangeMocIter<T, Hpx<T>>,
    RangeMOC2Elem<T, Time<T>, T, Hpx<T>>,
  > for RangeMoc2DIterFromFits<T, R>
{
}

// st-moc pre_v2 read iterator

fn from_fits_range2d_29<R>(
  reader: R,
  depth_max_time: u8,
  depth_max_hpx: u8,
  n_ranges: u64,
) -> Result<RangeMoc2DPreV2IterFromFits<u64, R>, FitsError>
where
  R: BufRead,
{
  Ok(RangeMoc2DPreV2IterFromFits::new(
    depth_max_time,
    depth_max_hpx,
    reader,
    n_ranges,
  ))
}

#[derive(Debug)]
pub struct RangeMoc2DPreV2IterFromFits<T: Idx, R: BufRead> {
  depth_max_time: u8,
  depth_max_hpx: u8,
  reader: R,
  n_ranges: u64,
  prev_t: Option<Range<T>>,
}

impl<T: Idx, R: BufRead> RangeMoc2DPreV2IterFromFits<T, R> {
  fn new(
    depth_max_time: u8,
    depth_max_hpx: u8,
    reader: R,
    n_ranges: u64,
  ) -> RangeMoc2DPreV2IterFromFits<T, R> {
    RangeMoc2DPreV2IterFromFits {
      depth_max_time,
      depth_max_hpx,
      reader,
      n_ranges,
      prev_t: None,
    }
  }
}
impl<T: Idx, R: BufRead> HasTwoMaxDepth for RangeMoc2DPreV2IterFromFits<T, R> {
  fn depth_max_1(&self) -> u8 {
    self.depth_max_time
  }
  fn depth_max_2(&self) -> u8 {
    self.depth_max_hpx
  }
}
impl<T: Idx, R: BufRead> ZSorted for RangeMoc2DPreV2IterFromFits<T, R> {}
impl<T: Idx, R: BufRead> NonOverlapping for RangeMoc2DPreV2IterFromFits<T, R> {}
impl<T: Idx, R: BufRead> MOC2Properties for RangeMoc2DPreV2IterFromFits<T, R> {}
impl<T: Idx, R: BufRead> Iterator for RangeMoc2DPreV2IterFromFits<T, R> {
  type Item = RangeMOC2Elem<T, Time<T>, T, Hpx<T>>;
  fn next(&mut self) -> Option<Self::Item> {
    use byteorder::ReadBytesExt;

    let mut tranges: Vec<Range<T>> = Vec::with_capacity(1000);
    if let Some(trange) = self.prev_t.take() {
      tranges.push(trange);
    }
    let mut sranges: Vec<Range<T>> = Vec::with_capacity(1000);
    while self.n_ranges > 0_u64 {
      let from = self.reader.read_i64::<BigEndian>(); // i64::read::<_, BigEndian>(&mut self.reader);
      let to = self.reader.read_i64::<BigEndian>(); // i64::read::<_, BigEndian>(&mut self.reader);
      if let (Ok(start), Ok(end)) = (from, to) {
        self.n_ranges -= 1;
        if start < 0 && end < 0 {
          tranges.push(Range {
            start: T::from_u64_idx(-start as u64),
            end: T::from_u64_idx(-end as u64),
          })
        } else {
          sranges.push(Range {
            start: T::from_u64_idx(start as u64),
            end: T::from_u64_idx(end as u64),
          });
          break;
        }
      } else {
        // Early stop due to read error. Better to return a Result!
        return None;
      }
    }
    while self.n_ranges > 0_u64 {
      let from = self.reader.read_i64::<BigEndian>(); // i64::read::<_, BigEndian>(&mut self.reader);
      let to = self.reader.read_i64::<BigEndian>(); // i64::read::<_, BigEndian>(&mut self.reader);
      if let (Ok(start), Ok(end)) = (from, to) {
        self.n_ranges -= 1;
        if start < 0 && end < 0 {
          self.prev_t = Some(Range {
            start: T::from_u64_idx(-start as u64),
            end: T::from_u64_idx(-end as u64),
          });
          break;
        } else {
          sranges.push(Range {
            start: T::from_u64_idx(start as u64),
            end: T::from_u64_idx(end as u64),
          });
        }
      } else {
        // Early stop due to read error. Better to return a Result!
        return None;
      }
    }
    if tranges.is_empty() && sranges.is_empty() {
      None
    } else {
      Some(RangeMOC2Elem::new(
        RangeMOC::new(self.depth_max_time, MocRanges::new_unchecked(tranges)),
        RangeMOC::new(self.depth_max_hpx, MocRanges::new_unchecked(sranges)),
      ))
    }
  }
  // No size int because should be the number of RangeMOC2Elem instead of the
  // total number of ranges...
}
impl<T: Idx, R: BufRead>
  RangeMOC2Iterator<
    T,
    Time<T>,
    RangeMocIter<T, Time<T>>,
    T,
    Hpx<T>,
    RangeMocIter<T, Hpx<T>>,
    RangeMOC2Elem<T, Time<T>, T, Hpx<T>>,
  > for RangeMoc2DPreV2IterFromFits<T, R>
{
}

#[cfg(test)]
mod tests {

  use std::fs::File;
  use std::io::{BufReader, BufWriter};
  use std::ops::Range;
  use std::path::PathBuf;

  use crate::deser::fits::{
    from_fits_ivoa, hpx_cells_to_fits_ivoa, rangemoc2d_to_fits_ivoa, ranges_to_fits_ivoa,
    FitsError, MocIdxType, MocQtyType, MocType, STMocType,
  };
  use crate::elem::cell::Cell;
  use crate::elemset::{
    cell::{Cells, MocCells},
    range::{HpxRanges, MocRanges, TimeRanges},
  };
  use crate::moc::{
    cell::CellMOC, range::RangeMOC, CellMOCIntoIterator, HasMaxDepth, RangeMOCIntoIterator,
  };
  use crate::moc2d::{
    range::{RangeMOC2, RangeMOC2Elem},
    HasTwoMaxDepth, RangeMOC2ElemIt,
  };
  use crate::qty::{Hpx, Time};

  #[test]
  fn test_err() {
    let buff = [0_u8; 10];
    let reader = BufReader::new(&buff[..]);
    match from_fits_ivoa(reader) {
      Err(e) => assert!(matches!(e, FitsError::Io(..))),
      _ => assert!(false),
    }
  }

  #[test]
  fn test_read_v2_smoc_uniq_fits() {
    let path_buf1 = PathBuf::from("resources/MOC2.0/GW190425.fits");
    let path_buf2 = PathBuf::from("../resources/MOC2.0/GW190425.fits");
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    match from_fits_ivoa(reader) {
      Ok(MocIdxType::U32(MocQtyType::Hpx(MocType::Cells(moc1)))) => {
        assert_eq!(moc1.depth_max(), 8);
        assert_eq!(875, moc1.len());
      }
      Err(e) => println!("{}", e),
      _ => assert!(false),
    }
  }

  #[test]
  fn test_read_v1_smoc_fits() {
    let path_buf1 = PathBuf::from("resources/MOC2.0/SMOC_test.fits");
    let path_buf2 = PathBuf::from("../resources/MOC2.0/SMOC_test.fits");
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    match from_fits_ivoa(reader) {
      Ok(MocIdxType::U64(MocQtyType::Hpx(MocType::Cells(moc1)))) => {
        assert_eq!(moc1.depth_max(), 29);
        assert_eq!(moc1.len(), 10);
        let mut vec_cells: Vec<Cell<u64>> = vec![
          Cell::from_uniq_hpx(259_u64),
          Cell::from_uniq_hpx(266_u64),
          Cell::from_uniq_hpx(1040_u64),
          Cell::from_uniq_hpx(1041_u64),
          Cell::from_uniq_hpx(1042_u64),
          Cell::from_uniq_hpx(1046_u64),
          Cell::from_uniq_hpx(4115_u64),
          Cell::from_uniq_hpx(4116_u64),
          Cell::from_uniq_hpx(68719476958_u64),
          Cell::from_uniq_hpx(288230376275168533_u64),
        ];
        vec_cells.sort_by(|a, b| a.flat_cmp::<Hpx<u64>>(&b));
        let moc2 =
          CellMOC::<u64, Hpx<u64>>::new(29, MocCells::new(Cells(vec_cells.into_boxed_slice())));
        for (c1, c2) in moc1.into_cell_moc_iter().zip(moc2.into_cell_moc_iter()) {
          assert_eq!(c1, c2);
        }
      }
      Err(e) => println!("{}", e),
      _ => assert!(false),
    }
  }

  #[test]
  fn test_read_v2_tmoc_fits() {
    let path_buf1 = PathBuf::from("resources/MOC2.0/TMOC_test.fits");
    let path_buf2 = PathBuf::from("../resources/MOC2.0/TMOC_test.fits");
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    match from_fits_ivoa(reader) {
      Ok(MocIdxType::U64(MocQtyType::Time(MocType::Ranges(mut moc)))) => {
        assert_eq!(moc.depth_max(), 35);
        assert_eq!(moc.size_hint(), (1, Some(1)));
        assert_eq!(
          moc.next(),
          Some(Range {
            start: 1073741824,
            end: 2684354560
          })
        );
        assert_eq!(moc.next(), None);
      }
      // Err(e) => println!("{}", e),
      _ => assert!(false),
    }
  }

  #[test]
  fn test_write_ranges_fits() {
    let ranges = vec![Range {
      start: 1073741824_u64,
      end: 2684354560_u64,
    }];
    let moc: RangeMOC<u64, Time<u64>> = RangeMOC::new(35, MocRanges::new_unchecked(ranges));
    let path_buf1 = PathBuf::from("resources/MOC2.0/TMOC_test.fits");
    let path_buf2 = PathBuf::from("../resources/MOC2.0/TMOC_test.fits");
    let file = File::create(&path_buf1)
      .or_else(|_| File::create(&path_buf2))
      .unwrap();
    let writer = BufWriter::new(file);
    // write: it only tests that no error occur while writing
    ranges_to_fits_ivoa(moc.into_range_moc_iter(), None, None, writer).unwrap();
  }

  #[test]
  fn test_write_cells_fits() {
    let moc = CellMOC::<u64, Hpx<u64>>::new(
      29,
      MocCells::new(Cells(
        vec![
          Cell::from_uniq_hpx(259_u64),
          Cell::from_uniq_hpx(266_u64),
          Cell::from_uniq_hpx(1040_u64),
          Cell::from_uniq_hpx(1041_u64),
          Cell::from_uniq_hpx(1042_u64),
          Cell::from_uniq_hpx(1046_u64),
          Cell::from_uniq_hpx(4115_u64),
          Cell::from_uniq_hpx(4116_u64),
          Cell::from_uniq_hpx(68719476958_u64),
          Cell::from_uniq_hpx(288230376275168533_u64),
        ]
        .into_boxed_slice(),
      )),
    );
    let path_buf1 = PathBuf::from("resources/MOC2.0/SMOC_test.fits");
    let path_buf2 = PathBuf::from("../resources/MOC2.0/SMOC_test.fits");
    let file = File::create(&path_buf1)
      .or_else(|_| File::create(&path_buf2))
      .unwrap();
    let writer = BufWriter::new(file);
    // write: it only tests that no error occur while writing
    hpx_cells_to_fits_ivoa(moc.into_cell_moc_iter(), None, None, writer).unwrap();
  }

  #[test]
  fn test_moc2d_to_fits() {
    let path_buf1 = PathBuf::from("resources/MOC2.0/STMOC.fits");
    let path_buf2 = PathBuf::from("../resources/MOC2.0/STMOC.fits");
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    let mut it = match from_fits_ivoa(reader).unwrap() {
      MocIdxType::U64(MocQtyType::TimeHpx(STMocType::V2(it))) => it,
      _ => unreachable!(),
    };
    assert_eq!(it.depth_max_1(), 61);
    assert_eq!(it.depth_max_2(), 4);
    /*for e in it {
      // RangeMOC2Elem<T, Time<T>, T, Hpx<T>>
      print!("t: ");
      let (moc_l_it, moc_r_it) = e.mocs_it();
      for Range{ start, end } in moc_l_it {
        print!(" {}-{} ", start, end);
      }
      println!("");
      print!("s: ");
      for Range {start, end } in moc_r_it {
        print!(" {}-{} ", start, end);
      }
      println!("");
    }*/
    /*
    t:  1-2  3-4  5-6
    s:  4503599627370496-18014398509481984
    t:  50-51  52-53
    s:  28147497671065600-29273397577908224
    t61/1 3 5 s3/1-3 t61/50 52 s4/25
    */
    let (mut t_it, mut s_it) = it.next().unwrap().range_mocs_it();
    assert_eq!(t_it.next(), Some(Range { start: 1, end: 2 }));
    assert_eq!(t_it.next(), Some(Range { start: 3, end: 4 }));
    assert_eq!(t_it.next(), Some(Range { start: 5, end: 6 }));
    assert_eq!(t_it.next(), None);
    assert_eq!(
      s_it.next(),
      Some(Range {
        start: 4503599627370496,
        end: 18014398509481984
      })
    );
    assert_eq!(s_it.next(), None);
    let (mut t_it, mut s_it) = it.next().unwrap().range_mocs_it();
    assert_eq!(t_it.next(), Some(Range { start: 50, end: 51 }));
    assert_eq!(t_it.next(), Some(Range { start: 52, end: 53 }));
    assert_eq!(t_it.next(), None);
    assert_eq!(
      s_it.next(),
      Some(Range {
        start: 28147497671065600,
        end: 29273397577908224
      })
    );
    assert_eq!(s_it.next(), None);
    assert!(it.next().is_none());
  }

  #[test]
  fn test_write_ranges2d_fits() {
    // Build moc
    let mut elems: Vec<RangeMOC2Elem<u64, Time<u64>, u64, Hpx<u64>>> = Default::default();
    elems.push(RangeMOC2Elem::new(
      RangeMOC::new(61, TimeRanges::new_unchecked(vec![1..2, 3..4, 5..6])),
      RangeMOC::new(
        4,
        HpxRanges::new_unchecked(vec![4503599627370496..18014398509481984]),
      ),
    ));
    elems.push(RangeMOC2Elem::new(
      RangeMOC::new(61, TimeRanges::new_unchecked(vec![50..51, 52..53])),
      RangeMOC::new(
        4,
        HpxRanges::new_unchecked(vec![28147497671065600..29273397577908224]),
      ),
    ));
    let moc2 = RangeMOC2::new(61, 4, elems);
    // Open file
    let path_buf1 = PathBuf::from("resources/MOC2.0/STMOC_test.fits");
    let path_buf2 = PathBuf::from("../resources/MOC2.0/STMOC_test.fits");
    let file = File::create(&path_buf1)
      .or_else(|_| File::create(&path_buf2))
      .unwrap();
    let writer = BufWriter::new(file);
    // write: it only tests that no error occur while writing
    rangemoc2d_to_fits_ivoa(&moc2, None, None, writer).unwrap();
  }
}
