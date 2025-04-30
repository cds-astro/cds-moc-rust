//! We chose to store the MOCs on the WASM side and only provide MOCs identifiers (uniq names)
//! on the Javascript side.
//! The MOCs are available in a "store" protected from concurrent access.

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use slab::Slab;

use super::common::InternalMoc;

static STORE: RwLock<Slab<(u8, InternalMoc)>> = RwLock::new(Slab::new());

fn exec_on_readonly_store<R, F>(op: F) -> Result<R, String>
where
  F: FnOnce(RwLockReadGuard<'_, Slab<(u8, InternalMoc)>>) -> Result<R, String>,
{
  STORE
    .read()
    .map_err(|e| format!("Read lock poisoned: {}", e))
    .and_then(op)
}

fn exec_on_readwrite_store<R, F>(op: F) -> Result<R, String>
where
  F: FnOnce(RwLockWriteGuard<'_, Slab<(u8, InternalMoc)>>) -> Result<R, String>,
{
  STORE
    .write()
    .map_err(|e| format!("Write lock poisoned: {}", e))
    .and_then(op)
}

pub(crate) fn exec_on_one_readonly_moc<T, F>(index: usize, op: F) -> Result<T, String>
where
  F: FnOnce(&InternalMoc) -> Result<T, String>,
{
  exec_on_readonly_store(|store| {
    store
      .get(index)
      .ok_or_else(|| format!("MOC at index '{}' not found", index))
      .and_then(|(_, moc)| op(moc))
  })
}

pub(crate) fn exec_on_one_readwrite_moc<T, F>(index: usize, op: F) -> Result<T, String>
where
  F: FnOnce(&mut InternalMoc) -> Result<T, String>,
{
  exec_on_readwrite_store(|mut store| {
    store
      .get_mut(index)
      .ok_or_else(|| format!("MOC at index '{}' not found", index))
      .and_then(|(_, moc)| op(moc))
  })
}

pub(crate) fn exec_on_two_readonly_mocs<T, F>(il: usize, ir: usize, op: F) -> Result<T, String>
where
  F: Fn(&InternalMoc, &InternalMoc) -> Result<T, String>,
{
  exec_on_readonly_store(|store| {
    let (_, l) = store
      .get(il)
      .ok_or_else(|| format!("MOC at index '{}' not found", il))?;
    let (_, r) = store
      .get(ir)
      .ok_or_else(|| format!("MOC at index '{}' not found", ir))?;
    op(l, r)
  })
}

fn exec_on_n_readonly_mocs<T, F>(indices: &[usize], op: F) -> Result<T, String>
where
  F: Fn(Vec<&InternalMoc>) -> Result<T, String>,
{
  exec_on_readonly_store(|store| {
    let mocs: Vec<&InternalMoc> = indices
      .iter()
      .cloned()
      .map(|i| {
        store
          .get(i)
          .map(|(_, moc)| moc)
          .ok_or_else(|| format!("MOC at index '{}' not found", i))
      })
      .collect::<Result<_, _>>()?;
    op(mocs)
  })
}

/// Add a new MOC to the store, retrieve the index at which it has been inserted
pub(crate) fn add<T: Into<InternalMoc>>(moc: T) -> Result<usize, String> {
  exec_on_readwrite_store(|mut store| Ok(store.insert((1, moc.into()))))
}

/// Add a new MOC to the store, retrieve the index at which it has been inserted
pub(crate) fn copy_moc(index: usize) -> Result<(), String> {
  exec_on_readwrite_store(|mut store| {
    store
      .get_mut(index)
      .ok_or_else(|| format!("MOC at index '{}' not found", index))
      .and_then(|entry| {
        if entry.0 == 255 {
          Err(String::from(
            "Unable to copy MOC: 255 copies already reached",
          ))
        } else {
          entry.0 += 1;
          Ok(())
        }
      })
  })
}

/// Drop and return the content of the store at the given index.
pub(crate) fn drop(index: usize) -> Result<Option<InternalMoc>, String> {
  exec_on_readwrite_store(move |mut store| {
    let count = store
      .get_mut(index)
      .map(|entry| {
        entry.0 -= 1;
        entry.0
      })
      .ok_or_else(|| format!("MOC at index '{}' not found", index))?;
    Ok(if count == 0 {
      let moc = store.remove(index).1;
      Some(moc)
    } else {
      None
    })
  })
}

/*
/// Returns the MOCs identifiers (names)
pub(crate) fn list_mocs() -> Result<Array, String> {
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
*/

/// Perform an operation on a MOC and store the resulting MOC, providing in output
/// its index in the store.
pub(crate) fn op1<F>(index: usize, op: F) -> Result<usize, String>
where
  F: Fn(&InternalMoc) -> Result<InternalMoc, String>,
{
  // Exec first the read operation (that may take some time) using only a read lock
  let moc = exec_on_one_readonly_moc(index, op)?;
  // Then use the write lock only to store the result (short operation)
  add(moc)
}

/// Perform an operation on a MOC and store the resulting MOC, returning their indices.
pub(crate) fn op1_multi_res<F>(index: usize, op: F) -> Result<Vec<usize>, String>
where
  F: Fn(&InternalMoc) -> Result<Vec<InternalMoc>, String>,
{
  // Exec first the read operation (that may take some time) using only a read lock
  let mut mocs = exec_on_one_readonly_moc(index, op)?;
  // Then use the write lock only to store the results (shorter operation)
  exec_on_readwrite_store(move |mut store| {
    Ok(
      mocs
        .drain(..)
        .map(move |moc| store.insert((1, moc)))
        .collect(),
    )
  })
}

/// Perform an operation between 2 MOCs and store the resulting MOC.
pub(crate) fn op2<F>(left_index: usize, right_index: usize, op: F) -> Result<usize, String>
where
  F: Fn(&InternalMoc, &InternalMoc) -> Result<InternalMoc, String>,
{
  // Exec first the read operation (that may take some time) using only a read lock
  let moc = exec_on_two_readonly_mocs(left_index, right_index, op)?;
  // Then use the write lock only to store the result (short operation)
  add(moc)
}

/// Perform an operation between 2 MOCs and store the resulting MOC.
pub(crate) fn opn<F>(indices: &[usize], op: F) -> Result<usize, String>
where
  F: Fn(Vec<&InternalMoc>) -> Result<InternalMoc, String>,
{
  // Exec first the read operation (that may take some time) using only a read lock
  let moc = exec_on_n_readonly_mocs(indices, op)?;
  // Then use the write lock only to store the result (short operation)
  add(moc)
}
