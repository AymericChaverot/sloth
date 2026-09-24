use crossterm::{
    ExecutableCommand,
    event::{DisableMouseCapture, EnableMouseCapture},
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
pub mod views;

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
    enter_terminal(state.config.mouse)?;

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
        if let Some(request) = state.refresh.take() {
            refresh(&mut state, request, &worker);
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
            leave_terminal()?;
            println!("🔄 Updating sloth...");
            match crate::updater::perform_update() {
                Ok(()) => {
                    println!("✅ Update complete! Please restart sloth.");
                    std::process::exit(0);
                }
                Err(e) => {
                    println!("❌ Update failed: {}", e);
                    // Re-enter TUI
                    enter_terminal(state.config.mouse)?;
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

    leave_terminal()?;

    Ok(())
}

/// Runs the confirmed action in the background; progress comes back as events.
pub(crate) fn start_execution(state: &mut AppState, action: UiAction, tx: &Sender<ScannerEvent>) {
    use crate::engine::{Observer, OpResult};

    let plans = build_plans(state, action);
    if plans.is_empty() {
        return;
    }
    state.execution = Some(state::Execution {
        total: plans.iter().map(|p| p.operations.len()).sum(),
        ..Default::default()
    });

    let record = state
        .journal
        .clone()
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

fn enter_terminal(mouse: bool) -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    if mouse {
        stdout().execute(EnableMouseCapture)?;
    }
    Ok(())
}

fn leave_terminal() -> io::Result<()> {
    stdout().execute(DisableMouseCapture)?;
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

/// Leaves raw mode and the alternate screen before a panic message is
/// printed, so a crash does not leave the terminal unusable.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = leave_terminal();
        default_hook(info);
    }));
}

/// Applies a background event to the state.
pub(crate) fn apply_event(
    state: &mut AppState,
    event: ScannerEvent,
    worker: &crate::worker::Worker,
) {
    let find = |state: &AppState, path: &std::path::Path| {
        state.repositories.iter().position(|r| r.path == path)
    };
    match event {
        ScannerEvent::RepoFound(path) => {
            state
                .repositories
                .push(crate::git::RepoStatus::pending(path));
        }
        ScannerEvent::ScanComplete => state.is_scanning = false,
        ScannerEvent::RepoAnalyzed(repo) => match find(state, &repo.path) {
            Some(i) => {
                state.selection.retain_existing(&repo);
                state.repositories[i] = repo;
            }
            None => state.repositories.push(repo),
        },
        ScannerEvent::RepoFailed { path, error } => {
            if let Some(i) = find(state, &path) {
                state.repositories[i].analyzed = true;
                state.repositories[i].error = Some(error);
            }
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

            // Re-analyze what changed.
            let mut touched: Vec<PathBuf> = Vec::new();
            for result in &execution.results {
                if !touched.contains(&result.repo) {
                    touched.push(result.repo.clone());
                }
            }
            refresh(state, state::Refresh::Repos(touched), worker);
        }
        ScannerEvent::DiffLoaded { id, lines } => {
            if id == state.diff_request_id {
                state.diff_lines = Some(lines);
            }
        }
    }
}

/// Re-analyzes some repositories, or rediscovers all of them. The selection
/// is kept: items that no longer exist are dropped as results come in.
fn refresh(state: &mut AppState, request: state::Refresh, worker: &crate::worker::Worker) {
    match request {
        state::Refresh::Repos(paths) => {
            for repo in state
                .repositories
                .iter_mut()
                .filter(|r| paths.contains(&r.path))
            {
                repo.analyzed = false;
                repo.error = None;
                repo.graph_lines = None;
            }
            state.is_analyzing = true;
            worker.refresh(paths);
        }
        state::Refresh::Rescan => {
            state.repositories.clear();
            state.graph_loading.clear();
            state.is_scanning = true;
            state.is_analyzing = true;
            worker.scan(state.root.clone(), state.config.scan_exclude.clone());
        }
    }
}

/// Starts background loads for whatever the current view needs.
fn request_loads(state: &mut AppState, tx: &Sender<ScannerEvent>) {
    if (state.show_graph || state.graph_maximized)
        && let Some(repo) = state.focused()
        && repo.analyzed
        && repo.graph_lines.is_none()
        && !state.graph_loading.contains(&repo.path)
    {
        let path = repo.path.clone();
        state.graph_loading.insert(path.clone());
        loader::load_graph(tx, path);
    }

    if matches!(state.pending_action, Some(UiAction::DeepClean))
        && state.confirm_preview_lines.is_none()
        && !state.preview_loading
    {
        state.preview_loading = true;
        loader::load_deep_clean_preview(
            tx,
            state.action_targets(),
            state.config.deep_clean_keep.clone(),
        );
    }

    if let Some((repo, target)) = &state.diff_target
        && state.diff_lines.is_none()
        && !state.diff_requested
    {
        state.diff_requested = true;
        state.diff_request_id += 1;
        loader::load_diff(tx, state.diff_request_id, repo.clone(), target.clone());
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
        state::Tab::Branches => components::branches::render(f, state, body),
        state::Tab::Queue => components::queue::render(f, state, body),
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
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(area);
    if state.show_graph {
        // Keep the tables readable: stack them and give the graph the right side.
        let [repos, details] =
            Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(left);
        components::repositories::render(f, state, repos);
        components::details::render(f, state, details);
        components::graph::render(f, state, right);
    } else {
        components::repositories::render(f, state, left);
        components::details::render(f, state, right);
    }
}

/// Turns the confirmed action and the current selection into engine plans.
fn build_plans(state: &AppState, action: UiAction) -> Vec<crate::engine::RepoPlan> {
    use crate::engine::{Operation, RepoPlan};

    let operation = match action {
        UiAction::CleanRepo => return state.selection.plans(&state.repositories, &state.config),
        UiAction::PruneRemotes => Operation::PruneRemotes,
        UiAction::GarbageCollect => Operation::GarbageCollect,
        UiAction::DeepClean => Operation::DeepClean {
            keep: state.config.deep_clean_keep.clone(),
        },
    };
    state
        .action_targets()
        .into_iter()
        .map(|repo| RepoPlan {
            repo,
            operations: vec![operation.clone()],
        })
        .collect()
}
