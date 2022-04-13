# `moc` Change Log

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

