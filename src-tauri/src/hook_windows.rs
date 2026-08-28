

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MAPVK_VK_TO_CHAR, MAPVK_VK_TO_VSC,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WM_APP, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::debounce::{Debouncer, Decision};
use crate::hook::{KeyEvent, Shared};

const PROBE_VK: u32 = 0x87;

const WM_REINSTALL: u32 = WM_APP + 1;

const PROBE_EVERY: Duration = Duration::from_secs(4);
const PROBE_GRACE: Duration = Duration::from_millis(600);

const MISSES_BEFORE_REINSTALL: u32 = 3;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

struct Runtime {
    shared: Arc<Shared>,
    tx: Sender<KeyEvent>,
    debouncer: Mutex<Debouncer<u32>>,
    was_filtering: AtomicBool,
    probe_seen: AtomicBool,
    hook_thread: AtomicU32,
}

impl Runtime {

    fn decide(&self, vk: u32, pressed: bool) -> bool {
        if pressed {
            self.shared.seen.fetch_add(1, Ordering::Relaxed);
        }

        let filtering = self.shared.filtering.load(Ordering::Relaxed);
        if self.was_filtering.swap(filtering, Ordering::Relaxed) != filtering {

            if let Ok(mut d) = self.debouncer.lock() {
                d.clear();
            }
        }
        if !filtering {
            return false;
        }

        let window = Duration::from_millis(self.shared.window_ms.load(Ordering::Relaxed));
        let now = Instant::now();

        let decision = match self.debouncer.lock() {
            Ok(mut d) => {
                if pressed {
                    d.on_press(vk, now, window)
                } else {
                    d.on_release(vk, now)
                }
            }

            Err(_) => Decision::Forward,
        };

        match decision {
            Decision::Forward => {
                if pressed {
                    let _ = self.tx.send(KeyEvent {
                        kind: "pass",
                        key: None,
                        gap_ms: None,
                        total: self.shared.blocked.load(Ordering::Relaxed),
                    });
                }
                false
            }
            Decision::Drop { gap } => {
                if pressed {
                    let total = self.shared.blocked.fetch_add(1, Ordering::Relaxed) + 1;
                    let _ = self.tx.send(KeyEvent {
                        kind: "block",
                        key: Some(key_name(vk)),
                        gap_ms: Some(gap.as_millis() as u64),
                        total,
                    });
                }
                true
            }
        }
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        if let Some(rt) = RUNTIME.get() {
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let vk = info.vkCode;
            let message = wparam.0 as u32;
            let pressed = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
            let released = message == WM_KEYUP || message == WM_SYSKEYUP;

            if vk == PROBE_VK {
                rt.probe_seen.store(true, Ordering::Relaxed);
                return LRESULT(1);
            }
            if (pressed || released) && rt.decide(vk, pressed) {
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn install() -> windows::core::Result<HHOOK> {
    unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) }
}

fn send_probe() {

    let scan = unsafe { MapVirtualKeyW(PROBE_VK, MAPVK_VK_TO_VSC) } as u16;
    let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(PROBE_VK as u16),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

pub fn spawn(shared: Arc<Shared>, tx: Sender<KeyEvent>) {
    let runtime = Runtime {
        shared: Arc::clone(&shared),
        tx,
        debouncer: Mutex::new(Debouncer::new()),
        was_filtering: AtomicBool::new(true),
        probe_seen: AtomicBool::new(false),
        hook_thread: AtomicU32::new(0),
    };
    if RUNTIME.set(runtime).is_err() {
        return;
    }
    let rt = RUNTIME.get().expect("runtime just set");

    let hook_shared = Arc::clone(&shared);
    std::thread::Builder::new()
        .name("keyboard-hook".into())
        .spawn(move || hook_thread(hook_shared, rt))
        .expect("failed to start the keyboard hook thread");

    std::thread::Builder::new()
        .name("hook-watchdog".into())
        .spawn(move || watchdog(shared, rt))
        .expect("failed to start the hook watchdog thread");
}

fn hook_thread(shared: Arc<Shared>, rt: &'static Runtime) {
    rt.hook_thread
        .store(unsafe { GetCurrentThreadId() }, Ordering::Release);

    let mut hook = match install() {
        Ok(h) => h,
        Err(e) => {
            shared.fail(format!("the keyboard hook could not be installed ({e})"));
            return;
        }
    };

    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if result.0 <= 0 {
            break;
        }
        if msg.message == WM_REINSTALL {
            unsafe { let _ = UnhookWindowsHookEx(hook); }
            match install() {
                Ok(h) => {
                    hook = h;
                    shared.clear_failure();
                }
                Err(e) => {
                    shared.fail(format!("the keyboard hook could not be reinstalled ({e})"));
                    return;
                }
            }
        }
    }

    unsafe { let _ = UnhookWindowsHookEx(hook); }
    shared.fail("the keyboard hook exited".into());
}

fn watchdog(shared: Arc<Shared>, rt: &'static Runtime) {
    let mut misses = 0u32;
    loop {
        std::thread::sleep(PROBE_EVERY);

        let thread_id = rt.hook_thread.load(Ordering::Acquire);
        if thread_id == 0 {
            continue;
        }

        rt.probe_seen.store(false, Ordering::Relaxed);
        send_probe();
        std::thread::sleep(PROBE_GRACE);

        if rt.probe_seen.load(Ordering::Relaxed) {
            misses = 0;
            continue;
        }

        misses += 1;
        if misses < MISSES_BEFORE_REINSTALL {
            continue;
        }
        misses = 0;
        let posted = unsafe {
            PostThreadMessageW(thread_id, WM_REINSTALL, WPARAM(0), LPARAM(0)).is_ok()
        };
        if !posted {
            shared.fail("the keyboard hook stopped and could not be restarted".into());
        }
    }
}

fn key_name(vk: u32) -> String {
    let named = match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x13 => "Pause",
        0x14 => "Caps Lock",
        0x1B => "Esc",
        0x20 => "Space",
        0x21 => "Page Up",
        0x22 => "Page Down",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2C => "Print Screen",
        0x2D => "Insert",
        0x2E => "Delete",
        0x5B => "Win Left",
        0x5C => "Win Right",
        0x5D => "Menu",
        0x90 => "Num Lock",
        0x91 => "Scroll Lock",
        0xA0 => "Shift Left",
        0xA1 => "Shift Right",
        0xA2 => "Ctrl Left",
        0xA3 => "Ctrl Right",
        0xA4 => "Alt Left",
        0xA5 => "Alt Right",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_string();
    }
    if (0x60..=0x69).contains(&vk) {
        return format!("Num {}", vk - 0x60);
    }
    if (0x70..=0x87).contains(&vk) {
        return format!("F{}", vk - 0x6F);
    }

    let ch = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_CHAR) } & 0x7FFF;
    match char::from_u32(ch) {
        Some(c) if !c.is_control() && c != ' ' => c.to_string(),
        _ => format!("VK {vk:#04X}"),
    }
}
