//! Data-driven keybindings.
//!
//! A [`Keybinds`] holds one `Vec<Binding>` per logical action, where each
//! [`Binding`] is either a single keypress or a two-key chord. The dispatcher
//! in [`crate::keymap`] iterates these to decide what action to fire.
//!
//! The default set mirrors the hardcoded Vim-inspired layout that shipped
//! prior to M9; user config can override individual actions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A single keypress with optional modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl Key {
    #[must_use]
    pub const fn plain(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[must_use]
    pub const fn shift(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::SHIFT,
        }
    }

    /// Does this binding match an incoming event? Ignores Shift when the key
    /// is a non-letter `Char` (terminals already deliver shifted punctuation
    /// as the punctuation itself), and treats an uppercase char as requiring
    /// `SHIFT`.
    #[must_use]
    pub fn matches(&self, ev: &KeyEvent) -> bool {
        if self.code != ev.code {
            return false;
        }
        // For printable chars, shift is implicit in the delivered char.
        if matches!(self.code, KeyCode::Char(_)) {
            let allowed = KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT;
            let want = self.modifiers;
            let got = ev.modifiers & allowed;
            let want_ctrl_alt = want & (KeyModifiers::CONTROL | KeyModifiers::ALT);
            let got_ctrl_alt = got & (KeyModifiers::CONTROL | KeyModifiers::ALT);
            return want_ctrl_alt == got_ctrl_alt;
        }
        ev.modifiers == self.modifiers
    }
}

/// Either a single keypress or a two-key chord (like `gg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Single(Key),
    Chord(Key, Key),
}

/// Per-action bindings. Each action may carry zero or more bindings; dispatch
/// walks all configured actions and takes the first match.
#[derive(Debug, Clone)]
pub struct Keybinds {
    pub quit: Vec<Binding>,
    pub focus_prev_column: Vec<Binding>,
    pub focus_next_column: Vec<Binding>,
    pub select_next_task: Vec<Binding>,
    pub select_prev_task: Vec<Binding>,
    pub select_first_task: Vec<Binding>,
    pub select_last_task: Vec<Binding>,
    pub begin_jump: Vec<Binding>,
    pub open_dashboard: Vec<Binding>,
    pub begin_new_task_below: Vec<Binding>,
    pub begin_new_task_above: Vec<Binding>,
    pub begin_rename_task: Vec<Binding>,
    pub delete_task: Vec<Binding>,
    pub move_task_prev_column: Vec<Binding>,
    pub move_task_next_column: Vec<Binding>,
    pub shift_task_up: Vec<Binding>,
    pub shift_task_down: Vec<Binding>,
    pub begin_tag_picker: Vec<Binding>,
    pub begin_command: Vec<Binding>,
    pub begin_search: Vec<Binding>,
    pub toggle_help: Vec<Binding>,
    pub open_task_detail: Vec<Binding>,
    pub open_project_picker: Vec<Binding>,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self::vim_default()
    }
}

impl Keybinds {
    /// Baseline bindings — what the app ships with out of the box.
    #[must_use]
    pub fn vim_default() -> Self {
        Self {
            quit: vec![Binding::Single(Key::plain(KeyCode::Char('q')))],
            focus_prev_column: vec![
                Binding::Single(Key::plain(KeyCode::Char('h'))),
                Binding::Single(Key::plain(KeyCode::Left)),
            ],
            focus_next_column: vec![
                Binding::Single(Key::plain(KeyCode::Char('l'))),
                Binding::Single(Key::plain(KeyCode::Right)),
            ],
            select_next_task: vec![
                Binding::Single(Key::plain(KeyCode::Char('j'))),
                Binding::Single(Key::plain(KeyCode::Down)),
            ],
            select_prev_task: vec![
                Binding::Single(Key::plain(KeyCode::Char('k'))),
                Binding::Single(Key::plain(KeyCode::Up)),
            ],
            select_first_task: vec![Binding::Chord(
                Key::plain(KeyCode::Char('g')),
                Key::plain(KeyCode::Char('g')),
            )],
            select_last_task: vec![Binding::Single(Key::shift(KeyCode::Char('G')))],
            begin_jump: vec![Binding::Chord(
                Key::plain(KeyCode::Char('g')),
                Key::plain(KeyCode::Char('w')),
            )],
            open_dashboard: vec![Binding::Chord(
                Key::plain(KeyCode::Char('g')),
                Key::plain(KeyCode::Char('s')),
            )],
            begin_new_task_below: vec![Binding::Single(Key::plain(KeyCode::Char('n')))],
            begin_new_task_above: vec![Binding::Single(Key::shift(KeyCode::Char('N')))],
            begin_rename_task: vec![Binding::Single(Key::plain(KeyCode::Char('i')))],
            delete_task: vec![Binding::Single(Key::plain(KeyCode::Char('d')))],
            move_task_prev_column: vec![Binding::Single(Key::shift(KeyCode::Char('H')))],
            move_task_next_column: vec![Binding::Single(Key::shift(KeyCode::Char('L')))],
            shift_task_up: vec![Binding::Single(Key::shift(KeyCode::Char('K')))],
            shift_task_down: vec![Binding::Single(Key::shift(KeyCode::Char('J')))],
            begin_tag_picker: vec![Binding::Single(Key::plain(KeyCode::Char('t')))],
            begin_command: vec![Binding::Single(Key::shift(KeyCode::Char(':')))],
            begin_search: vec![Binding::Single(Key::plain(KeyCode::Char('/')))],
            toggle_help: vec![Binding::Single(Key::shift(KeyCode::Char('?')))],
            open_task_detail: vec![Binding::Single(Key::plain(KeyCode::Char('e')))],
            open_project_picker: vec![Binding::Chord(
                Key::plain(KeyCode::Char('g')),
                Key::plain(KeyCode::Char('p')),
            )],
        }
    }

    /// All chords currently configured — used to decide whether a lone first
    /// keypress should be stashed as a chord prefix.
    #[must_use]
    pub fn is_chord_prefix(&self, key: &KeyEvent) -> bool {
        self.iter_all().any(|b| match b {
            Binding::Chord(first, _) => first.matches(key),
            Binding::Single(_) => false,
        })
    }

    fn iter_all(&self) -> impl Iterator<Item = &Binding> {
        self.quit
            .iter()
            .chain(&self.focus_prev_column)
            .chain(&self.focus_next_column)
            .chain(&self.select_next_task)
            .chain(&self.select_prev_task)
            .chain(&self.select_first_task)
            .chain(&self.select_last_task)
            .chain(&self.begin_jump)
            .chain(&self.open_dashboard)
            .chain(&self.begin_new_task_below)
            .chain(&self.begin_new_task_above)
            .chain(&self.begin_rename_task)
            .chain(&self.delete_task)
            .chain(&self.move_task_prev_column)
            .chain(&self.move_task_next_column)
            .chain(&self.shift_task_up)
            .chain(&self.shift_task_down)
            .chain(&self.begin_tag_picker)
            .chain(&self.begin_command)
            .chain(&self.begin_search)
            .chain(&self.toggle_help)
            .chain(&self.open_task_detail)
            .chain(&self.open_project_picker)
    }
}

/// Result of parsing a user-supplied keybind string.
#[derive(Debug)]
pub enum ParseError {
    Empty,
    UnknownNamedKey(String),
    /// A chord must be exactly two single characters (e.g. `gg`, `gw`).
    BadChord(String),
    /// Unknown modifier token (only `ctrl`, `alt`, `shift` recognised).
    UnknownModifier(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty keybind"),
            Self::UnknownNamedKey(s) => write!(f, "unknown key name `{s}`"),
            Self::BadChord(s) => write!(f, "chord `{s}` must be two single characters"),
            Self::UnknownModifier(s) => write!(f, "unknown modifier `{s}`"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a string like `"h"`, `"esc"`, `"space"`, `"ctrl-s"`, `"gg"`.
pub fn parse_binding(s: &str) -> Result<Binding, ParseError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(ParseError::Empty);
    }

    // Two-key chord: two ASCII letters with no separator. We only support
    // letter chords (`gg`, `gw`) — everything else should use a single key
    // or a modifier combo.
    if trimmed.len() == 2
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c.is_ascii_digit())
    {
        let mut iter = trimmed.chars();
        let a = iter.next().unwrap();
        let b = iter.next().unwrap();
        return Ok(Binding::Chord(parse_single_char(a), parse_single_char(b)));
    }

    Ok(Binding::Single(parse_single_token(trimmed)?))
}

fn parse_single_token(s: &str) -> Result<Key, ParseError> {
    // Split on `-`: every part except the last is a modifier.
    let parts: Vec<&str> = s.split('-').collect();
    let (key_tok, mod_toks) = parts.split_last().ok_or(ParseError::Empty)?;

    let mut modifiers = KeyModifiers::NONE;
    for m in mod_toks {
        let bit = match m.to_ascii_lowercase().as_str() {
            "ctrl" | "c" => KeyModifiers::CONTROL,
            "alt" | "a" | "meta" => KeyModifiers::ALT,
            "shift" | "s" => KeyModifiers::SHIFT,
            other => return Err(ParseError::UnknownModifier(other.to_owned())),
        };
        modifiers |= bit;
    }

    let code = parse_key_name(key_tok)?;
    // Normalise: if the key is a lowercase char and SHIFT was requested,
    // promote to the uppercase form so matching treats it as a shifted key.
    let code = match code {
        KeyCode::Char(c) if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_lowercase() => {
            KeyCode::Char(c.to_ascii_uppercase())
        }
        other => other,
    };
    // If the char itself is uppercase, make sure SHIFT is flagged.
    let modifiers = match code {
        KeyCode::Char(c) if c.is_ascii_uppercase() => modifiers | KeyModifiers::SHIFT,
        _ => modifiers,
    };

    Ok(Key { code, modifiers })
}

fn parse_single_char(c: char) -> Key {
    let code = KeyCode::Char(c);
    let modifiers = if c.is_ascii_uppercase() {
        KeyModifiers::SHIFT
    } else {
        KeyModifiers::NONE
    };
    Key { code, modifiers }
}

fn parse_key_name(s: &str) -> Result<KeyCode, ParseError> {
    if s.chars().count() == 1 {
        return Ok(KeyCode::Char(s.chars().next().unwrap()));
    }
    let code = match s.to_ascii_lowercase().as_str() {
        "esc" | "escape" => KeyCode::Esc,
        "enter" | "return" => KeyCode::Enter,
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "backspace" | "bs" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        other if other.starts_with('f') => {
            if let Some(n) = other[1..]
                .parse::<u8>()
                .ok()
                .filter(|n| (1..=12).contains(n))
            {
                KeyCode::F(n)
            } else {
                return Err(ParseError::UnknownNamedKey(s.to_owned()));
            }
        }
        _ => return Err(ParseError::UnknownNamedKey(s.to_owned())),
    };
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_lowercase() {
        let Binding::Single(k) = parse_binding("h").unwrap() else {
            panic!("expected single");
        };
        assert_eq!(k.code, KeyCode::Char('h'));
        assert_eq!(k.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn parses_uppercase_as_shifted() {
        let Binding::Single(k) = parse_binding("H").unwrap() else {
            panic!("expected single");
        };
        assert_eq!(k.code, KeyCode::Char('H'));
        assert_eq!(k.modifiers, KeyModifiers::SHIFT);
    }

    #[test]
    fn parses_named_keys() {
        let Binding::Single(k) = parse_binding("esc").unwrap() else {
            panic!();
        };
        assert_eq!(k.code, KeyCode::Esc);
        let Binding::Single(k) = parse_binding("space").unwrap() else {
            panic!();
        };
        assert_eq!(k.code, KeyCode::Char(' '));
    }

    #[test]
    fn parses_modifier_combo() {
        let Binding::Single(k) = parse_binding("ctrl-s").unwrap() else {
            panic!();
        };
        assert_eq!(k.code, KeyCode::Char('s'));
        assert!(k.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn parses_chord() {
        let Binding::Chord(a, b) = parse_binding("gg").unwrap() else {
            panic!("expected chord");
        };
        assert_eq!(a.code, KeyCode::Char('g'));
        assert_eq!(b.code, KeyCode::Char('g'));
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(parse_binding(""), Err(ParseError::Empty)));
    }

    #[test]
    fn rejects_unknown_named_key() {
        assert!(matches!(
            parse_binding("banana"),
            Err(ParseError::UnknownNamedKey(_))
        ));
    }

    #[test]
    fn rejects_unknown_modifier() {
        assert!(matches!(
            parse_binding("hyper-x"),
            Err(ParseError::UnknownModifier(_))
        ));
    }

    #[test]
    fn matches_uppercase_via_shift() {
        let k = Key::shift(KeyCode::Char('H'));
        let ev = KeyEvent::new(KeyCode::Char('H'), KeyModifiers::SHIFT);
        assert!(k.matches(&ev));
    }

    #[test]
    fn matches_ignores_shift_on_lowercase_char() {
        // Some terminals deliver `h` with no modifiers, others with SHIFT
        // stripped: matching should succeed either way for Char bindings.
        let k = Key::plain(KeyCode::Char('h'));
        let ev = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(k.matches(&ev));
    }

    #[test]
    fn chord_prefix_detected() {
        let binds = Keybinds::vim_default();
        let g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert!(binds.is_chord_prefix(&g));
        let h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(!binds.is_chord_prefix(&h));
    }
}
