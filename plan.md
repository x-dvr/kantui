# kantui — Implementation Plan

A terminal kanban board written in Rust using [ratatui](https://ratatui.rs/), structured as a Cargo workspace with a hexagonal (ports-and-adapters) architecture.

---

## 1. Goals & non-goals

### Goals
- Interactive TUI for managing kanban boards from the terminal.
- Vim-inspired modal keyboard control (no mouse required).
- CRUD on **projects** (boards), **states** (workflow stages rendered as columns), **tasks**, and **tags**.
- Move tasks between states; reorder within a state.
- Pluggable storage backend: SQLite (default, file-based) or PostgreSQL.
- Strict hexagonal architecture: domain core has **zero** dependencies on infrastructure, UI, or external crates beyond `std`.
- Custom error types must be defined in core crate. They should be able to incapsulate cause and provide nice support for logging (into file). Other crates must convert own errors to domain errors.
- should have config file to change keybinds color theme (catppuccin frappe by default)
- app should keep log/statistic of how long each task is kept in each state. Sould be a dashboard showing statistic.
- bottom row shold have status/command bar like in helix editor

### Non-goals (v1)
- Multi-user / collaboration / sync.
- Authentication.
- Web or GUI frontend.
- Plugins / scripting.
- Attachment storage.

---

## 2. Workspace layout

```
kantui/
├── Cargo.toml                 # [workspace] root, shared deps via [workspace.dependencies]
├── Cargo.lock
├── rust-toolchain.toml        # pin stable channel
├── CLAUDE.md
├── plan.md
├── README.md
├── .gitignore
├── crates/
│   ├── core/                  # domain + ports (NO infra, NO ratatui, NO sqlx)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── domain/        # entities + value objects
│   │       │   ├── mod.rs
│   │       │   ├── project.rs
│   │       │   ├── state.rs
│   │       │   ├── task.rs
│   │       │   ├── tag.rs
│   │       │   └── ids.rs
│   │       ├── ports/         # traits implemented by adapters
│   │       │   ├── mod.rs
│   │       │   ├── project_repo.rs
│   │       │   ├── task_repo.rs
│   │       │   ├── tag_repo.rs
│   │       │   └── clock.rs
│   │       ├── services/      # use cases orchestrating the domain via ports
│   │       │   ├── mod.rs
│   │       │   ├── project_service.rs
│   │       │   ├── task_service.rs
│   │       │   └── tag_service.rs
│   │       └── error.rs
│   ├── store/                 # adapter: implements core::ports using sqlx
│   │   ├── Cargo.toml
│   │   ├── migrations/
│   │   │   ├── sqlite/
│   │   │   └── postgres/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sqlite/        # gated behind feature "sqlite"
│   │       ├── postgres/      # gated behind feature "postgres"
│   │       └── mapping.rs     # row <-> domain conversions
│   ├── widgets/               # reusable ratatui widgets
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── board.rs       # multi-column board view
│   │       ├── column.rs      # single column (rendered as a column card with title)
│   │       ├── task_card.rs   # task row/card
│   │       ├── task_detail.rs # task side-panel / modal
│   │       ├── input.rs       # single-line text input
│   │       ├── prompt.rs      # command/search prompt
│   │       └── help.rs        # keybinding cheat sheet
│   └── kantui/                # binary: composition root + event loop
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── app.rs         # App state machine
│           ├── config.rs      # CLI args, config file, DB selection
│           ├── event.rs       # crossterm event normalization
│           ├── keymap.rs      # Vim-style key dispatcher
│           ├── mode.rs        # Normal / Insert / Command / Search
│           ├── view.rs        # draw(f, &state)
│           └── actions.rs     # intent-level actions invoked by keymap
```

### Dependency graph (enforced by Cargo)

```
kantui ──▶ widgets ──▶ core
   │
   └─────▶ store ────▶ core
```

- `core` depends on nothing from this workspace.
- `store` depends only on `core` (for port traits and domain types).
- `widgets` depends only on `core` (for read-only view models).
- `kantui` depends on all three and wires them at `main`.

---

## 3. Domain model (`core`)

`core` depends only on `std` and `async-trait`. That rules out `uuid`, `chrono`, `thiserror`, and `serde`, so the domain defines its own small set of value types (`EntityId`, `Timestamp`, `Color`) and hand-rolls `Display`/`Error`. Adapters convert to their native equivalents at the boundary.

Throughout the domain, a kanban workflow stage is called a **State**. The TUI renders a project's states horizontally as columns, but the domain concept is the state itself.

### Value types (all `std`-only)

```rust
// ids.rs — opaque 128-bit identifier; adapter picks encoding (UUIDv4, ULID, ...)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId([u8; 16]);

impl EntityId {
    pub const fn from_bytes(b: [u8; 16]) -> Self { Self(b) }
    pub const fn as_bytes(&self) -> &[u8; 16] { &self.0 }
}

// Per-entity newtypes — prevent mixing ID spaces at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct ProjectId(pub EntityId);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct StateId  (pub EntityId);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct TaskId   (pub EntityId);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct TagId    (pub EntityId);
```

```rust
// time.rs — domain timestamp, wraps std::time::SystemTime
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(std::time::SystemTime);

impl Timestamp {
    pub fn now_from(clock: &dyn Clock) -> Self { clock.now() }
    pub fn to_system_time(self) -> std::time::SystemTime { self.0 }
    pub fn from_system_time(t: std::time::SystemTime) -> Self { Self(t) }
}

// Duration used for sojourn statistics — re-export std's Duration as a domain alias.
pub type Duration = std::time::Duration;
```

### Enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority  { Low, Normal, High, Critical }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Complexity { Light, Deep }

// Domain-level color palette, independent of ratatui / terminal encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color { Red, Green, Yellow, Blue, Magenta, Cyan, White, Gray, Custom([u8; 3]) }
```

### Entities

```rust
// project.rs
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub description: Option<String>,
    pub states: Vec<State>,       // ordered
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

// state.rs
pub struct State {
    pub id: StateId,
    pub project_id: ProjectId,
    pub name: String,
    pub position: i32,            // ordering within project
    pub wip_limit: Option<u32>,   // optional WIP limit
}

// task.rs
pub struct Task {
    pub id: TaskId,
    pub project_id: ProjectId,
    pub state_id: StateId,
    pub title: String,
    pub description: Option<String>,
    pub priority: Priority,
    pub complexity: Complexity,
    pub due_date: Option<Timestamp>,
    pub tags: Vec<TagId>,
    pub position: i32,            // ordering within state
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

// tag.rs
pub struct Tag {
    pub id: TagId,
    pub name: String,
    pub color: Color,
}

// history.rs — one row per state transition, powers the sojourn dashboard
pub struct TaskTransition {
    pub task_id: TaskId,
    pub from_state: Option<StateId>, // None for creation
    pub to_state:   StateId,
    pub at: Timestamp,
}

// Aggregated view, computed by the service from the transition log.
pub struct StateSojourn {
    pub state_id: StateId,
    pub total:  Duration,
    pub count:  u32,
}
```

### Ports (traits — the hexagon's boundary)

```rust
// ports/project_repo.rs
#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn create(&self, project: &Project) -> Result<()>;
    async fn get(&self, id: ProjectId) -> Result<Option<Project>>;
    async fn list(&self) -> Result<Vec<Project>>;
    async fn update(&self, project: &Project) -> Result<()>;
    async fn delete(&self, id: ProjectId) -> Result<()>;

    async fn add_state(&self, state: &State) -> Result<()>;
    async fn remove_state(&self, id: StateId) -> Result<()>;
    async fn reorder_states(&self, project_id: ProjectId, ordered: &[StateId]) -> Result<()>;
}
```

`TaskRepository` and `TagRepository` follow the same pattern. `TaskRepository` additionally exposes `move_task(task_id, target_state, target_position, at: Timestamp)` which atomically updates the task *and* appends a `TaskTransition` row; `list_transitions(task_id)` and `aggregate_sojourns(project_id)` feed the statistics dashboard.

Additional ports:

```rust
// clock.rs — deterministic time for services/tests
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

// id_gen.rs — core does not know what a UUID is; the adapter decides.
pub trait IdGenerator: Send + Sync {
    fn new_id(&self) -> EntityId;
}
```

### Services (use cases)

Thin orchestration layer that validates inputs, enforces invariants (e.g. state belongs to project, WIP limit), and calls the repositories. Services take the repo traits as generic type parameters so `core` never names a concrete adapter.

```rust
pub struct TaskService<R, C, G>
where R: TaskRepository, C: Clock, G: IdGenerator
{ repo: R, clock: C, ids: G }
```

Alongside the CRUD services, a `StatsService` computes per-state sojourn metrics by folding `TaskTransition` rows returned by `TaskRepository::list_transitions`.

### Errors

Because `core` may not depend on `thiserror`, the error type is hand-written. It captures a variant, a human-readable message, and an optional boxed cause; `Display` renders a single line for status-bar use, while a `log_chain()` helper walks `source()` and produces the multi-line cause chain that goes into the log file. Adapters convert their native errors (e.g. `sqlx::Error`) into `CoreError::Storage { source: Box<...> }` at the boundary.

```rust
// error.rs (sketch)
#[derive(Debug)]
pub enum CoreError {
    NotFound { entity: EntityKind, id: EntityId },
    Validation(String),
    Conflict(String),
    WipLimitExceeded { state: StateId, limit: u32 },
    Storage { message: String, source: Box<dyn std::error::Error + Send + Sync> },
}

#[derive(Debug, Clone, Copy)]
pub enum EntityKind { Project, State, Task, Tag }

impl std::fmt::Display for CoreError { /* one-line summary */ }
impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CoreError::Storage { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl CoreError {
    /// Multi-line "error: ... \n  caused by: ..." formatting for the log file.
    pub fn log_chain(&self) -> String { /* walks self.source() */ }
}

pub type CoreResult<T> = Result<T, CoreError>;
```

---

## 4. Storage adapter (`store`)

- Uses `sqlx` with the `runtime-tokio` feature.
- Features: `sqlite` (default), `postgres`. Both may be compiled in; the binary picks one at runtime based on config.
- One module per backend with a shared `mapping.rs` converting `sqlx::Row` ↔ domain structs.
- Migrations live alongside code (`migrations/sqlite`, `migrations/postgres`) and are applied on startup via `sqlx::migrate!`.

### Schema sketch

```sql
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE states (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    position INTEGER NOT NULL,
    wip_limit INTEGER
);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    state_id TEXT NOT NULL REFERENCES states(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT,
    priority TEXT NOT NULL,
    due_date TEXT,
    position INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL
);

CREATE TABLE task_tags (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    tag_id  TEXT NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
    PRIMARY KEY (task_id, tag_id)
);

-- Append-only log of every state move; powers the sojourn dashboard.
CREATE TABLE task_transitions (
    task_id    TEXT NOT NULL REFERENCES tasks(id)   ON DELETE CASCADE,
    from_state TEXT REFERENCES states(id)           ON DELETE SET NULL, -- NULL on creation
    to_state   TEXT NOT NULL REFERENCES states(id)  ON DELETE CASCADE,
    at         TEXT NOT NULL,
    PRIMARY KEY (task_id, at)
);
CREATE INDEX task_transitions_by_state ON task_transitions (to_state, at);
```

The `tasks` schema also carries a `complexity TEXT NOT NULL` column (`"light"` | `"deep"`).

Position management uses a sparse integer scheme (steps of 1024); reorders rebalance only when a gap hits zero, so normal moves are O(1) writes.

Move semantics: `TaskRepository::move_task` runs as a single transaction — updates `tasks.state_id`/`position`/`updated_at` and inserts one row into `task_transitions`. This guarantees the log never diverges from live task state.

---

## 4a. Statistics & dashboard

Per-state sojourn statistics come straight from the `task_transitions` log:

- `StatsService::project_sojourns(project_id) -> Vec<StateSojourn>` — for every task, pair consecutive transitions, add `to - from` to the `from_state`'s totals; the task's currently-occupied state accrues `now - last_at` as *live* time so the dashboard updates in real time.
- `StatsService::task_history(task_id) -> Vec<(StateId, Duration)>` — per-task breakdown rendered in the task-detail panel.
- `StatsService::throughput(project_id, window) -> Throughput` — tasks moved into a "done" state per day/week (done-state id is a project setting).

Rendering lives in a `Dashboard` widget (see §5) that shows: average/median sojourn per state, slowest-moving tasks, WIP vs. limit, a simple bar chart of throughput.

---

## 5. Widgets (`widgets`)

Each widget implements `ratatui::widgets::Widget` (or `StatefulWidget` where state is needed) and takes read-only references to domain view models defined in `core`. Widgets do **no** I/O and hold **no** repository references.

- `BoardView` — renders a project's states horizontally (as columns) with a cursor and scroll state.
- `StateView` — a single state's task list with selection highlight, rendered as a column.
- `TaskCard` — title, priority/complexity glyphs, due-date, tag chips.
- `TaskDetail` — multi-line modal panel for editing full task fields; shows per-state sojourn for the selected task.
- `Input` — single-line text editor (backspace, cursor, clipboard-less paste).
- `Prompt` — bottom-bar `:` command / `/` search input.
- `StatusBar` — **Helix-style** bottom row: `[mode]  project › state  task-title   counts   diagnostics   clock`. Colored by mode (Normal/Insert/Command/Search). Shares its row with `Prompt` when the prompt is active.
- `Dashboard` — project-level statistics screen: sojourn chart, throughput sparkline, WIP per state.
- `HelpOverlay` — cheat sheet rendered from the active keymap table.
- `JumpLabels` — two-letter overlays for `gw` quick-jump (Helix-style).

All widgets take a `Theme` reference (see §7) and resolve domain `Color` variants to `ratatui::style::Color` via a thin mapping function.

---

## 6. Binary (`kantui`) — event loop & state machine

### Modes

- **Normal** — navigation and commands.
- **Insert** — text entry into the focused field.
- **Command** — `:new-board`, `:delete-state "Done"`, `:tag add bug`, etc.
- **Search** — `/query` fuzzy-filters tasks in the current board.

### Keymap (Vim-inspired, normal mode)

| Key                   | Action                                   |
| --------------------- | ---------------------------------------- |
| `h` / `l`             | Move cursor between states               |
| `j` / `k`             | Move cursor between tasks in state       |
| `gg` / `G`            | Top / bottom of state                    |
| `gw`                  | Navigate to two-letter index (like in helix)|
| `0` / `$`             | First / last state                       |
| `H` / `L`             | Move selected task left / right state    |
| `K` / `J`             | Move selected task up / down in state    |
| `n` / `N`             | New task below / above cursor            |
| `i`                   | Edit selected task title inline          |
| `e`                   | Open task detail for editing             |
| `d`                   | Delete selected task (with confirm)      |
| `y` / `p`             | Yank / paste task                        |
| `r`                   | Rename state                             |
| `t`                   | Tag picker for selected task             |
| `Tab` / `Shift-Tab`   | Cycle focused state                      |
| `Space`               | Toggle task done / move to Done state    |
| `?`                   | Toggle help overlay                      |
| `:`                   | Enter command mode                       |
| `/`                   | Enter search mode                        |
| `Esc`                 | Return to normal mode / cancel           |
| `q`                   | Quit (prompts if unsaved)                |
| `<count><motion>`     | Count-prefixed motions, e.g. `5j`, `3l`  |

### Event loop (pseudo)

```
loop {
    terminal.draw(|f| view::render(f, &state))?;
    match events.next().await? {
        Event::Key(k)  => { let action = keymap.dispatch(&state.mode, k); state.apply(action, &services).await?; }
        Event::Resize  => { /* ratatui handles layout */ }
        Event::Tick    => { /* animations, due-date refresh */ }
    }
    if state.should_quit { break; }
}
```

State mutations go through `actions.rs` → `services` (from `core`) → `store` adapter, keeping the UI layer oblivious to persistence.

---

## 7. Configuration

### CLI (via `clap`)

- `--db sqlite:///path/to/kantui.db` (default: `~/.local/share/kantui/kantui.db`)
- `--db postgres://user:pass@host/db`
- `--config path/to/config.toml` (default: `~/.config/kantui/config.toml`)
- `--log path/to/kantui.log` (default: `~/.cache/kantui/kantui.log`)

DB URL resolution order: CLI flag → env `KANTUI_DB` → config file → default SQLite path.

### Config file

TOML at `~/.config/kantui/config.toml`. Autogenerated with defaults on first run with --gen-conf argument. Schema:

```toml
[general]
default_project = "Inbox"
done_state      = "Done"        # used by stats throughput and Space-toggle

[theme]
name = "catppuccin-frappe"      # default; built-ins also "catppuccin-mocha", "gruvbox-dark", "solarized-dark" can create custom in ~/.config/kantui/themes

# Optional per-role overrides (all theme colors are resolvable without overrides).
[theme.overrides]
background  = "#303446"
foreground  = "#c6d0f5"
accent      = "#ca9ee6"
selection   = "#414559"
status_bar  = "#51576d"

[keybinds.normal]
left          = "h"
right         = "l"
down          = "j"
up            = "k"
top           = "gg"
bottom        = "G"
jump_two_char = "gw"            # Helix-like quick jump
new_task      = "n"
edit_inline   = "i"
open_detail   = "e"
delete        = "d"
yank          = "y"
paste         = "p"
move_left     = "H"
move_right    = "L"
move_up       = "K"
move_down     = "J"
toggle_done   = "space"
help          = "?"
command_mode  = ":"
search_mode   = "/"
quit          = "q"

[keybinds.insert]
cancel        = "esc"
submit        = "enter"
```

- Unknown keys are warnings, not errors, so stale configs keep working across versions.
- The built-in default theme is **catppuccin-frappe**. Theme palette files (TOML) can also be dropped into `~/.config/kantui/themes/` and referenced by `name`.
- Keybinds parse as either a single char (`"h"`), a named key (`"esc"`, `"space"`, `"enter"`, `"tab"`), a modifier combo (`"ctrl-s"`), or a fixed two-key chord (`"gg"`, `"gw"`).

### Logging

- `tracing-subscriber` writes to the log file only (TUI owns the terminal).
- Default level `info`; override with `RUST_LOG` or `--log-level`.
- `CoreError::log_chain()` is invoked at the edge (in `actions.rs`) so every error entering the UI layer is logged with its full cause chain before being surfaced as a short message in the status bar.

---

## 8. Testing strategy

- **`core` unit tests**: pure Rust, no async runtime needed for domain logic; services tested with in-memory fake repos living in `core/tests/fakes/` (they implement the port traits).
- **`store` integration tests**: spin up an in-memory SQLite (`sqlite::memory:`) and run the full repo contract; a shared test harness in `store/tests/contract.rs` is parameterised across backends so Postgres tests can be gated behind `--features postgres-tests`.
- **`widgets` snapshot tests**: use `ratatui::backend::TestBackend` + `insta` to diff buffers.
- **`kantui` end-to-end**: drive the app with synthetic key events against an in-memory repo, assert the rendered buffer.
- CI runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`, and the Postgres tests inside a `services: postgres` job.

---

## 9. Milestones

| #  | Milestone                                                       | Scope |
|----|-----------------------------------------------------------------|-------|
| 0  | Workspace scaffolding + CI skeleton                             | ✅ Empty crates compile, `cargo test --workspace` runs, clippy `-D warnings` + fmt green. |
| 1  | `core` domain + ports + hand-rolled errors + in-memory fakes    | ✅ 22 tests: project/task/tag services, WIP limits, sojourn stats, error-chain log. |
| 2  | `store` SQLite adapter + migrations (incl. `task_transitions`)  | ✅ 14 store tests: project/task/tag CRUD, reorder, move-transaction, cascade deletes, tag attach/detach — all pass against in-memory SQLite. |
| 3  | `widgets` minimal set (board, state, card, input, status bar)   | ✅ 15 tests (8 unit + 7 snapshot): catppuccin-frappe theme, TaskCard/StateColumn/BoardView/Input/StatusBar rendered against `TestBackend` + `insta`. |
| 4  | `kantui` event loop, Normal mode, basic navigation, log-to-file | ✅ 3 e2e tests: seeded demo project renders, h/j/k/l/gg/G/q navigate, errors log via `CoreError::log_chain`. Binary composes SQLite pool + widgets through a library entry point. |
| 5  | CRUD on tasks + move/reorder + Insert mode                      | ✅ 9 e2e tests: `n`/`N`/`i` insert flow, `d` delete, `H`/`L` column moves, `K`/`J` within-column shifts. Mode-aware keymap, InputState-backed prompt, controller dispatches actions through `TaskService`. |
| 6  | Command mode, search, help overlay, `gw` two-char jump          | ✅ 17 e2e tests (+8 new): `:` command parser (`q`, `help`, `new-state`, `rename-state`, `delete-state`, `new-task`), live `/` search with filter preservation, `?` help overlay, `gw` two-char jump across visible tasks. |
| 7  | Tags + tag picker + filtering by tag                            | ✅ `t` opens tag-picker overlay; `[letter]` toggles attach/detach; `/#name` filters by tag; `:tag-new` / `:tag-delete` manage tags globally; tag chips render on cards. |
| 8  | Statistics service + Dashboard widget                           | Per-state sojourn, throughput, WIP view. |
| 9  | Config file (TOML) + catppuccin-frappe theme + keybind overrides | All keybinds/colors data-driven; invalid config warns, keeps running. |
| 10 | Postgres adapter + parameterised contract tests                 | Second backend live. |
| 11 | Polish, README, packaging                                       | v0.1.0 release. |

Each milestone ends with a green workspace (`cargo test --workspace`) and a runnable binary.

---

## 10. Crate-level `Cargo.toml` sketches

### Workspace root
```toml
[workspace]
resolver = "3"
members  = ["crates/core", "crates/store", "crates/widgets", "crates/kantui"]

[workspace.package]
edition      = "2024"
rust-version = "1.85"
license      = "MIT OR Apache-2.0"

[workspace.dependencies]
# core: only async-trait — everything else is for adapters / binary.
async-trait        = "0.1"
tokio              = { version = "1", features = ["rt-multi-thread", "macros", "signal"] }
thiserror          = "1"                              # adapters/binary only
uuid               = { version = "1", features = ["v4"] }
serde              = { version = "1", features = ["derive"] }
sqlx               = { version = "0.8", features = ["runtime-tokio", "macros"] }
ratatui            = "0.28"
crossterm          = "0.28"
clap               = { version = "4", features = ["derive"] }
directories        = "5"
toml               = "0.8"
tracing            = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
```

### `core` (no infra deps — this is the rule)
```toml
[dependencies]
async-trait = { workspace = true }
# intentionally nothing else: no sqlx, no ratatui, no uuid, no chrono, no thiserror.
# CoreError is hand-written; IDs are [u8; 16]; time is std::time::SystemTime.
```

### `store`
```toml
[features]
default  = ["sqlite"]
sqlite   = ["sqlx/sqlite"]
postgres = ["sqlx/postgres"]

[dependencies]
core        = { path = "../core" }
sqlx        = { workspace = true }
tokio       = { workspace = true }
async-trait = { workspace = true }
thiserror   = { workspace = true }
uuid        = { workspace = true }                    # generates IdGenerator bytes
```

### `widgets`
```toml
[dependencies]
core    = { path = "../core" }
ratatui = { workspace = true }
```

### `kantui`
```toml
[dependencies]
core               = { path = "../core" }
store              = { path = "../store" }
widgets            = { path = "../widgets" }
ratatui            = { workspace = true }
crossterm          = { workspace = true }
tokio              = { workspace = true }
clap               = { workspace = true }
directories        = { workspace = true }
toml               = { workspace = true }
serde              = { workspace = true }
tracing            = { workspace = true }
tracing-subscriber = { workspace = true }
uuid               = { workspace = true }             # concrete IdGenerator
```

---

## 11. Open questions (decide during M0–M1)

- Async or sync repo traits? — plan uses `async_trait` so the same trait covers SQLite and Postgres; the cost is an extra heap box per call, acceptable for a TUI.
- UUID v4 vs ULID for `EntityId` bytes? — UUID v4 in v1 (the `store` adapter decides); `core` only sees opaque `[u8; 16]`.
- Event-sourcing vs state-based persistence? — hybrid: live state is row-based (`tasks`), transitions are an append-only log (`task_transitions`). Enough for stats without the complexity of full event sourcing.
- Should "currently-occupied state time" be materialised or always computed? — always computed from `now - last_transition.at` in v1; if the dashboard gets slow, cache it in a background task.
