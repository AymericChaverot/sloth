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
mod worker;

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
    let worker = worker::Worker::new(tx.clone(), config.deep_clean_keep.clone());
    worker.scan(
        config.scan_root(args.path.as_deref()),
        config.scan_exclude.clone(),
    );

    // Spawn background update check (non-blocking, best-effort)
    let tx_update = tx.clone();
    tokio::task::spawn_blocking(move || {
        if let Some(version) = updater::check_for_update() {
            let _ = tx_update.send(ui::ScannerEvent::UpdateAvailable(version));
        }
    });

    let state = AppState::new(config);

    match ui::run_tui(state, rx, tx)? {
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
