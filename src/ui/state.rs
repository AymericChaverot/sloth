use crate::git::RepoStatus;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug)]
pub enum ScannerEvent {
    RepoFound(PathBuf),
    ScanComplete,
    RepoAnalyzed(RepoStatus),
    AnalysisComplete,
    UpdateAvailable(String),
}

#[derive(PartialEq, Debug)]
pub enum Focus {
    Repositories,
    Details,
    GitGraph,
}

#[derive(Debug, Clone)]
pub enum UiAction {
    CleanRepo,
    PruneRemotes,
    GarbageCollect,
    DeepClean,
}

pub struct AppState {
    pub repositories: Vec<RepoStatus>,
    pub focus: Focus,
    pub repo_index: usize,
    pub detail_index: usize,
    pub repo_state: ListState,
    pub detail_state: ListState,
    pub graph_scroll_y: u16,
    pub graph_scroll_x: u16,
    pub selected_repositories: HashSet<usize>,
    pub selected_branches: HashMap<usize, HashSet<String>>,
    pub selected_stashes: HashMap<usize, HashSet<usize>>,
    pub show_graph: bool,
    pub graph_maximized: bool,
    pub should_quit: bool,
    pub action: Option<UiAction>,
    pub is_scanning: bool,
    pub is_analyzing: bool,
    pub scanned_count: usize,
    pub analyzed_count: usize,
    pub loader_tick: usize,
    pub update_available: Option<String>,
    pub is_updating: bool,
    pub diff_modal_open: bool,
    pub diff_lines: Option<Vec<String>>,
    pub diff_scroll: u16,
    pub theme_index: usize,
    pub is_searching: bool,
    pub search_query: String,
}

impl AppState {
    pub fn new() -> Self {
        let mut repo_state = ListState::default();
        repo_state.select(Some(0));
        let mut detail_state = ListState::default();
        detail_state.select(Some(0));

        Self {
            repositories: Vec::new(),
            focus: Focus::Repositories,
            repo_index: 0,
            detail_index: 0,
            repo_state,
            detail_state,
            graph_scroll_y: 0,
            graph_scroll_x: 0,
            selected_repositories: HashSet::new(),
            selected_branches: HashMap::new(),
            selected_stashes: HashMap::new(),
            show_graph: false,
            graph_maximized: false,
            should_quit: false,
            action: None,
            is_scanning: true,
            is_analyzing: true,
            scanned_count: 0,
            analyzed_count: 0,
            loader_tick: 0,
            update_available: None,
            is_updating: false,
            diff_modal_open: false,
            diff_lines: None,
            diff_scroll: 0,
            theme_index: crate::ui::theme::load_saved_theme(),
            is_searching: false,
            search_query: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_initialization() {
        let state = AppState::new();
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
