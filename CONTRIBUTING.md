# Contributing to Sloth

Thank you for your interest in contributing to Sloth! This guide will help you get started.

## Getting Started

### Prerequisites

- **Rust 1.85+** (edition 2024) — install via [rustup](https://rustup.rs)
- **Git** accessible on your `PATH`
- A terminal that supports Unicode and ANSI colors

### Setup

```bash
git clone https://github.com/your-username/sloth.git
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

# Update snapshots if needed
cargo insta review
```

- **Unit tests** go in the same file as the code under `#[cfg(test)] mod tests`.
- **Snapshot tests** use `insta` for golden-file validation.
- Every new parsing function or state transition should have test coverage.

### 4. Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add horizontal scrollbar to graph view
fix: prevent crash when no repos found
refactor: extract event handler to ui/events.rs
docs: update README with new shortcuts
test: add parse_shortstat edge cases
```

### 5. Pull Requests

1. Ensure `cargo fmt`, `cargo clippy`, and `cargo test` all pass.
2. Write a clear PR description explaining **what** changed and **why**.
3. Reference any related issues.
4. Keep PRs focused — one feature or fix per PR.

## Project Structure

```
src/
├── main.rs          # CLI args + async orchestration
├── scanner.rs       # Filesystem walker
├── engine.rs        # Execution engine (branch/stash operations)
├── git/             # Git domain logic
│   ├── models.rs    # Data structures
│   └── commands.rs  # Git command wrappers
├── ui.rs            # TUI runner
└── ui/              # UI modules
    ├── state.rs     # AppState + enums
    ├── events.rs    # Keyboard handler
    └── components/  # Render functions (repos, details, graph)
```

### Adding a New Feature

1. **Data model** → Add or modify structs in `git/models.rs`
2. **Git logic** → Add commands in `git/commands.rs` with tests
3. **State** → Update `AppState` in `ui/state.rs` if new UI state is needed
4. **Events** → Handle new keys in `ui/events.rs`
5. **Rendering** → Create or update components in `ui/components/`
6. **Wire up** → Connect everything in `ui.rs` and/or `main.rs`

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
