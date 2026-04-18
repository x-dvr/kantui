//! Hand-rolled error type. `core` may not depend on `thiserror`, so
//! [`CoreError`] implements `Display`, `Error::source`, and a `log_chain`
//! helper by hand. Adapters convert their native errors into
//! [`CoreError::Storage`] at the boundary.

use std::error::Error as StdError;
use std::fmt;

use crate::domain::{EntityId, StateId};

pub type CoreResult<T> = Result<T, CoreError>;

/// Which entity a `NotFound` refers to — lets the UI format a tailored message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Project,
    State,
    Task,
    Tag,
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EntityKind::Project => "project",
            EntityKind::State => "state",
            EntityKind::Task => "task",
            EntityKind::Tag => "tag",
        };
        f.write_str(s)
    }
}

/// Type-erased boxed cause carried through the storage boundary.
pub type BoxError = Box<dyn StdError + Send + Sync>;

#[derive(Debug)]
pub enum CoreError {
    NotFound { entity: EntityKind, id: EntityId },
    Validation(String),
    Conflict(String),
    WipLimitExceeded { state: StateId, limit: u32 },
    Storage { message: String, source: BoxError },
}

impl CoreError {
    #[must_use]
    pub fn storage(message: impl Into<String>, source: impl Into<BoxError>) -> Self {
        CoreError::Storage {
            message: message.into(),
            source: source.into(),
        }
    }

    #[must_use]
    pub fn validation(msg: impl Into<String>) -> Self {
        CoreError::Validation(msg.into())
    }

    #[must_use]
    pub fn conflict(msg: impl Into<String>) -> Self {
        CoreError::Conflict(msg.into())
    }

    /// Multi-line cause chain suitable for a log file:
    ///
    /// ```text
    /// error: storage: connection refused
    ///   caused by: io error: Connection refused (os error 111)
    /// ```
    #[must_use]
    pub fn log_chain(&self) -> String {
        let mut out = format!("error: {self}");
        let mut cause = self.source();
        while let Some(err) = cause {
            out.push_str("\n  caused by: ");
            out.push_str(&err.to_string());
            cause = err.source();
        }
        out
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::NotFound { entity, id } => {
                write!(f, "{entity} not found: {id}")
            }
            CoreError::Validation(msg) => write!(f, "invalid input: {msg}"),
            CoreError::Conflict(msg) => write!(f, "conflict: {msg}"),
            CoreError::WipLimitExceeded { state, limit } => {
                write!(f, "WIP limit {limit} exceeded on state {state}")
            }
            CoreError::Storage { message, .. } => write!(f, "storage: {message}"),
        }
    }
}

impl StdError for CoreError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            CoreError::Storage { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Inner(String, Option<Box<Inner>>);

    impl fmt::Display for Inner {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.0)
        }
    }

    impl StdError for Inner {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            self.1.as_deref().map(|b| b as &dyn StdError)
        }
    }

    #[test]
    fn log_chain_walks_sources() {
        let leaf = Inner("disk full".into(), None);
        let mid = Inner("io error".into(), Some(Box::new(leaf)));
        let err = CoreError::storage("write failed", mid);
        let chain = err.log_chain();
        assert!(chain.contains("error: storage: write failed"));
        assert!(chain.contains("caused by: io error"));
        assert!(chain.contains("caused by: disk full"));
    }

    #[test]
    fn display_is_single_line() {
        let err = CoreError::NotFound {
            entity: EntityKind::Task,
            id: EntityId::nil(),
        };
        assert_eq!(
            err.to_string(),
            format!("task not found: {}", EntityId::nil())
        );
    }
}
