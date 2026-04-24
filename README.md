# kantui

A terminal kanban board in Rust. Vim-inspired modal keyboard control,
SQLite-backed persistence, live per-state sojourn statistics, and a
catppuccin-frappe theme out of the box.

```
┌─ Todo ─────┬─ Doing ────┬─ Done ─────┐
│ Wire event │ Render     │ Scaffold   │
│ loop       │ board      │ workspace  │
│            │            │            │
│ Read       │            │            │
│ CLAUDE.md  │            │            │
└────────────┴────────────┴────────────┘
 NOR  kantui › Todo  Wire event loop    4 / 1   12:34
```

## Install

```
cargo install --path crates/kantui
```

Requires stable Rust (MSRV 1.85, Rust 2024 edition). A pinned channel lives
in `rust-toolchain.toml`.

## Run

### From source (no install)

```bash
cargo run -p kantui -- --seed-demo        # first run with sample tasks
cargo run -p kantui                       # subsequent runs (data persists in ~/.local/share/kantui/)
cargo run --release -p kantui -- --seed-demo   # snappier
```

### After `cargo install`

```
kantui                          # default SQLite under $XDG_DATA_HOME/kantui/
kantui --seed-demo              # seed an example project on first run
kantui --db sqlite:///tmp/k.db  # isolated DB for experimenting
kantui --gen-conf               # write default config to $XDG_CONFIG_HOME/kantui/config.toml
kantui --log-level debug        # more verbose log file
```

On first launch an empty project is created with `Todo` / `Doing` / `Done`
columns. `--seed-demo` also inserts a handful of sample tasks.

Inside the app: `?` for help, `j`/`k`/`h`/`l` to navigate, `n` to add a task,
`:` for commands, `q` to quit. The log file is at `~/.cache/kantui/kantui.log`
— `tail -f` it in another terminal if something misbehaves.

## Keybindings

Normal mode (see `?` in-app for the live cheatsheet):

| Keys            | Action                                   |
|-----------------|------------------------------------------|
| `h` / `l`       | Focus previous / next column             |
| `j` / `k`       | Select next / previous task              |
| `gg` / `G`      | Top / bottom of column                   |
| `gw`            | Two-character jump (Helix-style)         |
| `gs`            | Open statistics dashboard                |
| `n` / `N`       | New task below / above selection         |
| `i`             | Rename selected task                     |
| `d`             | Delete selected task                     |
| `H` / `L`       | Move task to previous / next column      |
| `K` / `J`       | Shift task up / down within column       |
| `t`             | Tag picker (toggle tags on selected task)|
| `:`             | Command mode                             |
| `/`             | Search (live; `#tag` filters by tag)     |
| `?`             | Toggle help overlay                      |
| `q` / `Ctrl-C`  | Quit                                     |

Command mode (`:`) accepts: `q`, `help`, `new-state <name>`,
`rename-state <name>`, `delete-state`, `new-task <title>`,
`tag-new <name> [color]`, `tag-delete <name>`.

Every binding can be overridden in the config file — see below.

## Configuration

Config lives at `$XDG_CONFIG_HOME/kantui/config.toml` (usually
`~/.config/kantui/config.toml`). Run `kantui --gen-conf` to drop an annotated
default in place, then edit.

Unknown sections, unknown keys, invalid colours, or malformed keybinds
produce **warnings in the log**, not errors — a stale config keeps working
across versions.

```toml
[general]
default_project = "kantui"
done_state      = "Done"

[theme]
name = "catppuccin-frappe"

[theme.overrides]
accent    = "#ca9ee6"
selection = "#414559"

[keybinds.normal]
quit                  = "q"
focus_prev_column     = "h"
focus_next_column     = "l"
select_first_task     = "gg"   # chord
begin_jump            = "gw"
open_dashboard        = "gs"
move_task_prev_column = "H"    # uppercase == shifted
# ...
```

Key spec syntax: single char (`"h"`), named key (`"esc"`, `"space"`,
`"enter"`, `"tab"`, `"left"`, ...), modifier combo (`"ctrl-s"`), or a
two-character chord (`"gg"`, `"gw"`).

## CLI flags

| Flag                | Default                                  |
|---------------------|------------------------------------------|
| `--db <url>`        | `sqlite://$XDG_DATA_HOME/kantui/kantui.db` |
| `--config <path>`   | `$XDG_CONFIG_HOME/kantui/config.toml`    |
| `--log <path>`      | `$XDG_CACHE_HOME/kantui/kantui.log`      |
| `--log-level <lvl>` | `info` (`error` / `warn` / `debug` / `trace`) |
| `--gen-conf`        | Write default config and exit            |
| `--seed-demo`       | Seed an example project on first run     |

## Architecture

Strictly hexagonal workspace. See `CLAUDE.md` and `plan.md` for the full
story.

```
kantui (binary, event loop, keymap, config)
  │
  ├── widgets (ratatui views — read-only view models)
  └── store   (sqlx/SQLite adapter)
         │
         └── core (domain + ports + services — zero infra deps)
```

Dependency direction is one-way. `core` has no dependency on `ratatui`,
`sqlx`, or any workspace crate — only `std` + `async-trait`. Errors flow
through a hand-written `CoreError` with a multi-line `log_chain()` for the
logfile.

## Development

```bash
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p kantui -- --seed-demo
```

## License

Dual-licensed under MIT or Apache-2.0, at your option.
