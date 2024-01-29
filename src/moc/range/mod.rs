use std::{
  convert::{TryFrom, TryInto},
  error::Error,
  fs::File,
  io::BufReader,
  marker::PhantomData,
  num::TryFromIntError,
  ops::Range,
  path::Path,
  slice,
  vec::IntoIter,
};

use healpix::{
  compass_point::{MainWind, Ordinal, OrdinalSet},
  nested::{
    append_external_edge, bmoc::BMOC, box_coverage, cone_coverage_approx_custom,
    custom_polygon_coverage, elliptical_cone_coverage_custom, external_edge, external_edge_struct,
    hash, polygon_coverage, ring_coverage_approx_custom, zone_coverage,
  },
  sph_geom::{coo3d::Coo3D, ContainsSouthPoleMethod},
};

use crate::{
  deser::{
    ascii::AsciiError,
    fits::{from_fits_ivoa, MocIdxType, MocQtyType, MocType},
  },
  elem::{cell::Cell, range::MocRange},
  elemset::{
    cell::{Cells, MocCells},
    range::MocRanges,
  },
  idx::Idx,
  moc::{
    adapters::DepthMaxCellsFromRanges,
    builder::{
      fixed_depth::{FixedDepthMocBuilder, OwnedOrderedFixedDepthCellsToRangesFromU64},
      maxdepth_range::RangeMocBuilder,
    },
    cell::CellMOC,
    range::op::{
      and::{and, AndRangeIter},
      merge::merge_sorted,
      minus::{minus, MinusRangeIter},
      multi_op::kway_or,
      or::{or, OrRangeIter},
      xor::xor,
    },
    CellMOCIntoIterator, CellMOCIterator, CellOrCellRangeMOCIterator, HasMaxDepth, MOCProperties,
    NonOverlapping, RangeMOCIntoIterator, RangeMOCIterator, ZSorted,
  },
  qty::{Bounded, Frequency, Hpx, MocQty, Time},
  ranges::{BorrowedRanges, Ranges, SNORanges},
};

pub mod op;

/// Structure made to draw MOCs in AladinLite.
/// It contains an HEALPix cell and the list of edges to be drawn.
pub struct CellAndEdges<T: Idx> {
  pub uniq: T,
  pub edges: OrdinalSet,
}

/// A MOC made of (ordered and non-overlaping) ranges.
#[derive(Debug, Clone)]
pub struct RangeMOC<T: Idx, Q: MocQty<T>> {
  depth_max: u8,
  ranges: MocRanges<T, Q>,
}
impl<T: Idx, Q: MocQty<T>> RangeMOC<T, Q> {
  pub fn new(depth_max: u8, ranges: MocRanges<T, Q>) -> Self {
    Self { depth_max, ranges }
  }
  pub fn new_empty(depth_max: u8) -> Self {
    Self::new(depth_max, MocRanges::default())
  }
  pub fn new_full_domain(depth_max: u8) -> Self {
    let range = Range {
      start: T::zero(),
      end: Q::upper_bound_exclusive(),
    };
    Self {
      depth_max,
      ranges: Ranges::new_unchecked(vec![range]).into(),
    }
  }
  pub fn depth_max(&self) -> u8 {
    self.depth_max
  }
  /// Returns the number of ranges the MOC contains
  pub fn len(&self) -> usize {
    self.ranges.0 .0.len()
  }
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn first_index(&self) -> Option<T> {
    self.ranges.0.iter().nth(0).map(|r| r.start)
  }

  pub fn last_index(&self) -> Option<T> {
    self.ranges.0.iter().last().map(|r| r.end)
  }

  pub fn moc_ranges(&self) -> &MocRanges<T, Q> {
    &self.ranges
  }
  pub fn into_moc_ranges(self) -> MocRanges<T, Q> {
    self.ranges
  }
  pub fn to_ascii(&self) -> Result<String, AsciiError> {
    let mut sink = Vec::new();
    self
      .into_range_moc_iter()
      .cells()
      .cellranges()
      .to_ascii_ivoa(Some(80), false, &mut sink)?;
    Ok(unsafe { String::from_utf8_unchecked(sink) })
  }

  pub fn eq_without_depth(&self, rhs: &Self) -> bool {
    self.ranges.eq(&rhs.ranges)
  }
  pub fn n_depth_max_cells(&self) -> T {
    let range_sum = self.range_sum();
    let shift = Q::shift_from_depth_max(self.depth_max);
    range_sum.unsigned_shr(shift as u32)
  }
  pub fn range_sum(&self) -> T {
    self.ranges.range_sum()
  }

  pub fn coverage_percentage(&self) -> f64 {
    self.ranges.coverage_percentage()
  }

  pub fn from_fixed_depth_cells<I: Iterator<Item = T>>(
    depth: u8,
    cells_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    let mut builder = FixedDepthMocBuilder::new(depth, buf_capacity);
    for cell in cells_it {
      builder.push(cell);
    }
    builder.into_moc()
  }

  pub fn from_cells<I: Iterator<Item = (u8, T)>>(
    depth: u8,
    cells_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    let mut builder = RangeMocBuilder::new(depth, buf_capacity);
    let it = cells_it.map(move |depth_idx| MocRange::<T, Q>::from(depth_idx).0);
    for range in it {
      builder.push(range);
    }
    builder.into_moc()
  }

  pub fn from_maxdepth_ranges<I>(depth: u8, it: I, buf_capacity: Option<usize>) -> Self
  where
    I: Iterator<Item = Range<T>>,
  {
    let mut builder = RangeMocBuilder::new(depth, buf_capacity);
    for range in it {
      builder.push(range);
    }
    builder.into_moc()
  }

  /// Flatten the MOC returning an (ordered) iterator of cells at the MOC depth_max depth.
  pub fn flatten_to_fixed_depth_cells(
    &self,
  ) -> DepthMaxCellsFromRanges<T, Q, RangeRefMocIter<'_, T, Q>> {
    DepthMaxCellsFromRanges::new(self.into_range_moc_iter())
  }

  /// The value must be at the MOC depth
  pub fn contains_depth_max_val(&self, x: &T) -> bool {
    self.contains_val(&x.unsigned_shl(Q::shift_from_depth_max(self.depth_max) as u32))
  }

  /// The value must be at the Quantity MAX_DEPTH
  pub fn contains_val(&self, x: &T) -> bool {
    self.ranges.contains_val(x)
  }

  pub fn contains_cell(&self, depth: u8, idx: T) -> bool {
    let range = MocRange::<T, Q>::from((depth, idx));
    self.ranges.contains_range(&range.0)
  }

  pub fn append_fixed_depth_cells<I: Iterator<Item = T>>(
    self,
    depth: u8,
    cells_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    assert_eq!(depth, self.depth_max);
    let mut builder = FixedDepthMocBuilder::from(buf_capacity, self);
    for cell in cells_it {
      builder.push(cell)
    }
    builder.into_moc()
  }

  pub fn and(&self, rhs: &RangeMOC<T, Q>) -> RangeMOC<T, Q> {
    let depth_max = self.depth_max.max(rhs.depth_max);
    let ranges = self.ranges.intersection(&rhs.ranges);
    RangeMOC::new(depth_max, ranges)
  }
  pub fn intersection(&self, rhs: &RangeMOC<T, Q>) -> RangeMOC<T, Q> {
    self.and(rhs)
  }

  pub fn or(&self, rhs: &RangeMOC<T, Q>) -> RangeMOC<T, Q> {
    let depth_max = self.depth_max.max(rhs.depth_max);
    let ranges = self.ranges.union(&rhs.ranges);
    RangeMOC::new(depth_max, ranges)
  }
  pub fn union(&self, rhs: &RangeMOC<T, Q>) -> RangeMOC<T, Q> {
    self.or(rhs)
  }

  pub fn not(&self) -> RangeMOC<T, Q> {
    self.complement()
  }
  pub fn complement(&self) -> RangeMOC<T, Q> {
    RangeMOC::new(self.depth_max, self.ranges.complement())
  }

  pub fn xor(&self, rhs: &RangeMOC<T, Q>) -> RangeMOC<T, Q> {
    let depth_max = self.depth_max.max(rhs.depth_max);
    let ranges = xor(self.into_range_moc_iter(), rhs.into_range_moc_iter()).collect();
    RangeMOC::new(depth_max, ranges)
  }

  pub fn minus(&self, rhs: &RangeMOC<T, Q>) -> RangeMOC<T, Q> {
    let depth_max = self.depth_max.max(rhs.depth_max);
    let ranges = minus(self.into_range_moc_iter(), rhs.into_range_moc_iter()).collect();
    RangeMOC::new(depth_max, ranges)
  }

  pub fn degraded(&self, new_depth: u8) -> RangeMOC<T, Q> {
    let depth_max = self.depth_max.min(new_depth);
    let ranges = self.ranges.degraded(new_depth);
    RangeMOC::new(depth_max, ranges)
  }

  // CONTAINS: union that stops at first elem found
  // OVERLAP (=!CONTAINS on the COMPLEMENT ;) )

  // pub fn owned_and() -> RangeMOC<T, Q> { }
  // pub fn lazy_and() ->

  /*pub fn into_range_moc_iter(self) -> LazyRangeMOCIter<T, Q, IntoIter<Range<T>>> {
    LazyRangeMOCIter::new(self.depth_max, self.ranges.0.0.into_iter())
  }*/

  /*pub fn range_moc_iter(&self) -> LazyRangeMOCVecIter<'_, H> {
    LazyRangeMOCVecIter::new(self.depth_max, self.ranges.iter())
  }*/
  /*pub fn into_cells_iter(self) -> CellMOCIteratorFromRanges<T, Q, Self> {
    CellMOCIteratorFromRanges::new(self)
  }*/
  /*pub fn to_cells_iter(&self) -> CellMOCIteratorFromRanges<T, Q, Self> {
    CellMOCIteratorFromRanges::new(self)
  }*/
}
impl<T: Idx, Q: MocQty<T>> HasMaxDepth for RangeMOC<T, Q> {
  fn depth_max(&self) -> u8 {
    self.depth_max
  }
}
impl<T: Idx, Q: MocQty<T>> ZSorted for RangeMOC<T, Q> {}
impl<T: Idx, Q: MocQty<T>> NonOverlapping for RangeMOC<T, Q> {}

impl From<BMOC> for RangeMOC<u64, Hpx<u64>> {
  fn from(bmoc: BMOC) -> Self {
    /*println!("BMOC depth max: {}", bmoc.get_depth_max());
    for raw_cell in bmoc.iter() {
      let bmoc::Cell { raw_value, depth, hash, is_full } = bmoc.from_raw_value(*raw_cell);
      println!("depth: {}, idx: {}", depth, hash);
    }*/
    let shift = Hpx::<u64>::shift_from_depth_max(bmoc.get_depth_max());
    let mut ranges = bmoc.to_ranges();
    for range in ranges.iter_mut() {
      range.start <<= shift;
      range.end <<= shift;
    }
    // TODO: add a debug_assert! checking that the result is sorted!
    RangeMOC::new(
      bmoc.get_depth_max(),
      MocRanges::new_unchecked(ranges.to_vec()),
    )
  }
}

/// Complex type returned by the `expanded_iter` method.
pub type ExpandedIter<'a, T> = OrRangeIter<
  T,
  Hpx<T>,
  RangeRefMocIter<'a, T, Hpx<T>>,
  OwnedOrderedFixedDepthCellsToRangesFromU64<T, Hpx<T>, IntoIter<u64>>,
>;

/// Complex type returned by the `contracted_iter` method.
pub type ContractedIter<'a, T> = MinusRangeIter<
  T,
  Hpx<T>,
  RangeRefMocIter<'a, T, Hpx<T>>,
  OwnedOrderedFixedDepthCellsToRangesFromU64<T, Hpx<T>, IntoIter<u64>>,
>;

/// Complex type returned by the `external_border_iter` method.
pub type ExtBorderIter<'a, T> =
  MinusRangeIter<T, Hpx<T>, ExpandedIter<'a, T>, RangeRefMocIter<'a, T, Hpx<T>>>;

/// Complex type returned by the `internal_border_iter` method.
pub type IntBorderIter<'a, T> =
  AndRangeIter<T, Hpx<T>, RangeMocIter<T, Hpx<T>>, RangeRefMocIter<'a, T, Hpx<T>>>;

impl<T: Idx> RangeMOC<T, Hpx<T>> {
  /// Returns `true` if the given coordinates (in radians) is in the MOC
  pub fn is_in(&self, lon: f64, lat: f64) -> bool {
    let hash = hash(self.depth_max, lon, lat);
    let hash = T::from_u64(hash);
    self.contains_depth_max_val(&hash)
  }

  /// Add the MOC external border of depth `self.depth_max`.
  pub fn expanded(&self) -> Self {
    self.expanded_iter().into_range_moc()
  }

  pub fn expanded_iter(&self) -> ExpandedIter<'_, T> {
    let mut ext: Vec<u64> = Vec::with_capacity(10 * self.ranges.ranges().0.len()); // constant to be adjusted
    for Cell { depth, idx } in self.into_range_moc_iter().cells() {
      append_external_edge(depth, idx.to_u64(), self.depth_max - depth, &mut ext);
    }
    ext.sort_unstable(); // parallelize with rayon? It is the slowest part!!
    let ext_range_iter =
      OwnedOrderedFixedDepthCellsToRangesFromU64::new(self.depth_max, ext.into_iter());
    or(self.into_range_moc_iter(), ext_range_iter)
  }

  pub fn contracted(&self) -> Self {
    self.contracted_iter().into_range_moc()
  }

  pub fn contracted_iter(&self) -> ContractedIter<'_, T> {
    let mut ext_of_complement: Vec<u64> = Vec::with_capacity(10 * self.ranges.ranges().0.len()); // constant to be adjusted
    for Cell { depth, idx } in self.into_range_moc_iter().not().cells() {
      append_external_edge(
        depth,
        idx.to_u64(),
        self.depth_max - depth,
        &mut ext_of_complement,
      );
    }
    // _by(|a, b| a.flat_cmp::<Hpx<u64>>(&b))
    ext_of_complement.sort_unstable(); // parallelize with rayon? It is the slowest part!!
    let ext_range_iter = OwnedOrderedFixedDepthCellsToRangesFromU64::new(
      self.depth_max,
      ext_of_complement.into_iter(),
    );
    minus(self.into_range_moc_iter(), ext_range_iter)
  }

  /// Returns this MOC external border
  pub fn external_border(&self) -> Self {
    self.external_border_iter().into_range_moc()
  }

  /// Returns this MOC external border
  pub fn external_border_iter(&self) -> ExtBorderIter<'_, T> {
    minus(self.expanded_iter(), self.into_range_moc_iter())
  }

  /// Returns this MOC internal border
  pub fn internal_border(&self) -> Self {
    let not = self.not();
    and(not.expanded_iter(), self.into_range_moc_iter()).into_range_moc()
  }

  pub fn internal_border_iter(&self) -> IntBorderIter<'_, T> {
    let left = self.not().expanded();
    and(left.into_range_moc_iter(), self.into_range_moc_iter())
  }

  /// Fill the possible holes the MOC contains, except the `except_n_largest` (0 as default)
  /// `n` largest ones.
  /// This operation may be an heavy operation, here the algorithm we use:
  /// * perform a `split_into_joint_mocs` operation on the moc `complement`
  /// * remove the (1 + except_n_largest) largest sub-mocs
  /// * perform the union of the moc with the remaining sub-mocs
  pub fn fill_holes(&self, except_n_largest: Option<usize>) -> Self {
    let convert = |cell_moc: CellMOC<T, Hpx<T>>| {
      let range_moc = cell_moc.into_cell_moc_iter().ranges().into_range_moc();
      let coverage = range_moc.coverage_percentage();
      Some((coverage, range_moc))
    };
    let mut cov_mocs: Vec<(f64, Self)> = self
      .complement()
      .split_into_joint_mocs_gen(external_edge, convert);
    // Use b.cmp(a) instead of a.cmp(b)
    cov_mocs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    self.or(&kway_or(Box::new(
      cov_mocs
        .into_iter()
        .skip(1 + except_n_largest.unwrap_or(0))
        .map(|(_, moc)| moc),
    )))
  }

  /// Fill the possible holes in MOC whoch are smaller than the given `sky_fraction` (in `[0, 1])
  /// This operation may be an heavy operation, here the algorithm we use:
  /// * perform a `split_into_joint_mocs` operation on the moc `complement`
  /// * remove the sub-mocs covering more than the given sky fraction
  /// * perform the union of the moc with the remaining sub-mocs
  pub fn fill_holes_smaller_than(&self, sky_fraction: f64) -> Self {
    let convert = |cell_moc: CellMOC<T, Hpx<T>>| {
      let range_moc = cell_moc.into_cell_moc_iter().ranges().into_range_moc();
      let coverage = range_moc.coverage_percentage();
      if coverage <= sky_fraction {
        Some(range_moc)
      } else {
        None
      }
    };
    let mocs: Vec<Self> = self
      .complement()
      .split_into_joint_mocs_gen(external_edge, convert);
    self.or(&kway_or(Box::new(mocs.into_iter())))
  }

  /// Split the disjoint MOC into joint MOCs.
  /// # Param
  ///   `includeIndirectNeighbours`: see [this page](http://www.imageprocessingplace.com/downloads_V3/root_downloads/tutorials/contour_tracing_Abeer_George_Ghuneim/connect.html),
  ///     by default we use a 4-connectivity (i.e. we consider only direct-neighbours,
  ///     i.e. neighbouring cells sharing an edge with the central cell).
  ///     It makes sense if we consider the contours when splitting a MOC.
  ///     Setting `includeIndirectNeighbours` to `true` we consider the indirect-neighbours, i.e.
  ///     the cell only sharing a vertex with the central cell, as been part of the same MOC.
  ///   
  /// # Algo description
  /// * We work on zuniq indices (i.e. special uniq notation for which the natural order follows
  ///  the z-order curve order, mixing the various resolutions), shifted of one bit to the left
  ///  so that we use the LSB to mark the cells we already visited.
  /// * The MOC is a list of ordered zuniq cells + marker bit.
  /// * We start with the first cell, mark it, look for its neighbour.
  /// * For each neighbour we find (or we find a super-cel containing it),
  ///   we mark the cell as visited and push it into the stack of cells we want to check the neighbours
  /// * When the stackis empty, we visited all neighbours of neighbours of neighbours of ...
  /// * We form a new MOC from all marked cells that we put in a list,
  /// * and we remove them from the original MOC.
  /// * We continue the process with the updated original MOC
  pub fn split_into_joint_mocs(
    &self,
    include_indirect_neighbours: bool,
  ) -> Vec<CellMOC<T, Hpx<T>>> {
    let convert = Some;
    if include_indirect_neighbours {
      self.split_into_joint_mocs_gen(external_edge, convert)
    } else {
      self.split_into_joint_mocs_gen(
        |depth, idx, delta_depth| {
          let ext_edge = external_edge_struct(depth, idx, delta_depth);
          ext_edge
            .get_edge(&Ordinal::SE)
            .iter()
            .chain(ext_edge.get_edge(&Ordinal::SW).iter())
            .chain(ext_edge.get_edge(&Ordinal::NE).iter())
            .chain(ext_edge.get_edge(&Ordinal::NW).iter())
            .cloned()
            .collect::<Vec<u64>>()
            .into_boxed_slice()
        },
        convert,
      )
    }
  }

  fn split_into_joint_mocs_gen<F, M, C>(&self, fn_neighbours: F, convert: C) -> Vec<M>
  where
    F: Fn(u8, u64, u8) -> Box<[u64]>,
    C: Fn(CellMOC<T, Hpx<T>>) -> Option<M>,
  {
    let mut elems: Vec<T> = self
      .into_range_moc_iter()
      .cells()
      .map(|cell| cell.zuniq::<Hpx<T>>() << 1) // add the "already_visit" bit set to 0
      .collect();
    // The vector is supposed to be sorted!
    debug_assert!(
      elems
        .iter()
        .fold((true, T::zero()), |(b, prev), curr| (
          b & (prev <= *curr),
          *curr
        ))
        .0
    );
    let mut mocs: Vec<M> = Default::default();
    while !elems.is_empty() {
      let mut stack: Vec<T> = Default::default();
      let first_mut: &mut T = elems.first_mut().unwrap(); // unwrap ok since we tested empty just before
      *first_mut |= T::one();
      stack.push((*first_mut) >> 1); // Put the value without the bit flag
      while let Some(zuniq) = stack.pop() {
        let Cell { depth, idx } = Cell::<T>::from_zuniq::<Hpx<T>>(zuniq);
        let mut stack_changed = false;
        for neig in fn_neighbours(depth, idx.to_u64(), self.depth_max - depth).iter() {
          let neig = T::from_u64(*neig);
          let neig_zuniq = <Hpx<T>>::to_zuniq(self.depth_max, neig) << 1;
          match elems.binary_search(&neig_zuniq) {
            Ok(i) => {
              // => not marked
              let elem_mut: &mut T = elems.get_mut(i).unwrap();
              *elem_mut |= T::one();
              stack.push((*elem_mut) >> 1);
              stack_changed = true;
              debug_assert!((*elems.get(i).unwrap()) & T::one() == T::one());
            }
            Err(i) => {
              // The deeper zuniq may be lower or higher than the one's of the larger cells
              // containing it. It depends if the location of the sentinel bit of low resolution
              // cells match a 0 or a 1 in the deeper resolution index.
              if i > 0 {
                // Check the lower zuniq
                let zuniq_with_flag_mut = elems.get_mut(i - 1).unwrap();
                if *zuniq_with_flag_mut & T::one() != T::one() {
                  // flag not yet set (else do nothing)
                  let Cell {
                    depth: tdepth,
                    idx: tidx,
                  } = Cell::<T>::from_zuniq::<Hpx<T>>((*zuniq_with_flag_mut) >> 1);
                  if tidx == (neig >> ((self.depth_max - tdepth) << 1) as usize) {
                    // neig included in tidx
                    *zuniq_with_flag_mut |= T::one();
                    stack.push((*zuniq_with_flag_mut) >> 1);
                    stack_changed = true;
                    debug_assert!((*elems.get(i - 1).unwrap()) & T::one() == T::one());
                  }
                }
              }
              if i < elems.len() {
                // Check the higher zuniq
                // Yes, duplicated code, onle the index is different and we should put this in a function we call...
                let zuniq_with_flag_mut = elems.get_mut(i).unwrap();
                if *zuniq_with_flag_mut & T::one() != T::one() {
                  // flag not yet set (else do nothing)
                  let Cell {
                    depth: tdepth,
                    idx: tidx,
                  } = Cell::<T>::from_zuniq::<Hpx<T>>((*zuniq_with_flag_mut) >> 1);
                  if tidx == (neig >> ((self.depth_max - tdepth) << 1) as usize) {
                    // neig included in tidx
                    *zuniq_with_flag_mut |= T::one();
                    stack.push((*zuniq_with_flag_mut) >> 1);
                    stack_changed = true;
                    debug_assert!((*elems.get(i).unwrap()) & T::one() == T::one());
                  }
                }
              }
            }
          }
        }
        // Ensure the stack is a stack (ordered, no duplicate)
        if stack_changed {
          stack.sort_unstable(); // probably slow to call this at each iteration... but BTreeSet.pop_first is nighlty so far :o/
          debug_assert!({
            // The stack does not contains duplicates
            let l1 = elems.len();
            elems.dedup();
            let l2 = elems.len();
            l1 == l2
          });
        }
      }
      // One could use drain_filter but it is not stable yet (we do not want to use nightly) :o/
      let moc = CellMOC::new(
        self.depth_max,
        MocCells::new(Cells::new(
          elems
            .iter()
            .cloned()
            .filter(|zuniq_with_flag| (*zuniq_with_flag) & T::one() == T::one())
            .map(|zuniq_with_flag| Cell::<T>::from_zuniq::<Hpx<T>>(zuniq_with_flag >> 1))
            .collect(),
        )),
      );
      if let Some(moc) = convert(moc) {
        mocs.push(moc);
      }
      elems.retain(|zuniq| *zuniq & T::one() == T::zero());
    }
    mocs
  }

  /// Returns the list of cells (uniq notation) with the list of its edges to be drawn in such
  /// a way that no edge overlapping occurs.
  pub fn all_edges_only_once(&self) -> impl Iterator<Item = CellAndEdges<T>> {
    let zuniqs: Vec<T> = self
      .into_range_moc_iter()
      .cells()
      .map(|cell| cell.zuniq::<Hpx<T>>())
      .collect();
    let mut uniqs: Vec<T> = zuniqs
      .iter()
      .cloned()
      .map(|zuniq| Cell::from_zuniq::<Hpx<T>>(zuniq).uniq_hpx())
      .collect();
    uniqs.sort();

    // we look only for neighbours having a depth <= the current cell depth
    // since deeper cells have not yet been iterated over.
    // We have already iterated on cell of depth < current cell depth
    // and cell depth == current cell depth && idx < current idx
    let neig_already_visited = move |depth: u8, org_idx: u64, neig_idx: u64| -> bool {
      let neig_idx_t = T::from_u64(neig_idx);
      let neig_zuniq = Hpx::<T>::to_zuniq(depth, neig_idx_t);
      match zuniqs.binary_search(&neig_zuniq) {
        Ok(_) => neig_idx < org_idx, // the order is the same, but neighbour has been already visited!
        Err(i) => {
          // Test index i
          if let Some((d, h)) = zuniqs.get(i - 1).map(|zuniq| Hpx::<T>::from_zuniq(*zuniq)) {
            if d < depth && (neig_idx_t >> ((depth - d) << 1) as usize) == h {
              // there is a neighbour having a lower depth, so it has already been visited
              return false;
            }
          }
          // Test index i + 1
          if let Some((d, h)) = zuniqs.get(i).map(|zuniq| Hpx::<T>::from_zuniq(*zuniq)) {
            if d < depth && (neig_idx_t >> ((depth - d) << 1) as usize) == h {
              // there is a neighbour having a lower depth, so it has already been visited
              return false;
            }
          }
          // no neighbour or neighbour having a deeper depth, so we need to visit
          true
        }
      }
    };

    uniqs.into_iter().map(move |uniq| {
      let cell = Cell::from_uniq_hpx(uniq);
      let idx_u64 = <T as Idx>::to_u64(cell.idx);
      let neigs_u64 = healpix::nested::neighbours(cell.depth, idx_u64, false);
      let mut edges = OrdinalSet::new();
      if let Some(idx_neig_u64) = neigs_u64.get(MainWind::NE) {
        if neig_already_visited(cell.depth, idx_u64, *idx_neig_u64) {
          edges.set(Ordinal::NE, true);
        }
      }
      if let Some(idx_neig_u64) = neigs_u64.get(MainWind::SE) {
        if neig_already_visited(cell.depth, idx_u64, *idx_neig_u64) {
          edges.set(Ordinal::SE, true);
        }
      }
      if let Some(idx_neig_u64) = neigs_u64.get(MainWind::NW) {
        if neig_already_visited(cell.depth, idx_u64, *idx_neig_u64) {
          edges.set(Ordinal::NW, true);
        }
      }
      if let Some(idx_neig_u64) = neigs_u64.get(MainWind::SW) {
        if neig_already_visited(cell.depth, idx_u64, *idx_neig_u64) {
          edges.set(Ordinal::SW, true);
        }
      }

      CellAndEdges { uniq, edges }
    })
  }

  /// Returns an iterator over the list of elementary edges (i.e. the edges of the border
  /// cells at the MOC maximum depth).
  /// Each edge is a tuple made of 2 points `A` and `B`: `((lonA, latA), (lonB, latB))`
  /// with longitudes and latitudes in radians.
  ///
  /// Algo:
  /// * for each cell (all at max depth) of the external border
  ///     + for each of the 4 border, we look if the neighbour cell is in the original MOC
  ///     + if yes, add the vertex tuples in output
  pub fn border_elementary_edges(&self) -> impl Iterator<Item = CellAndEdges<T>> + '_ {
    let hp = healpix::nested::get(self.depth_max);
    self
      .internal_border_iter()
      .flatten_to_fixed_depth_cells()
      .filter_map(move |idx| {
        let mut edges = OrdinalSet::new();
        let h = <T as Idx>::to_u64(idx);
        let neigs = hp.neighbours(h, false);
        if let Some(nh) = neigs.get(MainWind::NE) {
          if !self.contains_depth_max_val(&T::from_u64(*nh)) {
            edges.set(Ordinal::NE, true);
          }
        }
        if let Some(nh) = neigs.get(MainWind::SE) {
          if !self.contains_depth_max_val(&T::from_u64(*nh)) {
            edges.set(Ordinal::SE, true);
          }
        }
        if let Some(nh) = neigs.get(MainWind::NW) {
          if !self.contains_depth_max_val(&T::from_u64(*nh)) {
            edges.set(Ordinal::NW, true);
          }
        }
        if let Some(nh) = neigs.get(MainWind::SW) {
          if !self.contains_depth_max_val(&T::from_u64(*nh)) {
            edges.set(Ordinal::SW, true);
          }
        }
        let uniq = Hpx::<T>::uniq_hpx(self.depth_max, idx);
        if edges.is_empty() {
          None
        } else {
          Some(CellAndEdges { uniq, edges })
        }
      })
  }
}

fn from<T: Idx + TryFrom<u64, Error = TryFromIntError>>(
  range_moc: RangeMOC<u64, Hpx<u64>>,
) -> RangeMOC<T, Hpx<T>> {
  let depth_max = range_moc.depth_max;
  let ranges = range_moc.ranges.0;
  let shift = u64::N_BITS - T::N_BITS;
  let ranges: Vec<Range<T>> = ranges
    .0
    .iter()
    .map(|Range { start, end }| {
      (start >> shift).try_into().unwrap()..(end >> shift).try_into().unwrap()
    })
    .collect();
  RangeMOC::new(depth_max, MocRanges::new_unchecked(ranges))
}

impl From<RangeMOC<u64, Hpx<u64>>> for RangeMOC<u32, Hpx<u32>> {
  fn from(range_moc: RangeMOC<u64, Hpx<u64>>) -> Self {
    assert!(range_moc.depth_max < 14);
    from(range_moc)
  }
}

impl From<RangeMOC<u64, Hpx<u64>>> for RangeMOC<u16, Hpx<u16>> {
  fn from(range_moc: RangeMOC<u64, Hpx<u64>>) -> Self {
    assert!(range_moc.depth_max < 6);
    from(range_moc)
  }
}

impl RangeMOC<u64, Hpx<u64>> {
  pub fn from_fits_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
    let file = File::open(path)?;
    match from_fits_ivoa(BufReader::new(file))? {
      MocIdxType::U16(MocQtyType::Hpx(MocType::Cells(moc))) => Ok(
        moc
          .into_cell_moc_iter()
          .ranges()
          .convert::<u64, Hpx<u64>>()
          .into_range_moc(),
      ),
      MocIdxType::U32(MocQtyType::Hpx(MocType::Cells(moc))) => Ok(
        moc
          .into_cell_moc_iter()
          .ranges()
          .convert::<u64, Hpx<u64>>()
          .into_range_moc(),
      ),
      MocIdxType::U64(MocQtyType::Hpx(MocType::Cells(moc))) => {
        Ok(moc.into_cell_moc_iter().ranges().into_range_moc())
      }
      MocIdxType::U16(MocQtyType::Hpx(MocType::Ranges(moc))) => {
        Ok(moc.convert::<u64, Hpx<u64>>().into_range_moc())
      }
      MocIdxType::U32(MocQtyType::Hpx(MocType::Ranges(moc))) => {
        Ok(moc.convert::<u64, Hpx<u64>>().into_range_moc())
      }
      MocIdxType::U64(MocQtyType::Hpx(MocType::Ranges(moc))) => Ok(moc.into_range_moc()),
      _ => Err(String::from("Unsupported type in FITS file").into()),
    }
  }

  /// # Panics
  ///   If a `lat` is **not in** `[-pi/2, pi/2]`, this method panics.
  pub fn from_coos<I: Iterator<Item = (f64, f64)>>(
    depth: u8,
    coo_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    let layer = healpix::nested::get(depth);
    Self::from_fixed_depth_cells(
      depth,
      coo_it.map(move |(lon_rad, lat_rad)| layer.hash(lon_rad, lat_rad)),
      buf_capacity,
    )
  }

  /// # Input
  /// - `lon` the longitude of the center of the cone, in radians
  /// - `lat` the latitude of the center of the cone, in radians
  /// - `radius` the radius of the cone, in radians
  /// - `depth`: the MOC depth
  /// - `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  ///
  /// # Panics
  /// If this layer depth + `delta_depth` > the max depth (i.e. 29)
  pub fn from_cone(lon: f64, lat: f64, radius: f64, depth: u8, delta_depth: u8) -> Self {
    Self::from(cone_coverage_approx_custom(
      depth,
      delta_depth,
      lon,
      lat,
      radius,
    ))
  }

  /// Create a MOC from a not too large list of relatively large cones (i.e. cones containing a lot
  /// of cells).
  /// For a large list of small cones (i.e. cones containing only a few cells), see
  /// [from_small_cones](#from_small_cones)
  ///
  /// # Input
  /// - `depth`: the MOC depth
  /// - `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  /// - `Iterator of (lon, lat, radius)`, in (radians, radians, radians)
  ///
  /// # Panics
  /// If this layer depth + `delta_depth` > the max depth (i.e. 29)
  pub fn from_large_cones<I: Iterator<Item = (f64, f64, f64)>>(
    depth: u8,
    delta_depth: u8,
    coo_it: I,
  ) -> Self {
    /*kway_or_it(
      coo_it.map(|(lon, lat, radius)| Self::from_cone(lon, lat, radius, depth, delta_depth).into_range_moc_iter())
    )*/
    let it =
      coo_it.map(move |(lon, lat, radius)| Self::from_cone(lon, lat, radius, depth, delta_depth));
    kway_or(Box::new(it))
  }

  /// Create a MOC from a possibly large list of relatively small cones
  /// (i.e. cones containing a few cells).
  /// For a small list of large cones (i.e. cones containing a lot of cells), see
  /// [from_large_cones](#from_large_cones)
  ///
  /// See also [best_starting_depth](https://docs.rs/cdshealpix/latest/cdshealpix/fn.best_starting_depth.html)
  /// to help you in choosing a depth.
  ///
  /// # Input
  /// - `depth`: the MOC depth
  /// - `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  /// - `Iterator of (lon, lat, radius)`, in (radians, radians, radians)
  /// - `buf_capacity`: optional capacity of the internal builder buffer (number of cells at depth)
  ///
  /// # Panics
  /// If this layer depth + `delta_depth` > the max depth (i.e. 29)
  pub fn from_small_cones<I: Iterator<Item = (f64, f64, f64)>>(
    depth: u8,
    delta_depth: u8,
    cone_it: I,
    buf_capacity: Option<usize>,
  ) -> Self {
    Self::from_fixed_depth_cells(
      depth,
      cone_it.flat_map(move |(lon_rad, lat_rad, radius_rad)| {
        cone_coverage_approx_custom(depth, delta_depth, lon_rad, lat_rad, radius_rad)
          .into_flat_iter()
      }),
      buf_capacity,
    )
  }

  /// # Input
  /// - `lon` the longitude of the center of the elliptical cone, in radians
  /// - `lat` the latitude of the center of the elliptical cone, in radians
  /// - `a` the semi-major axis of the elliptical cone, in radians
  /// - `b` the semi-minor axis of the elliptical cone, in radians
  /// - `pa` the position angle (i.e. the angle between the north and the semi-major axis, east-of-north), in radians
  /// - `depth`: the MOC depth
  /// - `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  ///
  /// # Panics
  /// - if the semi-major axis is > PI/2
  /// - if this layer depth + `delta_depth` > the max depth (i.e. 29)
  pub fn from_elliptical_cone(
    lon: f64,
    lat: f64,
    a: f64,
    b: f64,
    pa: f64,
    depth: u8,
    delta_depth: u8,
  ) -> Self {
    Self::from(elliptical_cone_coverage_custom(
      depth,
      delta_depth,
      lon,
      lat,
      a,
      b,
      pa,
    ))
  }

  /// # Input
  /// - `lon` the longitude of the center of the ring, in radians
  /// - `lat` the latitude of the center of the ring, in radians
  /// - `radius_int` the internal radius of the ring, in radians
  /// - `radius_ext` the external radius of the ring, in radians
  /// - `depth`: the MOC depth
  /// - `delta_depth` the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  ///
  /// # Panics
  /// * If this layer depth + `delta_depth` > the max depth (i.e. 29)
  /// * If `radius_ext < radius_int`
  pub fn from_ring(
    lon: f64,
    lat: f64,
    radius_int: f64,
    radius_ext: f64,
    depth: u8,
    delta_depth: u8,
  ) -> Self {
    Self::from(ring_coverage_approx_custom(
      depth,
      delta_depth,
      lon,
      lat,
      radius_int,
      radius_ext,
    ))
  }

  /// # Input
  /// - `lon` the longitude of the center of the box, in radians
  /// - `lat` the latitude of the center of the box, in radians
  /// - `a` the semi-major axis of the box (half the box width), in radians
  /// - `b` the semi-minor axis of the box (half the box height), in radians
  /// - `pa` the position angle (i.e. the angle between the north and the semi-major axis, east-of-north), in radians
  /// - `depth`: the MOC depth
  ///
  /// # Panics
  /// - if `a` not in `]0, pi/2]`
  /// - if `b` not in `]0, a]`
  /// - if `pa` not in `[0, pi[`
  ///
  pub fn from_box(lon: f64, lat: f64, a: f64, b: f64, pa: f64, depth: u8) -> Self {
    Self::from(box_coverage(depth, lon, lat, a, b, pa))
  }

  /// # Input
  /// - `vertices` the list of vertices (in a slice) coordinates, in radians
  ///              `[(lon, lat), (lon, lat), ..., (lon, lat)]`
  /// - `complement` boolean used to get the complement of the polygon returned by default
  /// - `depth`: the MOC depth
  pub fn from_polygon(vertices: &[(f64, f64)], complement: bool, depth: u8) -> Self {
    Self::from(if !complement {
      polygon_coverage(depth, vertices, true)
    } else {
      custom_polygon_coverage(
        depth,
        vertices,
        &ContainsSouthPoleMethod::DefaultComplement,
        true,
      )
    })
  }

  /// # Input
  /// - `vertices` the list of vertices (in a slice) coordinates, in radians
  ///              `[(lon, lat), (lon, lat), ..., (lon, lat)]`
  /// - `control_point` the control point that must be inside the polygon, in radians
  /// - `depth`: the MOC depth
  pub fn from_polygon_with_control_point(
    vertices: &[(f64, f64)],
    control_point: (f64, f64),
    depth: u8,
  ) -> Self {
    let coo = Coo3D::from_sph_coo(control_point.0, control_point.1);
    let method = ContainsSouthPoleMethod::ControlPointIn(coo);
    let bmoc = custom_polygon_coverage(depth, vertices, &method, true);
    Self::from(bmoc)
  }

  /// # Input
  /// - `lon_min` the longitude of the bottom left corner, in radians
  /// - `lat_min` the latitude of the bottom left corner, in radians
  /// - `lon_max` the longitude of the upper left corner, in radians
  /// - `lat_max` the latitude of the upper left corner, in radians
  /// - `depth`: the MOC depth
  ///
  /// # Remark
  /// - If `lon_min > lon_max` then we consider that the zone crosses the primary meridian.
  /// - The north pole is included only if `lon_min == 0 && lat_max == pi/2`
  ///
  /// # Panics
  /// * if `lon_min` or `lon_max` not in `[0, 2\pi[`
  /// * if `lat_min` or `lat_max` not in `[-\pi/2, \pi/2[`
  /// * `lat_min >= lat_max`.
  pub fn from_zone(lon_min: f64, lat_min: f64, lon_max: f64, lat_max: f64, depth: u8) -> Self {
    Self::from(zone_coverage(depth, lon_min, lat_min, lon_max, lat_max))
  }
}

impl<T: Idx> RangeMOC<T, Time<T>> {
  pub fn from_microsec_since_jd0<I>(depth: u8, it: I, buf_capacity: Option<usize>) -> Self
  where
    I: Iterator<Item = u64>,
  {
    let shift = Time::<T>::shift_from_depth_max(depth) as u32;
    let mut builder = FixedDepthMocBuilder::new(depth, buf_capacity);
    for t in it {
      builder.push(T::from_u64_idx(t).unsigned_shr(shift));
    }
    builder.into_moc()
  }

  pub fn from_microsec_ranges_since_jd0<I>(depth: u8, it: I, buf_capacity: Option<usize>) -> Self
  where
    I: Iterator<Item = Range<u64>>,
  {
    Self::from_maxdepth_ranges(
      depth,
      it.map(|range| T::from_u64_idx(range.start)..T::from_u64_idx(range.end)),
      buf_capacity,
    )
  }

  /// Add the MOC external border of depth `self.depth_max`.
  pub fn expanded(&self) -> Self {
    let zero = T::zero();
    let n_cells_max = Time::<T>::n_cells_max();
    let one_at_max_depth: T =
      T::one().unsigned_shl(Time::<T>::shift_from_depth_max(self.depth_max) as u32);
    merge_sorted(
      self.depth_max,
      self.ranges.iter().map(|Range { start, end }| {
        let start = if *start > zero {
          *start - one_at_max_depth
        } else {
          *start
        };
        let end = if *end < n_cells_max {
          *end + one_at_max_depth
        } else {
          *end
        };
        start..end
      }),
    )
    .into_range_moc()
  }

  pub fn contracted(&self) -> Self {
    let one_at_max_depth: T =
      T::one().unsigned_shl(Time::<T>::shift_from_depth_max(self.depth_max) as u32);
    let two_at_max_depth = one_at_max_depth << 1;
    let ranges = self
      .ranges
      .iter()
      .filter_map(|Range { start, end }| {
        if (*end - *start) > two_at_max_depth {
          Some(*start + one_at_max_depth..*end - one_at_max_depth)
        } else {
          None
        }
      })
      .collect();
    Self::new(self.depth_max, MocRanges::new_unchecked(ranges))
  }
}

impl<T: Idx> RangeMOC<T, Frequency<T>> {
  pub fn from_freq_in_hz<I>(depth: u8, it: I, buf_capacity: Option<usize>) -> Self
  where
    I: Iterator<Item = f64>,
  {
    let shift = Frequency::<T>::shift_from_depth_max(depth) as u32;
    let mut builder = FixedDepthMocBuilder::new(depth, buf_capacity);
    for hz in it {
      builder.push(Frequency::<T>::freq2hash(hz).unsigned_shr(shift));
    }
    builder.into_moc()
  }

  pub fn from_freq_ranges_in_hz<I>(depth: u8, it: I, buf_capacity: Option<usize>) -> Self
  where
    I: Iterator<Item = Range<f64>>,
  {
    let mut builder = RangeMocBuilder::new(depth, buf_capacity);
    for range in it {
      builder.push(Frequency::<T>::freq2hash(range.start)..Frequency::<T>::freq2hash(range.end));
    }
    builder.into_moc()
  }

  /// Add the MOC external border of depth `self.depth_max`.
  pub fn expanded(&self) -> Self {
    let zero = T::zero();
    let n_cells_max = Frequency::<T>::n_cells_max();
    let one_at_max_depth: T =
      T::one().unsigned_shl(Frequency::<T>::shift_from_depth_max(self.depth_max) as u32);
    merge_sorted(
      self.depth_max,
      self.ranges.iter().map(|Range { start, end }| {
        let start = if *start > zero {
          *start - one_at_max_depth
        } else {
          *start
        };
        let end = if *end < n_cells_max {
          *end + one_at_max_depth
        } else {
          *end
        };
        start..end
      }),
    )
    .into_range_moc()
  }

  pub fn contracted(&self) -> Self {
    let one_at_max_depth: T =
      T::one().unsigned_shl(Frequency::<T>::shift_from_depth_max(self.depth_max) as u32);
    let two_at_max_depth = one_at_max_depth << 1;
    let ranges = self
      .ranges
      .iter()
      .filter_map(|Range { start, end }| {
        if (*end - *start) > two_at_max_depth {
          Some(*start + one_at_max_depth..*end - one_at_max_depth)
        } else {
          None
        }
      })
      .collect();
    Self::new(self.depth_max, MocRanges::new_unchecked(ranges))
  }
}

impl<T: Idx, Q: MocQty<T>> PartialEq for RangeMOC<T, Q> {
  fn eq(&self, other: &Self) -> bool {
    self.depth_max == other.depth_max && self.ranges.eq(&other.ranges)
  }
}

/// Iterator taking the ownership of the `RangeMOC` it iterates over.
pub struct RangeMocIter<T: Idx, Q: MocQty<T>> {
  depth_max: u8,
  iter: IntoIter<Range<T>>,
  last: Option<Range<T>>,
  _qty: PhantomData<Q>,
}
impl<T: Idx, Q: MocQty<T>> HasMaxDepth for RangeMocIter<T, Q> {
  fn depth_max(&self) -> u8 {
    self.depth_max
  }
}
impl<T: Idx, Q: MocQty<T>> ZSorted for RangeMocIter<T, Q> {}
impl<T: Idx, Q: MocQty<T>> NonOverlapping for RangeMocIter<T, Q> {}
impl<T: Idx, Q: MocQty<T>> MOCProperties for RangeMocIter<T, Q> {}
impl<T: Idx, Q: MocQty<T>> Iterator for RangeMocIter<T, Q> {
  type Item = Range<T>;
  fn next(&mut self) -> Option<Self::Item> {
    self.iter.next()
  }
  // Declaring size_hint, a 'collect' can directly allocate the right number of elements
  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
}
impl<T: Idx, Q: MocQty<T>> RangeMOCIterator<T> for RangeMocIter<T, Q> {
  type Qty = Q;

  fn peek_last(&self) -> Option<&Range<T>> {
    self.last.as_ref()
  }
}
impl<T: Idx, Q: MocQty<T>> RangeMOCIntoIterator<T> for RangeMOC<T, Q> {
  type Qty = Q;
  type IntoRangeMOCIter = RangeMocIter<T, Self::Qty>;

  fn into_range_moc_iter(self) -> Self::IntoRangeMOCIter {
    let l = self.ranges.0 .0.len();
    let last: Option<Range<T>> = if l > 0 {
      Some(self.ranges.0 .0[l - 1].clone())
    } else {
      None
    };
    RangeMocIter {
      depth_max: self.depth_max,
      iter: self.ranges.0 .0.into_vec().into_iter(),
      last,
      _qty: PhantomData,
    }
  }
}

/// Iterator borrowing the `RangeMOC` it iterates over.
pub struct RangeRefMocIter<'a, T: Idx, Q: MocQty<T>> {
  depth_max: u8,
  iter: slice::Iter<'a, Range<T>>,
  last: Option<Range<T>>,
  _qty: PhantomData<Q>,
}
impl<'a, T: Idx, Q: MocQty<T>> HasMaxDepth for RangeRefMocIter<'a, T, Q> {
  fn depth_max(&self) -> u8 {
    self.depth_max
  }
}
impl<'a, T: Idx, Q: MocQty<T>> ZSorted for RangeRefMocIter<'a, T, Q> {}
impl<'a, T: Idx, Q: MocQty<T>> NonOverlapping for RangeRefMocIter<'a, T, Q> {}
impl<'a, T: Idx, Q: MocQty<T>> MOCProperties for RangeRefMocIter<'a, T, Q> {}
impl<'a, T: Idx, Q: MocQty<T>> Iterator for RangeRefMocIter<'a, T, Q> {
  type Item = Range<T>;
  fn next(&mut self) -> Option<Self::Item> {
    self.iter.next().cloned()
  }
  // Declaring size_hint, a 'collect' can directly allocate the right number of elements
  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
}
impl<'a, T: Idx, Q: MocQty<T>> RangeMOCIterator<T> for RangeRefMocIter<'a, T, Q> {
  type Qty = Q;

  fn peek_last(&self) -> Option<&Range<T>> {
    self.last.as_ref()
  }
}
impl<'a, T: Idx, Q: MocQty<T>> RangeMOCIntoIterator<T> for &'a RangeMOC<T, Q> {
  type Qty = Q;
  type IntoRangeMOCIter = RangeRefMocIter<'a, T, Self::Qty>;

  fn into_range_moc_iter(self) -> Self::IntoRangeMOCIter {
    let l = self.ranges.0 .0.len();
    let last: Option<Range<T>> = if l > 0 {
      Some(self.ranges.0 .0[l - 1].clone())
    } else {
      None
    };
    RangeRefMocIter {
      depth_max: self.depth_max,
      iter: self.ranges.iter(),
      last,
      _qty: PhantomData,
    }
  }
}

impl<'a, T: Idx, Q: MocQty<T>> RangeRefMocIter<'a, T, Q> {
  /// # Warning
  /// Unsafe because:
  /// * we do not check that the quantity is the right one
  /// * we do not check that the ranges are sorted (they must be!), ...
  /// * we do not check that each range bounds is a cell not deeper than the given depth
  pub fn from_borrowed_ranges_unsafe(depth_max: u8, ranges: BorrowedRanges<'a, T>) -> Self {
    let l = ranges.0.len();
    let last: Option<Range<T>> = if l > 0 {
      Some(ranges.0[l - 1].clone())
    } else {
      None
    };
    Self {
      depth_max,
      iter: ranges.0.iter(),
      last,
      _qty: PhantomData,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_n_depth_max_cells() {
    let moc = RangeMOC::<u64, Hpx<u64>>::new_full_domain(0);
    assert_eq!(moc.n_depth_max_cells(), 12);
    let moc = RangeMOC::<u64, Hpx<u64>>::new_full_domain(2);
    assert_eq!(moc.n_depth_max_cells(), 192);
    let moc = RangeMOC::<u64, Hpx<u64>>::new_full_domain(24);
    assert_eq!(moc.n_depth_max_cells(), 3377699720527872);
  }

  #[test]
  fn test_to_fixed_depth_cells() {
    let moc = RangeMOC::<u64, Hpx<u64>>::new_full_domain(0);
    assert_eq!(
      moc.flatten_to_fixed_depth_cells().collect::<Vec<u64>>(),
      (0..12).into_iter().collect::<Vec<u64>>()
    );
    let moc = RangeMOC::<u64, Hpx<u64>>::new_full_domain(2);
    assert_eq!(
      moc.flatten_to_fixed_depth_cells().collect::<Vec<u64>>(),
      (0..192).into_iter().collect::<Vec<u64>>()
    );
  }

  #[test]
  fn test_from_polygon_aladinlitev3() {
    let vertices = [
      (2.762765958009174, -1.1558461939389928),
      (1.8511822776529407, -1.0933557485463217),
      (1.2223938385258124, -0.9257482282529843),
      (0.7433555354994513, -0.7588156605217202),
      (0.3057973131669505, -0.6330807489082455),
      (-0.13178353851370578, -0.5615526603403407),
      (-0.5856691165836956, -0.5400327446156468),
      (-0.4015158856736461, -0.255385904573062),
      (-0.36447295312493533, 0.055767062790012194),
      (-0.46118312909272624, 0.3439014598749883),
      (-0.7077120187325645, 0.5509825177593931),
      (-1.0770630524112423, 0.6032198101231719),
      (-1.4301394011313524, 0.46923928877261123),
      (-1.4878998343615297, 0.8530569092752573),
      (-1.9282441740007021, 1.1389659504677638),
      (-2.7514745722416727, 1.0909408969626568),
      (-3.043766621461535, 0.7764645583461347),
      (-3.0294959096529364, 0.4089232125719977),
      (-2.878305764522211, 0.05125802264111997),
      (3.0734739115502143, 0.057021963147195695),
      (2.7821939201916965, -0.05879878325187485),
      (2.5647762800185414, -0.27363879379223127),
      (2.4394176374317205, -0.5545388864809319),
      (2.444623710270669, -0.8678251267471937),
    ];
    let moc = RangeMOC::<u64, Hpx<u64>>::from_polygon(&vertices, false, 2);
    // println!("{:?}",moc.flatten_to_fixed_depth_cells().collect::<Vec<u64>>());
    // draw moc 2/32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 48, 49, 50, 51, 56, 57, 58, 64, 72, 73, 74, 75, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 108, 109, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 134, 136, 137, 138, 139, 144, 145, 146, 147, 148, 149, 150, 151, 152, 156, 157, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191
    for (lon, lat) in vertices {
      let hash = healpix::nested::hash(2, lon, lat);
      assert!(moc.contains_depth_max_val(&hash));
    }
  }

  #[test]
  fn test_border_elementary_edges() {
    let moc = RangeMOC::<u64, Hpx<u64>>::from_coos(8, [(0.0_f64, 0.0_f64)].iter().cloned(), None);
    let elementary_edges = moc
      .border_elementary_edges()
      .collect::<Vec<CellAndEdges<u64>>>();
    assert_eq!(elementary_edges.len(), 1);
    assert_eq!(elementary_edges[0].edges, OrdinalSet::all());

    let moc = moc.expanded();
    let elementary_edges = moc
      .border_elementary_edges()
      .collect::<Vec<CellAndEdges<u64>>>();
    assert_eq!(elementary_edges.len(), 8);
  }

  #[test]
  fn test_border_elementary_edges_2() {
    let range_moc =
      RangeMOC::from_fits_file("resources/CDS_P_DESI-Legacy-Surveys_DR10_color.moc.fits")
        .or_else(|_| {
          RangeMOC::from_fits_file("../resources/CDS_P_DESI-Legacy-Surveys_DR10_color.moc.fits")
        })
        .unwrap();
    let _ = range_moc
      .border_elementary_edges()
      .collect::<Vec<CellAndEdges<u64>>>();

    let range_moc = range_moc.degraded(5);
    // println!("{}", range_moc.to_ascii().unwrap());
    // let ext_border = range_moc.external_border();
    // println!("{}", ext_border.to_ascii().unwrap());
    let _ = range_moc
      .border_elementary_edges()
      .collect::<Vec<CellAndEdges<u64>>>();

    assert!(true)
  }
}
