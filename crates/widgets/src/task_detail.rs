//! Task detail overlay: a modal panel showing every field on a task plus
//! per-state sojourn and timestamps. Read-only — edits are driven by the
//! binary's keymap/controller, which mutate the underlying task and rebuild
//! the view model.

use kantui_core::{Complexity, Priority, Timestamp};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use crate::task_card::format_due_date;
use crate::theme::{Theme, map_domain_color};
use crate::view::TagChip;

/// View model for [`TaskDetail`]. The binary assembles this per frame from
/// the currently-selected `Task` + cached sojourn.
#[derive(Debug, Clone, Copy)]
pub struct TaskDetailView<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub priority: Priority,
    pub complexity: Complexity,
    pub due_date: Option<Timestamp>,
    pub tags: &'a [TagChip<'a>],
    /// `(state_name, duration_secs)` pairs to render in the sojourn section.
    pub sojourn: &'a [(&'a str, u64)],
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Overlay widget. Draws a centered bordered box with the view contents.
pub struct TaskDetail<'a> {
    pub view: TaskDetailView<'a>,
    pub theme: &'a Theme,
}

impl<'a> TaskDetail<'a> {
    #[must_use]
    pub const fn new(view: TaskDetailView<'a>, theme: &'a Theme) -> Self {
        Self { view, theme }
    }
}

impl Widget for TaskDetail<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(area, 70, 80);
        Clear.render(popup, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(Span::styled(
                " Task ",
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(self.theme.border_focused))
            .style(Style::default().bg(self.theme.background));

        let inner = block.inner(popup);
        block.render(popup, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let lines = build_lines(&self.view, self.theme);

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(self.theme.background))
            .render(inner, buf);
    }
}

fn build_lines<'a>(view: &TaskDetailView<'a>, theme: &Theme) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();

    // Tags row — shown on top per UX request.
    if view.tags.is_empty() {
        lines.push(Line::from(Span::styled(
            "no tags",
            Style::default().fg(theme.muted),
        )));
    } else {
        let mut spans: Vec<Span<'a>> = Vec::with_capacity(view.tags.len() * 2);
        for (i, tag) in view.tags.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(
                format!("#{}", tag.name),
                Style::default().fg(map_domain_color(tag.color)),
            ));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::raw(""));

    // Title — bold, accent-coloured.
    lines.push(Line::from(Span::styled(
        view.title.to_owned(),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::raw(""));

    // Priority / complexity / due row.
    let priority_color = match view.priority {
        Priority::Low => theme.priority_low,
        Priority::Normal => theme.priority_normal,
        Priority::High => theme.priority_high,
        Priority::Critical => theme.priority_critical,
    };
    let priority_label = match view.priority {
        Priority::Low => "low",
        Priority::Normal => "normal",
        Priority::High => "high",
        Priority::Critical => "critical",
    };
    let complexity_label = match view.complexity {
        Complexity::Light => "light",
        Complexity::Deep => "deep",
    };
    let mut chips: Vec<Span<'a>> = vec![
        Span::styled("priority ", Style::default().fg(theme.muted)),
        Span::styled(
            priority_label.to_owned(),
            Style::default()
                .fg(priority_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled("complexity ", Style::default().fg(theme.muted)),
        Span::styled(
            complexity_label.to_owned(),
            Style::default().fg(theme.foreground),
        ),
    ];
    if let Some(due) = view.due_date {
        chips.push(Span::raw("   "));
        chips.push(Span::styled("due ", Style::default().fg(theme.muted)));
        chips.push(Span::styled(
            format_due_date(due),
            Style::default().fg(theme.foreground),
        ));
    }
    lines.push(Line::from(chips));
    lines.push(Line::raw(""));

    // Description section — may be absent.
    lines.push(Line::from(Span::styled(
        "Description",
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    )));
    if let Some(desc) = view.description {
        for raw in desc.lines() {
            lines.push(Line::from(Span::styled(
                raw.to_owned(),
                Style::default().fg(theme.foreground),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "(none — press `e` to add)",
            Style::default().fg(theme.muted),
        )));
    }
    lines.push(Line::raw(""));

    // Sojourn section.
    lines.push(Line::from(Span::styled(
        "Time per state",
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    )));
    if view.sojourn.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no transitions yet)",
            Style::default().fg(theme.muted),
        )));
    } else {
        let name_width = view
            .sojourn
            .iter()
            .map(|(n, _)| n.chars().count())
            .max()
            .unwrap_or(0)
            .max(5);
        for (name, secs) in view.sojourn {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {name:<width$}  ", name = name, width = name_width),
                    Style::default().fg(theme.foreground),
                ),
                Span::styled(fmt_duration(*secs), Style::default().fg(theme.accent)),
            ]));
        }
    }
    lines.push(Line::raw(""));

    // Timestamps.
    lines.push(Line::from(vec![
        Span::styled("created ", Style::default().fg(theme.muted)),
        Span::styled(
            format_due_date(view.created_at),
            Style::default().fg(theme.foreground),
        ),
        Span::raw("   "),
        Span::styled("updated ", Style::default().fg(theme.muted)),
        Span::styled(
            format_due_date(view.updated_at),
            Style::default().fg(theme.foreground),
        ),
    ]));
    lines.push(Line::raw(""));

    // Footer hint.
    lines.push(Line::from(Span::styled(
        "p priority · e description · D due · i title · t tags · Esc close",
        Style::default().fg(theme.muted),
    )));

    lines
}

fn fmt_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3600)
    }
}

/// Centered rect with the given percentages of `parent`.
fn centered_rect(parent: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let w = parent.width.saturating_mul(pct_x) / 100;
    let h = parent.height.saturating_mul(pct_y) / 100;
    let x = parent.x + (parent.width.saturating_sub(w)) / 2;
    let y = parent.y + (parent.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}
