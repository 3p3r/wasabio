use once_cell::sync::Lazy;
use std::hint;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

const ENABLE_DEADLOCK_DETECTION: bool = false;
const MAX_SPINS: usize = 1000000;

#[derive(Debug)]
struct AtomicLock {
    resource: Arc<AtomicUsize>,
    held: bool,
    id: usize,
}

impl AtomicLock {
    pub fn unpoison(&mut self) {
        self.resource.store(0, Ordering::SeqCst);
    }
    pub fn poison(&mut self) {
        self.resource.store(1, Ordering::SeqCst);
    }
    pub fn poisoned(&self) -> bool {
        self.resource.load(Ordering::SeqCst) != 0
    }
    pub fn block(&mut self) {
        let mut attempts = 0;
        while self.poisoned() {
            attempts += 1;
            hint::spin_loop();
            if ENABLE_DEADLOCK_DETECTION && attempts > MAX_SPINS {
                panic!("Deadlock detected!")
            }
        }
    }
}

impl Default for AtomicLock {
    fn default() -> Self {
        Self {
            resource: Arc::new(AtomicUsize::new(0)),
            held: true,
            id: 0,
        }
    }
}

type LockId = usize;

static mut LOCKS: Lazy<Vec<AtomicLock>> = Lazy::new(|| Vec::new());

pub unsafe fn sab_lock_diag() {
    web_sys::console::log_1(&format!("[WASABIO:LOCK] LOCKS: {:?}", LOCKS).into());
}

pub unsafe fn sab_lock_reboot() {
    LOCKS = Lazy::new(|| Vec::new());
}

#[wasm_bindgen]
/// Allocates a new lock and returns its id. Recycles old locks if possible.
pub unsafe fn sab_lock_new() -> Option<LockId> {
    for lock in LOCKS.iter_mut() {
        if !lock.held {
            lock.held = true;
            return Some(lock.id);
        }
    }
    let id = LOCKS.len();
    let mut lock = AtomicLock::default();
    lock.id = id;
    LOCKS.push(lock);
    Some(id)
}

#[wasm_bindgen]
/// Frees a lock so other workers can use it.
pub unsafe fn sab_lock_free(lock: LockId) {
    LOCKS.get_mut(lock).and_then(|lock| {
        lock.unpoison();
        lock.held = false;
        Some(())
    });
}

#[wasm_bindgen]
/// Acquires a lock (same as locking a mutex)
pub unsafe fn sab_lock_acquire(lock: LockId) {
    LOCKS.get_mut(lock).and_then(|lock| {
        lock.block();
        lock.poison();
        Some(())
    });
}

#[wasm_bindgen]
/// Releases a lock (same as unlocking a mutex)
pub unsafe fn sab_lock_release(lock: LockId) {
    LOCKS.get_mut(lock).and_then(|lock| {
        lock.unpoison();
        Some(())
    });
}

/// Convenience wrapper around the sab-lock API.
#[derive(Debug)]
pub struct Lock(pub LockId);

impl Lock {
    /// Allocates a new lock.
    pub fn new() -> Option<Self> {
        Some(Self(unsafe { sab_lock_new()? }))
    }
    /// Acquires a lock manually.
    pub fn acquire(&mut self) {
        unsafe { sab_lock_acquire(self.0) };
    }
    /// Releases a lock manually.
    pub fn release(&mut self) {
        unsafe { sab_lock_release(self.0) };
    }
    /// Returns true if the lock is currently held.
    pub fn held(&self) -> bool {
        unsafe {
            LOCKS
                .get(self.0)
                .map(|lock| lock.poisoned())
                .unwrap_or(false)
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            sab_lock_free(self.0);
        }
    }
}

/// RAII style wrapper around the sab-lock API.
/// Automatically acquires a lock on creation and releases it on drop.
pub struct Guard(LockId);

impl Guard {
    pub fn new(lock: &Lock) -> Self {
        unsafe {
            sab_lock_acquire(lock.0);
            Self(lock.0)
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        unsafe { sab_lock_release(self.0) };
    }
}

#[macro_export]
macro_rules! guard {
    ($lock:expr) => {
        let _scoped_lock_guard = crate::lock::Guard::new(unsafe { &$lock });
    };
}
