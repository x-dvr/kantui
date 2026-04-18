//! Storage adapter implementing `kantui_core` ports via `sqlx`.
//!
//! Feature-gated: `sqlite` (default) or `postgres`.

mod clock;
mod id;
mod mapping;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use clock::SystemClock;
pub use id::UuidV4;
