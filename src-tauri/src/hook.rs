

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use serde::Serialize;

#[cfg(windows)]
#[path = "hook_windows.rs"]
mod backend;

#[cfg(not(windows))]
#[path = "hook_portable.rs"]
mod backend;

pub struct Shared {
    pub window_ms: AtomicU64,
    pub blocked: AtomicU64,

    pub seen: AtomicU64,
    pub filtering: AtomicBool,
    pub error: Mutex<Option<String>>,
}

impl Shared {
    pub fn new(window_ms: u64) -> Self {
        Self {
            window_ms: AtomicU64::new(window_ms),
            blocked: AtomicU64::new(0),
            seen: AtomicU64::new(0),
            filtering: AtomicBool::new(true),
            error: Mutex::new(None),
        }
    }

    pub fn fail(&self, message: String) {
        eprintln!("[zkeydebouncer] {message}");
        if let Ok(mut slot) = self.error.lock() {
            *slot = Some(message);
        }
    }

    pub fn clear_failure(&self) {
        if let Ok(mut slot) = self.error.lock() {
            *slot = None;
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyEvent {
    pub kind: &'static str,

    pub key: Option<String>,
    pub gap_ms: Option<u64>,
    pub total: u64,
}

pub fn spawn(shared: Arc<Shared>, tx: Sender<KeyEvent>) {
    backend::spawn(shared, tx);
}
