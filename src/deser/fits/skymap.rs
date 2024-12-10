use std::{
  io::{self, BufRead, BufReader, Read, Seek},
  mem::size_of,
};

use byteorder::{BigEndian, ReadBytesExt};
use log::warn;

use healpix::depth;

use crate::{
  deser::{
    fits::{
      common::{
        check_keyword_and_get_str_val, check_keyword_and_parse_uint_val, check_keyword_and_val,
        consume_primary_hdu, next_36_chunks_of_80_bytes,
      },
      error::FitsError,
      keywords::{FitsCard, IndexSchema, MocKeywords, MocKeywordsMap, MocOrder, Nside, Ordering},
    },
    gz::{is_gz, uncompress},
  },
  elem::{
    cell::Cell, cellrange::CellRange, range::MocRange, valuedcell::valued_cells_to_moc_with_opt,
  },
  moc::range::RangeMOC,
  qty::Hpx,
};

/// We expect the FITS file to be a BINTABLE containing a skymap.
/// [Here](https://gamma-astro-data-formats.readthedocs.io/en/latest/skymaps/healpix/index.html)
/// a description of the format.
/// We so far implemented a subset of the format only:
/// * `INDXSCHM= 'IMPLICIT'`
/// * `ORDERING= 'NESTED  '`
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
/// TTYPE1  = 'XXX'       // SHOULD STARS WITH 'PROB', else WARNING                                                            
/// TFORM1  = 'XXX'       // MUST CONTAINS D (f64) or E (f32)                                                            
/// TUNIT1  = 'pix-1    '
/// TTYPE2  = ???                                                         
/// TFORM2  = ???                                                            
/// ...
/// MOC     =                    T                                                  
/// PIXTYPE = 'HEALPIX '           / HEALPIX pixelisation                           
/// ORDERING= 'NESTED  '           / Pixel ordering scheme: RING, NESTED, or NUNIQ  
/// COORDSYS= 'C       '  // WARNING if not found
/// NSIDE    =                  ?? / MOC resolution (best nside)
///  or
/// ORDER    =                  ?? / MOC resolution (best order), superseded by NSIDE
///                                / (because NSIDE which are not power of 2 are possible in RING)
/// INDXSCHM= 'IMPLICIT'           / Indexing: IMPLICIT or EXPLICIT
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
#[allow(clippy::too_many_arguments)]
pub fn from_fits_skymap<R: Read + Seek>(
  mut reader: BufReader<R>,
  skip_value_le_this: f64,
  cumul_from: f64,
  cumul_to: f64,
  asc: bool,
  strict: bool,
  no_split: bool,
  reverse_decent: bool,
) -> Result<RangeMOC<u64, Hpx<u64>>, FitsError> {
  if is_gz(&mut reader)? {
    from_fits_skymap_internal(
      uncompress(reader),
      skip_value_le_this,
      cumul_from,
      cumul_to,
      asc,
      strict,
      no_split,
      reverse_decent,
    )
  } else {
    from_fits_skymap_internal(
      reader,
      skip_value_le_this,
      cumul_from,
      cumul_to,
      asc,
      strict,
      no_split,
      reverse_decent,
    )
  }
}

#[allow(clippy::too_many_arguments)]
fn from_fits_skymap_internal<R: BufRead>(
  mut reader: R,
  skip_value_le_this: f64,
  cumul_from: f64,
  cumul_to: f64,
  asc: bool,
  strict: bool,
  no_split: bool,
  reverse_decent: bool,
) -> Result<RangeMOC<u64, Hpx<u64>>, FitsError> {
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
  // check_keyword_and_val(it80.next().unwrap(), b"TTYPE1 ", b"'PROB    '")?;
  // check_keyword_and_val(it80.next().unwrap(), b"TFORM1 ", b"'D       '")?;
  let ttype1 = check_keyword_and_get_str_val(it80.next().unwrap(), b"TTYPE1 ")?;
  if !ttype1.to_uppercase().starts_with("PROB") {
    let err = FitsError::UnexpectedValue(
      String::from("TTYPE1"),
      String::from("starts with 'PROB'"),
      String::from(ttype1),
    );
    warn!("{}", err);
  }
  let tform1 = check_keyword_and_get_str_val(it80.next().unwrap(), b"TFORM1 ")?;
  let (is_f64, n_pack) = if tform1 == "D" || tform1 == "1D" {
    Ok((true, 1_u64))
  } else if tform1 == "E" || tform1 == "1E" {
    Ok((false, 1_u64))
  } else if tform1 == "1024E" {
    Ok((false, 1024_u64))
  } else {
    // TODO: ALSO SUPPORT B or 1B and TTYPE = M (for MASK)!!
    Err(FitsError::UnexpectedValue(
      String::from("TFORM1"),
      String::from("['D', '1D', 'E', '1E' or '1024E']"),
      String::from(tform1),
    ))
  }?;

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
  // moc_kws.check_ordering(Ordering::Nested)?;
  if let Err(e) = moc_kws.check_coordsys() {
    warn!("{}", e);
  }
  moc_kws.check_index_schema(IndexSchema::Implicit)?;
  // - get MOC depth
  let depth_max = match moc_kws.get::<MocOrder>() {
    Some(MocKeywords::MOCOrder(MocOrder { depth })) => *depth,
    _ => match moc_kws.get::<Nside>() {
      Some(MocKeywords::Nside(Nside { nside })) => {
        if healpix::is_nside(*nside) {
          depth(*nside)
        } else {
          return Err(FitsError::Custom(format!(
            "Nside is not valid (to be used in nested mode at least): {}",
            nside
          )));
        }
      }
      _ => return Err(FitsError::MissingKeyword(MocOrder::keyword_string())),
    },
  };
  if n_rows * n_pack != healpix::n_hash(depth_max) {
    return Err(FitsError::Custom(format!(
      "Number of elements {} do not match number of HEALPix cells {}",
      n_rows * n_pack,
      healpix::n_hash(depth_max)
    )));
  }
  // Read data
  let (uniq_val_dens, cumul_skipped) = match moc_kws.get::<Ordering>() {
    Some(MocKeywords::Ordering(Ordering::Nested)) => {
      if is_f64 {
        let first_elem_byte_size = size_of::<f64>() * n_pack as usize;
        let n_byte_skip = n_bytes_per_row as usize - first_elem_byte_size;
        load_from_nested(
          reader,
          |r| r.read_f64::<BigEndian>(),
          skip_value_le_this,
          depth_max,
          n_pack,
          n_byte_skip,
          n_rows,
        )
      } else {
        let first_elem_byte_size = size_of::<f32>() * n_pack as usize;
        let n_byte_skip = n_bytes_per_row as usize - first_elem_byte_size;
        load_from_nested(
          reader,
          |r| r.read_f32::<BigEndian>().map(|v| v as f64),
          skip_value_le_this,
          depth_max,
          n_pack,
          n_byte_skip,
          n_rows,
        )
      }?
    }
    Some(MocKeywords::Ordering(Ordering::Ring)) => {
      if is_f64 {
        let first_elem_byte_size = size_of::<f64>() * n_pack as usize;
        let n_byte_skip = n_bytes_per_row as usize - first_elem_byte_size;
        load_from_ring(
          reader,
          |r| r.read_f64::<BigEndian>(),
          skip_value_le_this,
          depth_max,
          n_pack,
          n_byte_skip,
          n_rows,
        )
      } else {
        let first_elem_byte_size = size_of::<f32>() * n_pack as usize;
        let n_byte_skip = n_bytes_per_row as usize - first_elem_byte_size;
        load_from_ring(
          reader,
          |r| r.read_f32::<BigEndian>().map(|v| v as f64),
          skip_value_le_this,
          depth_max,
          n_pack,
          n_byte_skip,
          n_rows,
        )
      }?
    }
    Some(MocKeywords::Ordering(other_ordering)) => {
      return Err(FitsError::UnexpectedValue(
        Ordering::keyword_string(),
        format!(
          "{} or {}",
          Ordering::Nested.to_fits_value(),
          Ordering::Ring.to_fits_value()
        ),
        other_ordering.to_fits_value(),
      ))
    }
    Some(_other_keyword) => unreachable!(),
    None => return Err(FitsError::MissingKeyword(Ordering::keyword_string())),
  };
  // Build the MOC
  let ranges = valued_cells_to_moc_with_opt(
    depth_max,
    uniq_val_dens,
    cumul_from - cumul_skipped,
    cumul_to - cumul_skipped,
    asc,
    strict,
    no_split,
    reverse_decent,
  );
  Ok(RangeMOC::new(depth_max, ranges))
}

#[allow(clippy::type_complexity)]
fn load_from_nested<R, F>(
  mut reader: R,
  read_f64: F,
  skip_value_le_this: f64,
  depth_max: u8,
  n_pack: u64,
  n_byte_skip: usize,
  n_rows: u64,
) -> io::Result<(Vec<(u64, f64, f64)>, f64)>
where
  R: BufRead,
  F: Fn(&mut R) -> io::Result<f64>,
{
  let mut sink = vec![0; n_byte_skip];
  let mut prev_range = 0..0;
  let mut prev_val = 0.0;
  let mut uniq_val_dens: Vec<(u64, f64, f64)> = Vec::with_capacity(10_240);
  let mut cumul_skipped = 0_f64;
  if n_pack == 1 {
    for ipix in 0..n_rows {
      let val = read_f64(&mut reader)?;
      // - we skip too low value (e.g. all cells set to 0)
      // - we pack together, in a same range, consecutive cells having the same value
      //   and we build a multi resolution map to reuse existing code
      if val > skip_value_le_this {
        if val == prev_val && ipix == prev_range.end {
          prev_range.end = ipix + 1;
        } else {
          if prev_range.start != prev_range.end {
            let moc_range: MocRange<u64, Hpx<u64>> =
              CellRange::from_depth_range(depth_max, prev_range.clone()).into();
            for moc_cell in moc_range {
              let n_cells = 1_u64 << ((depth_max - moc_cell.depth()) << 1);
              let uniq = Cell::<u64>::from(moc_cell).uniq_hpx();
              let uval = prev_val * (n_cells as f64);
              uniq_val_dens.push((uniq, uval, prev_val))
            }
          }
          prev_val = val;
          prev_range = ipix..ipix + 1;
        }
      } else {
        cumul_skipped += val;
      }
      // Skip other columns bits
      reader.read_exact(&mut sink)?;
    }
  } else {
    for i_row in 0..n_rows {
      let start = i_row * n_pack;
      for ipix in start..start + n_pack {
        let val = read_f64(&mut reader)?;
        // - we skip too low value (e.g. all cells set to 0)
        // - we pack together, in a same range, consecutive cells having the same value
        //   and we build a multi resolution map to reuse existing code
        if val > skip_value_le_this {
          if val == prev_val && ipix == prev_range.end {
            prev_range.end = ipix + 1;
          } else {
            if prev_range.start != prev_range.end {
              let moc_range: MocRange<u64, Hpx<u64>> =
                CellRange::from_depth_range(depth_max, prev_range.clone()).into();
              for moc_cell in moc_range {
                let n_cells = 1_u64 << ((depth_max - moc_cell.depth()) << 1);
                let uniq = Cell::<u64>::from(moc_cell).uniq_hpx();
                let uval = prev_val * (n_cells as f64);
                uniq_val_dens.push((uniq, uval, prev_val))
              }
            }
            prev_val = val;
            prev_range = ipix..ipix + 1;
          }
        } else {
          cumul_skipped += val;
        }
      }
      // Skip other columns bits
      reader.read_exact(&mut sink)?;
    }
  }
  if prev_range.start != prev_range.end {
    let moc_range: MocRange<u64, Hpx<u64>> =
      CellRange::from_depth_range(depth_max, prev_range).into();
    for moc_cell in moc_range {
      let n_cells = 1_u64 << ((depth_max - moc_cell.depth()) << 1);
      let uniq = Cell::<u64>::from(moc_cell).uniq_hpx();
      let uval = prev_val * (n_cells as f64);
      uniq_val_dens.push((uniq, uval, prev_val))
    }
  }
  Ok((uniq_val_dens, cumul_skipped))
}

#[allow(clippy::type_complexity)]
fn load_from_ring<R, F>(
  mut reader: R,
  read_f64: F,
  skip_value_le_this: f64,
  depth_max: u8,
  n_pack: u64,
  n_byte_skip: usize,
  n_rows: u64,
) -> io::Result<(Vec<(u64, f64, f64)>, f64)>
where
  R: BufRead,
  F: Fn(&mut R) -> io::Result<f64>,
{
  let nested_layer = healpix::nested::get(depth_max);
  let mut sink = vec![0; n_byte_skip];
  let mut uniq_val_dens: Vec<(u64, f64, f64)> = Vec::with_capacity(10_240);
  let mut cumul_skipped = 0_f64;
  if n_pack == 1 {
    for ipix_ring in 0..n_rows {
      let val = read_f64(&mut reader)?;
      // - we skip too low value (e.g. all cells set to 0)
      if val > skip_value_le_this {
        let ipix_nested = nested_layer.from_ring(ipix_ring);
        let uniq = Hpx::<u64>::uniq_hpx(depth_max, ipix_nested);
        uniq_val_dens.push((uniq, val, val))
      } else {
        cumul_skipped += val;
      }
      // Skip other columns bits
      reader.read_exact(&mut sink)?;
    }
  } else {
    for i_row in 0..n_rows {
      let start = i_row * n_pack;
      for ipix_ring in start..start + n_pack {
        let val = read_f64(&mut reader)?;
        // - we skip too low value (e.g. all cells set to 0)
        if val > skip_value_le_this {
          let ipix_nested = nested_layer.from_ring(ipix_ring);
          let uniq = Hpx::<u64>::uniq_hpx(depth_max, ipix_nested);
          uniq_val_dens.push((uniq, val, val))
        } else {
          cumul_skipped += val;
        }
      }
      // Skip other columns bits
      reader.read_exact(&mut sink)?;
    }
  }
  Ok((uniq_val_dens, cumul_skipped))
}

#[cfg(test)]
mod tests {

  use super::from_fits_skymap;
  use crate::deser::fits::ranges_to_fits_ivoa;
  use crate::moc::RangeMOCIntoIterator;
  use std::fs::File;
  use std::io::{BufReader, BufWriter};
  use std::path::PathBuf;

  // Perform only in release mode (else slow: the decompressed fits files is 1.6GB large)!
  #[cfg(not(debug_assertions))]
  #[test]
  fn test_skymap_v1() {
    let path_buf1 = PathBuf::from("resources/Skymap/bayestar.fits.gz");
    let path_buf2 = PathBuf::from("../resources/Skymap/bayestar.fits.gz");
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);

    let res = from_fits_skymap(reader, 0.0, 0.0, 0.9, false, true, true, false);
    match res {
      Ok(o) => {
        let path_buf1 = PathBuf::from("resources/Skymap/bayestar.moc.out.fits");
        let path_buf2 = PathBuf::from("../resources/Skymap/bayestar.moc.out.fits");
        let file = File::create(&path_buf1)
          .or_else(|_| File::create(&path_buf2))
          .unwrap();
        let writer = BufWriter::new(file);
        print!("{:?}", &o);
        ranges_to_fits_ivoa(o.into_range_moc_iter(), None, None, writer).unwrap();
        assert!(true)
      }
      Err(e) => {
        print!("{:?}", e);
        assert!(false)
      }
    }
  }

  #[test]
  fn test_skymap_v2() {
    let path_buf1 = PathBuf::from("resources/Skymap/gbuts_healpix_systematic.fits");
    let path_buf2 = PathBuf::from("../resources/Skymap/gbuts_healpix_systematic.fits");

    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);

    let res = from_fits_skymap(reader, 0.0, 0.0, 0.9, false, true, true, false);
    match res {
      Ok(o) => {
        let path_buf1 = PathBuf::from("resources/Skymap/gbuts_healpix_systematic.moc.out.fits");
        let path_buf2 = PathBuf::from("../resources/Skymap/gbuts_healpix_systematic.moc.out.fits");
        let file = File::create(&path_buf1)
          .or_else(|_| File::create(&path_buf2))
          .unwrap();
        let writer = BufWriter::new(file);
        print!("{:?}", &o);
        ranges_to_fits_ivoa(o.into_range_moc_iter(), None, None, writer).unwrap();
        assert!(true)
      }
      Err(e) => {
        print!("{:?}", e);
        assert!(false)
      }
    }
  }

  #[test]
  fn test_skymap_v3() {
    let path_buf1 = PathBuf::from("resources/Skymap/gbm_subthresh_514434454.487999_healpix.fits");
    let path_buf2 =
      PathBuf::from("../resources/Skymap/gbm_subthresh_514434454.487999_healpix.fits");

    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);

    let res = from_fits_skymap(reader, 0.0, 0.0, 0.9, false, true, true, false);
    match res {
      Ok(o) => {
        print!("{:?}", o);

        print!("{}", o.to_ascii().unwrap());

        assert!(true)
      }
      Err(e) => {
        print!("{:?}", e);
        assert!(false)
      }
    }
  }

  #[test]
  fn test_skymap_v4() {
    let path_buf1 = PathBuf::from("resources/Skymap/hese_59031_run00134244.evt000034406854.fits");
    let path_buf2 =
      PathBuf::from("../resources/Skymap/hese_59031_run00134244.evt000034406854.fits");

    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);

    let res = from_fits_skymap(reader, 0.0, 0.0, 0.9, false, true, true, false);
    match res {
      Ok(o) => {
        // print!("{:?}", o);

        print!("{}", o.to_ascii().unwrap());

        assert!(true)
      }
      Err(e) => {
        print!("{:?}", e);
        assert!(false)
      }
    }
  }

  #[test]
  fn test_skymap_v5() {
    let path_buf1 = PathBuf::from("resources/Skymap/cWB.fits.gz");
    let path_buf2 = PathBuf::from("../resources/Skymap/cWB.fits.gz");

    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);

    let res = from_fits_skymap(reader, 0.0, 0.0, 0.9, false, true, true, false);
    match res {
      Ok(o) => {
        // print!("{:?}", o);
        print!("{}", o.to_ascii().unwrap());
        assert!(true)
      }
      Err(e) => {
        print!("{:?}", e);
        assert!(false)
      }
    }
  }
}
