# `moc` Change Log

## 0.19.1

Released 2025-09-26

### Changed

* The way F-MOCs are computed (including fmin, fmax and the number of orders; see comments in the code)!

## 0.18.0

Released 2025-05-23

### Changed

* Ignore too deep `NUNIQ` in bugged FITS file from another lib
* Accept `MOCORDER` with no `MOCORD_S` in FITS file of v2.0
* Change `exec_on_readwrite_store` signature
* Change 'xxST_xx' metods names into `xx2Dxx` names, and conversely

### Added

* Accept FITS `TFORM = B/I/J/K` in addition to `1B/1I/1K/1K` in FITS files
* Add `refine` method on MOCs
* Add SF-MOC support!!!

### Fixed

* Fix the `ST-MOC` `contains_val` method
* Fix ST-MOC method `from_time_and_coos` (was no called in MOCPy)

## 0.17.0

Released 2024-10-16

### Changed

* Bump cdshealpix version to 0.7
* Use u8 slice `trim_ascii_start` and `trim_ascii_end` now that they have been stabilized

### Added

* Add methods in `RangeMOC` and `storage`:
    + `from_small_cones_par`
    + `from_small_boxes`, `from_small_boxes_par`
    + `from_large_boxes`, `from_large_boxes_par`
    + `border_elementary_edges_vertices`
* Add `multiordermap_filter_mask_moc` in `storage`

## 0.16.0

Released 2024-XX-XX

### Added

* MOM filtering to return values in a MOC and associated weights

## 0.15.0

Released 2024-06-27

### Fixed

* Remove spurious coma in empty MOC JSON serialization

### Added

* Add methods `all_cells_with_unidirectional_neig`
* Re-export 'OrdinalMap' and 'OrdinalSet'
* Add a `BorrowedRangeMOC` struct with method `overlap` operator
* Add method `overlapped_by_iter` to both `RangeMOC` and `BorrowedRangeMOC`

## 0.14.2

Released 2024-05-28

### Added

* Method 'all_cells_with_unidirectional_neigs' for AladinLite
* Re-export `cdshealpix::compass_point::OrdinalMap` and `cdshealpix::compass_point::OrdinalSet` in `moc::range`

## 0.14.1

Released 2024-05-28

## Added

* Re-export `cdshealpix::compass_point::Ordinal` in `moc::range`

## 0.14.0

Released 2024-05-27

### Added

* `U64MocStore.new_empty_stmoc`
* `is_empty` in `CellOrCellRangeMOC` and `CellOrCellRangeMOC2`

### Changed

* Add max time depth and max space depth at the end of the ST-MOC ASCII representation
* Add max time depth and max space depth at the end of the ST-MOC JSON representation

### Fixed

* Fix the empty ASCII represention of an empty ST-MOC
* Fix the empty JSON represention of an empty ST-MOC
* Empty ST-MOC loaded as... empty (instead of containing one element imade of an empty T-MOC and and emtpy S-MOC)

## 0.13.0

Released 2024-05-14

### Added

* `CellSelection` in methods building a RangeMOC from a BMOC
* `mutliresolution/order map (mom or mrm) sum`

### Changed

* Bump cdshealpix to 0.6.8

## 0.12.1

Released 2023-12-20

### Fixed

* Bugs in the stcs2moc: allky not complete + intesection error comming from cdshealpix BMOC
* Update CDSHealpix

## 0.12.0

Released 2023-12-11

### Added

* stcs2moc functionnality

## 0.11.4

Never Released

### Added

* test index validity in ASCII MOCs

## 0.11.3 (no bug fix, but minor add)

Released 2023-03-31

### Added

* Methods for frequency MOCs in store

## 0.11.2

Released 2023-03-06

### Fixed

* Wrong constant (pi/2 instead of pi) when checking elliptical cone position angle

## 0.11.1

Released 2023-02-17

### Fixed

* Dumb initialization of the counts in the store

## 0.11.0

Released 2023-02-17

### Added

* Operations `fill_holes` and `fill_holes_smaller_than`
* Add reference count in store to handle external MOC copies
  (e.g. when using python multiprocessing silnetly resorting
  on pickle, which cause bugs since the MOC store index is no
  more uniq and is may be dropped)

## 0.10.1

Released 2023-02-13

### Fixed

* Remove spurious "WARNING: Keyword 'TTYPE1' found more than once in a same HDU! We use the first occurrence."
  when readding FITs files

## 0.10.0

Released 2023-02-13

### Fixed

* Computation of T-MOCs and F-MOCs from ranges

### Added

* Add computation of a MOC mean centre and the maximum distance from a given point.
* Add generation of PNG files to visualize a MOC
    + add the mapproj crate dependency
    + add the png crate dependency
* Supports ring indexed skymaps
* Switch to standard ASCII serialisation in RangeMOC
* Add `TTYPE1=RANGE` keyword in FITS files (TTYPE is optional in the FITS standard but without
  it astropy seems not to be able to read the file)
* Add the `CellHpxMOCIterator` trait to easily save S-MOC in FITS files compatible with v1.0
  of the MOC standard.
* Add the store features (for MOCPy, MOCWasm, ...)

## 0.9.0

Released 2022-09-09

### Bug correction

* Print the deepest order in JSON output even when it contains no cell

## 0.9.0-alpha

Released 2022-06-17

### Added

* Add Frequency MOCs

## 0.8.0

Released 2022-04-13

### Added

* Add the multi `or` operation
* Add MOC from mulitple cones (a lot of small cones or a resonnable number of large cones)
* Add support for specific FITS skymaps (possibly gzipped)
* Add gzip support for FITS Multi-Order Map

## 0.7.1

Released 2022-03-22 (not a bug fix)

### Added

* Possibility to perform operations on borrowed ranges in addition to owned ranges

## 0.7.0

Released 2022-02-04

### Added

* Add MOC fromm ring
* Add the possibility to choose indirect neighbours (8, instead of the 4 direct neighbours) when splitting a MOC

### Enhancement

* Make FITS deserialization more robust for UNIQ indices
  (to cope with a -- now fixed -- Aladin bug adding trailing '0' uniq indices)

## 0.6.1

Released 2021-11-15

### Modification

* Remove the wasm/nowasm `create_from_time_ranges_spatial_coverage` instead of wasm only (used in MOCPy)

## 0.6.0

Released 2021-11-15

### Added

* Add well formed ASCII/Json tests at deserialization
* `Split` a disjoint MOC into joint MOCs
* Add direct support for FITS Multi-Order Map

## 0.5.0

Released 2021-10-18

### Added

* ST-MOC union on iterators
* ST-MOC builder from (time\_idx, pos\_idx) iterator
* ST-MOC builder from (time\_range, pos\_idx) iterator
* Options to MocFromValuedCells

### Bug correction

* Correct a bug in MocFromValuedCells
* Several other bug corrections

## 0.4.0

Put apart from MOCPy: 2021-08-16

### Previous versions

The original code was part of [MOCPy](https://github.com/cds-astro/mocpy).
It has evolved and put in this separated crate.
See `src/interval` (then renamed `src/moc`)
in the [mocpy](https://github.com/cds-astro/mocpy) project.

