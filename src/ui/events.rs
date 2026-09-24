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
                    state.selected_branches.remove(&state.repo_index);
                    state.selected_stashes.remove(&state.repo_index);
                    state.selected_worktrees.remove(&state.repo_index);
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
                } else if state.focus == Focus::Details && !state.repositories.is_empty() {
                    let repo = &state.repositories[state.repo_index];
                    let b_len = repo.branches.len();
                    if state.detail_index < b_len {
                        // It's a branch
                        let branch = &repo.branches[state.detail_index];
                        let protected =
                            crate::cleanup::branch_protection(repo, branch, &state.config)
                                .is_some();
                        let b_name = branch.name.clone();
                        let set = state.selected_branches.entry(state.repo_index).or_default();
                        if protected || set.contains(&b_name) {
                            set.remove(&b_name);
                        } else {
                            set.insert(b_name);
                        }
                    } else {
                        let s_idx = state.detail_index.saturating_sub(b_len);
                        if s_idx < repo.stashes.len() {
                            // It's a stash
                            let s_id = repo.stashes[s_idx].index;
                            let set = state.selected_stashes.entry(state.repo_index).or_default();
                            if set.contains(&s_id) {
                                set.remove(&s_id);
                            } else {
                                set.insert(s_id);
                            }
                        } else {
                            // It's a worktree
                            let w_idx = s_idx.saturating_sub(repo.stashes.len());
                            if w_idx < repo.worktrees.len()
                                && crate::cleanup::worktree_protection(&repo.worktrees[w_idx])
                                    .is_none()
                            {
                                let w_path = repo.worktrees[w_idx].path.clone();
                                let set = state
                                    .selected_worktrees
                                    .entry(state.repo_index)
                                    .or_default();
                                if set.contains(&w_path) {
                                    set.remove(&w_path);
                                } else {
                                    set.insert(w_path);
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char('a')
                if state.focus == Focus::Details && !state.repositories.is_empty() =>
            {
                let repo = &state.repositories[state.repo_index];
                let set = state.selected_branches.entry(state.repo_index).or_default();
                for branch in &repo.branches {
                    if crate::cleanup::is_smart_candidate(repo, branch, &state.config) {
                        set.insert(branch.name.clone());
                    }
                }
            }
            KeyCode::Char('A')
                if state.focus == Focus::Details && !state.repositories.is_empty() =>
            {
                let repo = &state.repositories[state.repo_index];
                let set = state.selected_branches.entry(state.repo_index).or_default();
                for branch in &repo.branches {
                    if crate::cleanup::branch_protection(repo, branch, &state.config).is_none() {
                        set.insert(branch.name.clone());
                    }
                }
            }
            KeyCode::Enter if state.focus == Focus::Details => {
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
