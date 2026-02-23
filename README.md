<p align="center">
  <h1 align="center">🦥 Sloth</h1>
  <p align="center"><strong>A blazing-fast TUI for managing multiple Git repositories at once.</strong></p>
  <p align="center">
    Scan a directory tree, visualize branches &amp; stashes, explore commit graphs, and clean up dead branches — all from one terminal.
  </p>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#usage">Usage</a> •
  <a href="#keyboard-shortcuts">Shortcuts</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#contributing">Contributing</a>
</p>

---

## Features

| Feature | Description |
|---|---|
| **⚡ Async scanning** | Discovers nested Git repos instantly using parallel filesystem walking |
| **📊 Branch analytics** | Shows ahead/behind counts and diff stats relative to `main`/`master` or upstream |
| **🌳 Git graph** | Interactive ASCII commit graph with ANSI colors, fullscreen mode, and scrolling |
| **🧹 Repo cleaning** | Select and bulk-delete dead branches and old stashes with a single `Enter` |
| **🎨 Live loaders** | Animated spinners while scanning & analyzing — the TUI never blocks |
| **🖥️ Three-pane layout** | Repositories → Branch details → Git Graph, navigable with arrow keys |

## Installation

### Quick Install (recommended)

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/AymericChaverot/sloth/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/AymericChaverot/sloth/main/install.ps1 | iex
```

This will download the latest release, install the binary to the correct location, and add it to your `PATH`.

| OS | Install location |
|---|---|
| Linux | `~/.local/bin/sloth` |
| macOS | `~/.local/bin/sloth` |
| Windows | `%LOCALAPPDATA%\Programs\sloth\sloth.exe` |

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

Sloth will recursively discover every Git repository under the specified path, analyze their branches and stashes, then present an interactive TUI.

## Keyboard Shortcuts

### Navigation

| Key | Action |
|---|---|
| `↑` / `↓` | Navigate items in the focused pane |
| `←` / `→` | Move focus between panes (Repos → Details → Graph) |
| `q` | Quit |
| `u` | Update to latest version (when available) |

### Repository Pane

| Key | Action |
|---|---|
| `→` | Enter details view for the selected repository |

### Details Pane (Branches & Stashes)

| Key | Action |
|---|---|
| `Space` | Toggle selection on a branch or stash |
| `Enter` | Execute deletion of selected branches/stashes |
| `g` | Toggle the Git Graph pane |

### Git Graph Pane

| Key | Action |
|---|---|
| `↑` / `↓` | Scroll vertically |
| `←` / `→` | Scroll horizontally |
| `f` | Toggle fullscreen mode |
| `Esc` | Exit graph back to details |
| `g` | Hide graph pane |

### Self-Update

| Key | Action |
|---|---|
| `u` | Download and install the latest release (when banner is shown) |

## Architecture

```
src/
├── main.rs              # Entry point, CLI args, async orchestration
├── scanner.rs           # Async filesystem walker (tokio + ignore crate)
├── engine.rs            # Batch execution engine (branch delete, stash drop)
├── git/
│   ├── mod.rs           # Public facade
│   ├── models.rs        # RepoStatus, BranchInfo, StashInfo, GitError
│   └── commands.rs      # Git command wrappers + unit tests
├── ui.rs                # TUI runner (terminal setup, render loop)
└── ui/
    ├── state.rs         # AppState, Focus enum, ScannerEvent + unit tests
    ├── events.rs        # Keyboard input handler
    └── components/
        ├── mod.rs       # Component module exports
        ├── repositories.rs  # Left pane — repo list
        ├── details.rs       # Middle pane — branches & stashes
        └── graph.rs         # Right pane — ANSI git graph
```

### Tech Stack

| Crate | Role |
|---|---|
| [`clap`](https://crates.io/crates/clap) | CLI argument parsing |
| [`tokio`](https://crates.io/crates/tokio) | Async runtime & task spawning |
| [`rayon`](https://crates.io/crates/rayon) | Data parallelism |
| [`ratatui`](https://crates.io/crates/ratatui) | Terminal UI framework |
| [`crossterm`](https://crates.io/crates/crossterm) | Cross-platform terminal I/O |
| [`gix`](https://crates.io/crates/gix) | Pure-Rust Git repository access |
| [`ansi-to-tui`](https://crates.io/crates/ansi-to-tui) | ANSI escape → ratatui text conversion |
| [`thiserror`](https://crates.io/crates/thiserror) | Ergonomic error types |
| [`insta`](https://crates.io/crates/insta) | Snapshot testing (dev) |

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture
```

The test suite covers:
- **`git::commands`** — `parse_shortstat` edge cases (empty, partial, malformed input)
- **`ui::state`** — `AppState` initialization defaults and flag assertions

## License

This project is provided as-is for personal and educational use.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute.
