use crate::ui::state::{AppState, Focus};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let mut detail_items: Vec<ListItem> = Vec::new();

    let is_repo_analyzed = state
        .repositories
        .get(state.repo_index)
        .map_or(false, |r| r.analyzed);

    if let Some(repo) = state.repositories.get(state.repo_index) {
        if repo.analyzed {
            detail_items
                .push(ListItem::new("--- Branches ---").style(Style::default().fg(Color::Blue)));

            let selected_b = state.selected_branches.entry(state.repo_index).or_default();
            for (b_idx, branch) in repo.branches.iter().enumerate() {
                let is_active = if b_idx == state.detail_index {
                    ">> "
                } else {
                    "   "
                };
                let checkbox = if selected_b.contains(&branch.name) {
                    "[x] "
                } else {
                    "[ ] "
                };
                let dead_marker = if branch.is_dead { " (DEAD)" } else { "" };

                let is_selected = b_idx == state.detail_index && state.focus == Focus::Details;

                let mut spans = vec![
                    Span::styled(
                        is_active,
                        if is_selected {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(
                        checkbox,
                        if is_selected {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(
                        branch.name.clone() + dead_marker,
                        if branch.is_dead {
                            Style::default().fg(Color::Red)
                        } else if is_selected {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default()
                        },
                    ),
                ];

                let display_stats = if branch.is_dead || branch.upstream.is_some() {
                    if !branch.is_dead
                        && branch.ahead == 0
                        && branch.behind == 0
                        && branch.diff_insertions == 0
                        && branch.diff_deletions == 0
                    {
                        spans.push(Span::styled(
                            " (Up to date with upstream)",
                            Style::default().fg(Color::DarkGray),
                        ));
                        false
                    } else {
                        true
                    }
                } else {
                    spans.push(Span::styled(
                        " (Local)",
                        Style::default().fg(Color::DarkGray),
                    ));
                    false
                };

                if display_stats {
                    spans.push(Span::raw(" "));
                    if branch.ahead > 0 {
                        spans.push(Span::styled(
                            format!("\u{2191}{}", branch.ahead),
                            Style::default().fg(Color::Green),
                        ));
                    } else {
                        spans.push(Span::styled(
                            format!("\u{2191}0"),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    spans.push(Span::raw(" "));
                    if branch.behind > 0 {
                        spans.push(Span::styled(
                            format!("\u{2193}{}", branch.behind),
                            Style::default().fg(Color::Red),
                        ));
                    } else {
                        spans.push(Span::styled(
                            format!("\u{2193}0"),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    spans.push(Span::raw(" ("));
                    if branch.diff_insertions > 0 {
                        spans.push(Span::styled(
                            format!("+{}", branch.diff_insertions),
                            Style::default().fg(Color::Green),
                        ));
                    } else {
                        spans.push(Span::styled(
                            format!("+0"),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    spans.push(Span::raw(" "));
                    if branch.diff_deletions > 0 {
                        spans.push(Span::styled(
                            format!("-{}", branch.diff_deletions),
                            Style::default().fg(Color::Red),
                        ));
                    } else {
                        spans.push(Span::styled(
                            format!("-0"),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    spans.push(Span::raw(")"));
                }

                detail_items.push(ListItem::new(Line::from(spans)));
            }

            detail_items
                .push(ListItem::new("--- Stashes ---").style(Style::default().fg(Color::Blue)));
            let selected_s = state.selected_stashes.entry(state.repo_index).or_default();
            for (s_idx, stash) in repo.stashes.iter().enumerate() {
                let actual_idx = repo.branches.len() + s_idx;
                let is_active = if actual_idx == state.detail_index {
                    ">> "
                } else {
                    "   "
                };
                let checkbox = if selected_s.contains(&stash.index) {
                    "[x] "
                } else {
                    "[ ] "
                };

                let is_selected = actual_idx == state.detail_index && state.focus == Focus::Details;

                let spans = vec![
                    Span::styled(
                        is_active,
                        if is_selected {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(
                        checkbox,
                        if is_selected {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(
                        stash.message.clone(),
                        if is_selected {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default()
                        },
                    ),
                ];

                detail_items.push(ListItem::new(Line::from(spans)));
            }
        }
    }

    let detail_count = detail_items.len();
    let mut title_spans = vec![if state.repositories.is_empty() {
        if state.is_scanning || state.is_analyzing {
            Span::raw("Details (Waiting...)")
        } else {
            Span::raw("Details")
        }
    } else {
        Span::raw("Details (Branches & Stashes)")
    }];

    if let Some(repo) = state.repositories.get(state.repo_index) {
        if let Some(url) = &repo.remote_url {
            title_spans.push(Span::raw(" ["));
            title_spans.push(Span::styled(url.clone(), Style::default().fg(Color::Cyan)));
            title_spans.push(Span::raw("]"));
        }
    }

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title_spans))
        .border_style(if state.focus == Focus::Details {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    if state.repositories.is_empty() {
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = spinner[(state.loader_tick / 4) % spinner.len()];
        let text = if state.is_scanning {
            format!("{} Waiting for scan to complete...", frame)
        } else if state.is_analyzing {
            format!("{} Analyzing branches and stashes...", frame)
        } else {
            "No repositories found.".to_string()
        };
        let p = Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray))
            .block(detail_block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
    } else if !is_repo_analyzed {
        // Repo found but not yet analyzed — show spinner
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = spinner[(state.loader_tick / 4) % spinner.len()];
        let text = format!("{} Analyzing repository...", frame);
        let p = Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray))
            .block(detail_block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
    } else {
        let details_list = List::new(detail_items).block(detail_block);

        // Map the semantic detail_index to the visual list index to ensure accurate scrolling
        let mut mapped_idx = state.detail_index;
        if let Some(repo) = state.repositories.get(state.repo_index) {
            mapped_idx += 1; // +1 for "--- Branches ---"
            if state.detail_index >= repo.branches.len() {
                mapped_idx += 1; // +1 for "--- Stashes ---"
            }
        }
        state.detail_state.select(Some(mapped_idx));

        f.render_stateful_widget(details_list, area, &mut state.detail_state);

        let mut detail_scrollbar_state =
            ScrollbarState::new(detail_count.saturating_sub(1)).position(mapped_idx);
        f.render_stateful_widget(
            Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
            area,
            &mut detail_scrollbar_state,
        );
    }
}
