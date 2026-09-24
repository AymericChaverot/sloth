//! Key bindings shown in the status bar and the help overlay.

use crate::ui::state::{AppState, Focus, Tab};

pub type Binding = (&'static str, &'static str);

pub const GLOBAL: &[Binding] = &[
    ("1-9 / Tab", "switch tab"),
    ("?", "toggle this help"),
    ("x / Enter", "review & run the cleanup queue"),
    ("C", "clear the queue"),
    ("r", "refresh the marked (or focused) repositories"),
    ("R", "rescan the directory"),
    ("t", "cycle theme"),
    ("u", "install available update"),
    ("q", "quit"),
];

pub const REPOS: &[Binding] = &[
    ("↑↓ PgUp/Dn", "move"),
    ("Space", "mark repository (targets of a/p/c/X)"),
    ("→ / Enter", "open details"),
    ("a", "smart-select merged & gone branches"),
    ("p", "prune remote-tracking branches"),
    ("c", "garbage collect"),
    ("X", "deep clean untracked & ignored files"),
    ("/", "filter by path or remote"),
    ("s", "cycle sort (name, cleanable, reclaimable, .git)"),
    ("Esc", "clear filter, then marks"),
    ("g", "toggle graph"),
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

pub const BRANCHES: &[Binding] = &[
    ("↑↓ PgUp/Dn", "move"),
    ("Space", "add to / remove from queue"),
    ("a", "add every listed cleanable branch"),
    ("A", "add every listed unprotected branch"),
    (
        "f",
        "cycle filter: cleanable, merged, gone, stale, unmerged, all",
    ),
    ("s", "cycle sort: repository, oldest, name"),
    ("/", "search repository or branch"),
    ("v", "view diff"),
    ("Enter", "open the branch in its repository"),
];

pub const QUEUE: &[Binding] = &[
    ("↑↓", "move"),
    ("Space / d", "remove from the queue"),
    ("Enter / x", "review & run everything"),
    ("C", "clear the queue"),
];

pub const DASHBOARD: &[Binding] = &[
    ("a", "queue every merged & gone branch of every repository"),
    ("Enter", "open the repository"),
];

pub const SECTIONS: &[(&str, &[Binding])] = &[
    ("Global", GLOBAL),
    ("Repositories", REPOS),
    ("Details", DETAILS),
    ("Graph", GRAPH),
    ("Branches (all repositories)", BRANCHES),
    ("Queue", QUEUE),
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
                ("s", "sort"),
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
        Tab::Branches => &[
            ("Space", "select"),
            ("a", "select cleanable"),
            ("f", "filter"),
            ("s", "sort"),
            ("/", "search"),
            ("Enter", "open repo"),
            ("x", "run queue"),
        ],
        Tab::Queue => &[
            ("Space", "remove"),
            ("Enter", "run"),
            ("C", "clear"),
            ("?", "help"),
        ],
        Tab::Dashboard => &[
            ("a", "queue all cleanable"),
            ("Enter", "open repo"),
            ("?", "help"),
        ],
    }
}
