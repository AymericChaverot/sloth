use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
};
use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

pub mod components;
pub mod events;
pub mod state;
pub mod theme;

pub use state::{AppState, ScannerEvent, UiAction};

pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

type TuiResult = io::Result<Option<(Vec<PathBuf>, UiAction, Vec<String>, Vec<usize>, Vec<String>)>>;

pub fn run_tui(mut state: AppState, rx: Receiver<ScannerEvent>) -> TuiResult {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    loop {
        state.loader_tick = state.loader_tick.wrapping_add(1);

        // Pump background tasks
        while let Ok(event) = rx.try_recv() {
            match event {
                ScannerEvent::RepoFound(path) => {
                    state.scanned_count += 1;
                    state.repositories.push(crate::git::RepoStatus {
                        path,
                        remote_url: None,
                        branches: Vec::new(),
                        stashes: Vec::new(),
                        worktrees: Vec::new(),
                        graph_lines: None,
                        analyzed: false,
                        size_bytes: None,
                        untracked_size_bytes: None,
                        size_finalized: false,
                    });
                }
                ScannerEvent::ScanComplete => state.is_scanning = false,
                ScannerEvent::RepoAnalyzed(repo) => {
                    if let Some(existing) =
                        state.repositories.iter_mut().find(|r| r.path == repo.path)
                    {
                        *existing = repo;
                    } else {
                        state.repositories.push(repo);
                    }
                    state.analyzed_count += 1;
                }
                ScannerEvent::SizePartial { path, size_bytes } => {
                    if let Some(repo) = state.repositories.iter_mut().find(|r| r.path == path) {
                        repo.size_bytes = Some(size_bytes);
                    }
                }
                ScannerEvent::SizeComputed {
                    path,
                    size_bytes,
                    untracked_size_bytes,
                    worktree_sizes,
                } => {
                    if let Some(repo) = state.repositories.iter_mut().find(|r| r.path == path) {
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
            }
        }

        let mut needs_fetch = false;
        let mut fetch_path = PathBuf::new();

        if (state.show_graph || state.graph_maximized)
            && let Some(repo) = state.repositories.get(state.repo_index)
            && repo.graph_lines.is_none()
        {
            needs_fetch = true;
            fetch_path = repo.path.clone();
        }

        if needs_fetch {
            if let Ok(lines) = crate::git::get_git_graph(&fetch_path, &crate::sys::RealSystem) {
                if let Some(repo) = state.repositories.get_mut(state.repo_index) {
                    repo.graph_lines = Some(lines);
                }
            } else if let Some(repo) = state.repositories.get_mut(state.repo_index) {
                repo.graph_lines = Some(vec![]);
            }
        }

        // Fetch deep clean preview when pending
        if matches!(state.pending_action, Some(UiAction::DeepClean))
            && state.confirm_preview_lines.is_none()
        {
            use crate::sys::GitExecutor as _;
            let sys = crate::sys::RealSystem;
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
            let mut preview: Vec<String> = Vec::new();
            for path in &paths {
                let label = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                match sys.run_git_command(path, &["clean", "-nxdff", "--exclude=.git"]) {
                    Ok(out) => {
                        for line in out.lines() {
                            preview.push(format!("[{}] {}", label, line));
                        }
                    }
                    Err(e) => preview.push(format!("[{}] error: {}", label, e)),
                }
            }
            if preview.is_empty() {
                preview.push("(nothing to clean)".to_string());
            }
            state.confirm_preview_lines = Some(preview);
        }

        // Fetch diff if modal is opened and lines are empty
        if state.diff_modal_open
            && state.diff_lines.is_none()
            && let Some(repo) = state.repositories.get(state.repo_index)
        {
            let b_len = repo.branches.len();
            if state.detail_index < b_len {
                let b = &repo.branches[state.detail_index];
                let target = if !b.is_dead && b.upstream.is_some() {
                    b.upstream.clone().unwrap()
                } else if repo.branches.iter().any(|b| b.name == "main") {
                    "main".to_string()
                } else {
                    "master".to_string()
                };
                let diff_target = format!("{}...{}", target, b.name);
                if let Ok(diff) = crate::git::commands::get_branch_diff(
                    &repo.path,
                    &diff_target,
                    &crate::sys::RealSystem,
                ) {
                    state.diff_lines = Some(diff);
                }
            } else {
                let s_idx = state.detail_index.saturating_sub(b_len);
                if let Some(stash) = repo.stashes.get(s_idx)
                    && let Ok(diff) = crate::git::commands::get_stash_diff(
                        &repo.path,
                        stash.index,
                        &crate::sys::RealSystem,
                    )
                {
                    state.diff_lines = Some(diff);
                }
            }
        }

        terminal.draw(|f| {
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
            components::header::render(f, &mut state, chunks[0]);

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
                components::dashboard::render(f, &mut state, chunks[1]);
            } else {
                if !state.graph_maximized {
                    components::repositories::render(f, &mut state, main_chunks[0]);
                    components::details::render(f, &mut state, main_chunks[1]);
                }

                if state.show_graph || state.graph_maximized {
                    let target_chunk = if state.graph_maximized {
                        main_chunks[0]
                    } else {
                        main_chunks[2]
                    };
                    components::graph::render(f, &mut state, target_chunk);
                }
            }

            // Help bar
            components::help::render(f, &mut state, chunks[2]);

            // Diff Modal Overlay (if open)
            components::diff_modal::render(f, &mut state, f.area());

            // Confirm Modal Overlay (if a pending action awaits confirmation)
            components::confirm_modal::render(f, &state, f.area());
        })?;

        events::handle_events(&mut state)?;

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

    if let Some(action) = state.action {
        let mut paths = Vec::new();

        // If 'CleanRepo', it only applies to the currently focused repository (not bulk)
        if matches!(action, UiAction::CleanRepo) || state.selected_repositories.is_empty() {
            paths.push(state.repositories[state.repo_index].path.clone());
        } else {
            // Bulk action triggered
            for idx in &state.selected_repositories {
                if let Some(repo) = state.repositories.get(*idx) {
                    paths.push(repo.path.clone());
                }
            }
        }

        let branches = state
            .selected_branches
            .remove(&state.repo_index)
            .unwrap_or_default()
            .into_iter()
            .collect();
        let stashes = state
            .selected_stashes
            .remove(&state.repo_index)
            .unwrap_or_default()
            .into_iter()
            .collect();
        let worktrees = state
            .selected_worktrees
            .remove(&state.repo_index)
            .unwrap_or_default()
            .into_iter()
            .collect();
        Ok(Some((paths, action, branches, stashes, worktrees)))
    } else {
        Ok(None)
    }
}
