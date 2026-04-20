# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

**kantui** — a terminal kanban board in Rust, built on [ratatui](https://ratatui.rs/). Vim-inspired modal keyboard control, pluggable storage (SQLite default, PostgreSQL optional). See `plan.md` for the full implementation plan and milestones.

## Architecture — hexagonal, strictly

Cargo workspace with four crates:

- `crates/core` — domain entities (`Project`, `Column`, `Task`, `Tag`), **port traits** (`ProjectRepository`, `TaskRepository`, `TagRepository`, `Clock`), and service use cases. **No dependency on `ratatui`, `sqlx`, `tokio` runtime features, or any other workspace crate.** This is the rule to defend first.
- `crates/store` — adapter implementing the port traits with `sqlx`. Gated behind `sqlite` / `postgres` cargo features.
- `crates/widgets` — reusable `ratatui` widgets. Read-only view models only; no I/O, no repository references.
- `crates/kantui` — the binary: composition root, event loop, keymap, modes. Wires everything together at `main`.

Dependency direction is one-way: `kantui → {widgets, store} → core`. If a change makes `core` depend on anything outside itself, that change is wrong.

## Key architectural rules

- **Domain purity**: `core` uses only `std`, `async-trait`. No TUI types, no SQL types leak into `core`.
- **Ports over impls**: services in `core` take repositories as generic `R: ProjectRepository` bounds, never concrete adapter types.
- **Widgets are dumb**: a widget receives a view model and draws it. It never queries the store, never awaits, never mutates domain state.
- **Actions go one way**: UI event → `actions.rs` → `core::services` → port → `store` adapter → database. The reverse direction is rendering only.
- **Errors**: one `CoreError` in `core` using `thiserror`; adapters wrap backend errors into `CoreError::Storage`. The binary converts to user-facing messages at the edge. And logs errors to file with detailes information about full error path.

## Rust conventions

- Edition **2024**, MSRV pinned in `rust-toolchain.toml`.
- `cargo fmt` and `cargo clippy -- -D warnings` are mandatory; CI enforces both.
- Prefer `?` over `unwrap`/`expect` outside tests and `main`. An `expect` needs a message explaining the invariant.
- Newtype IDs (`ProjectId(Uuid)`, etc.) — never pass raw `Uuid` across module boundaries.
- Derive `Debug` on all domain types; derive `Clone` only when the type is cheap or the caller needs ownership; avoid `Copy` on domain structs.
- Public items on `core` get doc comments (`///`). Private items get a comment only when the *why* is non-obvious.
- Async: `tokio` multi-thread runtime in the binary. Traits that may be implemented by multiple backends use `#[async_trait]`; purely internal async fns use native `async fn`.
- Use `tracing` for logs (never `println!`/`eprintln!` outside `main` bootstrap). TUI apps must route logs to a file, not stdout.
- `#[must_use]` on constructors and builders that return owned values.
- No `unsafe` without a `// SAFETY:` comment proving the invariant.
- Compiler warnings must be treated as errors (always fix immediately)

## Testing

- `core`: pure unit tests with in-memory fake repos in `core/tests/fakes/`.
- `store`: a shared repo-contract test suite runs against SQLite in-memory; Postgres variant gated behind `--features postgres-tests`.
- `widgets`: snapshot tests with `ratatui::backend::TestBackend` + `insta`.
- `kantui`: end-to-end tests drive the app with synthetic key events against in-memory repos.
- Run the full suite with `cargo test --workspace`. Don't add tests that require network or a running database without gating them behind a feature.

## Common commands

```bash
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p kantui -- --db sqlite://./kantui.db
```

## Working style in this repo

- When asked to add a feature, start by naming which crate it belongs in. If the feature is domain logic, it lives in `core`; if it's a widget, in `widgets`; and so on. Crossing boundaries is a design smell — stop and surface it.
- Do not add crates to `core`'s `Cargo.toml` without a clear justification. Every new dep on `core` is a potential architecture leak.
- When adding a port, add an in-memory fake in the same PR so services remain testable.
- Prefer editing `plan.md` over letting it drift; update the milestone table as things land.
- Migrations are append-only. Don't edit a migration that has been committed.
- Match the scope of the task. A bug fix is a bug fix; don't refactor surrounding code unless asked.
