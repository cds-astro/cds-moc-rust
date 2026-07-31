stmocpp(1)
==========

Name
----
stmocpp - query a set of (time range, S-MOC) tuples (i.e. teh precursors/projnitors of a ST-MOC)
pre-saved in a single large binary file which is indexed with an implicit RTree on time ranges.


Synopsis
--------

*stmocpp* _SUBCMD_ _SUBCMDPARAMS_

*stmocpp* _SUBCMD_ *--help*

*stmocpp* *--version*

*command* | *stmocpp* make ...


SUBCMD
------
_make_::
  Make a new stmocpp file

_info_::
  Get information on the structure/content of a stmocpp file

_list_::
  Provide the list of the (time range, S-MOC) tuples in the stmocpp file

_query_::
  Query a stmocpp


Examples
--------

stmocpp make --input input.txt --delimiter , --time-type mjd --page-size 1 stmocpp.bin

stmocpp info stmocpp.bin

stmocpp list stmocpp.bin --time-fmt mjd entries --print-smoc

stmocpp query -ts -o mjd stmocpp.bin simple --input-time-fmt mjd 51575.40 51575.55 9/1484223

stmocpp query --help

stmocpp query simple --help



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


