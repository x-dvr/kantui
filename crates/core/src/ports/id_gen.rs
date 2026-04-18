use crate::domain::EntityId;

/// Generates opaque 128-bit identifiers. Adapters decide the encoding
/// (UUIDv4, ULID, ...) — the domain never sees it.
pub trait IdGenerator: Send + Sync {
    fn new_id(&self) -> EntityId;
}
