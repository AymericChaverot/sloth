use super::models::{BranchInfo, GitError, RepoStatus, StashInfo, WorktreeInfo};
use crate::sys::GitExecutor;
use std::path::Path;

const REF_FORMAT: &str = "%(refname:lstrip=2)%00%(HEAD)%00%(objectname)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(committerdate:unix)%00%(worktreepath)";

const STASH_FORMAT: &str = "--format=%gd%x00%H%x00%ct%x00%gs";

/// Branch names that are treated as the default branch, in order of preference,
/// when `origin/HEAD` is not set.
const DEFAULT_BRANCH_CANDIDATES: [&str; 4] = ["main", "master", "trunk", "develop"];

pub fn analyze_repository(path: &Path, sys: &impl GitExecutor) -> Result<RepoStatus, GitError> {
    // The first git call doubles as the check that this is a usable repository.
    let mut branches = list_branches(path, sys)?;

    let remote_url = sys
        .run_git_command(path, &["config", "--get", "remote.origin.url"])
        .ok()
        .map(|url| simplify_remote_url(url.trim()))
        .filter(|url| !url.is_empty());

    let default_branch = detect_default_branch(path, &branches, sys);
    if let Some(default) = &default_branch {
        compute_divergence(path, default, &mut branches, sys);
        super::squash::detect_hidden_merges(path, default, &mut branches, sys);
    }
    let current_branch = branches
        .iter()
        .find(|b| b.is_active)
        .map(|b| b.name.clone());

    let is_dirty = sys
        .run_git_command(path, &["status", "--porcelain", "--untracked-files=no"])
        .is_ok_and(|out| !out.trim().is_empty());

    let stashes = sys
        .run_git_command(path, &["stash", "list", STASH_FORMAT])
        .map(|out| parse_stash_list(&out))
        .unwrap_or_default();

    let mut worktrees = sys
        .run_git_command(path, &["worktree", "list", "--porcelain"])
        .map(|out| parse_worktree_list(&out))
        .unwrap_or_default();
    for wt in worktrees
        .iter_mut()
        .filter(|w| !w.is_main && !w.is_prunable)
    {
        wt.is_dirty = sys
            .run_git_command(Path::new(&wt.path), &["status", "--porcelain"])
            .is_ok_and(|out| !out.trim().is_empty());
    }

    Ok(RepoStatus {
        remote_url,
        default_branch,
        current_branch,
        is_dirty,
        branches,
        stashes,
        worktrees,
        analyzed: true,
        ..RepoStatus::pending(path.to_path_buf())
    })
}

fn list_branches(path: &Path, sys: &impl GitExecutor) -> Result<Vec<BranchInfo>, GitError> {
    let format = format!("--format={REF_FORMAT}");
    sys.run_git_command(path, &["for-each-ref", &format, "refs/heads"])
        .map(|out| out.lines().filter_map(parse_branch_line).collect())
        .map_err(|e| GitError::NotARepository(e.to_string().trim().to_string()))
}

/// Parses one line produced by [`REF_FORMAT`] (fields separated by NUL).
pub(crate) fn parse_branch_line(line: &str) -> Option<BranchInfo> {
    let fields: Vec<&str> = line.split('\0').collect();
    if fields.len() < 7 || fields[0].is_empty() {
        return None;
    }
    let upstream = Some(fields[3].trim())
        .filter(|u| !u.is_empty())
        .map(str::to_string);
    let (upstream_ahead, upstream_behind, is_dead) = parse_track(fields[4]);
    Some(BranchInfo {
        name: fields[0].to_string(),
        is_active: fields[1].trim() == "*",
        sha: fields[2].trim().to_string(),
        upstream,
        upstream_ahead,
        upstream_behind,
        is_dead,
        last_commit_ts: fields[5].trim().parse().ok(),
        worktree_path: Some(fields[6].trim())
            .filter(|p| !p.is_empty())
            .map(str::to_string),
        ..Default::default()
    })
}

/// Parses `%(upstream:track,nobracket)`: "", "gone", "ahead 1", "behind 2", "ahead 1, behind 2".
pub(crate) fn parse_track(track: &str) -> (usize, usize, bool) {
    let track = track.trim();
    if track == "gone" {
        return (0, 0, true);
    }
    let mut ahead = 0;
    let mut behind = 0;
    for part in track.split(',') {
        let mut words = part.split_whitespace();
        match (words.next(), words.next().and_then(|n| n.parse().ok())) {
            (Some("ahead"), Some(n)) => ahead = n,
            (Some("behind"), Some(n)) => behind = n,
            _ => {}
        }
    }
    (ahead, behind, false)
}

fn detect_default_branch(
    path: &Path,
    branches: &[BranchInfo],
    sys: &impl GitExecutor,
) -> Option<String> {
    let exists = |name: &str| branches.iter().any(|b| b.name == name);

    if let Ok(out) = sys.run_git_command(
        path,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ) && let Some((_, name)) = out.trim().split_once('/')
        && exists(name)
    {
        return Some(name.to_string());
    }

    DEFAULT_BRANCH_CANDIDATES
        .iter()
        .find(|name| exists(name))
        .map(|name| name.to_string())
}

/// Fills ahead/behind (relative to the default branch), merge state and diff stats.
fn compute_divergence(
    path: &Path,
    default: &str,
    branches: &mut [BranchInfo],
    sys: &impl GitExecutor,
) {
    let counts = ahead_behind_batch(path, default, sys);

    for branch in branches.iter_mut().filter(|b| b.name != default) {
        let (ahead, behind) = match &counts {
            Some(map) => map.get(&branch.name).copied().unwrap_or((0, 0)),
            None => super::stats::rev_list_ahead_behind(path, default, &branch.name, sys),
        };
        branch.ahead = ahead;
        branch.behind = behind;
        branch.is_merged = ahead == 0;

        if ahead > 0 {
            let range = format!("{default}...{}", branch.name);
            let (ins, del) = super::stats::diff_shortstat(path, &range, sys);
            branch.diff_insertions = ins;
            branch.diff_deletions = del;
        }
    }
}

/// Ahead/behind counts for every local branch in a single git call
/// (`%(ahead-behind:...)`, git >= 2.41). Returns `None` on older git.
fn ahead_behind_batch(
    path: &Path,
    default: &str,
    sys: &impl GitExecutor,
) -> Option<std::collections::HashMap<String, (usize, usize)>> {
    let format = format!("--format=%(refname:lstrip=2)%00%(ahead-behind:{default})");
    let out = sys
        .run_git_command(path, &["for-each-ref", &format, "refs/heads"])
        .ok()?;
    let mut map = std::collections::HashMap::new();
    for line in out.lines() {
        let (name, counts) = line.split_once('\0')?;
        let mut nums = counts.split_whitespace().map(|n| n.parse::<usize>());
        match (nums.next(), nums.next()) {
            (Some(Ok(ahead)), Some(Ok(behind))) => map.insert(name.to_string(), (ahead, behind)),
            _ => return None,
        };
    }
    Some(map)
}

/// Parses the output of `git stash list` with [`STASH_FORMAT`].
pub(crate) fn parse_stash_list(out: &str) -> Vec<StashInfo> {
    out.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let fields: Vec<&str> = line.split('\0').collect();
            if fields.len() < 4 {
                return None;
            }
            let index = fields[0]
                .trim_start_matches("stash@{")
                .trim_end_matches('}')
                .parse()
                .unwrap_or(i);
            Some(StashInfo {
                index,
                sha: fields[1].to_string(),
                created_ts: fields[2].parse().ok(),
                message: fields[3].to_string(),
            })
        })
        .collect()
}

/// Parses `git worktree list --porcelain`. The first entry is the main worktree.
pub(crate) fn parse_worktree_list(out: &str) -> Vec<WorktreeInfo> {
    let mut worktrees: Vec<WorktreeInfo> = Vec::new();
    for line in out.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            worktrees.push(WorktreeInfo {
                path: path.trim().to_string(),
                is_main: worktrees.is_empty(),
                ..Default::default()
            });
        } else if let Some(wt) = worktrees.last_mut() {
            if let Some(branch) = line.strip_prefix("branch ") {
                wt.branch = Some(branch.trim().trim_start_matches("refs/heads/").to_string());
            } else if line == "locked" || line.starts_with("locked ") {
                wt.is_locked = true;
            } else if line == "prunable" || line.starts_with("prunable ") {
                wt.is_prunable = true;
            }
        }
    }
    worktrees
}

/// Turns any remote URL form into `host/owner/repo`.
pub(crate) fn simplify_remote_url(url: &str) -> String {
    let mut rest = url.trim();
    let had_scheme = match rest.split_once("://") {
        Some((_, after)) => {
            rest = after;
            true
        }
        None => false,
    };
    if let Some((user, after)) = rest.split_once('@')
        && !user.contains('/')
    {
        rest = after;
    }
    // scp-like syntax (no scheme): host:owner/repo
    let rest = match rest.split_once(':') {
        Some((host, path)) if !had_scheme && !host.contains('/') => format!("{host}/{path}"),
        _ => rest.to_string(),
    };
    rest.trim_end_matches('/')
        .trim_end_matches(".git")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::mock::MockSystem;
    use std::path::PathBuf;

    fn ref_line(fields: [&str; 7]) -> String {
        fields.join("\x00")
    }

    const AHEAD_BEHIND_ARGS: [&str; 3] = [
        "for-each-ref",
        "--format=%(refname:lstrip=2)%00%(ahead-behind:main)",
        "refs/heads",
    ];

    #[test]
    fn parses_branch_line() {
        let line = ref_line([
            "feature/x",
            " ",
            "abc123",
            "origin/feature/x",
            "ahead 2, behind 1",
            "1700000000",
            "",
        ]);
        let b = parse_branch_line(&line).unwrap();
        assert_eq!(b.name, "feature/x");
        assert!(!b.is_active);
        assert_eq!(b.sha, "abc123");
        assert_eq!(b.upstream.as_deref(), Some("origin/feature/x"));
        assert_eq!((b.upstream_ahead, b.upstream_behind), (2, 1));
        assert!(!b.is_dead);
        assert_eq!(b.last_commit_ts, Some(1_700_000_000));
        assert_eq!(b.worktree_path, None);
    }

    #[test]
    fn parses_gone_and_checked_out_branch() {
        let line = ref_line(["main", "*", "def", "origin/main", "gone", "1", "/repo"]);
        let b = parse_branch_line(&line).unwrap();
        assert!(b.is_active);
        assert!(b.is_dead);
        assert_eq!(b.worktree_path.as_deref(), Some("/repo"));
    }

    #[test]
    fn rejects_malformed_branch_line() {
        assert!(parse_branch_line("garbage").is_none());
    }

    #[test]
    fn parses_track() {
        assert_eq!(parse_track(""), (0, 0, false));
        assert_eq!(parse_track("gone"), (0, 0, true));
        assert_eq!(parse_track("behind 4"), (0, 4, false));
        assert_eq!(parse_track("ahead 1, behind 2"), (1, 2, false));
    }

    #[test]
    fn parses_stash_list() {
        let out = [
            "stash@{0}\x00aaa\x001700000000\x00WIP on main: fix",
            "stash@{1}\x00bbb\x001600000000\x00On dev: tmp",
        ]
        .join("\n");
        let stashes = parse_stash_list(&out);
        assert_eq!(stashes.len(), 2);
        assert_eq!(stashes[1].index, 1);
        assert_eq!(stashes[1].sha, "bbb");
        assert_eq!(stashes[1].created_ts, Some(1_600_000_000));
        assert_eq!(stashes[1].message, "On dev: tmp");
    }

    #[test]
    fn parses_worktree_list() {
        let out = "worktree /repo\nHEAD 1\nbranch refs/heads/main\n\nworktree /repo-wt\nHEAD 2\nbranch refs/heads/feature/a\nlocked\n\nworktree /gone\nHEAD 3\ndetached\nprunable gitdir file points to non-existent location\n";
        let wts = parse_worktree_list(out);
        assert_eq!(wts.len(), 3);
        assert!(wts[0].is_main);
        assert_eq!(wts[1].branch.as_deref(), Some("feature/a"));
        assert!(wts[1].is_locked && !wts[1].is_main);
        assert!(wts[2].is_prunable);
        assert_eq!(wts[2].branch, None);
    }

    #[test]
    fn simplifies_remote_urls() {
        for (input, expected) in [
            ("git@github.com:owner/repo.git", "github.com/owner/repo"),
            ("https://github.com/owner/repo.git", "github.com/owner/repo"),
            (
                "https://user@gitlab.com/group/sub/repo",
                "gitlab.com/group/sub/repo",
            ),
            (
                "ssh://git@bitbucket.org:7999/team/repo.git",
                "bitbucket.org:7999/team/repo",
            ),
            (
                "git@ssh.dev.azure.com:v3/org/proj/repo",
                "ssh.dev.azure.com/v3/org/proj/repo",
            ),
        ] {
            assert_eq!(simplify_remote_url(input), expected, "{input}");
        }
    }

    fn mock_repo() -> (MockSystem, PathBuf) {
        let mut mock = MockSystem::new();
        let path = PathBuf::from("/fake/repo");
        mock.add_command_output(
            &path,
            &["config", "--get", "remote.origin.url"],
            Ok("git@github.com:AymericChaverot/sloth.git\n".into()),
        );
        let refs = [
            ref_line([
                "main",
                "*",
                "m1",
                "origin/main",
                "",
                "1700000000",
                "/fake/repo",
            ]),
            ref_line([
                "feature/test",
                " ",
                "f1",
                "origin/feature/test",
                "gone",
                "1600000000",
                "",
            ]),
            ref_line(["old", " ", "o1", "", "", "1500000000", ""]),
        ]
        .join("\n");
        mock.add_command_output(
            &path,
            &[
                "for-each-ref",
                &format!("--format={REF_FORMAT}"),
                "refs/heads",
            ],
            Ok(refs),
        );
        mock.add_command_output(
            &path,
            &[
                "symbolic-ref",
                "--quiet",
                "--short",
                "refs/remotes/origin/HEAD",
            ],
            Ok("origin/main\n".into()),
        );
        mock.add_command_output(
            &path,
            &AHEAD_BEHIND_ARGS,
            Ok("main\x000 0\nfeature/test\x002 1\nold\x000 5\n".into()),
        );
        mock.add_command_output(
            &path,
            &["diff", "--shortstat", "main...feature/test"],
            Ok(" 3 files changed, 45 insertions(+), 12 deletions(-)\n".into()),
        );
        mock.add_command_output(
            &path,
            &["stash", "list", STASH_FORMAT],
            Ok("stash@{0}\x00s0\x001\x00WIP on main\n".into()),
        );
        mock.add_command_output(
            &path,
            &["worktree", "list", "--porcelain"],
            Ok("worktree /fake/repo\nHEAD 1\nbranch refs/heads/main\n\nworktree /fake/wt1\nHEAD 2\nbranch refs/heads/feature/test\n".into()),
        );
        mock.add_command_output(
            Path::new("/fake/wt1"),
            &["status", "--porcelain"],
            Ok(" M src/lib.rs\n".into()),
        );
        (mock, path)
    }

    #[test]
    fn analyzes_repository() {
        let (mock, path) = mock_repo();
        let status = analyze_repository(&path, &mock).unwrap();

        assert_eq!(
            status.remote_url.as_deref(),
            Some("github.com/AymericChaverot/sloth")
        );
        assert_eq!(status.default_branch.as_deref(), Some("main"));
        assert_eq!(status.current_branch.as_deref(), Some("main"));
        assert!(!status.is_dirty);
        assert_eq!(status.branches.len(), 3);

        let feature = &status.branches[1];
        assert!(feature.is_dead);
        assert_eq!((feature.ahead, feature.behind), (2, 1));
        assert!(!feature.is_merged);
        assert_eq!((feature.diff_insertions, feature.diff_deletions), (45, 12));

        let old = &status.branches[2];
        assert!(old.is_merged);

        assert_eq!(status.stashes.len(), 1);
        assert_eq!(status.stashes[0].sha, "s0");
        assert_eq!(status.worktrees.len(), 2);
        assert!(status.worktrees[0].is_main);
        assert!(status.worktrees[1].is_dirty);
    }

    #[test]
    fn falls_back_to_well_known_default_branch() {
        let (mut mock, path) = mock_repo();
        mock.add_command_output(
            &path,
            &[
                "symbolic-ref",
                "--quiet",
                "--short",
                "refs/remotes/origin/HEAD",
            ],
            Err("not a symbolic ref".into()),
        );
        let status = analyze_repository(&path, &mock).unwrap();
        assert_eq!(status.default_branch.as_deref(), Some("main"));
    }

    #[test]
    fn falls_back_to_rev_list_on_old_git() {
        let (mut mock, path) = mock_repo();
        mock.add_command_output(
            &path,
            &AHEAD_BEHIND_ARGS,
            Err("unknown field name: ahead-behind".into()),
        );
        mock.add_command_output(
            &path,
            &["rev-list", "--left-right", "--count", "main...feature/test"],
            Ok("1\t3\n".into()),
        );
        let status = analyze_repository(&path, &mock).unwrap();
        assert_eq!(
            (status.branches[1].ahead, status.branches[1].behind),
            (3, 1)
        );
    }
}
