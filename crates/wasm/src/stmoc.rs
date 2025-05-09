use std::{
  collections::HashMap,
  str::from_utf8,
  sync::{Once, RwLock},
};

use js_sys::Array;
use wasm_bindgen::{prelude::*, JsValue};

use moclib::storage::u64idx::U64MocStore;

use crate::{from_url, smoc::MOC, tmoc::TMOC, IsMOC, MocQType};

/// Function used only once to init the store.
static MOC_STORE_INIT: Once = Once::new();
/// The MOC store (a simple hashmap), protected from concurrent access by a RwLock.
static mut MOC_STORE: Option<RwLock<HashMap<String, STMOC>>> = None;

/// Get (or create and get) the read/write protected MOC store
/// All read/write  operations on the store have to call this method.
pub(crate) fn get_store() -> &'static RwLock<HashMap<String, STMOC>> {
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
pub struct STMOC {
  store_index: usize,
}

impl IsMOC for STMOC {
  fn from_store_index(store_index: usize) -> Self {
    Self { store_index }
  }

  fn storage_index(&self) -> usize {
    self.store_index
  }

  fn get_type(&self) -> MocQType {
    MocQType::SpaceTime
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
      .load_stmoc_from_ascii(data)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn from_json(data: &str) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .load_stmoc_from_json(data)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn from_fits(data: &[u8]) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .load_stmoc_from_fits_buff(data)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }
}

#[wasm_bindgen]
impl STMOC {
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
  pub fn get_moc_loaded_from_local_file(name: &str) -> Result<STMOC, JsValue> {
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

  #[wasm_bindgen(catch)]
  pub fn depth_time(&self) -> Result<u8, JsValue> {
    U64MocStore::get_global_store()
      .get_stmoc_depths(self.storage_index())
      .map(|(depth, _)| depth)
      .map_err(|e| e.into())
  }

  #[wasm_bindgen(catch)]
  pub fn depth_space(&self) -> Result<u8, JsValue> {
    U64MocStore::get_global_store()
      .get_stmoc_depths(self.storage_index())
      .map(|(_, depth)| depth)
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
  pub fn from_ascii(data: &str) -> Result<STMOC, JsValue> {
    IsMOC::from_ascii(data)
  }

  #[wasm_bindgen(js_name = "fromAsciiUrl", catch)]
  /// WARNING: if this is not working, check e.g. with `wget -v -S ${url}` the the content type is
  /// `Content-Type: text/plain`.
  pub async fn from_ascii_url(url: String) -> Result<STMOC, JsValue> {
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
  pub fn from_json(data: &str) -> Result<STMOC, JsValue> {
    IsMOC::from_json(data)
  }

  #[wasm_bindgen(js_name = "fromJsonUrl", catch)]
  /// WARNING: if this i not working, check e.g. with `wget -v -S ${url}` the the content type is
  /// `Content-Type: application/json`.
  pub async fn from_json_url(url: String) -> Result<STMOC, JsValue> {
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
  pub fn from_fits(data: &[u8]) -> Result<STMOC, JsValue> {
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
  #[wasm_bindgen(js_name = "fromFitsUrl", catch)]
  pub async fn from_fits_url(
    url: String,
    accept_mime_types: Option<String>,
  ) -> Result<STMOC, JsValue> {
    match accept_mime_types {
      None => from_url(url, "application/fits", Box::new(Self::from_fits)).await,
      Some(mime_types) => from_url(url, &mime_types, Box::new(Self::from_fits)).await,
    }
  }

  /// Returns the union of the S-MOCs associated to T-MOCs intersecting the given T-MOC.
  #[wasm_bindgen(js_name = "timeFold", catch)]
  pub fn time_fold(&self, time_moc: &TMOC) -> Result<MOC, JsValue> {
    U64MocStore::get_global_store()
      .time_fold(time_moc.storage_index(), self.storage_index())
      .map(MOC::from_store_index)
      .map_err(|e| e.into())
  }

  /// Returns the union of the T-MOCs associated to S-MOCs intersecting the given S-MOC.
  #[wasm_bindgen(js_name = "spaceFold", catch)]
  pub fn space_fold(&self, space_moc: &MOC) -> Result<TMOC, JsValue> {
    U64MocStore::get_global_store()
      .space_fold(space_moc.storage_index(), self.storage_index())
      .map(TMOC::from_store_index)
      .map_err(|e| e.into())
  }
}
