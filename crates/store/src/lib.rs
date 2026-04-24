//! Storage adapter implementing `kantui_core` ports via `sqlx` on SQLite.

mod clock;
mod id;
mod mapping;

pub mod sqlite;

pub use clock::SystemClock;
pub use id::UuidV4;
pub use sqlx::SqlitePool;
