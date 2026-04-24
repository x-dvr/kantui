//! Theme palette + domain-to-ratatui color mapping.
//!
//! The built-in default is catppuccin-frappe. Callers can construct custom
//! themes directly or (later) load them from TOML in the binary crate. This
//! module keeps the mapping from domain [`core::Color`] to `ratatui::style::Color`
//! in a single place so every widget renders the same way.

use kantui_core::Color as DomainColor;
use ratatui::style::Color;

/// Palette of role colors used by widgets. Every role has a default value so
/// widgets can render without per-role overrides.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub muted: Color,
    pub accent: Color,
    pub selection: Color,
    pub status_bar: Color,
    pub status_bar_fg: Color,
    pub border: Color,
    pub border_focused: Color,
    pub mode_normal: Color,
    pub mode_insert: Color,
    pub mode_command: Color,
    pub mode_search: Color,
    pub priority_low: Color,
    pub priority_normal: Color,
    pub priority_high: Color,
    pub priority_critical: Color,
}

impl Theme {
    /// Catppuccin Frappé — the project-wide default.
    #[must_use]
    pub const fn catppuccin_frappe() -> Self {
        Self {
            background: Color::Rgb(0x30, 0x34, 0x46),
            foreground: Color::Rgb(0xc6, 0xd0, 0xf5),
            muted: Color::Rgb(0x83, 0x8b, 0xa7),
            accent: Color::Rgb(0xba, 0xbb, 0xf1),
            selection: Color::Rgb(0x41, 0x45, 0x59),
            status_bar: Color::Rgb(0x51, 0x57, 0x6d),
            status_bar_fg: Color::Rgb(0xc6, 0xd0, 0xf5),
            border: Color::Rgb(0x62, 0x68, 0x80),
            border_focused: Color::Rgb(0xba, 0xbb, 0xf1),
            mode_normal: Color::Rgb(0x8c, 0xaa, 0xee),
            mode_insert: Color::Rgb(0xa6, 0xd1, 0x89),
            mode_command: Color::Rgb(0xe5, 0xc8, 0x90),
            mode_search: Color::Rgb(0xef, 0x9f, 0x76),
            priority_low: Color::Rgb(0x83, 0x8b, 0xa7),
            priority_normal: Color::Rgb(0xc6, 0xd0, 0xf5),
            priority_high: Color::Rgb(0xe5, 0xc8, 0x90),
            priority_critical: Color::Rgb(0xe7, 0x82, 0x84),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::catppuccin_frappe()
    }
}

/// Map a domain [`DomainColor`] to the terminal color used for rendering.
#[must_use]
pub fn map_domain_color(c: DomainColor) -> Color {
    match c {
        DomainColor::Red => Color::Red,
        DomainColor::Green => Color::Green,
        DomainColor::Yellow => Color::Yellow,
        DomainColor::Blue => Color::Blue,
        DomainColor::Magenta => Color::Magenta,
        DomainColor::Cyan => Color::Cyan,
        DomainColor::White => Color::White,
        DomainColor::Gray => Color::Gray,
        DomainColor::Custom([r, g, b]) => Color::Rgb(r, g, b),
    }
}
