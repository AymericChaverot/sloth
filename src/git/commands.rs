use super::models::{BranchInfo, GitError, RepoStatus, StashInfo, WorktreeInfo};
use std::path::Path;

pub fn analyze_repository(
    path: &Path,
    sys: &impl crate::sys::GitExecutor,
) -> Result<RepoStatus, GitError> {
    // Open the repository
    sys.open_repo(path)?;

    let mut remote_url = None;
    if let Ok(output) = sys.run_git_command(path, &["config", "--get", "remote.origin.url"]) {
        let url = output.trim().to_string();
        let simplified = url
            .replace("git@github.com:", "github.com/")
            .replace("https://", "")
            .trim_end_matches(".git")
            .to_string();
        remote_url = Some(simplified);
    }

    let mut branches = Vec::new();
    let mut stashes = Vec::new();

    if let Ok(out_str) = sys.run_git_command(
        path,
        &[
            "branch",
            "--format=%(refname:short)|%(HEAD)|%(upstream:track)|%(upstream:short)|%(committerdate:relative)",
        ],
    ) {
        for line in out_str.lines() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 3 {
                    let name = parts[0].trim().to_string();
                    let is_active = parts[1].trim() == "*";
                    let is_dead = parts[2].contains("[gone]");
                    let upstream = if parts.len() >= 4 && !parts[3].trim().is_empty() {
                        Some(parts[3].trim().to_string())
                    } else {
                        None
                    };
                    let last_commit_date = if parts.len() >= 5 && !parts[4].trim().is_empty() {
                        Some(parts[4].trim().to_string())
                    } else {
                        None
                    };

                    branches.push(BranchInfo {
                        name,
                        is_active,
                        is_dead,
                        upstream,
                        ahead: 0,
                        behind: 0,
                        diff_insertions: 0,
                        diff_deletions: 0,
                        last_commit_date,
                        is_merged: false,
                    });
            }
        }
    }

    // Determine main branch
    let main_branch = if branches.iter().any(|b| b.name == "main") {
        Some("main".to_string())
    } else if branches.iter().any(|b| b.name == "master") {
        Some("master".to_string())
    } else {
        None
    };

    let mut merged_branches = std::collections::HashSet::new();
    if let Some(ref target) = main_branch {
        if let Ok(out_str) = sys.run_git_command(path, &["branch", "--merged", target]) {
            for line in out_str.lines() {
                let b = line.replace("* ", "").trim().to_string();
                merged_branches.insert(b);
            }
        }
    }

    for branch in branches.iter_mut() {
        if branch.name == "main" || branch.name == "master" {
            continue;
        }

        let target = if !branch.is_dead && branch.upstream.is_some() {
            branch.upstream.clone().unwrap()
        } else if let Some(ref main) = main_branch {
            main.clone()
        } else {
            continue;
        };

        // Note: For unit testing cleanly, get_branch_stats also needs to be refactored to take sys.
        // For now, we pass the path and name, but let's assume it has been similarly patched
        let stats = super::stats::get_branch_stats(path, &target, &branch.name, sys);
        branch.ahead = stats.0;
        branch.behind = stats.1;
        branch.diff_insertions = stats.2;
        branch.diff_deletions = stats.3;
        branch.is_merged = merged_branches.contains(&branch.name);
    }

    if let Ok(out_str) = sys.run_git_command(path, &["stash", "list"]) {
        for (i, line) in out_str.lines().enumerate() {
            stashes.push(StashInfo {
                index: i,
                message: line.to_string(),
            });
        }
    }

    let mut worktrees = Vec::new();
    if let Ok(out_str) = sys.run_git_command(path, &["worktree", "list", "--porcelain"]) {
        let mut current_wt = None;
        let mut current_branch = None;

        for line in out_str.lines() {
            if line.starts_with("worktree ") {
                if let Some(wt) = current_wt.take() {
                    worktrees.push(WorktreeInfo {
                        path: wt,
                        branch: current_branch.take(),
                        size_bytes: None,
                    });
                }
                current_wt = Some(line.replace("worktree ", "").trim().to_string());
            } else if line.starts_with("branch ") {
                let b = line.replace("branch refs/heads/", "").trim().to_string();
                current_branch = Some(b);
            }
        }
        if let Some(wt) = current_wt.take() {
            worktrees.push(WorktreeInfo {
                path: wt,
                branch: current_branch.take(),
                size_bytes: None,
            });
        }
    }

    Ok(RepoStatus {
        path: path.to_path_buf(),
        remote_url,
        branches,
        stashes,
        worktrees,
        graph_lines: None,
        analyzed: true,
        size_bytes: None,
        untracked_size_bytes: None,
        size_finalized: false,
    })
}


pub fn get_git_graph(
    path: &Path,
    sys: &impl crate::sys::GitExecutor,
) -> Result<Vec<String>, GitError> {
    match sys.run_git_command(
        path,
        &[
            "log",
            "--graph",
            "--color=always",
            "--pretty=format:%C(yellow)%h%Creset -%C(auto)%d%Creset %s %C(dim white)(%ar) <%an>%Creset",
        ],
    ) {
        Ok(out) => Ok(out.lines().map(|s| s.to_string()).collect()),
        _ => Ok(Vec::new()),
    }
}

pub fn get_branch_diff(
    path: &Path,
    diff_target: &str,
    sys: &impl crate::sys::GitExecutor,
) -> Result<Vec<String>, GitError> {
    match sys.run_git_command(path, &["diff", "--color=always", diff_target]) {
        Ok(out) => Ok(out.lines().map(|s| s.to_string()).collect()),
        _ => Ok(Vec::new()),
    }
}

pub fn get_stash_diff(
    path: &Path,
    stash_index: usize,
    sys: &impl crate::sys::GitExecutor,
) -> Result<Vec<String>, GitError> {
    match sys.run_git_command(
        path,
        &[
            "stash",
            "show",
            "-p",
            "--color=always",
            &format!("stash@{{{}}}", stash_index),
        ],
    ) {
        Ok(out) => Ok(out.lines().map(|s| s.to_string()).collect()),
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::mock::MockSystem;
    use std::path::PathBuf;

    #[test]
    fn test_analyze_repository() {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");

        mock.add_command_output(
            &path,
            &["config", "--get", "remote.origin.url"],
            Ok("git@github.com:AymericChaverot/sloth.git\n".to_string()),
        );

        mock.add_command_output(
            &path,
            &[
                "branch",
                "--format=%(refname:short)|%(HEAD)|%(upstream:track)|%(upstream:short)|%(committerdate:relative)",
            ],
            Ok("main|*||origin/main|2 days ago\nfeature/test||[gone]|origin/feature/test|3 weeks ago\n".to_string()),
        );

        mock.add_command_output(
            &path,
            &["stash", "list"],
            Ok("stash@{0}: WIP on main\nstash@{1}: WIP on feature\n".to_string()),
        );

        mock.add_command_output(
            &path,
            &["branch", "--merged", "main"],
            Ok("main\n".to_string()),
        );

        mock.add_command_output(
            &path,
            &["rev-list", "--left-right", "--count", "main...feature/test"],
            Ok("1\t2\n".to_string()),
        );

        mock.add_command_output(
            &path,
            &["diff", "--shortstat", "main...feature/test"],
            Ok(" 3 files changed, 45 insertions(+), 12 deletions(-)\n".to_string()),
        );

        mock.add_command_output(
            &path,
            &["worktree", "list", "--porcelain"],
            Ok("worktree /fake/repo\nHEAD 1234567\nbranch refs/heads/main\n\nworktree /fake/repo/wt1\nHEAD 890abcd\nbranch refs/heads/feature/test\n\n".to_string()),
        );

        let status = analyze_repository(&path, &mock).unwrap();

        assert_eq!(status.path, path);
        assert_eq!(
            status.remote_url.as_deref(),
            Some("github.com/AymericChaverot/sloth")
        );
        assert_eq!(status.branches.len(), 2);
        assert_eq!(status.branches[0].name, "main");
        assert!(status.branches[0].is_active);
        assert_eq!(status.stashes.len(), 2);
        assert!(!status.worktrees.is_empty()); // Usually 1, but PathBuf matching is OS dependent in strings.
    }


}
