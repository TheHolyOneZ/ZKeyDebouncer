

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rdev::{Event, EventType, Key};

use crate::debounce::{Debouncer, Decision};
use crate::hook::{KeyEvent, Shared};

pub fn spawn(shared: Arc<Shared>, tx: Sender<KeyEvent>) {
    std::thread::Builder::new()
        .name("keyboard-hook".into())
        .spawn(move || {
            let debouncer = Mutex::new(Debouncer::<Key>::new());
            let was_filtering = AtomicBool::new(true);
            let shared_cb = Arc::clone(&shared);

            let result = rdev::grab(move |event: Event| {
                let (key, pressed) = match event.event_type {
                    EventType::KeyPress(k) => (k, true),
                    EventType::KeyRelease(k) => (k, false),
                    _ => return Some(event),
                };

                if pressed {
                    shared_cb.seen.fetch_add(1, Ordering::Relaxed);
                }

                let filtering = shared_cb.filtering.load(Ordering::Relaxed);
                if was_filtering.swap(filtering, Ordering::Relaxed) != filtering {
                    if let Ok(mut d) = debouncer.lock() {
                        d.clear();
                    }
                }
                if !filtering {
                    return Some(event);
                }

                let window =
                    Duration::from_millis(shared_cb.window_ms.load(Ordering::Relaxed));
                let now = Instant::now();

                let decision = match debouncer.lock() {
                    Ok(mut d) => {
                        if pressed {
                            d.on_press(key, now, window)
                        } else {
                            d.on_release(key, now)
                        }
                    }

                    Err(_) => Decision::Forward,
                };

                match decision {
                    Decision::Forward => {
                        if pressed {
                            let _ = tx.send(KeyEvent {
                                kind: "pass",
                                key: None,
                                gap_ms: None,
                                total: shared_cb.blocked.load(Ordering::Relaxed),
                            });
                        }
                        Some(event)
                    }
                    Decision::Drop { gap } => {
                        if pressed {
                            let total =
                                shared_cb.blocked.fetch_add(1, Ordering::Relaxed) + 1;
                            let _ = tx.send(KeyEvent {
                                kind: "block",
                                key: Some(key_name(key)),
                                gap_ms: Some(gap.as_millis() as u64),
                                total,
                            });
                        }
                        None
                    }
                }
            });

            let message = match result {
                Ok(()) => "the keyboard hook exited".to_string(),
                Err(err) => format!("{err:?}"),
            };
            shared.fail(message);
        })
        .expect("failed to start the keyboard hook thread");
}

fn key_name(key: Key) -> String {
    let raw = format!("{key:?}");
    if let Some(rest) = raw.strip_prefix("Key") {
        return rest.to_string();
    }
    if let Some(rest) = raw.strip_prefix("Num") {
        if rest.chars().all(|c| c.is_ascii_digit()) {
            return rest.to_string();
        }
    }
    if let Some(rest) = raw.strip_prefix("Unknown(") {
        return format!("#{}", rest.trim_end_matches(')'));
    }
    let mut out = String::with_capacity(raw.len() + 2);
    for (i, ch) in raw.char_indices() {
        if i > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}
