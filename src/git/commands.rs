use super::models::GitError;
use std::path::Path;

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
