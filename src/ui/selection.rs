//! Items selected for cleanup, across every repository. This is the cleanup
//! queue: whatever view an item was selected from, it ends up here.

use crate::config::Config;
use crate::engine::{OpResult, Operation, RepoPlan};
use crate::git::RepoStatus;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RepoSelection {
    pub branches: BTreeSet<String>,
    /// Stash commit ids.
    pub stashes: BTreeSet<String>,
    /// Worktree paths.
    pub worktrees: BTreeSet<String>,
}

impl RepoSelection {
    pub fn len(&self) -> usize {
        self.branches.len() + self.stashes.len() + self.worktrees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What kind of item an entry of the selection is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Worktree,
    Branch,
    Stash,
}

#[derive(Debug, Default, Clone)]
pub struct Selection {
    repos: BTreeMap<PathBuf, RepoSelection>,
}

impl Selection {
    pub fn repo(&self, path: &Path) -> Option<&RepoSelection> {
        self.repos.get(path)
    }

    pub fn contains(&self, path: &Path, kind: ItemKind, id: &str) -> bool {
        self.repos
            .get(path)
            .is_some_and(|sel| set(sel, kind).contains(id))
    }

    /// Adds or removes an item; returns whether it is now selected.
    pub fn toggle(&mut self, path: &Path, kind: ItemKind, id: &str) -> bool {
        if self.contains(path, kind, id) {
            self.remove(path, kind, id);
            false
        } else {
            self.insert(path, kind, id);
            true
        }
    }

    pub fn insert(&mut self, path: &Path, kind: ItemKind, id: &str) {
        let sel = self.repos.entry(path.to_path_buf()).or_default();
        set_mut(sel, kind).insert(id.to_string());
    }

    pub fn remove(&mut self, path: &Path, kind: ItemKind, id: &str) {
        if let Some(sel) = self.repos.get_mut(path) {
            set_mut(sel, kind).remove(id);
            if sel.is_empty() {
                self.repos.remove(path);
            }
        }
    }

    pub fn clear_repo(&mut self, path: &Path) {
        self.repos.remove(path);
    }

    pub fn clear(&mut self) {
        self.repos.clear();
    }

    pub fn len(&self) -> usize {
        self.repos.values().map(RepoSelection::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.repos.is_empty()
    }

    pub fn repo_count(&self) -> usize {
        self.repos.len()
    }

    /// `(branches, stashes, worktrees)` over all repositories.
    pub fn counts(&self) -> (usize, usize, usize) {
        self.repos.values().fold((0, 0, 0), |(b, s, w), sel| {
            (
                b + sel.branches.len(),
                s + sel.stashes.len(),
                w + sel.worktrees.len(),
            )
        })
    }

    /// Drops the items of `repo` that no longer exist, e.g. after a refresh.
    pub fn retain_existing(&mut self, repo: &RepoStatus) {
        let Some(sel) = self.repos.get_mut(&repo.path) else {
            return;
        };
        sel.branches
            .retain(|b| repo.branches.iter().any(|x| &x.name == b));
        sel.stashes
            .retain(|s| repo.stashes.iter().any(|x| &x.sha == s));
        sel.worktrees
            .retain(|w| repo.worktrees.iter().any(|x| &x.path == w));
        if sel.is_empty() {
            self.repos.remove(&repo.path);
        }
    }

    /// Removes the items that were successfully cleaned up.
    pub fn forget_done(&mut self, results: &[OpResult]) {
        for result in results.iter().filter(|r| r.is_ok()) {
            let (kind, id) = match &result.operation {
                Operation::DeleteBranch { name, .. } => (ItemKind::Branch, name),
                Operation::DropStash { sha, .. } => (ItemKind::Stash, sha),
                Operation::RemoveWorktree { path, .. } => (ItemKind::Worktree, path),
                _ => continue,
            };
            self.remove(&result.repo, kind, id);
        }
    }

    /// Engine plans for the whole selection. Protected items are filtered out.
    pub fn plans(&self, repos: &[RepoStatus], config: &Config) -> Vec<RepoPlan> {
        self.repos
            .iter()
            .filter_map(|(path, sel)| {
                let repo = repos.iter().find(|r| &r.path == path)?;
                let operations = crate::cleanup::cleanup_operations(
                    repo,
                    sel.branches.iter().map(String::as_str),
                    sel.stashes.iter().map(String::as_str),
                    sel.worktrees.iter().map(String::as_str),
                    config,
                );
                (!operations.is_empty()).then(|| RepoPlan {
                    repo: path.clone(),
                    operations,
                })
            })
            .collect()
    }
}

fn set(sel: &RepoSelection, kind: ItemKind) -> &BTreeSet<String> {
    match kind {
        ItemKind::Branch => &sel.branches,
        ItemKind::Stash => &sel.stashes,
        ItemKind::Worktree => &sel.worktrees,
    }
}

fn set_mut(sel: &mut RepoSelection, kind: ItemKind) -> &mut BTreeSet<String> {
    match kind {
        ItemKind::Branch => &mut sel.branches,
        ItemKind::Stash => &mut sel.stashes,
        ItemKind::Worktree => &mut sel.worktrees,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Operation;
    use crate::git::models::BranchInfo;

    fn repo(path: &str, branches: &[&str]) -> RepoStatus {
        let mut repo = RepoStatus::pending(PathBuf::from(path));
        repo.analyzed = true;
        repo.default_branch = Some("main".into());
        repo.branches = branches
            .iter()
            .map(|name| BranchInfo {
                name: name.to_string(),
                sha: format!("{name}-sha"),
                ..Default::default()
            })
            .collect();
        repo
    }

    #[test]
    fn toggles_items_per_repository() {
        let mut sel = Selection::default();
        let a = Path::new("/a");
        assert!(sel.toggle(a, ItemKind::Branch, "feat"));
        assert!(sel.contains(a, ItemKind::Branch, "feat"));
        assert!(!sel.contains(Path::new("/b"), ItemKind::Branch, "feat"));
        assert!(!sel.toggle(a, ItemKind::Branch, "feat"));
        assert!(sel.is_empty());
    }

    #[test]
    fn builds_one_plan_per_repository() {
        let repos = vec![repo("/a", &["main", "x"]), repo("/b", &["y", "z"])];
        let mut sel = Selection::default();
        sel.insert(Path::new("/a"), ItemKind::Branch, "x");
        sel.insert(Path::new("/a"), ItemKind::Branch, "main"); // protected
        sel.insert(Path::new("/b"), ItemKind::Branch, "z");
        sel.insert(Path::new("/b"), ItemKind::Branch, "y");
        assert_eq!(sel.counts(), (4, 0, 0));
        assert_eq!(sel.repo_count(), 2);

        let plans = sel.plans(&repos, &Config::default());
        assert_eq!(plans.len(), 2);
        assert_eq!(
            plans[0].operations,
            vec![Operation::DeleteBranch {
                name: "x".into(),
                sha: "x-sha".into()
            }]
        );
        assert_eq!(plans[1].operations.len(), 2);
    }

    #[test]
    fn forgets_items_that_disappeared() {
        let mut sel = Selection::default();
        sel.insert(Path::new("/a"), ItemKind::Branch, "gone");
        sel.insert(Path::new("/a"), ItemKind::Branch, "kept");
        sel.retain_existing(&repo("/a", &["kept"]));
        assert_eq!(sel.len(), 1);
        assert!(sel.contains(Path::new("/a"), ItemKind::Branch, "kept"));
    }

    #[test]
    fn forgets_only_successful_operations() {
        let mut sel = Selection::default();
        sel.insert(Path::new("/a"), ItemKind::Branch, "done");
        sel.insert(Path::new("/a"), ItemKind::Branch, "failed");
        let result = |name: &str, ok: bool| OpResult {
            repo: PathBuf::from("/a"),
            operation: Operation::DeleteBranch {
                name: name.into(),
                sha: String::new(),
            },
            outcome: if ok {
                Ok(String::new())
            } else {
                Err(String::new())
            },
            freed_bytes: 0,
        };
        sel.forget_done(&[result("done", true), result("failed", false)]);
        assert!(!sel.contains(Path::new("/a"), ItemKind::Branch, "done"));
        assert!(sel.contains(Path::new("/a"), ItemKind::Branch, "failed"));
    }
}
