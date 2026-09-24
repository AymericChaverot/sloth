//! Cleanup domain rules: what may be deleted, and why not.

use crate::config::{Config, glob_match};
use crate::engine::Operation;
use crate::git::models::{BranchInfo, RepoStatus, StashInfo, WorktreeInfo};

/// Why an item cannot be selected for deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    DefaultBranch,
    CheckedOut,
    Configured,
    MainWorktree,
}

impl Protection {
    pub fn label(self) -> &'static str {
        match self {
            Protection::DefaultBranch => "default",
            Protection::CheckedOut => "checked out",
            Protection::Configured => "protected",
            Protection::MainWorktree => "main",
        }
    }
}

pub fn branch_protection(
    repo: &RepoStatus,
    branch: &BranchInfo,
    config: &Config,
) -> Option<Protection> {
    if repo.default_branch.as_deref() == Some(branch.name.as_str()) {
        Some(Protection::DefaultBranch)
    } else if branch.is_active || branch.worktree_path.is_some() {
        Some(Protection::CheckedOut)
    } else if config
        .protected_branches
        .iter()
        .any(|pattern| glob_match(pattern, &branch.name))
    {
        Some(Protection::Configured)
    } else {
        None
    }
}

pub fn worktree_protection(worktree: &WorktreeInfo) -> Option<Protection> {
    worktree.is_main.then_some(Protection::MainWorktree)
}

/// No commit for more than `stale_days` days.
pub fn is_stale(branch: &BranchInfo, config: &Config, now: i64) -> bool {
    branch
        .last_commit_ts
        .is_some_and(|ts| now - ts > i64::from(config.stale_days) * 86_400)
}

/// Branches that are safe to clean up: merged (in any way) or whose upstream is gone.
pub fn is_smart_candidate(repo: &RepoStatus, branch: &BranchInfo, config: &Config) -> bool {
    branch_protection(repo, branch, config).is_none()
        && (branch.is_fully_merged() || branch.is_dead)
}

/// Turns selected items into engine operations. Protected or unknown items are
/// dropped, so a plan can never touch something the rules forbid.
pub fn cleanup_operations<'a>(
    repo: &RepoStatus,
    branches: impl IntoIterator<Item = &'a str>,
    stash_shas: impl IntoIterator<Item = &'a str>,
    worktree_paths: impl IntoIterator<Item = &'a str>,
    config: &Config,
) -> Vec<Operation> {
    let mut operations = Vec::new();

    let mut worktrees: Vec<&WorktreeInfo> = worktree_paths
        .into_iter()
        .filter_map(|p| repo.worktrees.iter().find(|w| w.path == p))
        .filter(|w| worktree_protection(w).is_none())
        .collect();
    worktrees.sort_by(|a, b| a.path.cmp(&b.path));
    // Worktrees first: a branch checked out in a removed worktree becomes deletable.
    operations.extend(worktrees.iter().map(|w| Operation::RemoveWorktree {
        path: w.path.clone(),
        force: w.is_dirty,
        prunable: w.is_prunable,
    }));

    let freed: Vec<&str> = worktrees
        .iter()
        .filter_map(|w| w.branch.as_deref())
        .collect();
    let mut selected: Vec<&BranchInfo> = branches
        .into_iter()
        .filter_map(|name| repo.branches.iter().find(|b| b.name == name))
        .filter(|b| match branch_protection(repo, b, config) {
            None => true,
            Some(Protection::CheckedOut) => !b.is_active && freed.contains(&b.name.as_str()),
            Some(_) => false,
        })
        .collect();
    selected.sort_by(|a, b| a.name.cmp(&b.name));
    operations.extend(selected.iter().map(|b| Operation::DeleteBranch {
        name: b.name.clone(),
        sha: b.sha.clone(),
    }));

    let mut stashes: Vec<&StashInfo> = stash_shas
        .into_iter()
        .filter_map(|sha| repo.stashes.iter().find(|s| s.sha == sha))
        .collect();
    stashes.sort_by_key(|s| s.index);
    operations.extend(stashes.iter().map(|s| Operation::DropStash {
        sha: s.sha.clone(),
        message: s.message.clone(),
    }));

    operations
}

/// What could be lost by running `op`, if anything.
pub fn operation_warning(repo: &RepoStatus, op: &Operation) -> Option<&'static str> {
    match op {
        Operation::DeleteBranch { name, .. } => repo
            .branches
            .iter()
            .find(|b| &b.name == name)
            .filter(|b| b.has_unique_commits())
            .map(|_| "unmerged, unpushed commits"),
        Operation::RemoveWorktree { force: true, .. } => Some("uncommitted changes"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn plan_filters_protected_items_and_orders_worktrees_first() {
        let mut repo = repo();
        repo.branches = vec![
            branch("trunk"),
            BranchInfo {
                sha: "f1".into(),
                worktree_path: Some("/wt".into()),
                ..branch("feat")
            },
            BranchInfo {
                sha: "o1".into(),
                ..branch("old")
            },
        ];
        repo.worktrees = vec![
            WorktreeInfo {
                path: "/r".into(),
                is_main: true,
                ..Default::default()
            },
            WorktreeInfo {
                path: "/wt".into(),
                branch: Some("feat".into()),
                is_dirty: true,
                ..Default::default()
            },
        ];
        repo.stashes = vec![StashInfo {
            index: 0,
            sha: "s0".into(),
            message: "wip".into(),
            created_ts: None,
        }];

        let ops = cleanup_operations(
            &repo,
            ["trunk", "old", "feat", "missing"],
            ["s0", "nope"],
            ["/r", "/wt"],
            &Config::default(),
        );
        assert_eq!(
            ops,
            vec![
                Operation::RemoveWorktree {
                    path: "/wt".into(),
                    force: true,
                    prunable: false
                },
                Operation::DeleteBranch {
                    name: "feat".into(),
                    sha: "f1".into()
                },
                Operation::DeleteBranch {
                    name: "old".into(),
                    sha: "o1".into()
                },
                Operation::DropStash {
                    sha: "s0".into(),
                    message: "wip".into()
                },
            ]
        );
    }

    #[test]
    fn plan_keeps_branch_of_kept_worktree() {
        let mut repo = repo();
        repo.branches = vec![BranchInfo {
            worktree_path: Some("/wt".into()),
            ..branch("feat")
        }];
        let ops = cleanup_operations(&repo, ["feat"], [], [], &Config::default());
        assert!(ops.is_empty());
    }

    fn repo() -> RepoStatus {
        let mut repo = RepoStatus::pending(PathBuf::from("/r"));
        repo.default_branch = Some("trunk".into());
        repo
    }

    fn branch(name: &str) -> BranchInfo {
        BranchInfo {
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn protects_default_checked_out_and_configured_branches() {
        let config = Config::default();
        let repo = repo();
        assert_eq!(
            branch_protection(&repo, &branch("trunk"), &config),
            Some(Protection::DefaultBranch)
        );
        let mut checked_out = branch("feat");
        checked_out.worktree_path = Some("/wt".into());
        assert_eq!(
            branch_protection(&repo, &checked_out, &config),
            Some(Protection::CheckedOut)
        );
        assert_eq!(
            branch_protection(&repo, &branch("release/1.0"), &config),
            Some(Protection::Configured)
        );
        assert_eq!(branch_protection(&repo, &branch("feat"), &config), None);
    }

    #[test]
    fn smart_selection_skips_protected_and_unmerged() {
        let config = Config::default();
        let repo = repo();
        let mut merged = branch("main");
        merged.is_merged = true;
        assert!(!is_smart_candidate(&repo, &merged, &config));

        let mut gone = branch("feat");
        gone.is_dead = true;
        assert!(is_smart_candidate(&repo, &gone, &config));

        let mut wip = branch("wip");
        wip.ahead = 3;
        assert!(!is_smart_candidate(&repo, &wip, &config));
    }

    #[test]
    fn main_worktree_is_protected() {
        let wt = WorktreeInfo {
            is_main: true,
            ..Default::default()
        };
        assert_eq!(worktree_protection(&wt), Some(Protection::MainWorktree));
    }
}
