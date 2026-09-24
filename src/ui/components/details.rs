use crate::ui::state::{AppState, Focus};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

pub fn render(f: &mut Frame, state: &mut AppState, area: Rect) {
    let theme = crate::ui::theme::get_theme(state.theme_index);
    let mut detail_items: Vec<ListItem> = Vec::new();

    let is_repo_analyzed = state
        .repositories
        .get(state.repo_index)
        .is_some_and(|r| r.analyzed);

    if let Some(repo) = state.repositories.get(state.repo_index)
        && repo.analyzed
    {
        detail_items
            .push(ListItem::new("--- Branches ---").style(Style::default().fg(theme.secondary)));

        let selected_b = state.selected_branches.entry(state.repo_index).or_default();
        for (b_idx, branch) in repo.branches.iter().enumerate() {
            if state.is_searching
                && !state.search_query.is_empty()
                && !branch
                    .name
                    .to_lowercase()
                    .contains(&state.search_query.to_lowercase())
            {
                continue;
            }

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
                        Style::default().fg(theme.primary)
                    } else {
                        Style::default().fg(theme.text_normal)
                    },
                ),
                Span::styled(
                    checkbox,
                    if is_selected {
                        Style::default().fg(theme.primary)
                    } else {
                        Style::default().fg(theme.text_normal)
                    },
                ),
                Span::styled(
                    branch.name.clone() + dead_marker,
                    if branch.is_dead {
                        Style::default().fg(theme.error)
                    } else if is_selected {
                        Style::default().fg(theme.primary)
                    } else {
                        Style::default().fg(theme.text_normal)
                    },
                ),
            ];

            if let Some(ts) = branch.last_commit_ts {
                spans.push(Span::styled(
                    format!(
                        " ({} ago)",
                        crate::git::stats::format_age(ts, crate::git::stats::now_ts())
                    ),
                    Style::default().fg(theme.secondary),
                ));
            }

            if branch.is_fully_merged() {
                spans.push(Span::styled(" (Merged)", Style::default().fg(theme.merged)));
            }

            let display_stats = if branch.is_dead || branch.upstream.is_some() {
                if !branch.is_dead
                    && branch.ahead == 0
                    && branch.behind == 0
                    && branch.diff_insertions == 0
                    && branch.diff_deletions == 0
                {
                    spans.push(Span::styled(
                        " (Up to date with upstream)",
                        Style::default().fg(theme.text_dimmed),
                    ));
                    false
                } else {
                    true
                }
            } else {
                spans.push(Span::styled(
                    " (Local)",
                    Style::default().fg(theme.text_dimmed),
                ));
                false
            };

            if display_stats {
                spans.push(Span::raw(" "));
                if branch.ahead > 0 {
                    spans.push(Span::styled(
                        format!("\u{2191}{}", branch.ahead),
                        Style::default().fg(theme.success),
                    ));
                } else {
                    spans.push(Span::styled(
                        "\u{2191}0".to_string(),
                        Style::default().fg(theme.text_dimmed),
                    ));
                }
                spans.push(Span::raw(" "));
                if branch.behind > 0 {
                    spans.push(Span::styled(
                        format!("\u{2193}{}", branch.behind),
                        Style::default().fg(theme.error),
                    ));
                } else {
                    spans.push(Span::styled(
                        "\u{2193}0".to_string(),
                        Style::default().fg(theme.text_dimmed),
                    ));
                }
                spans.push(Span::styled(" (", Style::default().fg(theme.text_normal)));
                if branch.diff_insertions > 0 {
                    spans.push(Span::styled(
                        format!("+{}", branch.diff_insertions),
                        Style::default().fg(theme.success),
                    ));
                } else {
                    spans.push(Span::styled(
                        "+0".to_string(),
                        Style::default().fg(theme.text_dimmed),
                    ));
                }
                spans.push(Span::raw(" "));
                if branch.diff_deletions > 0 {
                    spans.push(Span::styled(
                        format!("-{}", branch.diff_deletions),
                        Style::default().fg(theme.error),
                    ));
                } else {
                    spans.push(Span::styled(
                        "-0".to_string(),
                        Style::default().fg(theme.text_dimmed),
                    ));
                }
                spans.push(Span::styled(")", Style::default().fg(theme.text_normal)));
            }

            detail_items.push(ListItem::new(Line::from(spans)));
        }

        detail_items
            .push(ListItem::new("--- Stashes ---").style(Style::default().fg(theme.secondary)));
        let selected_s = state.selected_stashes.entry(state.repo_index).or_default();
        for (s_idx, stash) in repo.stashes.iter().enumerate() {
            if state.is_searching
                && !state.search_query.is_empty()
                && !stash
                    .message
                    .to_lowercase()
                    .contains(&state.search_query.to_lowercase())
            {
                continue;
            }

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
                        Style::default().fg(theme.primary)
                    } else {
                        Style::default().fg(theme.text_normal)
                    },
                ),
                Span::styled(
                    checkbox,
                    if is_selected {
                        Style::default().fg(theme.primary)
                    } else {
                        Style::default().fg(theme.text_normal)
                    },
                ),
                Span::styled(
                    stash.message.clone(),
                    if is_selected {
                        Style::default().fg(theme.primary)
                    } else {
                        Style::default().fg(theme.text_normal)
                    },
                ),
            ];

            detail_items.push(ListItem::new(Line::from(spans)));
        }

        detail_items
            .push(ListItem::new("--- Worktrees ---").style(Style::default().fg(theme.secondary)));
        let selected_wt = state
            .selected_worktrees
            .entry(state.repo_index)
            .or_default();
        for (wt_idx, wt) in repo.worktrees.iter().enumerate() {
            if state.is_searching
                && !state.search_query.is_empty()
                && !wt
                    .path
                    .to_lowercase()
                    .contains(&state.search_query.to_lowercase())
            {
                continue;
            }

            let actual_idx = repo.branches.len() + repo.stashes.len() + wt_idx;
            let is_active = if actual_idx == state.detail_index {
                ">> "
            } else {
                "   "
            };
            let checkbox = if selected_wt.contains(&wt.path) {
                "[x] "
            } else {
                "[ ] "
            };
            let is_selected = actual_idx == state.detail_index && state.focus == Focus::Details;

            let mut spans = vec![
                Span::styled(
                    is_active,
                    if is_selected {
                        Style::default().fg(theme.primary)
                    } else {
                        Style::default().fg(theme.text_normal)
                    },
                ),
                Span::styled(
                    checkbox,
                    if is_selected {
                        Style::default().fg(theme.primary)
                    } else {
                        Style::default().fg(theme.text_normal)
                    },
                ),
                Span::styled(
                    wt.path.clone(),
                    if is_selected {
                        Style::default().fg(theme.primary)
                    } else {
                        Style::default().fg(theme.text_normal)
                    },
                ),
            ];

            if let Some(b) = &wt.branch {
                spans.push(Span::styled(
                    format!(" [{}]", b),
                    Style::default().fg(theme.secondary),
                ));
            }

            detail_items.push(ListItem::new(Line::from(spans)));
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
        let mut recoverable = 0;
        let mut sel_b = 0usize;
        let mut sel_s = 0usize;
        let mut sel_w = 0usize;
        if let Some(repo) = state.repositories.get(state.repo_index) {
            let selected_wt = state
                .selected_worktrees
                .get(&state.repo_index)
                .cloned()
                .unwrap_or_default();
            for wt in &repo.worktrees {
                if selected_wt.contains(&wt.path) {
                    recoverable += wt.size_bytes.unwrap_or(0);
                }
            }
            sel_b = state
                .selected_branches
                .get(&state.repo_index)
                .map_or(0, |s| s.len());
            sel_s = state
                .selected_stashes
                .get(&state.repo_index)
                .map_or(0, |s| s.len());
            sel_w = selected_wt.len();
        }
        let has_selection = sel_b + sel_s + sel_w > 0;
        let mut label = "Details (Branches, Stashes, Worktrees)".to_string();
        if has_selection {
            label.push_str(&format!(
                " [{} branch(es), {} stash(es), {} worktree(s) selected",
                sel_b, sel_s, sel_w
            ));
            if recoverable > 0 {
                label.push_str(&format!(
                    ", {} recoverable",
                    crate::git::stats::format_size(recoverable)
                ));
            }
            label.push(']');
        } else if recoverable > 0 {
            label.push_str(&format!(
                " [{} recoverable]",
                crate::git::stats::format_size(recoverable)
            ));
        }
        Span::raw(label)
    }];

    if let Some(repo) = state.repositories.get(state.repo_index)
        && let Some(url) = &repo.remote_url
    {
        title_spans.push(Span::raw(" ["));
        title_spans.push(Span::styled(
            url.clone(),
            Style::default().fg(theme.secondary),
        ));
        title_spans.push(Span::raw("]"));
    }

    if state.is_searching {
        title_spans.push(Span::styled(
            format!(" (Searching: {})", state.search_query),
            Style::default().fg(theme.text_normal),
        ));
    }

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(title_spans))
        .border_style(if state.focus == Focus::Details {
            Style::default().fg(theme.border_active)
        } else {
            Style::default().fg(theme.border)
        });

    if state.repositories.is_empty() {
        let spinner = crate::ui::SPINNER;
        let frame = spinner[(state.loader_tick / 4) % spinner.len()];
        let text = if state.is_scanning {
            format!("{} Waiting for scan to complete...", frame)
        } else if state.is_analyzing {
            format!("{} Analyzing branches and stashes...", frame)
        } else {
            "No repositories found.".to_string()
        };
        let p = Paragraph::new(text)
            .style(Style::default().fg(theme.text_dimmed))
            .block(detail_block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
    } else if !is_repo_analyzed {
        // Repo found but not yet analyzed — show spinner
        let spinner = crate::ui::SPINNER;
        let frame = spinner[(state.loader_tick / 4) % spinner.len()];
        let text = format!("{} Analyzing repository...", frame);
        let p = Paragraph::new(text)
            .style(Style::default().fg(theme.text_dimmed))
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
            if state.detail_index >= repo.branches.len() + repo.stashes.len() {
                mapped_idx += 1; // +1 for "--- Worktrees ---"
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
