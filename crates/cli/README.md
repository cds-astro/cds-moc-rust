<meta charset="utf-8"/>

# `moc-cli`

Executable to read/write/create/manipulate HEALPix **M**ulti-**O**rder **C**overage maps (**MOC**s) using command lines.


## About

This **C**ommand *Line** **I**nterface (CLI) is made from the [Rust MOC library](TODO: TBD).
It implements the v2.0 of the [MOC standard](https://ivoa.net/documents/MOC/),
including (S-)MOCs, T-MOCs and ST-MOCs.

For tools able to display MOCs, see:
* the [Aladin Desktop](https://aladin.u-strasbg.fr/) sky atlas in Java (also supports MOC operations)
* the [Aladin Lite](https://aladin.u-strasbg.fr/AladinLite/), "a lightweight version of the Aladin Sky Atlas running in the browser".
* [MOCPy](https://cds-astro.github.io/mocpy/), a python wrapper using the very same Rust MOC library.

## Install/test

### From the source code

[Install rust](https://www.rust-lang.org/tools/install)
(and check that `~/.cargo/bin/` is in your path),
or update the Rust compiler with:
```bash
rustup update
``` 

Install from [crates.io] using `cargo`:
```bash
cargo install moc-cli
```
(due to a heavy use of [monomorphization](https://en.wikipedia.org/wiki/Monomorphization), 
the compilation time may be very long, i.e. more than a minute).


### From pre-compile binaries

TBD/TBW

## Command line help

Once installed, you can get help messages using `moc [SUBCOMMAND [SUBSUBCOMMAND [...]]] --help`. 

A the root level `moc --help`:
```bash
Create, manipulate and filter files using HEALPix Multi-Order Coverage maps (MOCs).

USAGE:
    moc <SUBCOMMAND>

[...]

SUBCOMMANDS:
    convert    Converts an input format to the (most recent versions of) an output format
    filter     Filter file rows using a MOC
    from       Create a MOC from given parameters
    help       Prints this message or the help of the given subcommand(s)
    info       Prints information on the given MOC
    op         Perform operations on MOCs
    table      Prints MOC constants
```

`moc from --help`:
```bash
USAGE:
    moc from <SUBCOMMAND>

[...]

SUBCOMMANDS:
    box          Create a Spatial MOC from the given box
    cone         Create a Spatial MOC from the given cone
    ellipse      Create a Spatial MOC from the given elliptical cone
    help         Prints this message or the help of the given subcommand(s)
    polygon      Create a Spatial MOC from the given polygon
    pos          Create a Spatial MOC from a list of positions in decimal degrees (one pair per line, longitude
                 first, then latitude)
    timerange    Create a Time MOC from a list of time range (one range per line, lower bound first, then upper
                 bound)
    timestamp    Create a Time MOC from a list of timestamp (one per line)
    zone         Create a Spatial MOC from the given zone
```

`moc op --help`:
```bash
USAGE:
    moc op <SUBCOMMAND>

[...]

SUBCOMMANDS:
    complement    Performs a logical 'NOT' on the input MOC (= MOC complement)
    contract      Remove an the internal border made of cells having the MOC depth, SMOC only
    degrade       Degrade the input MOC (= MOC complement)
    diff          Performs a logical 'XOR' between 2 MOCs (= MOC difference)
    extborder     Returns the MOC external border (made of cell of depth the MOC depth), SMOC only
    extend        Add an extra border of cells having the MOC depth, SMOC only
    help          Prints this message or the help of the given subcommand(s)
    intborder     Returns the MOC internal border (made of cell of depth the MOC depth), SMOC only
    inter         Performs a logical 'AND' between 2 MOCs (= MOC intersection)
    minus         Performs the logical operation 'AND(left, NOT(right))' between 2 MOCs (= left minus right)
    sfold         Returns the union of the T-MOCs associated to S-MOCs intersecting the given S-MOC. Left: S-MOC,
                  right: ST-MOC, res: T-MOC
    tfold         Returns the union of the S-MOCs associated to T-MOCs intersecting the given T-MOC. Left: T-MOC,
                  right: ST-MOC, res: S-MOC
    union         Performs a logical 'OR' between 2 MOCs (= MOC union)
```

and so on (e.g `moc op degrade --help`).

## Examples

```bash
moc table space
moc info resources/V_147_sdss12.moc.fits
moc info resources/CDS-I-125A-catalog_MOC.fits
moc op inter resources/V_147_sdss12.moc.fits resources/CDS-I-125A-catalog_MOC.fits fits my_res.fits
moc info my_res.fits

moc from cone 11 0.0 +0.0 0.1 ascii --fold 50 my_cone.ascii
moc convert -t smoc my_cone.ascii fits -f my_cone.fits
```

Building a MOC from the [Hipparcos](https://vizier.u-strasbg.fr/viz-bin/VizieR-3?-source=I/239/hip_main&-out.max=50&-out.form=HTML%20Table&-out.add=_r&-out.add=_RAJ,_DEJ&-sort=_r&-oc.form=sexa)
positions:
```bash
egrep "^ *[0-9]" hip.psv | cut -d '|' -f 2-3 | tr -d ' ' | moc from pos 5 - --separator '|' ascii
```

## Performances hint

### Build MOC from positions

On a regular desktop, it took **3.7s** to build the MOC at **order 7** of the **16,622,442** positions of the
[KIDS DR2](https://vizier.u-strasbg.fr/viz-bin/VizieR-3?-source=II/344&-out.max=50&-out.form=HTML%20Table&-out.add=_r&-out.add=_RAJ,_DEJ&-sort=_r&-oc.form=sexa)
table:
```bash
time moc from pos 7 kids_dr2.csv -s , ascii --fold 80 > kids_dr2.moc.ascii
> 3.7s on 16_622_442 position in a file of 552 MB
```

### Filter file using a MOC

On a classical HDD (~130 MB/s), the disk is the limiting factor when filtering a file.
Test perform on a 25 GB file containing 16_622_443 rows (KIDS DR2):

|              |       HDD        |       SSD       |
|--------------|------------------|-----------------|
| `wc -l`      | 3m21s = 127 MB/s | 19s = 1347 MB/s |
| `moc filter` | 3m21s = 127 MB/s | 31s =  825 MB/s |

We get the same results with or without multithreading.  

Now we select only 3 fields. We get a ~1 GB (961 MB) file.
Since the results are the same for HDD and SSD, we deduce that the full file is in the disk cache:

|                 |       HDD        |       SSD       |
|-----------------|------------------|-----------------|
| `wc -l`         | 0.3s = 3200 MB/s | 0.3s = 3200 MB/s |
| `moc filter`    | 4s   =  240 MB/s | 4s   =  240 MB/s |
| `moc filter 4T` | 2s   =  480 MB/s | 2s   =  480 MB/s |

Commands used:
```bash
time moc filter position SMOC_GLIMPSE_u32.fits kids_dr2.csv --has-header --lon RAJ2000 --lat DECJ2000 > /dev/null
time moc filter position SMOC_GLIMPSE_u32.fits kids_dr2.csv --has-header --lon RAJ2000 --lat DECJ2000 --n-threads 4 > /dev/null
```
(no rows in output)


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

This work has been supported by the ESCAPE project.  
ESCAPE - The **E**uropean **S**cience **C**luster of **A**stronomy & **P**article Physics **E**SFRI Research Infrastructures -
has received funding from the **European Union’s Horizon 2020** research and innovation programme under **Grant Agreement no. 824064**.

