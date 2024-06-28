# `moc-wasm` Change Log

## 0.9.1

Released 2024-06-28

* Fix spurious coma in serialization of empty MOC in JSON


## 0.9.0

Released 2024-05-14

* Bump cdshealpix to 0.6.8
* remove `static ref` because of [this rust issue](https://github.com/rust-lang/rust/issues/114447)

## 0.8.2

Released 2024-01-31

* No modification, release due to new mocset release


## 0.8.1

Released 2023-12-20

* Fix issues S-MOC from STC-S
* Update cdshealpix


## 0.8.0

Released 2023-12-12

* Add `fromSTCS` function


## 0.7.0

Released 2023-04-21

### ⚠️ BREAKING Changes

The full API has been changed! 
Instead of relying on functions accessing MOCs by names,
we use methods on Javascirpt objects (MOC, TMOC, FMOC and STMOC). 
The internal store is now the same as the one used in [MOCPy](https://github.com/cds-astro/mocpy/).
See the README file and the example index.html file for more details.


## 0.6.0

Released 2023-03-28

* No modification, release due to new moc-cli release
* TBD: use MOC Lib Rust store, use object and indices on the JS side (like for MOCPy) 


## 0.5.3

Realeased 2022-11-10

* Supports ring indexed skymaps
* Add `TTYPE1=RANGE` keyword in FITS files (TTYPE is optional in the FITS standard but without
  it astropy seems not to be able to read the file)
* Add option in `toFits` functions to generate a FITS file compatible with v1.0
  of the MOC standard (S-MOC only).


## 0.5.2

Realeased 2022-09-12


## 0.5.1

Realeased 2022-09-09

### Bug correction

* Print the deepest order in JSON output even when it contains no cell


## 0.4.0

Realeased 2022-04-13

* Add support for (possibly gzipped) skymaps of fixed format
* Add gzip support for multi-resolution maps


## 0.3.2

Realeased 2022-03-22

* Fix all `toFile` methods (encoding problem when direclty saving a blob) 
* Add the possibility to overwrite the supported mime type when loading a FITS file from a URL  

## 0.3.1

Realeased 2022-02-07

* Fix erroneous inequality tests in `from_box` and `from_ellipse`


## 0.3.0

Released 2022-02-04

* Add moc from ring
* Add indirect split (in addition to direct split)
* More robust FITS deserialization with the UNIQ scheme (no bug in case of trailing 0 uniq indices)


## 0.2.0

Released 2021-11-09.

### Added

* `from_local_multiordermap`: load a multi-order map from a local file and create a MOC
* `from_multiordermap_url`: load a multi-order map from a URL file and create a MOC
* `from_multiordermap_fits_file`:  parse a multi-order map FITS file content and create a MOC
* `from_valued_cells`: MOC from multi-order map
* `split`: split a disjoint S-MOC into joint S-MOCs
* `split_count`: count the number of joint S-MOCs in a disjoint S-MOC


## 0.1.0

Released 2021-10-18.

## 0.1.0-alpha

Released 2021-08-16.

