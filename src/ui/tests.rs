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
    // Keep snapshots stable across version bumps.
    terminal
        .backend()
        .to_string()
        .replace(concat!("v", env!("CARGO_PKG_VERSION")), "vX.Y.Z")
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

fn press(state: &mut AppState, keys: &str) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    for c in keys.chars() {
        let code = match c {
            '\n' => KeyCode::Enter,
            '\x1b' => KeyCode::Esc,
            '↓' => KeyCode::Down,
            '→' => KeyCode::Right,
            c => KeyCode::Char(c),
        };
        crate::ui::events::handle_key(state, KeyEvent::new(code, KeyModifiers::NONE));
    }
}

#[test]
fn filtered_navigation_acts_on_the_visible_repository() {
    let mut state = fixture_state();
    // Filter to "web", then mark and open it: the hidden "api" is untouched.
    press(&mut state, "/we\n ");
    assert_eq!(
        state.marked_repos.iter().collect::<Vec<_>>(),
        vec![&PathBuf::from("/work/web")]
    );
    press(&mut state, "→a");
    assert!(state.selection.contains(
        &PathBuf::from("/work/web"),
        crate::ui::selection::ItemKind::Branch,
        "chore/deps"
    ));
    assert!(state.selection.repo(&PathBuf::from("/work/api")).is_none());
}

#[test]
fn protected_items_cannot_be_selected() {
    let mut state = fixture_state();
    press(&mut state, "→ "); // cursor on `main`, the default branch
    assert!(state.selection.is_empty());
    assert!(state.toast.is_some());
}

#[test]
fn branches_tab_lists_cleanable_branches_of_every_repository() {
    let mut state = fixture_state();
    state.tab = Tab::Branches;
    insta::assert_snapshot!(render(&mut state, 120, 12));
}

#[test]
fn branches_tab_all_filter() {
    let mut state = fixture_state();
    state.tab = Tab::Branches;
    press(&mut state, "fffff"); // cleanable → merged → gone → stale → unmerged → all
    insta::assert_snapshot!(render(&mut state, 120, 14));
}

#[test]
fn branches_tab_selects_across_repositories() {
    let mut state = fixture_state();
    state.tab = Tab::Branches;
    press(&mut state, "a");
    assert_eq!(state.selection.summary(), "3 branches in 2 repos");
}

#[test]
fn queue_tab_reviews_items_of_several_repositories() {
    let mut state = fixture_state();
    state.tab = Tab::Branches;
    press(&mut state, "a"); // 3 cleanable branches in api and web
    state.tab = Tab::Repos;
    press(&mut state, "→↓↓ "); // + wip/cache, which has unpushed work
    state.tab = Tab::Queue;
    insta::assert_snapshot!(render(&mut state, 120, 12));

    press(&mut state, " "); // remove the first row
    assert_eq!(state.selection.summary(), "3 branches in 2 repos");
}

#[test]
fn confirmation_lists_the_queue_with_warnings() {
    let mut state = fixture_state();
    state.tab = Tab::Branches;
    press(&mut state, "fffff"); // all branches
    press(&mut state, "A"); // every unprotected branch
    press(&mut state, "x");
    insta::assert_snapshot!(render(&mut state, 100, 24));
}

#[test]
fn empty_queue_explains_how_to_fill_it() {
    let mut state = fixture_state();
    state.tab = Tab::Queue;
    insta::assert_snapshot!(render(&mut state, 100, 12));
}

fn click(state: &mut AppState, column: u16, row: u16) {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    crate::ui::events::handle_mouse(
        state,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        },
    );
}

#[test]
fn mouse_switches_tabs_and_toggles_rows() {
    let mut state = fixture_state();
    render(&mut state, 120, 20);
    let (tab_area, _) = state.layout.tabs[1];
    click(&mut state, tab_area.x + 2, tab_area.y);
    assert_eq!(state.tab, Tab::Branches);

    render(&mut state, 120, 20);
    let table = state.layout.branch_table;
    click(&mut state, table.x + 2, table.y + 3); // mark column, 2nd row
    assert!(state.selection.contains(
        &PathBuf::from("/work/api"),
        crate::ui::selection::ItemKind::Branch,
        "fix/typo"
    ));
    assert_eq!(state.branch_cursor, 1);
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
    insta::assert_snapshot!(render(&mut state, 120, 40));
}

/// The real pipeline (scan, analysis, sizes) on real repositories, then a
/// full cleanup through the UI state: every tab must render without panicking.
#[tokio::test(flavor = "multi_thread")]
async fn end_to_end_on_real_repositories() {
    use std::time::{Duration, Instant};

    let workspace = crate::test_support::TempRepo::new("e2e-ws");
    for name in ["alpha", "beta"] {
        let repo = crate::test_support::TempRepo::at(workspace.path.join(name));
        repo.commit("a.txt", "a\n");
        repo.git(&["checkout", "-q", "-b", "feat/done"]);
        repo.commit("b.txt", "b\n");
        repo.git(&["checkout", "-q", "main"]);
        repo.git(&["merge", "-q", "--no-ff", "feat/done", "-m", "merge"]);
        std::fs::write(repo.path.join("build.log"), "x".repeat(4096)).unwrap();
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let worker = crate::worker::Worker::new(tx.clone(), Vec::new());
    let mut state = AppState::new(Config::default(), workspace.path.clone());
    worker.scan(workspace.path.clone(), Vec::new());

    // Pump events until the pipeline has settled.
    let pump = |state: &mut AppState, until: &dyn Fn(&AppState) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !until(state) {
            assert!(Instant::now() < deadline, "pipeline did not settle");
            while let Ok(event) = rx.try_recv() {
                crate::ui::apply_event(state, event, &worker);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let settled = |s: &AppState| {
        !s.is_scanning
            && !s.is_analyzing
            && s.repositories.len() == 3
            && s.repositories
                .iter()
                .all(|r| r.analyzed && r.size_finalized)
    };
    pump(&mut state, &settled);

    // The workspace itself is a (empty) repository too.
    assert_eq!(state.totals().cleanable_branches, 2);
    for tab in Tab::ALL {
        state.tab = tab;
        render(&mut state, 120, 30);
    }

    // Queue everything cleanable and run it inside the UI.
    state.tab = Tab::Dashboard;
    press(&mut state, "ax");
    assert!(state.pending_action.is_some());
    press(&mut state, "y");
    let action = state.action.take().expect("confirmed");
    crate::ui::start_execution(&mut state, action, &tx);
    pump(&mut state, &|s: &AppState| {
        s.execution.as_ref().is_some_and(|e| e.finished) && settled(s)
    });

    let execution = state.execution.as_ref().unwrap();
    assert_eq!(execution.results.len(), 2);
    assert_eq!(execution.failed(), 0);
    assert!(state.selection.is_empty());
    assert_eq!(state.totals().cleanable_branches, 0);
    render(&mut state, 120, 30);
}

#[test]
fn graph_pane_and_diff_modal() {
    let mut state = fixture_state();
    state.repositories[0].graph_lines = Some(vec![
        "* \u{1b}[33mabc1234\u{1b}[m - (HEAD -> main) merge feat/login (2 days ago) <dev>".into(),
        "|\\  ".into(),
        "| * \u{1b}[33mdef5678\u{1b}[m - login form (3 weeks ago) <dev>".into(),
        "|/  ".into(),
        "* \u{1b}[33m0123abc\u{1b}[m - init (1 year ago) <dev>".into(),
    ]);
    state.show_graph = true;
    insta::assert_snapshot!("graph_pane", render(&mut state, 120, 20));

    press(&mut state, "→↓v");
    state.diff_lines = Some(vec![
        "diff --git a/login.rs b/login.rs".into(),
        "\u{1b}[32m+fn login() {}\u{1b}[m".into(),
    ]);
    insta::assert_snapshot!("diff_modal", render(&mut state, 100, 14));
}

/// Sloth only prints ASCII and plain Unicode symbols: no emoji, which render
/// inconsistently (width, color) across terminals and fonts.
#[test]
fn sources_contain_no_emoji() {
    fn is_emoji(c: char) -> bool {
        // Symbol blocks where most characters have an emoji presentation,
        // minus the plain check marks used for results.
        !matches!(c, '✓' | '✗')
            && matches!(c as u32, 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0x1F000..=0x1FFFF | 0xFE0F)
    }
    fn visit(dir: &std::path::Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, found);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for (n, line) in text.lines().enumerate() {
                    if line.chars().any(is_emoji) {
                        found.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                    }
                }
            }
        }
    }
    let mut found = Vec::new();
    visit(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    assert!(found.is_empty(), "emoji found:\n{}", found.join("\n"));
}

#[test]
fn clean_and_quit_are_always_visible() {
    for tab in Tab::ALL {
        let mut state = fixture_state();
        state.tab = tab;
        state.warn("a long notification that takes a lot of room in the status bar");
        let screen = render(&mut state, 60, 12);
        let status = screen.lines().last().unwrap();
        assert!(
            status.starts_with("\" x clean  q quit │"),
            "{tab:?}: {status}"
        );
    }
}

#[test]
fn branch_search_is_echoed_in_the_status_bar() {
    let mut state = fixture_state();
    state.tab = Tab::Branches;
    press(&mut state, "/log");
    let screen = render(&mut state, 100, 10);
    assert!(screen.lines().last().unwrap().contains("Filter: log"));
    assert!(state.repo_filter.is_empty());
}
