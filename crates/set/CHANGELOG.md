# `moc-set` Change Log

## 0.8.2

Released 2024-01-31

### Added

* Naive parallelism in 'mocset query':
  we expect poor performances on HDD with cold cache but better ones with 
  SSDs with cold cache (parallel reading).
  Performances does not seem to improve a lot so far on a single MVNe SSD (x2 factor).
* `strip = "debuginfo"` in main `Cargo.toml` to reduce the size of the generated exec file.

### Bug correction

* No more 'panic' info showing-up on stderr when piping output in commands
  endding the process before full write, such as 'head'.


## 0.8.1

Released 2023-12-20

* No modification, release due to new moc-cli and moc-wasm release
* Only update cdshealpix


## 0.8.0

Released 2023-12-12

* Add 'union' command in moc-set
* Add possible list of IDs to 'union' and 'chgstatus' commands

### ⚠️ BREAKING Changes 

* In the command `chgstatus`, the identifier can now be a list,
  and it is now the last arguments so that 
```bash
mocset chgstatus mocset.bin 50 deprecated
``
is now
```bash
mocset chgstatus mocset.bin deprecated 50
```

## 0.7.0

Released 2023-04-21

* No modification, release due to new moc-wasm release


## 0.6.0

Released 2023-03-28

* No modification, release due to new moc-cli release


## 0.5.3

Realeased 2022-11-10

* Add `TTYPE1=RANGE` keyword in FITS files (TTYPE is optional in the FITS standard but without
  it astropy seems not to be able to read the file)
* Add option `force_v1` to extract a FITS file compatible with v1.0 of the MOC standard.


## 0.5.2

Released 2022-09-12.

### Bug correction

* Accept negative number in arguments


## 0.5.1

Released 2022-09-09.

### Bug correction

* Print the deepest order in JSON output even when it contains no cell


## 0.5.0

Released 2022-09-08.

