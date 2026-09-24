use crate::ui::state::{AppState, Focus};
use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let render_height = area.height.saturating_sub(2) as usize; // Account for borders

    let mut graph_lines = Vec::new();
    let mut total_graph_count = 0;

    if let Some(repo) = state.focused()
        && let Some(lines) = &repo.graph_lines
    {
        total_graph_count = lines.len();
        let start_idx = state.graph_scroll_y as usize;
        let end_idx = (start_idx + render_height).min(total_graph_count);

        // Only parse the visible slice
        for line in &lines[start_idx..end_idx] {
            if let Ok(mut text) = line.as_str().into_text() {
                for line_ref in &mut text.lines {
                    let mut in_graph = true;
                    for span in &mut line_ref.spans {
                        if in_graph {
                            let mut replaced = String::with_capacity(span.content.len());
                            for c in span.content.chars() {
                                if in_graph {
                                    match c {
                                        '*' => replaced.push('●'),
                                        '|' => replaced.push('│'),
                                        '/' => replaced.push('╱'),
                                        '\\' => replaced.push('╲'),
                                        '_' => replaced.push('─'),
                                        ' ' => replaced.push(' '),
                                        c if c.is_alphanumeric() => {
                                            in_graph = false;
                                            replaced.push(c);
                                        }
                                        c => replaced.push(c),
                                    }
                                } else {
                                    replaced.push(c);
                                }
                            }
                            span.content = std::borrow::Cow::Owned(replaced);
                        }
                    }
                }
                graph_lines.extend(text.lines);
            } else {
                graph_lines.push(Line::from(line.clone()));
            }
        }
    }

    let graph_paragraph = Paragraph::new(graph_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Git Graph")
                .border_style(if state.focus == Focus::GitGraph {
                    Style::default().fg(theme.border_active)
                } else {
                    Style::default().fg(theme.border)
                }),
        )
        // Note: Y scroll is ALWAYS 0 for the Paragraph because we already sliced the collection!
        .scroll((0, state.graph_scroll_x));

    f.render_widget(graph_paragraph, area);

    let mut graph_scrollbar_state =
        ScrollbarState::new(total_graph_count.saturating_sub(render_height))
            .position(state.graph_scroll_y as usize);
    f.render_stateful_widget(
        Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
        area,
        &mut graph_scrollbar_state,
    );
}
