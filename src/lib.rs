#![feature(mutex_unpoison)]
use once_cell::sync::Lazy;
use std::collections::HashMap;
use wasm_bindgen::__rt::__wbindgen_malloc;
use wasm_bindgen::prelude::*;

#[allow(unused_unsafe)]
pub mod bus;
pub mod fs;
pub mod kv;
pub mod lock;

#[wasm_bindgen]
/// Prints a diagnostic message about the WASABIO internal global state.
pub unsafe fn wasabio_diag() {
    lock::sab_lock_diag();
    bus::sab_bus_diag();
    fs::sab_fs_diag();
    kv::sab_kv_diag();
    web_sys::console::log_1(
        &format!("[WASABIO:TLS] COUNTER_ADDRESS: {:?}", COUNTER_ADDRESS).into(),
    );
    for (meta, data) in TLS_ALLOCATIONS.iter() {
        web_sys::console::log_1(
            &format!("[WASABIO:TLS] TLS_ALLOCATIONS: {:?} {:?}", meta, data).into(),
        );
    }
}

#[wasm_bindgen]
/// Reboots the WASABIO internal global state. This operation is not thread-safe.
pub unsafe fn wasabio_reboot() {
    lock::sab_lock_reboot();
    bus::sab_bus_reboot();
    fs::sab_fs_reboot();
    kv::sab_kv_reboot();
}

#[wasm_bindgen]
pub unsafe fn wasabio_locked() -> bool {
    fs::sab_fs_locked()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AllocationMeta {
    pub id: i32,
    pub kind: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AllocationData {
    pub base: usize,
    pub size: usize,
    pub align: usize,
}

static mut TLS_ALLOCATIONS: Lazy<HashMap<AllocationMeta, AllocationData>> =
    Lazy::new(|| HashMap::new());
static mut COUNTER_ADDRESS: Option<i32> = None;

fn get_thread_count() -> i32 {
    (unsafe { *(COUNTER_ADDRESS.unwrap() as *mut u8) }) as i32
}

#[no_mangle]
pub unsafe extern "C" fn __wbindgen_tls_malloc(
    size: usize,
    align: usize,
    thread_counter_addr: i32,
    kind: i32,
) -> *mut u8 {
    if COUNTER_ADDRESS.is_none() {
        COUNTER_ADDRESS = Some(thread_counter_addr);
    } else {
        assert!(COUNTER_ADDRESS.unwrap() == thread_counter_addr);
    }
    let id = get_thread_count();
    if let Some((_, data)) = TLS_ALLOCATIONS.iter().find(|(meta, data)| {
        meta.id == id && meta.kind == kind && data.size == size && data.align == align
    }) {
        let mut ptr = data.base;
        let end = data.base + data.size;
        while ptr < end {
            *(ptr as *mut u8) = 0;
            ptr += 1;
        }
        return data.base as *mut u8;
    } else {
        let base = __wbindgen_malloc(size, align) as usize;
        let data = AllocationData { base, size, align };
        let meta = AllocationMeta { id, kind };
        TLS_ALLOCATIONS.insert(meta, data);
        return base as *mut u8;
    }
}
