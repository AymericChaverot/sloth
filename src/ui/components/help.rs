use crate::ui::keymap;
use crate::ui::state::{AppState, ToastLevel};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// Bottom line: context key hints on the left; toast or queue size on the right.
pub fn render_status_bar(f: &mut Frame, state: &AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let key = Style::default().fg(theme.primary);
    let desc = Style::default().fg(theme.text_dimmed);

    let right = if let Some(toast) = &state.toast {
        let color = match toast.level {
            ToastLevel::Info => theme.success,
            ToastLevel::Warning => theme.error,
        };
        Line::from(Span::styled(
            format!(" {} ", toast.message),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
    } else if !state.selection.is_empty() {
        let (b, s, w) = state.selection.counts();
        Line::from(vec![
            Span::styled(format!("Queue ({}): ", state.selection.len()), desc),
            Span::styled(
                format!("{b} branches · {s} stashes · {w} worktrees"),
                Style::default().fg(theme.merged),
            ),
            Span::styled(
                format!(" in {} repos  (x run) ", state.selection.repo_count()),
                desc,
            ),
        ])
    } else {
        Line::default()
    };

    let mut left = Vec::new();
    if state.is_searching {
        left.push(Span::styled("Filter: ", key));
        left.push(Span::raw(format!("{}▏", state.search_query)));
        left.push(Span::styled("  Enter/Esc done", desc));
    } else {
        for (k, d) in keymap::hints(state) {
            left.push(Span::styled(format!(" {k}"), key));
            left.push(Span::styled(format!(" {d} "), desc));
        }
    }

    let [left_area, right_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right.width() as u16)])
            .areas(area);
    f.render_widget(Paragraph::new(Line::from(left)), left_area);
    f.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        right_area,
    );
}

/// Full key reference, toggled with `?`.
pub fn render_overlay(f: &mut Frame, state: &AppState, area: Rect) {
    if !state.show_help {
        return;
    }
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let mut lines = Vec::new();
    for (title, bindings) in keymap::SECTIONS {
        lines.push(Line::from(Span::styled(
            *title,
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        )));
        for (key, desc) in *bindings {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<12}"), Style::default().fg(theme.primary)),
                Span::raw(*desc),
            ]));
        }
        lines.push(Line::raw(""));
    }
    lines.push(Line::from(Span::styled(
        "Deleted branches and stashes can be restored with `sloth restore`.",
        Style::default().fg(theme.text_dimmed),
    )));

    let area = super::centered_rect(60, 85, area);
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(" Keys — ? or Esc to close ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.secondary)),
        ),
        area,
    );
}
