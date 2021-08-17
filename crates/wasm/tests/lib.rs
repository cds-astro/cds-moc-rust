#![cfg(target_arch = "wasm32")]

extern crate wasm_bindgen_test;
use moc_wasm::*;
use wasm_bindgen_test::*;

use wasm_bindgen::JsValue;


#[wasm_bindgen_test]
fn cone() {
  let name = "cone";  
  from_cone(name, 4,  0.0, 0.0, 5.0).unwrap();
  assert_eq!(to_ascii(name, None), JsValue::from_str("3/271 304 4/1128 1130-1131 1172-1173 1175 "));
}
