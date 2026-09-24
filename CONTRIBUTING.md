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

- Create a feature branch from `main`:
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
- When real Git behavior matters, use `test_support::TempRepo` to build a throwaway repository.
- UI changes are covered by snapshot tests in `src/ui/tests.rs`: after an intended change, run `cargo insta review` (or `INSTA_UPDATE=always cargo test`) and review the `.snap` diffs.
- Every new parsing function, command wrapper, engine operation or key binding should have test coverage.

### 4. System Abstraction

All Git and filesystem operations must go through the trait interfaces defined in `sys.rs`:

- **`GitExecutor`** for any `git` CLI call (`run_git_command`, `run_git_command_async`, `run_git_command_with_input`)
- **`FileSystem`** for disk usage (`get_size`)

Never call `std::process::Command::new("git")` directly. Always accept the trait as a parameter (`&impl GitExecutor` or `&impl FileSystem`).

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

1. Ensure `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` all pass.
2. Write a clear PR description explaining **what** changed and **why**.
3. Reference any related issues.
4. Keep PRs focused — one feature or fix per PR.

## Project Structure

See the [Architecture section of the README](README.md#architecture) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

### Adding a New Feature

1. **Data model** → structs in `git/models.rs`
2. **Git logic** → `git/analyze.rs` or `git/commands.rs`, taking `&impl GitExecutor`, tested with `MockSystem`
3. **Cleanup rules** → protection, plans and warnings in `cleanup.rs`
4. **Engine** → new operations in `engine::Operation` and `engine::run`
5. **State & views** → `ui/state.rs` for state, `ui/views.rs` for what a table lists
6. **Input** → `ui/events.rs`, and document the key in `ui/keymap.rs` (status bar + `?` overlay) and the README
7. **Rendering** → components in `ui/components/`, with a snapshot test

## CI Pipeline

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs on every push and PR:

| Step | Command |
|---|---|
| Format check | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Test (Linux, macOS, Windows) | `cargo test -- --test-threads=1` |
| Coverage | `cargo llvm-cov` |
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
