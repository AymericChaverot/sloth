use crate::ui::state::{AppState, Focus, UiAction};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

pub fn handle_events(state: &mut AppState) -> std::io::Result<()> {
    if event::poll(Duration::from_millis(16))?
        && let Event::Key(key) = event::read()?
        && key.kind == KeyEventKind::Press
    {
        // Confirm modal takes priority over everything else
        if state.pending_action.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    state.action = state.pending_action.take();
                    state.confirm_preview_lines = None;
                    state.should_quit = true;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    state.pending_action = None;
                    state.confirm_preview_lines = None;
                }
                _ => {}
            }
            return Ok(());
        }

        if state.diff_modal_open {
            match key.code {
                KeyCode::Esc | KeyCode::Char('v') => {
                    state.diff_modal_open = false;
                }
                KeyCode::Up => {
                    state.diff_scroll = state.diff_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    state.diff_scroll = state.diff_scroll.saturating_add(1);
                }
                KeyCode::PageUp => {
                    state.diff_scroll = state.diff_scroll.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    state.diff_scroll = state.diff_scroll.saturating_add(10);
                }
                KeyCode::Home => {
                    state.diff_scroll = 0;
                }
                _ => {}
            }
            return Ok(());
        }

        if state.is_searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    state.is_searching = false;
                }
                KeyCode::Backspace => {
                    state.search_query.pop();
                }
                KeyCode::Char(c) => {
                    state.search_query.push(c);
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('q') => state.should_quit = true,
            KeyCode::Char('u') if state.update_available.is_some() && !state.is_updating => {
                state.is_updating = true;
            }
            KeyCode::Esc => {
                if state.focus == Focus::Details {
                    if let Some(repo) = state.repositories.get(state.repo_index) {
                        state.selection.clear_repo(&repo.path);
                    }
                } else if state.focus == Focus::Dashboard {
                    state.focus = Focus::Repositories;
                } else if state.focus == Focus::GitGraph {
                    state.graph_maximized = false;
                    state.focus = Focus::Details;
                }
            }
            KeyCode::Char('f') | KeyCode::Char('m') if state.focus == Focus::GitGraph => {
                state.graph_maximized = !state.graph_maximized;
            }
            KeyCode::Char('g') => {
                state.show_graph = !state.show_graph;
                if !state.show_graph && state.focus == Focus::GitGraph {
                    state.graph_maximized = false;
                    state.focus = Focus::Details;
                }
            }
            KeyCode::Right => {
                if state.focus == Focus::Dashboard {
                    state.focus = Focus::Repositories;
                } else if state.focus == Focus::Repositories && !state.repositories.is_empty() {
                    state.focus = Focus::Details;
                    state.detail_index = 0;
                } else if state.focus == Focus::Details && state.show_graph {
                    state.focus = Focus::GitGraph;
                } else if state.focus == Focus::GitGraph {
                    state.graph_scroll_x = state.graph_scroll_x.saturating_add(2);
                }
            }
            KeyCode::Left => {
                if state.focus == Focus::Dashboard || state.focus == Focus::Details {
                    state.focus = Focus::Repositories;
                } else if state.focus == Focus::GitGraph {
                    if state.graph_scroll_x > 0 {
                        state.graph_scroll_x = state.graph_scroll_x.saturating_sub(2);
                    } else {
                        // Navigate back if scroll is 0 and not full screen
                        if !state.graph_maximized {
                            state.focus = Focus::Details;
                        }
                    }
                }
            }
            KeyCode::Char(' ') => {
                if state.focus == Focus::Repositories && !state.repositories.is_empty() {
                    if state.selected_repositories.contains(&state.repo_index) {
                        state.selected_repositories.remove(&state.repo_index);
                    } else {
                        state.selected_repositories.insert(state.repo_index);
                    }
                } else if state.focus == Focus::Details
                    && let Some(repo) = state.repositories.get(state.repo_index)
                    && let Some((kind, id)) = detail_item(repo, state.detail_index, &state.config)
                {
                    let path = repo.path.clone();
                    state.selection.toggle(&path, kind, &id);
                }
            }
            KeyCode::Char('a')
                if state.focus == Focus::Details && !state.repositories.is_empty() =>
            {
                smart_select(state, &[state.repo_index]);
            }
            KeyCode::Char('a')
                if state.focus == Focus::Repositories && !state.repositories.is_empty() =>
            {
                // Smart select across every marked repository (or the focused one).
                let targets: Vec<usize> = if state.selected_repositories.is_empty() {
                    vec![state.repo_index]
                } else {
                    state.selected_repositories.iter().copied().collect()
                };
                smart_select(state, &targets);
            }
            KeyCode::Char('A')
                if state.focus == Focus::Details && !state.repositories.is_empty() =>
            {
                let repo = &state.repositories[state.repo_index];
                for branch in &repo.branches {
                    if crate::cleanup::branch_protection(repo, branch, &state.config).is_none() {
                        state.selection.insert(
                            &repo.path,
                            crate::ui::selection::ItemKind::Branch,
                            &branch.name,
                        );
                    }
                }
            }
            KeyCode::Enter
                if matches!(state.focus, Focus::Details | Focus::Repositories)
                    && !state.selection.is_empty() =>
            {
                state.pending_action = Some(UiAction::CleanRepo);
            }
            KeyCode::Char('p')
                if state.focus == Focus::Repositories && !state.repositories.is_empty() =>
            {
                state.pending_action = Some(UiAction::PruneRemotes);
            }
            KeyCode::Char('c')
                if state.focus == Focus::Repositories && !state.repositories.is_empty() =>
            {
                state.pending_action = Some(UiAction::GarbageCollect);
            }
            KeyCode::Char('C') => state.selection.clear(),
            KeyCode::Char('t') => {
                state.theme_index = (state.theme_index + 1) % crate::ui::theme::THEMES.len();
                let name = crate::ui::theme::get_theme(state.theme_index).name;
                state.config.save_theme(name);
            }
            KeyCode::Char('v') if state.focus == Focus::Details => {
                state.diff_modal_open = true;
                state.diff_lines = None;
                state.diff_scroll = 0;
            }
            KeyCode::Char('X')
                if state.focus == Focus::Repositories && !state.repositories.is_empty() =>
            {
                state.pending_action = Some(UiAction::DeepClean);
            }
            KeyCode::Char('d') => {
                if state.focus == Focus::Dashboard {
                    state.focus = Focus::Repositories;
                } else {
                    state.focus = Focus::Dashboard;
                }
            }
            KeyCode::Char('/') => {
                state.is_searching = true;
            }
            KeyCode::Up => match state.focus {
                Focus::Repositories => {
                    if state.repo_index > 0 {
                        state.repo_index -= 1;
                        state.repo_state.select(Some(state.repo_index));
                        state.graph_scroll_y = 0;
                        state.graph_scroll_x = 0;
                    }
                }
                Focus::Details => {
                    if state.detail_index > 0 {
                        state.detail_index -= 1;
                    }
                }
                Focus::GitGraph => {
                    state.graph_scroll_y = state.graph_scroll_y.saturating_sub(1);
                }
                Focus::Dashboard => {}
            },
            KeyCode::Down => match state.focus {
                Focus::Repositories => {
                    if state.repo_index + 1 < state.repositories.len() {
                        state.repo_index += 1;
                        state.repo_state.select(Some(state.repo_index));
                        state.graph_scroll_y = 0;
                        state.graph_scroll_x = 0;
                    }
                }
                Focus::Details => {
                    if !state.repositories.is_empty() {
                        let repo = &state.repositories[state.repo_index];
                        let total = repo.branches.len() + repo.stashes.len() + repo.worktrees.len();
                        if state.detail_index + 1 < total {
                            state.detail_index += 1;
                        }
                    }
                }
                Focus::GitGraph => {
                    if let Some(repo) = state.repositories.get(state.repo_index) {
                        let bounds = if let Some(l) = &repo.graph_lines {
                            l.len()
                        } else {
                            0
                        };
                        if (state.graph_scroll_y as usize) + 1 < bounds {
                            state.graph_scroll_y += 1;
                        }
                    }
                }
                Focus::Dashboard => {}
            },
            _ => {}
        }
    }
    Ok(())
}

/// The selectable item at `index` in the details list (branches, then stashes,
/// then worktrees), or `None` for protected items.
fn detail_item(
    repo: &crate::git::RepoStatus,
    index: usize,
    config: &crate::config::Config,
) -> Option<(crate::ui::selection::ItemKind, String)> {
    use crate::ui::selection::ItemKind;

    if let Some(branch) = repo.branches.get(index) {
        return crate::cleanup::branch_protection(repo, branch, config)
            .is_none()
            .then(|| (ItemKind::Branch, branch.name.clone()));
    }
    let index = index - repo.branches.len();
    if let Some(stash) = repo.stashes.get(index) {
        return Some((ItemKind::Stash, stash.sha.clone()));
    }
    let worktree = repo.worktrees.get(index - repo.stashes.len())?;
    crate::cleanup::worktree_protection(worktree)
        .is_none()
        .then(|| (ItemKind::Worktree, worktree.path.clone()))
}

/// Selects merged and gone branches in the given repositories.
fn smart_select(state: &mut AppState, repo_indices: &[usize]) {
    for &i in repo_indices {
        let Some(repo) = state.repositories.get(i) else {
            continue;
        };
        for branch in &repo.branches {
            if crate::cleanup::is_smart_candidate(repo, branch, &state.config) {
                state.selection.insert(
                    &repo.path,
                    crate::ui::selection::ItemKind::Branch,
                    &branch.name,
                );
            }
        }
    }
}
