use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
};
use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

pub mod components;
pub mod events;
pub mod keymap;
pub mod loader;
pub mod selection;
pub mod state;
pub mod theme;

#[cfg(test)]
mod tests;

pub use state::{AppState, ScannerEvent, UiAction};

pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Input polling interval while something is animating (spinners)...
const ANIMATION_TICK: Duration = Duration::from_millis(80);
/// ...and while idle: only background events can change the screen.
const IDLE_TICK: Duration = Duration::from_millis(250);

pub fn run_tui(
    mut state: AppState,
    rx: Receiver<ScannerEvent>,
    tx: Sender<ScannerEvent>,
    worker: crate::worker::Worker,
) -> io::Result<()> {
    install_panic_hook();
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut dirty = true;

    loop {
        while let Ok(event) = rx.try_recv() {
            apply_event(&mut state, event, &worker);
            dirty = true;
        }
        if let Some(action) = state.action.take() {
            start_execution(&mut state, action, &tx);
            dirty = true;
        }
        request_loads(&mut state, &tx);
        if state.expire_toast() {
            dirty = true;
        }

        let animating = state.is_animating();
        if dirty || animating {
            terminal.draw(|f| draw(f, &mut state))?;
            dirty = false;
        }

        let tick = if animating { ANIMATION_TICK } else { IDLE_TICK };
        if events::handle_events(&mut state, tick)? {
            dirty = true;
        }

        // Handle self-update request
        if state.is_updating {
            // Exit TUI cleanly before performing update
            disable_raw_mode()?;
            stdout().execute(LeaveAlternateScreen)?;
            println!("🔄 Updating sloth...");
            match crate::updater::perform_update() {
                Ok(()) => {
                    println!("✅ Update complete! Please restart sloth.");
                    std::process::exit(0);
                }
                Err(e) => {
                    println!("❌ Update failed: {}", e);
                    // Re-enter TUI
                    enable_raw_mode()?;
                    stdout().execute(EnterAlternateScreen)?;
                    terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
                    state.is_updating = false;
                    dirty = true;
                    continue;
                }
            }
        }

        if state.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

/// Runs the confirmed action in the background; progress comes back as events.
fn start_execution(state: &mut AppState, action: UiAction, tx: &Sender<ScannerEvent>) {
    use crate::engine::{Observer, OpResult};

    let plans = build_plans(state, action);
    if plans.is_empty() {
        return;
    }
    state.execution = Some(state::Execution {
        total: plans.iter().map(|p| p.operations.len()).sum(),
        ..Default::default()
    });

    let record = crate::journal::Journal::open_default()
        .map(|journal| journal.observer(crate::git::stats::now_ts()));
    let progress = tx.clone();
    let observer: Observer = std::sync::Arc::new(move |result: &OpResult| {
        if let Some(record) = &record {
            record(result);
        }
        let _ = progress.send(ScannerEvent::OperationDone(result.clone()));
    });
    let done = tx.clone();
    tokio::spawn(async move {
        crate::engine::execute(plans, false, crate::sys::RealSystem, Some(observer)).await;
        let _ = done.send(ScannerEvent::ExecutionFinished);
    });
}

/// Leaves raw mode and the alternate screen before a panic message is
/// printed, so a crash does not leave the terminal unusable.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        default_hook(info);
    }));
}

/// Applies a background event to the state.
fn apply_event(state: &mut AppState, event: ScannerEvent, worker: &crate::worker::Worker) {
    let find = |state: &AppState, path: &std::path::Path| {
        state.repositories.iter().position(|r| r.path == path)
    };
    match event {
        ScannerEvent::RepoFound(path) => {
            state.scanned_count += 1;
            state
                .repositories
                .push(crate::git::RepoStatus::pending(path));
        }
        ScannerEvent::ScanComplete => state.is_scanning = false,
        ScannerEvent::RepoAnalyzed(repo) => {
            match find(state, &repo.path) {
                Some(i) => {
                    state.selection.retain_existing(&repo);
                    state.repositories[i] = repo;
                }
                None => state.repositories.push(repo),
            }
            state.analyzed_count += 1;
        }
        ScannerEvent::RepoFailed { path, error } => {
            if let Some(i) = find(state, &path) {
                state.repositories[i].analyzed = true;
                state.repositories[i].error = Some(error);
            }
            state.analyzed_count += 1;
        }
        ScannerEvent::SizePartial {
            path,
            size_bytes,
            untracked_size_bytes,
        } => {
            if let Some(i) = find(state, &path) {
                let repo = &mut state.repositories[i];
                repo.size_bytes = size_bytes;
                repo.untracked_size_bytes = Some(untracked_size_bytes);
            }
        }
        ScannerEvent::SizeComputed {
            path,
            size_bytes,
            untracked_size_bytes,
            worktree_sizes,
        } => {
            if let Some(i) = find(state, &path) {
                let repo = &mut state.repositories[i];
                repo.size_bytes = size_bytes;
                repo.untracked_size_bytes = untracked_size_bytes;
                repo.size_finalized = true;
                for wt in repo.worktrees.iter_mut() {
                    if let Some(&size) = worktree_sizes.get(&wt.path) {
                        wt.size_bytes = size;
                    }
                }
            }
        }
        ScannerEvent::AnalysisComplete => state.is_analyzing = false,
        ScannerEvent::UpdateAvailable(version) => {
            state.update_available = Some(version);
        }
        ScannerEvent::DeepCleanPreview(lines) => {
            state.confirm_preview_lines = Some(lines);
            state.preview_loading = false;
        }
        ScannerEvent::GraphLoaded { path, lines } => {
            state.graph_loading.remove(&path);
            if let Some(i) = find(state, &path) {
                state.repositories[i].graph_lines = Some(lines);
            }
        }
        ScannerEvent::OperationDone(result) => {
            if let Some(execution) = &mut state.execution {
                execution.results.push(result);
            }
        }
        ScannerEvent::ExecutionFinished => {
            let Some(execution) = &mut state.execution else {
                return;
            };
            execution.finished = true;
            state.selection.forget_done(&execution.results);

            // Re-analyze what changed; keep the repository order stable.
            let mut touched: Vec<PathBuf> = Vec::new();
            for result in &execution.results {
                if !touched.contains(&result.repo) {
                    touched.push(result.repo.clone());
                }
            }
            for repo in state
                .repositories
                .iter_mut()
                .filter(|r| touched.contains(&r.path))
            {
                repo.analyzed = false;
                repo.graph_lines = None;
            }
            state.is_analyzing = true;
            worker.refresh(touched);
        }
        ScannerEvent::DiffLoaded { id, lines } => {
            if id == state.diff_request_id {
                state.diff_lines = Some(lines);
            }
        }
    }
}

/// Starts background loads for whatever the current view needs.
fn request_loads(state: &mut AppState, tx: &Sender<ScannerEvent>) {
    if (state.show_graph || state.graph_maximized)
        && let Some(repo) = state.repositories.get(state.repo_index)
        && repo.analyzed
        && repo.graph_lines.is_none()
        && !state.graph_loading.contains(&repo.path)
    {
        state.graph_loading.insert(repo.path.clone());
        loader::load_graph(tx, repo.path.clone());
    }

    if matches!(state.pending_action, Some(UiAction::DeepClean))
        && state.confirm_preview_lines.is_none()
        && !state.preview_loading
    {
        state.preview_loading = true;
        let paths: Vec<PathBuf> = if state.selected_repositories.is_empty() {
            state
                .repositories
                .get(state.repo_index)
                .map(|r| vec![r.path.clone()])
                .unwrap_or_default()
        } else {
            state
                .selected_repositories
                .iter()
                .filter_map(|&i| state.repositories.get(i).map(|r| r.path.clone()))
                .collect()
        };
        loader::load_deep_clean_preview(tx, paths, state.config.deep_clean_keep.clone());
    }

    if state.diff_modal_open && state.diff_lines.is_none() && !state.diff_requested {
        state.diff_requested = true;
        state.diff_request_id += 1;
        let Some(repo) = state.repositories.get(state.repo_index) else {
            return;
        };
        let target = if let Some(b) = repo.branches.get(state.detail_index) {
            Some(loader::DiffTarget::Branch {
                name: b.name.clone(),
                base: repo.default_branch.clone().filter(|d| d != &b.name),
            })
        } else {
            repo.stashes
                .get(state.detail_index - repo.branches.len())
                .map(|s| loader::DiffTarget::Stash { sha: s.sha.clone() })
        };
        match target {
            Some(target) => loader::load_diff(tx, state.diff_request_id, repo.path.clone(), target),
            None => state.diff_lines = Some(vec!["No diff for worktrees.".to_string()]),
        }
    }
}

pub(crate) fn draw(f: &mut Frame, state: &mut AppState) {
    let [header, tabs, body, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(f.area());

    components::header::render(f, state, header);
    components::tabs::render(f, state, tabs);
    match state.tab {
        state::Tab::Repos => draw_repos_tab(f, state, body),
        state::Tab::Dashboard => components::dashboard::render(f, state, body),
    }
    components::help::render_status_bar(f, state, status);

    // Overlays, from least to most important.
    components::diff_modal::render(f, state, f.area());
    components::confirm_modal::render(f, state, f.area());
    components::execution_modal::render(f, state, f.area());
    components::help::render_overlay(f, state, f.area());
}

fn draw_repos_tab(f: &mut Frame, state: &mut AppState, area: ratatui::layout::Rect) {
    if state.graph_maximized {
        components::graph::render(f, state, area);
        return;
    }
    let constraints = if state.show_graph {
        vec![
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ]
    } else {
        vec![Constraint::Percentage(40), Constraint::Percentage(60)]
    };
    let panes = Layout::horizontal(constraints).split(area);
    components::repositories::render(f, state, panes[0]);
    components::details::render(f, state, panes[1]);
    if state.show_graph {
        components::graph::render(f, state, panes[2]);
    }
}

/// Turns the confirmed action and the current selection into engine plans.
fn build_plans(state: &AppState, action: UiAction) -> Vec<crate::engine::RepoPlan> {
    use crate::engine::{Operation, RepoPlan};

    if matches!(action, UiAction::CleanRepo) {
        return state.selection.plans(&state.repositories, &state.config);
    }

    let focused = state.repositories.get(state.repo_index);
    let targets: Vec<&crate::git::RepoStatus> = if state.selected_repositories.is_empty() {
        focused.into_iter().collect()
    } else {
        let mut indices: Vec<usize> = state.selected_repositories.iter().copied().collect();
        indices.sort_unstable();
        indices
            .into_iter()
            .filter_map(|i| state.repositories.get(i))
            .collect()
    };

    targets
        .into_iter()
        .map(|repo| {
            let operations = match action {
                UiAction::CleanRepo => unreachable!("handled above"),
                UiAction::PruneRemotes => vec![Operation::PruneRemotes],
                UiAction::GarbageCollect => vec![Operation::GarbageCollect],
                UiAction::DeepClean => vec![Operation::DeepClean {
                    keep: state.config.deep_clean_keep.clone(),
                }],
            };
            RepoPlan {
                repo: repo.path.clone(),
                operations,
            }
        })
        .filter(|plan| !plan.operations.is_empty())
        .collect()
}
