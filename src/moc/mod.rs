//! A MOC is a set of ordered, non-overlapping MOC elements, associated to a maximum depth.

use std::{io::Write, marker::PhantomData, ops::Range};

use healpix::sph_geom::coo3d::UnitVect3;
use healpix::{
  nested::{
    self,
    bmoc::{BMOCBuilderUnsafe, BMOC},
  },
  sph_geom::coo3d::{vec3_of, LonLat, Vec3},
  Customf64,
};

use crate::{
  deser::{
    self,
    fits::{error::FitsError, hpx_cells_to_fits_ivoa, keywords, ranges_to_fits_ivoa},
  },
  elem::{cell::Cell, cellcellrange::CellOrCellRange, range::MocRange},
  idx::Idx,
  moc::{
    adapters::{
      CellMOCIteratorFromRanges, CellOrCellRangeMOCIteratorFromCells, DepthMaxCellsFromRanges,
      RangeMOCIteratorFromCellOrCellRanges, RangeMOCIteratorFromCells,
    },
    range::{
      op::{
        and::{and, AndRangeIter},
        check::{check, CheckedIterator},
        convert::{convert, ConvertIterator},
        degrade::{degrade, DegradeRangeIter},
        minus::{minus, MinusRangeIter},
        not::{not, NotRangeIter},
        or::{or, OrRangeIter},
        xor::{xor, XorRangeIter},
      },
      RangeMOC,
    },
  },
  qty::{Bounded, Hpx, MocQty},
};

pub mod adapters;
pub mod builder;
pub mod cell;
pub mod cellcellrange;
pub mod range;

/// Returns the maximum depth of an item the implementor contains.
pub trait HasMaxDepth {
  fn depth_max(&self) -> u8;
}

/// Marker trait telling that ranges lower bound or cells or indices in the implementor are sorted
/// according to the natural Z-order curve order (no hierarchical order => cells of various depth
/// are mixed).
pub trait ZSorted {}

/// Marker trait telling that something the implementor contains non-overlapping items.
pub trait NonOverlapping {}

/// Commodity trait containing all MOC properties
pub trait MOCProperties: HasMaxDepth + ZSorted + NonOverlapping {}

/// Iterator over MOC cells
pub trait CellMOCIterator<T: Idx>: Sized + MOCProperties + Iterator<Item = Cell<T>> {
  type Qty: MocQty<T>;

  /// If available, returns the upper cell the iterator will return, without consuming
  /// the iterator.
  /// This information is available e.g. for an Iterator from a Vector, but may not be available
  /// for an Iterator coming from a stream.
  /// If available, this information can be used for fast rejection tests.
  fn peek_last(&self) -> Option<&Cell<T>>;

  /// Returns the mean center of the MOC: `(lon, lat)` in radians.
  fn mean_center(self) -> (f64, f64) {
    let mut x = 0_f64;
    let mut y = 0_f64;
    let mut z = 0_f64;
    let depth_max = self.depth_max();
    for Cell { depth, idx } in self {
      let weight = (1_u64 << ((depth_max - depth) << 1)) as f64;
      let (lon, lat) = nested::center(depth, idx.to_u64());
      // println!("depth: {}; idx: {}; weight: {}: center: ({}, {})",
      //         depth, idx, weight, lon.to_degrees(), lat.to_degrees());
      let unit_vec = vec3_of(lon, lat);
      x += unit_vec.x() * weight; // we could optimize since multiplication by a power of 2
      y += unit_vec.y() * weight; // we could optimize since multiplication by a power of 2
      z += unit_vec.z() * weight; // we could optimize since multiplication by a power of 2
    }
    let norm = (x.pow2() + y.pow2() + z.pow2()).sqrt();
    let LonLat { lon, lat } = UnitVect3::new_unsafe(x / norm, y / norm, z / norm).lonlat();
    (lon, lat)
  }

  /// Returns the upper bound on the largest distance, in radians, from the given center to the MOC border.
  fn max_distance_from(self, from_lon: f64, from_lat: f64) -> f64 {
    /// Returns `(s/2)^2` with `s` the segment (i.e. the Euclidean distance) between
    /// the two given points  `P1` and `P2` on the unit-sphere.
    /// We recall that `s = 2 sin(ad/2)` with `ad` the angular distance between the two points.
    /// # Input
    /// - `dlon` the longitude difference, i.e. (P2.lon - P1.lon), in radians
    /// - `dlat` the latitude difference, i.e. (P2.lat - P1.lat), in radians
    /// - `cos_lat1` cosine of the latitude of the first point
    /// - `cos_lat2` cosine of the latitude of the second point
    fn squared_half_segment(dlon: f64, dlat: f64, cos_lat1: f64, cos_lat2: f64) -> f64 {
      dlat.half().sin().pow2() + cos_lat1 * cos_lat2 * dlon.half().sin().pow2()
    }
    // let dmax_center_to_vertex = healpix::largest_center_to_vertex_distance(self.depth_max(), from_lon, from_lat);
    let cos_from_lat = from_lat.cos();
    let shs_max = self
      .flat_map(move |Cell { depth, idx }| {
        nested::vertices(depth, idx.to_u64()).map(|(lon, lat)| {
          squared_half_segment(lon - from_lon, lat - from_lat, cos_from_lat, lat.cos())
        })
      })
      .fold(0_f64, f64::max);
    shs_max.sqrt().asin().twice() // + dmax_center_to_vertex
  }

  fn cellranges(self) -> CellOrCellRangeMOCIteratorFromCells<T, Self::Qty, Self> {
    CellOrCellRangeMOCIteratorFromCells::new(self)
  }

  fn ranges(self) -> RangeMOCIteratorFromCells<T, Self::Qty, Self> {
    let last: Option<Range<T>> = self
      .peek_last()
      .map(|cell| MocRange::<T, Self::Qty>::from(cell).0);
    RangeMOCIteratorFromCells::new(self, last)
  }

  fn to_json_aladin<W: Write>(self, fold: Option<usize>, writer: W) -> std::io::Result<()> {
    deser::json::to_json_aladin(self, &fold, "", writer)
  }
}

pub trait CellHpxMOCIterator<T: Idx>: CellMOCIterator<T, Qty = Hpx<T>> {
  fn hpx_cells_to_fits_ivoa<W: Write>(
    self,
    moc_id: Option<String>,
    moc_type: Option<keywords::MocType>,
    writer: W,
  ) -> Result<(), FitsError> {
    hpx_cells_to_fits_ivoa(self, moc_id, moc_type, writer)
  }
}
/// All types that implement `CellMOCIterator<T, Qty = Hpx<T>>` get methods defined in
/// `CellHpxMOCIterator` for free.
impl<T: Idx, I: CellMOCIterator<T, Qty = Hpx<T>>> CellHpxMOCIterator<T> for I {}

pub trait IntoBMOC<T: Idx + Into<u64>>: CellMOCIterator<T> {
  fn into_bmoc(self) -> BMOC {
    let (_, upper_bound) = self.size_hint();
    let mut builder = BMOCBuilderUnsafe::new(self.depth_max(), upper_bound.unwrap_or(1000));
    for Cell { depth, idx } in self {
      builder.push(depth, idx.into(), true);
    }
    builder.to_bmoc()
  }
}
impl<T: Idx + Into<u64>, U: CellMOCIterator<T>> IntoBMOC<T> for U {}

/// Transform an object into an iterator over MOC cells.
pub trait CellMOCIntoIterator<T: Idx>: Sized {
  type Qty: MocQty<T>;
  type IntoCellMOCIter: CellMOCIterator<T, Qty = Self::Qty>;

  fn into_cell_moc_iter(self) -> Self::IntoCellMOCIter;
}

pub trait CellOrCellRangeMOCIterator<T: Idx>:
  Sized + MOCProperties + Iterator<Item = CellOrCellRange<T>>
{
  type Qty: MocQty<T>;

  /// If available, returns the upper cell or cell range the iterator will return,
  /// without consuming the iterator.
  /// This information is available e.g. for an Iterator from a Vector, but may not be available
  /// for an Iterator coming from a stream.
  /// If available, this information can be used for fast rejection tests.
  fn peek_last(&self) -> Option<&CellOrCellRange<T>>;

  /// # WARNING
  /// - `use_offset=true` is not compatible with the current IVOA standard!
  fn to_ascii_ivoa<W: Write>(
    self,
    fold: Option<usize>,
    use_offset: bool,
    writer: W,
  ) -> std::io::Result<()> {
    deser::ascii::to_ascii_ivoa(self, &fold, use_offset, writer)
  }
  /// # WARNING
  /// - this is not compatible with the current IVOA standard!
  fn to_ascii_stream<W: Write>(self, use_offset: bool, writer: W) -> std::io::Result<()> {
    deser::ascii::to_ascii_stream(self, use_offset, writer)
  }

  fn ranges(self) -> RangeMOCIteratorFromCellOrCellRanges<T, Self::Qty, Self> {
    let last: Option<Range<T>> = self
      .peek_last()
      .map(|ccr| MocRange::<T, Self::Qty>::from(ccr).0);
    RangeMOCIteratorFromCellOrCellRanges::new(self, last)
  }
}
pub trait CellOrCellRangeMOCIntoIterator<T: Idx>: Sized {
  type Qty: MocQty<T>;
  type IntoCellOrCellRangeMOCIter: CellOrCellRangeMOCIterator<T, Qty = Self::Qty>;

  fn into_cellcellrange_moc_iter(self) -> Self::IntoCellOrCellRangeMOCIter;
}

// Convert Cell --> Ranges
// TODO: merge (Cell --> Ranges) and (CellOrCellRange --> Ranges) in a single obj using Into<Range> ?

// RangeMOCIterator<T, Q, B>: Sized + MOCProperties + Iterator<Item=B>
// with B: Borrow<MocRange<T, Q>>
// Don't do this, we will clone as iterating if necessary

pub trait RangeMOCIterator<T: Idx>: Sized + MOCProperties + Iterator<Item = Range<T>> {
  type Qty: MocQty<T>;

  /// If available, returns the last range of the Iterator (or at least a range having the last
  /// range upper bound), without consuming the iterator.
  /// This information is available e.g. for an Iterator from a Vector, but may not be available
  /// for an Iterator coming from a stream.
  /// If available, this information can be used for fast rejection tests.
  fn peek_last(&self) -> Option<&Range<T>>;

  /// For debug purpose: transform this iterator into an iterator that panics if the iterator
  /// is not made of ordered and non-overlapping ranges.
  fn into_checked(self) -> CheckedIterator<T, Self::Qty, Self> {
    check(self)
  }

  /// For debug purpose: check that this iterator is made of sorted and non-overlapping ranges.
  /// This method is to be used on `(&RangeMOC).into_range_moc_iter()`
  fn check(self) {
    for _ in self.into_checked() {}
  }

  fn to_fits_ivoa<W: Write>(
    self,
    moc_id: Option<String>,
    moc_type: Option<keywords::MocType>,
    writer: W,
  ) -> Result<(), FitsError> {
    ranges_to_fits_ivoa(self, moc_id, moc_type, writer)
  }

  fn into_range_moc(self) -> RangeMOC<T, Self::Qty> {
    RangeMOC::new(self.depth_max(), self.collect())
  }

  fn range_sum(self) -> T {
    let mut sum = T::zero();
    for Range { start, end } in self {
      sum += end - start;
    }
    sum
  }

  fn coverage_percentage(self) -> f64 {
    let mut rsum = self.range_sum();
    let mut tot = Self::Qty::upper_bound_exclusive();
    if T::N_BITS > 52 {
      // 52 = n mantissa bits in a f64
      // Divide by the same power of 2, dropping the LSBs
      let shift = (T::N_BITS - 52) as u32;
      rsum = rsum.unsigned_shr(shift);
      tot = tot.unsigned_shr(shift);
    }
    rsum.cast_to_f64() / tot.cast_to_f64()
  }

  fn flatten_to_fixed_depth_cells(self) -> DepthMaxCellsFromRanges<T, Self::Qty, Self> {
    DepthMaxCellsFromRanges::new(self)
  }

  // I have not yet found a way to ensure that the quantity is the same, with only the a different
  // type T --> U
  fn convert<U, R>(self) -> ConvertIterator<T, Self::Qty, Self, U, R>
  where
    U: Idx + From<T>,
    R: MocQty<U>,
  {
    convert(self)
  }

  fn cells(self) -> CellMOCIteratorFromRanges<T, Self::Qty, Self> {
    CellMOCIteratorFromRanges::new(self)
  }

  fn degrade(self, new_depth: u8) -> DegradeRangeIter<T, Self::Qty, Self> {
    degrade(self, new_depth)
  }

  fn not(self) -> NotRangeIter<T, Self::Qty, Self> {
    not(self)
  }

  fn and<I>(self, other: I) -> AndRangeIter<T, Self::Qty, Self, I>
  where
    I: RangeMOCIterator<T, Qty = Self::Qty>,
  {
    and(self, other)
  }

  fn or<I>(self, other: I) -> OrRangeIter<T, Self::Qty, Self, I>
  where
    I: RangeMOCIterator<T, Qty = Self::Qty>,
  {
    or(self, other)
  }

  fn xor<I>(self, other: I) -> XorRangeIter<T, Self::Qty, Self, I>
  where
    I: RangeMOCIterator<T, Qty = Self::Qty>,
  {
    xor(self, other)
  }

  fn minus<I>(self, other: I) -> MinusRangeIter<T, Self::Qty, Self, I>
  where
    I: RangeMOCIterator<T, Qty = Self::Qty>,
  {
    minus(self, other)
  }
}

pub trait RangeMOCIntoIterator<T: Idx>: Sized {
  type Qty: MocQty<T>;
  type IntoRangeMOCIter: RangeMOCIterator<T, Qty = Self::Qty>;

  fn into_range_moc_iter(self) -> Self::IntoRangeMOCIter;
}

/// Defines an empty `RangeMOCIterator<`
pub struct EmptyRangeMOCIterator<T: Idx, Q: MocQty<T>>(u8, PhantomData<T>, PhantomData<Q>);
impl<T: Idx, Q: MocQty<T>> EmptyRangeMOCIterator<T, Q> {
  pub fn new(depth: u8) -> Self {
    EmptyRangeMOCIterator(depth, PhantomData, PhantomData)
  }
}

impl<T: Idx, Q: MocQty<T>> HasMaxDepth for EmptyRangeMOCIterator<T, Q> {
  fn depth_max(&self) -> u8 {
    self.0
  }
}
impl<T: Idx, Q: MocQty<T>> ZSorted for EmptyRangeMOCIterator<T, Q> {}
impl<T: Idx, Q: MocQty<T>> NonOverlapping for EmptyRangeMOCIterator<T, Q> {}
impl<T: Idx, Q: MocQty<T>> MOCProperties for EmptyRangeMOCIterator<T, Q> {}
impl<T: Idx, Q: MocQty<T>> Iterator for EmptyRangeMOCIterator<T, Q> {
  type Item = Range<T>;
  fn next(&mut self) -> Option<Self::Item> {
    None
  }
}
impl<T: Idx, Q: MocQty<T>> RangeMOCIterator<T> for EmptyRangeMOCIterator<T, Q> {
  type Qty = Q;
  fn peek_last(&self) -> Option<&Range<T>> {
    None
  }
}

// NUniq MOC
pub struct NUniqMOC<T: Idx> {
  pub depth_max: u8,
  pub zsorted_nuniq: Vec<T>,
}
impl<T: Idx> NUniqMOC<T> {
  pub fn new(depth_max: u8, zsorted_nuniq: Vec<T>) -> Self {
    Self {
      depth_max,
      zsorted_nuniq,
    }
  }
}

#[cfg(test)]
mod tests {

  use std::cmp::Ordering;
  use std::ops::Range;

  use crate::elem::{cell::Cell, cellcellrange::CellOrCellRange, cellrange::CellRange};
  use crate::elemset::range::{hpx::HpxUniq2DepthIdxIter, MocRanges};
  use crate::moc::{
    range::RangeMOC, CellMOCIterator, CellOrCellRangeMOCIterator, RangeMOCIntoIterator,
    RangeMOCIterator,
  };
  use crate::qty::Hpx;

  #[test]
  fn test_range2cells_1() {
    let ranges = MocRanges::<u64, Hpx<u64>>::new_unchecked(vec![0..5]);
    let rm = RangeMOC::new(29, ranges);
    let rit = (&rm).into_range_moc_iter();
    let v1: Vec<Cell<u64>> = rit.cells().collect();
    assert_eq!(v1, vec![Cell::new(28, 0), Cell::new(29, 4)]);

    let v2: Vec<(i8, u64)> = HpxUniq2DepthIdxIter::new(rm.into_moc_ranges()).collect();
    assert_eq!(v2, vec![(28, 0), (29, 4)]);
  }

  #[test]
  fn test_range2cells_2() {
    let ranges: Vec<Range<u64>> = vec![2..8];
    let ranges = MocRanges::<u64, Hpx<u64>>::new_unchecked(ranges);
    let rm = RangeMOC::new(29, ranges);
    let rit = (&rm).into_range_moc_iter();
    let v1: Vec<Cell<u64>> = rit.cells().collect();
    assert_eq!(
      v1,
      vec![Cell::new(29, 2), Cell::new(29, 3), Cell::new(28, 1)]
    );

    let v2: Vec<(i8, u64)> = HpxUniq2DepthIdxIter::new(rm.into_moc_ranges()).collect();
    assert_eq!(v2, vec![(28, 1), (29, 2), (29, 3)]);
  }

  #[test]
  fn test_range2cells_3() {
    let ranges = MocRanges::<u64, Hpx<u64>>::new_unchecked(vec![
      0..5,
      6..59,
      78..6953,
      12458..55587,
      55787..65587,
    ]);
    let rm = RangeMOC::new(29, ranges);
    let rit = (&rm).into_range_moc_iter();
    let mut v1: Vec<Cell<u64>> = rit.cells().collect();
    // println!("{:?}", v1);
    v1.sort_by(|a, b| match a.depth.cmp(&b.depth) {
      Ordering::Less => Ordering::Less,
      Ordering::Greater => Ordering::Greater,
      Ordering::Equal => a.idx.cmp(&b.idx),
    });

    let v2: Vec<(i8, u64)> = HpxUniq2DepthIdxIter::new(rm.into_moc_ranges()).collect();
    // println!("{:?}", v2);
    assert_eq!(v1.len(), v2.len());
    for (Cell { depth, idx }, (depth2, idx2)) in v1.into_iter().zip(v2.into_iter()) {
      assert_eq!(depth, depth2 as u8);
      assert_eq!(idx, idx2);
    }
  }

  #[test]
  fn test_range2cellrange() {
    let ranges: Vec<Range<u64>> = vec![2..8];
    let ranges = MocRanges::<u64, Hpx<u64>>::new_unchecked(ranges);
    let rm = RangeMOC::new(29, ranges);
    let res: Vec<CellOrCellRange<u64>> = (&rm).into_range_moc_iter().cells().cellranges().collect();
    assert_eq!(
      res,
      vec![
        CellOrCellRange::CellRange(CellRange::new(29, 2, 4)),
        CellOrCellRange::Cell(Cell::new(28, 1)),
      ]
    );
  }

  #[test]
  fn test_to_ascii() {
    let ranges = MocRanges::<u64, Hpx<u64>>::new_unchecked(vec![
      0..5,
      6..59,
      78..6953,
      12458..55587,
      55787..65587,
    ]);
    let rm = RangeMOC::new(29, ranges);
    let mut sink = Vec::new();
    (&rm)
      .into_range_moc_iter()
      .cells()
      .cellranges()
      .to_ascii_ivoa(Some(80), false, &mut sink)
      .unwrap();
    // println!("{}\n", &String::from_utf8_lossy(&sink));

    let mut sink = Vec::new();
    (&rm)
      .into_range_moc_iter()
      .cells()
      .cellranges()
      .to_ascii_ivoa(Some(80), true, &mut sink)
      .unwrap();
    //  println!("{}\n", &String::from_utf8_lossy(&sink));

    let mut sink = Vec::new();
    (&rm)
      .into_range_moc_iter()
      .cells()
      .cellranges()
      .to_ascii_ivoa(None, false, &mut sink)
      .unwrap();
    println!("{}\n", &String::from_utf8_lossy(&sink));

    let mut sink = Vec::new();
    (&rm)
      .into_range_moc_iter()
      .cells()
      .cellranges()
      .to_ascii_ivoa(None, true, &mut sink)
      .unwrap();
    //  println!("{}\n", &String::from_utf8_lossy(&sink));

    let mut sink = Vec::new();
    (&rm)
      .into_range_moc_iter()
      .cells()
      .cellranges()
      .to_ascii_stream(false, &mut sink)
      .unwrap();
    //  println!("{}\n", &String::from_utf8_lossy(&sink));

    let mut sink = Vec::new();
    (&rm)
      .into_range_moc_iter()
      .cells()
      .cellranges()
      .to_ascii_stream(true, &mut sink)
      .unwrap();
    //  println!("{}\n", &String::from_utf8_lossy(&sink));
  }

  #[test]
  fn test_to_json() {
    let ranges = MocRanges::<u64, Hpx<u64>>::new_unchecked(vec![
      0..5,
      6..59,
      78..6953,
      12458..55587,
      55787..65587,
    ]);
    let rm = RangeMOC::new(29, ranges);
    let mut sink = Vec::new();
    (&rm)
      .into_range_moc_iter()
      .cells()
      .to_json_aladin(Some(40), &mut sink)
      .unwrap();
    //  println!("{}\n", &String::from_utf8_lossy(&sink));

    let mut sink = Vec::new();
    (&rm)
      .into_range_moc_iter()
      .cells()
      .to_json_aladin(None, &mut sink)
      .unwrap();
    //  println!("{}\n", &String::from_utf8_lossy(&sink));
  }
}
