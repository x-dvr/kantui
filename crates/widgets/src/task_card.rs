//! Task card: title + glyphs + due-date + tag chips. Drawn as a bordered box
//! inside a state column. Selection highlight is handled by the parent column.

use kantui_core::{Complexity, Priority, Timestamp};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use crate::theme::{Theme, map_domain_color};
use crate::view::TaskCardView;

/// Ratatui widget rendering a single task card.
pub struct TaskCard<'a> {
    pub view: TaskCardView<'a>,
    pub theme: &'a Theme,
    pub selected: bool,
}

impl<'a> TaskCard<'a> {
    #[must_use]
    pub const fn new(view: TaskCardView<'a>, theme: &'a Theme) -> Self {
        Self {
            view,
            theme,
            selected: false,
        }
    }

    #[must_use]
    pub const fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl Widget for TaskCard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title_line = build_title_line(&self.view, self.theme);
        let meta_line = build_meta_line(&self.view, self.theme);

        let mut lines = vec![title_line];
        if let Some(line) = meta_line {
            lines.push(line);
        }
        if !self.view.tags.is_empty() {
            lines.push(build_tags_line(&self.view));
        }

        let border_color = if self.selected {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let base_style = if self.selected {
            Style::default()
                .fg(self.theme.foreground)
                .bg(self.theme.selection)
        } else {
            Style::default().fg(self.theme.foreground)
        };

        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: true })
            .style(base_style)
            .render(area, buf);
    }
}

fn build_title_line<'a>(view: &TaskCardView<'a>, theme: &Theme) -> Line<'a> {
    let priority_color = match view.priority {
        Priority::Low => theme.priority_low,
        Priority::Normal => theme.priority_normal,
        Priority::High => theme.priority_high,
        Priority::Critical => theme.priority_critical,
    };
    let priority_glyph = match view.priority {
        Priority::Low => "↓",
        Priority::Normal => "·",
        Priority::High => "↑",
        Priority::Critical => "!",
    };
    let complexity_glyph = match view.complexity {
        Complexity::Light => "○",
        Complexity::Deep => "●",
    };

    Line::from(vec![
        Span::styled(
            priority_glyph.to_string(),
            Style::default()
                .fg(priority_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            complexity_glyph.to_string(),
            Style::default().fg(theme.muted),
        ),
        Span::raw(" "),
        Span::styled(
            view.title.to_owned(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ])
}

fn build_meta_line(view: &TaskCardView<'_>, theme: &Theme) -> Option<Line<'static>> {
    let due = view.due_date?;
    let label = format_due_date(due);
    Some(Line::from(vec![
        Span::styled("due ", Style::default().fg(theme.muted)),
        Span::styled(label, Style::default().fg(theme.foreground)),
    ]))
}

fn build_tags_line<'a>(view: &TaskCardView<'a>) -> Line<'a> {
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
    Line::from(spans)
}

/// `YYYY-MM-DD` formatting — sufficient for M3. Locale formatting is the
/// binary's problem.
fn format_due_date(ts: Timestamp) -> String {
    use std::time::UNIX_EPOCH;
    let secs = ts
        .to_system_time()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_date(secs);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Convert unix-epoch seconds into `(year, month, day)` using Howard Hinnant's
/// civil-from-days algorithm. Avoids pulling in `chrono`/`time` just for a
/// formatter.
fn civil_date(secs: i64) -> (i32, u32, u32) {
    let days = secs.div_euclid(86_400);
    // Shift epoch to 0000-03-01 so the Feb 29 leap day lands at year-end.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::civil_date;

    #[test]
    fn civil_date_unix_epoch() {
        assert_eq!(civil_date(0), (1970, 1, 1));
    }

    #[test]
    fn civil_date_leap_day() {
        // 2020-02-29 00:00:00 UTC = 1_582_934_400
        assert_eq!(civil_date(1_582_934_400), (2020, 2, 29));
    }

    #[test]
    fn civil_date_known_date() {
        // 2026-04-20 00:00:00 UTC = 1_776_643_200
        assert_eq!(civil_date(1_776_643_200), (2026, 4, 20));
    }
}
