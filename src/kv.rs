#![allow(non_snake_case)]

use crate::bus::EventEmitter;
use once_cell::sync::Lazy;
use serde_json::json;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

static mut STORE: Lazy<HashMap<String, String>> = Lazy::new(|| HashMap::new());
static mut EMITTER: Lazy<EventEmitter> = Lazy::new(|| EventEmitter::new("kv"));

#[wasm_bindgen]
pub unsafe fn sab_kv_diag() {
    web_sys::console::log_1(&format!("[WASABIO:KV] STORE: {:?}", STORE).into());
}

#[wasm_bindgen]
pub unsafe fn sab_kv_reboot() {
    EMITTER = Lazy::new(|| EventEmitter::new("kv"));
}

#[wasm_bindgen]
pub unsafe fn sab_kv_get(key: &str) -> JsValue {
    if let Some(out) = STORE.get(key) {
        return JsValue::from_str(&out);
    } else {
        JsValue::NULL
    }
}

#[wasm_bindgen]
pub unsafe fn sab_kv_set(key: &str, value: &str) -> JsValue {
    let old = STORE.insert(key.into(), value.into());
    if old.is_none() {
        let ev = json!({
            "key": key,
            "newValue": value,
        });
        let _ = EMITTER.emit("set".into(), ev.to_string());
    } else {
        let ev = json!({
            "key": key,
            "oldValue": old,
            "newValue": value,
        });
        let _ = EMITTER.emit("set".into(), ev.to_string());
    }
    JsValue::UNDEFINED
}

#[wasm_bindgen]
pub unsafe fn sab_kv_del(key: &str) -> JsValue {
    let old = STORE.remove(key);
    let ev = json!({
        "key": key,
        "oldValue": old,
    });
    let _ = EMITTER.emit("del".into(), ev.to_string());
    JsValue::UNDEFINED
}

#[wasm_bindgen]
pub unsafe fn sab_kv_key(index: usize) -> JsValue {
    if let Some((key, _)) = STORE.iter().nth(index) {
        return JsValue::from_str(&key);
    }
    JsValue::NULL
}

#[wasm_bindgen]
pub unsafe fn sab_kv_clear() -> JsValue {
    STORE.clear();
    JsValue::UNDEFINED
}

#[wasm_bindgen]
pub unsafe fn sab_kv_length() -> usize {
    STORE.len()
}
