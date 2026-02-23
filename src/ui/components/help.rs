use crate::ui::state::{AppState, Focus};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let mut help_text = match state.focus {
        Focus::Repositories => {
            "Left Pane: Up/Down navigate. Right enter details. 'p' prune remotes. 'g' toggle graph. 'q' quit."
                .to_string()
        }
        Focus::Details => {
            "Middle Pane: Up/Down navigate. 'Space' select item. 'Enter' execute. Right graph. Left repos."
                .to_string()
        }
        Focus::GitGraph => {
            "Right Pane: Arrows to scroll. 'f' fullscreen. 'Esc' or 'g' back. 'q' quit.".to_string()
        }
    };
    if let Some(ref ver) = state.update_available {
        help_text = format!(
            "🔄 Update available: {} — press 'u' to update  |  {}",
            ver, help_text
        );
    }
    let details_footer =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(details_footer, area);
}
