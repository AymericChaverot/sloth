use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
};
use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

pub mod components;
pub mod events;
pub mod loader;
pub mod selection;
pub mod state;
pub mod theme;

pub use state::{AppState, ScannerEvent, UiAction};

pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Input polling interval while something is animating (spinners)...
const ANIMATION_TICK: Duration = Duration::from_millis(80);
/// ...and while idle: only background events can change the screen.
const IDLE_TICK: Duration = Duration::from_millis(250);

type TuiResult = io::Result<Option<Vec<crate::engine::RepoPlan>>>;

pub fn run_tui(
    mut state: AppState,
    rx: Receiver<ScannerEvent>,
    tx: Sender<ScannerEvent>,
) -> TuiResult {
    install_panic_hook();
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut dirty = true;

    loop {
        while let Ok(event) = rx.try_recv() {
            apply_event(&mut state, event);
            dirty = true;
        }
        request_loads(&mut state, &tx);

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

    Ok(state
        .action
        .take()
        .map(|action| build_plans(&state, action)))
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
fn apply_event(state: &mut AppState, event: ScannerEvent) {
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
                Some(i) => state.repositories[i] = repo,
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

fn draw(f: &mut Frame, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(7),
                Constraint::Min(10),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.area());

    // Header (rainbow ASCII art)
    components::header::render(f, state, chunks[0]);

    // Main content panes
    let main_chunks = if state.graph_maximized {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)].as_ref())
            .split(chunks[1])
    } else if state.show_graph {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                    Constraint::Percentage(40),
                ]
                .as_ref(),
            )
            .split(chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
            .split(chunks[1])
    };

    if state.focus == state::Focus::Dashboard {
        components::dashboard::render(f, state, chunks[1]);
    } else {
        if !state.graph_maximized {
            components::repositories::render(f, state, main_chunks[0]);
            components::details::render(f, state, main_chunks[1]);
        }

        if state.show_graph || state.graph_maximized {
            let target_chunk = if state.graph_maximized {
                main_chunks[0]
            } else {
                main_chunks[2]
            };
            components::graph::render(f, state, target_chunk);
        }
    }

    // Help bar
    components::help::render(f, state, chunks[2]);

    // Diff Modal Overlay (if open)
    components::diff_modal::render(f, state, f.area());

    // Confirm Modal Overlay (if a pending action awaits confirmation)
    components::confirm_modal::render(f, state, f.area());
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
