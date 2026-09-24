use crate::ui::state::AppState;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph},
};

/// Progress of a running cleanup, then its results.
pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let Some(execution) = &state.execution else {
        return;
    };
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let area = super::centered_rect(70, 70, area);
    f.render_widget(Clear, area);

    let title = if execution.finished {
        " Cleanup finished "
    } else {
        " Cleaning up… "
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.secondary));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let [gauge_area, log_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .areas(inner);

    let done = execution.results.len();
    let ratio = if execution.total == 0 {
        1.0
    } else {
        done as f64 / execution.total as f64
    };
    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(theme.primary))
            .label(format!("{done}/{}", execution.total))
            .ratio(ratio.min(1.0)),
        gauge_area,
    );

    let mut lines: Vec<Line> = Vec::new();
    let mut current_repo = None;
    for result in &execution.results {
        if current_repo != Some(&result.repo) {
            current_repo = Some(&result.repo);
            lines.push(Line::from(Span::styled(
                state.display_path(&result.repo),
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        let (mark, detail, color) = match &result.outcome {
            Ok(msg) => ("✔", msg.as_str(), theme.success),
            Err(err) => ("✘", err.as_str(), theme.error),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {mark} "), Style::default().fg(color)),
            Span::raw(result.operation.describe()),
            Span::styled(
                format!(" — {detail}"),
                Style::default().fg(theme.text_dimmed),
            ),
        ]));
    }
    f.render_widget(
        Paragraph::new(lines).scroll((execution.scroll, 0)),
        log_area,
    );

    let footer = if execution.finished {
        let mut spans = vec![
            Span::styled(
                format!("{} succeeded", done - execution.failed()),
                Style::default().fg(theme.success),
            ),
            Span::raw(", "),
            Span::styled(
                format!("{} failed", execution.failed()),
                Style::default().fg(if execution.failed() > 0 {
                    theme.error
                } else {
                    theme.text_dimmed
                }),
            ),
            Span::raw(format!(
                ", {} freed. ",
                crate::git::stats::format_size(execution.freed_bytes())
            )),
        ];
        if execution.restorable() {
            spans.push(Span::styled(
                "Undo with `sloth restore --last`. ",
                Style::default().fg(theme.text_dimmed),
            ));
        }
        spans.push(Span::styled(
            "Enter to close",
            Style::default().fg(theme.primary),
        ));
        Line::from(spans)
    } else {
        Line::from(Span::styled(
            format!("{} running…", state.spinner()),
            Style::default().fg(theme.text_dimmed),
        ))
    };
    f.render_widget(Paragraph::new(vec![Line::raw(""), footer]), footer_area);
}
