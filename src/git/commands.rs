use super::models::{BranchInfo, GitError, RepoStatus, StashInfo};
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

    Ok(RepoStatus {
        path: path.to_path_buf(),
        remote_url,
        branches,
        stashes,
        graph_lines: None,
        analyzed: true,
    })
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
