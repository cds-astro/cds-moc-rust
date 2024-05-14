use std::{
  collections::HashMap,
  str::from_utf8,
  sync::{Once, RwLock},
};

use js_sys::Array;
use wasm_bindgen::{prelude::*, JsValue};

use moclib::storage::u64idx::U64MocStore;

use crate::{from_url, IsMOC, IsOneDimMOC, MocQType};

/// Function used only once to init the store.
static MOC_STORE_INIT: Once = Once::new();
/// The MOC store (a simple hashmap), protected from concurrent access by a RwLock.
static mut MOC_STORE: Option<RwLock<HashMap<String, TMOC>>> = None;

/// Get (or create and get) the read/write protected MOC store
/// All read/write  operations on the store have to call this method.
pub(crate) fn get_store() -> &'static RwLock<HashMap<String, TMOC>> {
  unsafe {
    // Inspired from the Option get_or_insert_with method, modified to ensure thread safety with
    // https://doc.rust-lang.org/std/sync/struct.Once.html
    // This implements a double-checked lock.
    if let None = MOC_STORE {
      MOC_STORE_INIT.call_once(|| {
        MOC_STORE = Some(RwLock::new(HashMap::new()));
      });
    }
    MOC_STORE.as_ref().unwrap()
  }
}

#[wasm_bindgen]
pub struct TMOC {
  store_index: usize,
}

impl IsMOC for TMOC {
  fn from_store_index(store_index: usize) -> Self {
    Self { store_index }
  }

  fn storage_index(&self) -> usize {
    self.store_index
  }

  fn get_type(&self) -> MocQType {
    MocQType::Time
  }

  fn add_to_store(name: &str, moc: Self) -> Result<(), JsValue> {
    let mut store = get_store()
      .write()
      .map_err(|_| JsValue::from_str("Write lock poisoned"))?;
    (*store).insert(String::from(name), moc);
    Ok(())
  }

  fn from_ascii(data: &str) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .load_tmoc_from_ascii(data)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn from_json(data: &str) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .load_tmoc_from_json(data)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn from_fits(data: &[u8]) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .load_tmoc_from_fits_buff(data)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }
}

impl IsOneDimMOC for TMOC {
  fn depth(&self) -> Result<u8, JsValue> {
    U64MocStore::get_global_store()
      .get_tmoc_depth(self.storage_index())
      .map_err(|e| e.into())
  }
}

#[wasm_bindgen]
impl TMOC {
  #[wasm_bindgen(js_name = "listMocsLoadedFromLocalFile", catch)]
  /// Returns the MOCs identifiers (names) currently in the store (MOCs loaded from local files)
  pub fn list_mocs_loaded_from_local_file() -> Result<Array, JsValue> {
    Ok(
      get_store()
        .read()
        .map_err(|_| JsValue::from_str("Read lock poisoned"))?
        .iter()
        .map(|(key, _)| JsValue::from_str(key))
        .collect::<Array>(),
    )
  }

  #[wasm_bindgen(js_name = "getMocLoadedFromLocalFile", catch)]
  /// Get (and remove from the store) the MOC of given name loaded from a local file.
  pub fn get_moc_loaded_from_local_file(name: &str) -> Result<TMOC, JsValue> {
    let mut store = get_store()
      .write()
      .map_err(|_| JsValue::from_str("Write lock poisoned"))?;
    match (*store).remove(name) {
      Some(moc) => Ok(moc),
      None => Err(JsValue::from_str(&format!(
        "No MOC named '{}' found in store",
        name
      ))),
    }
  }

  #[wasm_bindgen(js_name = "newEmpty", catch)]
  /// Creates a new empty T-MOC of given depth.
  pub fn new_empty(depth: u8) -> Result<TMOC, JsValue> {
    U64MocStore::get_global_store()
      .new_empty_tmoc(depth)
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
  pub fn from_ascii(data: &str) -> Result<TMOC, JsValue> {
    IsMOC::from_ascii(data)
  }

  #[wasm_bindgen(js_name = "fromAsciiUrl", catch)]
  /// WARNING: if this is not working, check e.g. with `wget -v -S ${url}` the the content type is
  /// `Content-Type: text/plain`.
  pub async fn from_ascii_url(url: String) -> Result<TMOC, JsValue> {
    const ERR: &str = "File content is not valid UTF-8.";
    from_url(
      url,
      "text/plain",
      Box::new(|data| Self::from_ascii(from_utf8(data).unwrap_or(ERR))),
    )
    .await
  }

  #[wasm_bindgen(js_name = "fromJson", catch)]
  /// Create a MOC from its JSON serialization
  ///
  /// # Arguments
  /// * `data`: JSON serialization
  pub fn from_json(data: &str) -> Result<TMOC, JsValue> {
    IsMOC::from_json(data)
  }

  #[wasm_bindgen(js_name = "fromJsonUrl", catch)]
  /// WARNING: if this i not working, check e.g. with `wget -v -S ${url}` the the content type is
  /// `Content-Type: application/json`.
  pub async fn from_json_url(url: String) -> Result<TMOC, JsValue> {
    const ERR: &str = "File content is not valid UTF-8.";
    from_url(
      url,
      "application/json",
      Box::new(|data| Self::from_json(from_utf8(data).unwrap_or(ERR))),
    )
    .await
  }

  #[wasm_bindgen(js_name = "fromFits", catch)]
  /// Create a MOC from its FITS serialization
  ///
  /// # Arguments
  /// * `data`: FITS serialization
  pub fn from_fits(data: &[u8]) -> Result<TMOC, JsValue> {
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
  pub async fn from_fits_url(
    url: String,
    accept_mime_types: Option<String>,
  ) -> Result<TMOC, JsValue> {
    match accept_mime_types {
      None => from_url(url, "application/fits", Box::new(Self::from_fits)).await,
      Some(mime_types) => from_url(url, &mime_types, Box::new(Self::from_fits)).await,
    }
  }

  // IsOneDimMOC methods (put here because wasm_bindgen does not export trait methods)

  #[wasm_bindgen(js_name = "getDepth", catch)]
  /// Returns the MOC depth.
  pub fn depth(&self) -> Result<u8, JsValue> {
    IsOneDimMOC::depth(self)
  }

  #[wasm_bindgen(js_name = "coveragePercentage", catch)]
  pub fn coverage_percentage(&self) -> Result<f64, JsValue> {
    IsOneDimMOC::coverage_percentage(self)
  }
  #[wasm_bindgen(js_name = "nRanges", catch)]
  pub fn n_ranges(&self) -> Result<u32, JsValue> {
    IsOneDimMOC::n_ranges(self)
  }
  #[wasm_bindgen(catch)]
  pub fn not(&self) -> Result<TMOC, JsValue> {
    IsOneDimMOC::not(self)
  }
  #[wasm_bindgen(catch)]
  pub fn complement(&self) -> Result<TMOC, JsValue> {
    IsOneDimMOC::complement(self)
  }
  #[wasm_bindgen(catch)]
  pub fn degrade(&self, new_depth: u8) -> Result<TMOC, JsValue> {
    IsOneDimMOC::degrade(self, new_depth)
  }
  #[wasm_bindgen(catch)]
  pub fn or(&self, rhs: &TMOC) -> Result<TMOC, JsValue> {
    IsOneDimMOC::or(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn union(&self, rhs: &TMOC) -> Result<TMOC, JsValue> {
    IsOneDimMOC::union(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn and(&self, rhs: &TMOC) -> Result<TMOC, JsValue> {
    IsOneDimMOC::and(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn intersection(&self, rhs: &TMOC) -> Result<TMOC, JsValue> {
    IsOneDimMOC::intersection(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn xor(&self, rhs: &TMOC) -> Result<TMOC, JsValue> {
    IsOneDimMOC::xor(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn symmetric_difference(&self, rhs: &TMOC) -> Result<TMOC, JsValue> {
    IsOneDimMOC::symmetric_difference(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn minus(&self, rhs: &TMOC) -> Result<TMOC, JsValue> {
    IsOneDimMOC::minus(self, rhs)
  }
  #[wasm_bindgen(catch)]
  pub fn difference(&self, rhs: &TMOC) -> Result<TMOC, JsValue> {
    IsOneDimMOC::difference(self, rhs)
  }

  #[wasm_bindgen(js_name = "fromDecimalJDs", catch)]
  /// Create a new T-MOC from the given list of decimal Julian Days (JD) times.
  ///
  /// # Arguments
  /// * `depth` - T-MOC maximum depth in `[0, 61]`
  /// * `jd` - array of decimal JD time (`f64`)
  ///
  /// # WARNING
  /// Using decimal Julian Days stored on `f64`, the precision does not reach the microsecond
  /// since JD=0.
  /// In Javascript, there is no `u64` type (integers are stored on the mantissa of
  /// a double -- a `f64` --, which is made of 52 bits).
  /// The other approach is to use a couple of `f64`: one for the integer part of the JD, the
  /// other for the fractional part of the JD.
  /// We will add such a method later if required by users.
  pub fn from_decimal_jd(depth: u8, jd: Box<[f64]>) -> Result<TMOC, JsValue> {
    U64MocStore::get_global_store()
      .from_decimal_jd_values(depth, jd.into_iter().cloned())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(js_name = "fromDecimalJDRanges", catch)]
  /// Create a new T-MOC from the given list of decimal Julian Days (JD) ranges.
  ///
  /// # Arguments
  /// * `depth` - T-MOC maximum depth in `[0, 61]`
  /// * `jd_ranges` - array of decimal JD ranges (`f64`): `[jd_min_1, jd_max_2, ..., jd_min_n, jd_max_n]`
  ///
  /// # WARNING
  /// Using decimal Julian Days stored on `f64`, the precision does not reach the microsecond
  /// since JD=0.
  /// In Javascript, there is no `u64` type (integers are stored on the mantissa of
  /// a double -- a `f64` --, which is made of 52 bits).
  /// The other approach is to use a couple of `f64`: one for the integer part of the JD, the
  /// other for the fractional part of the JD.
  /// We will add such a method later if required by users.
  pub fn from_decimal_jd_range(depth: u8, jd_ranges: Box<[f64]>) -> Result<TMOC, JsValue> {
    let jd_ranges_iter = jd_ranges
      .iter()
      .step_by(2)
      .zip(jd_ranges.iter().skip(1).step_by(2))
      .map(|(jd_min, jd_max)| *jd_min..*jd_max);
    U64MocStore::get_global_store()
      .from_decimal_jd_ranges(depth, jd_ranges_iter)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  // - filter

  /// Returns an array of boolean (u8 set to 1 or 0) telling if the time (in Julian Days)
  /// in the input array are in (true=1) or out of (false=0) the T-MOC of given name.
  ///
  /// # Arguments
  /// * `jds`: array of decimal JD time (`f64`)
  ///
  /// # Remarks
  /// The size of the returned boolean (u8) array his the same as the size of the input array.
  #[wasm_bindgen(js_name = "filterJDs", catch)]
  pub fn filter_time(&self, jds: Box<[f64]>) -> Result<Box<[u8]>, JsValue> {
    U64MocStore::get_global_store()
      .filter_time_approx(self.storage_index(), jds.into_iter().cloned(), |b| b as u8)
      .map(|v| v.into_boxed_slice())
      .map_err(|e| e.into())
  }
}
