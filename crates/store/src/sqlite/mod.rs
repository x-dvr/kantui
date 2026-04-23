//! SQLite-backed implementations of the `kantui_core` repository ports.

mod project_repo;
mod tag_repo;
mod task_repo;

pub use project_repo::SqliteProjectRepo;
pub use tag_repo::SqliteTagRepo;
pub use task_repo::SqliteTaskRepo;

use std::str::FromStr;

use kantui_core::{CoreError, CoreResult};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/sqlite");

/// Connect to a SQLite database at `url`, enable foreign keys, and run
/// pending migrations.
///
/// `url` may be a file path URL (`sqlite:///path/to/kantui.db`) or the
/// in-memory form (`sqlite::memory:`). For tests that need an in-memory
/// database whose data survives across pool checkouts, use
/// [`connect_memory`] instead.
pub async fn connect(url: &str) -> CoreResult<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(url)
        .map_err(|e| CoreError::storage(format!("bad sqlite url: {url}"), e))?
        .foreign_keys(true)
        .create_if_missing(true);
    if let Some(parent) = opts.get_filename().parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            CoreError::storage(format!("create db dir {} failed", parent.display()), e)
        })?;
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .map_err(|e| CoreError::storage("connect failed", e))?;
    MIGRATOR
        .run(&pool)
        .await
        .map_err(|e| CoreError::storage("migrate failed", e))?;
    Ok(pool)
}

/// Open a fresh in-memory SQLite pool and run migrations.
///
/// Uses `max_connections=1` + `min_connections=1` so the single connection
/// owning the `:memory:` database is kept alive for the pool's lifetime.
pub async fn connect_memory() -> CoreResult<SqlitePool> {
    let opts = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(opts)
        .await
        .map_err(|e| CoreError::storage("connect in-memory failed", e))?;
    MIGRATOR
        .run(&pool)
        .await
        .map_err(|e| CoreError::storage("migrate failed", e))?;
    Ok(pool)
}

pub(crate) fn sqlx_err(ctx: &'static str) -> impl FnOnce(sqlx::Error) -> CoreError {
    move |e| CoreError::storage(ctx, e)
}
