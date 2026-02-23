use clap::Parser;

mod engine;
mod git;
mod scanner;
mod ui;
mod updater;

use crate::ui::AppState;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The root directory to scan for Git repositories
    #[arg(short, long, default_value = ".")]
    path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let (tx, rx) = std::sync::mpsc::channel();
    let path_clone = args.path.clone();

    // Clone sender for the update checker before moving into scanner task
    let tx_update = tx.clone();

    tokio::spawn(async move {
        let mut rx_scan = scanner::scan_for_repositories(&path_clone);
        let mut git_repos = Vec::new();

        // Collect paths
        while let Some(res) = rx_scan.recv().await {
            if let Ok(path) = res {
                git_repos.push(path.clone());
                let _ = tx.send(ui::ScannerEvent::RepoFound(path));
            }
        }
        let _ = tx.send(ui::ScannerEvent::ScanComplete);

        // Analyze concurrently
        let mut tasks = Vec::new();
        for path in git_repos {
            tasks.push(tokio::task::spawn_blocking(move || {
                git::analyze_repository(&path)
            }));
        }

        for task in tasks {
            if let Ok(Ok(status)) = task.await {
                let _ = tx.send(ui::ScannerEvent::RepoAnalyzed(status));
            }
        }
        let _ = tx.send(ui::ScannerEvent::AnalysisComplete);
    });

    // Spawn background update check (non-blocking, best-effort)
    tokio::task::spawn_blocking(move || {
        if let Some(version) = updater::check_for_update() {
            let _ = tx_update.send(ui::ScannerEvent::UpdateAvailable(version));
        }
    });

    let state = AppState::new();

    // Start TUI
    if let Some((path, action, branches, stashes)) = ui::run_tui(state, rx)? {
        let engine_action = match action {
            ui::UiAction::CleanRepo => {
                if branches.is_empty() && stashes.is_empty() {
                    println!("No branches or stashes selected for deletion.");
                    return Ok(());
                }
                engine::Action::CleanRepo { branches, stashes }
            }
            ui::UiAction::PruneRemotes => engine::Action::PruneRemotes,
        };

        println!(
            "\nExecuting {} on {} (Dry-run false)...",
            match engine_action {
                engine::Action::CleanRepo { .. } => "CleanRepo",
                engine::Action::PruneRemotes => "PruneRemotes",
            },
            path.display()
        );

        let results = engine::execute_batch(
            vec![path],
            engine_action,
            false, // REAL EXECUTION!
        )
        .await;

        for res in results {
            let symbol = if res.success { "✅" } else { "❌" };
            println!("{} {}: {}", symbol, res.repo_path.display(), res.message);
        }
    } else {
        println!("No action executed.");
    }

    Ok(())
}
