# `moc` Change Log

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

