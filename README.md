<p align="center">
  <h1 align="center">Sloth</h1>
  <p align="center"><strong>A blazing-fast TUI for cleaning up many Git repositories at once.</strong></p>
  <p align="center">
    Scan a directory tree, find merged, gone and stale branches in every project, queue them up across repositories, review, clean — and undo if needed.
  </p>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#usage">Usage</a> •
  <a href="#keyboard-shortcuts">Shortcuts</a> •
  <a href="#configuration">Configuration</a> •
  <a href="#safety">Safety</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#contributing">Contributing</a>
</p>

---

## Features

| Feature | Description |
|---|---|
| **Cross-repository queue** | Select branches, stashes and worktrees in any number of repositories, review them in one queue, run everything at once |
| **Branches tab** | Every branch of every repository in one table, filtered by cleanable / merged / gone / stale / unmerged, sortable and searchable |
| **Smart detection** | Merged branches — including **squash-** and **rebase-merged** ones — branches whose remote is gone, and stale branches |
| **Safety first** | Default, checked-out and configured branches are protected; unmerged or unpushed work and dirty worktrees are flagged before anything runs |
| **Undo** | Every deleted branch and stash is journaled and can be restored with `sloth restore` |
| **Maintenance** | Prune remote-tracking branches, garbage collect, deep clean untracked & ignored files (keeping `.env` & co. and nested repos) |
| **Fast** | Parallel discovery, each repository analyzed as soon as it is found, one `git for-each-ref` per repository, disk usage measured in the background without descending into ignored folders |
| **Dashboard** | Cleanable branches, reclaimable space, `.git` sizes, and the repositories with the most to clean |
| **Git graph & diffs** | Colored commit graph of all branches and branch/stash diffs, without blocking the UI |
| **Mouse & keyboard** | Tabs, tables, filters, sorting, vim keys, mouse clicks and wheel, `?` for the full key reference |
| **Scriptable** | `sloth clean --merged --gone --dry-run` runs without the TUI |
| **Configurable** | TOML config file for the default path, protected branches, stale threshold, theme and more |
| **Self-update** | Automatic update check on startup with one-key installation |

## Installation

### Quick Install (recommended)

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/AymericChaverot/sloth/main/scripts/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/AymericChaverot/sloth/main/scripts/install.ps1 | iex
```

This will download the latest release, install the binary to the correct location, and add it to your `PATH`.

| OS | Install location |
|---|---|
| Linux | `~/.sloth/bin/sloth` |
| macOS | `~/.sloth/bin/sloth` |
| Windows | `%USERPROFILE%\.sloth\bin\sloth.exe` |

### Build from source

Prerequisites: [Rust](https://www.rust-lang.org/tools/install) 1.85+ and `git` on your `PATH`.

```bash
git clone https://github.com/AymericChaverot/sloth.git
cd sloth
cargo build --release
```

The binary will be at `target/release/sloth` (or `sloth.exe` on Windows).

## Usage

```bash
# Scan the current directory (or `default_path` from the config)
sloth

# Scan a specific directory
sloth --path ~/projects        # or: sloth -p ~/projects
```

Sloth discovers every Git repository under the directory, analyzes branches, stashes and worktrees, measures disk usage in the background, and opens the TUI.

### A typical cleanup

1. Open the **Branches** tab (`2`): it lists the cleanable branches of *all* repositories.
2. Press `a` to queue them all, or `Space` to pick them one by one (`f` shows other filters, `/` searches).
3. Open the **Queue** tab (`3`) to review what will run, with warnings for unmerged or unpushed work.
4. Press `x` (or Enter), confirm with `y`. Results appear in place, and the affected repositories are re-analyzed.
5. Changed your mind? `sloth restore --last`.

### Without the TUI

```bash
# What would be deleted? (merged and gone branches by default)
sloth clean -p ~/projects --dry-run

# Delete merged branches and branches untouched for 180 days, without prompting
sloth clean -p ~/projects --merged --stale 180 --yes

# Only branches whose name contains "renovate"
sloth clean --branch renovate --merged
```

| Option | Description |
|---|---|
| `--merged` | Branches merged into the default branch (including squash/rebase merges) |
| `--gone` | Branches whose upstream was deleted on the remote |
| `--stale [DAYS]` | Branches without a commit for DAYS days (`stale_days` from the config by default) |
| `--branch TEXT` | Only branches whose name contains TEXT |
| `-n`, `--dry-run` | Print the plan, change nothing |
| `-y`, `--yes` | Do not ask for confirmation (required when not run from a terminal) |

### Undo

```bash
sloth restore            # list recently deleted branches and stashes
sloth restore 3 5        # restore entries #3 and #5
sloth restore --last     # restore everything deleted by the last run
```

Restoring works as long as Git has not garbage-collected the commits (two weeks by default). A branch whose name was reused since is restored as `<name>-restored-<sha>`.

## Keyboard Shortcuts

`x` (clean) and `q` (quit) are always shown at the bottom of the screen. Press `?` in the TUI for the full reference. Most tables also accept `j`/`k`, Page Up/Down, Home/End and the mouse.

### Global

| Key | Action |
|---|---|
| `1`–`4`, `Tab` / `Shift-Tab` | Switch tab (Repos, Branches, Queue, Dashboard) |
| `x` | Review & run the cleanup queue |
| `C` | Clear the queue |
| `r` / `R` | Refresh the marked (or focused) repositories / rescan everything |
| `t` | Cycle theme |
| `?` | Key reference |
| `u` | Install the available update |
| `q` | Quit |

### Repos tab — repository list

| Key | Action |
|---|---|
| `Space` | Mark the repository (target of `a`, `p`, `c`, `X`, `r`) |
| `→` / `Enter` | Open its details |
| `a` | Queue merged & gone branches of the marked repositories |
| `p` / `c` / `X` | Prune remote-tracking branches / garbage collect / deep clean (with confirmation and preview) |
| `/` | Filter by path or remote |
| `s` | Sort by name (A-Z, the default), cleanable branches, reclaimable space or `.git` size |
| `g` | Toggle the git graph (`f` fullscreen) |
| `Esc` | Clear the filter, then the marks |

### Repos tab — details

| Key | Action |
|---|---|
| `Space` | Add / remove the branch, stash or worktree from the queue |
| `a` / `A` | Queue merged & gone branches / every unprotected branch |
| `v` | View the diff of the branch or stash |
| `Esc` | Clear this repository's selection |
| `←` | Back to the list |

### Branches tab

| Key | Action |
|---|---|
| `Space` | Add / remove the branch from the queue |
| `a` / `A` | Queue every listed cleanable / unprotected branch |
| `f` | Filter: cleanable, merged, gone, stale, unmerged, all |
| `s` | Sort by repository, oldest commit, name |
| `/` | Search repository or branch names |
| `v` | View the diff |
| `Enter` | Open the branch in its repository |

### Queue tab

| Key | Action |
|---|---|
| `Space` / `d` | Remove the item |
| `Enter` / `x` | Review & run everything |
| `C` | Clear the queue |

### Dashboard

| Key | Action |
|---|---|
| `a` | Queue every merged & gone branch of every repository |
| `Enter` | Open the repository |

## Configuration

Sloth creates a commented configuration file on first run:

| OS | Location |
|---|---|
| Linux | `~/.config/sloth/config.toml` |
| macOS | `~/Library/Application Support/sloth/config.toml` |
| Windows | `%APPDATA%\sloth\config.toml` |

Set `SLOTH_CONFIG` to use another file.

```toml
# Directory scanned when --path is not given.
default_path = "~/Projects"

# Branches that can never be selected for deletion (`*` and `?` wildcards).
# The default branch and checked-out branches are always protected.
protected_branches = ["main", "master", "develop", "trunk", "release/*"]

# Untracked or ignored files a deep clean must never delete (gitignore syntax).
deep_clean_keep = [".env", ".env.*", ".idea/", ".vscode/"]

# Branches whose last commit is older than this many days are "stale".
stale_days = 90

# Mouse support in the TUI (clicks and wheel). Disable it to select text.
mouse = true

# Directory names skipped while scanning for repositories.
scan_exclude = ["node_modules", "target", ".venv", "vendor"]

# Saved automatically when cycling themes with `t`.
theme = "Nord"
```

## Safety

Sloth deletes things, so it is careful about what it deletes:

- **Protected branches** — the default branch (from `origin/HEAD`, else `main`/`master`/`trunk`/`develop`), any checked-out branch and `protected_branches` cannot be selected. The main worktree cannot be removed.
- **Warnings** — branches with commits that are neither merged nor pushed, and worktrees with uncommitted changes, are flagged in the Queue tab and the confirmation window.
- **No surprises** — a branch is only deleted if it still points to the commit that was analyzed; stashes are matched by commit id, not by their shifting index.
- **Deep clean** — never touches nested repositories nor `deep_clean_keep` files, and shows what it will delete first.
- **Undo** — deleted branches and stashes are journaled (`<data dir>/sloth/journal.jsonl`, or `$SLOTH_JOURNAL`) and restorable with `sloth restore`.

## Architecture

```
src/
├── main.rs              # CLI parsing and startup
├── config.rs            # TOML configuration
├── scanner.rs           # Parallel repository discovery (ignore crate)
├── worker.rs            # Background pipeline: discovery → analysis → disk usage
├── cleanup.rs           # Protection rules, cleanup plans, warnings
├── engine.rs            # Concurrent execution of cleanup plans
├── journal.rs           # Deletion journal and restore
├── sys.rs               # GitExecutor / FileSystem traits, RealSystem, MockSystem
├── updater.rs           # Self-update (GitHub releases)
├── cli/                 # `sloth clean` and `sloth restore`
├── git/
│   ├── analyze.rs       # Repository analysis (for-each-ref, stashes, worktrees)
│   ├── squash.rs        # Squash- and rebase-merge detection
│   ├── commands.rs      # Graph and diff commands
│   ├── models.rs        # RepoStatus, BranchInfo, StashInfo, WorktreeInfo
│   └── stats.rs         # Ahead/behind fallback, shortstat, sizes and ages
├── ui.rs                # TUI loop: events, background loads, execution
└── ui/
    ├── state.rs         # AppState, tabs, events
    ├── selection.rs     # The cross-repository cleanup queue
    ├── views.rs         # Rows of each table (filtering, sorting)
    ├── events.rs        # Keyboard and mouse handling
    ├── keymap.rs        # Key bindings shown in the status bar and help
    ├── loader.rs        # Graph, diff and preview loading off the UI thread
    ├── theme.rs         # Color themes
    ├── tests.rs         # Snapshot and end-to-end UI tests
    └── components/      # One render function per widget (tabs, tables, modals…)
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the data flow and design decisions.

### Tech Stack

| Crate | Role |
|---|---|
| [`ratatui`](https://crates.io/crates/ratatui) / [`crossterm`](https://crates.io/crates/crossterm) | Terminal UI and I/O |
| [`tokio`](https://crates.io/crates/tokio) | Async runtime & task spawning |
| [`ignore`](https://crates.io/crates/ignore) | Fast `.gitignore`-respecting directory walking |
| [`clap`](https://crates.io/crates/clap) | CLI parsing |
| [`serde`](https://crates.io/crates/serde) / [`toml`](https://crates.io/crates/toml) / [`toml_edit`](https://crates.io/crates/toml_edit) / [`serde_json`](https://crates.io/crates/serde_json) | Configuration and journal |
| [`ansi-to-tui`](https://crates.io/crates/ansi-to-tui) | Colored git output in the TUI |
| [`insta`](https://crates.io/crates/insta) | UI snapshot tests |

## Testing

```bash
cargo test                 # unit, snapshot and end-to-end tests
cargo insta review         # review UI snapshot changes (cargo install cargo-insta)
```

- **Unit tests** use `MockSystem` to fake git output: parsers, analysis, protection rules, plans, engine, selection, config.
- **Real-git tests** create temporary repositories to check squash detection, deep clean, restore, and the full TUI flow (scan → queue → run → refresh).
- **Snapshot tests** render every tab and overlay into a test backend and compare them with reviewed snapshots.

Git must be installed; everything runs in temporary directories.

## License

This project is provided as-is for personal and educational use.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute.
