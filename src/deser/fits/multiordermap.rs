use std::{
  f64::consts::PI,
  io::{BufRead, BufReader, Read, Seek},
};

use byteorder::{BigEndian, ReadBytesExt};
use log::warn;

use crate::{
  deser::{
    fits::{
      common::{
        check_keyword_and_parse_uint_val, check_keyword_and_val, consume_primary_hdu,
        next_36_chunks_of_80_bytes,
      },
      error::FitsError,
      keywords::{FitsCard, MocKeywords, MocKeywordsMap, MocOrder, Ordering},
    },
    gz::{is_gz, uncompress},
  },
  elem::valuedcell::valued_cells_to_moc_with_opt,
  idx::Idx,
  moc::range::RangeMOC,
  mom::{HpxMOMIterator, HpxMomIter},
  qty::Hpx,
};

/// We expect the FITS file to be a BINTABLE containing a multi-order map.
/// To be fast (in execution and development), we start by a non-flexible approach in which we
/// expect the BINTABLE extension to contains:
/// ```bash
/// XTENSION= 'BINTABLE'           / binary table extension                         
/// BITPIX  =                    8 / array data type                                
/// NAXIS   =                    2 / number of array dimensions
/// NAXIS1  =                    ?? / length of dimension 1                          
/// NAXIS2  =                   ?? / length of dimension 2                          
/// PCOUNT  =                    0 / number of group parameters                     
/// GCOUNT  =                    1 / number of groups                               
/// TFIELDS =                   ?? / number of table fields
/// TTYPE1  = 'UNIQ    '                                                            
/// TFORM1  = 'K       '                                                            
/// TTYPE2  = 'PROBDENSITY'                                                         
/// TFORM2  = 'D       '                                                            
/// TUNIT2  = 'sr-1    '
/// ...
/// MOC     =                    T                                                  
/// PIXTYPE = 'HEALPIX '           / HEALPIX pixelisation                           
/// ORDERING= 'NUNIQ   '           / Pixel ordering scheme: RING, NESTED, or NUNIQ  
/// COORDSYS= 'C       '           / Ecliptic, Galactic or Celestial (equatorial)   
/// MOCORDER=                   ?? / MOC resolution (best order)
/// ...
/// END
/// ```
///
/// # Params
/// * `reader`: the reader over the FITS content
/// * `cumul_from`: the cumulative value from which cells are put in the MOC
/// * `cumul_to`: the cumulative value to which cells are put in the MOC
/// * `asc`: cumulative value computed from lower to highest densities instead of from highest to lowest
/// * `strict`: (sub-)cells overlapping the `cumul_from` or `cumul_to` values are not added
/// * `no_split`: cells overlapping the `cumul_from` or `cumul_to` values are not recursively split
/// * `reverse_decent`: perform the recursive decent from the highest cell number to the lowest (to be compatible with Aladin)
///
/// # Info
///   Supports gz input stream
///
pub fn from_fits_multiordermap<R: Read + Seek>(
  mut reader: BufReader<R>,
  cumul_from: f64,
  cumul_to: f64,
  asc: bool,
  strict: bool,
  no_split: bool,
  reverse_decent: bool,
) -> Result<RangeMOC<u64, Hpx<u64>>, FitsError> {
  if is_gz(&mut reader)? {
    let reader = uncompress(reader);
    from_fits_multiordermap_internal(
      reader,
      cumul_from,
      cumul_to,
      asc,
      strict,
      no_split,
      reverse_decent,
    )
  } else {
    from_fits_multiordermap_internal(
      reader,
      cumul_from,
      cumul_to,
      asc,
      strict,
      no_split,
      reverse_decent,
    )
  }
}

fn from_fits_multiordermap_internal<R: BufRead>(
  reader: R,
  cumul_from: f64,
  cumul_to: f64,
  asc: bool,
  strict: bool,
  no_split: bool,
  reverse_decent: bool,
) -> Result<RangeMOC<u64, Hpx<u64>>, FitsError> {
  let data_it = MultiOrderMapIterator::open(reader)?;
  let depth_max = data_it.depth_max;
  let area_per_cell = data_it.area_per_cell;
  let uniq_val_dens = data_it
    .map(|res_uniq_dens| {
      res_uniq_dens.map(|(uniq, dens)| {
        let (cdepth, _ipix) = Hpx::<u64>::from_uniq_hpx(uniq);
        let n_sub_cells = (1_u64 << (((depth_max - cdepth) << 1) as u32)) as f64;
        let value = dens * n_sub_cells * area_per_cell;
        (uniq, value, dens)
      })
    })
    .collect::<Result<Vec<(u64, f64, f64)>, FitsError>>()?;
  // Build the MOC
  let ranges = valued_cells_to_moc_with_opt(
    depth_max,
    uniq_val_dens,
    cumul_from,
    cumul_to,
    asc,
    strict,
    no_split,
    reverse_decent,
  );
  Ok(RangeMOC::new(depth_max, ranges))
}

/// Returns the sum of the multi-order map values associated with cells inside the given MOC.
/// If a cell is partially covered by the MOC, we apply on the value a factor equals to the ratio
/// of the cell area covered by the MOC over the total cell area.  
pub fn sum_from_fits_multiordermap<R: Read + Seek>(
  mut reader: BufReader<R>,
  moc: &RangeMOC<u64, Hpx<u64>>,
) -> Result<f64, FitsError> {
  if is_gz(&mut reader)? {
    let reader = uncompress(reader);
    sum_from_fits_multiordermap_internal(reader, moc)
  } else {
    sum_from_fits_multiordermap_internal(reader, moc)
  }
}

fn sum_from_fits_multiordermap_internal<R: BufRead>(
  reader: R,
  moc: &RangeMOC<u64, Hpx<u64>>,
) -> Result<f64, FitsError> {
  let data_it = MultiOrderMapIterator::open(reader)?;
  let depth_max = data_it.depth_max;
  let area_per_cell = data_it.area_per_cell;
  let mom = data_it
    .map(|res_uniq_dens| {
      res_uniq_dens.map(|(uniq, dens)| {
        let (cdepth, _ipix) = Hpx::<u64>::from_uniq_hpx(uniq);
        let n_sub_cells = (1_u64 << (((depth_max - cdepth) << 1) as u32)) as f64;
        let value = dens * n_sub_cells * area_per_cell;
        (uniq, value)
      })
    })
    .collect::<Result<Vec<(u64, f64)>, FitsError>>()?;
  let mom_it = HpxMomIter::<u64, Hpx<u64>, f64, _>::new(mom.into_iter());
  Ok(mom_it.sum_values_in_hpxmoc(moc))
}

struct MultiOrderMapIterator<R: BufRead> {
  /// Reader
  reader: R,
  /// MOM depth
  depth_max: u8,
  /// Area of a cell at the max MOM depth
  area_per_cell: f64,
  /// Number of rows to be read
  n_rows: u64,
  /// Number of rows already returned
  n_rows_consumed: u64,
  /// Used to consume row bytes we are not interested in
  sink: Vec<u8>,
}

impl<R: BufRead> MultiOrderMapIterator<R> {
  fn open(mut reader: R) -> Result<Self, FitsError> {
    let mut header_block = [b' '; 2880];
    consume_primary_hdu(&mut reader, &mut header_block)?;
    // Read the extention HDU
    let mut it80 = next_36_chunks_of_80_bytes(&mut reader, &mut header_block)?;
    // See Table 10 and 17 in https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf
    check_keyword_and_val(it80.next().unwrap(), b"XTENSION", b"'BINTABLE'")?;
    check_keyword_and_val(it80.next().unwrap(), b"BITPIX  ", b"8")?;
    check_keyword_and_val(it80.next().unwrap(), b"NAXIS  ", b"2")?;
    let n_bytes_per_row =
      check_keyword_and_parse_uint_val::<u64>(it80.next().unwrap(), b"NAXIS1  ")?;
    let n_rows = check_keyword_and_parse_uint_val::<u64>(it80.next().unwrap(), b"NAXIS2 ")?;
    check_keyword_and_val(it80.next().unwrap(), b"PCOUNT  ", b"0")?;
    check_keyword_and_val(it80.next().unwrap(), b"GCOUNT  ", b"1")?;
    let _n_cols = check_keyword_and_parse_uint_val::<u64>(it80.next().unwrap(), b"TFIELDS ")?;
    check_keyword_and_val(it80.next().unwrap(), b"TTYPE1 ", b"'UNIQ    '")?;
    check_keyword_and_val(it80.next().unwrap(), b"TFORM1 ", b"'K       '")?;
    check_keyword_and_val(it80.next().unwrap(), b"TTYPE2 ", b"'PROBDENSITY'")?;
    check_keyword_and_val(it80.next().unwrap(), b"TFORM2 ", b"'D       '")?;
    // nbits = |BITPIX|xGCOUNTx(PCOUNT+NAXIS1xNAXIS2x...xNAXISn)
    // In our case (bitpix = 8, GCOUNT = 1, PCOUNT = 0) => nbytes = n_cells * size_of(T)
    // let data_size n_bytes as usize * n_cells as usize; // N_BYTES ok since BITPIX = 8
    // Read MOC keywords
    let mut moc_kws = MocKeywordsMap::new();
    'hr: loop {
      for kw_record in &mut it80 {
        // Parse only MOC related keywords and ignore others
        if let Some(mkw) = MocKeywords::is_moc_kw(kw_record) {
          if let Some(previous_mkw) = moc_kws.insert(mkw?) {
            // A FITS keyword MUST BE uniq (I may be more relax here, taking the last one and not complaining)
            // return Err(FitsError::MultipleKeyword(previous_mkw.keyword_str().to_string()))
            warn!(
              "Keyword '{}' found more than once in a same HDU! We use the first occurrence.",
              previous_mkw.keyword_str()
            );
            moc_kws.insert(previous_mkw);
          }
          // else keyword added without error
        } else if &kw_record[0..4] == b"END " {
          break 'hr;
        }
      }
      // Read next 2880 bytes
      it80 = next_36_chunks_of_80_bytes(&mut reader, &mut header_block)?;
    }
    // Check header params
    moc_kws.check_pixtype()?;
    moc_kws.check_ordering(Ordering::Nuniq)?;
    moc_kws.check_coordsys()?;
    // - get MOC depth
    let depth_max = match moc_kws.get::<MocOrder>() {
      Some(MocKeywords::MOCOrder(MocOrder { depth })) => Ok(*depth),
      _ => Err(FitsError::MissingKeyword(MocOrder::keyword_string())),
    }?;
    let n_byte_skip = (n_bytes_per_row - 16) as usize;
    let sink = vec![0; n_byte_skip];
    let area_per_cell = (PI / 3.0) / (1_u64 << (depth_max << 1) as u32) as f64; // = 4pi / (12*4^depth)
    Ok(Self {
      reader,
      depth_max,
      area_per_cell,
      n_rows,
      n_rows_consumed: 0,
      sink,
    })
  }
}
impl<R: BufRead> Iterator for MultiOrderMapIterator<R> {
  type Item = Result<(u64, f64), FitsError>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.n_rows_consumed < self.n_rows {
      self.n_rows_consumed += 1;
      Some(
        u64::read::<_, BigEndian>(&mut self.reader)
          .and_then(|uniq| {
            self.reader.read_f64::<BigEndian>().and_then(|dens| {
              self
                .reader
                .read_exact(&mut self.sink)
                .map(|()| (uniq, dens))
            })
          })
          .map_err(FitsError::Io),
      )
    } else {
      None
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    let n_rows_remaining = (self.n_rows - self.n_rows_consumed) as usize;
    (n_rows_remaining, Some(n_rows_remaining))
  }
}

#[cfg(test)]
mod tests {

  use std::{fs::File, io::BufReader, path::PathBuf};

  use super::{from_fits_multiordermap, sum_from_fits_multiordermap};

  #[test]
  fn test_mutliordermap() {
    let path_buf1 = PathBuf::from("resources/LALInference.multiorder.fits");
    let path_buf2 = PathBuf::from("../resources/LALInference.multiorder.fits");
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    let res = from_fits_multiordermap(reader, 0.0, 0.9, false, true, true, false);
    match res {
      Ok(o) => {
        print!("{:?}", o);
        assert!(true)
      }
      Err(e) => {
        print!("{:?}", e);
        assert!(false)
      }
    }
  }

  #[test]
  fn test_mutliordermap_sum() {
    let path_buf1 = PathBuf::from("resources/LALInference.multiorder.fits");
    let path_buf2 = PathBuf::from("../resources/LALInference.multiorder.fits");
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    // First create MOC
    let moc = from_fits_multiordermap(reader, 0.0, 0.9, false, true, true, false).unwrap();

    // Then compute the sum inside the MOC (should be 90%, i.e, 0.9).
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    let sum = sum_from_fits_multiordermap(reader, &moc).unwrap();
    println!("value: {}", sum);
    assert!((0.8999..0.9001).contains(&sum));
  }
}
