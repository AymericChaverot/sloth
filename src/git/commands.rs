use std::path::Path;

/// Commits shown in the graph: enough to see every recent branch, bounded so
/// that huge histories stay fast.
const GRAPH_MAX_COMMITS: &str = "2000";

/// Colored `git log --graph` of all local and remote branches.
pub fn get_git_graph(path: &Path, sys: &impl crate::sys::GitExecutor) -> Vec<String> {
    lines_of(sys.run_git_command(
        path,
        &[
            "log",
            "--graph",
            "--all",
            "--color=always",
            "-n",
            GRAPH_MAX_COMMITS,
            "--pretty=format:%C(yellow)%h%Creset -%C(auto)%d%Creset %s %C(dim white)(%ar) <%an>%Creset",
        ],
    ))
}

/// Colored diff for a revision range such as `main...feature`.
pub fn get_branch_diff(
    path: &Path,
    range: &str,
    sys: &impl crate::sys::GitExecutor,
) -> Vec<String> {
    lines_of(sys.run_git_command(path, &["diff", "--color=always", range]))
}

/// Colored patch of a stash, identified by its commit id.
pub fn get_stash_diff(path: &Path, stash: &str, sys: &impl crate::sys::GitExecutor) -> Vec<String> {
    lines_of(sys.run_git_command(path, &["stash", "show", "-p", "--color=always", stash]))
}

fn lines_of(output: std::io::Result<String>) -> Vec<String> {
    output
        .map(|out| out.lines().map(str::to_string).collect())
        .unwrap_or_default()
}
