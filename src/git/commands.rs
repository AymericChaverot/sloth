use super::models::{BranchInfo, GitError, RepoStatus, StashInfo, WorktreeInfo};
use std::path::Path;
use std::process::Command;

/// Analyzes a Git repository to extract its status
pub fn analyze_repository(path: &Path) -> Result<RepoStatus, GitError> {
    // Open the repository
    let _repo = gix::open(path)?;

    let mut remote_url = None;
    if let Ok(output) = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(path)
        .output()
    {
        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let simplified = url
                .replace("git@github.com:", "github.com/")
                .replace("https://", "")
                .trim_end_matches(".git")
                .to_string();
            remote_url = Some(simplified);
        }
    }

    let mut branches = Vec::new();
    let mut stashes = Vec::new();

    if let Ok(output) = Command::new("git")
        .args([
            "branch",
            "--format=%(refname:short)|%(HEAD)|%(upstream:track)|%(upstream:short)|%(committerdate:relative)",
        ])
        .current_dir(path)
        .output()
    {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout);
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
        if let Ok(output) = Command::new("git")
            .args(["branch", "--merged", target])
            .current_dir(path)
            .output()
        {
            if output.status.success() {
                let out_str = String::from_utf8_lossy(&output.stdout);
                for line in out_str.lines() {
                    let b = line.replace("* ", "").trim().to_string();
                    merged_branches.insert(b);
                }
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

        let stats = super::stats::get_branch_stats(path, &target, &branch.name);
        branch.ahead = stats.0;
        branch.behind = stats.1;
        branch.diff_insertions = stats.2;
        branch.diff_deletions = stats.3;
        branch.is_merged = merged_branches.contains(&branch.name);
    }

    if let Ok(output) = Command::new("git")
        .args(["stash", "list"])
        .current_dir(path)
        .output()
    {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            for (i, line) in out_str.lines().enumerate() {
                stashes.push(StashInfo {
                    index: i,
                    message: line.to_string(),
                });
            }
        }
    }

    let mut worktrees = Vec::new();
    if let Ok(output) = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(path)
        .output()
    {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout);
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
    })
}

/// Computes the disk space sizes asynchronously to avoid blocking the main analysis loop
pub fn compute_repo_sizes(
    path: &Path,
    worktree_paths: Vec<String>,
) -> (
    Option<u64>,                                    // size_bytes (.git)
    Option<u64>,                                    // untracked_size_bytes
    std::collections::HashMap<String, Option<u64>>, // worktree sizes
) {
    let size_bytes = super::stats::get_repo_size(&path.join(".git")).ok();

    let mut untracked_size = 0;
    let mut has_untracked = false;
    if let Ok(output) = Command::new("git")
        .args(["clean", "-ndx"])
        .current_dir(path)
        .output()
    {
        if output.status.success() {
            has_untracked = true;
            let out_str = String::from_utf8_lossy(&output.stdout);
            for line in out_str.lines() {
                if line.starts_with("Would remove ") {
                    let to_remove = line.trim_start_matches("Would remove ");
                    let full_path = path.join(to_remove);
                    if full_path.exists() {
                        if full_path.is_dir() {
                            untracked_size += super::stats::get_repo_size(&full_path).unwrap_or(0);
                        } else {
                            untracked_size +=
                                std::fs::metadata(&full_path).map(|m| m.len()).unwrap_or(0);
                        }
                    }
                }
            }
        }
    }

    let untracked_size_bytes = if has_untracked {
        Some(untracked_size)
    } else {
        None
    };

    let mut worktree_sizes = std::collections::HashMap::new();
    for wt in worktree_paths {
        let s = super::stats::get_repo_size(Path::new(&wt)).ok();
        worktree_sizes.insert(wt, s);
    }

    (size_bytes, untracked_size_bytes, worktree_sizes)
}

pub fn get_git_graph(path: &Path) -> Result<Vec<String>, GitError> {
    let output = Command::new("git")
        .args([
            "log",
            "--graph",
            "--color=always",
            "--pretty=format:%C(yellow)%h%Creset -%C(auto)%d%Creset %s %C(dim white)(%ar) <%an>%Creset",
        ])
        .current_dir(path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(stdout.lines().map(|s| s.to_string()).collect())
        }
        _ => Ok(Vec::new()),
    }
}

pub fn get_branch_diff(path: &Path, diff_target: &str) -> Result<Vec<String>, GitError> {
    let output = Command::new("git")
        .args(["diff", "--color=always", diff_target])
        .current_dir(path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(stdout.lines().map(|s| s.to_string()).collect())
        }
        _ => Ok(Vec::new()),
    }
}

pub fn get_stash_diff(path: &Path, stash_index: usize) -> Result<Vec<String>, GitError> {
    let output = Command::new("git")
        .args([
            "stash",
            "show",
            "-p",
            "--color=always",
            &format!("stash@{{{}}}", stash_index),
        ])
        .current_dir(path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(stdout.lines().map(|s| s.to_string()).collect())
        }
        _ => Ok(Vec::new()),
    }
}
