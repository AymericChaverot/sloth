use crate::ui::state::{AppState, Focus};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);

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

    let title_suffix = if state.is_searching {
        format!(" (Searching: {})", state.search_query)
    } else {
        String::new()
    };
    let repo_block = Block::default()
        .title(format!("{}{}", repo_block_title, title_suffix))
        .borders(Borders::ALL)
        .border_style(if state.focus == Focus::Repositories {
            Style::default().fg(theme.border_active)
        } else {
            Style::default().fg(theme.border)
        });

    if state.is_scanning && state.repositories.is_empty() {
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = spinner[(state.loader_tick / 4) % spinner.len()];
        let p = Paragraph::new(format!("{} Scanning directory structure...", frame))
            .style(Style::default().fg(theme.text_dimmed))
            .block(repo_block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
    } else {
        let repo_items: Vec<ListItem> = state
            .repositories
            .iter()
            .enumerate()
            .filter_map(|(i, repo)| {
                let repo_name = repo
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                if state.is_searching && !state.search_query.is_empty() {
                    if !repo_name
                        .to_lowercase()
                        .contains(&state.search_query.to_lowercase())
                    {
                        return None;
                    }
                }

                let is_focused = i == state.repo_index;
                let prefix = if is_focused { ">> " } else { "   " };
                let is_selected = state.selected_repositories.contains(&i);
                let checkbox = if is_selected { "[x] " } else { "[ ] " };
                let mut content_spans = vec![
                    ratatui::text::Span::raw(prefix),
                    ratatui::text::Span::styled(
                        checkbox,
                        if is_selected {
                            Style::default().fg(theme.primary)
                        } else {
                            Style::default().fg(theme.text_normal)
                        },
                    ),
                    ratatui::text::Span::raw(repo.path.display().to_string()),
                ];
                if let Some(size) = repo.size_bytes {
                    let size_str = format_size(size);
                    content_spans.push(ratatui::text::Span::styled(
                        format!(" [{}]", size_str),
                        Style::default().fg(theme.text_dimmed),
                    ));
                } else if !repo.analyzed {
                    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                    let frame = spinner[(state.loader_tick / 4) % spinner.len()];
                    content_spans.push(ratatui::text::Span::styled(
                        format!(" [{}]", frame),
                        Style::default().fg(theme.text_dimmed),
                    ));
                }

                let mut style = Style::default().fg(theme.text_normal);
                if i == state.repo_index && state.focus == Focus::Repositories {
                    style = style.fg(theme.primary);
                }
                Some(ListItem::new(ratatui::text::Line::from(content_spans)).style(style))
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
