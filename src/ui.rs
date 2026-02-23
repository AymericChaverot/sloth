use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

pub mod components;
pub mod events;
pub mod state;

pub use state::{AppState, Focus, ScannerEvent};

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
                    // Find and replace the placeholder entry
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
                .constraints([
                    Constraint::Length(7), // ASCII Art + Version
                    Constraint::Min(10),   // Main Content
                    Constraint::Length(3), // Help Footer
                ].as_ref())
                .split(f.area());

            let version = env!("CARGO_PKG_VERSION");
            let art_lines = [
                "  ▄▄▄▄▄  ▄▄                ",
                " ██▀▀▀▀█▄ ██       █▄ █▄   ",
                " ▀██▄  ▄▀ ██      ▄██▄██   ",
                "   ▀██▄▄  ██ ▄███▄ ██ ████▄",
                " ▄   ▀██▄ ██ ██ ██ ██ ██ ██",
                " ▀██████▀▄██▄▀███▀▄██▄██ ██",
            ];
            let max_diag = (art_lines.len() + 28) as f64; // row + max col

            // HSL to RGB conversion (s=0.8, l=0.55 for balanced rainbow)
            let hsl_to_rgb = |h: f64| -> (u8, u8, u8) {
                let s = 0.8_f64;
                let l = 0.55_f64;
                let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
                let h2 = h / 60.0;
                let x = c * (1.0 - ((h2 % 2.0) - 1.0).abs());
                let m = l - c / 2.0;
                let (r1, g1, b1) = match h2 as u32 {
                    0 => (c, x, 0.0),
                    1 => (x, c, 0.0),
                    2 => (0.0, c, x),
                    3 => (0.0, x, c),
                    4 => (x, 0.0, c),
                    _ => (c, 0.0, x),
                };
                (((r1 + m) * 255.0) as u8, ((g1 + m) * 255.0) as u8, ((b1 + m) * 255.0) as u8)
            };

            let mut header_lines: Vec<Line> = Vec::new();
            header_lines.push(Line::from("")); // top padding
            for (row, art) in art_lines.iter().enumerate() {
                let mut spans: Vec<Span> = Vec::new();
                for (col, ch) in art.chars().enumerate() {
                    let diag = (row + col) as f64;
                    let hue = (diag / max_diag) * 300.0; // 0° (red) → 300° (magenta)
                    let (r, g, b) = hsl_to_rgb(hue);
                    spans.push(Span::styled(
                        ch.to_string(),
                        Style::default().fg(Color::Rgb(r, g, b)),
                    ));
                }
                if row == art_lines.len() - 1 {
                    spans.push(Span::styled(
                        format!(" v{} - The git repository cleaner tool", version),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                header_lines.push(Line::from(spans));
            }
            let header = Paragraph::new(header_lines)
                .alignment(ratatui::layout::Alignment::Left);
            f.render_widget(header, chunks[0]);

            let main_chunks = if state.graph_maximized {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(chunks[1])
            } else if state.show_graph {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(30), Constraint::Percentage(40)].as_ref())
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
                let target_chunk = if state.graph_maximized { main_chunks[0] } else { main_chunks[2] };
                components::graph::render(f, &mut state, target_chunk);
            }

            let mut help_text = match state.focus {
                Focus::Repositories => "Left Pane: Up/Down navigate. Right enter details. 'g' toggle graph. 'q' quit.".to_string(),
                Focus::Details => "Middle Pane: Up/Down navigate. 'Space' select item. 'Enter' execute. Right graph. Left repos.".to_string(),
                Focus::GitGraph => "Right Pane: Arrows to scroll. 'f' fullscreen. 'Esc' or 'g' back. 'q' quit.".to_string(),
            };
            if let Some(ref ver) = state.update_available {
                help_text = format!("🔄 Update available: {} — press 'u' to update  |  {}", ver, help_text);
            }
            let details_footer = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title("Help"));
            f.render_widget(details_footer, chunks[2]);
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
