<meta charset="utf-8"/>

# `moc`

The Rust MOC library used in
[MOCPy](https://github.com/cds-astro/mocpy),
[MOCli](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/cli) and
[MOCWasm](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/wasm).
Read, write, create and manipulate HEALPix **M**ulti-**O**rder **C**overage maps (**MOC**s),
i.e. discretized geomatrical surfaces on the unit sphere.

## About

This Rust library implements the v2.0 of the [MOC standard](https://ivoa.net/documents/MOC/),
including (S-)MOCs, T-MOCs and ST-MOCs.

It is used in:
* [MOCPy](https://github.com/cds-astro/mocpy), a Python wrapper to manipulate MOCs;
* a standalone command line tool [MOCli](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/cli) for linux, MacOS and Windows;
* a WASM library [MOCWasm](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/wasm) to be used in web browsers.

For tools able to display MOCs, see:
* the [Aladin Desktop](https://aladin.u-strasbg.fr/) sky atlas in Java (also supports MOC operations);
* [Aladin Lite](https://aladin.u-strasbg.fr/AladinLite/), "a lightweight version of the Aladin Sky Atlas running in the browser";
* [MOCPy](https://cds-astro.github.io/mocpy/) scripts, a python wrapper using the very same Rust MOC library.


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
* [ ] Implement the compact notation (bits coding quad-tree traversal) for S-MOCs (binary + ASCII Base 64)
* [ ] Implement compact S-MOC: single z-order curve sorted array of indices with a 2 bits flag telling
      whether the index is a single index, a range lower bound or a range upper bound
* [ ] Implement multi-MOC operations resorting to a sweep line like algo.
* [ ] Make a PostgresQL wrapper using e.g. [pgx](https://github.com/zombodb/pgx/)

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


## Acknowledgements

This work has been partly supported by the ESCAPE project.  
ESCAPE - The **E**uropean **S**cience **C**luster of **A**stronomy & **P**article Physics **E**SFRI Research Infrastructures -
has received funding from the **European Union’s Horizon 2020** research and innovation programme under **Grant Agreement no. 824064**.

