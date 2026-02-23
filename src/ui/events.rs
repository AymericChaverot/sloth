use crate::ui::state::{AppState, Focus, UiAction};
use crossterm::event::{self, Event, KeyCode};
use std::time::Duration;

pub fn handle_events(state: &mut AppState) -> std::io::Result<()> {
    if event::poll(Duration::from_millis(16))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => state.should_quit = true,
                    KeyCode::Char('u') => {
                        if state.update_available.is_some() && !state.is_updating {
                            state.is_updating = true;
                        }
                    }
                    KeyCode::Esc => {
                        if state.focus == Focus::GitGraph {
                            state.graph_maximized = false;
                            state.focus = Focus::Details;
                        }
                    }
                    KeyCode::Char('f') | KeyCode::Char('m') => {
                        // support 'm' logic from earlier requests
                        if state.focus == Focus::GitGraph {
                            state.graph_maximized = !state.graph_maximized;
                        }
                    }
                    KeyCode::Char('g') => {
                        state.show_graph = !state.show_graph;
                        if !state.show_graph && state.focus == Focus::GitGraph {
                            state.graph_maximized = false;
                            state.focus = Focus::Details;
                        }
                    }
                    KeyCode::Right => {
                        if state.focus == Focus::Repositories && !state.repositories.is_empty() {
                            state.focus = Focus::Details;
                            state.detail_index = 0;
                        } else if state.focus == Focus::Details && state.show_graph {
                            state.focus = Focus::GitGraph;
                        } else if state.focus == Focus::GitGraph {
                            state.graph_scroll_x = state.graph_scroll_x.saturating_add(2);
                        }
                    }
                    KeyCode::Left => {
                        if state.focus == Focus::Details {
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
                        if state.focus == Focus::Details && !state.repositories.is_empty() {
                            let repo = &state.repositories[state.repo_index];
                            let b_len = repo.branches.len();
                            if state.detail_index < b_len {
                                // It's a branch
                                let b_name = repo.branches[state.detail_index].name.clone();
                                let set =
                                    state.selected_branches.entry(state.repo_index).or_default();
                                if set.contains(&b_name) {
                                    set.remove(&b_name);
                                } else {
                                    set.insert(b_name);
                                }
                            } else {
                                // It's a stash
                                let s_idx = state.detail_index.saturating_sub(b_len);
                                if s_idx < repo.stashes.len() {
                                    let s_id = repo.stashes[s_idx].index;
                                    let set =
                                        state.selected_stashes.entry(state.repo_index).or_default();
                                    if set.contains(&s_id) {
                                        set.remove(&s_id);
                                    } else {
                                        set.insert(s_id);
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Enter => {
                        if state.focus == Focus::Details {
                            state.action = Some(UiAction::CleanRepo);
                            state.should_quit = true;
                        }
                    }
                    KeyCode::Char('p') => {
                        if state.focus == Focus::Repositories {
                            state.action = Some(UiAction::PruneRemotes);
                            state.should_quit = true;
                        }
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
                                let total = repo.branches.len() + repo.stashes.len();
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
                    },
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
