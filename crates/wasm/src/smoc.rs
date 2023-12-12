
use std::{
  sync::{Once, RwLock},
  collections::HashMap,
  str::from_utf8,
};

use wasm_bindgen::{
  JsValue,
  prelude::*
};
use js_sys::Array;


use moclib::{
  qty::Hpx,
  storage::u64idx::U64MocStore
};

use crate::{MocQType, IsMOC, IsOneDimMOC, from_url, from_local_multiordermap, from_local_skymap};

/// Function used only once to init the store.
static MOC_STORE_INIT: Once = Once::new();
/// The MOC store (a simple hashmap), protected from concurrent access by a RwLock.
static mut MOC_STORE: Option<RwLock<HashMap<String, MOC>>> = None;

/// Get (or create and get) the read/write protected MOC store
/// All read/write  operations on the store have to call this method.
pub(crate) fn get_store() -> &'static RwLock<HashMap<String, MOC>> {
  unsafe {
    // Inspired from the Option get_or_insert_with method, modified to ensure thread safety with
    // https://doc.rust-lang.org/std/sync/struct.Once.html
    // This implements a double-checked lock.
    if let None = MOC_STORE {
      MOC_STORE_INIT.call_once(|| {
        MOC_STORE = Some(RwLock::new(HashMap::new()));
      });
    }
    match &MOC_STORE {
      Some(v) => v,
      None => unreachable!(),
    }
  }
}



#[wasm_bindgen]
pub struct MOC {
  store_index: usize
}

impl IsMOC for MOC {

  fn from_store_index(store_index: usize) -> Self {
    Self { store_index }
  }

  fn storage_index(&self) -> usize {
    self.store_index
  }

  fn get_type(&self) -> MocQType {
    MocQType::Space
  }

  fn add_to_store(name: &str, moc: Self) -> Result<(), JsValue> {
    let mut store = get_store().write().map_err(|_| JsValue::from_str("Write lock poisoned"))?;
    (*store).insert(String::from(name), moc);
    Ok(())
  }


  fn from_ascii(data: &str) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .load_smoc_from_ascii(data)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn from_json(data: &str) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .load_smoc_from_json(data)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn from_fits(data: &[u8]) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .load_smoc_from_fits_buff(data)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }


}

impl IsOneDimMOC for MOC {

  fn depth(&self) -> Result<u8, JsValue> {
    U64MocStore::get_global_store()
      .get_smoc_depth(self.storage_index())
      .map_err(|e| e.into())
  }

}

#[wasm_bindgen]
impl MOC {

  #[wasm_bindgen(js_name = "listMocsLoadedFromLocalFile", catch)]
  /// Returns the MOCs identifiers (names) currently in the store (MOCs loaded from local files)
  pub fn list_mocs_loaded_from_local_file() -> Result<Array, JsValue> {
    Ok(
      get_store().read().map_err(|_| JsValue::from_str("Read lock poisoned"))?
        .iter()
        .map(|(key, _)| JsValue::from_str(key))
        .collect::<Array>()
    )

  }

  #[wasm_bindgen(js_name = "getMocLoadedFromLocalFile", catch)]
  /// Get (and remove from the store) the MOC of given name loaded from a local file.
  pub fn get_moc_loaded_from_local_file(name: &str) -> Result<MOC, JsValue> {
    let mut store = get_store().write().map_err(|_| JsValue::from_str("Write lock poisoned"))?;
    match (*store).remove(name) {
      Some(moc) => Ok(moc),
      None => Err(JsValue::from_str(&format!("No MOC named '{}' found in store", name))),
    }
  }
  
  #[wasm_bindgen(js_name = "newEmpty", catch)]
  /// Creates a new empty MOC of given depth.
  pub fn new_empty(depth: u8) -> Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .new_empty_smoc(depth)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  // IsMOC methods (put here because wasm_bindgen does not export trait methods)

  #[wasm_bindgen(js_name = "getType")]
  /// Returns the type of the MOC.
  pub fn get_type(&self) -> MocQType {
    IsMOC::get_type(self)
  }

  #[wasm_bindgen(js_name = "fromLocalFile", catch)]
  /// Trigger a file dialog event and load the selected MOCs from selected file to a local storage.
  /// You then have to use:
  /// * `list_mocs_loaded_from_local_file`
  /// * `get_moc_loaded_from_local_file`
  pub fn from_local_file() -> Result<(), JsValue> {
    <Self as IsMOC>::from_local_file()
  }

  #[wasm_bindgen(js_name = "fromAscii", catch)]
  /// Create a MOC from its ASCII serialization
  ///
  /// # Arguments
  /// * `data`: ASCII serialization
  pub fn from_ascii(data: &str) -> Result<MOC, JsValue> {
    IsMOC::from_ascii(data)
  }
  
  #[wasm_bindgen(js_name = "fromAsciiUrl", catch)]
  /// WARNING: if this is not working, check e.g. with `wget -v -S ${url}` the the content type is
  /// `Content-Type: text/plain`.
  pub async fn from_ascii_url(url: String) -> Result<MOC, JsValue> {
    const ERR: &str = "File content is not valid UTF-8.";
    from_url(
      url, "text/plain",
      Box::new(|data| Self::from_ascii(from_utf8(data).unwrap_or(ERR)) )
    ).await
  }
  
  
  #[wasm_bindgen(js_name = "fromJson", catch)]
  /// Create a MOC from its JSON serialization
  ///
  /// # Arguments
  /// * `data`: JSON serialization
  pub fn from_json(data: &str) -> Result<MOC, JsValue> {
    IsMOC::from_json(data)
  }

  #[wasm_bindgen(js_name = "fromJsonUrl", catch)]
  /// WARNING: if this i not working, check e.g. with `wget -v -S ${url}` the the content type is
  /// `Content-Type: application/json`.
  pub async fn from_json_url(url: String) -> Result<MOC, JsValue> {
    const ERR: &str = "File content is not valid UTF-8.";
    from_url(
      url, "application/json",
      Box::new(|data| Self::from_json(from_utf8(data).unwrap_or(ERR)) )
    ).await
  }
  
  
  #[wasm_bindgen(js_name = "fromFits", catch)]
  /// Create a MOC from its FITS serialization
  ///
  /// # Arguments
  /// * `data`: FITS serialization
  pub fn from_fits(data: &[u8]) -> Result<MOC, JsValue> {
    IsMOC::from_fits(data)
  }

  #[wasm_bindgen(js_name = "fromFitsUrl", catch)]
  /// # Arguments
  /// * `url`: URL of the FITS file
  /// * `accept_mime_types`: use `None` (Rust) or `null` (Javascript) to use the default `application/fits`
  /// # WARNING
  ///   If this is not working, check e.g. with `wget -v -S ${url}` the the content type is
  ///   `Content-Type: application/fits`.
  ///   Else use the `accept_mime_types` option to set the `Accept` HTTP request parameter, with e.g:
  ///   * `application/fits` (default value)
  ///   * `application/fits, application/octet-stream`
  #[wasm_bindgen(js_name = "fromFitsUrl")]
  pub async fn from_fits_url(url: String, accept_mime_types: Option<String>) -> Result<MOC, JsValue> {
    match accept_mime_types {
      None =>             from_url(url, "application/fits", Box::new(Self::from_fits)).await,
      Some(mime_types) => from_url(url, &mime_types, Box::new(Self::from_fits)).await,
    }
  }

  #[wasm_bindgen(js_name = "toAscii", catch)]
  /// Returns the ASCII serialization of the MOC.
  ///
  /// # Arguments
  /// * `fold`: fold option to limit the width of the string
  pub fn to_ascii(&self, fold: Option<usize>) -> Result<JsValue, JsValue> {
    IsMOC::to_ascii(self, fold)
  }

  #[wasm_bindgen(js_name = "toJson", catch)]
  /// Returns the JSON serialization of the MOC.
  ///
  /// # Arguments
  /// * `fold`: fold option to limit the width of the string
  pub fn to_json(&self, fold: Option<usize>) -> Result<JsValue, JsValue> {
    IsMOC::to_json(self, fold)
  }

  #[wasm_bindgen(js_name = "toFits", catch)]
  /// Returns in memory the FITS serialization of the MOC.
  ///
  /// # Arguments
  /// * `force_v1_compatibility`: for S-MOCs, force compatibility with Version 1 of the MOC standard.
  pub fn to_fits(&self, force_v1_compatibility: Option<bool>) -> Result<Box<[u8]>, JsValue> {
    IsMOC::to_fits(self, force_v1_compatibility)
  }

  #[wasm_bindgen(js_name = "toAsciiFile", catch)]
  /// Download the ASCII serialization of the MOC.
  ///
  /// # Arguments
  /// * `fold`: fold option to limit the width of the string
  pub fn to_ascii_file(&self, fold: Option<usize>) -> Result<(), JsValue> {
    IsMOC::to_ascii_file(self, fold)
  }

  #[wasm_bindgen(js_name = "toJsonFile", catch)]
  /// Download the JSON serialization of the MOC.
  ///
  /// # Arguments
  /// * `fold`: fold option to limit the width of the string
  pub fn to_json_file(&self, fold: Option<usize>) -> Result<(), JsValue> {
    IsMOC::to_json_file(self, fold)
  }

  #[wasm_bindgen(js_name = "toFitsFile", catch)]
  /// Download the FITS serialization of the MOC.
  ///
  /// # Arguments
  /// * `force_v1_compatibility`: for S-MOCs, force compatibility with Version 1 of the MOC standard.
  pub fn to_fits_file(&self, force_v1_compatibility: Option<bool>) -> Result<(), JsValue> {
    IsMOC::to_fits_file(self, force_v1_compatibility)
  }

  // IsOneDimMOC methods (put here because wasm_bindgen does not export trait methods)

  #[wasm_bindgen(js_name = "getDepth", catch)]
  /// Returns the MOC depth.
  pub fn depth(&self) -> Result<u8, JsValue> {
    IsOneDimMOC::depth(self)
  }
  #[wasm_bindgen(js_name = "coveragePercentage", catch)]
  pub fn coverage_percentage(&self) -> Result<f64, JsValue>  {
    IsOneDimMOC::coverage_percentage(self)
  }
  #[wasm_bindgen(js_name = "nRanges", catch)]
  pub fn n_ranges(&self) -> Result<u32, JsValue> {
    IsOneDimMOC::n_ranges(self)
  }
  #[wasm_bindgen(catch)]
  pub fn not(&self) -> Result<MOC, JsValue> {
    IsOneDimMOC::not(self)
  }
  #[wasm_bindgen(catch)]
  pub fn complement(&self) -> Result<MOC, JsValue> {
    IsOneDimMOC::complement(self)
  }
  #[wasm_bindgen(catch)]
  pub fn degrade(&self, new_depth: u8) -> Result<MOC, JsValue> {
    IsOneDimMOC::degrade(self, new_depth)
  }
  #[wasm_bindgen(catch)]
  pub fn or(&self, rhs: &MOC) -> Result<MOC, JsValue> {
    IsOneDimMOC::or(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn union(&self, rhs: &MOC) -> Result<MOC, JsValue> {
    IsOneDimMOC::union(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn and(&self, rhs: &MOC) -> Result<MOC, JsValue> {
    IsOneDimMOC::and(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn intersection(&self, rhs: &MOC) -> Result<MOC, JsValue> {
    IsOneDimMOC::intersection(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn xor(&self, rhs: &MOC) -> Result<MOC, JsValue> {
    IsOneDimMOC::xor(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn symmetric_difference(&self, rhs: &MOC) -> Result<MOC, JsValue> {
    IsOneDimMOC::symmetric_difference(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn minus(&self, rhs: &MOC) -> Result<MOC, JsValue> {
    IsOneDimMOC::minus(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn difference(&self, rhs: &MOC) -> Result<MOC, JsValue> {
    IsOneDimMOC::difference(self, rhs)
  }


  // Specific methods

  // - from local file

  #[wasm_bindgen(js_name = "fromLocalMultiOrderMap", catch)]
  /// Trigger a file dialog event and load the selected multi-order map file to a local storage.
  /// You then have to use:
  /// * `list_mocs_loaded_from_local_file`
  /// * `get_moc_loaded_from_local_file`
  ///
  /// # Warning
  /// Because of security restriction, the call to this method
  /// **"needs to be triggered within a code block that was the handler of a user-initiated event"**  
  pub fn from_local_multiordermap_file(
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
  ) -> Result<(), JsValue> {
    from_local_multiordermap(from_threshold, to_threshold, asc, not_strict, split, revese_recursive_descent)
  }

  #[wasm_bindgen(js_name = "fromLocalSkymap", catch)]
  /// Trigger a file dialog event and load the selected skymap file to a local storage.
  /// You then have to use:
  /// * `list_mocs_loaded_from_local_file`
  /// * `get_moc_loaded_from_local_file`
  ///
  /// # Warning
  /// Because of security restriction, the call to this method
  /// **"needs to be triggered within a code block that was the handler of a user-initiated event"**  
  pub fn from_local_skymap(
    skip_values_le: f64,
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
  ) -> Result<(), JsValue> {
    from_local_skymap(skip_values_le, from_threshold, to_threshold, asc, not_strict, split, revese_recursive_descent)
  }


  // - specific creation methods

  #[wasm_bindgen(js_name = "fromCone", catch)]
  /// Create a MOC from the given cone.
  ///
  /// # Arguments
  /// * `depth` - the MOC depth
  /// * `lon_deg` - the longitude of the center of the cone, in degrees
  /// * `lat_deg` - the latitude of the center of the cone, in degrees
  /// * `radius_deg` - the radius of the cone, in degrees
  /// * `delta_depth` - the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small, default = 2).
  ///
  pub fn from_cone(depth: u8, lon_deg: f64, lat_deg: f64, radius_deg: f64, delta_depth: Option<u8>) ->  Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .from_cone(lon_deg, lat_deg, radius_deg, depth, delta_depth.unwrap_or(2))
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromSTCS", catch)]
  /// Create a MOC from the given STC-S string.
  ///
  /// # Arguments
  /// * `depth` - the MOC depth
  /// * `ascii_stcs` - the STC-S string (see the MOC Lib Rust README file for WARNINGs about discrepancies from the standard).
  /// * `delta_depth` - the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small, default = 2).
  ///
  pub fn from_stcs(depth: u8, ascii_stcs: &str, delta_depth: Option<u8>) ->  Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .from_stcs(depth, delta_depth.unwrap_or(2), ascii_stcs)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromRing", catch)]
  /// Create a MOC from the given ring.
  ///
  /// # Arguments
  /// * `depth` - the MOC depth
  /// * `lon_deg` - the longitude of the center of the ring, in degrees
  /// * `lat_deg` - the latitude of the center of the ring, in degrees
  /// * `internal_radius_deg` - the internal radius of the ring, in degrees
  /// * `external_radius_deg` - the external radius of the ring, in degrees
  /// * `delta_depth` - the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  ///
  pub fn from_ring(
    depth: u8,
    lon_deg: f64,
    lat_deg: f64,
    internal_radius_deg: f64,
    external_radius_deg: f64,
    delta_depth: Option<u8>
  ) ->  Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .from_ring(lon_deg, lat_deg, internal_radius_deg, external_radius_deg, depth, delta_depth.unwrap_or(2))
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromEllipse", catch)]
  /// Create a MOC from the given elliptical cone.
  ///
  /// # Arguments
  /// * `depth` - the MOC depth
  /// * `lon_deg` - the longitude of the center of the elliptical cone, in degrees
  /// * `lat_deg` - the latitude of the center of the elliptical cone, in degrees
  /// * `a_deg` - the semi-major axis of the elliptical cone, in degrees
  /// * `b_deg` - the semi-minor axis of the elliptical cone, in degrees
  /// * `pa_deg` - the position angle (i.e. the angle between the north and the semi-major axis, east-of-north), in degrees
  /// * `delta_depth` - the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  ///
  pub fn from_elliptical_cone(
    depth: u8,
    lon_deg: f64, lat_deg: f64,
    a_deg: f64, b_deg: f64, pa_deg: f64,
    delta_depth: Option<u8>
  ) ->  Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .from_elliptical_cone(lon_deg, lat_deg, a_deg, b_deg, pa_deg, depth, delta_depth.unwrap_or(2))
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromZone", catch)]
  /// Create a MOC from the given zone.
  ///
  /// # Arguments
  /// * `depth` - the MOC depth
  /// * `lon_deg_min` - the longitude of the bottom left corner, in degrees
  /// * `lat_deg_min` - the latitude of the bottom left corner, in degrees
  /// * `lon_deg_max` - the longitude of the upper left corner, in degrees
  /// * `lat_deg_max` - the latitude of the upper left corner, in degrees
  ///
  /// # Remark
  /// - If `lon_min > lon_max` then we consider that the zone crosses the primary meridian.
  /// - The north pole is included only if `lon_min == 0 && lat_max == pi/2`
  pub fn from_zone(
    depth: u8,
    lon_deg_min: f64, lat_deg_min: f64,
    lon_deg_max: f64, lat_deg_max: f64
  ) ->  Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .from_zone(lon_deg_min, lat_deg_min, lon_deg_max, lat_deg_max, depth)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromBox", catch)]
  /// Create a MOC from the given box.
  ///
  /// # Arguments
  /// * `depth` - the MOC depth
  /// * `lon_deg` - the longitude of the center of the box, in degrees
  /// * `lat_deg` - the latitude of the center of the box, in degrees
  /// * `a_deg` - the semi-major axis of the box (half the box width), in degrees
  /// * `b_deg` - the semi-minor axis of the box (half the box height), in degrees
  /// * `pa_deg` - the position angle (i.e. the angle between the north and the semi-major axis, east-of-north), in degrees
  ///
  pub fn from_box(
    depth: u8,
    lon_deg: f64, lat_deg: f64,
    a_deg: f64, b_deg: f64, pa_deg: f64
  ) ->  Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .from_box(lon_deg, lat_deg, a_deg, b_deg, pa_deg, depth)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromPolygon", catch)]
  /// Create a new MOC from the given polygon vertices.
  ///
  /// # Arguments
  /// * `depth` - MOC maximum depth in `[0, 29]`
  /// * `vertices` - vertices coordinates, in degrees `[lon_v1, lat_v1, lon_v2, lat_v2, ..., lon_vn, lat_vn]`
  /// * `complement` - reverse the default inside/outside of the polygon
  pub fn from_polygon(
    depth: u8,
    vertices_deg: Box<[f64]>,
    complement: bool
  ) ->  Result<MOC, JsValue> {
    let vertices_iter = vertices_deg.iter().step_by(2)
      .zip(vertices_deg.iter().skip(1).step_by(2))
      .map(|(lon_deg, lat_deg)| (*lon_deg, *lat_deg));
    U64MocStore::get_global_store()
      .from_polygon(vertices_iter, complement, depth)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromCoo", catch)]
  /// Create a new MOC from the given list of coordinates (assumed to be equatorial)
  ///
  /// # Arguments
  /// * `depth` - MOC maximum depth in `[0, 29]`
  /// * `coos_deg` - list of coordinates in degrees `[lon_1, lat_1, lon_2, lat_2, ..., lon_n, lat_n]`
  pub fn from_coo(
    depth: u8,
    coos_deg: Box<[f64]>,
  ) ->  Result<MOC, JsValue> {
    let coo_iter = coos_deg.iter().step_by(2)
      .zip(coos_deg.iter().skip(1).step_by(2))
      .map(|(lon_deg, lat_deg)| (*lon_deg, *lat_deg));
    U64MocStore::get_global_store()
      .from_coo(depth, coo_iter)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromSmallCones", catch)]
  /// Create a new MOC from the given list of cone centers and radii
  /// Adapted for a large number of small cones (a few cells each).
  ///
  /// # Arguments
  /// * `depth` - MOC maximum depth in `[0, 29]`
  /// * `delta_depth` - the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  /// * `coos_and_radius_deg` - list of coordinates and radii in degrees ``[lon_1, lat_1, rad_1, lon_2, lat_2, rad_2, ..., lon_n, lat_n, rad_n]``
  pub fn from_small_cones(
    depth: u8, delta_depth: u8,
    coos_and_radius_deg: Box<[f64]>,
  ) ->  Result<MOC, JsValue> {
    let coos_rad_iter = coos_and_radius_deg.iter().step_by(3).zip(
      coos_and_radius_deg.iter().skip(1).step_by(3)).zip(
      coos_and_radius_deg.iter().skip(2).step_by(3)
    ).map(|((lon_deg, lat_deg), radius_deg)| ((*lon_deg, *lat_deg), *radius_deg));
    U64MocStore::get_global_store()
      .from_small_cones(depth, delta_depth, coos_rad_iter)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }


  #[wasm_bindgen(js_name = "fromLargeCones", catch)]
  /// Create a new MOC from the given list of cone centers and radii
  /// Adapted for a reasonable number of possibly large cones.
  ///
  /// # Arguments
  /// * `depth` - MOC maximum depth in `[0, 29]`
  /// * `delta_depth` - the difference between the MOC depth and the depth at which the computations
  ///   are made (should remain quite small).
  /// * `coos_and_radius_deg` - list of coordinates and radii in degrees
  ///   `[lon_1, lat_1, rad_1, lon_2, lat_2, rad_2, ..., lon_n, lat_n, rad_n]`
  pub fn from_large_cones(
    depth: u8, delta_depth: u8,
    coos_and_radius_deg: Box<[f64]>,
  ) ->  Result<MOC, JsValue> {
    let coos_rad_iter = coos_and_radius_deg.iter().step_by(3).zip(
      coos_and_radius_deg.iter().skip(1).step_by(3)).zip(
      coos_and_radius_deg.iter().skip(2).step_by(3)
    ).map(|((lon_deg, lat_deg), radius_deg)| ((*lon_deg, *lat_deg), *radius_deg));
    U64MocStore::get_global_store()
      .from_large_cones(depth, delta_depth, coos_rad_iter)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromValuedCells", catch)]
  /// Create a new S-MOC from the given lists of UNIQ and Values.
  ///
  /// # Arguments
  /// * `depth` - S-MOC maximum depth in `[0, 29]`, Must be >= largest input cells depth.
  /// * `density` - Input values are densities, i.e. they are not proportional to the area of their associated cells.
  /// * `from_threshold` - Cumulative value at which we start putting cells in he MOC (often = 0).
  /// * `to_threshold` - Cumulative value at which we stop putting cells in the MOC.
  /// * `asc` - Compute cumulative value from ascending density values instead of descending (often = false).
  /// * `not_strict` - Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
  /// * `split` - Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
  /// * `revese_recursive_descent` - Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
  /// * `uniqs` - array of uniq HEALPix cells
  /// * `values` - array of values associated to the HEALPix cells
  pub fn from_valued_cells(
    depth: u8,
    density: bool,
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
    uniqs: Box<[f64]>,
    values: Box<[f64]>,
  ) -> Result<MOC, JsValue> {
    let depth = depth.max(
      uniqs.iter()
        .map(|uniq| Hpx::<u64>::from_uniq_hpx(*uniq as u64).0)
        .max()
        .unwrap_or(depth)
    );
    let uniq_vals = uniqs.into_iter().zip(values.into_iter())
      .map(|(u, v)| (*u as u64, *v));
    U64MocStore::get_global_store()
      .from_valued_cells(depth, density, from_threshold, to_threshold, asc, not_strict, split, revese_recursive_descent, uniq_vals)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromFitsMultiOrderMap", catch)]
  /// Create o S-MOC from a FITS multi-order map plus other parameters.
  ///
  /// # Arguments
  /// * `from_threshold`: Cumulative value at which we start putting cells in he MOC (often = 0).
  /// * `to_threshold`: Cumulative value at which we stop putting cells in the MOC.
  /// * `asc`: Compute cumulative value from ascending density values instead of descending (often = false).
  /// * `not_strict`: Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
  /// * `split`: Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
  /// * `revese_recursive_descent`: Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
  pub fn from_multiordermap_fits_file(
    data: &[u8],
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
  ) -> Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .from_multiordermap_fits_file_content(data, from_threshold, to_threshold, asc, not_strict, split, revese_recursive_descent)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromMultiOrderMapFitsUrl")]
  /// # Arguments
  /// * `name`: name used to store the loaded MOC
  /// * `url`: URL of the FITS file
  /// * `...`: same paramters as `fromFitsMultiOrderMap`
  /// * `accept_mime_types`: use `None` (Rust) or `null` (Javascript) to use the default `application/fits`
  /// # WARNING
  ///   If this is not working, check e.g. with `wget -v -S ${url}` the the content type is
  ///   `Content-Type: application/fits`.
  ///   Else use the `accept_mime_types` option to set the `Accept` HTTP request parameter, with e.g:
  ///   * `application/fits` (default value)
  ///   * `application/fits, application/octet-stream`
  pub async fn from_multiordermap_fits_url(
    url: String,
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
    accept_mime_types: Option<String>
  ) -> Result<MOC, JsValue>
  {
    let func = move |data: &[u8]| MOC::from_multiordermap_fits_file(data, from_threshold, to_threshold, asc, not_strict, split, revese_recursive_descent);
    match accept_mime_types {
      None =>             from_url(url, "application/fits", Box::new(func)).await,
      Some(mime_types) => from_url(url, &mime_types, Box::new(func)).await,
    }
  }

  #[wasm_bindgen(js_name = "fromFitsSkymap", catch)]
  /// Create o S-MOC from a FITS skymap plus other parameters.
  ///
  /// # Arguments
  /// * `skip_values_le`: skip cells associated to values lower or equal to the given value
  /// * `from_threshold`: Cumulative value at which we start putting cells in he MOC (often = 0).
  /// * `to_threshold`: Cumulative value at which we stop putting cells in the MOC.
  /// * `asc`: Compute cumulative value from ascending density values instead of descending (often = false).
  /// * `not_strict`: Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
  /// * `split`: Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
  /// * `revese_recursive_descent`: Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
  pub fn from_skymap_fits_file(
    data: &[u8],
    skip_values_le: f64,
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
  ) -> Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .from_skymap_fits_file_content(data, skip_values_le, from_threshold, to_threshold, asc, not_strict, split, revese_recursive_descent)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromSkymapFitsUrl", catch)]
  /// # Arguments
  /// * `url`: URL of the FITS file
  /// * `...`: same paramters as `fromFitsMultiOrderMap`
  /// * `accept_mime_types`: use `None` (Rust) or `null` (Javascript) to use the default `application/fits`
  ///
  /// # WARNING
  ///   If this is not working, check e.g. with `wget -v -S ${url}` the the content type is
  ///   `Content-Type: application/fits`.
  ///   Else use the `accept_mime_types` option to set the `Accept` HTTP request parameter, with e.g:
  ///   * `application/fits` (default value)
  ///   * `application/fits, application/octet-stream`
  pub async fn from_skymap_fits_url(
    url: String,
    skip_values_le: f64,
    from_threshold: f64,
    to_threshold: f64,
    asc: bool,
    not_strict: bool,
    split: bool,
    revese_recursive_descent: bool,
    accept_mime_types: Option<String>
  ) -> Result<MOC, JsValue>
  {
    let func = move |data: &[u8]| Self::from_skymap_fits_file(data, skip_values_le, from_threshold, to_threshold, asc, not_strict, split, revese_recursive_descent);
    match accept_mime_types {
      None =>             from_url(url, "application/fits", Box::new(func)).await,
      Some(mime_types) => from_url(url, &mime_types, Box::new(func)).await,
    }
  }

  // - filter

  #[wasm_bindgen(js_name = "filterCoos", catch)]
  /// Returns an array of boolean (u8 set to 1 or 0) telling if the pairs of coordinates
  /// in the input array are in (true=1) or out of (false=0) the S-MOC of given name.
  ///
  /// # Arguments
  /// * `coos_deg` - list of coordinates in degrees `[lon_1, lat_1, lon_2, lat_2, ..., lon_n, lat_n]`
  ///
  /// # Remarks
  /// The size of the returned boolean (u8) array his half the size of the input array
  /// (since the later contains pairs of coordinates).
  pub fn filter_pos(&self, coos_deg: Box<[f64]>) ->  Result<Box<[u8]>, JsValue> {
    let coo_iter = coos_deg.iter().step_by(2)
      .zip(coos_deg.iter().skip(1).step_by(2))
      .map(|(lon_deg, lat_deg)| (*lon_deg, *lat_deg));
    U64MocStore::get_global_store()
      .filter_pos(self.storage_index(), coo_iter, |b| b as u8)
      .map(|v| v.into_boxed_slice())
      .map_err(|e| e.into())
  }

  // - specific operations

  #[wasm_bindgen(catch)]
  /// Split the given disjoint S-MOC int joint S-MOCs.
  /// Split "direct", i.e. we consider 2 neighboring cells to be the same only if the share an edge.
  /// WARNING: may create a lot of new MOCs, exec `splitCount` first!!
  pub fn split(&self) -> Result<Box<[JsValue]>, JsValue> {
    U64MocStore::get_global_store()
      .split(self.storage_index())
      .map(|v| v.into_iter()
        .map(|i| Self::from_store_index(i).into())
        .collect::<Vec<JsValue>>()
        .into_boxed_slice()
      )
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "splitCount", catch)]
  /// Count the number of joint S-MOC splitting ("direct") the given disjoint S-MOC.
  pub fn split_count(&self) -> Result<u32, JsValue> {
    U64MocStore::get_global_store()
      .split_count(self.storage_index())
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "splitIndirect", catch)]
  /// Split the given disjoint S-MOC int joint S-MOCs.
  /// Split "indirect", i.e. we consider 2 neighboring cells to be the same if the share an edge
  /// or a vertex.
  /// WARNING: may create a lot of new MOCs, exec `splitIndirectCount` first!!
  pub fn split_indirect(&self) -> Result<Box<[JsValue]>, JsValue> {
    U64MocStore::get_global_store()
      .split_indirect(self.storage_index())
      .map(|v| v.into_iter()
        .map(|i| Self::from_store_index(i).into())
        .collect::<Vec<JsValue>>()
        .into_boxed_slice()
      )
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "splitIndirectCount", catch)]
  /// Count the number of joint S-MOC splitting ("indirect") the given disjoint S-MOC.
  pub fn split_indirect_count(&self) -> Result<u32, JsValue> {
    U64MocStore::get_global_store()
      .split_indirect_count(self.storage_index())
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(catch)]
  /// Returns a new MOC having an additional external border made of depth max cells.
  pub fn extend(&self) -> Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .extend(self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(catch)]
  /// Returns a new MOC removing the internal border made of depth max cells.
  pub fn contract(&self) -> Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .contract(self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "externalBorder", catch)]
  /// Returns the external border made of depth max cells.
  pub fn ext_border(&self) -> Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .ext_border(self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "internalBorder",catch)]
  /// Returns the internal border made of depth max cells.
  pub fn int_border(&self) -> Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .int_border(self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

}