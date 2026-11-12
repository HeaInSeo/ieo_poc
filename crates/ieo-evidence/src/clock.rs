//! Mock clock (02-02): virtual time for reproducing simulated events
//! (validation plan: record inputs, virtual time, fault order and seed).
//! A real run uses `SystemClock`; tests and simulations use `MockClock`,
//! which only moves when told to. Simulated runs must not be reported as
//! real service runs.

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
    /// `real` or `mock`, recorded next to timestamps derived from this clock.
    fn kind(&self) -> &'static str;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
    fn kind(&self) -> &'static str {
        "real"
    }
}

/// Deterministic clock: starts at a fixed instant and advances only by `advance`.
pub struct MockClock {
    t: Mutex<SystemTime>,
}

impl MockClock {
    /// Start at `unix_ms` milliseconds after the epoch.
    pub fn at_unix_ms(unix_ms: u64) -> Self {
        MockClock {
            t: Mutex::new(UNIX_EPOCH + Duration::from_millis(unix_ms)),
        }
    }
    pub fn advance(&self, d: Duration) {
        let mut t = self.t.lock().unwrap_or_else(|e| e.into_inner());
        *t += d;
    }
}

impl Clock for MockClock {
    fn now(&self) -> SystemTime {
        *self.t.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn kind(&self) -> &'static str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::rfc3339;

    #[test]
    fn mock_clock_moves_only_when_advanced() {
        let c = MockClock::at_unix_ms(1_794_483_297_000);
        assert_eq!(rfc3339(c.now()), "2026-11-12T11:34:57.000Z");
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(rfc3339(c.now()), "2026-11-12T11:34:57.000Z");
        c.advance(Duration::from_millis(1_500));
        assert_eq!(rfc3339(c.now()), "2026-11-12T11:34:58.500Z");
        assert_eq!(c.kind(), "mock");
        assert_eq!(SystemClock.kind(), "real");
    }
}
