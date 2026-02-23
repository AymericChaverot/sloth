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

pub use state::{AppState, ScannerEvent};

pub fn run_tui(
    mut state: AppState,
    rx: Receiver<ScannerEvent>,
) -> io::Result<Option<(PathBuf, Vec<String>, Vec<usize>)>> {
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
                        graph_lines: None,
                        analyzed: false,
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
                ScannerEvent::AnalysisComplete => state.is_analyzing = false,
                ScannerEvent::UpdateAvailable(version) => {
                    state.update_available = Some(version);
                }
            }
        }

        let mut needs_fetch = false;
        let mut fetch_path = PathBuf::new();

        if state.show_graph || state.graph_maximized {
            if let Some(repo) = state.repositories.get(state.repo_index) {
                if repo.graph_lines.is_none() {
                    needs_fetch = true;
                    fetch_path = repo.path.clone();
                }
            }
        }

        if needs_fetch {
            if let Ok(lines) = crate::git::get_git_graph(&fetch_path) {
                if let Some(repo) = state.repositories.get_mut(state.repo_index) {
                    repo.graph_lines = Some(lines);
                }
            } else {
                if let Some(repo) = state.repositories.get_mut(state.repo_index) {
                    repo.graph_lines = Some(vec![]);
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

            // Help bar
            components::help::render(f, &mut state, chunks[2]);
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

    if state.should_execute {
        let path = state.repositories[state.repo_index].path.clone();
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
        Ok(Some((path, branches, stashes)))
    } else {
        Ok(None)
    }
}
