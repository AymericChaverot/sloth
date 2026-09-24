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
    /// Delete merged, gone or stale branches across repositories, without the TUI
    ///
    /// Without criteria, deletes merged and gone branches. Protected branches
    /// (default, checked out, `protected_branches`) are never deleted.
    Clean(cli::CleanArgs),
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
    let root = config.scan_root(args.path.as_deref());
    if !root.is_dir() {
        anyhow::bail!("{} is not a directory", root.display());
    }

    if let Some(Command::Clean(clean_args)) = args.command {
        if let Some(warning) = config_warning {
            eprintln!("⚠ {warning} — using defaults.");
        }
        return cli::clean(root, config, clean_args).await;
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let worker = worker::Worker::new(tx.clone(), config.deep_clean_keep.clone());
    worker.scan(root.clone(), config.scan_exclude.clone());

    // Spawn background update check (non-blocking, best-effort)
    let tx_update = tx.clone();
    tokio::task::spawn_blocking(move || {
        if let Some(version) = updater::check_for_update() {
            let _ = tx_update.send(ui::ScannerEvent::UpdateAvailable(version));
        }
    });

    let mut state = AppState::new(config, root);
    state.journal = journal::Journal::open_default();
    if let Some(warning) = config_warning {
        state.warn(format!("{warning} — using defaults"));
    }
    ui::run_tui(state, rx, tx, worker)?;
    Ok(())
}
