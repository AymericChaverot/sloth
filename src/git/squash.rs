//! Detects branches whose work landed on the default branch without a merge
//! commit: rebase-merged (commits cherry-picked one by one) or squash-merged
//! (all changes folded into a single commit). `git branch --merged` misses both.

use super::models::BranchInfo;
use crate::sys::GitExecutor;
use std::collections::HashSet;
use std::path::Path;

/// Upper bound of default-branch commits inspected for squashed patches.
const MAX_SCANNED_COMMITS: &str = "1000";

pub fn detect_hidden_merges(
    path: &Path,
    default: &str,
    branches: &mut [BranchInfo],
    sys: &impl GitExecutor,
) {
    let candidates: Vec<usize> = branches
        .iter()
        .enumerate()
        .filter(|(_, b)| is_candidate(b, default))
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return;
    }

    // Patch ids of the default-branch commits made since the oldest candidate
    // tip: a squash of that branch can only have landed afterwards.
    let since = candidates
        .iter()
        .filter_map(|&i| branches[i].last_commit_ts)
        .min()
        .unwrap_or(0);
    let default_patch_ids = default_branch_patch_ids(path, default, since, sys);

    for i in candidates {
        let branch = &mut branches[i];
        if is_rebase_merged(path, default, &branch.name, sys)
            || branch_patch_id(path, default, &branch.name, sys)
                .is_some_and(|id| default_patch_ids.contains(&id))
        {
            branch.is_squash_merged = true;
        }
    }
}

/// Only unmerged branches without a live upstream are worth the extra git calls:
/// that is where finished work that was squashed on the forge ends up.
fn is_candidate(branch: &BranchInfo, default: &str) -> bool {
    branch.name != default
        && !branch.is_merged
        && branch.ahead > 0
        && (branch.upstream.is_none() || branch.is_dead)
}

/// Every commit of the branch has an equivalent patch on the default branch.
fn is_rebase_merged(path: &Path, default: &str, branch: &str, sys: &impl GitExecutor) -> bool {
    sys.run_git_command(path, &["cherry", default, branch])
        .is_ok_and(|out| {
            let mut lines = out.lines().peekable();
            lines.peek().is_some() && lines.all(|l| l.starts_with('-'))
        })
}

fn default_branch_patch_ids(
    path: &Path,
    default: &str,
    since: i64,
    sys: &impl GitExecutor,
) -> HashSet<String> {
    let since = format!("--since={since}");
    let Ok(log) = sys.run_git_command(
        path,
        &[
            "log",
            "-p",
            "--no-color",
            "--no-merges",
            "-n",
            MAX_SCANNED_COMMITS,
            &since,
            default,
        ],
    ) else {
        return HashSet::new();
    };
    if log.trim().is_empty() {
        return HashSet::new();
    }
    sys.run_git_command_with_input(path, &["patch-id", "--stable"], &log)
        .map(|out| parse_patch_ids(&out))
        .unwrap_or_default()
}

/// Patch id of the whole branch as if it were squashed into one commit.
fn branch_patch_id(
    path: &Path,
    default: &str,
    branch: &str,
    sys: &impl GitExecutor,
) -> Option<String> {
    let base = sys
        .run_git_command(path, &["merge-base", default, branch])
        .ok()?;
    let range = format!("{}..{}", base.trim(), branch);
    let diff = sys
        .run_git_command(path, &["diff", "--no-color", &range])
        .ok()?;
    if diff.trim().is_empty() {
        return None;
    }
    let out = sys
        .run_git_command_with_input(path, &["patch-id", "--stable"], &diff)
        .ok()?;
    parse_patch_ids(&out).into_iter().next()
}

/// `git patch-id` prints `<patch-id> <commit-id>` per patch.
fn parse_patch_ids(out: &str) -> HashSet<String> {
    out.lines()
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::mock::MockSystem;
    use std::path::PathBuf;

    fn branch(name: &str) -> BranchInfo {
        BranchInfo {
            name: name.into(),
            ahead: 1,
            last_commit_ts: Some(100),
            ..Default::default()
        }
    }

    fn log_args() -> Vec<&'static str> {
        vec![
            "log",
            "-p",
            "--no-color",
            "--no-merges",
            "-n",
            MAX_SCANNED_COMMITS,
            "--since=100",
            "main",
        ]
    }

    #[test]
    fn skips_branches_with_live_upstream() {
        let mut b = branch("wip");
        b.upstream = Some("origin/wip".into());
        assert!(!is_candidate(&b, "main"));
        b.is_dead = true;
        assert!(is_candidate(&b, "main"));
    }

    #[test]
    fn detects_squash_merge() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/r");
        mock.add_command_output(&path, &["cherry", "main", "feat"], Ok("+ abc\n".into()));
        mock.add_command_output(&path, &log_args(), Ok("LOG".into()));
        mock.add_command_output_with_input(
            &path,
            &["patch-id", "--stable"],
            "LOG",
            Ok("p1 c1\np2 c2\n".into()),
        );
        mock.add_command_output(&path, &["merge-base", "main", "feat"], Ok("base\n".into()));
        mock.add_command_output(
            &path,
            &["diff", "--no-color", "base..feat"],
            Ok("DIFF".into()),
        );
        mock.add_command_output_with_input(
            &path,
            &["patch-id", "--stable"],
            "DIFF",
            Ok("p2 0000\n".into()),
        );

        let mut branches = vec![branch("feat")];
        detect_hidden_merges(&path, "main", &mut branches, &mock);
        assert!(branches[0].is_squash_merged);
    }

    #[test]
    fn detects_rebase_merge() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/r");
        mock.add_command_output(
            &path,
            &["cherry", "main", "feat"],
            Ok("- abc\n- def\n".into()),
        );
        let mut branches = vec![branch("feat")];
        detect_hidden_merges(&path, "main", &mut branches, &mock);
        assert!(branches[0].is_squash_merged);
    }

    /// End-to-end check against a real git repository.
    #[test]
    fn detects_squash_merge_in_real_repo() {
        let repo = crate::test_support::TempRepo::new("squash");
        repo.commit("a.txt", "base\n");
        repo.git(&["checkout", "-q", "-b", "feat"]);
        repo.commit("b.txt", "one\n");
        repo.commit("b.txt", "one\ntwo\n");
        repo.git(&["checkout", "-q", "main"]);
        repo.git(&["merge", "-q", "--squash", "feat"]);
        repo.git(&["commit", "-q", "-m", "squashed feat"]);
        repo.git(&["checkout", "-q", "-b", "wip"]);
        repo.commit("c.txt", "unfinished\n");
        repo.git(&["checkout", "-q", "main"]);

        let status = crate::git::analyze_repository(&repo.path, &crate::sys::RealSystem).unwrap();
        let find = |name: &str| status.branches.iter().find(|b| b.name == name).unwrap();
        assert!(find("feat").is_squash_merged);
        assert!(!find("feat").is_merged);
        assert!(!find("wip").is_squash_merged);
        assert!(find("wip").has_unique_commits());
    }

    #[test]
    fn keeps_unmerged_work() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/r");
        mock.add_command_output(&path, &["cherry", "main", "feat"], Ok("+ abc\n".into()));
        mock.add_command_output(&path, &log_args(), Ok("LOG".into()));
        mock.add_command_output_with_input(
            &path,
            &["patch-id", "--stable"],
            "LOG",
            Ok("p1 c1\n".into()),
        );
        mock.add_command_output(&path, &["merge-base", "main", "feat"], Ok("base\n".into()));
        mock.add_command_output(
            &path,
            &["diff", "--no-color", "base..feat"],
            Ok("DIFF".into()),
        );
        mock.add_command_output_with_input(
            &path,
            &["patch-id", "--stable"],
            "DIFF",
            Ok("p9 0000\n".into()),
        );

        let mut branches = vec![branch("feat")];
        detect_hidden_merges(&path, "main", &mut branches, &mock);
        assert!(!branches[0].is_squash_merged);
    }
}
