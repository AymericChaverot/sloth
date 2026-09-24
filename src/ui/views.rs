//! What each list shows, in which order: pure functions of the state, shared
//! by rendering and key handling so the cursor always matches the screen.

use crate::git::RepoStatus;
use crate::git::models::{BranchInfo, StashInfo, WorktreeInfo};
use crate::ui::selection::ItemKind;
use crate::ui::state::{AppState, RepoSort};

/// Indices into `state.repositories` of the repositories to list, filtered and sorted.
pub fn visible_repos(state: &AppState) -> Vec<usize> {
    let filter = state.repo_filter.to_lowercase();
    let mut indices: Vec<usize> = state
        .repositories
        .iter()
        .enumerate()
        .filter(|(_, repo)| {
            filter.is_empty()
                || state
                    .display_path(&repo.path)
                    .to_lowercase()
                    .contains(&filter)
                || repo
                    .remote_url
                    .as_deref()
                    .is_some_and(|url| url.to_lowercase().contains(&filter))
        })
        .map(|(i, _)| i)
        .collect();

    let repos = &state.repositories;
    indices.sort_by(|&a, &b| {
        let (ra, rb) = (&repos[a], &repos[b]);
        let by_path = || ra.path.cmp(&rb.path);
        match state.repo_sort {
            RepoSort::Path => by_path(),
            RepoSort::Cleanable => cleanable_branches(rb, &state.config)
                .cmp(&cleanable_branches(ra, &state.config))
                .then_with(by_path),
            RepoSort::Reclaimable => rb
                .untracked_size_bytes
                .cmp(&ra.untracked_size_bytes)
                .then_with(by_path),
            RepoSort::GitSize => rb.size_bytes.cmp(&ra.size_bytes).then_with(by_path),
        }
    });
    indices
}

/// Branches that smart selection would pick in this repository.
pub fn cleanable_branches(repo: &RepoStatus, config: &crate::config::Config) -> usize {
    repo.branches
        .iter()
        .filter(|b| crate::cleanup::is_smart_candidate(repo, b, config))
        .count()
}

/// One row of the details table.
#[derive(Debug, Clone, Copy)]
pub enum DetailRow<'a> {
    Branch(&'a BranchInfo),
    Stash(&'a StashInfo),
    Worktree(&'a WorktreeInfo),
}

impl DetailRow<'_> {
    /// Selection identity of the row.
    pub fn item(&self) -> (ItemKind, &str) {
        match self {
            DetailRow::Branch(b) => (ItemKind::Branch, &b.name),
            DetailRow::Stash(s) => (ItemKind::Stash, &s.sha),
            DetailRow::Worktree(w) => (ItemKind::Worktree, &w.path),
        }
    }

    pub fn protection(
        &self,
        repo: &RepoStatus,
        config: &crate::config::Config,
    ) -> Option<crate::cleanup::Protection> {
        match self {
            DetailRow::Branch(b) => crate::cleanup::branch_protection(repo, b, config),
            DetailRow::Stash(_) => None,
            DetailRow::Worktree(w) => crate::cleanup::worktree_protection(w),
        }
    }
}

/// Branches, then stashes, then worktrees.
pub fn detail_rows(repo: &RepoStatus) -> Vec<DetailRow<'_>> {
    repo.branches
        .iter()
        .map(DetailRow::Branch)
        .chain(repo.stashes.iter().map(DetailRow::Stash))
        .chain(repo.worktrees.iter().map(DetailRow::Worktree))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn sorts_by_cleanable_branches_then_path() {
        let mut state = AppState::new(crate::config::Config::default(), PathBuf::from("/w"));
        let mut busy = RepoStatus::pending(PathBuf::from("/w/z"));
        busy.branches = vec![BranchInfo {
            name: "old".into(),
            is_merged: true,
            ..Default::default()
        }];
        state.repositories = vec![
            RepoStatus::pending(PathBuf::from("/w/b")),
            busy,
            RepoStatus::pending(PathBuf::from("/w/a")),
        ];
        state.repo_sort = RepoSort::Cleanable;
        assert_eq!(visible_repos(&state), vec![1, 2, 0]);
    }

    #[test]
    fn filters_on_path_and_remote() {
        let mut state = AppState::new(crate::config::Config::default(), PathBuf::from("/w"));
        let mut remote = RepoStatus::pending(PathBuf::from("/w/x"));
        remote.remote_url = Some("github.com/acme/Payments".into());
        state.repositories = vec![RepoStatus::pending(PathBuf::from("/w/pay")), remote];
        state.repo_filter = "PAY".into();
        assert_eq!(visible_repos(&state), vec![0, 1]);
        state.repo_filter = "acme".into();
        assert_eq!(visible_repos(&state), vec![1]);
    }
}
