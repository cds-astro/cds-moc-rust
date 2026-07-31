# stmoc-pp

A command-line tool to build and query a **persistent** set of Time range and
spatial Multi-Order Coverage map (MOC, see [this IVOA standard](https://ivoa.net/documents/MOC)) tuples.

## About

A ST-MOC is typically constructed from a collection of time ranges, each associated with an S-MOC.
A common example is a set of astronomical images, where each image is described by its observation time range
and its spatial footprint represented as an S-MOC.
Once a ST-MOC has been built, the original provenance information is lost.
In other words, given a sub-ST-MOC, it is no longer possible to determine which images contributed to it.
As a result, a ST-MOC cannot directly support the following use case:
> Given a ST-MOC, return all image identifiers whose (time range, S-MOC) pair intersects it.

The ST-MOC Persistent Precursor/Projenitors (STMOC-PP) is designed to address this limitation.
STMOC-PP is not itself a ST-MOC; rather, it stores the information required to efficiently construct a ST-MOC,
which explains the term "precursor/projectiro".
In addition, STMOC-PP is intended to be built once from a potentially very large dataset -- possibly larger
than the available RAM -- and then queried directly from disk.
This persistence property explains the term "persistent".

## Installation

### Compile from source code

[Install rust](https://www.rust-lang.org/tools/install)
(and check that `~/.cargo/bin/` is in your path),
or update the Rust compiler with:

```bash
rustup update
``` 

Clone the [moc lib rust](https://github.com/cds-astro/cds-moc-rust) project:

```dtd
git clone https://github.com/cds-astro/cds-moc-rust
```

Install from using `cargo`:

```bash
cargo install --path crates/pp
```

or

```
RUSTFLAGS='-C target-cpu=native' cargo install --path crates/pp
```

## Usage

Exec `stmocpp` or `stmocpp --help` to get the help message:

```bash
> stmocpp 
Command-line to create and query a ST-MOC persitent precursors set, i.e. a list of (Time rrange, S-MOC) couples.

Usage: stmocpp <COMMAND>

Commands:
  make   Make a new ST-MOC PP file
  info   Get information on a ST-MOC PP file
  list   List all entries of a ST-MOC PP file
  query  Query a ST-MOC PP file
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Creation of a STMOC-PP file

The input file must contain, for each line, and in this order, the four columns:

* column 1: a unique identifier, must be an integer in `[0, 4_294_967_296[`
* column 2: a starting time (inclusive), in JD, MJD, ISO, ...
* column 3: an ending time (exclusive), same format as starting time
* column 4: either (but the same for all rows)
    + a path to a FITS S-MOC file
    + an inlined ASCII S-MOC
    + a STC-S that will be transformed in a S-MOC

For example, for XMM FOVs (circle of 17 arcmin around FOV centres):

* input compatible with `stmocpp make --time-type mjd --smoc-type path ...` (or no `--smoc-type` option):

```bash
864053501,59205.242,59205.39,mocs/864053502.moc.fits
741581501,57016.97,57017.074,mocs/741581502.moc.fits
403390101,54134.484,54135.043,mocs/403390102.moc.fits
125310101,51696.44,51696.79,mocs/125310102.moc.fits
...
```

* input compatible with `stmocpp make --time-type mjd --smoc-type "stcs(13)" ...`:

```bash
0864053501,59205.242,59205.39,CIRCLE ICRS 0.006916667 -39.50072 0.283333
0741581501,57016.97,57017.074,CIRCLE ICRS 0.052 8.257417 0.283333
0403390101,54134.484,54135.043,CIRCLE ICRS 0.09345833 62.48517 0.283333
0125310101,51696.44,51696.79,CIRCLE ICRS 0.098291665 -25.101528 0.283333
...
```

* input compatible with `stmocpp make --time-type mjd --smoc-type ascii ...`:

```bash
864053501,2459205.742,2459205.89,8/262198 262201 9/1048781-1048783 1048816 10/4195103 ...
741581501,2457017.47,2457017.574,8/312271 9/1249078-1249079 1249081 1249128 1249130 ...
403390101,2454134.984,2454135.543,8/48879 9/195514-195515 195560 915319 915325 915327 ...
125310101,2451696.94,2451697.29,8/265468 9/1061850-1061851 1061854 1061876-1061878 ...
...
```

Then, to build the `pp` file for the first input file example, use the command line
(run `stmocpp make --help` for option details):

```bash
> stmocpp make --input input.txt --delimiter , --time-type mjd --page-size 1 stmocpp.bin
```

For option details, run the help:

```bash
> stmocpp make --help
Make a new ST-MOC PP file

Usage: stmocpp make [OPTIONS] <FILE>

Arguments:
  <FILE>  Output file, storing the ST-MOC PP

Options:
  -i, --input <FILE>
          Input file containing the 'id,tmin,tmax,moc_path' list (default: read 
from stdin) 'id' must be a positive integer that can be stored on 4 bytes
  -d, --delimiter <CHAR>
          Delimiter used in the input file [default: " "]
      --time-type <TIME>
          Input time format: 'jd' (julian date), 'mjd' (modified julian date), '
usec' (microsec since JD=0), 'isorfc' (Gregorian date-time, Rfc3339, WARNING: no
 conversion to TCB), or 'isosimple' (Gregorian date, 'YYYY-MM-DDTHH:MM:SS' WARNI
NG: no conversion to TCB) [default: jd]
      --smoc-type <SMOC>
          S-MOC format in the input file: 'path' (path to a FITS file), 'ascii' 
(inlined ASCII MOC) or 'stcs(DEPTH)' (compute from STC-S at given DEPTH) [defaul
t: path]
  -p, --page-size <PAGE_SIZE_KB>
          Size of the implicit R-Tree page, in kB [default: 4]
  -c, --chunk-size <EXTSORT_CHUNK_SIZE>
          External sort param: #rows in a chunk (a temporary file) [default: 500
000]
  -k, --k-way <EXTSORT_KWAY>
          External sort param: 'k' value of the external k-way merge sort [defau
lt: 7]
  -t, --tmp-dir <EXTSORT_TMP_DIR>
          External sort param: temp directory [default: .ext_sort_tmp]
  -h, --help
          Print help
```

### Check the STMOC-PP file content

For general information on the structure of a STMOC-PP file, run:

```bash
> stmocpp info stmocpp.bin
ST-MOC PP
* Version: 1
* Page size (kB): 1
* SMOC maximum depth: 13
* Number of entries: 3058539
* File size (B): 320234128
* Layout:
...
```

and, to list all entries:

```bash
# Without S-MOCs
> stmocpp list stmocpp.bin --time-fmt mjd entries
id,tmin,tmax
116200701,51575.387000000104,51575.55500000017
116340601,51577.46499999985,51577.50999999978
...

# With S-MOCs
> stmocpp list stmocpp.bin --time-fmt mjd entries --print-smoc
id,tmin,tmax,smoc
116200701,51575.387000000104,51575.55500000017,9/1484221-1484223 1484264 1484266-1484267 ...
116340601,51577.46499999985,51577.50999999978,9/1484221-1484223 1484264 1484266-1484267 ...
...
```

### Query the STMOC-PP file

So far, the tool allow for 2 query modes:

    1. by single time range and S-MOC: returns all identifiers intersecting the input (time range, S-MOC) couple,
       and possibly print the result of the intersection:

```bash
# Single cell S-MOC
> stmocpp query stmocpp.bin simple --input-time-fmt mjd 51575.40 51575.55 9/1484223
id
116200701

# Same result with an all-sky S-MOC
> stmocpp query stmocpp.bin simple --input-time-fmt mjd 51575.40 51575.55 0/0-11
id
116200701

# Add intersection
> stmocpp query -ts -o mjd stmocpp.bin simple --input-time-fmt mjd 51575.40 51575.55 9/1484223
id,tmin,tmax,smoc
116200701,51575.39999999991,51575.549999999814,9/1484223 13/
...

# See all options using both
> stmocpp query --help
...
> stmocpp query simple --help
...
```

    2. by ST-MOCs: returns all identifiers intersecting the ST-MOC (may contains duplicates),
       and possibly print the result of the intersection for each entry:

```bash
> stmocpp query -ts -o mjd xmm.stmocpp.v2.bin stmoc stmoc.fits
```

## Example / Bench

Test on HST obscore table. 3 millions entries, ...
TBW!

## Data structure and algorithm

The global structure, in the file, is an implicit one dimensional R-Tree.

Here the following assumptions:

* the input is made a list of rows, each one associated to an input image, containing:
    + a image identifier (an integer);
    + a time range (a long integer couple);
    + the path a the associated SMOC.
* there is no very wide time ranges overlapping a significant part of other time ranges.

Here the building algorithm:

* perform an external sort on all entries, comparing the time range centers $(max + min)/2$.
* compute the size of 1 dimensional R-tree nodes
* sequentially read the sorted file and for each entry:
    + load the SMOC and compute its size
    + append the entries in the leaf part of the file, stored on 4 `u64`:
        - tmin: `u64` (8B)
        - tmax: `u64` (8B)
        - smoc depth: `u8` (1B)
        - smoc starting byte offset: `u56` (7B)
        - smoc len in bytes: `u32` (4B)
        - smoc id: `u32` (4B)
    + append the smoc in the smoc part of the file
* compute and write the node part of the R-tree

The file structure is the following one:

* minimal header (32 bytes):
    + magic number: `ST-MOC PP Ver` (13B)
    + version number: `u8` (1B)
    + page size (in kB):  `u8` (1B)
    + maximum SMOC depth: `u8` (1B)
    + total number of entries: `u32` (4B, less than 4 billion)
    + leaves byte offset: `u32` (4B, less than 4GB)
    + total file size: `u64` (8B, written at the end to be check the file has been correctly written)
* nodes part of the implicit R-tree
* leaves of the implicit R-tree
* SMOCs

### Remarks

For simplicity, especially in debugging, I have chose to separate store R-Tree nodes
and leaves in separated locations.
Hence, it is easy to perform brut force queries iterating on all leaves, completly ignoring
the node hierarchy.
However, I do think that performances would have been better storing leaves in the node hierarchy.
(better chances to be in the CPU cache).

Also, in thi code, for the query part, I decided to use a leaf iterator resorting to
`Box<dyn Iterator>` (dynamic dispatch) instead of a more classical visitor (allowing for static dispatch).
I suppose it also has a negative impact on the performances.
The `visitor` algorithm can be implemented in parallel to test this hypothesis.

Possible mprovement: we could store int eh header `tmin` and `tmax` (and maybe the union of all S-MOCs)
for fast rejection tests!

It is possible to extend the code to support updates, in appending mode only (i.e. adding time ranges more recent
than the one already in the file), till teh R-Tree structure is fully balanced (regular pages only).

## To-do list

* [ ] Make command to build a ST-MOC from a STMOC-PP file?
* [ ] Make a query between two STMOC-PP files (returning the identifiers in both files)?
* [ ] Implements the quick rejection test based at least on tmin and tmax (to be added on the header)?
* [ ] When querying from a ST-MOC we known that `tmax(range[i]) <= tmin(range[i+1])`, it could help
  in testing less ranges in the RTree root page (improvements to be implemented).
* [ ] Implements the Visitor to compare to the dynamic dispatch based iterator approach.
* [ ] Clean the code and add more comments.

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
