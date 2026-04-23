//! kantui domain core — entities, ports, services, errors.
//!
//! This crate has zero dependencies on infrastructure or UI. It defines the
//! hexagon's interior: the domain model plus the traits adapters must implement
//! to persist it. Services orchestrate use cases via those traits.

pub mod domain;
pub mod error;
pub mod ports;
pub mod services;

pub use domain::{
    Color, Complexity, Duration, EntityId, Priority, Project, ProjectId, State, StateId,
    StateSojourn, Tag, TagId, Task, TaskId, TaskTransition, Throughput, Timestamp,
};
pub use error::{CoreError, CoreResult, EntityKind};
pub use ports::{Clock, IdGenerator, ProjectRepository, TagRepository, TaskRepository};
pub use services::{
    NewProject, NewState, NewTask, ProjectService, StatsService, TagService, TaskService,
    TaskUpdate,
};
