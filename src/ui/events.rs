use crate::ui::loader::DiffTarget;
use crate::ui::selection::ItemKind;
use crate::ui::state::{AppState, Focus, Tab, UiAction};
use crate::ui::views::{self, DetailRow};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use std::path::PathBuf;
use std::time::Duration;

/// Rows moved by Page Up / Page Down.
const PAGE: isize = 10;

/// Waits up to `timeout` for input. Returns whether the screen must be redrawn.
pub fn handle_events(state: &mut AppState, timeout: Duration) -> std::io::Result<bool> {
    if !event::poll(timeout)? {
        return Ok(false);
    }
    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            handle_key(state, key);
            Ok(true)
        }
        Event::Resize(..) => Ok(true),
        _ => Ok(false),
    }
}

pub fn handle_key(state: &mut AppState, key: KeyEvent) {
    // Overlays capture input, from the most to the least important.
    if state.execution.is_some() {
        return execution_key(state, key.code);
    }
    if state.show_help {
        if matches!(
            key.code,
            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q')
        ) {
            state.show_help = false;
        }
        return;
    }
    if state.pending_action.is_some() {
        return confirm_key(state, key.code);
    }
    if state.diff_target.is_some() {
        return diff_key(state, key.code);
    }
    if state.editing_filter {
        return filter_key(state, key.code);
    }
    if global_key(state, key.code) {
        return;
    }
    match state.tab {
        Tab::Repos => match state.focus {
            Focus::Repositories => repo_list_key(state, key.code),
            Focus::Details => details_key(state, key.code),
            Focus::GitGraph => graph_key(state, key.code),
        },
        Tab::Dashboard => {}
    }
}

fn execution_key(state: &mut AppState, code: KeyCode) {
    let Some(execution) = &mut state.execution else {
        return;
    };
    match code {
        KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') if execution.finished => {
            state.execution = None;
        }
        KeyCode::Up => execution.scroll = execution.scroll.saturating_sub(1),
        KeyCode::Down => execution.scroll = execution.scroll.saturating_add(1),
        _ => {}
    }
}

fn confirm_key(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            state.action = state.pending_action.take();
            state.confirm_preview_lines = None;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            state.pending_action = None;
            state.confirm_preview_lines = None;
        }
        _ => {}
    }
}

fn diff_key(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('v') | KeyCode::Char('q') => state.diff_target = None,
        KeyCode::Up | KeyCode::Char('k') => state.diff_scroll = state.diff_scroll.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => {
            state.diff_scroll = state.diff_scroll.saturating_add(1)
        }
        KeyCode::PageUp => state.diff_scroll = state.diff_scroll.saturating_sub(PAGE as u16),
        KeyCode::PageDown => state.diff_scroll = state.diff_scroll.saturating_add(PAGE as u16),
        KeyCode::Home => state.diff_scroll = 0,
        _ => {}
    }
}

fn filter_key(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Enter => state.editing_filter = false,
        KeyCode::Esc => {
            state.editing_filter = false;
            state.repo_filter.clear();
        }
        KeyCode::Backspace => {
            state.repo_filter.pop();
        }
        KeyCode::Char(c) => state.repo_filter.push(c),
        _ => {}
    }
}

/// Keys that work the same in every tab. Returns whether the key was used.
fn global_key(state: &mut AppState, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('q') => state.should_quit = true,
        KeyCode::Char('?') => state.show_help = true,
        KeyCode::Char('u') if state.update_available.is_some() && !state.is_updating => {
            state.is_updating = true;
        }
        KeyCode::Char(c @ '1'..='9') => {
            if let Some(tab) = Tab::ALL.get(c as usize - '1' as usize) {
                state.tab = *tab;
            }
        }
        KeyCode::Tab => state.tab = state.tab.next(),
        KeyCode::BackTab => state.tab = state.tab.previous(),
        KeyCode::Char('t') => {
            state.theme_index = (state.theme_index + 1) % crate::ui::theme::THEMES.len();
            let name = crate::ui::theme::get_theme(state.theme_index).name;
            state.config.save_theme(name);
            state.notify(format!("Theme: {name}"));
        }
        KeyCode::Char('C') => {
            if !state.selection.is_empty() {
                state.selection.clear();
                state.notify("Queue cleared");
            }
        }
        KeyCode::Char('x') => request_queue_run(state),
        _ => return false,
    }
    true
}

fn request_queue_run(state: &mut AppState) {
    if state.selection.is_empty() {
        state.warn("The queue is empty: select items with Space, or `a` for smart selection");
    } else {
        state.pending_action = Some(UiAction::CleanRepo);
    }
}

fn repo_list_key(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => state.move_repo_cursor(-1),
        KeyCode::Down | KeyCode::Char('j') => state.move_repo_cursor(1),
        KeyCode::PageUp => state.move_repo_cursor(-PAGE),
        KeyCode::PageDown => state.move_repo_cursor(PAGE),
        KeyCode::Home => state.move_repo_cursor(isize::MIN),
        KeyCode::End => state.move_repo_cursor(isize::MAX),
        KeyCode::Char('/') => state.editing_filter = true,
        KeyCode::Esc if !state.repo_filter.is_empty() => state.repo_filter.clear(),
        KeyCode::Esc => state.marked_repos.clear(),
        KeyCode::Char('s') => {
            state.repo_sort = state.repo_sort.next();
            state.notify(format!("Sorted by {}", state.repo_sort.label()));
        }
        KeyCode::Char('g') => state.show_graph = !state.show_graph,
        _ => {}
    }
    let Some(focused) = state.focused().map(|r| r.path.clone()) else {
        return;
    };
    // Keep the cursor on the same repository as the list changes.
    state.focus_repo(focused.clone());
    match code {
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => state.focus = Focus::Details,
        KeyCode::Char(' ') => {
            if !state.marked_repos.remove(&focused) {
                state.marked_repos.insert(focused);
            }
            state.move_repo_cursor(1);
        }
        KeyCode::Char('a') => smart_select(state, &state.action_targets()),
        KeyCode::Char('p') => state.pending_action = Some(UiAction::PruneRemotes),
        KeyCode::Char('c') => state.pending_action = Some(UiAction::GarbageCollect),
        KeyCode::Char('X') => state.pending_action = Some(UiAction::DeepClean),
        _ => {}
    }
}

fn details_key(state: &mut AppState, code: KeyCode) {
    let Some(repo) = state.focused() else {
        state.focus = Focus::Repositories;
        return;
    };
    let path = repo.path.clone();
    let rows = views::detail_rows(repo);
    let last = rows.len().saturating_sub(1);
    let current = rows.get(state.detail_index).copied();
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.detail_index = state.detail_index.saturating_sub(1)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.detail_index = (state.detail_index + 1).min(last)
        }
        KeyCode::PageUp => state.detail_index = state.detail_index.saturating_sub(PAGE as usize),
        KeyCode::PageDown => state.detail_index = (state.detail_index + PAGE as usize).min(last),
        KeyCode::Home => state.detail_index = 0,
        KeyCode::End => state.detail_index = last,
        KeyCode::Left | KeyCode::Char('h') => state.focus = Focus::Repositories,
        KeyCode::Right | KeyCode::Char('l') if state.show_graph => state.focus = Focus::GitGraph,
        KeyCode::Char(' ') => {
            if let Some(row) = current {
                let repo = state.focused().expect("checked above");
                match row.protection(repo, &state.config) {
                    Some(p) => state.warn(format!("Protected: {}", p.label())),
                    None => {
                        let (kind, id) = row.item();
                        let id = id.to_string();
                        state.selection.toggle(&path, kind, &id);
                        state.detail_index = (state.detail_index + 1).min(last);
                    }
                }
            }
        }
        KeyCode::Char('a') => smart_select(state, &[path]),
        KeyCode::Char('A') => {
            let names: Vec<String> = repo
                .branches
                .iter()
                .filter(|b| crate::cleanup::branch_protection(repo, b, &state.config).is_none())
                .map(|b| b.name.clone())
                .collect();
            for name in &names {
                state.selection.insert(&path, ItemKind::Branch, name);
            }
            state.notify(format!("{} branches selected", names.len()));
        }
        KeyCode::Enter => request_queue_run(state),
        KeyCode::Esc => {
            if state.selection.repo(&path).is_some() {
                state.selection.clear_repo(&path);
                state.notify("Selection cleared for this repository");
            } else {
                state.focus = Focus::Repositories;
            }
        }
        KeyCode::Char('v') => {
            let default = repo.default_branch.clone();
            match current {
                Some(DetailRow::Branch(b)) => {
                    let target = DiffTarget::Branch {
                        name: b.name.clone(),
                        base: default.filter(|d| d != &b.name),
                    };
                    state.open_diff(path, target);
                }
                Some(DetailRow::Stash(s)) => {
                    let target = DiffTarget::Stash { sha: s.sha.clone() };
                    state.open_diff(path, target);
                }
                _ => state.warn("No diff for worktrees"),
            }
        }
        KeyCode::Char('g') => state.show_graph = !state.show_graph,
        _ => {}
    }
}

fn graph_key(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.graph_scroll_y = state.graph_scroll_y.saturating_sub(1)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let lines = state
                .focused()
                .and_then(|r| r.graph_lines.as_ref())
                .map_or(0, Vec::len);
            if (state.graph_scroll_y as usize) + 1 < lines {
                state.graph_scroll_y += 1;
            }
        }
        KeyCode::Right => state.graph_scroll_x = state.graph_scroll_x.saturating_add(2),
        KeyCode::Left if state.graph_scroll_x > 0 => {
            state.graph_scroll_x = state.graph_scroll_x.saturating_sub(2);
        }
        KeyCode::Left | KeyCode::Esc | KeyCode::Char('g') => {
            state.graph_maximized = false;
            state.focus = Focus::Details;
            if code == KeyCode::Char('g') {
                state.show_graph = false;
            }
        }
        KeyCode::Char('f') | KeyCode::Char('m') => state.graph_maximized = !state.graph_maximized,
        _ => {}
    }
}

/// Selects merged and gone branches in the given repositories.
fn smart_select(state: &mut AppState, repos: &[PathBuf]) {
    let mut added = 0;
    for path in repos {
        let Some(repo) = state.repo(path) else {
            continue;
        };
        let names: Vec<String> = repo
            .branches
            .iter()
            .filter(|b| crate::cleanup::is_smart_candidate(repo, b, &state.config))
            .map(|b| b.name.clone())
            .collect();
        for name in names {
            if !state.selection.contains(path, ItemKind::Branch, &name) {
                state.selection.insert(path, ItemKind::Branch, &name);
                added += 1;
            }
        }
    }
    if added == 0 {
        state.notify("No merged or gone branches to add");
    } else {
        state.notify(format!("{added} merged/gone branches added to the queue"));
    }
}
