//! Traits implemented by adapters. `core` never names a concrete adapter —
//! services take these as generic bounds.

mod clock;
mod id_gen;
mod project_repo;
mod tag_repo;
mod task_repo;

pub use clock::Clock;
pub use id_gen::IdGenerator;
pub use project_repo::ProjectRepository;
pub use tag_repo::TagRepository;
pub use task_repo::TaskRepository;
