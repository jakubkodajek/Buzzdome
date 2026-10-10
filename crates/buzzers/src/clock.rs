//! The show clock: the time presses and commands are stamped with
//! (`quiz_core::Stamped::at_ms`). It starts from the wall clock once and then
//! counts on from a monotonic clock, so it never goes back, nor jumps, when
//! the system sets its clock (for example on joining the venue's network).
//! The receiver thread and the app read the same clock.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy)]
pub struct ShowClock {
    /// The wall clock when the clock started, in milliseconds since the Unix epoch.
    anchor_ms: u64,
    started: Instant,
}

impl ShowClock {
    /// A clock that reads the wall clock now and counts on from there.
    pub fn start() -> ShowClock {
        let anchor_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        ShowClock { anchor_ms, started: Instant::now() }
    }

    /// Milliseconds since the Unix epoch, as the wall clock read at the
    /// start plus the time since; never less than an earlier reading.
    pub fn now_ms(&self) -> u64 {
        self.anchor_ms.saturating_add(self.started.elapsed().as_millis() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_show_clock_never_goes_back() {
        let clock = ShowClock::start();
        let mut last = clock.now_ms();
        for _ in 0..1_000 {
            let now = clock.now_ms();
            assert!(now >= last);
            last = now;
        }
        // A copy, as the receiver thread has, reads the same time.
        let copy = clock;
        assert!(copy.now_ms() >= last);
    }
}
