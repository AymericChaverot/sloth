use crate::git::RepoStatus;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
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
    Dashboard,
}

impl Tab {
    pub const ALL: [Tab; 2] = [Tab::Repos, Tab::Dashboard];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Repos => "Repos",
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

pub struct AppState {
    pub repositories: Vec<RepoStatus>,
    pub tab: Tab,
    pub show_help: bool,
    pub toast: Option<Toast>,
    pub focus: Focus,
    pub repo_index: usize,
    pub detail_index: usize,
    pub repo_state: ListState,
    pub detail_state: ListState,
    pub graph_scroll_y: u16,
    pub graph_scroll_x: u16,
    pub selected_repositories: HashSet<usize>,
    /// Cleanup queue: items selected in any repository.
    pub selection: crate::ui::selection::Selection,
    pub show_graph: bool,
    pub graph_maximized: bool,
    pub should_quit: bool,
    pub action: Option<UiAction>,
    pub is_scanning: bool,
    pub is_analyzing: bool,
    pub scanned_count: usize,
    pub analyzed_count: usize,
    /// Drives spinner animations.
    pub started: std::time::Instant,
    /// Repositories whose graph is being loaded.
    pub graph_loading: HashSet<PathBuf>,
    pub update_available: Option<String>,
    pub is_updating: bool,
    pub diff_modal_open: bool,
    pub diff_lines: Option<Vec<String>>,
    pub diff_scroll: u16,
    pub diff_request_id: u64,
    pub diff_requested: bool,
    pub theme_index: usize,
    pub is_searching: bool,
    pub search_query: String,
    pub pending_action: Option<UiAction>,
    pub confirm_preview_lines: Option<Vec<String>>,
    pub preview_loading: bool,
    pub config: crate::config::Config,
    /// Directory that was scanned.
    pub root: PathBuf,
    /// Cleanup running (or just finished) inside the TUI.
    pub execution: Option<Execution>,
}

impl AppState {
    pub fn new(config: crate::config::Config, root: PathBuf) -> Self {
        let mut repo_state = ListState::default();
        repo_state.select(Some(0));
        let mut detail_state = ListState::default();
        detail_state.select(Some(0));

        Self {
            repositories: Vec::new(),
            tab: Tab::Repos,
            show_help: false,
            toast: None,
            focus: Focus::Repositories,
            repo_index: 0,
            detail_index: 0,
            repo_state,
            detail_state,
            graph_scroll_y: 0,
            graph_scroll_x: 0,
            selected_repositories: HashSet::new(),
            selection: Default::default(),
            show_graph: false,
            graph_maximized: false,
            should_quit: false,
            action: None,
            is_scanning: true,
            is_analyzing: true,
            scanned_count: 0,
            analyzed_count: 0,
            started: std::time::Instant::now(),
            graph_loading: HashSet::new(),
            update_available: None,
            is_updating: false,
            diff_modal_open: false,
            diff_lines: None,
            diff_scroll: 0,
            diff_request_id: 0,
            diff_requested: false,
            theme_index: crate::ui::theme::index_by_name(config.theme.as_deref()),
            config,
            root,
            execution: None,
            is_searching: false,
            search_query: String::new(),
            pending_action: None,
            confirm_preview_lines: None,
            preview_loading: false,
        }
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
            || (self.diff_modal_open && self.diff_lines.is_none())
            || self
                .repositories
                .iter()
                .any(|r| r.analyzed && r.error.is_none() && !r.size_finalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_initialization() {
        let state = AppState::new(crate::config::Config::default(), PathBuf::from("."));
        assert_eq!(state.focus, Focus::Repositories);
        assert_eq!(state.repo_index, 0);
        assert_eq!(state.detail_index, 0);
        assert!(!state.show_graph);
        assert!(!state.graph_maximized);
        assert!(state.is_scanning);
        assert!(state.is_analyzing);
        assert_eq!(state.scanned_count, 0);
        assert_eq!(state.analyzed_count, 0);
        assert!(state.update_available.is_none());
        assert!(!state.is_updating);
    }
}
