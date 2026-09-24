use clap::Parser;

mod engine;
mod git;
mod scanner;
mod sys;
mod ui;
mod updater;

#[cfg(test)]
mod test_support;

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

    let tx_scanner = tx.clone();
    let tx_update = tx.clone();
    let tx_ui = tx.clone();

    tokio::spawn(async move {
        let tx = tx_scanner;
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
                let sys = crate::sys::RealSystem;
                git::analyze_repository(&path, &sys)
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
                        .filter(|w| !w.is_main)
                        .map(|w| w.path.clone())
                        .collect::<Vec<_>>(),
                ));
                let _ = tx.send(ui::ScannerEvent::RepoAnalyzed(status));
            }
        }
        let _ = tx.send(ui::ScannerEvent::AnalysisComplete);

        // Spawn background size calculator (sequential to avoid I/O thrashing).
        // Sends SizePartial updates as each file/dir is discovered so the UI
        // can show a live growing estimate rather than a blank then a jump.
        tokio::task::spawn_blocking(move || {
            use crate::sys::{FileSystem as _, GitExecutor as _};
            let sys = crate::sys::RealSystem;

            for (path, wt_paths) in size_tasks_args {
                // 1. .git directory size — send immediately so something appears
                let git_size = sys.get_size(&path.join(".git")).ok();
                let _ = tx.send(ui::ScannerEvent::SizePartial {
                    path: path.clone(),
                    size_bytes: git_size,
                    untracked_size_bytes: 0,
                });

                // 2. Walk untracked/ignored files, accumulating and streaming updates
                let mut untracked_size = 0u64;
                let mut has_untracked = false;
                if let Ok(out_str) = sys.run_git_command(&path, &["clean", "-ndx"]) {
                    has_untracked = true;
                    for line in out_str.lines() {
                        if line.starts_with("Would remove ") {
                            let to_remove = line.trim_start_matches("Would remove ");
                            let full_path = path.join(to_remove);
                            if sys.exists(&full_path) {
                                let file_size = sys.get_size(&full_path).unwrap_or(0);
                                untracked_size += file_size;
                                let _ = tx.send(ui::ScannerEvent::SizePartial {
                                    path: path.clone(),
                                    size_bytes: git_size,
                                    untracked_size_bytes: untracked_size,
                                });
                            }
                        }
                    }
                }

                // 3. Worktree sizes
                let mut worktree_sizes = std::collections::HashMap::new();
                for wt in &wt_paths {
                    let s = sys.get_size(std::path::Path::new(wt)).ok();
                    worktree_sizes.insert(wt.clone(), s);
                }

                // 4. Final definitive event
                let _ = tx.send(ui::ScannerEvent::SizeComputed {
                    path,
                    size_bytes: git_size,
                    untracked_size_bytes: if has_untracked {
                        Some(untracked_size)
                    } else {
                        None
                    },
                    worktree_sizes,
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

    if let Some((paths, action, branches, stashes, worktrees)) = ui::run_tui(state, rx, tx_ui)? {
        let sys = crate::sys::RealSystem;
        let mut size_before = 0;
        for path in &paths {
            if let Ok(s) = crate::git::stats::get_repo_size(path, &sys) {
                size_before += s;
            }
        }
        for wt in &worktrees {
            if let Ok(s) = crate::git::stats::get_repo_size(std::path::Path::new(wt), &sys) {
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
            sys.clone(),
        )
        .await;

        for res in results {
            let symbol = if res.success { "✅" } else { "❌" };
            println!("{} {}: {}", symbol, res.repo_path.display(), res.message);
        }

        let mut size_after = 0;
        for path in &paths {
            if let Ok(s) = crate::git::stats::get_repo_size(path, &sys) {
                size_after += s;
            }
        }
        for wt in &worktrees {
            if let Ok(s) = crate::git::stats::get_repo_size(std::path::Path::new(wt), &sys) {
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
