# `moc-cli` Change Log

## 0.11.0

Released 2025-09-26

* Change the way F-MOC are computed (including fmin, fmax and the number of orders).

## 0.10.1

Released 2025-06-10

* No modification, release due to new moc-set release

## 0.10.0

Released 2025-05-23

* Bump cdshealpix to 0.7.x
* Add SF-MOC support
* Accept FITS `TFORM = B/I/J/K` in addition to `1B/1I/1K/1K` in FITS files
* Accept `MOCORDER` with no `MOCORD_S` in FITS file of v2.0
* Ignore too deep `NUNIQ` in bugged FITS file from another lib

## 0.9.1

Released 2024-06-28

* Fix spurious coma in serialization of empty MOC in JSON

## 0.9.0

Released 2024-05-14

* Add `moc op momsum`
* Bump cdshealpix to 0.6.8

## 0.8.2

Released 2024-01-31

* Add `strip = "debuginfo"` in main `Cargo.toml` to reduce the size of the generated exec file.

## 0.8.1

Released 2023-12-20

* Fix issues S-MOC from STC-S
* Update cdshealpix

## 0.8.0

Released 2023-12-12

* Build a S-MOC from a STC-S string
* Minor option bound checks updated

## 0.7.0

Released 2023-04-21

* No modification, release due to new moc-wasm release

## 0.6.0

Released 2023-03-28

* Fix `from timeranges`
* Add the `view` command to save PNG files and display S-MOCs.
* Add `fillexcept` and `fillholes` operations

## 0.5.3

Released 2022-11-10

* Supports ring indexed skymaps
* Add `TTYPE1=RANGE` keyword in FITS files (TTYPE is optional in the FITS standard but without
  it astropy seems not to be able to read the file)
* Add option `force_v1` to save a FITS file compatible with v1.0 of the MOC standard (S-MOC only).

## 0.5.2

Released 2022-09-12

## 0.5.1

Released 2022-09-09

### Bug correction

* Print the deepest order in JSON output even when it contains no cell

## 0.5.0-alpha

Released 2022-06-17

* Add support for frequency MOCs
* Add 'hprint' (human print) command for time and frequency

## 0.4.0

Released 2022-04-13

* Add `from multi` to build a MOC from muliple regions at once
* Add `from cones` to build a MOC from multiple cones (possibly a lot of small cones) at once
* Accept (possibly gzipped) multi-resolution fits files
* Accept (possibly gzipped) skymap fits files

## 0.3.2

Released 2022-03-22

* No change, new release because of MOCwasm, moc-lib updated

## 0.3.1

Released 2022-02-07

* No change, new release because of MOCwasm

## 0.3.0

Released 2022-02-04

* Add moc from ring
* Add indirect split (in addition to direct split)
* More robust FITS deserialization with the UNIQ scheme (no bug in case of trailing 0 uniq indices)

## 0.2.0

Released 2021-11-09.

* Add `split` operation on S-MOCs

## 0.1.0

Released 2021-10-18.

* Add creation of S-MOC from a Multi-Order Map, i.e. a non-overlapping list of (uniq, value) rows
* Add building ST-MOC from (time, ra, dec) rows
* Add building ST-MOC from (tmin, tmax, ra, dec) rows
* Replace ST-MOC in-memory union by an streaming (iterator-based) union

## 0.1.0-alpha

Released 2021-08-16.

