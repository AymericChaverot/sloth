use std::path::Path;
use std::process::Command;

pub(crate) fn parse_shortstat(stat: &str) -> (usize, usize) {
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

pub(crate) fn get_branch_stats(
    path: &Path,
    main_branch: &str,
    branch: &str,
) -> (usize, usize, usize, usize) {
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

pub fn get_repo_size(path: &Path) -> Result<u64, std::io::Error> {
    let mut size = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                size += get_repo_size(&path)?;
            } else {
                size += entry.metadata()?.len();
            }
        }
    }
    Ok(size)
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
