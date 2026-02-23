use crate::ui::state::{AppState, Focus};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let repo_block_title = if state.is_scanning {
        format!("Repositories (Scanning... {} found)", state.scanned_count)
    } else if state.is_analyzing {
        format!(
            "Repositories (Analyzing... {}/{})",
            state.analyzed_count, state.scanned_count
        )
    } else {
        format!("Repositories ({})", state.repositories.len())
    };

    let repo_block = Block::default()
        .title(repo_block_title)
        .borders(Borders::ALL)
        .border_style(if state.focus == Focus::Repositories {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    if state.is_scanning && state.repositories.is_empty() {
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = spinner[(state.loader_tick / 4) % spinner.len()];
        let p = Paragraph::new(format!("{} Scanning directory structure...", frame))
            .style(Style::default().fg(Color::DarkGray))
            .block(repo_block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
    } else {
        let repo_items: Vec<ListItem> = state
            .repositories
            .iter()
            .enumerate()
            .map(|(i, repo)| {
                let prefix = if i == state.repo_index { ">> " } else { "   " };
                let content = format!("{}{}", prefix, repo.path.display());
                let mut style = Style::default();
                if i == state.repo_index && state.focus == Focus::Repositories {
                    style = style.fg(Color::Yellow);
                }
                ListItem::new(content).style(style)
            })
            .collect();

        let repo_count = repo_items.len();
        let target_list = List::new(repo_items).block(repo_block);
        f.render_stateful_widget(target_list, area, &mut state.repo_state);

        let mut repo_scrollbar_state = ScrollbarState::new(repo_count.saturating_sub(1))
            .position(state.repo_state.selected().unwrap_or(0));
        f.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            area,
            &mut repo_scrollbar_state,
        );
    }
}
