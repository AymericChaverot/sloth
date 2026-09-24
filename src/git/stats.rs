use std::path::Path;

pub(crate) fn parse_shortstat(stat: &str) -> (usize, usize) {
    let mut insertions = 0;
    let mut deletions = 0;
    for part in stat.split(',') {
        let part = part.trim();
        if part.contains("insertion") {
            let num: String = part.chars().filter(|c| c.is_ascii_digit()).collect();
            insertions = num.parse().unwrap_or(0);
        } else if part.contains("deletion") {
            let num: String = part.chars().filter(|c| c.is_ascii_digit()).collect();
            deletions = num.parse().unwrap_or(0);
        }
    }
    (insertions, deletions)
}

/// Returns `(ahead, behind)` of `branch` relative to `base`, one git call per branch.
/// Used when git is too old for `%(ahead-behind:...)`.
pub(crate) fn rev_list_ahead_behind(
    path: &Path,
    base: &str,
    branch: &str,
    sys: &impl crate::sys::GitExecutor,
) -> (usize, usize) {
    let range = format!("{}...{}", base, branch);
    let Ok(out_str) = sys.run_git_command(path, &["rev-list", "--left-right", "--count", &range])
    else {
        return (0, 0);
    };
    let parts: Vec<&str> = out_str.split_whitespace().collect();
    if parts.len() == 2 {
        let behind = parts[0].parse().unwrap_or(0);
        let ahead = parts[1].parse().unwrap_or(0);
        (ahead, behind)
    } else {
        (0, 0)
    }
}

/// Returns `(insertions, deletions)` for a diff range such as `main...feature`.
pub(crate) fn diff_shortstat(
    path: &Path,
    range: &str,
    sys: &impl crate::sys::GitExecutor,
) -> (usize, usize) {
    sys.run_git_command(path, &["diff", "--shortstat", range])
        .map(|out| parse_shortstat(&out))
        .unwrap_or((0, 0))
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

pub fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Compact relative age such as `5m`, `3h`, `12d`, `4mo`, `2y`.
pub fn format_age(ts: i64, now: i64) -> String {
    let secs = (now - ts).max(0);
    let (value, unit) = match secs {
        s if s < 3_600 => (s / 60, "m"),
        s if s < 86_400 => (s / 3_600, "h"),
        s if s < 86_400 * 60 => (s / 86_400, "d"),
        s if s < 86_400 * 365 => (s / (86_400 * 30), "mo"),
        s => (s / (86_400 * 365), "y"),
    };
    format!("{value}{unit}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_age() {
        let now = 1_000_000_000;
        assert_eq!(format_age(now - 30, now), "0m");
        assert_eq!(format_age(now - 7_200, now), "2h");
        assert_eq!(format_age(now - 86_400 * 3, now), "3d");
        assert_eq!(format_age(now - 86_400 * 90, now), "3mo");
        assert_eq!(format_age(now - 86_400 * 800, now), "2y");
        assert_eq!(format_age(now + 50, now), "0m");
    }

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
