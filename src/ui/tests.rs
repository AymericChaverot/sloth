//! Rendering tests: the whole screen is drawn into a test backend and compared
//! with a reviewed snapshot (`cargo insta review` after an intended change).

use crate::config::Config;
use crate::git::models::{BranchInfo, RepoStatus, StashInfo, WorktreeInfo};
use crate::git::stats::now_ts;
use crate::ui::state::{AppState, Focus, Tab};
use ratatui::{Terminal, backend::TestBackend};
use std::path::PathBuf;

const DAY: i64 = 86_400;

fn branch(name: &str, days_old: i64) -> BranchInfo {
    BranchInfo {
        name: name.into(),
        sha: format!("{name}-sha"),
        last_commit_ts: Some(now_ts() - days_old * DAY),
        ..Default::default()
    }
}

fn repo(name: &str) -> RepoStatus {
    let mut repo = RepoStatus::pending(PathBuf::from("/work").join(name));
    repo.analyzed = true;
    repo.size_finalized = true;
    repo.default_branch = Some("main".into());
    repo.current_branch = Some("main".into());
    repo
}

/// A small, deterministic workspace with typical cleanup candidates.
pub fn fixture_state() -> AppState {
    let mut api = repo("api");
    api.remote_url = Some("github.com/acme/api".into());
    api.size_bytes = Some(48 * 1024 * 1024);
    api.untracked_size_bytes = Some(310 * 1024 * 1024);
    api.is_dirty = true;
    api.branches = vec![
        BranchInfo {
            is_active: true,
            worktree_path: Some("/work/api".into()),
            upstream: Some("origin/main".into()),
            ..branch("main", 1)
        },
        BranchInfo {
            is_merged: true,
            upstream: Some("origin/feat/login".into()),
            is_dead: true,
            behind: 12,
            ..branch("feat/login", 95)
        },
        BranchInfo {
            ahead: 3,
            diff_insertions: 120,
            diff_deletions: 4,
            ..branch("wip/cache", 2)
        },
        BranchInfo {
            is_squash_merged: true,
            ahead: 2,
            behind: 30,
            ..branch("fix/typo", 200)
        },
    ];
    api.stashes = vec![StashInfo {
        index: 0,
        sha: "s0".into(),
        message: "WIP on main: try new cache".into(),
        created_ts: Some(now_ts() - 40 * DAY),
    }];
    api.worktrees = vec![
        WorktreeInfo {
            path: "/work/api".into(),
            branch: Some("main".into()),
            is_main: true,
            ..Default::default()
        },
        WorktreeInfo {
            path: "/work/api-hotfix".into(),
            branch: Some("hotfix".into()),
            is_dirty: true,
            size_bytes: Some(12 * 1024 * 1024),
            ..Default::default()
        },
    ];

    let mut web = repo("web");
    web.size_bytes = Some(120 * 1024 * 1024);
    web.untracked_size_bytes = Some(1_200 * 1024 * 1024);
    web.branches = vec![
        branch("main", 3),
        BranchInfo {
            is_merged: true,
            ..branch("chore/deps", 400)
        },
    ];

    let mut state = AppState::new(Config::default(), PathBuf::from("/work"));
    state.repositories = vec![api, web, repo("docs")];
    state.is_scanning = false;
    state.is_analyzing = false;
    state
}

fn render(state: &mut AppState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| crate::ui::draw(f, state)).unwrap();
    terminal.backend().to_string()
}

#[test]
fn repos_tab() {
    let mut state = fixture_state();
    insta::assert_snapshot!(render(&mut state, 120, 20));
}

#[test]
fn repos_tab_details_focused_with_selection() {
    let mut state = fixture_state();
    state.focus = Focus::Details;
    state.detail_index = 1;
    state.selection.insert(
        &PathBuf::from("/work/api"),
        crate::ui::selection::ItemKind::Branch,
        "feat/login",
    );
    insta::assert_snapshot!(render(&mut state, 120, 20));
}

#[test]
fn dashboard_tab() {
    let mut state = fixture_state();
    state.tab = Tab::Dashboard;
    insta::assert_snapshot!(render(&mut state, 100, 20));
}

#[test]
fn help_overlay() {
    let mut state = fixture_state();
    state.show_help = true;
    insta::assert_snapshot!(render(&mut state, 100, 45));
}
