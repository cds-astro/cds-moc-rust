//! We chose to store the MOCs on the WASM side and only provide MOCs identifiers (uniq names)
//! on the Javascript side.
//! The MOCs are available in a "store" protected from concurrent access.

use std::sync::{RwLock, Once};
use std::collections::HashMap;

use wasm_bindgen::JsValue;
use js_sys::Array;

use super::{MocQType, MocInfo};
use super::common::InternalMoc;

/// Fonction used only once to init the store
static MOC_STORE_INIT: Once = Once::new();
/// The MOC store (a simple hasmap), protected from concurrent access by a RwLock.
static mut MOC_STORE: Option<RwLock<HashMap<String, InternalMoc>>> = None;

/// Get (or create and get) the read/write protected MOC store
/// All read/write  operations on the store have to call this method.
pub(crate) fn get_store() -> &'static RwLock<HashMap<String, InternalMoc>> {
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

/// Add a new MOC to the store
pub(crate) fn add(name: &str, moc: InternalMoc) -> Result<(), JsValue> {
  let mut store = get_store().write().map_err(|_| JsValue::from_str("Write lock poisoned"))?;
  (*store).insert(String::from(name), moc);
  Ok(())
}

pub(crate) fn drop(name: &str) -> Result<(), JsValue> {
  let mut store = get_store().write().map_err(|_| JsValue::from_str("Write lock poisoned"))?;
  (*store).remove(name);
  Ok(())
}

/// Returns the MOCs identifiers (names)
pub(crate) fn list_mocs() -> Result<Array, JsValue> {
  Ok(
    get_store().read().map_err(|_| JsValue::from_str("Read lock poisoned"))?
    .iter()
    .map(|(key, _)| JsValue::from_str(key))
    .collect::<Array>()
  )
}

/// Returns the type of the MOC of given identifier, `None` is the identifier is not recognized.
pub(crate) fn get_qtype(name: &str) -> Option<MocQType> {
  get_store().read().unwrap()
    .get(name)
    .map(|moc| moc.get_qty_type())
}

/// Returns the type of the MOC of given identifier, `None` is the identifier is not recognized.
pub(crate) fn get_info(name: &str) -> Option<MocInfo> {
  get_store().read().unwrap()
    .get(name)
    .map(|moc| {
      let qtype = moc.get_qty_type();
      let (space_depth, time_depth) = moc.get_space_time_depths();
      let coverage_percentage = moc.get_coverage_percentage();
      let n_ranges = moc.get_nranges();
      MocInfo::new(qtype, space_depth, time_depth, coverage_percentage, n_ranges)
    })
}

pub(crate) fn exec<R, F>(name: &str, op: F) -> Option<R> 
  where
    R: ,
    F: Fn(&InternalMoc) -> R
{
  get_store().read().unwrap()
    .get(name)
    .map(|moc| op(moc))
}

/// Perform an operation on a MOC and store the resulting MOC.
pub(crate) fn op1<F>(name: &str, op: F, res_name: &str) -> Result<(), JsValue>
  where
    F: Fn(&InternalMoc) -> Result<InternalMoc, String>
{
  let store = get_store();
  // Perform read operations first
  let res_moc = {
    let store = store.read().map_err(|_| JsValue::from_str("Read lock poisoned"))?;
    let moc = store.get(name).ok_or_else(|| JsValue::from_str(&format!("MOC '{}' not found", name)))?;
    op(moc).map_err(|e| JsValue::from_str(&e))?
  };
  // Then write operation.
  // Remark: we could have called directly add(res_name, res_moc) 
  //         (still carefully releasing the read lock before the call), 
  //         but we (so far) preferred to spare one `get_store` call
  let mut store = store.write().map_err(|_| JsValue::from_str("Write lock poisoned"))?;
  (*store).insert(String::from(res_name), res_moc);
  Ok(())
}

/// Perform an operation between 2 MOCs and store the resulting MOC.
pub(crate) fn op2<F>(left_name: &str, right_name: &str, op: F, res_name: &str) -> Result<(), JsValue> 
  where 
    F: Fn(&InternalMoc, &InternalMoc) -> Result<InternalMoc, String>
{
  let store = get_store();
  // Perform read operations first
  let res_moc = {
    let store = store.read().map_err(|_| JsValue::from_str("Read lock poisoned"))?;
    let left = store.get(left_name).ok_or_else(|| JsValue::from_str(&format!("MOC '{}' not found", left_name)))?;
    let right = store.get(right_name).ok_or_else(|| JsValue::from_str(&format!("MOC '{}' not found", right_name)))?;
    op(left, right).map_err(|e| JsValue::from_str(&e))?
  };
  // Then write operation.
  // Remark: we could have called directly add(res_name, res_moc) 
  //         (still carefully releasing the read lock before the call), 
  //         but we (so far) preferred to spare one `get_store` call
  let mut store = store.write().map_err(|_| JsValue::from_str("Write lock poisoned"))?;
  (*store).insert(String::from(res_name), res_moc);
  Ok(())
}
