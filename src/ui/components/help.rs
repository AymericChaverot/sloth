use crate::ui::state::{AppState, Focus};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let mut help_text = match state.focus {
        Focus::Repositories => {
            "Repos: Up/Down navigate. Space select. X deep clean. 'p'/'c' prune/gc. 'd' dashboard. '/' search. 't' theme. 'q' quit."
                .to_string()
        }
        Focus::Details => {
            "Details: Up/Down navigate. Space select. 'a' smart-select, 'A' select all, Esc deselect. Enter clean. 'v' diff. 'g' graph. Left back."
                .to_string()
        }
        Focus::GitGraph => {
            "Graph: Arrows scroll. 'f'/'m' fullscreen. 't' theme. 'Esc'/'g' back. 'q' quit.".to_string()
        }
        Focus::Dashboard => {
            "Dashboard: 'q' quit. Left/Right/'d' to exit.".to_string()
        }
    };
    if !state.selection.is_empty() {
        help_text = format!(
            "Queue: {} item(s) in {} repo(s) — Enter review & run, 'C' clear  |  {}",
            state.selection.len(),
            state.selection.repo_count(),
            help_text
        );
    }
    if let Some(ref ver) = state.update_available {
        help_text = format!(
            "🔄 Update available: {} — press 'u' to update  |  {}",
            ver, help_text
        );
    }
    let title = format!("Help (Theme: {})", theme.name);
    let details_footer = Paragraph::new(help_text)
        .style(ratatui::style::Style::default().fg(theme.text_dimmed))
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(details_footer, area);
}
