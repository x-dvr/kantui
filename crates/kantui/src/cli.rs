//! Command-line arguments + path resolution.

use std::path::PathBuf;

use clap::Parser;
use directories::ProjectDirs;

/// Terminal kanban board.
#[derive(Debug, Parser)]
#[command(name = "kantui", version, about)]
pub struct Args {
    /// Database URL, e.g. `sqlite:///path/to/kantui.db`. Defaults to a file
    /// under the user's data directory.
    #[arg(long)]
    pub db: Option<String>,

    /// Path to the log file. Defaults to `<cache>/kantui/kantui.log`.
    #[arg(long)]
    pub log: Option<PathBuf>,

    /// Log level: `error`, `warn`, `info`, `debug`, `trace`.
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Path to the TOML config file. Defaults to
    /// `<config>/kantui/config.toml`.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Path to the persisted UI state file (last opened project, ...).
    /// Defaults to `<data>/kantui/state.toml`.
    #[arg(long)]
    pub state: Option<PathBuf>,

    /// Write a default config file to the config path and exit.
    #[arg(long, default_value_t = false)]
    pub gen_conf: bool,

    /// On first run (empty DB), also seed a few sample tasks. Without this,
    /// the default project is created with empty columns.
    #[arg(long, default_value_t = false)]
    pub seed_demo: bool,
}

/// Resolved paths + DB URL, with directories-based defaults applied.
pub struct Resolved {
    pub db_url: String,
    pub log_path: PathBuf,
    pub log_level: String,
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    pub gen_conf: bool,
    pub seed_demo: bool,
}

impl Args {
    pub fn resolve(self) -> Resolved {
        let dirs = ProjectDirs::from("", "", "kantui");

        let db_url = self.db.unwrap_or_else(|| default_db_url(dirs.as_ref()));
        let log_path = self.log.unwrap_or_else(|| default_log_path(dirs.as_ref()));
        let config_path = self
            .config
            .unwrap_or_else(|| default_config_path(dirs.as_ref()));
        let state_path = self
            .state
            .unwrap_or_else(|| default_state_path(dirs.as_ref()));

        Resolved {
            db_url,
            log_path,
            log_level: self.log_level,
            config_path,
            state_path,
            gen_conf: self.gen_conf,
            seed_demo: self.seed_demo,
        }
    }
}

fn default_db_url(dirs: Option<&ProjectDirs>) -> String {
    let path = dirs
        .map(|d| d.data_dir().join("kantui.db"))
        .unwrap_or_else(|| PathBuf::from("./kantui.db"));
    format!("sqlite://{}", path.display())
}

fn default_log_path(dirs: Option<&ProjectDirs>) -> PathBuf {
    dirs.map(|d| d.cache_dir().join("kantui.log"))
        .unwrap_or_else(|| PathBuf::from("./kantui.log"))
}

fn default_config_path(dirs: Option<&ProjectDirs>) -> PathBuf {
    dirs.map(|d| d.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("./kantui.toml"))
}

fn default_state_path(dirs: Option<&ProjectDirs>) -> PathBuf {
    dirs.map(|d| d.data_dir().join("state.toml"))
        .unwrap_or_else(|| PathBuf::from("./kantui-state.toml"))
}
