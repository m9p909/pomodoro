use chrono::{Local, Utc};
use std::time::Instant;

#[cfg(test)]
use std::sync::Mutex;

pub trait Clock: Send + Sync {
    fn utc_ms(&self) -> i64;
    fn mono_ms(&self) -> u64;
    fn local_day(&self) -> String;
}

pub struct SystemClock {
    start: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn utc_ms(&self) -> i64 {
        Utc::now().timestamp_millis()
    }

    fn mono_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn local_day(&self) -> String {
        Local::now().format("%Y-%m-%d").to_string()
    }
}

#[cfg(test)]
pub struct FakeClock {
    state: Mutex<(i64, u64, String)>,
}

#[cfg(test)]
impl FakeClock {
    pub fn new(utc_ms: i64, mono_ms: u64, day: &str) -> Self {
        Self {
            state: Mutex::new((utc_ms, mono_ms, day.to_string())),
        }
    }

    pub fn advance(&self, ms: u64) {
        let mut s = self.state.lock().unwrap();
        s.0 += ms as i64;
        s.1 += ms;
    }

    pub fn set_day(&self, day: &str) {
        self.state.lock().unwrap().2 = day.to_string();
    }
}

#[cfg(test)]
impl Clock for FakeClock {
    fn utc_ms(&self) -> i64 {
        self.state.lock().unwrap().0
    }

    fn mono_ms(&self) -> u64 {
        self.state.lock().unwrap().1
    }

    fn local_day(&self) -> String {
        self.state.lock().unwrap().2.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_advances_utc_and_monotonic_time() {
        let clock = FakeClock::new(1_000, 10, "2026-10-26");
        let before = clock.utc_ms();
        clock.advance(500);
        assert_eq!(clock.utc_ms(), before + 500);
        assert_eq!(clock.mono_ms(), 510);
    }

    #[test]
    fn fake_clock_can_change_local_day() {
        let clock = FakeClock::new(0, 0, "2026-10-26");
        clock.set_day("2026-10-27");
        assert_eq!(clock.local_day(), "2026-10-27");
    }
}
