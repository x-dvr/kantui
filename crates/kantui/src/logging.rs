//! File-only tracing setup. The TUI owns stdout, so logs MUST NOT write there.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::BoxMakeWriter;

/// Initialise tracing to write to `log_path`. If the file's parent directory
/// doesn't exist, it is created. `RUST_LOG` overrides `default_level`.
pub fn init(log_path: &Path, default_level: &str) -> io::Result<()> {
    if let Some(parent) = log_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    let writer = BoxMakeWriter::new(std::sync::Mutex::new(file));

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(false)
        .with_level(true)
        .init();

    Ok(())
}
