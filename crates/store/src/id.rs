//! [`IdGenerator`] that emits UUID v4 bytes.

use kantui_core::{EntityId, IdGenerator};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default)]
pub struct UuidV4;

impl UuidV4 {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl IdGenerator for UuidV4 {
    fn new_id(&self) -> EntityId {
        EntityId::from_bytes(*Uuid::new_v4().as_bytes())
    }
}
