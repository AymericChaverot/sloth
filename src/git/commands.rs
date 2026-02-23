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
            "--format=%(refname:short)|%(HEAD)|%(upstream:track)|%(upstream:short)",
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

                    branches.push(BranchInfo {
                        name,
                        is_active,
                        is_dead,
                        upstream,
                        ahead: 0,
                        behind: 0,
                        diff_insertions: 0,
                        diff_deletions: 0,
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

        let stats = get_branch_stats(path, &target, &branch.name);
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
    })
}

pub fn get_git_graph(path: &Path) -> Result<Vec<String>, GitError> {
    let output = Command::new("git")
        .args([
            "log",
            "--graph",
            "--color=always",
            // Custom pretty format: <Hash> - (<Date>) <Message> <BranchDecorations> - <Author>
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

fn parse_shortstat(stat: &str) -> (usize, usize) {
    let mut insertions = 0;
    let mut deletions = 0;
    for part in stat.split(',') {
        let part = part.trim();
        if part.contains("insertion") {
            let num: String = part.chars().filter(|c| c.is_digit(10)).collect();
            insertions = num.parse().unwrap_or(0);
        } else if part.contains("deletion") {
            let num: String = part.chars().filter(|c| c.is_digit(10)).collect();
            deletions = num.parse().unwrap_or(0);
        }
    }
    (insertions, deletions)
}

fn get_branch_stats(path: &Path, main_branch: &str, branch: &str) -> (usize, usize, usize, usize) {
    let mut ahead = 0;
    let mut behind = 0;
    let mut insertions = 0;
    let mut deletions = 0;

    let diff_target = format!("{}...{}", main_branch, branch);

    if let Ok(output) = Command::new("git")
        .args(["rev-list", "--left-right", "--count", &diff_target])
        .current_dir(path)
        .output()
    {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = out_str.split_whitespace().collect();
            if parts.len() == 2 {
                behind = parts[0].parse().unwrap_or(0);
                ahead = parts[1].parse().unwrap_or(0);
            }
        }
    }

    if let Ok(output) = Command::new("git")
        .args(["diff", "--shortstat", &diff_target])
        .current_dir(path)
        .output()
    {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            let (i, d) = parse_shortstat(&out_str);
            insertions = i;
            deletions = d;
        }
    }

    (ahead, behind, insertions, deletions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shortstat_both() {
        let input = " 3 files changed, 45 insertions(+), 12 deletions(-)";
        let (ins, del) = parse_shortstat(input);
        assert_eq!(ins, 45);
        assert_eq!(del, 12);
    }

    #[test]
    fn test_parse_shortstat_insertions_only() {
        let input = " 1 file changed, 10 insertions(+)";
        let (ins, del) = parse_shortstat(input);
        assert_eq!(ins, 10);
        assert_eq!(del, 0);
    }

    #[test]
    fn test_parse_shortstat_deletions_only() {
        let input = " 2 files changed, 5 deletions(-)";
        let (ins, del) = parse_shortstat(input);
        assert_eq!(ins, 0);
        assert_eq!(del, 5);
    }

    #[test]
    fn test_parse_shortstat_empty() {
        let input = "";
        let (ins, del) = parse_shortstat(input);
        assert_eq!(ins, 0);
        assert_eq!(del, 0);
    }

    #[test]
    fn test_parse_shortstat_malformed() {
        let input = " this is not a shortstat output";
        let (ins, del) = parse_shortstat(input);
        assert_eq!(ins, 0);
        assert_eq!(del, 0);
    }
}
