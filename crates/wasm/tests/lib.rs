use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use moc_wasm::smoc::MOC;

#[wasm_bindgen_test]
fn cone() {
  let moc = MOC::from_cone(4, 0.0, 0.0, 5.0, None).unwrap();
  assert_eq!(
    moc.to_ascii(None),
    Ok(JsValue::from_str(
      "3/271 304 4/1128 1130-1131 1172-1173 1175 "
    ))
  );
}
