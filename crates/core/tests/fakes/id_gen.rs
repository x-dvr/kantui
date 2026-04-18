use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use kantui_core::{EntityId, IdGenerator};

/// Deterministic id generator: emits `[counter_be_bytes, 0, 0, ...]`.
/// Clones share the counter.
#[derive(Clone)]
pub struct CountingIds {
    counter: Arc<AtomicU64>,
}

impl CountingIds {
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicU64::new(1)),
        }
    }
}

impl IdGenerator for CountingIds {
    fn new_id(&self) -> EntityId {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&n.to_be_bytes());
        EntityId::from_bytes(bytes)
    }
}
