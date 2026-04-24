<p align="center">
  <h1 align="center">🦥 Sloth</h1>
  <p align="center"><strong>A blazing-fast TUI for managing multiple Git repositories at once.</strong></p>
  <p align="center">
    Scan a directory tree, visualize branches, stashes &amp; worktrees, explore commit graphs, inspect diffs, track disk usage, and clean up — all from one terminal.
  </p>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#usage">Usage</a> •
  <a href="#keyboard-shortcuts">Shortcuts</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#testing">Testing</a> •
  <a href="#contributing">Contributing</a>
</p>

---

## Features

| Feature | Description |
|---|---|
| **⚡ Async scanning** | Discovers nested Git repos instantly using parallel filesystem walking |
| **📊 Branch analytics** | Shows ahead/behind counts, diff stats, merge status, and last commit date |
| **🌳 Git graph** | Interactive ASCII commit graph with ANSI colors, fullscreen mode, and scrolling |
| **🔍 Diff modal** | Inline branch and stash diff viewer with scroll position indicator and Page Up/Down |
| **🧹 Multi-action cleanup** | Delete branches, drop stashes, remove worktrees, prune remotes, garbage collect, or deep clean |
| **🛡️ Confirmation prompt** | Previews exactly what will be deleted before any destructive action executes |
| **📦 Disk space tracking** | Live streaming size estimate per repo as files are discovered, then finalized on completion |
| **📋 Dashboard** | Aggregated overview of all scanned repositories with totals |
| **🎨 Theme engine** | Switchable color themes with persistence across sessions |
| **🖥️ Multi-pane layout** | Header → Repositories → Details → Git Graph, navigable with arrow keys |
| **🔄 Self-update** | Automatic update check on startup with one-key installation |
| **🎬 Live loaders** | Animated spinners while scanning & analyzing — the TUI never blocks |

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
# Scan the current directory
sloth

# Scan a specific directory
sloth --path ~/projects

# Short form
sloth -p ~/projects
```

Sloth will recursively discover every Git repository under the specified path, analyze their branches, stashes and worktrees, stream disk usage live in the background, then present an interactive TUI.

## Keyboard Shortcuts

### Global

| Key | Action |
|---|---|
| `q` | Quit |
| `d` | Toggle Dashboard view |
| `t` | Cycle color theme |
| `/` | Search / filter repositories and items |
| `u` | Update to latest version (when available) |

### Repository Pane

| Key | Action |
|---|---|
| `↑` / `↓` | Navigate repositories |
| `Space` | Select / deselect a repository |
| `→` | Enter details pane for the focused repository |
| `p` | Prune dead remote tracking branches (with confirmation) |
| `c` | Garbage collect (with confirmation) |
| `X` | Deep clean — remove untracked & ignored files (with confirmation + preview) |

### Details Pane (Branches, Stashes & Worktrees)

| Key | Action |
|---|---|
| `↑` / `↓` | Navigate branches, stashes, and worktrees |
| `Space` | Toggle selection on an item |
| `a` | Smart auto-select dead and merged branches |
| `A` | Select all branches |
| `Esc` | Deselect all items in the current repository |
| `Enter` | Execute CleanRepo (with confirmation) |
| `v` | View diff for the focused branch or stash |
| `g` | Toggle the Git Graph pane |
| `←` | Return to repository list |

### Confirmation Modal

| Key | Action |
|---|---|
| `Y` | Confirm and execute the action |
| `N` / `Esc` | Cancel |

### Git Graph Pane

| Key | Action |
|---|---|
| `↑` / `↓` | Scroll vertically |
| `←` / `→` | Scroll horizontally |
| `f` / `m` | Toggle fullscreen mode |
| `Esc` / `g` | Exit graph back to details |

### Diff Modal

| Key | Action |
|---|---|
| `↑` / `↓` | Scroll one line |
| `Page Up` / `Page Down` | Scroll ten lines |
| `Home` | Jump to top |
| `Esc` / `v` | Close diff modal |

## Architecture

```
src/
├── main.rs              # Entry point, CLI args, async orchestration
├── scanner.rs           # Async filesystem walker (tokio + ignore crate)
├── engine.rs            # Batch execution engine (clean, prune, gc, deep clean)
├── sys.rs               # System abstraction traits (GitExecutor, FileSystem, MockSystem)
├── updater.rs           # Self-update checker (GitHub releases API)
├── git/
│   ├── mod.rs           # Public facade
│   ├── models.rs        # RepoStatus, BranchInfo, StashInfo, WorktreeInfo, GitError
│   ├── commands.rs      # Git command wrappers (analyze, graph, diff) + unit tests
│   └── stats.rs         # Branch stats (ahead/behind, shortstat parsing) + unit tests
├── ui.rs                # TUI runner (terminal setup, render loop, event dispatch)
└── ui/
    ├── state.rs         # AppState, Focus enum, ScannerEvent, UiAction
    ├── events.rs        # Keyboard input handler
    ├── theme.rs         # Theme engine with switchable color palettes and persistence
    └── components/
        ├── mod.rs           # Component module exports
        ├── header.rs        # Top bar — app title, repo count, theme indicator
        ├── repositories.rs  # Left pane — repo list with live size streaming and selection
        ├── details.rs       # Middle pane — branches, stashes, worktrees with stats
        ├── graph.rs         # Right pane — ANSI git graph with fullscreen toggle
        ├── dashboard.rs     # Aggregated stats overview across all repositories
        ├── diff_modal.rs    # Floating modal for branch/stash diffs
        ├── confirm_modal.rs # Confirmation prompt with action preview before execution
        └── help.rs          # Context-sensitive keyboard shortcut help bar
```

### Tech Stack

| Crate | Role |
|---|---|
| [`clap`](https://crates.io/crates/clap) | CLI argument parsing |
| [`tokio`](https://crates.io/crates/tokio) | Async runtime & task spawning |
| [`rayon`](https://crates.io/crates/rayon) | Data parallelism for filesystem scanning |
| [`ratatui`](https://crates.io/crates/ratatui) | Terminal UI framework |
| [`crossterm`](https://crates.io/crates/crossterm) | Cross-platform terminal I/O |
| [`gix`](https://crates.io/crates/gix) | Pure-Rust Git repository validation |
| [`ansi-to-tui`](https://crates.io/crates/ansi-to-tui) | ANSI escape → ratatui text conversion |
| [`thiserror`](https://crates.io/crates/thiserror) | Ergonomic error types |
| [`anyhow`](https://crates.io/crates/anyhow) | Top-level error handling |
| [`ignore`](https://crates.io/crates/ignore) | Fast `.gitignore`-respecting directory walking |

### System Abstraction Layer

All Git and filesystem operations are abstracted behind two traits defined in `sys.rs`:

- **`GitExecutor`** — `run_git_command()` (sync) and `run_git_command_async()` for async engine operations
- **`FileSystem`** — `exists()` and `get_size()` (recursive for directories) for storage queries

At runtime, `RealSystem` wraps native `std::process::Command` and `std::fs` calls. In tests, `MockSystem` provides deterministic, in-memory fakes — no disk access required.

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture
```

The test suite covers:

| Module | Tests |
|---|---|
| `git::commands` | `analyze_repository` with mocked Git outputs and virtual filesystems |
| `git::stats` | `parse_shortstat` edge cases (empty, partial, malformed, insertions-only, deletions-only) |
| `engine` | `execute_batch` for CleanRepo and PruneRemotes with mocked async commands |
| `ui::state` | `AppState` initialization defaults and flag assertions |

All tests run hermetically via `MockSystem` — no real Git repositories or filesystem access needed.

## License

This project is provided as-is for personal and educational use.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute.
