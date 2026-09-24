use ignore::{WalkBuilder, WalkState};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Error, Debug)]
pub enum ScannerError {
    #[error("Failed to read directory")]
    IoError(#[from] std::io::Error),
    #[error("Directory traversal error: {0}")]
    WalkError(#[from] ignore::Error),
}

/// Discovers Git repositories using a parallel filesystem walker.
/// Returns a receiver channel that emits paths to discovered repository roots.
/// Directories named in `exclude` are skipped entirely.
pub fn scan_for_repositories<P: AsRef<Path>>(
    root: P,
    exclude: Vec<String>,
) -> mpsc::Receiver<Result<PathBuf, ScannerError>> {
    let (tx, rx) = mpsc::channel(256);
    let root_path = root.as_ref().to_path_buf();

    tokio::task::spawn_blocking(move || {
        WalkBuilder::new(&root_path)
            .hidden(false)
            .ignore(true)
            .git_ignore(true)
            .filter_entry(move |entry| {
                !(entry.depth() > 0
                    && entry.file_type().is_some_and(|ft| ft.is_dir())
                    && exclude
                        .iter()
                        .any(|name| entry.file_name() == name.as_str()))
            })
            .build_parallel()
            .run(|| {
                let tx = tx.clone();
                Box::new(move |result| match result {
                    Ok(entry) => {
                        if entry.file_type().is_some_and(|ft| ft.is_dir())
                            && entry.file_name() == ".git"
                        {
                            if let Some(parent) = entry.path().parent() {
                                let _ = tx.blocking_send(Ok(parent.to_path_buf()));
                            }
                            WalkState::Skip
                        } else {
                            WalkState::Continue
                        }
                    }
                    Err(err) => {
                        let _ = tx.blocking_send(Err(err.into()));
                        WalkState::Continue
                    }
                })
            });
    });

    rx
}
