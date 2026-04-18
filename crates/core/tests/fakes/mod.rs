//! In-memory test doubles for `core` ports.
//!
//! Each integration-test binary includes this module and may only exercise a
//! subset of the fakes, so silence the per-binary dead_code warnings here.
#![allow(dead_code, unused_imports)]

pub mod clock;
pub mod id_gen;
pub mod project_repo;
pub mod tag_repo;
pub mod task_repo;

pub use clock::FakeClock;
pub use id_gen::CountingIds;
pub use project_repo::InMemoryProjectRepo;
pub use tag_repo::InMemoryTagRepo;
pub use task_repo::InMemoryTaskRepo;
