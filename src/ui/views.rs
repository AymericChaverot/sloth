//! What each list shows, in which order: pure functions of the state, shared
//! by rendering and key handling so the cursor always matches the screen.

use crate::git::RepoStatus;
use crate::git::models::{BranchInfo, StashInfo, WorktreeInfo};
use crate::ui::selection::ItemKind;
use crate::ui::state::{AppState, BranchFilter, BranchSort, RepoSort};

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

/// `(repository index, branch index)` of the rows of the Branches tab.
pub fn branch_rows(state: &AppState) -> Vec<(usize, usize)> {
    let now = crate::git::stats::now_ts();
    let query = state.branch_query.to_lowercase();
    let config = &state.config;
    let mut rows: Vec<(usize, usize)> = Vec::new();
    for (ri, repo) in state.repositories.iter().enumerate() {
        let repo_name = state.display_path(&repo.path).to_lowercase();
        for (bi, b) in repo.branches.iter().enumerate() {
            let protected = crate::cleanup::branch_protection(repo, b, config).is_some();
            let keep = match state.branch_filter {
                BranchFilter::Cleanable => crate::cleanup::is_smart_candidate(repo, b, config),
                BranchFilter::Merged => !protected && b.is_fully_merged(),
                BranchFilter::Gone => !protected && b.is_dead,
                BranchFilter::Stale => !protected && crate::cleanup::is_stale(b, config, now),
                BranchFilter::Unmerged => !protected && b.has_unique_commits(),
                BranchFilter::All => true,
            };
            let matches = query.is_empty()
                || b.name.to_lowercase().contains(&query)
                || repo_name.contains(&query);
            if keep && matches {
                rows.push((ri, bi));
            }
        }
    }

    let repos = &state.repositories;
    let branch = |&(r, b): &(usize, usize)| &repos[r].branches[b];
    rows.sort_by(|a, b| match state.branch_sort {
        BranchSort::Repository => repos[a.0]
            .path
            .cmp(&repos[b.0].path)
            .then_with(|| branch(a).name.cmp(&branch(b).name)),
        BranchSort::Oldest => branch(a)
            .last_commit_ts
            .unwrap_or(i64::MAX)
            .cmp(&branch(b).last_commit_ts.unwrap_or(i64::MAX)),
        BranchSort::Name => branch(a)
            .name
            .cmp(&branch(b).name)
            .then_with(|| repos[a.0].path.cmp(&repos[b.0].path)),
    });
    rows
}

/// Repositories with something to clean, most promising first.
pub fn dashboard_rows(state: &AppState) -> Vec<usize> {
    let config = &state.config;
    let score = |repo: &RepoStatus| {
        (
            cleanable_branches(repo, config) + repo.stashes.len(),
            repo.untracked_size_bytes.unwrap_or(0),
        )
    };
    let mut rows: Vec<usize> = (0..state.repositories.len())
        .filter(|&i| score(&state.repositories[i]) != (0, 0))
        .collect();
    rows.sort_by(|&a, &b| {
        score(&state.repositories[b])
            .cmp(&score(&state.repositories[a]))
            .then_with(|| state.repositories[a].path.cmp(&state.repositories[b].path))
    });
    rows
}

/// Rows of the Queue tab: every operation the queue would run, per repository.
pub fn queue_rows(state: &AppState) -> Vec<(std::path::PathBuf, crate::engine::Operation)> {
    state
        .selection
        .plans(&state.repositories, &state.config)
        .into_iter()
        .flat_map(|plan| {
            let repo = plan.repo;
            plan.operations
                .into_iter()
                .map(move |op| (repo.clone(), op))
        })
        .collect()
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
