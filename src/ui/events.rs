use crate::ui::selection::ItemKind;
use crate::ui::state::{AppState, Focus, Tab, UiAction};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use std::time::Duration;

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
    if state.diff_modal_open {
        return diff_key(state, key.code);
    }
    if state.is_searching {
        return search_key(state, key.code);
    }
    if global_key(state, key.code) {
        return;
    }
    match state.tab {
        Tab::Repos => repos_tab_key(state, key.code),
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
        KeyCode::Esc | KeyCode::Char('v') | KeyCode::Char('q') => state.diff_modal_open = false,
        KeyCode::Up => state.diff_scroll = state.diff_scroll.saturating_sub(1),
        KeyCode::Down => state.diff_scroll = state.diff_scroll.saturating_add(1),
        KeyCode::PageUp => state.diff_scroll = state.diff_scroll.saturating_sub(10),
        KeyCode::PageDown => state.diff_scroll = state.diff_scroll.saturating_add(10),
        KeyCode::Home => state.diff_scroll = 0,
        _ => {}
    }
}

fn search_key(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Enter => state.is_searching = false,
        KeyCode::Backspace => {
            state.search_query.pop();
        }
        KeyCode::Char(c) => state.search_query.push(c),
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

fn repos_tab_key(state: &mut AppState, code: KeyCode) {
    match state.focus {
        Focus::Repositories => repo_list_key(state, code),
        Focus::Details => details_key(state, code),
        Focus::GitGraph => graph_key(state, code),
    }
}

fn repo_list_key(state: &mut AppState, code: KeyCode) {
    if state.repositories.is_empty() {
        return;
    }
    match code {
        KeyCode::Up | KeyCode::Char('k') => move_repo_cursor(state, -1),
        KeyCode::Down | KeyCode::Char('j') => move_repo_cursor(state, 1),
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
            state.focus = Focus::Details;
            state.detail_index = 0;
        }
        KeyCode::Char(' ') => {
            if !state.selected_repositories.remove(&state.repo_index) {
                state.selected_repositories.insert(state.repo_index);
            }
        }
        KeyCode::Char('a') => {
            // Smart select across every marked repository (or the focused one).
            let targets: Vec<usize> = if state.selected_repositories.is_empty() {
                vec![state.repo_index]
            } else {
                state.selected_repositories.iter().copied().collect()
            };
            smart_select(state, &targets);
        }
        KeyCode::Char('p') => state.pending_action = Some(UiAction::PruneRemotes),
        KeyCode::Char('c') => state.pending_action = Some(UiAction::GarbageCollect),
        KeyCode::Char('X') => state.pending_action = Some(UiAction::DeepClean),
        KeyCode::Char('g') => state.show_graph = !state.show_graph,
        KeyCode::Char('/') => state.is_searching = true,
        _ => {}
    }
}

fn details_key(state: &mut AppState, code: KeyCode) {
    let Some(repo) = state.repositories.get(state.repo_index) else {
        return;
    };
    let item_count = repo.branches.len() + repo.stashes.len() + repo.worktrees.len();
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.detail_index = state.detail_index.saturating_sub(1)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.detail_index + 1 < item_count {
                state.detail_index += 1;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => state.focus = Focus::Repositories,
        KeyCode::Right | KeyCode::Char('l') if state.show_graph => state.focus = Focus::GitGraph,
        KeyCode::Char(' ') => toggle_detail_item(state),
        KeyCode::Char('a') => smart_select(state, &[state.repo_index]),
        KeyCode::Char('A') => {
            let path = repo.path.clone();
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
            let path = repo.path.clone();
            state.selection.clear_repo(&path);
        }
        KeyCode::Char('v') => {
            state.diff_modal_open = true;
            state.diff_lines = None;
            state.diff_requested = false;
            state.diff_scroll = 0;
        }
        KeyCode::Char('g') => state.show_graph = !state.show_graph,
        _ => {}
    }
}

fn graph_key(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Up => state.graph_scroll_y = state.graph_scroll_y.saturating_sub(1),
        KeyCode::Down => {
            let lines = state
                .repositories
                .get(state.repo_index)
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

fn move_repo_cursor(state: &mut AppState, delta: isize) {
    let last = state.repositories.len().saturating_sub(1);
    let next = state.repo_index.saturating_add_signed(delta).min(last);
    if next != state.repo_index {
        state.repo_index = next;
        state.repo_state.select(Some(next));
        state.graph_scroll_y = 0;
        state.graph_scroll_x = 0;
    }
}

fn toggle_detail_item(state: &mut AppState) {
    let Some(repo) = state.repositories.get(state.repo_index) else {
        return;
    };
    match detail_item(repo, state.detail_index, &state.config) {
        Ok(Some((kind, id))) => {
            let path = repo.path.clone();
            state.selection.toggle(&path, kind, &id);
        }
        Ok(None) => {}
        Err(reason) => state.warn(format!("Protected: {reason}")),
    }
}

/// The selectable item at `index` in the details list (branches, then stashes,
/// then worktrees). Protected items give the reason they cannot be selected.
fn detail_item(
    repo: &crate::git::RepoStatus,
    index: usize,
    config: &crate::config::Config,
) -> Result<Option<(ItemKind, String)>, &'static str> {
    if let Some(branch) = repo.branches.get(index) {
        return match crate::cleanup::branch_protection(repo, branch, config) {
            Some(p) => Err(p.label()),
            None => Ok(Some((ItemKind::Branch, branch.name.clone()))),
        };
    }
    let index = index - repo.branches.len();
    if let Some(stash) = repo.stashes.get(index) {
        return Ok(Some((ItemKind::Stash, stash.sha.clone())));
    }
    let Some(worktree) = repo.worktrees.get(index - repo.stashes.len()) else {
        return Ok(None);
    };
    match crate::cleanup::worktree_protection(worktree) {
        Some(p) => Err(p.label()),
        None => Ok(Some((ItemKind::Worktree, worktree.path.clone()))),
    }
}

/// Selects merged and gone branches in the given repositories.
fn smart_select(state: &mut AppState, repo_indices: &[usize]) {
    let mut added = 0;
    for &i in repo_indices {
        let Some(repo) = state.repositories.get(i) else {
            continue;
        };
        for branch in &repo.branches {
            if crate::cleanup::is_smart_candidate(repo, branch, &state.config)
                && !state
                    .selection
                    .contains(&repo.path, ItemKind::Branch, &branch.name)
            {
                state
                    .selection
                    .insert(&repo.path, ItemKind::Branch, &branch.name);
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
