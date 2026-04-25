//! User config file: TOML at `$XDG_CONFIG_HOME/kantui/config.toml`.
//!
//! The loader is deliberately lenient — unknown keys, bad colors, and
//! malformed keybinds become [`Warning`]s rather than fatal errors, so a
//! stale config still starts the app. Warnings are surfaced through the
//! tracing subscriber at `warn` level.
//!
//! Sections (see `plan.md` §7):
//!
//! ```toml
//! [general]
//! default_project = "Inbox"
//! done_state      = "Done"
//!
//! [theme]
//! name = "catppuccin-frappe"
//!
//! [theme.overrides]
//! background = "#303446"
//! accent     = "#babbf1"
//!
//! [keybinds.normal]
//! quit = "q"
//! focus_prev_column = "h"
//! # ...
//! ```

use std::fs;
use std::io;
use std::path::Path;

use kantui_widgets::Theme;
use ratatui::style::Color;
use toml::Value;

use crate::keybinds::{Binding, Keybinds, parse_binding};

/// Fully-resolved config. Construct via [`Config::default`] or [`load`].
#[derive(Debug, Clone)]
pub struct Config {
    pub general: General,
    pub theme: Theme,
    pub theme_name: String,
    pub keybinds: Keybinds,
}

#[derive(Debug, Clone)]
pub struct General {
    pub default_project: String,
    pub done_state: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: General::default(),
            theme: Theme::catppuccin_frappe(),
            theme_name: "catppuccin-frappe".to_owned(),
            keybinds: Keybinds::vim_default(),
        }
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            default_project: "kantui".to_owned(),
            done_state: "Done".to_owned(),
        }
    }
}

/// Warning emitted while loading a config. The app keeps running; these get
/// logged at `warn` level at startup.
pub type Warning = String;

/// Load a config from `path`. Missing file returns defaults silently; parse
/// errors and unknown/invalid fields become warnings and the offending value
/// falls back to its default.
pub fn load(path: &Path) -> (Config, Vec<Warning>) {
    let mut warnings: Vec<Warning> = Vec::new();

    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return (Config::default(), warnings);
        }
        Err(e) => {
            warnings.push(format!(
                "failed to read config at {}: {e} — using defaults",
                path.display()
            ));
            return (Config::default(), warnings);
        }
    };

    let value: Value = match toml::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!(
                "config parse error in {}: {e} — using defaults",
                path.display()
            ));
            return (Config::default(), warnings);
        }
    };

    (from_value(&value, &mut warnings), warnings)
}

/// Generate a default config file at `path`, creating the parent directory
/// if needed.
pub fn write_default(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_toml())
}

/// The canonical default config, as serialised TOML. Shipped verbatim by
/// `--gen-conf`.
#[must_use]
pub fn default_toml() -> String {
    // Hand-written so comments survive — toml-rs doesn't preserve them.
    r##"# kantui config. Unknown keys are warnings, not errors, so stale configs
# keep working across versions.

[general]
default_project = "kantui"
done_state      = "Done"

[theme]
# Built-in themes: "catppuccin-frappe", "catppuccin-mocha", "gruvbox-dark",
# "solarized-dark". Custom themes can be dropped into ~/.config/kantui/themes/.
name = "catppuccin-frappe"

# Optional per-role colour overrides. Hex #RRGGBB or named ratatui colours.
[theme.overrides]
# background = "#303446"
# foreground = "#c6d0f5"
# accent     = "#babbf1"
# selection  = "#414559"
# status_bar = "#51576d"

# Keybindings — single key ("h"), named key ("esc", "space", "enter", "tab"),
# modifier combo ("ctrl-s"), or two-key chord ("gg", "gw"). Unknown keys
# warn and keep the default binding.
[keybinds.normal]
quit                  = "q"
focus_prev_column     = "h"
focus_next_column     = "l"
select_next_task      = "j"
select_prev_task      = "k"
select_first_task     = "gg"
select_last_task      = "G"
begin_jump            = "gw"
open_dashboard        = "gs"
begin_new_task_below  = "n"
begin_new_task_above  = "N"
begin_rename_task     = "i"
delete_task           = "d"
move_task_prev_column = "H"
move_task_next_column = "L"
shift_task_up         = "K"
shift_task_down       = "J"
begin_tag_picker      = "t"
begin_command         = ":"
begin_search          = "/"
toggle_help           = "?"
open_task_detail      = "e"
open_project_picker   = "gp"
"##
    .to_owned()
}

fn from_value(root: &Value, warnings: &mut Vec<Warning>) -> Config {
    let mut config = Config::default();

    let table = match root.as_table() {
        Some(t) => t,
        None => {
            warnings.push("config root must be a TOML table — using defaults".to_owned());
            return config;
        }
    };

    for (k, v) in table {
        match k.as_str() {
            "general" => apply_general(&mut config.general, v, warnings),
            "theme" => apply_theme(&mut config, v, warnings),
            "keybinds" => apply_keybinds(&mut config.keybinds, v, warnings),
            other => warnings.push(format!("unknown config section `{other}`")),
        }
    }

    config
}

fn apply_general(general: &mut General, v: &Value, warnings: &mut Vec<Warning>) {
    let Some(tbl) = v.as_table() else {
        warnings.push("[general] must be a table".to_owned());
        return;
    };
    for (k, val) in tbl {
        match k.as_str() {
            "default_project" => {
                if let Some(s) = val.as_str() {
                    general.default_project = s.to_owned();
                } else {
                    warnings.push("general.default_project must be a string".to_owned());
                }
            }
            "done_state" => {
                if let Some(s) = val.as_str() {
                    general.done_state = s.to_owned();
                } else {
                    warnings.push("general.done_state must be a string".to_owned());
                }
            }
            other => warnings.push(format!("unknown key `general.{other}`")),
        }
    }
}

fn apply_theme(config: &mut Config, v: &Value, warnings: &mut Vec<Warning>) {
    let Some(tbl) = v.as_table() else {
        warnings.push("[theme] must be a table".to_owned());
        return;
    };
    for (k, val) in tbl {
        match k.as_str() {
            "name" => {
                let Some(s) = val.as_str() else {
                    warnings.push("theme.name must be a string".to_owned());
                    continue;
                };
                config.theme_name = s.to_owned();
                config.theme = builtin_theme(s).unwrap_or_else(|| {
                    warnings.push(format!(
                        "unknown theme `{s}` — falling back to catppuccin-frappe"
                    ));
                    Theme::catppuccin_frappe()
                });
            }
            "overrides" => apply_theme_overrides(&mut config.theme, val, warnings),
            other => warnings.push(format!("unknown key `theme.{other}`")),
        }
    }
}

/// Resolve a theme name to its built-in palette. `None` if unknown.
#[must_use]
pub fn builtin_theme(name: &str) -> Option<Theme> {
    match name {
        "catppuccin-frappe" | "frappe" | "default" => Some(Theme::catppuccin_frappe()),
        // Others listed in plan.md §7 aren't implemented yet; fall back so
        // the warning system highlights them.
        _ => None,
    }
}

fn apply_theme_overrides(theme: &mut Theme, v: &Value, warnings: &mut Vec<Warning>) {
    let Some(tbl) = v.as_table() else {
        warnings.push("[theme.overrides] must be a table".to_owned());
        return;
    };
    for (k, val) in tbl {
        let Some(s) = val.as_str() else {
            warnings.push(format!("theme.overrides.{k} must be a string"));
            continue;
        };
        let Some(color) = parse_color(s) else {
            warnings.push(format!(
                "theme.overrides.{k}: invalid colour `{s}` (expected #RRGGBB or named)"
            ));
            continue;
        };
        let slot = match k.as_str() {
            "background" => &mut theme.background,
            "foreground" => &mut theme.foreground,
            "muted" => &mut theme.muted,
            "accent" => &mut theme.accent,
            "selection" => &mut theme.selection,
            "status_bar" => &mut theme.status_bar,
            "status_bar_fg" => &mut theme.status_bar_fg,
            "border" => &mut theme.border,
            "border_focused" => &mut theme.border_focused,
            "mode_normal" => &mut theme.mode_normal,
            "mode_insert" => &mut theme.mode_insert,
            "mode_command" => &mut theme.mode_command,
            "mode_search" => &mut theme.mode_search,
            "priority_low" => &mut theme.priority_low,
            "priority_normal" => &mut theme.priority_normal,
            "priority_high" => &mut theme.priority_high,
            "priority_critical" => &mut theme.priority_critical,
            other => {
                warnings.push(format!("unknown theme role `theme.overrides.{other}`"));
                continue;
            }
        };
        *slot = color;
    }
}

/// Parse `#RRGGBB`, `#RGB`, or a handful of named colours.
#[must_use]
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    match s.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        _ => None,
    }
}

fn parse_hex(s: &str) -> Option<Color> {
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).ok()? * 0x11;
            let g = u8::from_str_radix(&s[1..2], 16).ok()? * 0x11;
            let b = u8::from_str_radix(&s[2..3], 16).ok()? * 0x11;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

fn apply_keybinds(binds: &mut Keybinds, v: &Value, warnings: &mut Vec<Warning>) {
    let Some(tbl) = v.as_table() else {
        warnings.push("[keybinds] must be a table".to_owned());
        return;
    };
    for (k, val) in tbl {
        match k.as_str() {
            "normal" => apply_normal_keybinds(binds, val, warnings),
            // [keybinds.insert] is reserved for future use; accept silently.
            "insert" => {}
            other => warnings.push(format!("unknown key `keybinds.{other}`")),
        }
    }
}

fn apply_normal_keybinds(binds: &mut Keybinds, v: &Value, warnings: &mut Vec<Warning>) {
    let Some(tbl) = v.as_table() else {
        warnings.push("[keybinds.normal] must be a table".to_owned());
        return;
    };
    for (k, val) in tbl {
        let Some(spec) = val.as_str() else {
            warnings.push(format!("keybinds.normal.{k} must be a string"));
            continue;
        };
        let binding = match parse_binding(spec) {
            Ok(b) => b,
            Err(e) => {
                warnings.push(format!(
                    "keybinds.normal.{k}: invalid binding `{spec}` ({e}) — keeping default"
                ));
                continue;
            }
        };
        if let Some(slot) = binding_slot(binds, k.as_str()) {
            *slot = vec![binding];
        } else {
            warnings.push(format!("unknown action `keybinds.normal.{k}`"));
        }
    }
}

fn binding_slot<'a>(binds: &'a mut Keybinds, name: &str) -> Option<&'a mut Vec<Binding>> {
    Some(match name {
        "quit" => &mut binds.quit,
        "focus_prev_column" => &mut binds.focus_prev_column,
        "focus_next_column" => &mut binds.focus_next_column,
        "select_next_task" => &mut binds.select_next_task,
        "select_prev_task" => &mut binds.select_prev_task,
        "select_first_task" => &mut binds.select_first_task,
        "select_last_task" => &mut binds.select_last_task,
        "begin_jump" => &mut binds.begin_jump,
        "open_dashboard" => &mut binds.open_dashboard,
        "begin_new_task_below" => &mut binds.begin_new_task_below,
        "begin_new_task_above" => &mut binds.begin_new_task_above,
        "begin_rename_task" => &mut binds.begin_rename_task,
        "delete_task" => &mut binds.delete_task,
        "move_task_prev_column" => &mut binds.move_task_prev_column,
        "move_task_next_column" => &mut binds.move_task_next_column,
        "shift_task_up" => &mut binds.shift_task_up,
        "shift_task_down" => &mut binds.shift_task_down,
        "begin_tag_picker" => &mut binds.begin_tag_picker,
        "begin_command" => &mut binds.begin_command,
        "begin_search" => &mut binds.begin_search,
        "toggle_help" => &mut binds.toggle_help,
        "open_task_detail" => &mut binds.open_task_detail,
        "open_project_picker" => &mut binds.open_project_picker,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn parse(toml: &str) -> (Config, Vec<Warning>) {
        let mut warnings = Vec::new();
        let value: Value = toml::from_str(toml).expect("test toml must parse");
        (from_value(&value, &mut warnings), warnings)
    }

    #[test]
    fn defaults_are_frappe_and_vim() {
        let c = Config::default();
        assert_eq!(c.theme_name, "catppuccin-frappe");
        assert_eq!(c.theme.background, Color::Rgb(0x30, 0x34, 0x46));
        // Quit defaults to "q".
        let Binding::Single(k) = c.keybinds.quit[0] else {
            panic!("expected single");
        };
        assert_eq!(k.code, KeyCode::Char('q'));
    }

    #[test]
    fn unknown_section_warns() {
        let (_, w) = parse("[what]\nfoo = 1\n");
        assert!(w.iter().any(|m| m.contains("unknown config section")));
    }

    #[test]
    fn unknown_general_key_warns() {
        let (_, w) = parse("[general]\nbogus = 1\n");
        assert!(w.iter().any(|m| m.contains("unknown key `general.bogus`")));
    }

    #[test]
    fn theme_override_hex_applied() {
        let (c, w) = parse("[theme.overrides]\naccent = \"#ff00aa\"\n");
        assert!(w.is_empty(), "unexpected warnings: {w:?}");
        assert_eq!(c.theme.accent, Color::Rgb(0xff, 0x00, 0xaa));
    }

    #[test]
    fn theme_override_bad_hex_warns_and_preserves_default() {
        let (c, w) = parse("[theme.overrides]\naccent = \"not-a-color\"\n");
        assert!(w.iter().any(|m| m.contains("invalid colour")), "got {w:?}");
        // Unchanged from frappe default.
        assert_eq!(c.theme.accent, Color::Rgb(0xba, 0xbb, 0xf1));
    }

    #[test]
    fn unknown_theme_name_warns() {
        let (c, w) = parse("[theme]\nname = \"solarized-dark\"\n");
        assert!(w.iter().any(|m| m.contains("unknown theme")));
        // Falls back to frappe.
        assert_eq!(c.theme.background, Color::Rgb(0x30, 0x34, 0x46));
    }

    #[test]
    fn keybind_override_applied() {
        let (c, w) = parse("[keybinds.normal]\nquit = \"Q\"\n");
        assert!(w.is_empty(), "got {w:?}");
        let Binding::Single(k) = c.keybinds.quit[0] else {
            panic!();
        };
        assert_eq!(k.code, KeyCode::Char('Q'));
        assert!(k.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn keybind_bad_spec_warns_and_keeps_default() {
        let (c, w) = parse("[keybinds.normal]\nquit = \"hyper-q\"\n");
        assert!(w.iter().any(|m| m.contains("invalid binding")), "got {w:?}");
        let Binding::Single(k) = c.keybinds.quit[0] else {
            panic!();
        };
        assert_eq!(k.code, KeyCode::Char('q'));
    }

    #[test]
    fn unknown_action_warns() {
        let (_, w) = parse("[keybinds.normal]\nmake_coffee = \"c\"\n");
        assert!(w.iter().any(|m| m.contains("unknown action")));
    }

    #[test]
    fn default_toml_parses_cleanly() {
        let (_, w) = parse(&default_toml());
        assert!(w.is_empty(), "default_toml produced warnings: {w:?}");
    }

    #[test]
    fn parse_color_named() {
        assert_eq!(parse_color("red"), Some(Color::Red));
        assert_eq!(parse_color("DarkGray"), Some(Color::DarkGray));
    }

    #[test]
    fn parse_color_hex3() {
        assert_eq!(parse_color("#f00"), Some(Color::Rgb(0xff, 0x00, 0x00)));
    }

    #[test]
    fn write_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("kantui-cfg-{}", std::process::id()));
        let path = dir.join("config.toml");
        write_default(&path).expect("write default");
        let (config, warnings) = load(&path);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(config.theme_name, "catppuccin-frappe");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_returns_defaults_silently() {
        let path = std::env::temp_dir().join("kantui-nonexistent-config-xyz.toml");
        // Ensure it's absent.
        let _ = fs::remove_file(&path);
        let (config, warnings) = load(&path);
        assert!(warnings.is_empty());
        assert_eq!(config.theme_name, "catppuccin-frappe");
    }

    #[test]
    fn malformed_toml_warns_and_returns_defaults() {
        let dir = std::env::temp_dir().join(format!("kantui-bad-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, "this is not valid = = = toml").unwrap();
        let (config, warnings) = load(&path);
        assert!(warnings.iter().any(|w| w.contains("parse error")));
        assert_eq!(config.theme_name, "catppuccin-frappe");
        fs::remove_dir_all(&dir).ok();
    }
}
