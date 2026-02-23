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

        let mut size_tasks_args = Vec::new();

        for task in tasks {
            if let Ok(Ok(status)) = task.await {
                size_tasks_args.push((
                    status.path.clone(),
                    status
                        .worktrees
                        .iter()
                        .map(|w| w.path.clone())
                        .collect::<Vec<_>>(),
                ));
                let _ = tx.send(ui::ScannerEvent::RepoAnalyzed(status));
            }
        }
        let _ = tx.send(ui::ScannerEvent::AnalysisComplete);

        // Spawn background size calculator (sequential to avoid I/O thrashing)
        tokio::task::spawn_blocking(move || {
            for (path, wt_paths) in size_tasks_args {
                let sizes = git::compute_repo_sizes(&path, wt_paths);
                let _ = tx.send(ui::ScannerEvent::SizeComputed {
                    path,
                    size_bytes: sizes.0,
                    untracked_size_bytes: sizes.1,
                    worktree_sizes: sizes.2,
                });
            }
        });
    });

    // Spawn background update check (non-blocking, best-effort)
    tokio::task::spawn_blocking(move || {
        if let Some(version) = updater::check_for_update() {
            let _ = tx_update.send(ui::ScannerEvent::UpdateAvailable(version));
        }
    });

    let state = AppState::new();

    // Start TUI
    if let Some((paths, action, branches, stashes, worktrees)) = ui::run_tui(state, rx)? {
        let mut size_before = 0;
        for path in &paths {
            if let Ok(s) = crate::git::stats::get_repo_size(path) {
                size_before += s;
            }
        }
        for wt in &worktrees {
            if let Ok(s) = crate::git::stats::get_repo_size(std::path::Path::new(wt)) {
                size_before += s;
            }
        }

        let engine_action = match action {
            ui::UiAction::CleanRepo => {
                if branches.is_empty() && stashes.is_empty() && worktrees.is_empty() {
                    println!("No branches, stashes, or worktrees selected for deletion.");
                    return Ok(());
                }
                engine::Action::CleanRepo {
                    branches,
                    stashes,
                    worktrees: worktrees.clone(),
                }
            }
            ui::UiAction::PruneRemotes => engine::Action::PruneRemotes,
            ui::UiAction::GarbageCollect => engine::Action::GarbageCollect,
            ui::UiAction::DeepClean => engine::Action::DeepClean,
        };

        if paths.len() == 1 {
            println!(
                "\nExecuting {} on {} (Dry-run false)...",
                match engine_action {
                    engine::Action::CleanRepo { .. } => "CleanRepo",
                    engine::Action::PruneRemotes => "PruneRemotes",
                    engine::Action::GarbageCollect => "GarbageCollect",
                    engine::Action::DeepClean => "DeepClean",
                },
                paths[0].display()
            );
        } else {
            println!(
                "\nExecuting {} on {} repositories (Dry-run false)...",
                match engine_action {
                    engine::Action::CleanRepo { .. } => "CleanRepo",
                    engine::Action::PruneRemotes => "PruneRemotes",
                    engine::Action::GarbageCollect => "GarbageCollect",
                    engine::Action::DeepClean => "DeepClean",
                },
                paths.len()
            );
        }

        let results = engine::execute_batch(
            paths.clone(),
            engine_action,
            false, // REAL EXECUTION!
        )
        .await;

        for res in results {
            let symbol = if res.success { "✅" } else { "❌" };
            println!("{} {}: {}", symbol, res.repo_path.display(), res.message);
        }

        let mut size_after = 0;
        for path in &paths {
            if let Ok(s) = crate::git::stats::get_repo_size(path) {
                size_after += s;
            }
        }
        for wt in &worktrees {
            if let Ok(s) = crate::git::stats::get_repo_size(std::path::Path::new(wt)) {
                size_after += s;
            }
        }

        let recovered = size_before.saturating_sub(size_after);
        if recovered > 0 {
            println!(
                "\nDisk space recovered: {}",
                crate::git::stats::format_size(recovered)
            );
        }
    } else {
        println!("No action executed.");
    }

    Ok(())
}
