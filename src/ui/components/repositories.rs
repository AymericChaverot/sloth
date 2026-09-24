use crate::ui::state::{AppState, Focus};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

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

    let mut title_suffix = if state.is_searching {
        format!(" (Searching: {})", state.search_query)
    } else {
        String::new()
    };

    let selected_count = state.selected_repositories.len();
    if selected_count > 0 {
        let mut selected_recoverable = 0;
        for &idx in &state.selected_repositories {
            if let Some(repo) = state.repositories.get(idx) {
                selected_recoverable += repo.untracked_size_bytes.unwrap_or(0);
            }
        }
        title_suffix.push_str(&format!(
            " [{} selected, {} recoverable]",
            selected_count,
            crate::git::stats::format_size(selected_recoverable)
        ));
    }
    let repo_block = Block::default()
        .title(format!("{}{}", repo_block_title, title_suffix))
        .borders(Borders::ALL)
        .border_style(if state.focus == Focus::Repositories {
            Style::default().fg(theme.border_active)
        } else {
            Style::default().fg(theme.border)
        });

    if state.is_scanning && state.repositories.is_empty() {
        let spinner = crate::ui::SPINNER;
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

                if state.is_searching
                    && !state.search_query.is_empty()
                    && !repo_name
                        .to_lowercase()
                        .contains(&state.search_query.to_lowercase())
                {
                    return None;
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
                if let Some(branch) = &repo.current_branch {
                    content_spans.push(ratatui::text::Span::styled(
                        format!(" ({})", branch),
                        Style::default().fg(theme.secondary),
                    ));
                }
                if repo.is_dirty {
                    content_spans.push(ratatui::text::Span::styled(
                        " ●",
                        Style::default().fg(theme.primary),
                    ));
                }
                let spinner = crate::ui::SPINNER;
                let frame = spinner[(state.loader_tick / 4) % spinner.len()];
                let fmt = crate::git::stats::format_size;
                match (repo.size_bytes, repo.size_finalized) {
                    (Some(size), true) => {
                        let untracked = repo.untracked_size_bytes.unwrap_or(0);
                        let label = if untracked > 0 {
                            format!(" [.git {} · {} untracked]", fmt(size), fmt(untracked))
                        } else {
                            format!(" [.git {}]", fmt(size))
                        };
                        content_spans.push(ratatui::text::Span::styled(
                            label,
                            Style::default().fg(theme.text_dimmed),
                        ));
                    }
                    (Some(size), false) => {
                        content_spans.push(ratatui::text::Span::styled(
                            format!(
                                " [.git {} · ~{} untracked {}]",
                                fmt(size),
                                fmt(repo.untracked_size_bytes.unwrap_or(0)),
                                frame
                            ),
                            Style::default().fg(theme.text_dimmed),
                        ));
                    }
                    (None, _) => {
                        content_spans.push(ratatui::text::Span::styled(
                            format!(" [{}]", frame),
                            Style::default().fg(theme.text_dimmed),
                        ));
                    }
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
