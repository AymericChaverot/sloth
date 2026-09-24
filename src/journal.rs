//! Append-only log of deleted branches and dropped stashes, so they can be
//! restored: git keeps the commits around until they are garbage collected.
//!
//! Location: `$SLOTH_JOURNAL` if set, otherwise `<data dir>/sloth/journal.jsonl`.

use crate::engine::{OpResult, Operation};
use crate::sys::GitExecutor;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Branch,
    Stash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Groups the entries of one cleanup run.
    pub batch: i64,
    pub ts: i64,
    pub repo: PathBuf,
    pub kind: EntryKind,
    /// Branch name, or stash message.
    pub name: String,
    pub sha: String,
}

impl Entry {
    /// Journal entry for a successful, restorable operation.
    pub fn from_result(batch: i64, result: &OpResult) -> Option<Self> {
        if !result.is_ok() {
            return None;
        }
        let (kind, name, sha) = match &result.operation {
            Operation::DeleteBranch { name, sha } => (EntryKind::Branch, name, sha),
            Operation::DropStash { message, sha } => (EntryKind::Stash, message, sha),
            _ => return None,
        };
        Some(Self {
            batch,
            ts: crate::git::stats::now_ts(),
            repo: std::path::absolute(&result.repo).unwrap_or_else(|_| result.repo.clone()),
            kind,
            name: name.clone(),
            sha: sha.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Journal {
    path: PathBuf,
}

impl Journal {
    pub fn open_default() -> Option<Self> {
        let path = match std::env::var_os("SLOTH_JOURNAL") {
            Some(p) => PathBuf::from(p),
            None => dirs::data_local_dir()?.join("sloth").join("journal.jsonl"),
        };
        Some(Self { path })
    }

    #[cfg(test)]
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, entry: &Entry) -> std::io::Result<()> {
        let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(entry).map_err(std::io::Error::other)?;
        writeln!(file, "{line}")
    }

    /// All entries, oldest first. Unreadable lines are skipped.
    pub fn entries(&self) -> Vec<Entry> {
        std::fs::read_to_string(&self.path)
            .map(|text| {
                text.lines()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// An engine observer that records every restorable operation.
    pub fn observer(self, batch: i64) -> crate::engine::Observer {
        std::sync::Arc::new(move |result: &OpResult| {
            if let Some(entry) = Entry::from_result(batch, result) {
                let _ = self.append(&entry);
            }
        })
    }
}

/// Recreates a deleted branch (under a new name if the old one is taken) or
/// re-stores a dropped stash.
pub fn restore(entry: &Entry, sys: &impl GitExecutor) -> Result<String, String> {
    let repo = entry.repo.as_path();
    if !repo.exists() {
        return Err(format!("repository {} no longer exists", repo.display()));
    }
    let object = format!("{}^{{commit}}", entry.sha);
    if sys
        .run_git_command(repo, &["cat-file", "-e", &object])
        .is_err()
    {
        return Err("commit is gone (garbage collected)".into());
    }
    match entry.kind {
        EntryKind::Branch => {
            let taken = |name: &str| {
                sys.run_git_command(
                    repo,
                    &[
                        "rev-parse",
                        "--verify",
                        "--quiet",
                        &format!("refs/heads/{name}"),
                    ],
                )
                .is_ok()
            };
            let mut name = entry.name.clone();
            if taken(&name) {
                name = format!(
                    "{}-restored-{}",
                    entry.name,
                    &entry.sha[..entry.sha.len().min(7)]
                );
            }
            sys.run_git_command(repo, &["branch", &name, &entry.sha])
                .map(|_| format!("branch {name} restored"))
                .map_err(|e| e.to_string().trim().to_string())
        }
        EntryKind::Stash => sys
            .run_git_command(repo, &["stash", "store", "-m", &entry.name, &entry.sha])
            .map(|_| format!("stash \"{}\" restored", entry.name))
            .map_err(|e| e.to_string().trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_journal(name: &str) -> Journal {
        let path = std::env::temp_dir().join(format!(
            "sloth-journal-{}-{}.jsonl",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        Journal::at(path)
    }

    fn result(operation: Operation, ok: bool) -> OpResult {
        OpResult {
            repo: PathBuf::from("/r"),
            operation,
            outcome: if ok {
                Ok(String::new())
            } else {
                Err(String::new())
            },
            freed_bytes: 0,
        }
    }

    #[test]
    fn records_only_successful_restorable_operations() {
        let journal = temp_journal("observer");
        let observer = journal.clone().observer(42);
        let branch = Operation::DeleteBranch {
            name: "feat".into(),
            sha: "abc".into(),
        };
        observer(&result(branch.clone(), true));
        observer(&result(branch, false));
        observer(&result(Operation::GarbageCollect, true));
        observer(&result(
            Operation::DropStash {
                sha: "s1".into(),
                message: "wip".into(),
            },
            true,
        ));

        let entries = journal.entries();
        let _ = std::fs::remove_file(journal.path());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, EntryKind::Branch);
        assert_eq!(entries[0].batch, 42);
        assert_eq!(entries[1].name, "wip");
    }

    #[test]
    fn restores_deleted_branch_and_stash() {
        let repo = crate::test_support::TempRepo::new("restore");
        repo.commit("a.txt", "a\n");
        repo.git(&["branch", "feat"]);
        let sha = repo.git(&["rev-parse", "feat"]).trim().to_string();
        std::fs::write(repo.path.join("a.txt"), "changed\n").unwrap();
        repo.git(&["stash", "push", "-q", "-m", "my wip"]);
        let stash_sha = repo.git(&["rev-parse", "stash@{0}"]).trim().to_string();
        repo.git(&["branch", "-D", "feat"]);
        repo.git(&["stash", "drop", "-q"]);

        let entry = |kind, name: &str, sha: &str| Entry {
            batch: 1,
            ts: 1,
            repo: repo.path.clone(),
            kind,
            name: name.into(),
            sha: sha.into(),
        };
        let sys = crate::sys::RealSystem;
        assert!(restore(&entry(EntryKind::Branch, "feat", &sha), &sys).is_ok());
        assert_eq!(repo.git(&["rev-parse", "feat"]).trim(), sha);

        // The name is taken now: restored under another name.
        let again = restore(&entry(EntryKind::Branch, "feat", &sha), &sys).unwrap();
        assert!(again.contains("feat-restored-"));

        assert!(restore(&entry(EntryKind::Stash, "my wip", &stash_sha), &sys).is_ok());
        assert!(repo.git(&["stash", "list"]).contains("my wip"));

        let missing = entry(
            EntryKind::Branch,
            "x",
            "0000000000000000000000000000000000000000",
        );
        assert!(restore(&missing, &sys).is_err());
    }
}
