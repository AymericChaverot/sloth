use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Failed to open repository: {0}")]
    OpenError(#[from] gix::open::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchInfo {
    pub name: String,
    pub is_active: bool,
    pub is_dead: bool,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub diff_insertions: usize,
    pub diff_deletions: usize,
    pub last_commit_date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StashInfo {
    pub index: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub path: PathBuf,
    pub remote_url: Option<String>,
    pub branches: Vec<BranchInfo>,
    pub stashes: Vec<StashInfo>,
    pub graph_lines: Option<Vec<String>>,
    pub analyzed: bool,
}
