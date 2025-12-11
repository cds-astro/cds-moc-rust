//! Contains code to generate an image from a given MOC

use std::{
  f64::consts::PI,
  fs::File,
  io::{self, BufWriter, Write},
  ops::RangeInclusive,
  path::Path,
};

use healpix::nested::{self, map::img::PosConversion};
use mapproj::{
  img2celestial::Img2Celestial, img2proj::ReversedEastPngImgXY2ProjXY, math::HALF_PI,
  pseudocyl::mol::Mol, zenithal::sin::Sin, CanonicalProjection, CenteredProjection, ImgXY, LonLat,
};

use crate::{
  elem::cell::Cell,
  idx::Idx,
  moc::{range::RangeMOC, CellMOCIterator, RangeMOCIntoIterator, RangeMOCIterator},
  qty::{Hpx, MocQty},
};

pub fn to_img_auto<T: Idx>(
  smoc: &RangeMOC<T, Hpx<T>>,
  img_size_y: u16,
  pos_convert: Option<PosConversion>,
) -> (Vec<u8>, (u16, u16)) {
  let (lon, lat) = smoc.into_range_moc_iter().cells().mean_center();
  let r_max = smoc
    .into_range_moc_iter()
    .cells()
    .max_distance_from(lon, lat);

  let proj_center = Some((lon, lat));
  if r_max > HALF_PI {
    // Mollweide, all sky
    let img_size = (img_size_y << 1, img_size_y);
    let rgba = to_img_default(smoc, img_size, proj_center, None, pos_convert);
    (rgba, img_size)
  } else {
    // Sinus, computed bounds from d_max
    let img_size = (img_size_y, img_size_y);
    let bound = r_max.sin() * 1.05; // add 5% of the distance
    let proj_bounds = Some((-bound..=bound, -bound..=bound));
    let rgba = to_img(
      smoc,
      img_size,
      Sin::new(),
      proj_center,
      proj_bounds,
      pos_convert,
    );
    (rgba, img_size)
  }
}

/// Returns an RGBA array (each pixel is made of 4 successive u8: RGBA) using the Mollweide projection.
///
/// # Params
/// * `smoc`: the Spatial MOC to be print;
/// * `size`: the `(X, Y)` number of pixels in the image;
/// * `proj_center`: the `(lon, lat)` coordinates of the center of the projection, in radians,
///                      if different from `(0, 0)`;
/// * `proj_bounds`: the `(X, Y)` bounds of the projection, if different from the default values
///                  which depends on the projection. For unbounded projections, de default value
///                  is `(-PI..PI, -PI..PI)`.
/// * `pos_convert`: to handle a different coordinate system between the MOC and the image.
pub fn to_img_default<T: Idx>(
  smoc: &RangeMOC<T, Hpx<T>>,
  img_size: (u16, u16),
  proj_center: Option<(f64, f64)>,
  proj_bounds: Option<(RangeInclusive<f64>, RangeInclusive<f64>)>,
  pos_convert: Option<PosConversion>,
) -> Vec<u8> {
  to_img(
    smoc,
    img_size,
    Mol::new(),
    proj_center,
    proj_bounds,
    pos_convert,
  )
}

/// Returns an RGBA array (each pixel is made of 4 successive u8: RGBA).
///
/// # Params
/// * `smoc`: the Spatial MOC to be print;
/// * `size`: the `(X, Y)` number of pixels in the image;
/// * `proj`: a projection, if different from Mollweide;
/// * `proj_center`: the `(lon, lat)` coordinates of the center of the projection, in radians,
///                      if different from `(0, 0)`;
/// * `proj_bounds`: the `(X, Y)` bounds of the projection, if different from the default values
///                  which depends on the projection. For unbounded projections, de default value
///                  is `(-PI..PI, -PI..PI)`.
/// * `pos_convert`: to handle a different coordinate system between the MOC and the image.
pub fn to_img<T: Idx, P: CanonicalProjection>(
  smoc: &RangeMOC<T, Hpx<T>>,
  img_size: (u16, u16),
  proj: P,
  proj_center: Option<(f64, f64)>,
  proj_bounds: Option<(RangeInclusive<f64>, RangeInclusive<f64>)>,
  pos_convert: Option<PosConversion>,
) -> Vec<u8> {
  let (size_x, size_y) = img_size;
  let mut v: Vec<u8> = Vec::with_capacity((size_x as usize * size_y as usize) << 2);

  let (proj_range_x, proj_range_y) = proj_bounds.unwrap_or((
    proj
      .bounds()
      .x_bounds()
      .as_ref()
      .cloned()
      .unwrap_or_else(|| -PI..=PI),
    proj
      .bounds()
      .y_bounds()
      .as_ref()
      .cloned()
      .unwrap_or_else(|| -PI..=PI),
  ));

  let img2proj =
    ReversedEastPngImgXY2ProjXY::from((size_x, size_y), (&proj_range_x, &proj_range_y));
  let mut img2cel = Img2Celestial::new(img2proj, CenteredProjection::new(proj));
  if let Some((lon, lat)) = proj_center {
    img2cel.set_proj_center_from_lonlat(&LonLat::new(lon, lat));
  }

  let hpx = nested::get(Hpx::<u64>::MAX_DEPTH);

  let pos_convert = pos_convert.unwrap_or(PosConversion::SameMapAndImg);
  let mappos2imgpos = pos_convert.convert_map_pos_to_img_pos();
  let imgpos2mappos = pos_convert.convert_img_pos_to_map_pos();

  // First check for each pixel if its center is in the MOC
  for y in 0..size_y {
    for x in 0..size_x {
      if let Some(lonlat) = img2cel.img2lonlat(&ImgXY::new(x as f64, y as f64)) {
        let (lon, lat) = imgpos2mappos(lonlat.lon(), lonlat.lat());
        let idx = hpx.hash(lon, lat);
        if smoc.contains_val(&T::from_u64_idx(idx)) {
          // in the moc
          v.push(255);
          v.push(0);
          v.push(0);
          v.push(255);
        } else {
          // out of the moc
          v.push(0);
          v.push(0);
          v.push(0);
          v.push(255);
        }
      } else {
        // Not in the proj area
        v.push(255);
        v.push(255);
        v.push(255);
        v.push(0);
      }
    }
  }
  // But, in case of sparse MOC, also light up the pixel containing a cell center
  for Cell { depth, idx } in smoc.into_range_moc_iter().cells() {
    let (lon_rad, lat_rad) = nested::center(depth, idx.to_u64());
    let (lon_rad, lat_rad) = mappos2imgpos(lon_rad, lat_rad);
    if let Some(xy) = img2cel.lonlat2img(&LonLat::new(lon_rad, lat_rad)) {
      let ix = xy.x() as u16;
      let iy = xy.y() as u16;
      if ix < img_size.0 && iy < img_size.1 {
        let from = (xy.y() as usize * size_x as usize + ix as usize) << 2; // <<2 <=> *4
        if v[from] == 0 {
          v[from] = 255;
          v[from + 1] = 0;
          v[from + 2] = 0;
          v[from + 3] = 128;
        }
      }
    }
  }
  v
}

/// # Params
/// * `smoc`: the Spatial MOC to be print;
/// * `size`: the `(X, Y)` number of pixels in the image;
/// * `proj`: a projection, if different from Mollweide;
/// * `proj_center`: the `(lon, lat)` coordinates of the center of the projection, in radians,
///                      if different from `(0, 0)`;
/// * `proj_bounds`: the `(X, Y)` bounds of the projection, if different from the default values
///                  which depends on the projection. For unbounded projections, de default value
///                  is `(-PI..PI, -PI..PI)`.
/// * `pos_convert`: to handle a different coordinate system between the MOC and the image.
/// * `writer`: the writer in which the image is going to be written
pub fn to_png<T: Idx, P: CanonicalProjection, W: Write>(
  smoc: &RangeMOC<T, Hpx<T>>,
  img_size: (u16, u16),
  proj: Option<P>,
  proj_center: Option<(f64, f64)>,
  proj_bounds: Option<(RangeInclusive<f64>, RangeInclusive<f64>)>,
  pos_convert: Option<PosConversion>,
  writer: W,
) -> Result<(), io::Error> {
  let (xsize, ysize) = img_size;
  let data = if let Some(proj) = proj {
    to_img(smoc, img_size, proj, proj_center, proj_bounds, pos_convert)
  } else {
    to_img_default(smoc, img_size, proj_center, proj_bounds, pos_convert)
  };
  let mut encoder = png::Encoder::new(writer, xsize as u32, ysize as u32); // Width is 2 pixels and height is 1.
  encoder.set_color(png::ColorType::Rgba);
  encoder.set_depth(png::BitDepth::Eight);
  let mut writer = encoder.write_header()?;
  writer.write_image_data(&data).expect("Wrong encoding");
  Ok(())
}

/// Automatically determines the center of the projection and if the projection to be used
/// is an allsky (Mollweide) ou bound limited (Sinus) projection.
/// In the first case, the image size along the x-axis is `2 * size_y`, and `size_y`
/// # Params
/// * `smoc`: the Spatial MOC to be print;
/// * `img_size_y`: the size of the image along the y-axis.
pub fn to_png_auto<T: Idx, W: Write>(
  smoc: &RangeMOC<T, Hpx<T>>,
  img_size_y: u16,
  pos_convert: Option<PosConversion>,
  writer: W,
) -> Result<(u16, u16), io::Error> {
  let (data, img_size) = to_img_auto(smoc, img_size_y, pos_convert);
  let mut encoder = png::Encoder::new(writer, img_size.0 as u32, img_size.1 as u32);
  encoder.set_color(png::ColorType::Rgba);
  encoder.set_depth(png::BitDepth::Eight);
  let mut writer = encoder.write_header()?;
  writer.write_image_data(&data).expect("Wrong encoding");
  Ok(img_size)
}

/// # Params
/// * `smoc`: the Spatial MOC to be print;
/// * `size`: the `(X, Y)` number of pixels in the image;
/// * `proj`: a projection, if different from Mollweide;
/// * `proj_center`: the `(lon, lat)` coordinates of the center of the projection, in radians,
///                      if different from `(0, 0)`;
/// * `proj_bounds`: the `(X, Y)` bounds of the projection, if different from the default values
///                  which depends on the projection. For unbounded projections, de default value
///                  is `(-PI..PI, -PI..PI)`.
/// * `pos_convert`: to handle a different coordinate system between the MOC and the image.
/// * `path`: the path of th PNG file to be written.
/// * `view`: set to true to visualize the saved image.
#[cfg(not(target_arch = "wasm32"))]
pub fn to_png_file<T: Idx, P: CanonicalProjection>(
  smoc: &RangeMOC<T, Hpx<T>>,
  img_size: (u16, u16),
  proj: Option<P>,
  proj_center: Option<(f64, f64)>,
  proj_bounds: Option<(RangeInclusive<f64>, RangeInclusive<f64>)>,
  pos_convert: Option<PosConversion>,
  path: &Path,
  view: bool,
) -> Result<(), io::Error> {
  // Brackets are important to be sure the file is closed before trying to open it.
  {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    to_png(
      smoc,
      img_size,
      proj,
      proj_center,
      proj_bounds,
      pos_convert,
      &mut writer,
    )?;
  }
  if view {
    show_with_default_app(path.to_string_lossy().as_ref())?;
  }
  Ok(())
}

/// Automatically determines the center of the projection and if the projection to be used
/// is an allsky (Mollweide) ou bound limited (Sinus) projection.
/// In the first case, the image size along the x-axis is `2 * size_y`, and `size_y`
/// # Params
/// * `smoc`: the Spatial MOC to be print;
/// * `img_size_y`: the size of the image along the y-axis.
/// * `path`: the path of th PNG file to be written.
/// * `view`: set to true to visualize the saved image.
#[cfg(not(target_arch = "wasm32"))]
pub fn to_png_file_auto<T: Idx>(
  smoc: &RangeMOC<T, Hpx<T>>,
  img_size_y: u16,
  pos_convert: Option<PosConversion>,
  path: &Path,
  view: bool,
) -> Result<(u16, u16), io::Error> {
  // Brackets are important to be sure the file is closed before trying to open it.
  let img_size = {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    to_png_auto(smoc, img_size_y, pos_convert, &mut writer)?
  };
  if view {
    show_with_default_app(path.to_string_lossy().as_ref())?;
  }
  Ok(img_size)
}

// Adapted from https://github.com/igiagkiozis/plotly/blob/master/plotly/src/plot.rs
#[cfg(target_os = "linux")]
fn show_with_default_app(path: &str) -> Result<(), io::Error> {
  use std::process::Command;
  Command::new("xdg-open").args([path]).output()?;
  // .map_err(|e| e.into())?;
  Ok(())
}

#[cfg(target_os = "macos")]
fn show_with_default_app(path: &str) -> Result<(), io::Error> {
  use std::process::Command;
  Command::new("open").args(&[path]).output()?;
  Ok(())
}

#[cfg(target_os = "windows")]
fn show_with_default_app(path: &str) -> Result<(), io::Error> {
  use std::process::Command;
  Command::new("cmd")
    .arg("/C")
    .arg(format!(r#"start {}"#, path))
    .output()?;
  Ok(())
}

#[cfg(test)]
mod tests {

  use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
  };

  use mapproj::{
    conic::{cod::Cod, coe::Coe, coo::Coo, cop::Cop},
    cylindrical::{car::Car, cea::Cea, cyp::Cyp, mer::Mer},
    hybrid::hpx::Hpx,
    pseudocyl::{ait::Ait, mol::Mol, par::Par, sfl::Sfl},
    zenithal::{
      air::Air, arc::Arc, azp::Azp, feye::Feye, ncp::Ncp, sin::Sin, stg::Stg, szp::Szp, tan::Tan,
      zea::Zea, zpn::Zpn,
    },
  };

  use crate::deser::img::to_img_auto;
  use crate::{
    deser::{
      fits::{from_fits_ivoa, MocIdxType, MocQtyType, MocType},
      img::to_png_file,
    },
    moc::{CellMOCIntoIterator, CellMOCIterator, RangeMOCIterator},
  };

  #[test]
  fn test_img() {
    let path_buf1 = PathBuf::from("resources/V_147_sdss12.moc.fits");
    let path_buf2 = PathBuf::from("../resources/V_147_sdss12.moc.fits");
    // let path_buf1 = PathBuf::from("resources/V_147_sdss12.moc.u64.fits");
    // let path_buf2 = PathBuf::from("../resources/V_147_sdss12.moc.u64.fits");
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    match from_fits_ivoa(reader) {
      Ok(MocIdxType::U32(MocQtyType::Hpx(MocType::Cells(moc)))) => {
        // Ok(MocIdxType::U64(MocQtyType::Hpx(MocType::Ranges(moc)))) => {
        // let moc = moc.into_range_moc();
        let moc = moc.into_cell_moc_iter().ranges().into_range_moc();
        let view = false;
        let img_size = (1600, 800);
        to_png_file(
          &moc,
          img_size,
          Some(Mol::new()),
          None,
          None,
          None,
          &Path::new("sdss_mol.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Ait::new()),
          None,
          None,
          None,
          &Path::new("sdss_ait.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Par::new()),
          None,
          None,
          None,
          &Path::new("sdss_par.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Sfl::new()),
          None,
          None,
          None,
          &Path::new("sdss_sfl.png"),
          view,
        )
        .unwrap();

        to_png_file(
          &moc,
          img_size,
          Some(Car::new()),
          None,
          None,
          None,
          &Path::new("sdss_car.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Cea::new()),
          None,
          None,
          None,
          &Path::new("sdss_cea.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Cyp::new()),
          None,
          None,
          None,
          &Path::new("sdss_cyp.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Mer::new()),
          None,
          None,
          None,
          &Path::new("sdss_mer.png"),
          view,
        )
        .unwrap();
        let img_size = (800, 800);
        to_png_file(
          &moc,
          img_size,
          Some(Cod::new()),
          None,
          None,
          None,
          &Path::new("sdss_cod.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Coe::new()),
          None,
          None,
          None,
          &Path::new("sdss_coe.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Coo::new()),
          None,
          None,
          None,
          &Path::new("sdss_coo.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Cop::new()),
          None,
          None,
          None,
          &Path::new("sdss_cop.png"),
          view,
        )
        .unwrap();
        let img_size = (1600, 800);
        to_png_file(
          &moc,
          img_size,
          Some(Hpx::new()),
          None,
          None,
          None,
          &Path::new("sdss_hpx.png"),
          view,
        )
        .unwrap();
        let img_size = (800, 800);
        to_png_file(
          &moc,
          img_size,
          Some(Air::new()),
          None,
          None,
          None,
          &Path::new("sdss_air.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Arc::new()),
          None,
          None,
          None,
          &Path::new("sdss_arc.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Azp::new()),
          None,
          Some((-3.0..=3.0, -3.0..=3.0)),
          None,
          &Path::new("sdss_azp.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Feye::new()),
          None,
          None,
          None,
          &Path::new("sdss_feye_front.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Feye::new()),
          Some((180.0, 0.0)),
          None,
          None,
          &Path::new("sdss_feye_back.png"),
          view,
        )
        .unwrap();

        to_png_file(
          &moc,
          img_size,
          Some(Ncp::new()),
          None,
          None,
          None,
          &Path::new("sdss_ncp_front.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Ncp::new()),
          Some((180.0, 0.0)),
          None,
          None,
          &Path::new("sdss_ncp_back.png"),
          view,
        )
        .unwrap();

        to_png_file(
          &moc,
          img_size,
          Some(Sin::new()),
          None,
          None,
          None,
          &Path::new("sdss_sin_front.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Sin::new()),
          Some((180.0, 0.0)),
          None,
          None,
          &Path::new("sdss_sin_back.png"),
          view,
        )
        .unwrap();

        to_png_file(
          &moc,
          img_size,
          Some(Stg::new()),
          None,
          None,
          None,
          &Path::new("sdss_stg.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Szp::new()),
          None,
          Some((-10.0..=10.0, -10.0..=10.0)),
          None,
          &Path::new("sdss_szp.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Tan::new()),
          None,
          None,
          None,
          &Path::new("sdss_tan.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Zea::new()),
          None,
          None,
          None,
          &Path::new("sdss_zea.png"),
          view,
        )
        .unwrap();
        to_png_file(
          &moc,
          img_size,
          Some(Zpn::from_params(vec![0.0, 1.0, 0.0, -50.0]).unwrap()),
          None,
          None,
          None,
          &Path::new("sdss_zpn.png"),
          view,
        )
        .unwrap();
        assert!(true);
      }
      Err(e) => println!("{}", e),
      _ => assert!(false),
    }
  }

  #[test]
  fn test_img_auto_allsky() {
    // let path_buf1 = PathBuf::from("resources/V_147_sdss12.moc.fits");
    // let path_buf2 = PathBuf::from("../resources/V_147_sdss12.moc.fits");
    let path_buf1 = PathBuf::from("resources/MOC2.0/SMOC_test.fits");
    let path_buf2 = PathBuf::from("../resources/MOC2.0/SMOC_test.fits");
    let file = File::open(&path_buf1)
      .or_else(|_| File::open(&path_buf2))
      .unwrap();
    let reader = BufReader::new(file);
    match from_fits_ivoa(reader) {
      Ok(MocIdxType::U64(MocQtyType::Hpx(MocType::Cells(moc)))) => {
        let moc = moc.into_cell_moc_iter().ranges().into_range_moc();
        to_img_auto(&moc, 800, None);
        assert!(true);
      }
      Err(e) => println!("{}", e),
      _ => assert!(false),
    }
  }
}
