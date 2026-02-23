use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Execution failed: {0}")]
    #[allow(dead_code)]
    ExecutionError(String),
}

#[derive(Debug, Clone)]
pub enum Action {
    CleanRepo {
        branches: Vec<String>,
        stashes: Vec<usize>,
        worktrees: Vec<String>,
    },
    PruneRemotes,
    GarbageCollect,
    DeepClean,
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub repo_path: PathBuf,
    pub success: bool,
    pub message: String,
}

/// Executes an action across multiple repositories concurrently.
/// Respects the dry-run flag to avoid mutating data.
pub async fn execute_batch(
    repositories: Vec<PathBuf>,
    action: Action,
    dry_run: bool,
) -> Vec<ExecutionResult> {
    let mut tasks = Vec::new();

    for path in repositories {
        let action = action.clone();
        tasks.push(tokio::spawn(async move {
            if dry_run {
                ExecutionResult {
                    repo_path: path.clone(),
                    success: true,
                    message: format!(
                        "Dry-run: Would execute {:?} on {:?}",
                        format_action(&action),
                        path.display()
                    ),
                }
            } else {
                match execute_action(&path, &action).await {
                    Ok(msg) => ExecutionResult {
                        repo_path: path,
                        success: true,
                        message: msg,
                    },
                    Err(e) => ExecutionResult {
                        repo_path: path,
                        success: false,
                        message: format!("Error: {}", e),
                    },
                }
            }
        }));
    }

    let mut results = Vec::new();
    for task in tasks {
        if let Ok(res) = task.await {
            results.push(res);
        }
    }
    results
}

fn format_action(action: &Action) -> String {
    match action {
        Action::CleanRepo {
            branches,
            stashes,
            worktrees,
        } => {
            format!(
                "Delete {} branches, drop {} stashes, remove {} worktrees",
                branches.len(),
                stashes.len(),
                worktrees.len()
            )
        }
        Action::PruneRemotes => "Prune dead remote tracking branches".to_string(),
        Action::GarbageCollect => "Garbage collect local repository".to_string(),
        Action::DeepClean => "Deep clean untracked/ignored directories".to_string(),
    }
}

async fn execute_action(path: &Path, action: &Action) -> Result<String, EngineError> {
    match action {
        Action::CleanRepo {
            branches,
            stashes,
            worktrees,
        } => {
            let mut msgs = Vec::new();

            for b in branches {
                let out = Command::new("git")
                    .args(["branch", "-D", b])
                    .current_dir(path)
                    .output()
                    .await;
                if let Ok(o) = out {
                    if o.status.success() {
                        msgs.push(format!("Deleted branch {}", b));
                    } else {
                        msgs.push(format!(
                            "Failed to delete branch {}: {}",
                            b,
                            String::from_utf8_lossy(&o.stderr)
                        ));
                    }
                }
            }

            // Drop stashes in reverse order so indices don't shift!
            let mut sorted_stashes = stashes.clone();
            sorted_stashes.sort_unstable_by(|a, b| b.cmp(a));

            for s in sorted_stashes {
                let out = Command::new("git")
                    .args(["stash", "drop", &format!("stash@{{{}}}", s)])
                    .current_dir(path)
                    .output()
                    .await;
                if let Ok(o) = out {
                    if o.status.success() {
                        msgs.push(format!("Dropped stash {}", s));
                    } else {
                        msgs.push(format!(
                            "Failed to drop stash {}: {}",
                            s,
                            String::from_utf8_lossy(&o.stderr)
                        ));
                    }
                }
            }

            for w in worktrees {
                let out = Command::new("git")
                    .args(["worktree", "remove", "--force", &w])
                    .current_dir(path)
                    .output()
                    .await;
                if let Ok(o) = out {
                    if o.status.success() {
                        msgs.push(format!("Removed worktree {}", w));
                    } else {
                        msgs.push(format!(
                            "Failed to remove worktree {}: {}",
                            w,
                            String::from_utf8_lossy(&o.stderr)
                        ));
                    }
                }
            }

            if msgs.is_empty() {
                msgs.push("No cleaning performed.".to_string());
            }

            Ok(msgs.join(", "))
        }
        Action::PruneRemotes => {
            let remote_check = Command::new("git")
                .args(["remote"])
                .current_dir(path)
                .output()
                .await;

            let has_origin = if let Ok(o) = remote_check {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .any(|l| l.trim() == "origin")
            } else {
                false
            };

            if !has_origin {
                return Ok("No 'origin' remote found. Skipped.".to_string());
            }

            let out = Command::new("git")
                .args(["remote", "prune", "origin"])
                .current_dir(path)
                .output()
                .await;
            if let Ok(o) = out {
                if o.status.success() {
                    let mut stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    if stdout.trim().is_empty() {
                        stdout = "No dead branches to prune.".to_string();
                    }
                    Ok(stdout.trim().to_string())
                } else {
                    Ok(format!(
                        "Failed to prune remotes: {}",
                        String::from_utf8_lossy(&o.stderr)
                    ))
                }
            } else {
                Err(EngineError::ExecutionError(
                    "Command failed to start".to_string(),
                ))
            }
        }
        Action::GarbageCollect => {
            let out = Command::new("git")
                .args(["gc"])
                .current_dir(path)
                .output()
                .await;
            if let Ok(o) = out {
                if o.status.success() {
                    let mut stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    if stdout.trim().is_empty() {
                        stdout = "Garbage collection completed.".to_string();
                    }
                    Ok(stdout.trim().to_string())
                } else {
                    Ok(format!(
                        "Failed to garbage collect: {}",
                        String::from_utf8_lossy(&o.stderr)
                    ))
                }
            } else {
                Err(EngineError::ExecutionError(
                    "Command failed to start".to_string(),
                ))
            }
        }
        Action::DeepClean => {
            let out = Command::new("git")
                .args(["clean", "-xdff", "--exclude=.git"])
                .current_dir(path)
                .output()
                .await;
            if let Ok(o) = out {
                if o.status.success() {
                    Ok("Deep clean successful. Untracked and ignored files purged.".to_string())
                } else {
                    Ok(format!(
                        "Failed to deep clean: {}",
                        String::from_utf8_lossy(&o.stderr)
                    ))
                }
            } else {
                Err(EngineError::ExecutionError(
                    "Command failed to start".to_string(),
                ))
            }
        }
    }
}
