//! Real-wall-clock [`Clock`] implementation.

use std::time::SystemTime;

use kantui_core::{Clock, Timestamp};

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl SystemClock {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_system_time(SystemTime::now())
    }
}
