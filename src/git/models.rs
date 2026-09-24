use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Failed to open repository: {0}")]
    OpenError(Box<gix::open::Error>),
}

impl From<gix::open::Error> for GitError {
    fn from(e: gix::open::Error) -> Self {
        GitError::OpenError(Box::new(e))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BranchInfo {
    pub name: String,
    /// Commit the branch points to.
    pub sha: String,
    /// Checked out in the worktree sloth was pointed at.
    pub is_active: bool,
    /// Worktree where this branch is checked out (main or linked), if any.
    pub worktree_path: Option<String>,
    /// The upstream tracking branch was deleted on the remote.
    pub is_dead: bool,
    pub upstream: Option<String>,
    /// Commits not yet pushed to the upstream.
    pub upstream_ahead: usize,
    /// Commits on the upstream not yet pulled.
    pub upstream_behind: usize,
    /// Commits on this branch that are not on the default branch.
    pub ahead: usize,
    /// Commits on the default branch that are not on this branch.
    pub behind: usize,
    pub diff_insertions: usize,
    pub diff_deletions: usize,
    /// Committer date of the tip, as a unix timestamp.
    pub last_commit_ts: Option<i64>,
    /// Every commit of the branch is reachable from the default branch.
    pub is_merged: bool,
    /// The branch content landed on the default branch as a squashed commit.
    pub is_squash_merged: bool,
}

impl BranchInfo {
    pub fn is_fully_merged(&self) -> bool {
        self.is_merged || self.is_squash_merged
    }

    /// Commits that would be lost if the branch were deleted: they are neither
    /// on the default branch nor pushed to a live upstream.
    pub fn has_unique_commits(&self) -> bool {
        if self.is_fully_merged() || self.ahead == 0 {
            return false;
        }
        match (&self.upstream, self.is_dead) {
            (Some(_), false) => self.upstream_ahead > 0,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashInfo {
    pub index: usize,
    /// Stash commit id: stable identity across drops, and what a restore needs.
    pub sha: String,
    pub message: String,
    pub created_ts: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: Option<String>,
    /// The main worktree cannot be removed.
    pub is_main: bool,
    pub is_dirty: bool,
    pub is_locked: bool,
    /// The worktree directory is gone; git can prune it.
    pub is_prunable: bool,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub path: PathBuf,
    pub remote_url: Option<String>,
    pub default_branch: Option<String>,
    pub current_branch: Option<String>,
    /// Tracked files have uncommitted modifications.
    pub is_dirty: bool,
    pub branches: Vec<BranchInfo>,
    pub stashes: Vec<StashInfo>,
    pub worktrees: Vec<WorktreeInfo>,
    pub graph_lines: Option<Vec<String>>,
    pub analyzed: bool,
    /// Set when the analysis failed.
    pub error: Option<String>,
    pub size_bytes: Option<u64>,
    pub untracked_size_bytes: Option<u64>,
    pub size_finalized: bool,
}

impl RepoStatus {
    /// A repository that has been discovered but not analyzed yet.
    pub fn pending(path: PathBuf) -> Self {
        Self {
            path,
            remote_url: None,
            default_branch: None,
            current_branch: None,
            is_dirty: false,
            branches: Vec::new(),
            stashes: Vec::new(),
            worktrees: Vec::new(),
            graph_lines: None,
            analyzed: false,
            error: None,
            size_bytes: None,
            untracked_size_bytes: None,
            size_finalized: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch() -> BranchInfo {
        BranchInfo {
            name: "feature".into(),
            ahead: 2,
            ..Default::default()
        }
    }

    #[test]
    fn local_only_branch_with_commits_is_unique() {
        assert!(branch().has_unique_commits());
    }

    #[test]
    fn merged_branch_has_no_unique_commits() {
        let mut b = branch();
        b.is_squash_merged = true;
        assert!(!b.has_unique_commits());
    }

    #[test]
    fn pushed_branch_has_no_unique_commits() {
        let mut b = branch();
        b.upstream = Some("origin/feature".into());
        assert!(!b.has_unique_commits());
        b.upstream_ahead = 1;
        assert!(b.has_unique_commits());
    }

    #[test]
    fn gone_upstream_does_not_protect_commits() {
        let mut b = branch();
        b.upstream = Some("origin/feature".into());
        b.is_dead = true;
        assert!(b.has_unique_commits());
    }
}
