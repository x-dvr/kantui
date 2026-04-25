//! Persisted UI state — small TOML file that survives across runs.
//!
//! Currently records only the last opened project so the binary can
//! re-open it on startup. Read-best-effort and write-best-effort: failures
//! are logged but never propagate; a fresh state file is always recoverable.

use std::fs;
use std::io;
use std::path::Path;

use kantui_core::{EntityId, ProjectId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UiState {
    /// Project most recently opened on the board. Stored as a 32-char hex
    /// string so the file stays human-readable and free of crate-private
    /// encodings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_project: Option<String>,
}

impl UiState {
    /// Read `path`. Missing file or any parse error returns the default
    /// (empty) state — UI state is best-effort.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };
        match toml::from_str::<UiState>(&contents) {
            Ok(state) => state,
            Err(err) => {
                tracing::warn!(path = %path.display(), %err, "ignoring malformed UI state");
                Self::default()
            }
        }
    }

    /// Persist `self` to `path`, creating the parent directory if necessary.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, serialized)
    }

    #[must_use]
    pub fn last_project_id(&self) -> Option<ProjectId> {
        let raw = self.last_project.as_deref()?;
        parse_hex_id(raw).map(ProjectId::new)
    }

    pub fn set_last_project(&mut self, id: ProjectId) {
        self.last_project = Some(id.to_string());
    }
}

fn parse_hex_id(s: &str) -> Option<EntityId> {
    if s.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(EntityId::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_last_project() {
        let id = ProjectId::new(EntityId::from_bytes([0xab; 16]));
        let mut state = UiState::default();
        state.set_last_project(id);
        let toml_str = toml::to_string(&state).unwrap();
        let parsed: UiState = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.last_project_id(), Some(id));
    }

    #[test]
    fn malformed_hex_returns_none() {
        let state = UiState {
            last_project: Some("not-hex".into()),
        };
        assert_eq!(state.last_project_id(), None);
    }

    #[test]
    fn empty_state_has_no_last_project() {
        assert_eq!(UiState::default().last_project_id(), None);
    }
}
