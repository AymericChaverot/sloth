use clap::{Parser, Subcommand};

mod cleanup;
mod cli;
mod config;
mod engine;
mod git;
mod journal;
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
    /// [default: `default_path` from the config file, or the current directory]
    #[arg(short, long, global = true)]
    path: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Restore branches and stashes deleted by sloth (lists them without arguments)
    Restore {
        /// Journal entry ids to restore
        ids: Vec<usize>,
        /// Restore everything deleted by the last cleanup run
        #[arg(long, conflicts_with = "ids")]
        last: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(Command::Restore { ids, last }) = &args.command {
        return cli::restore(ids, *last);
    }

    let (config, config_warning) = config::Config::load();
    if let Some(warning) = config_warning {
        eprintln!("⚠ {warning} — using defaults.");
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let path_clone = config.scan_root(args.path.as_deref());
    let scan_exclude = config.scan_exclude.clone();

    let tx_scanner = tx.clone();
    let tx_update = tx.clone();
    let tx_ui = tx.clone();

    tokio::spawn(async move {
        let tx = tx_scanner;
        let mut rx_scan = scanner::scan_for_repositories(&path_clone, scan_exclude);
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

    let state = AppState::new(config);

    match ui::run_tui(state, rx, tx_ui)? {
        Some(plans) if !plans.is_empty() => {
            let count: usize = plans.iter().map(|p| p.operations.len()).sum();
            println!(
                "\nRunning {} operation(s) on {} repositorie(s)...",
                count,
                plans.len()
            );
            let observer =
                journal::Journal::open_default().map(|j| j.observer(git::stats::now_ts()));
            let results = engine::execute(plans, false, sys::RealSystem, observer).await;
            print_results(&results);
            if results
                .iter()
                .any(|r| journal::Entry::from_result(0, r).is_some())
            {
                println!("Deleted branches and stashes can be restored with `sloth restore`.");
            }
        }
        Some(_) => println!("Nothing selected."),
        None => println!("No action executed."),
    }

    Ok(())
}

fn print_results(results: &[engine::OpResult]) {
    let mut current: Option<&std::path::Path> = None;
    for res in results {
        if current != Some(res.repo.as_path()) {
            println!("\n{}", res.repo.display());
            current = Some(res.repo.as_path());
        }
        match &res.outcome {
            Ok(msg) => println!("  ✅ {}: {}", res.operation.describe(), msg),
            Err(err) => println!("  ❌ {}: {}", res.operation.describe(), err),
        }
    }
    let freed: u64 = results.iter().map(|r| r.freed_bytes).sum();
    let failed = results.iter().filter(|r| !r.is_ok()).count();
    println!(
        "\n{} succeeded, {} failed, {} freed.",
        results.len() - failed,
        failed,
        git::stats::format_size(freed)
    );
}
