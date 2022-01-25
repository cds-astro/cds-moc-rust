extern crate console_error_panic_hook;

use std::panic;
use std::str::{from_utf8, from_utf8_unchecked};
use std::io::Cursor;

use unreachable::{UncheckedOptionExt, UncheckedResultExt};

use serde::{Serialize, Deserialize};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  Url, Blob, BlobPropertyBag,
  Event, FileReader,
  HtmlAnchorElement, HtmlInputElement,
  Request, RequestInit, RequestMode, Response
};
use js_sys::{Array, Uint8Array};

use moclib::qty::{MocQty, Hpx, Time};
use moclib::elem::valuedcell::valued_cells_to_moc_with_opt;
use moclib::elemset::range::HpxRanges;
use moclib::moc::{
  CellMOCIterator, CellMOCIntoIterator,
  RangeMOCIterator,
  CellOrCellRangeMOCIterator, CellOrCellRangeMOCIntoIterator,
  range::RangeMOC,
};
use moclib::moc2d::{
  RangeMOC2Iterator, RangeMOC2IntoIterator,
  CellMOC2IntoIterator,
  CellOrCellRangeMOC2IntoIterator,
};
use moclib::deser::{
  ascii::{from_ascii_ivoa, moc2d_from_ascii_ivoa},
  json::{from_json_aladin, cellmoc2d_from_json_aladin},
  fits::{
    from_fits_ivoa, MocIdxType,
    multiordermap::from_fits_multiordermap
  }
};

pub(crate) mod common;
pub(crate) mod store;
pub(crate) mod load;
pub(crate) mod op1;
pub(crate) mod op2;

use self::common::{
  PI, HALF_PI,
  InternalMoc,
  lon_deg2rad, lat_deg2rad
};
use self::load::{from_fits_gen, from_fits_u64};
use self::op1::{Op1, op1, op1_count_split};
use self::op2::{Op2, op2};

/// Number of microseconds in a 24h day.
const JD_TO_USEC: f64 = (24_u64 * 60 * 60 * 1_000_000) as f64;

////////////////////////
// IMPORT JS FONCTION //
// see https://rustwasm.github.io/docs/wasm-bindgen/examples/console-log.html
#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(js_namespace = console)]
  fn log(s: &str);
}


/// Activate debugging mode (Rust stacktrace)
#[wasm_bindgen(js_name = "debugOn")]
pub fn debug_on() {
  console_error_panic_hook::set_once();
}

/////////////////////////////
// GET INFO ON LOADED MOCs //

#[wasm_bindgen]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum MocQType {
  Space = "space",
  Time = "time",
  SpaceTime = "space-time",
}

#[derive(Serialize, Deserialize)]
pub struct MocInfo {
  pub qtype: MocQType,
  pub space_depth: Option<u8>,
  pub time_depth: Option<u8>,
  pub coverage_percentage: Option<f64>,
  pub n_ranges: i32,
}
impl MocInfo {
  pub(crate) fn new(
    qtype: MocQType, 
    space_depth: Option<u8>, 
    time_depth: Option<u8>,
    coverage_percentage: Option<f64>, 
    n_ranges: u32
  ) -> Self {
    MocInfo { 
      qtype, 
      space_depth,
      time_depth, 
      coverage_percentage,
      n_ranges: n_ranges as i32
    }
  }
}


/// List the name of the MOCs currently loaded in memory.
#[wasm_bindgen(catch)]
pub fn list() -> Result<Array, JsValue> {
  store::list_mocs()
}

/// Get the quantity type (space, time or space-time) of the MOC having the given name.
#[wasm_bindgen]
pub fn qtype(name: &str) -> Option<MocQType> {
  store::get_qtype(name)
}

/// Get information on the MOC having the given name.
#[wasm_bindgen]
pub fn info(name: &str) -> JsValue {
  store::get_info(name)
    .map(|o| JsValue::from_serde(&o).unwrap_or(JsValue::from_str("Serde error serializing info.")))
    .unwrap_or(JsValue::NULL)
}

/// Remove from memory the MOC having the given name.
#[wasm_bindgen(catch)]
pub fn drop(name: &str) -> Result<(), JsValue> {
  store::drop(name)
}

//////////////
// LOAD MOC //

// Replacing monomorphisation by a trait object, we save ~10 kB on the final wasm file.
// Monomorphisation:
//   async fn from_url<F>(name: String, url: String, mime: &'static str, parse: F) -> Result<(), JsValue>
//    where
//      F: Fn(&str, &[u8]) ->  Result<(), JsValue>
// Trait object: Box<dyn ...>
async fn from_url(
  name: String,
  url: String,
  mime: &'static str,
  parse: Box<dyn Fn(&str, &[u8]) ->  Result<(), JsValue>>
) -> Result<(), JsValue>
{
  // https://rustwasm.github.io/docs/wasm-bindgen/examples/fetch.html
  let mut opts = RequestInit::new();
  opts.method("GET");
  opts.mode(RequestMode::Cors);

  let request = Request::new_with_str_and_init(&url, &opts)?;
  request.headers().set("Accept", mime)?;

  let window = web_sys::window().ok_or_else(|| JsValue::from_str("Unable to get the Window"))?;
  let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

  let resp: Response = resp_value.dyn_into().map_err(|_| JsValue::from_str("Error casting to Response"))?;
  // Convert this other `Promise` into a rust `Future`.
  let buff = JsFuture::from(resp.array_buffer()?).await?;
  // log(&format!("Blob: {:?}", &blob));
  let file_content: Vec<u8> = js_sys::Uint8Array::new(&buff).to_vec();
  // log(&format!("Byte size: {}", file_content.len()));
  parse(&name, &file_content)
}

/// Open the file selection dialog and load the MOC contained in the selected file 
/// (for security reasons, we cannot simply provide a path on the client machine).
/// # Info
/// * For Json and Ascii file, requires the type of MOC to be loaded (Space, Time or Space-Time)
/// # Warning
/// Because of security restriction, the call to this method
/// **"needs to be triggered within a code block that was the handler of a user-initiated event"**
#[wasm_bindgen(js_name = "fromLocalFile", catch)]
pub fn from_local_file(qtype: Option<MocQType>) -> Result<(), JsValue> {
  // Create the file input action that will be fired by the event 'change'
  let file_input_action = Closure::wrap(Box::new(move |event: Event| {
    let element = unsafe { event.target().unchecked_unwrap().dyn_into::<HtmlInputElement>().unchecked_unwrap_ok() };
    let filelist = unsafe {  element.files().unchecked_unwrap() };
    for i in 0..filelist.length() {
      let file = unsafe {  filelist.get(i).unchecked_unwrap() };
      let file_name = file.name();
      let file_reader = unsafe {  FileReader::new().unchecked_unwrap_ok() };
      // There is a stream method, but I am not sure how to use it. I am so far going the easy way.
      match file_reader.read_as_array_buffer(&file) {
        Err(_) => log("Error reading file content"),
        _ => { },
      };
      let file_onload = Closure::wrap(Box::new(move |event: Event| {
        let file_reader: FileReader = unsafe { event.target().unchecked_unwrap().dyn_into().unchecked_unwrap_ok() };
        let file_content = unsafe { file_reader.result().unchecked_unwrap_ok() };
        let file_content: Vec<u8> = js_sys::Uint8Array::new(&file_content).to_vec();
        // log(&format!("File len {:?}", file_content.len()));
        // We accept only ".fits", ".json", ".ascii", and ".txt" files
        // so splitting on "." should be safe.
        let (name, ext) = unsafe { file_name.rsplit_once('.').unchecked_unwrap() };
        let res = match ext {
          "fits" => from_fits(name, &file_content),
          "json" => match qtype {
            Some(MocQType::Space) => smoc_from_json(name, unsafe{ from_utf8_unchecked(&file_content) }),
            Some(MocQType::Time) => tmoc_from_json(name,unsafe{ from_utf8_unchecked(&file_content) } ),
            Some(MocQType::SpaceTime) => stmoc_from_json(name, unsafe{ from_utf8_unchecked(&file_content) }),
            _ => {
              let msg = format!("Unable to load file '{}' without knowing the MOC quantity type", file_name);
              log(&msg);
              Err(JsValue::from_str(&msg))
            },
          },
          "txt" | "ascii" => match qtype {
            Some(MocQType::Space) => smoc_from_ascii(name, unsafe{ from_utf8_unchecked(&file_content) }),
            Some(MocQType::Time) => tmoc_from_ascii(name, unsafe{ from_utf8_unchecked(&file_content) }),
            Some(MocQType::SpaceTime) =>  stmoc_from_ascii(name, unsafe{ from_utf8_unchecked(&file_content) }),
            _ => {
              let msg = format!("Unable to load file '{}' without knowing the MOC quantity type", file_name);
              log(&msg);
              Err(JsValue::from_str(&msg))
            },
          },
          _ => unreachable!(), // since file_input.set_attribute("accept", ".fits, .json, .ascii, .txt");
        };
        match res {
          Err(e) => log(&e.as_string().unwrap_or_else(|| String::from("Error parsing file"))),
          _ => { },
        };
      }) as Box<dyn FnMut(_)>);
      file_reader.set_onload(Some(file_onload.as_ref().unchecked_ref()));
      file_onload.forget();
    }
  }) as Box<dyn FnMut(_)>);
  
  // Create a temporary input file and click on it
  // - get the body
  let window = web_sys::window().expect("no global `window` exists");
  // This could be used but not yet in web_sys: https://developer.mozilla.org/en-US/docs/Web/API/Window/showOpenFilePicker
  let document = window.document().expect("should have a document on window");
  let body = document.body().expect("document should have a body");
  // - create the input
  let file_input: HtmlInputElement = unsafe { document.create_element("input").unchecked_unwrap_ok().dyn_into()? };
  file_input.set_type("file");
  unsafe {
    file_input.set_attribute("multiple", "").unchecked_unwrap_ok();
    file_input.set_attribute("hidden", "").unchecked_unwrap_ok();
    file_input.set_attribute("accept", ".fits, .json, .ascii, .txt").unchecked_unwrap_ok();
  }
  file_input.add_event_listener_with_callback("change", file_input_action.as_ref().unchecked_ref())?;
  file_input_action.forget();
  // - attach the input
  body.append_child(&file_input)?;
  // - simulate a click
  file_input.click();
  // - remove the input
  body.remove_child(&file_input)?;
  Ok(())
}

/// Open the file selection dialog and load the mulit-order-map the fits file contains 
/// (for security reasons, we cannot simply provide a path on the client machine).
/// # Warning
/// Because of security restriction, the call to this method
/// **"needs to be triggered within a code block that was the handler of a user-initiated event"**
#[wasm_bindgen(js_name = "fromLocalMultiOrderMap", catch)]
pub fn from_local_multiordermap(
  from_threshold: f64,
  to_threshold: f64,
  asc: bool,
  not_strict: bool,
  split: bool,
  revese_recursive_descent: bool,
) -> Result<(), JsValue> {
  // Create the file input action that will be fired by the event 'change'
  let file_input_action = Closure::wrap(Box::new(move |event: Event| {
    let element = unsafe { event.target().unchecked_unwrap().dyn_into::<HtmlInputElement>().unchecked_unwrap_ok() };
    let filelist = unsafe {  element.files().unchecked_unwrap() };
    for i in 0..filelist.length() {
      let file = unsafe {  filelist.get(i).unchecked_unwrap() };
      let file_name = file.name();
      let file_reader = unsafe {  FileReader::new().unchecked_unwrap_ok() };
      // There is a stream method, but I am not sure how to use it. I am so far going the easy way.
      match file_reader.read_as_array_buffer(&file) {
        Err(_) => log("Error reading file content"),
        _ => { },
      };
      let file_onload = Closure::wrap(Box::new(move |event: Event| {
        let file_reader: FileReader = unsafe { event.target().unchecked_unwrap().dyn_into().unchecked_unwrap_ok() };
        let file_content = unsafe { file_reader.result().unchecked_unwrap_ok() };
        let file_content: Vec<u8> = js_sys::Uint8Array::new(&file_content).to_vec();
        // log(&format!("File len {:?}", file_content.len()));
        // We accept only ".fits" files so splitting on "." should be safe.
        let (name, ext) = unsafe { file_name.rsplit_once('.').unchecked_unwrap() };
        let res = match ext {
          "fits" => from_multitordermap_fits_file(
            name, &file_content, from_threshold, to_threshold, 
            asc, not_strict, split, revese_recursive_descent
          ),
          _ => unreachable!(), // since file_input.set_attribute("accept", ".fits");
        };
        match res {
          Err(e) => log(&e.as_string().unwrap_or_else(|| String::from("Error parsing file"))),
          _ => { },
        };
      }) as Box<dyn FnMut(_)>);
      file_reader.set_onload(Some(file_onload.as_ref().unchecked_ref()));
      file_onload.forget();
    }
  }) as Box<dyn FnMut(_)>);

  // Create a temporary input file and click on it
  // - get the body
  let window = web_sys::window().expect("no global `window` exists");
  // This could be used but not yet in web_sys: https://developer.mozilla.org/en-US/docs/Web/API/Window/showOpenFilePicker
  let document = window.document().expect("should have a document on window");
  let body = document.body().expect("document should have a body");
  // - create the input
  let file_input: HtmlInputElement = unsafe { document.create_element("input").unchecked_unwrap_ok().dyn_into()? };
  file_input.set_type("file");
  unsafe {
    file_input.set_attribute("multiple", "").unchecked_unwrap_ok();
    file_input.set_attribute("hidden", "").unchecked_unwrap_ok();
    file_input.set_attribute("accept", ".fits").unchecked_unwrap_ok();
  }
  file_input.add_event_listener_with_callback("change", file_input_action.as_ref().unchecked_ref())?;
  file_input_action.forget();
  // - attach the input
  body.append_child(&file_input)?;
  // - simulate a click
  file_input.click();
  // - remove the input
  body.remove_child(&file_input)?;
  Ok(())
}

// - from FITS 

#[wasm_bindgen(js_name = "fromFits", catch)]
pub fn from_fits(name: &str, data: &[u8]) -> Result<(), JsValue> {
  // log(&format!("Name: {}; File len {:?}", name, data.len()));
  // Build the MOC
  let moc = match from_fits_ivoa(Cursor::new(data)).map_err(|e| JsValue::from_str(&e.to_string()))? {
    MocIdxType::U16(moc) => from_fits_gen(moc),
    MocIdxType::U32(moc) => from_fits_gen(moc),
    MocIdxType::U64(moc) => from_fits_u64(moc),
  }.map_err(|e| JsValue::from_str(&e.to_string()))?;
  // Add it to the store
  store::add(name, moc)
}

/// Create o S-MOC from a FITS multi-prder map plus other parameters.
/// * `from_threshold`: Cumulative value at which we start putting cells in he MOC (often = 0).
/// * `to_threshold`: Cumulative value at which we stop putting cells in the MOC.
/// * `asc`: Compute cumulative value from ascending density values instead of descending (often = false).
/// * `not_strict`: Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
/// * `split`: Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
/// * `revese_recursive_descent`: Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
#[wasm_bindgen(js_name = "fromFitsMulitOrderMap", catch)]
pub fn from_multitordermap_fits_file(
  name: &str, 
  data: &[u8],
  from_threshold: f64,
  to_threshold: f64,
  asc: bool,
  not_strict: bool,
  split: bool,
  revese_recursive_descent: bool,
) -> Result<(), JsValue> {
  let moc = from_fits_multiordermap(
    Cursor::new(data),
    from_threshold,
    to_threshold,
    asc,
    !not_strict,
    split,
    revese_recursive_descent
  ).map_err(|e| JsValue::from_str(&e.to_string()))?;
  // Add it to the store
  store::add(name, InternalMoc::Space(moc))
}

/*
#[wasm_bindgen(js_name = "fromFitsMulitOrderMapStd", catch)]
pub fn from_mutlitordermap_fits_file_std(
  name: &str,
  data: &[u8],
) -> Result<(), JsValue> {
  let moc = from_fits_multiordermap(
    Cursor::new(data),
    0.0,
    0.9,
    false,
    true,
    false,
    false
  ).map_err(|e| JsValue::from_str(&e.to_string()))?;
  // Add it to the store
  store::add(name, InternalMoc::Space(moc))
}*/

/// WARNING: if this is not working, check e.g. with `wget -v -S ${url}` the the content type is
/// `Content-Type: application/fits`.
#[wasm_bindgen(js_name = "fromFitsUrl")]
pub async fn from_fits_url(name: String, url: String) -> Result<(), JsValue> {
  from_url(name, url, "application/fits", Box::new(from_fits)).await
}


/// WARNING: if this is not working, check e.g. with `wget -v -S ${url}` the the content type is
/// `Content-Type: application/fits`.
 #[wasm_bindgen(js_name = "fromMultiOrderMapFitsUrl")]
pub async fn from_multiordermap_url(
  name: String,
  url: String,
  from_threshold: f64,
  to_threshold: f64,
  asc: bool,
  not_strict: bool,
  split: bool,
  revese_recursive_descent: bool,
) -> Result<(), JsValue>
{
  let func = move |name: &str, data: &[u8]| from_multitordermap_fits_file(
    name, 
    data,
    from_threshold,
    to_threshold,
    asc,
    not_strict,
    split,
    revese_recursive_descent
  );
  from_url(name, url, "application/fits", Box::new(func)).await
}

/*
/// WARNING: if this is not working, check e.g. with `wget -v -S ${url}` the the content type is
/// `Content-Type: application/fits`.
#[wasm_bindgen(js_name = "fromMultiOrderMapFitsUrlStd")]
pub async fn from_multiordermap_url_std(name: String, url: String) -> Result<(), JsValue> {
  console_error_panic_hook::set_once();
  from_url(name, url, "application/fits", Box::new(from_mutlitordermap_fits_file_std)).await
}*/

// - from ASCII

#[wasm_bindgen(js_name = "smocFromAscii", catch)]
pub fn smoc_from_ascii(name: &str, data: &str) -> Result<(), JsValue> {
  let cellcellranges = from_ascii_ivoa::<u64, Hpx::<u64>>(data)
    .map_err(|e| JsValue::from_str(&e.to_string()))?;
  let moc = cellcellranges.into_cellcellrange_moc_iter().ranges().into_range_moc();
  store::add(name, InternalMoc::Space(moc))
}

/// WARNING: if this i not working, check e.g. with `wget -v -S ${url}` the the content type is
/// `Content-Type: text/plain`.
#[wasm_bindgen(js_name = "smocFromAsciiUrl")]
pub async fn smoc_from_ascii_url(name: String, url: String) -> Result<(), JsValue> {
  const ERR: &str = "File content is not valid UTF-8.";
  from_url(
    name, url, "text/plain",
    Box::new(|name, data| smoc_from_ascii(name, from_utf8(data).unwrap_or(ERR)) )
  ).await
}

#[wasm_bindgen(js_name = "tmocFromAscii", catch)]
pub fn tmoc_from_ascii(name: &str, data: &str) -> Result<(), JsValue> {
  let cellcellranges = from_ascii_ivoa::<u64, Time::<u64>>(data)
    .map_err(|e| JsValue::from_str(&e.to_string()))?;
  let moc = cellcellranges.into_cellcellrange_moc_iter().ranges().into_range_moc();
  store::add(name, InternalMoc::Time(moc))
}

/// WARNING: if this i not working, check e.g. with `wget -v -S ${url}` the the content type is
/// `Content-Type: text/plain`.
#[wasm_bindgen(js_name = "tmocFromAsciiUrl")]
pub async fn tmoc_from_ascii_url(name: String, url: String) -> Result<(), JsValue> {
  const ERR: &str = "File content is not valid UTF-8.";
  from_url(
    name, url, "text/plain",
    Box::new(|name, data| tmoc_from_ascii(name, from_utf8(data).unwrap_or(ERR)) )
  ).await
}

#[wasm_bindgen(js_name = "stmocFromAscii", catch)]
pub fn stmoc_from_ascii(name: &str, data: &str) -> Result<(), JsValue> {
  let cellrange2 = moc2d_from_ascii_ivoa::<u64, Time::<u64>, u64, Hpx::<u64>>(data)
    .map_err(|e| JsValue::from_str(&e.to_string()))?;
  let moc2 = cellrange2.into_cellcellrange_moc2_iter().into_range_moc2_iter().into_range_moc2();
  store::add(name, InternalMoc::TimeSpace(moc2))
}

/// WARNING: if this i not working, check e.g. with `wget -v -S ${url}` the the content type is
/// `Content-Type: text/plain`.
#[wasm_bindgen(js_name = "stmocFromAsciiUrl")]
pub async fn stmoc_from_ascii_url(name: String, url: String) -> Result<(), JsValue> {
  const ERR: &str = "File content is not valid UTF-8.";
  from_url(
    name, url, "text/plain",
    Box::new(|name, data| stmoc_from_ascii(name, from_utf8(data).unwrap_or(ERR)) )
  ).await
}

// - from JSON

#[wasm_bindgen(js_name = "smocFromJson", catch)]
pub fn smoc_from_json(name: &str, data: &str) -> Result<(), JsValue> {
  let cells = from_json_aladin::<u64, Hpx::<u64>>(data)
    .map_err(|e| JsValue::from_str(&e.to_string()))?;
  let moc = cells.into_cell_moc_iter().ranges().into_range_moc();
  store::add(name, InternalMoc::Space(moc))
}

/// WARNING: if this i not working, check e.g. with `wget -v -S ${url}` the the content type is
/// `Content-Type: application/json`.
#[wasm_bindgen(js_name = "smocFromJsonUrl")]
pub async fn smoc_from_json_url(name: String, url: String) -> Result<(), JsValue> {
  const ERR: &str = "File content is not valid UTF-8.";
  from_url(
    name, url, "application/json",
    Box::new(|name, data| smoc_from_json(name, from_utf8(data).unwrap_or(ERR)) )
  ).await
}

#[wasm_bindgen(js_name = "tmocFromJson", catch)]
pub fn tmoc_from_json(name: &str, data: &str) -> Result<(), JsValue> {
  let cells = from_json_aladin::<u64, Time::<u64>>(data)
    .map_err(|e| JsValue::from_str(&e.to_string()))?;
  let moc = cells.into_cell_moc_iter().ranges().into_range_moc();
  store::add(name, InternalMoc::Time(moc))
}

/// WARNING: if this i not working, check e.g. with `wget -v -S ${url}` the the content type is
/// `Content-Type: application/json`.
#[wasm_bindgen(js_name = "tmocFromJsonUrl")]
pub async fn tmoc_from_json_url(name: String, url: String) -> Result<(), JsValue> {
  const ERR: &str = "File content is not valid UTF-8.";
  from_url(
    name, url, "application/json",
    Box::new(|name, data| tmoc_from_json(name, from_utf8(data).unwrap_or(ERR)) )
  ).await
}

#[wasm_bindgen(js_name = "stmocFromJson", catch)]
pub fn stmoc_from_json(name: &str, data: &str) -> Result<(), JsValue> {
  let cell2 = cellmoc2d_from_json_aladin::<u64, Time::<u64>, u64, Hpx::<u64>>(data)
    .map_err(|e| JsValue::from_str(&e.to_string()))?;
  let moc2 = cell2.into_cell_moc2_iter().into_range_moc2_iter().into_range_moc2();
  store::add(name, InternalMoc::TimeSpace(moc2))
}

/// WARNING: if this i not working, check e.g. with `wget -v -S ${url}` the the content type is
/// `Content-Type: application/json`.
#[wasm_bindgen(js_name = "stmocFromJsonUrl")]
pub async fn stmoc_from_json_url(name: String, url: String) -> Result<(), JsValue> {
  const ERR: &str = "File content is not valid UTF-8.";
  from_url(
    name, url, "application/json",
    Box::new(|name, data| stmoc_from_json(name, from_utf8(data).unwrap_or(ERR)) )
  ).await
}

//////////////
// SAVE MOC //
// return a string or an array of u8?

#[wasm_bindgen(js_name = "toAscii")]
pub fn to_ascii(name: &str, fold: Option<usize>) -> JsValue {
  // from_str creates a copy :o/
  store::exec(name, move |moc| JsValue::from_str(&moc.to_ascii(fold)))
    .unwrap_or(JsValue::NULL)
}

// Instead of returning a String, we should probably return a map of (depth, array of indices) values :o/
#[wasm_bindgen(js_name = "toJson")]
pub fn to_json(name: &str, fold: Option<usize>) -> JsValue {
  store::exec(name, move |moc| JsValue::from_str(&moc.to_json(fold)))
    .unwrap_or(JsValue::NULL)
}

#[wasm_bindgen(js_name = "toFits")]
pub fn to_fits(name: &str) -> Option<Box<[u8]>> {
  store::exec(name, move |moc| moc.to_fits())
}

#[wasm_bindgen(js_name = "toAsciiFile", catch)]
pub fn to_ascii_file(name: &str, fold: Option<usize>) -> Result<(), JsValue> {
  let data: String = store::exec(name, move |moc| moc.to_ascii(fold))
    .ok_or_else(|| JsValue::from_str("MOC not found"))?;
  to_file(name, ".txt", "text/plain", data.into_bytes().into_boxed_slice())
}

#[wasm_bindgen(js_name = "toJsonFile", catch)]
pub fn to_json_file(name: &str, fold: Option<usize>) -> Result<(), JsValue> {
  let data: String = store::exec(name, move |moc| moc.to_json(fold))
    .ok_or_else(|| JsValue::from_str("MOC not found"))?;
  to_file(name, ".json", "application/json", data.into_bytes().into_boxed_slice())
}

#[wasm_bindgen(js_name = "toFitsFile", catch)]
pub fn to_fits_file(name: &str) -> Result<(), JsValue> {
  let data: Box<[u8]> = store::exec(name, move |moc| moc.to_fits())
    .ok_or_else(|| JsValue::from_str("MOC not found"))?;
  to_file(name,".fits", "application/fits", data)
}

/// # Params
/// * ext: `.fits` , `.ascii` or `.json`
/// * mime: `application/fits`, `text/plain` or `application/json`
/// * data: file content
fn to_file(
  name: &str,
  ext: &str, 
  mime: &str,
  data: Box<[u8]>
) -> Result<(), JsValue> {
  // Set filename
  let mut filename = String::from(name);
  if !filename.ends_with(ext) {
    filename.push_str(ext);
  }
  // Put data in a blob
  let data: Uint8Array = data.as_ref().into();
  let mut blob_prop = BlobPropertyBag::new();
  blob_prop.type_(mime);
  // let url_data = Array::new();
  // url_data.push(&data);
  let blob = Blob::new_with_u8_array_sequence_and_options(&data, &blob_prop)?;
  
  // Generate the URL with the attached data
  let url = Url::create_object_url_with_blob(&blob)?;

  // Create a temporary download link
  let window = web_sys::window().expect("no global `window` exists");
  let document = window.document().expect("should have a document on window");
  let body = document.body().expect("document should have a body");
  let anchor: HtmlAnchorElement = document.create_element("a").unwrap().dyn_into()?;
  anchor.set_href(&url);
  anchor.set_download(&filename);
  body.append_child(&anchor)?;
  // Simulate a click
  anchor.click();
  // Clean
  body.remove_child(&anchor)?;
  Url::revoke_object_url(&url)?;
  Ok(())
}


//////////////////
// MOC CREATION //
// array of f64 (positions) ?
// array of f64 (time, timerange)

#[wasm_bindgen(js_name = "fromCone", catch)]
pub fn from_cone(name: &str, depth: u8,  lon_deg: f64, lat_deg: f64, radius_deg: f64) ->  Result<(), JsValue> {
  let lon = lon_deg2rad(lon_deg)?;
  let lat = lat_deg2rad(lat_deg)?;
  let r = radius_deg.to_radians();
  if r <= 0.0 || PI <= r {
    Err(JsValue::from_str("Radius must be in ]0, pi["))
  } else {
    let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_cone(lon, lat, r, depth, 2);
    store::add(name, InternalMoc::Space(moc))
  }
}

#[wasm_bindgen(js_name = "fromRing", catch)]
pub fn from_ring(
  name: &str,
  depth: u8,
  lon_deg: f64,
  lat_deg: f64,
  internal_radius_deg: f64,
  external_radius_deg: f64
) ->  Result<(), JsValue> {
  let lon = lon_deg2rad(lon_deg)?;
  let lat = lat_deg2rad(lat_deg)?;
  let r_int = internal_radius_deg.to_radians();
  let r_ext = external_radius_deg.to_radians();
  if r_int <= 0.0 || PI <= r_int {
    Err(JsValue::from_str("Internal radius must be in ]0, pi["))
  } else if r_ext <= 0.0 || PI <= r_ext {
    Err(JsValue::from_str("External radius must be in ]0, pi["))
  } else if r_ext < r_int {
    Err(JsValue::from_str("External radius must be larger than the internal radius"))
  } else {
    let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_ring(lon, lat, r_int, r_ext, depth, 2);
    store::add(name, InternalMoc::Space(moc))
  }
}

#[wasm_bindgen(js_name = "fromEllipse", catch)]
pub fn from_elliptical_cone(
  name: &str, depth: u8,  
  lon_deg: f64, lat_deg: f64, 
  a_deg: f64, b_deg: f64, pa_deg: f64
) ->  Result<(), JsValue> {
  let lon = lon_deg2rad(lon_deg)?;
  let lat = lat_deg2rad(lat_deg)?;
  let a = a_deg.to_radians();
  let b = b_deg.to_radians();
  let pa = pa_deg.to_radians();
  if a <= 0.0 || HALF_PI <= a {
    Err(JsValue::from_str("Semi-major axis must be in ]0, pi/2]"))
  } else if b <= 0.0 || a <= b {
    Err(JsValue::from_str("Semi-minor axis must be in ]0, a["))
  } else if pa <= 0.0 || HALF_PI <= pa {
    Err(JsValue::from_str("Position angle must be in [0, pi["))
  } else {
    let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_elliptical_cone(lon, lat, a, b, pa, depth, 2);
    store::add(name, InternalMoc::Space(moc))
  }
}

#[wasm_bindgen(js_name = "fromZone", catch)]
pub fn from_zone(
  name: &str, depth: u8,
  lon_deg_min: f64, lat_deg_min: f64,
  lon_deg_max: f64, lat_deg_max: f64
) ->  Result<(), JsValue> {
  let lon_min = lon_deg2rad(lon_deg_min)?;
  let lat_min = lat_deg2rad(lat_deg_min)?;
  let lon_max = lon_deg2rad(lon_deg_max)?;
  let lat_max = lat_deg2rad(lat_deg_max)?;
  let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_zone(lon_min, lat_min, lon_max, lat_max, depth);
  store::add(name, InternalMoc::Space(moc))
}

#[wasm_bindgen(js_name = "fromBox", catch)]
pub fn from_box(
  name: &str, depth: u8,
  lon_deg: f64, lat_deg: f64,
  a_deg: f64, b_deg: f64, pa_deg: f64
) ->  Result<(), JsValue> {
  let lon = lon_deg2rad(lon_deg)?;
  let lat = lat_deg2rad(lat_deg)?;
  let a = a_deg.to_radians();
  let b = b_deg.to_radians();
  let pa = pa_deg.to_radians();
  if a <= 0.0 || HALF_PI <= a {
    Err(JsValue::from_str("Semi-major axis must be in ]0, pi/2]"))
  } else if b <= 0.0 || a <= b {
    Err(JsValue::from_str("Semi-minor axis must be in ]0, a["))
  } else if pa < 0.0 || PI <= pa {
    Err(JsValue::from_str("Position angle must be in [0, pi["))
  } else {
    let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_box(lon, lat, a, b, pa, depth);
    store::add(name, InternalMoc::Space(moc))
  }
}

/// Create a new MOC from the given polygon vertices.
/// # Params
/// * `name`: the name to be given to the MOC
/// * `depth`: MOC maximum depth in `[0, 29]`
/// * `vertices_deg`: vertices coordinates in degrees `[lon_v1, lat_v1, lon_v2, lat_v2, ..., lon_vn, lat_vn]` 
/// * `complement`: reverse the default inside/outside of the polygon
#[wasm_bindgen(js_name = "fromPolygon", catch)]
pub fn from_polygon(
  name: &str, depth: u8,
  vertices_deg: Box<[f64]>,
  complement: bool
) ->  Result<(), JsValue> {
  // An other solution would be to go unsafe to transmute in Box<[[f64; 2]]> ...
  let vertices = vertices_deg.iter().step_by(2).zip(vertices_deg.iter().skip(1).step_by(2))
    .map(|(lon_deg, lat_deg)| {
      let lon = lon_deg2rad(*lon_deg)?;
      let lat = lat_deg2rad(*lat_deg)?;
      Ok((lon, lat))
    }).collect::<Result<Vec<(f64, f64)>, JsValue>>()?;
  let moc: RangeMOC<u64, Hpx<u64>> = RangeMOC::from_polygon(&vertices, complement, depth);
  store::add(name, InternalMoc::Space(moc))
}

/// Create a new MOC from the given list of coordinates (assumed to be equatorial)
/// # Params
/// * `name`: the name to be given to the MOC
/// * `depth`: MOC maximum depth in `[0, 29]`
/// * `coos_deg`: list of coordinates in degrees `[lon_1, lat_1, lon_2, lat_2, ..., lon_n, lat_n]` 
#[wasm_bindgen(js_name = "fromCoo", catch)]
pub fn from_coo(
  name: &str, depth: u8,
  coos_deg: Box<[f64]>,
) ->  Result<(), JsValue> {
  // An other solution would be to go unsafe to transmute coos_deg in Box<[[f64; 2]]> ...
  let moc: RangeMOC<u64, Hpx<u64>> =  RangeMOC::from_coos(
    depth,
    coos_deg.iter().step_by(2).zip(coos_deg.iter().skip(1).step_by(2))
      .filter_map(|(lon_deg, lat_deg)| {
        let lon = lon_deg2rad(*lon_deg).ok();
        let lat = lat_deg2rad(*lat_deg).ok();
        match (lon, lat) {
          (Some(lon), Some(lat)) => Some((lon, lat)),
          _ => None,
        }
      }),
    None
  );
  store::add(name, InternalMoc::Space(moc))
}

/// Create a new T-MOC from the given list of decimal Julian Days (JD) times.
/// # Params
/// * `name`: the name to be given to the MOC
/// * `depth`: T-MOC maximum depth in `[0, 61]`
/// * `jd`: array of decimal JD time (`f64`)
/// # WARNING
/// Using decimal Julian Days stored on `f64`, the precision does not reach the microsecond
/// since JD=0.
/// In Javascript, there is no `u64` type (integers are stored on the mantissa of 
/// a double -- a `f64` --, which is made of 52 bits).
/// The other approach is to use a couple of `f64`: one for the integer part of the JD, the
/// other for the fractional part of the JD.
/// We will add such a method later if required by users.
#[wasm_bindgen(js_name = "fromDecimalJDs", catch)]
pub fn from_decimal_jd(name: &str, depth: u8, jd: Box<[f64]>) ->  Result<(), JsValue> {
  let moc = RangeMOC::<u64, Time<u64>>::from_microsec_since_jd0(
    depth, jd.iter().map(|jd| (jd * JD_TO_USEC) as u64), None
  );
  store::add(name, InternalMoc::Time(moc))
}

#[wasm_bindgen(js_name = "fromDecimalJDRanges", catch)]
pub fn from_decimal_jd_range(name: &str, depth: u8, jd_ranges: Box<[f64]>) ->  Result<(), JsValue> {
  let moc = RangeMOC::<u64, Time<u64>>::from_microsec_ranges_since_jd0(
    depth,
    jd_ranges.iter().step_by(2).zip(jd_ranges.iter().skip(1).step_by(2))
      .map(|(jd_min, jd_max)| (jd_min * JD_TO_USEC) as u64..(jd_max * JD_TO_USEC) as u64), 
    None
  );
  store::add(name, InternalMoc::Time(moc))
}

/// Create a new S-MOC from the given lists of UNIQ and Values.
/// # Params
/// * `name`: the name to be given to the MOC
/// * `depth`: S-MOC maximum depth in `[0, 29]`, Must be >= largest input cells depth.
/// * `density`: Input values are densities, i.e. they are not proportional to the area of their associated cells.
/// * `from_threshold`: Cumulative value at which we start putting cells in he MOC (often = 0).
/// * `to_threshold`: Cumulative value at which we stop putting cells in the MOC.
/// * `asc`: Compute cumulative value from ascending density values instead of descending (often = false).
/// * `not_strict`: Cells overlapping with the upper or the lower cumulative bounds are not rejected (often = false).
/// * `split`: Split recursively the cells overlapping the upper or the lower cumulative bounds (often = false).
/// * `revese_recursive_descent`: Perform the recursive descent from the highest to the lowest sub-cell, only with option 'split' (set both flags to be compatibile with Aladin)
/// * `uniqs`: array of uniq HEALPix cells
/// * `values`: array of values associated to the HEALPix cells
#[wasm_bindgen(js_name = "fromValuedCells", catch)]
pub fn from_valued_cells(
  name: &str,
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
) -> Result<(), JsValue> {
  let depth = depth.max(
    uniqs.iter()
      .map(|uniq| Hpx::<u64>::from_uniq_hpx(*uniq as u64).0)
      .max()
      .unwrap_or(depth)
  );
  let area_per_cell = (PI / 3.0) / (1_u64 << (depth << 1) as u32) as f64;  // = 4pi / (12*4^depth)
  let ranges: HpxRanges<u64> = if density {
    valued_cells_to_moc_with_opt::<u64, f64>(
      depth,
      uniqs.iter().zip(values.iter())
        .map(|(uniq, dens)| {
          let uniq = *uniq as u64;
          let (cdepth, _ipix) = Hpx::<u64>::from_uniq_hpx(uniq);
          let n_sub_cells = (1_u64 << (((depth - cdepth) << 1) as u32)) as f64;
          (uniq, dens * n_sub_cells * area_per_cell, *dens)
        }).collect(),
      from_threshold, to_threshold, asc, !not_strict, !split, revese_recursive_descent
    )
  } else {
    valued_cells_to_moc_with_opt::<u64, f64>(
      depth,
      uniqs.iter().zip(values.iter())
        .map(|(uniq, val)| {
          let uniq = *uniq as u64;
          let (cdepth, _ipix) = Hpx::<u64>::from_uniq_hpx(uniq);
          let n_sub_cells = (1_u64 << (((depth - cdepth) << 1) as u32)) as f64;
          (uniq, *val, val / (n_sub_cells * area_per_cell))
        }).collect(),
      from_threshold, to_threshold, asc, !not_strict, !split, revese_recursive_descent
    )
  };
  let moc = RangeMOC::new(depth, ranges);
  store::add(name, InternalMoc::Space(moc))
}

// BUILD ST-MOCs!!

/////////////////////////
// OPERATIONS ON 1 MOC //

// return a hierachical view (Json like) for display?
// (not necessary if display made from rust code too)

#[wasm_bindgen(catch)]
pub fn not(moc: &str, res_name: &str) -> Result<(), JsValue> {
  op1(moc, Op1::Complement, res_name)
}
#[wasm_bindgen(catch)]
pub fn complement(moc: &str, res_name: &str) -> Result<(), JsValue> {
  op1(moc, Op1::Complement, res_name)
}

/// Split the given disjoint S-MOC int joint S-MOCs.
/// Split "direct", i.e. we consider 2 neighboring cells to be the same only if the share an edge.
/// WARNING: may create a lot of new MOCs, exec `splitCount` first!!
#[wasm_bindgen(catch)]
pub fn split(moc: &str, res_name: &str) -> Result<(), JsValue> {
  op1(moc, Op1::Split, res_name)
}
/// Count the number of joint S-MOC splitting ("direct") the given disjoint S-MOC.
#[wasm_bindgen(js_name = "splitCount", catch)]
pub fn split_count(moc: &str) -> Result<u32, JsValue> {
  op1_count_split(moc, false)
}

/// Split the given disjoint S-MOC int joint S-MOCs.
/// Split "indirect", i.e. we consider 2 neighboring cells to be the same if the share an edge
/// or a vertex.
/// WARNING: may create a lot of new MOCs, exec `splitIndirectCount` first!!
#[wasm_bindgen(js_name = "splitIndirect", catch)]
pub fn split_indirect(moc: &str, res_name: &str) -> Result<(), JsValue> {
  op1(moc, Op1::SplitIndirect, res_name)
}
/// Count the number of joint S-MOC splitting ("direct") the given disjoint S-MOC.
#[wasm_bindgen(js_name = "splitIndirectCount", catch)]
pub fn split_indirect_count(moc: &str) -> Result<u32, JsValue> {
  op1_count_split(moc, true)
}


#[wasm_bindgen(catch)]
pub fn degrade(moc: &str, new_depth: u8, res_name: &str) -> Result<(), JsValue> {
  op1(moc, Op1::Degrade { new_depth }, res_name)
}

#[wasm_bindgen(catch)]
pub fn extend(moc: &str, res_name: &str) -> Result<(), JsValue> {
  op1(moc, Op1::Extend, res_name)
}

#[wasm_bindgen(catch)]
pub fn contract(moc: &str, res_name: &str) -> Result<(), JsValue> {
  op1(moc, Op1::Contract, res_name)
}

#[wasm_bindgen(js_name = "externalBorder", catch)]
pub fn ext_border(moc: &str, res_name: &str) -> Result<(), JsValue> {
  op1(moc, Op1::ExtBorder, res_name)
}

#[wasm_bindgen(js_name = "internalBorder",catch)]
pub fn int_border(moc: &str, res_name: &str) -> Result<(), JsValue> {
  op1(moc, Op1::IntBorder, res_name)
}


////////////////////////////////////////////////////
// LOGICAL OPERATIONS BETWEEN 2 MOCs of same type //

#[wasm_bindgen(catch)]
pub fn or(left_moc: &str, right_moc: &str, res_name: &str) -> Result<(), JsValue> {
  op2(left_moc, right_moc, Op2::Union, res_name)
}
#[wasm_bindgen(catch)]
pub fn union(left_moc: &str, right_moc: &str, res_name: &str) -> Result<(), JsValue> {
  op2(left_moc, right_moc, Op2::Union, res_name)
}

#[wasm_bindgen(catch)]
pub fn and(left_moc: &str, right_moc: &str, res_name: &str) -> Result<(), JsValue> {
  op2(left_moc, right_moc, Op2::Intersection, res_name)
}
#[wasm_bindgen(catch)]
pub fn intersection(left_moc: &str, right_moc: &str, res_name: &str) -> Result<(), JsValue> {
  op2(left_moc, right_moc, Op2::Intersection, res_name)
}

#[wasm_bindgen(catch)]
pub fn xor(left_moc: &str, right_moc: &str, res_name: &str) -> Result<(), JsValue> {
  op2(left_moc, right_moc, Op2::Difference, res_name)
}
#[wasm_bindgen(catch)]
pub fn difference(left_moc: &str, right_moc: &str, res_name: &str) -> Result<(), JsValue> {
  op2(left_moc, right_moc, Op2::Difference, res_name)
}

#[wasm_bindgen(catch)]
pub fn minus(left_moc: &str, right_moc: &str, res_name: &str) -> Result<(), JsValue> {
  op2(left_moc, right_moc, Op2::Minus, res_name)
}

////////////////////////
// ST-MOC projections //

/// Returns the union of the S-MOCs associated to T-MOCs intersecting the given T-MOC.
/// Left: T-MOC, right: ST-MOC, result: S-MOC.
#[wasm_bindgen(js_name = "timeFold", catch)]
pub fn time_fold(time_moc: &str, st_moc: &str, res_smoc_name: &str) -> Result<(), JsValue> {
  op2(time_moc, st_moc, Op2::TFold, res_smoc_name)
}

/// Returns the union of the T-MOCs associated to S-MOCs intersecting the given S-MOC. 
/// Left: S-MOC, right: ST-MOC, result: T-MOC.
#[wasm_bindgen(js_name = "spaceFold", catch)]
pub fn space_fold(space_moc: &str, st_moc: &str, res_tmoc_name: &str) -> Result<(), JsValue> {
  op2(space_moc, st_moc, Op2::SFold, res_tmoc_name)
}

//////////////////////////////////////////////////////
// Filter/Contains (returning an array of boolean?) //

/// Returns an array of boolean (u8 set to 1 or 0) telling if the pairs of coordinates
/// in the input array are in (true=1) or out of (false=0) the S-MOC of given name.
/// # Params
/// * `name`: the name of the S-MOC to be used for filtering
/// * `coos_deg`: list of coordinates in degrees `[lon_1, lat_1, lon_2, lat_2, ..., lon_n, lat_n]`
/// # Remarks
/// The size of the returned boolean (u8) array his half the size of the input array
/// (since the later contains pairs of coordinates).
#[wasm_bindgen(js_name = "filterCoos", catch)]
pub fn filter_pos(name: &str, coos_deg: Box<[f64]>) ->  Result<Box<[u8]>, JsValue> {
  let filter = move |moc: &InternalMoc| match moc {
    InternalMoc::Space(moc) => {
      let depth = moc.depth_max();
      let layer = healpix::nested::get(depth);
      let shift = Hpx::<u64>::shift_from_depth_max(depth) as u32;
      Ok(
        coos_deg.iter().step_by(2).zip(coos_deg.iter().skip(1).step_by(2))
          .map(|(lon_deg, lat_deg)| {
            let lon = lon_deg2rad(*lon_deg).ok();
            let lat = lat_deg2rad(*lat_deg).ok();
            match (lon, lat) {
              (Some(lon), Some(lat)) => {
                let icell = layer.hash(lon, lat) << shift;
                if moc.contains_val(&icell) {
                  1_u8
                } else {
                  0_u8
                }
              },
              _ => 0_u8,
            }
          })
          .collect::<Vec<u8>>()
          .into_boxed_slice()
      )
    },
    _ => Err(JsValue::from_str("Can't filter coos on a MOC different from a S-MOC")),
  };
  store::exec(name, filter).ok_or_else(|| JsValue::from_str("MOC not found")).and_then(|r| r)
}

/// Returns an array of boolean (u8 set to 1 or 0) telling if the time (in Julian Days)
/// in the input array are in (true=1) or out of (false=0) the T-MOC of given name.
/// # Params
/// * `name`: the name of the S-MOC to be used for filtering
/// * `jds`: array of decimal JD time (`f64`)
/// # Remarks
/// The size of the returned boolean (u8) array his the same as the size of the input array.
#[wasm_bindgen(js_name = "filterJDs", catch)]
pub fn filter_time(name: &str, jds: Box<[f64]>) ->  Result<Box<[u8]>, JsValue> {
  let filter = move |moc: &InternalMoc| match moc {
    InternalMoc::Time(moc) => {
      Ok(
        jds.iter()
          .map(|jd| {
            let usec = (jd * JD_TO_USEC) as u64;
            moc.contains_val(&usec) as u8
          })
          .collect::<Vec<u8>>()
          .into_boxed_slice()
      )
    },
    _ => Err(JsValue::from_str("Can't filter time on a MOC different from a T-MOC")),
  };
  store::exec(name, filter).ok_or_else(|| JsValue::from_str("MOC not found")).and_then(|r| r)
}

// add filter using st-moc ? Input: (lon, lat, jd)?

// Called when the wasm module is instantiated
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
  // Do nothing, be here commented example that build a web page (add download buttons, ...)
  /*
  // Use `web_sys`'s global `window` function to get a handle on the global
  // window object.
  let window = web_sys::window().expect("no global `window` exists");
  let document = window.document().expect("should have a document on window");
  let body = document.body().expect("document should have a body");

  // Manufacture the element we're gonna append
  let val = document.create_element("p")?;
  val.set_inner_html("<label>All MOCs from FITS:</label><button onclick=\"moc.fromLocalFile();\">Load local file</button>");
  body.append_child(&val)?;

  let val = document.create_element("p")?;
  val.set_inner_html("<label>S-MOC from FITS/JSON/ASCII:</label><button onclick=\"moc.fromLocalFile('space');\">Load local file</button>");
  body.append_child(&val)?;
  
  let val = document.create_element("p")?;
  val.set_inner_html("<label>T-MOC from FITS/JSON/ASCII:</label><button onclick=\"moc.fromLocalFile('time');\">Load local file</button>");
  body.append_child(&val)?;
  
  let val = document.create_element("p")?;
  val.set_inner_html("<label>ST-MOC from FITS/JSON/ASCII:</label><button onclick=\"moc.fromLocalFile('space-time');\">Load local file</button>");
  body.append_child(&val)?;
  */
  
  /*
  // Create this part of the HTML document:
  //   <label for="inputFileMoc">Add MOC from file: </label>
  //   <input type="file" id="inputFileMoc" name="inputFileMocSelect" accept=".fits, .json, .ascii, .txt"></input>
  let input_label_content = "Add MOC from file:";
  let input_id = "inputFileMoc";
  let input_name = "mocFileSelect";
  let moc_list_label_content = "Loaded MOCs: ";
  let moc_list_id = "listMOC";
  // - create label
  let mut file_input_label = document.create_element("label")?;
  file_input_label.set_attribute("for", input_id);
  file_input_label.set_inner_html(input_label_content);
  // - create input
  let file_input: HtmlInputElement = document.create_element("input").unwrap().dyn_into()?;
  file_input.set_type("file");
  file_input.set_attribute("id", input_id);
  file_input.set_attribute("name", input_name);
  file_input.set_attribute("accept", ".fits, .json, .ascii, .txt");

  let file_input_action = Closure::wrap(Box::new(move |event: Event| {
    let element = event.target().unwrap().dyn_into::<HtmlInputElement>().unwrap();
    let filelist = element.files().unwrap();
    let file = filelist.get(0).unwrap();
    //log(&file.name());
    //log(&format!("{:?}", file));
    // There is a stream method, not sure how to use it
    let file_reader = FileReader::new().unwrap();
    file_reader.read_as_array_buffer(&file);


    let mut file_onload = Closure::wrap(Box::new(move |event: Event| {
      let file_reader: FileReader = event.target().unwrap().dyn_into().unwrap();
      // let psd = file_reader.result().unwrap();
      // let psd = js_sys::Uint8Array::new(&psd);
      let file_content = file_reader.result().unwrap();
      let file_content: Vec<u8> = js_sys::Uint8Array::new(&file_content).to_vec();
      // log(&format!("File len {:?}", file_content.len()));

      from_fits(&file.name(), &file_content);
      let store = store::get_store().read().unwrap();
      for (key, val) in store.iter() {
        log(&format!("id: {}; type: {}", key, val.get_qtype()));
      }
      //let mut psd_file = vec![0; psd.length() as usize];
      //psd.copy_to(&mut psd_file);

      //store.borrow_mut().msg(&Msg::ReplacePsd(&psd_file));
    }) as Box<dyn FnMut(_)>);
    file_reader.set_onload(Some(file_onload.as_ref().unchecked_ref()));
    file_onload.forget();

    //let file_content = file_reader.result().unwrap();
    //let file_content: Vec<u8> = js_sys::Uint8Array::new(&file_content).to_vec();
    //log(&format!("File len {:?}", file_content.len()));
    // filereader.read_as_text(&file).unwrap();
    //log(filelist.length().to_string().as_str());
  }) as Box<dyn FnMut(_)>);
  file_input.add_event_listener_with_callback("change", file_input_action.as_ref().unchecked_ref())?;
  file_input_action.forget();
  // - append to the HTML document
  body.append_child(&file_input_label)?;
  body.append_child(&file_input)?;
  body.append_child(&document.create_element("p")?.into())?;

  // Create this part of the HTML document:
  //   <label for="fileList">Loaded MOCs: </label>
  //   <ul id="fileList"> </ul>
  // - create label
  let mut moc_list_label = document.create_element("label")?;
  moc_list_label.set_attribute("for", moc_list_id);
  moc_list_label.set_inner_html(moc_list_label_content);
  // - create list
  let mut moc_list: HtmlUListElement = document.create_element("ul").unwrap().dyn_into()?;
  moc_list.set_attribute("id", moc_list_id);
  // - append to the HTML document
  body.append_child(&moc_list_label)?;
  body.append_child(&moc_list)?;
  */
  Ok(())
}
