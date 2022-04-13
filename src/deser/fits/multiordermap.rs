
use std::io::{Read, Seek, BufRead, BufReader};
use std::f64::consts::PI;
use std::marker::PhantomData;

use byteorder::{ReadBytesExt, BigEndian};

use crate::idx::Idx;
use crate::qty::Hpx;
use crate::elem::valuedcell::valued_cells_to_moc_with_opt;
use crate::moc::range::RangeMOC;
use crate::deser::{
  gz::{is_gz, uncompress},
  fits::{
    error::FitsError,
    keywords::{
      MocKeywordsMap, MocKeywords, FitsCard,
      Ordering, MocOrder
    },
    common::{
      consume_primary_hdu, next_36_chunks_of_80_bytes, check_keyword_and_val,
      check_keyword_and_parse_uint_val
    }
  }
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
) -> Result<RangeMOC<u64, Hpx::<u64>>, FitsError> {
  if is_gz(&mut reader)? {
    let reader = uncompress(reader);
    from_fits_multiordermap_internal(reader,cumul_from, cumul_to, asc, strict, no_split, reverse_decent)
  } else {
    from_fits_multiordermap_internal(reader, cumul_from, cumul_to, asc, strict, no_split, reverse_decent)
  }
}

fn from_fits_multiordermap_internal<R: BufRead>(
  mut reader: R,
  cumul_from: f64,
  cumul_to: f64,
  asc: bool,
  strict: bool,
  no_split: bool,
  reverse_decent: bool,
) -> Result<RangeMOC<u64, Hpx::<u64>>, FitsError> {
  let mut header_block = [b' '; 2880];
  consume_primary_hdu(&mut reader, &mut header_block)?;
  // Read the extention HDU
  let mut it80 = next_36_chunks_of_80_bytes(&mut reader, &mut header_block)?;
  // See Table 10 and 17 in https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf
  check_keyword_and_val(it80.next().unwrap(), b"XTENSION", b"'BINTABLE'")?;
  check_keyword_and_val(it80.next().unwrap(), b"BITPIX  ", b"8")?;
  check_keyword_and_val(it80.next().unwrap(), b"NAXIS  ", b"2")?;
  let n_bytes_per_row = check_keyword_and_parse_uint_val::<u64>(it80.next().unwrap(), b"NAXIS1  ")?;
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
          eprintln!("WARNING: Keyword '{}' found more than once in a same HDU! We use the first occurrence.", previous_mkw.keyword_str());
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
  let depth_max = match moc_kws.get(PhantomData::<MocOrder>) {
    Some(MocKeywords::MOCOrder(MocOrder { depth })) => *depth,
    _ => return Err(FitsError::MissingKeyword(MocOrder::keyword_string())),
  };
  // Read data
  let n_byte_skip = (n_bytes_per_row - 16) as usize;
  let mut sink = vec![0; n_byte_skip];
  let area_per_cell = (PI / 3.0) / (1_u64 << (depth_max << 1) as u32) as f64;  // = 4pi / (12*4^depth)
  let mut uniq_val_dens: Vec<(u64, f64, f64)> = Vec::with_capacity(n_rows as usize);
  for _ in 0..n_rows {
    let uniq = u64::read::<_, BigEndian>(&mut reader)?;
    let dens = reader.read_f64::<BigEndian>()?;
    let (cdepth, _ipix) = Hpx::<u64>::from_uniq_hpx(uniq);
    let n_sub_cells = (1_u64 << (((depth_max - cdepth) << 1) as u32)) as f64;
    uniq_val_dens.push((uniq, dens * n_sub_cells * area_per_cell, dens));
    /*{
      // Discard remaining row bytes
      io::copy(&mut reader.by_ref().take(n_byte_skip), &mut io::sink());
    }*/
    reader.read_exact(&mut sink)?;
  }
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


#[cfg(test)]
mod tests {
  
  use std::path::PathBuf;
  use std::fs::File;
  use std::io::BufReader;
  use super::from_fits_multiordermap;
  
  #[test]
  fn test_mutliordermap() {
    let path_buf1 = PathBuf::from("resources/LALInference.multiorder.fits");
    let path_buf2 = PathBuf::from("../resources/LALInference.multiorder.fits");
    let file = File::open(&path_buf1).or_else(|_| File::open(&path_buf2)).unwrap();
    let reader = BufReader::new(file);
    let res = from_fits_multiordermap(reader, 0.0, 0.9, false, true, true, false);
    match res {
      Ok(o) => {
        print!("{:?}", o);
        assert!(true)
      },
      Err(e) => {
        print!("{:?}", e);
        assert!(false)
      },
    }
  }

}