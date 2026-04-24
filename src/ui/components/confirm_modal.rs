use crate::ui::state::{AppState, UiAction};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let pending = match &state.pending_action {
        Some(a) => a,
        None => return,
    };

    let theme = crate::ui::theme::get_theme(state.theme_index);

    let action_desc = match pending {
        UiAction::CleanRepo => {
            let b = state
                .selected_branches
                .get(&state.repo_index)
                .map_or(0, |s| s.len());
            let s = state
                .selected_stashes
                .get(&state.repo_index)
                .map_or(0, |s| s.len());
            let w = state
                .selected_worktrees
                .get(&state.repo_index)
                .map_or(0, |s| s.len());
            format!(
                "Delete {} branch(es), {} stash(es), {} worktree(s)",
                b, s, w
            )
        }
        UiAction::PruneRemotes => {
            let n = if state.selected_repositories.is_empty() {
                1
            } else {
                state.selected_repositories.len()
            };
            format!("Prune remote tracking branches on {} repo(s)", n)
        }
        UiAction::GarbageCollect => {
            let n = if state.selected_repositories.is_empty() {
                1
            } else {
                state.selected_repositories.len()
            };
            format!("Garbage collect {} repo(s)", n)
        }
        UiAction::DeepClean => {
            let n = if state.selected_repositories.is_empty() {
                1
            } else {
                state.selected_repositories.len()
            };
            format!("Deep clean (git clean -xdff) {} repo(s)", n)
        }
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Action: ", Style::default().fg(theme.text_dimmed)),
        Span::styled(
            action_desc,
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    if let UiAction::CleanRepo = pending {
        if let Some(repo) = state.repositories.get(state.repo_index) {
            if let Some(branches) = state.selected_branches.get(&state.repo_index) {
                if !branches.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "Branches:",
                        Style::default().fg(theme.secondary),
                    )));
                    let mut sorted: Vec<&String> = branches.iter().collect();
                    sorted.sort();
                    for b in sorted {
                        lines.push(Line::from(Span::styled(
                            format!("  - {}", b),
                            Style::default().fg(theme.error),
                        )));
                    }
                    lines.push(Line::raw(""));
                }
            }
            if let Some(stashes) = state.selected_stashes.get(&state.repo_index) {
                if !stashes.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "Stashes:",
                        Style::default().fg(theme.secondary),
                    )));
                    let mut sorted: Vec<usize> = stashes.iter().copied().collect();
                    sorted.sort();
                    for s in sorted {
                        if let Some(st) = repo.stashes.iter().find(|st| st.index == s) {
                            lines.push(Line::from(Span::styled(
                                format!("  - {}", st.message),
                                Style::default().fg(theme.error),
                            )));
                        }
                    }
                    lines.push(Line::raw(""));
                }
            }
            if let Some(worktrees) = state.selected_worktrees.get(&state.repo_index) {
                if !worktrees.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "Worktrees:",
                        Style::default().fg(theme.secondary),
                    )));
                    let mut sorted: Vec<&String> = worktrees.iter().collect();
                    sorted.sort();
                    for w in sorted {
                        lines.push(Line::from(Span::styled(
                            format!("  - {}", w),
                            Style::default().fg(theme.error),
                        )));
                    }
                    lines.push(Line::raw(""));
                }
            }
        }
    }

    if let UiAction::DeepClean = pending {
        match &state.confirm_preview_lines {
            None => {
                lines.push(Line::from(Span::styled(
                    "Loading preview...",
                    Style::default().fg(theme.text_dimmed),
                )));
            }
            Some(preview) => {
                lines.push(Line::from(Span::styled(
                    "Files that will be removed:",
                    Style::default().fg(theme.secondary),
                )));
                for line in preview.iter().take(15) {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", line),
                        Style::default().fg(theme.error),
                    )));
                }
                if preview.len() > 15 {
                    lines.push(Line::from(Span::styled(
                        format!("  ... and {} more", preview.len() - 15),
                        Style::default().fg(theme.text_dimmed),
                    )));
                }
            }
        }
        lines.push(Line::raw(""));
    }

    lines.push(Line::from(vec![
        Span::styled("Press ", Style::default().fg(theme.text_dimmed)),
        Span::styled(
            "Y",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to confirm, ", Style::default().fg(theme.text_dimmed)),
        Span::styled(
            "N",
            Style::default()
                .fg(theme.error)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("/Esc to cancel", Style::default().fg(theme.text_dimmed)),
    ]));

    let block = Block::default()
        .title(" Confirm Action ")
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        );

    let paragraph = Paragraph::new(lines).block(block);
    let modal_area = centered_rect(55, 70, area);
    f.render_widget(Clear, modal_area);
    f.render_widget(paragraph, modal_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
