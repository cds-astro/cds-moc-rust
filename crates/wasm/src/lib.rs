extern crate console_error_panic_hook;

use std::{
  panic,
  str::from_utf8_unchecked
};

use unreachable::{UncheckedOptionExt, UncheckedResultExt};

use serde::{Serialize, Deserialize};

use wasm_bindgen::{
  prelude::*,
  JsCast
};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  Url, Blob, BlobPropertyBag,
  Event, FileReader,
  HtmlAnchorElement, HtmlInputElement,
  Request, RequestInit, RequestMode, Response
};
use js_sys::{Array, Uint8Array};

use moclib::storage::u64idx::U64MocStore;

pub mod smoc;
pub mod tmoc;
pub mod fmoc;
pub mod stmoc;

use smoc::MOC;

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

/////////////////////////
// Common declarations //

#[wasm_bindgen]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum MocQType {
  Space = "space",
  Time = "time",
  Frequency = "frequence",
  SpaceTime = "space-time",
}

/// Common operations on all MOCs
trait IsMOC: Sized {
  fn from_store_index(store_index: usize) -> Self;
  fn storage_index(&self) -> usize;
  fn get_type(&self) -> MocQType;

  fn add_to_store(name: &str, moc: Self) -> Result<(), JsValue>;


  // - from

  fn from_ascii(data: &str) -> Result<Self, JsValue>;
  
  // /// async func not supported in trait...
  // async fn from_ascii_url(url: String) -> Result<Self, JsValue> 

  fn from_json(data: &str) -> Result<Self, JsValue>;

  // /// async func not supported in trait...
  // async fn smoc_from_json_url(url: String) -> Result<Self, JsValue>

  fn from_fits(data: &[u8]) -> Result<Self, JsValue>;

  // /// async func not supported in trait...
  // async fn from_fits_url(url: String, accept_mime_types: Option<String>) -> Result<(), JsValue>;

  /// Trigger a file dialog event and load the selected MOCs in a local storage.
  fn from_local_file() -> Result<(), JsValue> {
    from_local_files::<Self>()
  }
  
  // - to

  /// Returns the ASCII serialization of the given MOC.
  ///
  /// # Arguments
  /// * `fold`: fold option to limit the width of the string
  fn to_ascii(&self, fold: Option<usize>) -> Result<JsValue, JsValue> {
    // from_str creates a copy :o/
    U64MocStore::get_global_store()
      .to_ascii_str(self.storage_index(), fold)
      .map(|s| JsValue::from_str(&s))
      .map_err(|e| e.into())
  }

  /// Returns the JSON serialization of the given MOC.
  ///
  /// # Arguments
  /// * `fold`: fold option to limit the width of the string
  // Instead of returning a String, we should probably return a map of (depth, array of indices) values :o/
  fn to_json(&self, fold: Option<usize>) -> Result<JsValue, JsValue> {
    U64MocStore::get_global_store()
      .to_json_str(self.storage_index(), fold)
      .map(|s| JsValue::from_str(&s))
      .map_err(|e| e.into())
  }

  /// Returns in memory the FITS serialization of the MOC of given `name`.
  ///
  /// # Arguments
  /// * `force_v1_compatibility`: for S-MOCs, force compatibility with Version 1 of the MOC standard.
  fn to_fits(&self, force_v1_compatibility: Option<bool>) -> Result<Box<[u8]>, JsValue> {
    U64MocStore::get_global_store()
      .to_fits_buff(self.storage_index(), force_v1_compatibility)
      .map_err(|e| e.into())
  }

  // - to_file

  /// Download the ASCII serialization of the given MOC.
  ///
  /// # Arguments
  /// * `fold`: fold option to limit the width of the string
  fn to_ascii_file(&self, fold: Option<usize>) -> Result<(), JsValue> {
    U64MocStore::get_global_store()
      .to_ascii_str(self.storage_index(), fold)
      .map_err(|e| e.into())
      .and_then(|data| to_file("moc", ".txt", "text/plain", data.into_bytes().into_boxed_slice()))
  }

  /// Download the JSON serialization of the given MOC.
  ///
  /// # Arguments
  /// * `fold`: fold option to limit the width of the strin
  fn to_json_file(&self, fold: Option<usize>) -> Result<(), JsValue> {
    U64MocStore::get_global_store()
      .to_json_str(self.storage_index(), fold)
      .map_err(|e| e.into())
      .and_then(|data| to_file("moc", ".json", "application/json", data.into_bytes().into_boxed_slice()))
  }

  /// Download the FITS serialization of the MOC of given `name`.
  /// # Arguments
  /// * `force_v1_compatibility`: for S-MOCs, force compatibility with Version 1 of the MOC standard.
  fn to_fits_file(&self, force_v1_compatibility: Option<bool>) -> Result<(), JsValue> {
    U64MocStore::get_global_store()
      .to_fits_buff(self.storage_index(), force_v1_compatibility)
      .map_err(|e| e.into())
      .and_then(|data| to_file("moc",".fits", "application/fits", data))
  }

}

/// Common operations on 1D MOCs
trait IsOneDimMOC: IsMOC {
  
  fn depth(&self) -> Result<u8, JsValue>;

  fn coverage_percentage(&self) -> Result<f64, JsValue>  {
    U64MocStore::get_global_store()
      .get_coverage_percentage(self.storage_index())
      .map_err(|e| e.into())
  }

  fn n_ranges(&self) -> Result<u32, JsValue> {
    U64MocStore::get_global_store()
      .get_n_ranges(self.storage_index())
      .map_err(|e| e.into())
  }

  fn not(&self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .not(self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn complement(&self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .complement(self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn degrade(&self, new_depth: u8) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .degrade(self.storage_index(), new_depth)
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn or(&self, rhs: &Self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .or(rhs.storage_index(), self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }
  fn union(&self, rhs: &Self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .union(rhs.storage_index(), self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn and(&self, rhs: &Self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .and(rhs.storage_index(), self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }
  fn intersection(&self, rhs: &Self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .intersection(rhs.storage_index(), self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn xor(&self, rhs: &Self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .xor(rhs.storage_index(), self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }
  fn symmetric_difference(&self, rhs: &Self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .symmetric_difference(rhs.storage_index(), self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

  fn minus(&self, rhs: &Self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .minus(rhs.storage_index(), self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }
  fn difference(&self, rhs: &Self) -> Result<Self, JsValue> {
    U64MocStore::get_global_store()
      .difference(rhs.storage_index(), self.storage_index())
      .map(Self::from_store_index)
      .map_err(|e| e.into())
  }

}


//////////////
// LOAD MOC //

// Replacing monomorphisation by a trait object, we save ~10 kB on the final wasm file.
// Monomorphisation:
//   async fn from_url<F>(name: String, url: String, mime: &'static str, parse: F) -> Result<(), JsValue>
//    where
//      F: Fn(&str, &[u8]) ->  Result<(), JsValue>
// Trait object: Box<dyn ...>
async fn from_url<T>(
  url: String,
  mime: &str,
  parse: Box<dyn Fn(&[u8]) ->  Result<T, JsValue>>
) -> Result<T, JsValue>
{
  // https://rustwasm.github.io/docs/wasm-bindgen/examples/fetch.html
  let mut opts = RequestInit::new();
  opts.method("GET");
  opts.mode(RequestMode::Cors);
  
  let window = web_sys::window().ok_or_else(|| JsValue::from_str("Unable to get the Window"))?;
  
  let request = Request::new_with_str_and_init(&url, &opts)?;
  request.headers().set("Accept", mime)?;
  
  let document = window.document().ok_or_else(|| JsValue::from_str("Unable to get the Windows Document"))?;
  request.headers().set("Referer", &document.referrer())?; // For CORS

  let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

  let resp: Response = resp_value.dyn_into().map_err(|_| JsValue::from_str("Error casting to Response"))?;
  // Convert this other `Promise` into a rust `Future`.
  let buff = JsFuture::from(resp.array_buffer()?).await?;
  // log(&format!("Blob: {:?}", &blob));
  let file_content: Vec<u8> = js_sys::Uint8Array::new(&buff).to_vec();
  // log(&format!("Byte size: {}", file_content.len()));
  parse(&file_content)
}


/// Open the file selection dialog and load the MOC contained in the selected file 
/// (for security reasons, we cannot simply provide a path on the client machine).
/// # Warning
/// Because of security restriction, the call to this method
/// **"needs to be triggered within a code block that was the handler of a user-initiated event"**
pub(crate) fn from_local_files<T: IsMOC>() -> Result<(), JsValue> {
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
          "fits" | "gz" => T::from_fits(&file_content),
          "json" => T::from_json(unsafe{ from_utf8_unchecked(&file_content) }),
          "txt" | "ascii" => T::from_ascii(unsafe{ from_utf8_unchecked(&file_content) }),
          _ => unreachable!(), // since file_input.set_attribute("accept", ".fits, .json, .ascii, .txt");
        }.and_then(|moc| T::add_to_store(name, moc));
        // Handle here the error
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
pub(crate) fn from_local_multiordermap(
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
          "fits" | "gz" => MOC::from_multiordermap_fits_file(
            &file_content, from_threshold, to_threshold, 
            asc, not_strict, split, revese_recursive_descent
          ),
          _ => unreachable!(), // since file_input.set_attribute("accept", ".fits");
        }.and_then(|moc| MOC::add_to_store(name, moc));
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

/// Open the file selection dialog and load the skymap fits file 
/// (for security reasons, we cannot simply provide a path on the client machine).
/// # Warning
/// Because of security restriction, the call to this method
/// **"needs to be triggered within a code block that was the handler of a user-initiated event"**
pub(crate) fn from_local_skymap(
  skip_values_le: f64,
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
          "fits" | "gz" => MOC::from_skymap_fits_file(
            &file_content, skip_values_le, from_threshold, to_threshold,
            asc, not_strict, split, revese_recursive_descent
          ),
          _ => unreachable!(), // since file_input.set_attribute("accept", ".fits");
        }.and_then(|moc| MOC::add_to_store(name, moc));
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


/// Create a temporary link to trigger the download of the given content.
///
/// # Arguments
/// * name: name of the file
/// * ext: `.fits` , `.ascii` or `.json`
/// * mime: `application/fits`, `text/plain`, `application/json`, ...
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
  let bytes = Array::new();
  bytes.push(&data);
  let mut blob_prop = BlobPropertyBag::new();
  blob_prop.type_(mime);

  let blob = Blob::new_with_u8_array_sequence_and_options(&bytes, &blob_prop)?;
  
  // Generate the URL with the attached data
  let url = Url::create_object_url_with_blob(&blob)?;

  // Create a temporary download link
  let window = web_sys::window().expect("No global `window` exists");
  let document = window.document().expect("Should have a document on window");
  let body = document.body().expect("Document should have a body");
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

// Called when the wasm module is instantiated
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
  // Do nothing (could be removed)
  Ok(())
}
