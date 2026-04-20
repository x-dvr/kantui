//! Helix-style status bar: colored mode chip on the left, path breadcrumb in
//! the middle, counts / clock on the right.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::theme::Theme;
use crate::view::Mode;

/// Counts shown in the right-hand segment of the status bar.
#[derive(Debug, Default, Clone, Copy)]
pub struct StatusCounts {
    pub tasks_total: u32,
    pub tasks_done: u32,
}

/// Data driving a single render of the status bar.
#[derive(Debug, Clone, Copy)]
pub struct StatusBarView<'a> {
    pub mode: Mode,
    pub project: &'a str,
    pub state: &'a str,
    pub task_title: Option<&'a str>,
    pub counts: StatusCounts,
    pub clock: &'a str,
}

pub struct StatusBar<'a> {
    pub view: StatusBarView<'a>,
    pub theme: &'a Theme,
}

impl<'a> StatusBar<'a> {
    #[must_use]
    pub const fn new(view: StatusBarView<'a>, theme: &'a Theme) -> Self {
        Self { view, theme }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let StatusBarView {
            mode,
            project,
            state,
            task_title,
            counts,
            clock,
        } = self.view;
        let theme = self.theme;

        let mode_color = match mode {
            Mode::Normal => theme.mode_normal,
            Mode::Insert => theme.mode_insert,
            Mode::Command => theme.mode_command,
            Mode::Search => theme.mode_search,
        };

        let mode_chip = Span::styled(
            format!(" {} ", mode.label()),
            Style::default()
                .bg(mode_color)
                .fg(theme.background)
                .add_modifier(Modifier::BOLD),
        );

        let sep = Span::styled(" › ", Style::default().fg(theme.muted));

        let mut left: Vec<Span<'_>> = vec![
            mode_chip,
            Span::raw(" "),
            Span::styled(project.to_owned(), Style::default().fg(theme.status_bar_fg)),
            sep.clone(),
            Span::styled(state.to_owned(), Style::default().fg(theme.status_bar_fg)),
        ];
        if let Some(title) = task_title {
            left.push(sep);
            left.push(Span::styled(
                title.to_owned(),
                Style::default()
                    .fg(theme.status_bar_fg)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let right = format!(
            " {done}/{total} · {clock} ",
            done = counts.tasks_done,
            total = counts.tasks_total,
            clock = clock,
        );

        // Render background first so both halves sit on the status-bar color.
        let bg = Paragraph::new("").style(Style::default().bg(theme.status_bar));
        bg.render(area, buf);

        Paragraph::new(Line::from(left))
            .style(Style::default().bg(theme.status_bar))
            .render(area, buf);

        let right_width = right.chars().count() as u16;
        if right_width < area.width {
            let right_area = Rect {
                x: area.x + area.width - right_width,
                y: area.y,
                width: right_width,
                height: 1,
            };
            Paragraph::new(Line::from(Span::styled(
                right,
                Style::default()
                    .fg(theme.status_bar_fg)
                    .bg(theme.status_bar),
            )))
            .render(right_area, buf);
        }
    }
}
