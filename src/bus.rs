#![allow(non_snake_case)]
use crossbeam_queue::SegQueue;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

type BusId = usize;

static mut BUSES: Lazy<Vec<BusImpl>> = Lazy::new(|| Vec::new());
static mut EMITTERS: Lazy<HashMap<String, EventEmitterInternal>> = Lazy::new(|| HashMap::new());

pub unsafe fn sab_bus_diag() {
    web_sys::console::log_1(&format!("[WASABIO:BUS] BUSES: {:?}", BUSES).into());
    web_sys::console::log_1(&format!("[WASABIO:BUS] EMITTERS: {:?}", EMITTERS).into());
}

pub unsafe fn sab_bus_reboot() {
    EMITTERS = Lazy::new(|| HashMap::new());
    BUSES = Lazy::new(|| Vec::new());
}

#[derive(Debug)]
struct BusImpl {
    q: SegQueue<String>,
    id: BusId,
    held: bool,
}

impl Default for BusImpl {
    fn default() -> Self {
        Self {
            q: SegQueue::new(),
            held: true,
            id: 0,
        }
    }
}

#[wasm_bindgen]
/// Allocates a new event bus and returns the channel. Recycles if possible.
pub unsafe fn sab_bus_new() -> Option<BusId> {
    let mut channel = BUSES
        .iter()
        .enumerate()
        .find_map(|(i, c)| if c.held { None } else { Some(i) });
    if let Some(channel) = channel {
        BUSES[channel].held = true;
    } else {
        let id = BUSES.len();
        let mut pipe = BusImpl::default();
        pipe.id = id;
        pipe.held = true;
        BUSES.push(pipe);
        channel = Some(id);
    }
    channel
}

#[wasm_bindgen]
/// Closes the event bus for the given id (discards all pending events).
pub unsafe fn sab_bus_free(id: BusId) {
    BUSES.get_mut(id).and_then(|c| {
        c.held = false;
        Some(())
    });
}

#[wasm_bindgen]
/// Broadcasts an event to all other enabled channels except the one specified.
pub unsafe fn sab_bus_broadcast(from: BusId, value: String) {
    BUSES.iter().enumerate().for_each(|(_, c)| {
        if c.id != from {
            sab_bus_send(c.id, value.clone());
        }
    });
}

#[wasm_bindgen]
/// Sends an event to the specified channel directly.
pub unsafe fn sab_bus_send(to: BusId, value: String) {
    BUSES.get(to).and_then(|c| Some(c.q.push(value)));
}

#[wasm_bindgen]
/// Sends an event to all enabled channels.
pub unsafe fn sab_bus_yeet(value: String) {
    BUSES.iter().enumerate().for_each(|(_, c)| {
        sab_bus_send(c.id, value.clone());
    });
}

#[wasm_bindgen]
/// Receives an event from the specified channel, or None if none are pending.
pub unsafe fn sab_bus_receive(channel: BusId) -> String {
    BUSES
        .get(channel)
        .and_then(|c| c.q.pop())
        .unwrap_or("".to_string())
}

/// Convenience wrapper around the wasabio bus API.
pub struct Bus(pub BusId);

impl Bus {
    /// Allocates a new event bus and returns the channel. Recycles if possible.
    pub fn new() -> Self {
        Self(unsafe { sab_bus_new().unwrap() })
    }
    /// Broadcasts an event to all other enabled channels except the one specified.
    pub fn broadcast(&self, value: String) {
        unsafe { sab_bus_broadcast(self.0, value) }
    }
    /// Sends an event to the specified channel directly.
    pub fn send(&self, value: String) {
        unsafe { sab_bus_send(self.0, value) }
    }
    /// Sends an event to all enabled channels.
    pub fn yeet(&self, value: String) {
        unsafe { sab_bus_yeet(value) }
    }
    /// Receives an event from the specified channel, or None if none are pending.
    pub fn receive(&self) -> String {
        unsafe { sab_bus_receive(self.0) }
    }
    /// Closes the event bus for the given id (discards all pending events).
    pub fn dispose(&self) {
        while self.receive() != "" {}
        unsafe { sab_bus_free(self.0) }
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        self.dispose();
    }
}

#[derive(Debug)]
struct EventEmitterInternal {
    slots: HashMap<String, Vec<(bool, BusId)>>,
    limit: usize,
}

impl EventEmitterInternal {
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            limit: 0,
        }
    }
    pub fn addListener(&mut self, event: String, channel: BusId) -> Result<(), JsError> {
        self.on(event, channel)
    }
    pub fn on(&mut self, event: String, channel: BusId) -> Result<(), JsError> {
        if let Some(ch) = self.slots.get(&event) {
            if self.limit > 0 && ch.len() >= self.limit {
                return Err(JsError::new("Too many listeners"));
            }
        }
        if let Some(ch) = self.slots.get_mut(&event) {
            ch.push((false, channel));
        } else {
            self.slots.insert(event, vec![(false, channel)]);
        }
        Ok(())
    }
    pub fn once(&mut self, event: String, channel: BusId) -> Result<(), JsError> {
        if let Some(ch) = self.slots.get(&event) {
            if self.limit > 0 && ch.len() >= self.limit {
                return Err(JsError::new("Too many listeners"));
            }
        }
        if let Some(ch) = self.slots.get_mut(&event) {
            ch.push((true, channel));
        } else {
            self.slots.insert(event, vec![(true, channel)]);
        }
        Ok(())
    }
    pub fn removeListener(&mut self, event: String, channel: BusId) -> Result<(), JsError> {
        self.off(event, channel)
    }
    pub fn off(&mut self, event: String, channel: BusId) -> Result<(), JsError> {
        if let Some(ch) = self.slots.get_mut(&event) {
            ch.retain(|(_, c)| *c != channel);
        }
        Ok(())
    }
    pub fn removeAllListeners(&mut self, event: Option<String>) -> Result<(), JsError> {
        if let Some(event) = event {
            self.slots.remove(&event);
        } else {
            self.slots.clear();
        }
        Ok(())
    }
    pub fn setMaxListeners(&mut self, limit: usize) -> Result<(), JsError> {
        self.limit = limit;
        Ok(())
    }
    pub fn getMaxListeners(&mut self) -> Result<usize, JsError> {
        Ok(self.limit)
    }
    pub fn listeners(&mut self, event: String) -> Result<Vec<BusId>, JsError> {
        Ok(if let Some(ch) = self.slots.get(&event) {
            ch.iter().map(|(_, c)| *c).collect()
        } else {
            vec![]
        })
    }
    pub fn rawListeners(&mut self, event: String) -> Result<Vec<BusId>, JsError> {
        self.listeners(event)
    }
    pub fn emit(&mut self, event: String, value: String) -> Result<(), JsError> {
        if let Some(ch) = self.slots.get_mut(&event) {
            ch.retain(|(once, c)| {
                unsafe { sab_bus_send(*c, value.clone()) };
                !*once
            });
        }
        Ok(())
    }
    pub fn listenerCount(&mut self, event: String) -> Result<usize, JsError> {
        Ok(if let Some(ch) = self.slots.get(&event) {
            ch.len()
        } else {
            0
        })
    }
    pub fn prependListener(&mut self, event: String, channel: BusId) -> Result<(), JsError> {
        if let Some(ch) = self.slots.get_mut(&event) {
            ch.insert(0, (false, channel));
        } else {
            self.slots.insert(event, vec![(false, channel)]);
        }
        Ok(())
    }
    pub fn prependOnceListener(&mut self, event: String, channel: BusId) -> Result<(), JsError> {
        if let Some(ch) = self.slots.get_mut(&event) {
            ch.insert(0, (true, channel));
        } else {
            self.slots.insert(event, vec![(true, channel)]);
        }
        Ok(())
    }
    pub fn eventNames(&mut self) -> Result<js_sys::Array, JsError> {
        let arr = js_sys::Array::new();
        self.slots.keys().for_each(|k| {
            arr.push(&JsValue::from_str(k));
        });
        Ok(arr)
    }
}

fn ensureEmitterExists(name: &str) -> &'static mut EventEmitterInternal {
    unsafe {
        if !EMITTERS.contains_key(name) {
            EMITTERS.insert(name.to_string(), EventEmitterInternal::new());
        }
        EMITTERS.get_mut(name).unwrap()
    }
}

#[wasm_bindgen]
pub struct EventEmitter(String);

#[wasm_bindgen]
impl EventEmitter {
    fn emitter(&self) -> Result<&'static mut EventEmitterInternal, JsError> {
        Ok(ensureEmitterExists(&self.0))
    }
    #[wasm_bindgen(constructor)]
    pub fn new(name: &str) -> Self {
        ensureEmitterExists(name);
        Self(name.to_string())
    }
    pub fn addListener(&self, event: String, channel: BusId) -> Result<(), JsError> {
        self.emitter()?.addListener(event, channel)
    }
    pub fn on(&self, event: String, channel: BusId) -> Result<(), JsError> {
        self.emitter()?.on(event, channel)
    }
    pub fn once(&self, event: String, channel: BusId) -> Result<(), JsError> {
        self.emitter()?.once(event, channel)
    }
    pub fn removeListener(&self, event: String, channel: BusId) -> Result<(), JsError> {
        self.emitter()?.removeListener(event, channel)
    }
    pub fn off(&self, event: String, channel: BusId) -> Result<(), JsError> {
        self.emitter()?.off(event, channel)
    }
    pub fn removeAllListeners(&self, event: Option<String>) -> Result<(), JsError> {
        self.emitter()?.removeAllListeners(event)
    }
    pub fn setMaxListeners(&self, limit: usize) -> Result<(), JsError> {
        self.emitter()?.setMaxListeners(limit)
    }
    pub fn getMaxListeners(&self) -> Result<usize, JsError> {
        self.emitter()?.getMaxListeners()
    }
    pub fn listeners(&self, event: String) -> Result<Vec<BusId>, JsError> {
        self.emitter()?.listeners(event)
    }
    pub fn rawListeners(&self, event: String) -> Result<Vec<BusId>, JsError> {
        self.emitter()?.rawListeners(event)
    }
    pub fn emit(&self, event: String, value: String) -> Result<(), JsError> {
        self.emitter()?.emit(event, value)
    }
    pub fn listenerCount(&self, event: String) -> Result<usize, JsError> {
        self.emitter()?.listenerCount(event)
    }
    pub fn prependListener(&self, event: String, channel: BusId) -> Result<(), JsError> {
        self.emitter()?.prependListener(event, channel)
    }
    pub fn prependOnceListener(&self, event: String, channel: BusId) -> Result<(), JsError> {
        self.emitter()?.prependOnceListener(event, channel)
    }
    pub fn eventNames(&self) -> Result<js_sys::Array, JsError> {
        self.emitter()?.eventNames()
    }
}
