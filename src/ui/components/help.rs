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
        Line::from(vec![
            Span::styled("Queue: ", desc),
            Span::styled(state.selection.summary(), Style::default().fg(theme.merged)),
            Span::styled(" (x run) ", desc),
        ])
    } else {
        Line::default()
    };

    let mut left = Vec::new();
    if state.editing_filter {
        left.push(Span::styled("Filter: ", key));
        left.push(Span::raw(format!("{}▏", state.repo_filter)));
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
    let section = |&(title, bindings): &(&'static str, &'static [keymap::Binding])| {
        let mut lines = vec![Line::from(Span::styled(
            title,
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        ))];
        for (key, desc) in bindings {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<12}"), Style::default().fg(theme.primary)),
                Span::raw(*desc),
            ]));
        }
        lines.push(Line::raw(""));
        lines
    };

    // Two balanced columns of sections.
    let total: usize = keymap::SECTIONS.iter().map(|(_, b)| b.len() + 2).sum();
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for s in keymap::SECTIONS {
        if left.len() < total / 2 {
            left.extend(section(s));
        } else {
            right.extend(section(s));
        }
    }
    right.push(Line::from(Span::styled(
        "Deleted branches and stashes can be",
        Style::default().fg(theme.text_dimmed),
    )));
    right.push(Line::from(Span::styled(
        "restored with `sloth restore`.",
        Style::default().fg(theme.text_dimmed),
    )));

    let area = super::centered_rect(90, 90, area);
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(" Keys — ? or Esc to close ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.secondary));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).areas(inner);
    f.render_widget(Paragraph::new(left).wrap(Wrap { trim: false }), left_area);
    f.render_widget(Paragraph::new(right).wrap(Wrap { trim: false }), right_area);
}
