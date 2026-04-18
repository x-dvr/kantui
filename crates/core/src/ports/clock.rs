use crate::domain::Timestamp;

/// Provides the current time. Injected into services so tests can run against
/// a deterministic clock.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}
