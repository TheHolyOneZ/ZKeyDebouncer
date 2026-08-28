

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod debounce;
mod hook;

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;

use hook::{KeyEvent, Shared};

const DEFAULT_WINDOW_MS: u64 = 30;
const MIN_WINDOW_MS: u64 = 5;
const MAX_WINDOW_MS: u64 = 150;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    threshold_ms: u64,
    blocked: u64,
    seen: u64,
    filtering: bool,
    error: Option<String>,
}

#[tauri::command]
fn snapshot(state: tauri::State<'_, Arc<Shared>>) -> Snapshot {
    Snapshot {
        threshold_ms: state.window_ms.load(Ordering::Relaxed),
        blocked: state.blocked.load(Ordering::Relaxed),
        seen: state.seen.load(Ordering::Relaxed),
        filtering: state.filtering.load(Ordering::Relaxed),
        error: state.error.lock().ok().and_then(|e| e.clone()),
    }
}

#[tauri::command]
fn set_threshold(ms: u64, state: tauri::State<'_, Arc<Shared>>) {
    state
        .window_ms
        .store(ms.clamp(MIN_WINDOW_MS, MAX_WINDOW_MS), Ordering::Relaxed);
}

#[tauri::command]
fn set_filtering(on: bool, state: tauri::State<'_, Arc<Shared>>) {
    state.filtering.store(on, Ordering::Relaxed);
}

#[tauri::command]
fn reset_count(state: tauri::State<'_, Arc<Shared>>) {
    state.blocked.store(0, Ordering::Relaxed);
}

fn main() {
    let shared = Arc::new(Shared::new(DEFAULT_WINDOW_MS));
    let (tx, rx) = mpsc::channel::<KeyEvent>();

    hook::spawn(Arc::clone(&shared), tx);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::clone(&shared))
        .invoke_handler(tauri::generate_handler![
            snapshot,
            set_threshold,
            set_filtering,
            reset_count
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let shared = Arc::clone(&shared);

            std::thread::Builder::new()
                .name("event-forwarder".into())
                .spawn(move || {

                    if let Some(err) = shared.error.lock().ok().and_then(|e| e.clone()) {
                        let _ = handle.emit("hook-error", err);
                    }

                    while let Ok(event) = rx.recv() {
                        let _ = handle.emit("key-event", event);
                    }
                })?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ZKeyDebouncer");
}
