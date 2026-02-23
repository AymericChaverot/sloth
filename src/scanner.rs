use ignore::WalkBuilder;
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

/// Discovers Git repositories asynchronously.
/// Returns a receiver channel that emits paths to discovered .git directories.
pub fn scan_for_repositories<P: AsRef<Path>>(
    root: P,
) -> mpsc::Receiver<Result<PathBuf, ScannerError>> {
    let (tx, rx) = mpsc::channel(100);
    let root_path = root.as_ref().to_path_buf();

    tokio::task::spawn_blocking(move || {
        let walker = WalkBuilder::new(root_path)
            .hidden(false) // Don't ignore hidden folders since we need .git
            .ignore(true)
            .git_ignore(true)
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    // Check if it's a .git directory
                    if entry.file_type().is_some_and(|ft| ft.is_dir())
                        && entry.file_name() == ".git"
                    {
                        // Send the parent directory (the actual repository root)
                        if let Some(parent) = entry.path().parent() {
                            let _ = tx.blocking_send(Ok(parent.to_path_buf()));
                        }
                    }
                }
                Err(err) => {
                    let _ = tx.blocking_send(Err(err.into()));
                }
            }
        }
    });

    rx
}
