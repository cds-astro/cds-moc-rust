# `moc-cli` Change Log

## 0.3.0

Realeased 2022-01-XX

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

