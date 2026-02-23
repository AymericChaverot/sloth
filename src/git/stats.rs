use std::path::Path;

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
    sys: &impl crate::sys::GitExecutor,
) -> (usize, usize, usize, usize) {
    let mut ahead = 0;
    let mut behind = 0;
    let mut insertions = 0;
    let mut deletions = 0;

    let diff_target = format!("{}...{}", main_branch, branch);

    if let Ok(out_str) =
        sys.run_git_command(path, &["rev-list", "--left-right", "--count", &diff_target])
    {
        let parts: Vec<&str> = out_str.split_whitespace().collect();
        if parts.len() == 2 {
            behind = parts[0].parse().unwrap_or(0);
            ahead = parts[1].parse().unwrap_or(0);
        }
    }

    if let Ok(out_str) = sys.run_git_command(path, &["diff", "--shortstat", &diff_target]) {
        let (i, d) = parse_shortstat(&out_str);
        insertions = i;
        deletions = d;
    }

    (ahead, behind, insertions, deletions)
}

// Note: abstracting directory traversal natively into the trait is slightly more complex,
// for now `FileSystem` `get_size` acts as a proxy for the entire implementation.
// So we can completely drop recursive logic here and rely on the Trait mapping.
pub fn get_repo_size(path: &Path, fs: &impl crate::sys::FileSystem) -> Result<u64, std::io::Error> {
    fs.get_size(path)
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{} B", bytes)
    }
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
