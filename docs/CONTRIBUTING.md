# Contributing to stutter

Thanks for your interest in contributing! This document covers everything you need to get started.

## Table of Contents

- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Code Style](#code-style)
- [Adding a New Backend](#adding-a-new-backend)
- [Submitting a Pull Request](#submitting-a-pull-request)
- [Reporting Bugs](#reporting-bugs)

---

## Development Setup

**Requirements:**

- Rust **1.85+** (MSRV)
- A supported Wayland compositor running (Hyprland or Niri) — or use `--dry-run` for offline development
- `CAP_SYS_NICE` or root access if you want to test real priority changes

**Clone and build:**

```bash
git clone https://github.com/Kleshzz/stutter
cd stutter
cargo build
```

**Run in dry-run mode** (no actual `setpriority` calls, safe for development):

```bash
cargo run -- --dry-run
```

**Run tests:**

```bash
cargo test --all-targets
```

---

## Project Structure

```
src/
├── main.rs          # Entry point — event loop, signal handling
├── config.rs        # Config loading from ~/.config/stutter/config.toml
├── scheduler.rs     # setpriority(2) wrapper
├── error.rs         # Shared error types
└── backend/
    ├── mod.rs       # Backend trait, FocusEvent, compositor detection
    ├── hyprland.rs  # Hyprland IPC backend
    └── niri.rs      # Niri IPC backend
```

The core abstraction is the `WmBackend` trait in `src/backend/mod.rs`. Each compositor backend implements a single async method — `next_focus_event` — which yields `FocusEvent { pid, addr, class }` on every focus change. The main loop in `main.rs` is compositor-agnostic.

---

## Making Changes

1. **Fork** the repository and create a branch from `main`.
2. Keep commits focused — one logical change per commit.
3. Make sure `cargo clippy --all-targets -- -D warnings` passes before opening a PR.
4. Make sure `cargo +nightly fmt --all` has been run (see [Code Style](#code-style)).
5. If you are adding a feature or fixing a bug, add a brief description to your PR — the changelog is generated from PR titles and labels.

---

## Code Style

Formatting uses **nightly rustfmt** with the project's `.rustfmt.toml`:

```bash
cargo +nightly fmt --all
```

Linting uses **stable clippy** with `pedantic` and `nursery` sets enabled:

```bash
cargo +stable clippy --all-targets -- -D warnings
```

A few project-wide conventions to keep in mind:

- Prefer `?` over `unwrap`/`expect` in production paths — both are warned on by clippy.
- `unsafe` code is allowed but warned; leave a `// SAFETY:` comment explaining the invariant.
- Log with `tracing` macros (`info!`, `warn!`, `error!`). Do not use `println!`.
- Prefer logging errors once at the call site (e.g., in `main.rs`) rather than inside library functions to avoid duplicate log entries.
- Keep `max_width` at 110 characters (set in `.rustfmt.toml`).

Both the format check and clippy run automatically on every push and pull request via CI.

---

## Adding a New Backend

If you want to add support for a new Wayland compositor, here is the minimal path:

1. Create `src/backend/<compositor>.rs`.
2. Define a connection struct and implement the `WmBackend` trait:

```rust
use crate::{backend::{FocusEvent, WmBackend}, error::Result};

pub struct MyBackend { /* IPC socket, reader, … */ }

impl MyBackend {
    pub async fn connect() -> Result<Self> { … }
}

impl WmBackend for MyBackend {
    async fn next_focus_event(&mut self) -> Result<Option<FocusEvent>> {
        // Read the next focus-change event from the compositor IPC.
        // Return Ok(None) when the socket is closed (triggers reconnect).
    }
}
```

3. Add the new variant to the `Backend` enum in `src/backend/mod.rs` and wire it into `detect()` (check the relevant environment variable or socket path).
4. Add a match arm in `main.rs` where `backend::detect()` is called and where `next_focus_event` is dispatched.

Refer to the existing implementations in the `src/backend/` directory for reference.

---

## Submitting a Pull Request

- Target the `main` branch.
- Fill in the PR description: what changed and why.
- If your change is user-visible (new feature, behaviour change, bug fix), a changelog entry will be generated automatically from the PR title — write it clearly.
- CI must be green before merge: `lint` → `build & test` → `msrv` jobs all need to pass.
- New GitHub Actions workflows must follow the principle of least privilege by explicitly limiting `GITHUB_TOKEN` permissions (e.g., `permissions: contents: read`).

For larger changes (new backend, API redesign) it is worth opening an issue first to align on the approach before writing code.

---

## Reporting Bugs

Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.yml). Please include:

- Compositor name and version
- stutter version (`stutter --version` or from `Cargo.toml`)
- Relevant log output (`RUST_LOG=debug stutter`)
- Steps to reproduce

---

## License

By contributing you agree that your contribution will be licensed under the [MIT License](LICENSE).