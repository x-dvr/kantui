use std::fmt;
use std::time::SystemTime;

/// Domain timestamp, wrapping `std::time::SystemTime`.
///
/// Services obtain timestamps via the [`Clock`](crate::ports::Clock) port so
/// tests can control time.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(SystemTime);

impl Timestamp {
    #[must_use]
    pub const fn from_system_time(t: SystemTime) -> Self {
        Self(t)
    }

    #[must_use]
    pub const fn to_system_time(self) -> SystemTime {
        self.0
    }

    /// Saturating difference: returns `other - self`, or zero if `other` is earlier.
    #[must_use]
    pub fn saturating_since(self, earlier: Timestamp) -> Duration {
        self.0.duration_since(earlier.0).unwrap_or(Duration::ZERO)
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(d) => write!(f, "Timestamp({}s)", d.as_secs()),
            Err(_) => write!(f, "Timestamp(pre-epoch)"),
        }
    }
}

/// Re-export of `std::time::Duration` as a domain alias.
pub type Duration = std::time::Duration;
