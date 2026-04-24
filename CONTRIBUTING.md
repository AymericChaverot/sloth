# Contributing to Sloth

Thank you for your interest in contributing to Sloth! This guide will help you get started.

## Getting Started

### Prerequisites

- **Rust 1.85+** (edition 2024) — install via [rustup](https://rustup.rs)
- **Git** accessible on your `PATH`
- A terminal that supports Unicode and ANSI colors

### Setup

```bash
git clone https://github.com/AymericChaverot/sloth.git
cd sloth
cargo build
cargo test
```

## Development Workflow

### 1. Branch Strategy

- Create a feature branch from `master`:
  ```bash
  git checkout -b feat/my-feature
  ```
- Use prefixes: `feat/`, `fix/`, `refactor/`, `docs/`, `test/`

### 2. Code Style

- Run `cargo fmt` before committing — the CI enforces formatting.
- Run `cargo clippy` and address all warnings.
- Keep functions focused and files reasonably sized (~200 lines max).

### 3. Testing

```bash
# Run the full suite
cargo test

# Run with output
cargo test -- --nocapture

# Run a specific test
cargo test test_analyze_repository
```

- **Unit tests** go in the same file as the code under `#[cfg(test)] mod tests`.
- Use `MockSystem` from `sys::mock` for hermetic testing — no disk or Git access needed.
- Every new parsing function, command wrapper, or engine action should have test coverage.

### 4. System Abstraction

All Git and filesystem operations must go through the trait interfaces defined in `sys.rs`:

- **`GitExecutor`** for any `git` CLI call (use `run_git_command` for sync, `run_git_command_async` for async)
- **`FileSystem`** for any `std::fs` operation (`exists`, `get_size`)

Never call `std::process::Command::new("git")` or `std::fs::metadata()` directly. Always accept the trait as a parameter (`&impl GitExecutor` or `&impl FileSystem`).

### 5. Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add horizontal scrollbar to graph view
fix: prevent crash when no repos found
refactor: extract event handler to ui/events.rs
docs: update README with new shortcuts
test: add parse_shortstat edge cases
```

### 6. Pull Requests

1. Ensure `cargo fmt`, `cargo clippy`, and `cargo test` all pass.
2. Write a clear PR description explaining **what** changed and **why**.
3. Reference any related issues.
4. Keep PRs focused — one feature or fix per PR.

## Project Structure

```
src/
├── main.rs          # CLI args + async orchestration
├── scanner.rs       # Filesystem walker
├── engine.rs        # Execution engine (clean, prune, gc, deep clean)
├── sys.rs           # System traits (GitExecutor, FileSystem) + MockSystem
├── updater.rs       # Self-update checker
├── git/             # Git domain logic
│   ├── models.rs    # Data structures (RepoStatus, BranchInfo, StashInfo, WorktreeInfo)
│   ├── commands.rs  # Git command wrappers + unit tests
│   └── stats.rs     # Branch stats + size utilities + unit tests
├── ui.rs            # TUI runner
└── ui/              # UI modules
    ├── state.rs     # AppState + enums (Focus, ScannerEvent, UiAction)
    ├── events.rs    # Keyboard handler
    ├── theme.rs     # Theme engine with persistence
    └── components/  # Render functions
        ├── header.rs        # App title bar
        ├── repositories.rs  # Repo list pane
        ├── details.rs       # Branch/stash/worktree details pane
        ├── graph.rs         # Git commit graph pane
        ├── dashboard.rs     # Aggregated stats overlay
        ├── diff_modal.rs    # Branch/stash diff viewer
        ├── confirm_modal.rs # Pre-execution confirmation prompt with action preview
        └── help.rs          # Context-sensitive help bar
```

### Adding a New Feature

1. **Data model** → Add or modify structs in `git/models.rs`
2. **Git logic** → Add commands in `git/commands.rs` using `&impl GitExecutor` / `&impl FileSystem`, with tests using `MockSystem`
3. **State** → Update `AppState` in `ui/state.rs` if new UI state is needed
4. **Events** → Handle new keys in `ui/events.rs`
5. **Rendering** → Create or update components in `ui/components/`
6. **Engine** → If a new action is needed, add it to `Action` enum and `execute_action` in `engine.rs`
7. **Wire up** → Connect everything in `ui.rs` and/or `main.rs`

## CI Pipeline

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs on every push and PR:

| Step | Command |
|---|---|
| Format check | `cargo fmt -- --check` |
| Lint | `cargo clippy -- -D warnings` |
| Test | `cargo test` |
| Build | `cargo build --release` |

All checks must pass before merging.

## Reporting Issues

When opening an issue, please include:

- **OS** and terminal emulator
- **Rust version** (`rustc --version`)
- Steps to reproduce
- Expected vs actual behavior
- Any error output or screenshots

## Code of Conduct

Be respectful, constructive, and inclusive. We're all here to build great software.
