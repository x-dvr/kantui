//! Project dashboard: per-state sojourn stats and a simple throughput bar
//! chart. Renders as a centered modal, mirroring [`crate::HelpOverlay`].

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::theme::Theme;

/// One row of the per-state section.
#[derive(Debug, Clone, Copy)]
pub struct DashboardStateRow<'a> {
    pub name: &'a str,
    /// Current task count (live).
    pub tasks: u32,
    pub wip_limit: Option<u32>,
    /// Total time tasks have spent in this state.
    pub total_seconds: u64,
    /// How many visits contributed to `total_seconds`.
    pub visits: u32,
}

/// Per-day throughput histogram. `per_day[0]` is the oldest day; the last
/// entry is today.
#[derive(Debug, Clone, Copy)]
pub struct DashboardThroughput<'a> {
    pub total: u32,
    pub per_day: &'a [u32],
}

#[derive(Debug, Clone, Copy)]
pub struct DashboardView<'a> {
    pub project_name: &'a str,
    pub states: &'a [DashboardStateRow<'a>],
    pub throughput: DashboardThroughput<'a>,
}

pub struct Dashboard<'a> {
    pub view: DashboardView<'a>,
    pub theme: &'a Theme,
}

impl<'a> Dashboard<'a> {
    #[must_use]
    pub const fn new(view: DashboardView<'a>, theme: &'a Theme) -> Self {
        Self { view, theme }
    }
}

impl Widget for Dashboard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(area, 70, 80);
        Clear.render(popup, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(Span::styled(
                format!(" Dashboard — {} ", self.view.project_name),
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

        let states_height = (self.view.states.len() as u16).saturating_add(2);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(states_height.min(inner.height)),
                Constraint::Min(0),
            ])
            .split(inner);

        render_states(chunks[0], buf, self.view.states, self.theme);
        if chunks.len() > 1 {
            render_throughput(chunks[1], buf, self.view.throughput, self.theme);
        }
    }
}

fn render_states(area: Rect, buf: &mut Buffer, rows: &[DashboardStateRow<'_>], theme: &Theme) {
    let name_width = rows
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(5) as u16;

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(rows.len() + 2);
    lines.push(Line::from(vec![Span::styled(
        format!(
            " {:width$}   tasks   total   avg   visits ",
            "state",
            width = name_width as usize
        ),
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    )]));

    for row in rows {
        let tasks_str = match row.wip_limit {
            Some(limit) => format!("{}/{}", row.tasks, limit),
            None => format!("{}", row.tasks),
        };
        let avg_seconds = if row.visits > 0 {
            row.total_seconds / row.visits as u64
        } else {
            0
        };
        let line = format!(
            " {name:<name_w$}   {tasks:>5}   {total:>5}   {avg:>3}   {visits:>6} ",
            name = row.name,
            name_w = name_width as usize,
            tasks = tasks_str,
            total = fmt_duration(row.total_seconds),
            avg = fmt_duration(avg_seconds),
            visits = row.visits,
        );
        lines.push(Line::from(Span::styled(
            line,
            Style::default().fg(theme.foreground),
        )));
    }

    Paragraph::new(lines)
        .style(Style::default().bg(theme.background))
        .render(area, buf);
}

fn render_throughput(
    area: Rect,
    buf: &mut Buffer,
    throughput: DashboardThroughput<'_>,
    theme: &Theme,
) {
    if area.height < 3 {
        return;
    }

    let header = format!(
        " throughput · {} completed in last {} day(s) ",
        throughput.total,
        throughput.per_day.len()
    );
    let header_line = Line::from(Span::styled(
        header,
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    ));

    Paragraph::new(vec![header_line])
        .style(Style::default().bg(theme.background))
        .render(
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            buf,
        );

    let chart_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height - 1,
    };
    render_sparkline(chart_area, buf, throughput.per_day, theme);
}

fn render_sparkline(area: Rect, buf: &mut Buffer, per_day: &[u32], theme: &Theme) {
    if per_day.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }

    let max = *per_day.iter().max().unwrap_or(&0);
    let height = area.height.saturating_sub(1).max(1);
    let style = Style::default().fg(theme.accent).bg(theme.background);

    // Column width per day, at least 1 cell.
    let days = per_day.len() as u16;
    let col_width = (area.width / days).max(1);

    for (i, &count) in per_day.iter().enumerate() {
        let bar_h = if max == 0 {
            0
        } else {
            ((count as u32 * height as u32) / max as u32) as u16
        };
        let x = area.x + i as u16 * col_width;
        if x >= area.x + area.width {
            break;
        }
        for dy in 0..bar_h {
            let y = area.y + height - 1 - dy;
            for dx in 0..col_width.saturating_sub(1).max(1) {
                let cx = x + dx;
                if cx < area.x + area.width && y < area.y + area.height {
                    let cell = &mut buf[(cx, y)];
                    cell.set_char('█');
                    cell.set_style(style);
                }
            }
        }
        // Label row under each column: the count.
        let label = format!("{count}");
        let label_y = area.y + area.height - 1;
        let label_width = label.chars().count() as u16;
        if x < area.x + area.width && label_y < area.y + area.height {
            for (k, ch) in label.chars().enumerate() {
                let cx = x + k as u16;
                if k as u16 >= col_width || cx >= area.x + area.width {
                    break;
                }
                let _ = label_width;
                let cell = &mut buf[(cx, label_y)];
                cell.set_char(ch);
                cell.set_style(Style::default().fg(theme.muted).bg(theme.background));
            }
        }
    }
}

/// `Xs` / `Xm` / `Xh` / `Xd` — compact duration for tight columns.
fn fmt_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let width = area.width.saturating_mul(percent_x) / 100;
    let height = area.height.saturating_mul(percent_y) / 100;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}
