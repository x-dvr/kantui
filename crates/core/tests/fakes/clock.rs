use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use kantui_core::{Clock, Timestamp};

/// A deterministic clock. Starts at `UNIX_EPOCH` and only advances when asked.
/// Clones share the same underlying time — hand out clones to services while
/// keeping one for the test to drive with `advance`.
#[derive(Clone)]
pub struct FakeClock {
    now: Arc<Mutex<SystemTime>>,
}

impl FakeClock {
    pub fn new() -> Self {
        Self {
            now: Arc::new(Mutex::new(SystemTime::UNIX_EPOCH)),
        }
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self.now.lock().unwrap();
        *now += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_system_time(*self.now.lock().unwrap())
    }
}
