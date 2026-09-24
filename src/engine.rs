//! Executes cleanup plans: repositories run concurrently, the operations of a
//! repository run in order.

use crate::sys::{FileSystem, GitExecutor};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Maximum number of repositories processed at the same time.
const MAX_PARALLEL_REPOS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Deleted only if the branch still points to `sha`.
    DeleteBranch {
        name: String,
        sha: String,
    },
    /// Stashes are matched by commit id, so indices shifting is harmless.
    DropStash {
        sha: String,
        message: String,
    },
    /// `force` is required to discard uncommitted changes in the worktree.
    RemoveWorktree {
        path: String,
        force: bool,
        prunable: bool,
    },
    PruneRemotes,
    GarbageCollect,
    /// Removes untracked and ignored files, except those matching `keep`.
    DeepClean {
        keep: Vec<String>,
    },
}

impl Operation {
    pub fn describe(&self) -> String {
        match self {
            Operation::DeleteBranch { name, .. } => format!("delete branch {name}"),
            Operation::DropStash { message, .. } => format!("drop stash \"{message}\""),
            Operation::RemoveWorktree { path, .. } => format!("remove worktree {path}"),
            Operation::PruneRemotes => "prune remote-tracking branches".into(),
            Operation::GarbageCollect => "garbage collect".into(),
            Operation::DeepClean { .. } => "deep clean untracked and ignored files".into(),
        }
    }
}

/// Operations to run on one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPlan {
    pub repo: PathBuf,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone)]
pub struct OpResult {
    pub repo: PathBuf,
    pub operation: Operation,
    pub outcome: Result<String, String>,
    /// Disk space released by the operation, when it can be measured.
    pub freed_bytes: u64,
}

impl OpResult {
    pub fn is_ok(&self) -> bool {
        self.outcome.is_ok()
    }
}

/// Called after every operation, e.g. to report progress.
pub type Observer = Arc<dyn Fn(&OpResult) + Send + Sync>;

pub async fn execute<T>(
    plans: Vec<RepoPlan>,
    dry_run: bool,
    sys: T,
    observer: Option<Observer>,
) -> Vec<OpResult>
where
    T: GitExecutor + FileSystem + Clone + Send + Sync + 'static,
{
    let limit = Arc::new(tokio::sync::Semaphore::new(MAX_PARALLEL_REPOS));
    let mut tasks = tokio::task::JoinSet::new();

    for (order, plan) in plans.into_iter().enumerate() {
        let sys = sys.clone();
        let observer = observer.clone();
        let limit = limit.clone();
        tasks.spawn(async move {
            let _permit = limit.acquire_owned().await;
            let mut results = Vec::with_capacity(plan.operations.len());
            for operation in plan.operations {
                let (outcome, freed_bytes) = if dry_run {
                    (Ok(format!("dry run: would {}", operation.describe())), 0)
                } else {
                    run(&plan.repo, &operation, &sys).await
                };
                let result = OpResult {
                    repo: plan.repo.clone(),
                    operation,
                    outcome,
                    freed_bytes,
                };
                if let Some(observer) = &observer {
                    observer(&result);
                }
                results.push(result);
            }
            (order, results)
        });
    }

    let mut by_plan = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        if let Ok(done) = joined {
            by_plan.push(done);
        }
    }
    // Report in plan order regardless of completion order.
    by_plan.sort_by_key(|(order, _)| *order);
    by_plan.into_iter().flat_map(|(_, r)| r).collect()
}

async fn run(
    repo: &Path,
    operation: &Operation,
    sys: &(impl GitExecutor + FileSystem),
) -> (Result<String, String>, u64) {
    let git = |args: Vec<String>| async move {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        sys.run_git_command_async(repo, &args)
            .await
            .map_err(|e| e.to_string().trim().to_string())
    };
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();

    match operation {
        Operation::DeleteBranch { name, sha } => {
            let reference = format!("refs/heads/{name}");
            match git(args(&["rev-parse", "--verify", "--quiet", &reference])).await {
                Ok(current) if current.trim() == sha => {}
                Ok(_) => return (Err("branch moved since the analysis, skipped".into()), 0),
                Err(_) => return (Err("branch no longer exists".into()), 0),
            }
            let outcome = git(args(&["branch", "-D", name]))
                .await
                .map(|_| format!("deleted (was {})", short(sha)));
            (outcome, 0)
        }
        Operation::DropStash { sha, .. } => {
            let list = match git(args(&["stash", "list", "--format=%H"])).await {
                Ok(list) => list,
                Err(e) => return (Err(e), 0),
            };
            let Some(index) = list.lines().position(|l| l.trim() == sha) else {
                return (Err("stash no longer exists".into()), 0);
            };
            let outcome = git(args(&["stash", "drop", &format!("stash@{{{index}}}")]))
                .await
                .map(|_| format!("dropped (was {})", short(sha)));
            (outcome, 0)
        }
        Operation::RemoveWorktree {
            path,
            force,
            prunable,
        } => {
            if *prunable {
                let outcome = git(args(&["worktree", "prune"]))
                    .await
                    .map(|_| "pruned stale worktree entry".to_string());
                return (outcome, 0);
            }
            let size = sys.get_size(Path::new(path)).unwrap_or(0);
            let mut cmd = args(&["worktree", "remove"]);
            if *force {
                cmd.push("--force".into());
            }
            cmd.push(path.clone());
            match git(cmd).await {
                Ok(_) => (Ok("removed".into()), size),
                Err(e) => (Err(e), 0),
            }
        }
        Operation::PruneRemotes => {
            let remotes = git(args(&["remote"])).await.unwrap_or_default();
            if !remotes.lines().any(|l| l.trim() == "origin") {
                return (Ok("no 'origin' remote, skipped".into()), 0);
            }
            let outcome = git(args(&["remote", "prune", "origin"])).await.map(|out| {
                let pruned = out.lines().filter(|l| l.contains("[pruned]")).count();
                match pruned {
                    0 => "nothing to prune".to_string(),
                    n => format!("pruned {n} remote-tracking branch(es)"),
                }
            });
            (outcome, 0)
        }
        Operation::GarbageCollect => {
            let git_dir = repo.join(".git");
            let before = sys.get_size(&git_dir).unwrap_or(0);
            match git(args(&["gc", "--quiet"])).await {
                Ok(_) => {
                    let after = sys.get_size(&git_dir).unwrap_or(before);
                    (Ok("done".into()), before.saturating_sub(after))
                }
                Err(e) => (Err(e), 0),
            }
        }
        Operation::DeepClean { keep } => {
            let freed: u64 = deep_clean_candidates(repo, keep, sys)
                .unwrap_or_default()
                .iter()
                .map(|p| sys.get_size(&repo.join(p)).unwrap_or(0))
                .sum();
            let outcome = git(deep_clean_args(false, keep))
                .await
                .map(|out| format!("removed {} path(s)", deep_clean_paths(&out).count()));
            let freed = if outcome.is_ok() { freed } else { 0 };
            (outcome, freed)
        }
    }
}

/// `git clean` arguments for a deep clean (untracked and ignored files).
/// A single `-f` leaves nested repositories alone: they are separate projects.
/// Paths matching a `keep` pattern survive (`-e` still applies with `-x`).
pub fn deep_clean_args(dry_run: bool, keep: &[String]) -> Vec<String> {
    let mode = if dry_run { "-n" } else { "-f" };
    let mut args: Vec<String> = ["clean", "-x", "-d", mode]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for pattern in keep {
        args.push("-e".into());
        args.push(pattern.clone());
    }
    args
}

/// What a deep clean would remove, relative to the repository: ignored and
/// untracked paths (whole directories collapsed to `dir/`), minus `keep`
/// matches and nested repositories.
///
/// Uses `git ls-files --directory`, which does not descend into ignored
/// directories: `git clean -n` does, and can take minutes on a
/// `node_modules` containing a junction back to the repository.
pub fn deep_clean_candidates(
    repo: &Path,
    keep: &[String],
    sys: &(impl GitExecutor + FileSystem),
) -> std::io::Result<Vec<String>> {
    let base = [
        "ls-files",
        "-z",
        "--others",
        "--exclude-standard",
        "--directory",
    ];
    let untracked = sys.run_git_command(repo, &base)?;
    let mut with_ignored = base.to_vec();
    with_ignored.push("--ignored");
    let ignored = sys.run_git_command(repo, &with_ignored)?;

    let mut paths: Vec<String> = untracked
        .split('\0')
        .chain(ignored.split('\0'))
        .filter(|p| !p.is_empty())
        .filter(|p| !is_kept(p, keep))
        .filter(|p| !(p.ends_with('/') && sys.is_repository(&repo.join(p))))
        .map(str::to_string)
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Whether `path` (a `dir/` or a file) matches a `deep_clean_keep` pattern.
/// Patterns match the last path component; a trailing `/` only matches directories.
fn is_kept(path: &str, keep: &[String]) -> bool {
    let is_dir = path.ends_with('/');
    let name = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path);
    keep.iter().any(|pattern| match pattern.strip_suffix('/') {
        Some(dir_pattern) => is_dir && crate::config::glob_match(dir_pattern, name),
        None => crate::config::glob_match(pattern, name),
    })
}

/// Paths listed by `git clean` ("Would remove x" / "Removing x").
pub fn deep_clean_paths(output: &str) -> impl Iterator<Item = &str> {
    output.lines().filter_map(|l| {
        l.strip_prefix("Would remove ")
            .or_else(|| l.strip_prefix("Removing "))
    })
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(8)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::mock::MockSystem;
    use std::sync::Mutex;

    fn plan(ops: Vec<Operation>) -> Vec<RepoPlan> {
        vec![RepoPlan {
            repo: PathBuf::from("/fake/repo"),
            operations: ops,
        }]
    }

    #[tokio::test]
    async fn deletes_branch_only_if_unchanged() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");
        for (name, sha) in [("done", "aaa"), ("moved", "new")] {
            mock.add_command_output(
                &path,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{name}"),
                ],
                Ok(format!("{sha}\n")),
            );
        }
        mock.add_command_output(&path, &["branch", "-D", "done"], Ok(String::new()));

        let results = execute(
            plan(vec![
                Operation::DeleteBranch {
                    name: "done".into(),
                    sha: "aaa".into(),
                },
                Operation::DeleteBranch {
                    name: "moved".into(),
                    sha: "old".into(),
                },
            ]),
            false,
            mock,
            None,
        )
        .await;

        assert!(results[0].is_ok());
        assert!(!results[1].is_ok());
    }

    #[tokio::test]
    async fn drops_stash_by_sha() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");
        mock.add_command_output(
            &path,
            &["stash", "list", "--format=%H"],
            Ok("s0\ns1\ns2\n".into()),
        );
        mock.add_command_output(&path, &["stash", "drop", "stash@{2}"], Ok(String::new()));

        let results = execute(
            plan(vec![Operation::DropStash {
                sha: "s2".into(),
                message: "wip".into(),
            }]),
            false,
            mock,
            None,
        )
        .await;
        assert!(results[0].is_ok(), "{:?}", results[0].outcome);
    }

    #[tokio::test]
    async fn removes_dirty_worktree_with_force_only() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");
        mock.add_command_output(
            &path,
            &["worktree", "remove", "--force", "/wt"],
            Ok(String::new()),
        );
        mock.add_command_output(
            &path,
            &["worktree", "remove", "/wt"],
            Err("contains modified files".into()),
        );
        mock.file_sizes.insert(PathBuf::from("/wt"), 1_000);

        let op = |force| Operation::RemoveWorktree {
            path: "/wt".into(),
            force,
            prunable: false,
        };
        let results = execute(plan(vec![op(false), op(true)]), false, mock, None).await;
        assert!(!results[0].is_ok());
        assert!(results[1].is_ok());
        assert_eq!(results[1].freed_bytes, 1_000);
    }

    #[tokio::test]
    async fn prune_skips_repos_without_origin() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");
        mock.add_command_output(&path, &["remote"], Ok("upstream\n".into()));
        let results = execute(plan(vec![Operation::PruneRemotes]), false, mock, None).await;
        assert_eq!(
            results[0].outcome.as_deref(),
            Ok("no 'origin' remote, skipped")
        );
    }

    #[tokio::test]
    async fn prune_counts_pruned_branches() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");
        mock.add_command_output(&path, &["remote"], Ok("origin\n".into()));
        mock.add_command_output(
            &path,
            &["remote", "prune", "origin"],
            Ok("Pruning origin\n * [pruned] origin/a\n * [pruned] origin/b\n".into()),
        );
        let results = execute(plan(vec![Operation::PruneRemotes]), false, mock, None).await;
        assert_eq!(
            results[0].outcome.as_deref(),
            Ok("pruned 2 remote-tracking branch(es)")
        );
    }

    #[tokio::test]
    async fn deep_clean_reports_freed_space() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");
        mock_listing(&mut mock, &path, "notes.txt\0", "build/\0dist/\0.env\0");
        mock.add_command_output(
            &path,
            &["clean", "-x", "-d", "-f", "-e", ".env"],
            Ok("Removing build/\nRemoving dist/\nRemoving notes.txt\n".into()),
        );
        mock.file_sizes.insert(path.join("build/"), 500);
        mock.file_sizes.insert(path.join("dist/"), 20);
        mock.file_sizes.insert(path.join("notes.txt"), 3);
        mock.file_sizes.insert(path.join(".env"), 1_000);

        let op = Operation::DeepClean {
            keep: vec![".env".into()],
        };
        let results = execute(plan(vec![op]), false, mock, None).await;
        assert_eq!(results[0].outcome.as_deref(), Ok("removed 3 path(s)"));
        assert_eq!(results[0].freed_bytes, 523);
    }

    fn mock_listing(mock: &mut MockSystem, path: &Path, untracked: &str, ignored: &str) {
        let base = [
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
            "--directory",
        ];
        mock.add_command_output(path, &base, Ok(untracked.into()));
        let mut with_ignored = base.to_vec();
        with_ignored.push("--ignored");
        mock.add_command_output(path, &with_ignored, Ok(ignored.into()));
    }

    #[test]
    fn candidates_skip_kept_files_and_nested_repositories() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/r");
        mock_listing(
            &mut mock,
            &path,
            "nested/\0draft.md\0",
            "node_modules/\0.env.local\0.idea/\0sub/.vscode/\0.vscode\0",
        );
        mock.repositories.push(path.join("nested/"));
        let keep = crate::config::Config::default().deep_clean_keep;
        let candidates = deep_clean_candidates(&path, &keep, &mock).unwrap();
        // `.vscode/` only protects directories, so a `.vscode` file goes.
        assert_eq!(candidates, vec![".vscode", "draft.md", "node_modules/"]);
    }

    /// The listing matches what a real deep clean removes.
    #[test]
    fn candidates_match_real_git_clean() {
        let repo = crate::test_support::TempRepo::new("candidates");
        repo.commit(".gitignore", "build/\n*.log\n");
        std::fs::create_dir_all(repo.path.join("build/deep")).unwrap();
        std::fs::write(repo.path.join("build/deep/out.bin"), "x").unwrap();
        std::fs::write(repo.path.join("debug.log"), "x").unwrap();
        std::fs::write(repo.path.join("scratch.txt"), "x").unwrap();
        std::fs::write(repo.path.join(".env"), "x").unwrap();

        let keep = vec![".env".to_string()];
        let candidates = deep_clean_candidates(&repo.path, &keep, &crate::sys::RealSystem).unwrap();
        assert_eq!(candidates, vec!["build/", "debug.log", "scratch.txt"]);

        let args = deep_clean_args(true, &keep);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let dry_run = repo.git(&args);
        let mut listed: Vec<&str> = deep_clean_paths(&dry_run).collect();
        listed.sort();
        assert_eq!(listed, candidates);
    }

    /// Nested repositories and kept files survive a real deep clean.
    #[tokio::test]
    async fn deep_clean_spares_nested_repos_and_kept_files() {
        let repo = crate::test_support::TempRepo::new("deepclean");
        repo.commit(".gitignore", "build/\n");
        std::fs::create_dir_all(repo.path.join("build")).unwrap();
        std::fs::write(repo.path.join("build/out.bin"), "x").unwrap();
        std::fs::write(repo.path.join(".env"), "SECRET=1").unwrap();
        let nested = repo.path.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&nested)
            .status()
            .unwrap();
        std::fs::write(nested.join("work.txt"), "keep me").unwrap();

        let plans = vec![RepoPlan {
            repo: repo.path.clone(),
            operations: vec![Operation::DeepClean {
                keep: vec![".env".into()],
            }],
        }];
        let results = execute(plans, false, crate::sys::RealSystem, None).await;

        assert!(results[0].is_ok(), "{:?}", results[0].outcome);
        assert!(!repo.path.join("build").exists());
        assert!(repo.path.join(".env").exists());
        assert!(nested.join("work.txt").exists());
    }

    #[tokio::test]
    async fn dry_run_touches_nothing_and_reports_progress() {
        let seen = Arc::new(Mutex::new(0));
        let counter = seen.clone();
        let observer: Observer = Arc::new(move |_| *counter.lock().unwrap() += 1);
        let results = execute(
            plan(vec![Operation::GarbageCollect, Operation::PruneRemotes]),
            true,
            MockSystem::new(),
            Some(observer),
        )
        .await;
        assert!(results.iter().all(OpResult::is_ok));
        assert_eq!(*seen.lock().unwrap(), 2);
    }
}
