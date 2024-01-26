<meta charset="utf-8"/>

# `moc`

Read, write, create and manipulate HEALPix **M**ulti-**O**rder **C**overage maps (**MOC**s),
i.e. discretized geomatrical surfaces on the unit sphere.

[![](https://img.shields.io/crates/v/moc.svg)](https://crates.io/crates/moc)
[![](https://img.shields.io/crates/d/moc.svg)](https://crates.io/crates/moc)
[![API Documentation on docs.rs](https://docs.rs/moc/badge.svg)](https://docs.rs/moc/)
[![BenchLib](https://github.com/cds-astro/cds-moc-rust/actions/workflows/bench.yml/badge.svg)](https://github.com/cds-astro/cds-moc-rust/actions/workflows/bench.yml)

MOC Lib Rust, the Rust MOC library used in:
* [MOCPy](https://github.com/cds-astro/mocpy),
* [MOCli](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/cli);
* [MOCSet](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/set);
* [MOCWasm](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/wasm);
* [Aladin Lite V3](https://github.com/cds-astro/aladin-lite/tree/develop);
  see the [Cargo.tom](https://github.com/cds-astro/aladin-lite/blob/develop/src/core/Cargo.toml) file.

MOC Lib Rust rely on the [CDS HEALPix Rust library](https://github.com/cds-astro/cds-healpix-rust).

## About

This Rust library implements the v2.0 of the [MOC standard](https://ivoa.net/documents/MOC/),
including (S-)MOCs, T-MOCs and ST-MOCs.  
It also implements a still experimental F-MOC (F for Frequency).

MOC Lib Rust is used in:
* [MOCPy](https://github.com/cds-astro/mocpy), a Python wrapper to manipulate MOCs;
* a standalone command line tool [MOCli](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/cli) for linux, MacOS and Windows;
* a standalone command line tool [MOCSet](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/set) for linux, MacOS and Windows;
* a WASM library [MOCWasm](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/wasm) to be used in web browsers.

For tools able to display MOCs, see:
* the [Aladin Desktop](https://aladin.u-strasbg.fr/) sky atlas in Java (also supports MOC operations);
* [Aladin Lite](https://aladin.u-strasbg.fr/AladinLite/), "a lightweight version of the Aladin Sky Atlas running in the browser";
* [MOCPy](https://cds-astro.github.io/mocpy/) scripts, a python wrapper using the very same Rust MOC library.

## Release

The github [releases](https://github.com/cds-astro/cds-moc-rust/releases) section number 
is the [MOCCli](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/cli),
[MOCSet](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/set)
and [MOCWasm](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/wasm) 
release number.

## Install/test

[Install rust](https://www.rust-lang.org/tools/install)
(and check that `~/.cargo/bin/` is in your path),
or update the Rust compiler with:
```bash
rustup update
``` 

Run tests (with or without seeing `stdout`):
```bash
cargo test
cargo test -- --nocapture
```
Run benches:
```bash
cargo bench
```
Build documentation
```bash
cargo doc --open
```

Build the library for fast test or final build
```bash
# Fast build (large not optimized file) 
cargo build
# Optimized file
cargo build --release
```

## Particularities

* The core of this library is very generic
* We implemented lazy, streamed operations:
    + an operation between 2 MOCs takes in input 2 iterators and returns an iterator (**streaming**)
    + you can combine operations by combining iterators at no cost;
      the process start when starting to iterate on the outermost iterator (**lazyness**)
```rust
// Signature of the Union operation between 2 2D-MOCs
pub fn or<T, Q, U, R, I1, J1, K1, I2, J2, K2>(
  left_it: K1,
  right_it: K2
) -> OrRange2Iter<T, Q, I1, I2>
  where
    T: Idx,       // Type of the 1st quantity (e.g. u32 or u64)
    Q: MocQty<T>, // First quantity type, e.g Time
    U: Idx,       // Type of the 2nd quantity (e.g. u32 or u64)
    R: MocQty<U>, // Second quantity type, e.g Space (we use Hpx for HEALPix)
    I1: RangeMOCIterator<T, Qty=Q>,
    J1: RangeMOCIterator<U, Qty=R>,
    K1: RangeMOC2ElemIt<T, Q, U, R, It1=I1, It2=J1>,
    I2: RangeMOCIterator<T, Qty=Q>,
    J2: RangeMOCIterator<U, Qty=R>,
    K2: RangeMOC2ElemIt<T, Q, U, R, It1=I2, It2=J2>
```

## Possible Enhancements / Ideas

* [ ] Add operations on `RangeMOC2`
    + [X] `or`
    + [ ] `and`, `complement`, `fold`, ...
* [X] Implement a function dividing a disjoint MOCs into a list of joint MOCs
      (tip: use the order and the flag of a BMOC, the flag telling is the cell has already been visited).
* [ ] Implement the compact notation (bits coding quad-tree traversal) for S-MOCs (binary + ASCII Base 64)
* [ ] Implement compact S-MOC: single z-order curve sorted array of indices with a 2 bits flag telling
      whether the index is a single index, a range lower bound or a range upper bound
* [ ] Make a PostgresQL wrapper using e.g. [pgx](https://github.com/zombodb/pgx/)?


## WARNING about the STC-S to MOC function

STC-S parsing is ensured by the [STC crate](https://github.com/cds-astro/cds-stc-rust).

Current discrepancies between the STC standard and this implementation:

* The `DIFFERENCE` operation has been implemented as being a `symmetric difference`
    + why? probably because:
        1. I am biased towards Boolean algebra, it as `XOR`
           (exclusive `OR` or symmetric difference) but no `Difference`
        2. I read parts of the STC standard after the STC-S implementation
        3. `XOR` is already implemented in [cdshleapix](https://github.com/cds-astro/cds-healpix-rust), but `DIFFERENCE` is not.
    + has stated in the STC standard: `R1 – R2 = R1 AND (NOT R2))`;
      but also: `R1 - R2 = R1 AND (R1 XOR R2)`, and
      `XOR = (R1 OR R2) AND (NOT (R1 AND R2))` is more complex that `DIFFERENCE`
      (so is worth having implented?).
* For `Polygon`: we do not use the STC convention
    + we support self-intersecting polygons
    + we generally return the smallest area polygon (use `NOT` to get its complement!)
    + one convention could be to use an additional (last) provided points as a control point
        - note that for convex polygons, the control point could be the vertices gravity center
        - in a GUI, a user could define the inner part of the polygon by a final click
    + why?
        1. efficient algorithms dealing with polygons supports self-intersecting polygons
        2. to support arbitrary defined polygons by a user clicking in a viewer such as Aladin or Aladin Lite
        3. [cdshleapix](https://github.com/cds-astro/cds-healpix-rust) is based on self-intersecting polygons
* For `Box`: a position angle can be added as a last parameter, right after `bsize`.

So far, we reject STC-S having:
* a frame different from `ICRS`
* a flavor different from `Spher2`
* units different from `degrees`

## License

Like most projects in Rust, this project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.


## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

### Warning

The code is formatted using 2 tab spaces instead of the regular 4:

```bash
cargo fmt -- --config tab_spaces=2
```


## Acknowledgements

This work has been partly supported by the ESCAPE project.  
ESCAPE - The **E**uropean **S**cience **C**luster of **A**stronomy & **P**article Physics **E**SFRI Research Infrastructures -
has received funding from the **European Union’s Horizon 2020** research and innovation programme under **Grant Agreement no. 824064**.

