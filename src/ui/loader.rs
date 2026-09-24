//! On-demand git queries run off the UI thread; results come back as events.

use crate::sys::RealSystem;
use crate::ui::ScannerEvent;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// What a diff is requested for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffTarget {
    /// Changes of the branch since it forked from `base`.
    Branch {
        name: String,
        base: Option<String>,
    },
    Stash {
        sha: String,
    },
}

pub fn load_graph(tx: &Sender<ScannerEvent>, repo: PathBuf) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        let lines = crate::git::get_git_graph(&repo, &RealSystem);
        let _ = tx.send(ScannerEvent::GraphLoaded { path: repo, lines });
    });
}

/// `id` lets the UI ignore a diff that arrives after the user moved on.
pub fn load_diff(tx: &Sender<ScannerEvent>, id: u64, repo: PathBuf, target: DiffTarget) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        let lines = match target {
            DiffTarget::Branch { name, base } => {
                let range = match base {
                    Some(base) => format!("{base}...{name}"),
                    None => name,
                };
                crate::git::commands::get_branch_diff(&repo, &range, &RealSystem)
            }
            DiffTarget::Stash { sha } => {
                crate::git::commands::get_stash_diff(&repo, &sha, &RealSystem)
            }
        };
        let _ = tx.send(ScannerEvent::DiffLoaded { id, lines });
    });
}

/// Lists what a deep clean would remove in each repository.
pub fn load_deep_clean_preview(tx: &Sender<ScannerEvent>, repos: Vec<PathBuf>, keep: Vec<String>) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        let mut preview: Vec<String> = Vec::new();
        for path in &repos {
            let label = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match crate::engine::deep_clean_candidates(path, &keep, &RealSystem) {
                Ok(out) => {
                    for entry in out {
                        preview.push(format!("[{}] {}", label, entry));
                    }
                }
                Err(e) => preview.push(format!("[{}] error: {}", label, e.to_string().trim())),
            }
        }
        if preview.is_empty() {
            preview.push("(nothing to clean)".to_string());
        }
        let _ = tx.send(ScannerEvent::DeepCleanPreview(preview));
    });
}
