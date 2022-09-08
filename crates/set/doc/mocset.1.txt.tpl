mocset(1)
=========

Name
----
mocset - query a set of HEALPix Multi-Order Coverages map (MOC) pre-saved in a 
single large binary file. The file can be seen as a persistent cache preventing
from having to open/read/parse a possible large set of FITS files.


Synopsis
--------

*mocset* _SUBCMD_ _SUBCMDPARAMS_

*mocset* _SUBCMD_ *--help*

*mocset* *--version*

*command* | *mocset* query MOCSET_FILE moc - --format [ascii|json]


SUBCMD
------
_make_::
  Make a new mocset

_list_::
  Provide the list of the MOCs in a mocset and the associated flags

_query_::
  Query a mocset

_chgstatus_::
  Change the status flag of the given MOCs identifiers (valid, 
  deprecated, removed)

_append_::
  Append the given MOCs to an existing mocset

_purge_::
  Purge the mocset removing physically the MOCs flagged as 'removed'

_extract_::
  Extracts a MOC from the given moc-set

Examples
--------

mocset make --n128 3 --moc-list moclist.txt --delimiter , mocset.bin

mocset list mocset.bin

mocset query moclist.bin pos 90.0 +0.0

mocset query mocset.bin cone 90.0 +0.0 320.0 --precision 5

mocset query mocset.bin moc YOUR_PATH/CDS_IX_59_xmm4dr9s.fits

moc from polygon 5 "(0.0,0.0),(10.0,0.0),(0.0,10.0)" ascii | mocset query mocset.bin moc - --format ascii

mocset chgstatus mocset.bin 1 deprecated

mocset chgstatus mocset.bin 1 valid

mocset chgstatus mocset.bin 1 removed

mocset append mocset.bin 3000 MY_PATH/my_new_moc.fits

mocset purge mocset.bin



DESCRIPTION
-----------



VERSION
-------
{VERSION}


HOMEPAGE
--------
https://github.com/cds-astro/moc

Please report bugs and feature requests in the issue tracker.


AUTHORS
-------
F.-X. Pineau <francois-xavier.pineau@astro.unistra.fr>


