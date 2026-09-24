//! Key bindings shown in the status bar and the help overlay.

use crate::ui::state::{AppState, Focus, Tab};

pub type Binding = (&'static str, &'static str);

pub const GLOBAL: &[Binding] = &[
    ("1-2 / Tab", "switch tab"),
    ("?", "toggle this help"),
    ("x / Enter", "review & run the cleanup queue"),
    ("C", "clear the queue"),
    ("t", "cycle theme"),
    ("u", "install available update"),
    ("q", "quit"),
];

pub const REPOS: &[Binding] = &[
    ("↑↓", "move"),
    ("Space", "mark repository"),
    ("→ / l", "open details"),
    ("a", "smart-select merged & gone branches (marked repos)"),
    ("p", "prune remote-tracking branches"),
    ("c", "garbage collect"),
    ("X", "deep clean untracked & ignored files"),
    ("g", "toggle graph"),
    ("/", "filter"),
];

pub const DETAILS: &[Binding] = &[
    ("↑↓", "move"),
    ("Space", "add to / remove from queue"),
    ("a", "smart-select merged & gone branches"),
    ("A", "select every unprotected branch"),
    ("v", "view diff"),
    ("Esc", "clear this repository's selection"),
    ("← / h", "back to repositories"),
];

pub const GRAPH: &[Binding] = &[("↑↓←→", "scroll"), ("f", "fullscreen"), ("Esc / g", "back")];

pub const DASHBOARD: &[Binding] = &[("1", "back to repositories")];

pub const SECTIONS: &[(&str, &[Binding])] = &[
    ("Global", GLOBAL),
    ("Repositories", REPOS),
    ("Details", DETAILS),
    ("Graph", GRAPH),
    ("Dashboard", DASHBOARD),
];

/// The few bindings worth showing in the status bar for the current context.
pub fn hints(state: &AppState) -> &'static [Binding] {
    match state.tab {
        Tab::Repos => match state.focus {
            Focus::Repositories => &[
                ("Space", "mark"),
                ("→", "details"),
                ("a", "smart select"),
                ("p/c/X", "prune/gc/deep clean"),
                ("/", "filter"),
                ("?", "help"),
            ],
            Focus::Details => &[
                ("Space", "select"),
                ("a", "smart"),
                ("v", "diff"),
                ("x", "run queue"),
                ("←", "back"),
                ("?", "help"),
            ],
            Focus::GitGraph => &[("↑↓←→", "scroll"), ("f", "fullscreen"), ("Esc", "back")],
        },
        Tab::Dashboard => &[("1", "repos"), ("?", "help"), ("q", "quit")],
    }
}
