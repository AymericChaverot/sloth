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
            "Left Pane: Up/Down navigate. Space select. X deep clean. 'p' prune. 'c' gc. 'g' graph. 't' theme. 'q' quit."
                .to_string()
        }
        Focus::Details => {
            "Middle Pane: Up/Down navigate. Space select. 'A' auto-select. Enter clean. 'v' diff. 'g' graph. Left repos."
                .to_string()
        }
        Focus::GitGraph => {
            "Right Pane: Arrows to scroll. 'f' fullscreen. 't' theme. 'Esc' or 'g' back. 'q' quit.".to_string()
        }
    };
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
