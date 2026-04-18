//! Use-case layer: thin orchestration over ports. Validates inputs, enforces
//! invariants (WIP limits, state ownership, ...), and calls the repositories.

mod position;
mod project_service;
mod stats_service;
mod tag_service;
mod task_service;

pub use project_service::{NewProject, NewState, ProjectService};
pub use stats_service::StatsService;
pub use tag_service::TagService;
pub use task_service::{NewTask, TaskService, TaskUpdate};
