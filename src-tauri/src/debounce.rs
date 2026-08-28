

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {

    Forward,

    Drop { gap: Duration },
}

#[derive(Debug, Clone, Copy, Default)]
struct KeyState {

    held: bool,

    dropping: bool,
    last_release: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct Debouncer<K: Eq + Hash> {
    keys: HashMap<K, KeyState>,
}

impl<K: Eq + Hash> Debouncer<K> {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    pub fn on_press(&mut self, key: K, now: Instant, window: Duration) -> Decision {
        let state = self.keys.entry(key).or_default();

        if state.dropping {

            return Decision::Drop {
                gap: Duration::ZERO,
            };
        }
        if state.held {

            return Decision::Forward;
        }
        if let Some(released) = state.last_release {
            let gap = now.saturating_duration_since(released);
            if gap < window {
                state.dropping = true;
                return Decision::Drop { gap };
            }
        }
        state.held = true;
        Decision::Forward
    }

    pub fn on_release(&mut self, key: K, now: Instant) -> Decision {
        let state = self.keys.entry(key).or_default();
        state.last_release = Some(now);

        if state.dropping {
            state.dropping = false;
            return Decision::Drop {
                gap: Duration::ZERO,
            };
        }
        state.held = false;
        Decision::Forward
    }

    pub fn clear(&mut self) {
        self.keys.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Duration = Duration::from_millis(30);

    fn dropped(d: Decision) -> bool {
        matches!(d, Decision::Drop { .. })
    }

    #[test]
    fn a_clean_press_and_release_is_forwarded() {
        let mut d = Debouncer::new();
        let t = Instant::now();
        assert_eq!(d.on_press('a', t, WINDOW), Decision::Forward);
        assert_eq!(d.on_release('a', t + Duration::from_millis(60)), Decision::Forward);
    }

    #[test]
    fn a_bounce_and_its_release_are_both_dropped() {
        let mut d = Debouncer::new();
        let t = Instant::now();
        d.on_press('a', t, WINDOW);
        d.on_release('a', t + Duration::from_millis(50));

        let bounce = d.on_press('a', t + Duration::from_millis(58), WINDOW);
        assert_eq!(bounce, Decision::Drop { gap: Duration::from_millis(8) });
        assert!(dropped(d.on_release('a', t + Duration::from_millis(60))));
    }

    #[test]
    fn a_deliberate_second_press_outside_the_window_is_forwarded() {
        let mut d = Debouncer::new();
        let t = Instant::now();
        d.on_press('a', t, WINDOW);
        d.on_release('a', t + Duration::from_millis(50));
        let again = d.on_press('a', t + Duration::from_millis(90), WINDOW);
        assert_eq!(again, Decision::Forward);
    }

    #[test]
    fn auto_repeat_is_never_mistaken_for_chatter() {
        let mut d = Debouncer::new();
        let t = Instant::now();
        d.on_press('a', t, WINDOW);

        for ms in [500, 530, 560, 590] {
            let r = d.on_press('a', t + Duration::from_millis(ms), WINDOW);
            assert_eq!(r, Decision::Forward, "repeat at {ms}ms was dropped");
        }
    }

    #[test]
    fn keys_are_tracked_independently() {
        let mut d = Debouncer::new();
        let t = Instant::now();
        d.on_press('a', t, WINDOW);
        d.on_release('a', t + Duration::from_millis(10));

        assert_eq!(d.on_press('b', t + Duration::from_millis(12), WINDOW), Decision::Forward);
        assert!(dropped(d.on_press('a', t + Duration::from_millis(14), WINDOW)));
    }

    #[test]
    fn repeated_bounces_are_all_dropped() {
        let mut d = Debouncer::new();
        let t = Instant::now();
        d.on_press('a', t, WINDOW);
        d.on_release('a', t + Duration::from_millis(20));
        for i in 0..4 {
            let at = t + Duration::from_millis(22 + i * 4);
            assert!(dropped(d.on_press('a', at, WINDOW)));
            assert!(dropped(d.on_release('a', at + Duration::from_millis(2))));
        }

        assert_eq!(d.on_press('a', t + Duration::from_millis(200), WINDOW), Decision::Forward);
    }
}
