//! Cleanup domain rules: what may be deleted, and why not.

use crate::config::{Config, glob_match};
use crate::git::models::{BranchInfo, RepoStatus, WorktreeInfo};

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
            Protection::DefaultBranch => "default branch",
            Protection::CheckedOut => "checked out",
            Protection::Configured => "protected",
            Protection::MainWorktree => "main worktree",
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

/// Branches that are safe to clean up: merged (in any way) or whose upstream is gone.
pub fn is_smart_candidate(repo: &RepoStatus, branch: &BranchInfo, config: &Config) -> bool {
    branch_protection(repo, branch, config).is_none()
        && (branch.is_fully_merged() || branch.is_dead)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
