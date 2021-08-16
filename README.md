<meta charset="utf-8"/>

# `moc`

MOC library, in Rust, used to read/write/create/manipulate HEALPix **M**ulti-**O**rder **C**overage maps (**MOC**s).

## About

This Rust library implements the v2.0 of the [MOC standard](https://ivoa.net/documents/MOC/),
including (S-)MOCs, T-MOCs and ST-MOCs.

It is used in:
* [MOCPy](https://github.com/cds-astro/mocpy), a Python wrapper to manipulate MOCs;
* a standalone [command line tool](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/cli)
* a [WASM library](https://github.com/cds-astro/cds-moc-rust/tree/main/crates/wasm) to be used in web browsers 

For tools able to display MOCs, see:
* the [Aladin Desktop](https://aladin.u-strasbg.fr/) sky atlas in Java (also supports MOC operations)
* the [Aladin Lite](https://aladin.u-strasbg.fr/AladinLite/), "a lightweight version of the Aladin Sky Atlas running in the browser".
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

