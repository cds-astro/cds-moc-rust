moc(1)
=====

Name
----
moc - create and manipulate HEALPix Multi-Order Coverages maps (MOCs)


Synopsis
--------

*moc* _SUBCMD_ _SUBCMDPARAMS_

*moc* _SUBCMD_ *--help*

*moc* *--version*

*command* | *moc* from [pos|timestamp|timerange]  [_SUBCMDPARAM_]


SUBCMD
------
_table_::
  Print information tables

_info_::
  Print MOC information on an input FITS file

_convert_::
  Convert from on file format to another file format,
  or from an old FITS to a MOC2.0 compatible FITS.

_from_::
  Create a new MOC from an input data file or spherical object
  (cone, elliptical cone, ellipse, box, zone, polygon).

_op_::
  Perform an operation on a MOC or between two MOCs

_filter_::
  Filter a data file, printing the rows lying in a given MOC.


Examples
--------

moc table pos

moc table time

moc info MY_MOC.fits

moc convert MY_SMOC.json --type smoc fits             MY_MOC.fits

moc convert MY_SMOC.json --type smoc fits --force-u64 MY_MOC.fits

moc convert MY_MOC.fits ascii --fold 80

moc from cone 14 0.0 +0.0 0.25 fits MY_SMOC.fits

moc from ellipse 14 0.0 +0.0 0.75 0.5 45.0 fits MY_SMOC.fits

moc from zone 14 359.5 -0.5 0.5 0.5 MY_SMOC.fits

moc from box  14 0.0 +0.0 0.75 0.5 45.0 fits MY_SMOC.fits

moc from polygon 12 "(000.9160848095,+01.0736381331),(001.5961114529,-00.7062969568),(359.6079412529,+01.1296198985)" fits MY_MOC.fits --force-u64

moc from pos 7 MY_POS.csv -s , ascii --fold 80

moc from pos 7 MY_POS.csv -s , fits MY_SMOC.fits

moc filter position MY_SMOC.fits MY_DATA.csv --has-header --lon RAJ2000 --lat DECJ2000 --n-threads 4

moc op degrade 3 MY_MOC.fits ascii --fold 80

moc op complement MY_MOC.fits ascii --fold 80

moc op union LEFT_MOC.fits RIGHT_MOC.fits fits RES_MOC.fits

moc op inter LEFT_MOC.fits RIGHT_MOC.fits fits RES_MOC.fits

moc op diff  LEFT_MOC.fits RIGHT_MOC.fits fits RES_MOC.fits

moc op minus LEFT_MOC.fits RIGHT_MOC.fits fits RES_MOC.fits



DESCRIPTION
-----------



VERSION
-------
{VERSION}


HOMEPAGE
--------
https://github.com/cds-astro/cds-moc-rust

Please report bugs and feature requests in the issue tracker.


AUTHORS
-------
F.-X. Pineau <francois-xavier.pineau@astro.unistra.fr>


