use std::path::{Path, PathBuf};
use thiserror::Error;

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
pub async fn execute_batch<T: crate::sys::GitExecutor + Clone + Send + Sync + 'static>(
    repositories: Vec<PathBuf>,
    action: Action,
    dry_run: bool,
    sys: T,
) -> Vec<ExecutionResult> {
    let mut tasks = Vec::new();

    for path in repositories {
        let action = action.clone();
        let sys_clone = sys.clone();
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
                match execute_action(&path, &action, &sys_clone).await {
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

async fn execute_action(
    path: &Path,
    action: &Action,
    sys: &impl crate::sys::GitExecutor,
) -> Result<String, EngineError> {
    match action {
        Action::CleanRepo {
            branches,
            stashes,
            worktrees,
        } => {
            let mut msgs = Vec::new();

            for b in branches {
                match sys.run_git_command_async(path, &["branch", "-D", b]).await {
                    Ok(_) => msgs.push(format!("Deleted branch {}", b)),
                    Err(e) => msgs.push(format!("Failed to delete branch {}: {}", b, e)),
                }
            }

            // Drop stashes in reverse order so indices don't shift!
            let mut sorted_stashes = stashes.clone();
            sorted_stashes.sort_unstable_by(|a, b| b.cmp(a));

            for s in sorted_stashes {
                match sys
                    .run_git_command_async(path, &["stash", "drop", &format!("stash@{{{}}}", s)])
                    .await
                {
                    Ok(_) => msgs.push(format!("Dropped stash {}", s)),
                    Err(e) => msgs.push(format!("Failed to drop stash {}: {}", s, e)),
                }
            }

            for w in worktrees {
                match sys
                    .run_git_command_async(path, &["worktree", "remove", "--force", w])
                    .await
                {
                    Ok(_) => msgs.push(format!("Removed worktree {}", w)),
                    Err(e) => msgs.push(format!("Failed to remove worktree {}: {}", w, e)),
                }
            }

            if msgs.is_empty() {
                msgs.push("No cleaning performed.".to_string());
            }

            Ok(msgs.join(", "))
        }
        Action::PruneRemotes => {
            let has_origin = match sys.run_git_command_async(path, &["remote"]).await {
                Ok(out) => out.lines().any(|l| l.trim() == "origin"),
                Err(_) => false,
            };

            if !has_origin {
                return Ok("No 'origin' remote found. Skipped.".to_string());
            }

            match sys
                .run_git_command_async(path, &["remote", "prune", "origin"])
                .await
            {
                Ok(stdout) => {
                    let mut stdout = stdout.trim().to_string();
                    if stdout.is_empty() {
                        stdout = "No dead branches to prune.".to_string();
                    }
                    Ok(stdout)
                }
                Err(e) => Err(EngineError::ExecutionError(format!(
                    "Failed to prune remotes: {}",
                    e
                ))),
            }
        }
        Action::GarbageCollect => match sys.run_git_command_async(path, &["gc"]).await {
            Ok(stdout) => {
                let mut stdout = stdout.trim().to_string();
                if stdout.is_empty() {
                    stdout = "Garbage collection completed.".to_string();
                }
                Ok(stdout)
            }
            Err(e) => Err(EngineError::ExecutionError(format!(
                "Failed to garbage collect: {}",
                e
            ))),
        },
        Action::DeepClean => {
            match sys
                .run_git_command_async(path, &["clean", "-xdff", "--exclude=.git"])
                .await
            {
                Ok(_) => {
                    Ok("Deep clean successful. Untracked and ignored files purged.".to_string())
                }
                Err(e) => Err(EngineError::ExecutionError(format!(
                    "Failed to deep clean: {}",
                    e
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::mock::MockSystem;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_execute_batch_clean_repo() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");

        mock.add_command_output(&path, &["branch", "-D", "feature/test"], Ok("".to_string()));
        mock.add_command_output(&path, &["stash", "drop", "stash@{0}"], Ok("".to_string()));
        mock.add_command_output(
            &path,
            &["worktree", "remove", "--force", "/fake/repo/wt1"],
            Ok("".to_string()),
        );

        let action = Action::CleanRepo {
            branches: vec!["feature/test".to_string()],
            stashes: vec![0],
            worktrees: vec!["/fake/repo/wt1".to_string()],
        };

        let results = execute_batch(vec![path.clone()], action, false, mock).await;

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert!(results[0].message.contains("Deleted branch feature/test"));
        assert!(results[0].message.contains("Dropped stash 0"));
        assert!(
            results[0]
                .message
                .contains("Removed worktree /fake/repo/wt1")
        );
    }

    #[tokio::test]
    async fn test_execute_batch_prune_remotes() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");

        mock.add_command_output(&path, &["remote"], Ok("origin\n".to_string()));
        mock.add_command_output(&path, &["remote", "prune", "origin"], Ok("".to_string()));

        let action = Action::PruneRemotes;
        let results = execute_batch(vec![path.clone()], action, false, mock).await;

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(results[0].message, "No dead branches to prune.");
    }
}
