use crate::ui::state::AppState;
use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Text,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    if !state.diff_modal_open {
        return;
    }
    let theme = crate::ui::theme::get_theme(state.theme_index);

    let diff_text = if let Some(ref lines) = state.diff_lines {
        if lines.is_empty() {
            Text::raw("No differences found.")
        } else {
            // Optimization: Parse only visible lines based on chunk height
            let height = area.height.saturating_sub(4) as usize; // account for borders and padding
            let start = state.diff_scroll as usize;
            let end = (start + height).min(lines.len());

            let slice = &lines[start..end];
            let joined = slice.join("\n");
            joined
                .into_text()
                .unwrap_or_else(|_| Text::raw("Failed to parse ANSI diff"))
        }
    } else {
        Text::raw("Loading diff...")
    };

    let block = Block::default()
        .title(" Diff Preview (Esc/v to close, Up/Down to scroll) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.secondary));

    let paragraph = Paragraph::new(diff_text).block(block).scroll((0, 0)); // Parsing already sliced lines, so scroll is 0.

    let area = centered_rect(80, 80, area);
    f.render_widget(Clear, area); //this clears out the background
    f.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
