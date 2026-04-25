//! Project editor overlay. A centered modal that shows every editable field
//! of a project: name, description, and the ordered list of states (with WIP
//! limit and task count). The widget is read-only; the binary mutates the
//! project via core services and rebuilds the view model.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::theme::Theme;

/// What the cursor is currently sitting on inside the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectEditorFocus {
    Name,
    Description,
    /// One of the states; index into [`ProjectEditorView::states`].
    State(usize),
    /// Synthetic "[+ Add state]" row at the bottom of the state list.
    AddState,
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectEditorStateRow<'a> {
    pub name: &'a str,
    pub wip_limit: Option<u32>,
    pub task_count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectEditorView<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub states: &'a [ProjectEditorStateRow<'a>],
    pub focus: ProjectEditorFocus,
}

pub struct ProjectEditor<'a> {
    pub view: ProjectEditorView<'a>,
    pub theme: &'a Theme,
}

impl<'a> ProjectEditor<'a> {
    #[must_use]
    pub const fn new(view: ProjectEditorView<'a>, theme: &'a Theme) -> Self {
        Self { view, theme }
    }
}

impl Widget for ProjectEditor<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = centered_rect(area, 70, 80);
        Clear.render(popup, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(Span::styled(
                " Edit Project ",
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
            .style(Style::default().bg(self.theme.background))
            .render(inner, buf);
    }
}

fn build_lines<'a>(view: &ProjectEditorView<'a>, theme: &Theme) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();

    // Name row.
    lines.push(field_row(
        "Name",
        view.name,
        view.focus == ProjectEditorFocus::Name,
        theme,
    ));

    // Description row.
    let desc_text: String = view
        .description
        .map(|d| {
            if d.trim().is_empty() {
                "(none)".to_owned()
            } else {
                d.to_owned()
            }
        })
        .unwrap_or_else(|| "(none)".to_owned());
    let desc_muted = view.description.is_none_or(|d| d.trim().is_empty());
    lines.push(field_row_styled(
        "Description",
        desc_text,
        view.focus == ProjectEditorFocus::Description,
        theme,
        desc_muted,
    ));

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "States",
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    )));

    if view.states.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no states yet)",
            Style::default().fg(theme.muted),
        )));
    } else {
        let name_width = view
            .states
            .iter()
            .map(|s| s.name.chars().count())
            .max()
            .unwrap_or(0)
            .max(5);
        for (i, row) in view.states.iter().enumerate() {
            let selected = view.focus == ProjectEditorFocus::State(i);
            lines.push(state_row(row, name_width, selected, theme));
        }
    }

    // Add-state placeholder row.
    let add_selected = view.focus == ProjectEditorFocus::AddState;
    let cursor = if add_selected { "▶ " } else { "  " };
    let cursor_style = if add_selected {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    let add_style = if add_selected {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    let mut add_line = Line::from(vec![
        Span::styled(cursor.to_owned(), cursor_style),
        Span::styled("[+ Add state]".to_owned(), add_style),
    ]);
    if add_selected {
        add_line = add_line.style(Style::default().bg(theme.selection));
    }
    lines.push(add_line);

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "i edit · w wip · a add state · d delete state · K/J reorder · Esc back",
        Style::default().fg(theme.muted),
    )));

    lines
}

fn field_row<'a>(label: &'a str, value: &'a str, selected: bool, theme: &Theme) -> Line<'a> {
    field_row_styled(label, value.to_owned(), selected, theme, false)
}

fn field_row_styled<'a>(
    label: &'a str,
    value: String,
    selected: bool,
    theme: &Theme,
    muted_value: bool,
) -> Line<'a> {
    let cursor = if selected { "▶ " } else { "  " };
    let cursor_style = if selected {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    let label_style = Style::default()
        .fg(theme.muted)
        .add_modifier(Modifier::BOLD);
    let value_style = if muted_value {
        Style::default().fg(theme.muted)
    } else if selected {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.foreground)
    };

    let mut line = Line::from(vec![
        Span::styled(cursor.to_owned(), cursor_style),
        Span::styled(format!("{label:<11} "), label_style),
        Span::styled(value, value_style),
    ]);
    if selected {
        line = line.style(Style::default().bg(theme.selection));
    }
    line
}

fn state_row<'a>(
    row: &ProjectEditorStateRow<'a>,
    name_width: usize,
    selected: bool,
    theme: &Theme,
) -> Line<'a> {
    let cursor = if selected { "▶ " } else { "  " };
    let cursor_style = if selected {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    let name_style = if selected {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.foreground)
    };
    let wip = match row.wip_limit {
        Some(limit) => format!("wip {}/{}", row.task_count, limit),
        None => format!("{} tasks", row.task_count),
    };

    let mut line = Line::from(vec![
        Span::styled(cursor.to_owned(), cursor_style),
        Span::styled(
            format!("{name:<width$}", name = row.name, width = name_width),
            name_style,
        ),
        Span::styled(format!("   {wip}"), Style::default().fg(theme.muted)),
    ]);
    if selected {
        line = line.style(Style::default().bg(theme.selection));
    }
    line
}

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
