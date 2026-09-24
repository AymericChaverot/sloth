use crate::git::RepoStatus;
use ratatui::widgets::TableState;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug)]
pub enum ScannerEvent {
    RepoFound(PathBuf),
    ScanComplete,
    RepoAnalyzed(RepoStatus),
    RepoFailed {
        path: PathBuf,
        error: String,
    },
    /// Running estimate while untracked files are being measured.
    SizePartial {
        path: PathBuf,
        size_bytes: Option<u64>,
        untracked_size_bytes: u64,
    },
    SizeComputed {
        path: PathBuf,
        size_bytes: Option<u64>,
        untracked_size_bytes: Option<u64>,
        worktree_sizes: HashMap<String, Option<u64>>,
    },
    AnalysisComplete,
    UpdateAvailable(String),
    DeepCleanPreview(Vec<String>),
    GraphLoaded {
        path: PathBuf,
        lines: Vec<String>,
    },
    /// `id` matches `AppState::diff_request_id` when still relevant.
    DiffLoaded {
        id: u64,
        lines: Vec<String>,
    },
    /// One operation of a running cleanup finished.
    OperationDone(crate::engine::OpResult),
    ExecutionFinished,
}

/// Aggregates over every analyzed repository.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Totals {
    pub git_bytes: u64,
    pub untracked_bytes: u64,
    pub branches: usize,
    pub merged_branches: usize,
    pub gone_branches: usize,
    /// Branches that smart selection would pick.
    pub cleanable_branches: usize,
    pub stashes: usize,
    pub linked_worktrees: usize,
}

/// Progress and results of a cleanup run.
#[derive(Debug, Default)]
pub struct Execution {
    pub total: usize,
    pub results: Vec<crate::engine::OpResult>,
    pub finished: bool,
    pub scroll: u16,
}

impl Execution {
    pub fn failed(&self) -> usize {
        self.results.iter().filter(|r| !r.is_ok()).count()
    }

    pub fn freed_bytes(&self) -> u64 {
        self.results.iter().map(|r| r.freed_bytes).sum()
    }

    /// Whether anything restorable with `sloth restore` was deleted.
    pub fn restorable(&self) -> bool {
        self.results
            .iter()
            .any(|r| crate::journal::Entry::from_result(0, r).is_some())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Repos,
    Branches,
    Queue,
    Dashboard,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Repos, Tab::Branches, Tab::Queue, Tab::Dashboard];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Repos => "Repos",
            Tab::Branches => "Branches",
            Tab::Queue => "Queue",
            Tab::Dashboard => "Dashboard",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn next(self) -> Tab {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Tab {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Focused pane of the Repos tab.
#[derive(PartialEq, Debug)]
pub enum Focus {
    Repositories,
    Details,
    GitGraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Warning,
}

/// A short message shown in the status bar for a few seconds.
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub shown_at: std::time::Instant,
}

const TOAST_DURATION: std::time::Duration = std::time::Duration::from_secs(4);

#[derive(Debug, Clone)]
pub enum UiAction {
    CleanRepo,
    PruneRemotes,
    GarbageCollect,
    DeepClean,
}

/// Where things were drawn in the last frame, to map mouse clicks to them.
#[derive(Debug, Default, Clone)]
pub struct LayoutCache {
    pub tabs: Vec<(ratatui::layout::Rect, Tab)>,
    pub repo_table: ratatui::layout::Rect,
    pub detail_table: ratatui::layout::Rect,
    pub branch_table: ratatui::layout::Rect,
    pub queue_table: ratatui::layout::Rect,
    pub dashboard_table: ratatui::layout::Rect,
}

/// Re-analysis requested by the user, carried out by the main loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refresh {
    Repos(Vec<PathBuf>),
    /// Discover repositories again from scratch.
    Rescan,
}

/// Sort order of the repository list, cycled with `s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoSort {
    Path,
    Cleanable,
    Reclaimable,
    GitSize,
}

impl RepoSort {
    pub fn label(self) -> &'static str {
        match self {
            RepoSort::Path => "path",
            RepoSort::Cleanable => "cleanable branches",
            RepoSort::Reclaimable => "reclaimable space",
            RepoSort::GitSize => ".git size",
        }
    }

    pub fn next(self) -> RepoSort {
        match self {
            RepoSort::Path => RepoSort::Cleanable,
            RepoSort::Cleanable => RepoSort::Reclaimable,
            RepoSort::Reclaimable => RepoSort::GitSize,
            RepoSort::GitSize => RepoSort::Path,
        }
    }
}

/// Which branches the Branches tab lists, cycled with `f`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchFilter {
    /// Merged or gone, and not protected: what smart selection picks.
    Cleanable,
    Merged,
    Gone,
    Stale,
    /// Not merged and not pushed: deleting them loses work.
    Unmerged,
    All,
}

impl BranchFilter {
    pub const ALL: [BranchFilter; 6] = [
        BranchFilter::Cleanable,
        BranchFilter::Merged,
        BranchFilter::Gone,
        BranchFilter::Stale,
        BranchFilter::Unmerged,
        BranchFilter::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BranchFilter::Cleanable => "cleanable",
            BranchFilter::Merged => "merged",
            BranchFilter::Gone => "gone",
            BranchFilter::Stale => "stale",
            BranchFilter::Unmerged => "unmerged",
            BranchFilter::All => "all",
        }
    }

    pub fn next(self) -> BranchFilter {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// Sort order of the Branches tab, cycled with `s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchSort {
    Repository,
    Oldest,
    Name,
}

impl BranchSort {
    pub fn label(self) -> &'static str {
        match self {
            BranchSort::Repository => "repository",
            BranchSort::Oldest => "oldest first",
            BranchSort::Name => "name",
        }
    }

    pub fn next(self) -> BranchSort {
        match self {
            BranchSort::Repository => BranchSort::Oldest,
            BranchSort::Oldest => BranchSort::Name,
            BranchSort::Name => BranchSort::Repository,
        }
    }
}

pub struct AppState {
    pub repositories: Vec<RepoStatus>,
    pub tab: Tab,
    pub show_help: bool,
    pub toast: Option<Toast>,
    pub focus: Focus,
    /// Repository under the cursor, tracked by path so sorting, filtering
    /// and refreshes never move the cursor to another repository.
    pub focused_repo: Option<PathBuf>,
    /// Repositories targeted by maintenance actions and bulk smart selection.
    pub marked_repos: BTreeSet<PathBuf>,
    pub repo_sort: RepoSort,
    /// Case-insensitive filter on repository paths and remotes.
    pub repo_filter: String,
    pub editing_filter: bool,
    pub repo_table: TableState,
    pub detail_index: usize,
    pub detail_table: TableState,
    pub branch_filter: BranchFilter,
    pub branch_sort: BranchSort,
    /// Case-insensitive filter on repository and branch names.
    pub branch_query: String,
    pub branch_cursor: usize,
    pub branch_table: TableState,
    pub queue_cursor: usize,
    pub queue_table: TableState,
    pub dashboard_cursor: usize,
    pub dashboard_table: TableState,
    pub graph_scroll_y: u16,
    pub graph_scroll_x: u16,
    /// Cleanup queue: items selected in any repository.
    pub selection: crate::ui::selection::Selection,
    pub show_graph: bool,
    pub graph_maximized: bool,
    pub should_quit: bool,
    pub action: Option<UiAction>,
    pub refresh: Option<Refresh>,
    pub layout: LayoutCache,
    pub is_scanning: bool,
    pub is_analyzing: bool,
    /// Drives spinner animations.
    pub started: std::time::Instant,
    /// Repositories whose graph is being loaded.
    pub graph_loading: HashSet<PathBuf>,
    pub update_available: Option<String>,
    pub is_updating: bool,
    /// What the open diff shows; `None` when the diff modal is closed.
    pub diff_target: Option<(PathBuf, crate::ui::loader::DiffTarget)>,
    pub diff_lines: Option<Vec<String>>,
    pub diff_scroll: u16,
    pub diff_request_id: u64,
    pub diff_requested: bool,
    pub theme_index: usize,
    pub pending_action: Option<UiAction>,
    pub confirm_preview_lines: Option<Vec<String>>,
    pub preview_loading: bool,
    pub config: crate::config::Config,
    /// Directory that was scanned.
    pub root: PathBuf,
    /// Cleanup running (or just finished) inside the TUI.
    pub execution: Option<Execution>,
    /// Where deletions are recorded for `sloth restore` (none in tests).
    pub journal: Option<crate::journal::Journal>,
}

impl AppState {
    pub fn new(config: crate::config::Config, root: PathBuf) -> Self {
        Self {
            repositories: Vec::new(),
            tab: Tab::Repos,
            show_help: false,
            toast: None,
            focus: Focus::Repositories,
            focused_repo: None,
            marked_repos: BTreeSet::new(),
            repo_sort: RepoSort::Path,
            repo_filter: String::new(),
            editing_filter: false,
            repo_table: TableState::default(),
            detail_index: 0,
            detail_table: TableState::default(),
            branch_filter: BranchFilter::Cleanable,
            branch_sort: BranchSort::Repository,
            branch_query: String::new(),
            branch_cursor: 0,
            branch_table: TableState::default(),
            queue_cursor: 0,
            queue_table: TableState::default(),
            dashboard_cursor: 0,
            dashboard_table: TableState::default(),
            graph_scroll_y: 0,
            graph_scroll_x: 0,
            selection: Default::default(),
            show_graph: false,
            graph_maximized: false,
            should_quit: false,
            action: None,
            refresh: None,
            layout: LayoutCache::default(),
            is_scanning: true,
            is_analyzing: true,
            started: std::time::Instant::now(),
            graph_loading: HashSet::new(),
            update_available: None,
            is_updating: false,
            diff_target: None,
            diff_lines: None,
            diff_scroll: 0,
            diff_request_id: 0,
            diff_requested: false,
            theme_index: crate::ui::theme::index_by_name(config.theme.as_deref()),
            config,
            root,
            execution: None,
            journal: None,
            pending_action: None,
            confirm_preview_lines: None,
            preview_loading: false,
        }
    }

    /// The repository under the cursor (the first visible one by default).
    pub fn focused(&self) -> Option<&RepoStatus> {
        let visible = crate::ui::views::visible_repos(self);
        let index = self
            .focused_repo
            .as_ref()
            .and_then(|path| {
                visible
                    .iter()
                    .copied()
                    .find(|&i| &self.repositories[i].path == path)
            })
            .or_else(|| visible.first().copied())?;
        self.repositories.get(index)
    }

    /// Moves the repository cursor by `delta` rows in the visible list.
    pub fn move_repo_cursor(&mut self, delta: isize) {
        let visible = crate::ui::views::visible_repos(self);
        if visible.is_empty() {
            return;
        }
        let current = self
            .focused()
            .and_then(|f| {
                visible
                    .iter()
                    .position(|&i| self.repositories[i].path == f.path)
            })
            .unwrap_or(0);
        let next = current.saturating_add_signed(delta).min(visible.len() - 1);
        self.focus_repo(self.repositories[visible[next]].path.clone());
    }

    pub fn focus_repo(&mut self, path: PathBuf) {
        if self.focused_repo.as_ref() != Some(&path) {
            self.focused_repo = Some(path);
            self.detail_index = 0;
            self.graph_scroll_y = 0;
            self.graph_scroll_x = 0;
        }
    }

    /// Repositories a maintenance action applies to: the marked ones, or the focused one.
    pub fn action_targets(&self) -> Vec<PathBuf> {
        if self.marked_repos.is_empty() {
            self.focused().map(|r| r.path.clone()).into_iter().collect()
        } else {
            self.repositories
                .iter()
                .filter(|r| self.marked_repos.contains(&r.path))
                .map(|r| r.path.clone())
                .collect()
        }
    }

    pub fn repo(&self, path: &std::path::Path) -> Option<&RepoStatus> {
        self.repositories.iter().find(|r| r.path == path)
    }

    pub fn open_diff(&mut self, repo: PathBuf, target: crate::ui::loader::DiffTarget) {
        self.diff_target = Some((repo, target));
        self.diff_lines = None;
        self.diff_requested = false;
        self.diff_scroll = 0;
    }

    pub fn totals(&self) -> Totals {
        let mut t = Totals::default();
        for repo in &self.repositories {
            t.git_bytes += repo.size_bytes.unwrap_or(0);
            t.untracked_bytes += repo.untracked_size_bytes.unwrap_or(0);
            t.branches += repo.branches.len();
            t.stashes += repo.stashes.len();
            t.linked_worktrees += repo.worktrees.iter().filter(|w| !w.is_main).count();
            for branch in &repo.branches {
                t.merged_branches += usize::from(branch.is_fully_merged());
                t.gone_branches += usize::from(branch.is_dead);
                t.cleanable_branches += usize::from(crate::cleanup::is_smart_candidate(
                    repo,
                    branch,
                    &self.config,
                ));
            }
        }
        t
    }

    pub fn notify(&mut self, message: impl Into<String>) {
        self.show_toast(message, ToastLevel::Info);
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.show_toast(message, ToastLevel::Warning);
    }

    fn show_toast(&mut self, message: impl Into<String>, level: ToastLevel) {
        self.toast = Some(Toast {
            message: message.into(),
            level,
            shown_at: std::time::Instant::now(),
        });
    }

    /// Drops an expired toast; returns whether the screen must be redrawn.
    pub fn expire_toast(&mut self) -> bool {
        let expired = self
            .toast
            .as_ref()
            .is_some_and(|t| t.shown_at.elapsed() >= TOAST_DURATION);
        if expired {
            self.toast = None;
        }
        expired
    }

    /// Path relative to the scanned directory (the directory name for the root itself).
    pub fn display_path(&self, path: &std::path::Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
            _ => path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string()),
        }
    }

    /// Current spinner frame (time based, independent of the redraw rate).
    pub fn spinner(&self) -> &'static str {
        let frame = self.started.elapsed().as_millis() / 100;
        crate::ui::SPINNER[frame as usize % crate::ui::SPINNER.len()]
    }

    /// Whether something on screen animates, so the UI must keep redrawing.
    pub fn is_animating(&self) -> bool {
        self.is_scanning
            || self.is_analyzing
            || self.preview_loading
            || self.execution.as_ref().is_some_and(|e| !e.finished)
            || !self.graph_loading.is_empty()
            || (self.diff_target.is_some() && self.diff_lines.is_none())
            || self
                .repositories
                .iter()
                .any(|r| r.analyzed && r.error.is_none() && !r.size_finalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(paths: &[&str]) -> AppState {
        let mut state = AppState::new(crate::config::Config::default(), PathBuf::from("/w"));
        state.repositories = paths
            .iter()
            .map(|p| RepoStatus::pending(PathBuf::from(p)))
            .collect();
        state
    }

    #[test]
    fn focuses_first_repository_by_default() {
        let state = state_with(&["/w/b", "/w/a"]);
        assert_eq!(state.focused().unwrap().path, PathBuf::from("/w/a"));
    }

    #[test]
    fn cursor_follows_the_repository_not_the_index() {
        let mut state = state_with(&["/w/b", "/w/c"]);
        state.move_repo_cursor(1);
        assert_eq!(state.focused().unwrap().path, PathBuf::from("/w/c"));
        // A repository discovered later sorts before the focused one.
        state
            .repositories
            .push(RepoStatus::pending(PathBuf::from("/w/a")));
        assert_eq!(state.focused().unwrap().path, PathBuf::from("/w/c"));
    }

    #[test]
    fn cursor_skips_filtered_out_repositories() {
        let mut state = state_with(&["/w/api", "/w/docs", "/w/web-api"]);
        state.repo_filter = "api".into();
        state.move_repo_cursor(1);
        assert_eq!(state.focused().unwrap().path, PathBuf::from("/w/web-api"));
        state.move_repo_cursor(5);
        assert_eq!(state.focused().unwrap().path, PathBuf::from("/w/web-api"));
    }

    #[test]
    fn actions_target_marked_repositories_or_the_focused_one() {
        let mut state = state_with(&["/w/a", "/w/b"]);
        assert_eq!(state.action_targets(), vec![PathBuf::from("/w/a")]);
        state.marked_repos.insert(PathBuf::from("/w/b"));
        assert_eq!(state.action_targets(), vec![PathBuf::from("/w/b")]);
    }
}
