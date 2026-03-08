# Contributing to Harmonia

Thank you for your interest in contributing to Harmonia.

## Code of Conduct

This project follows the [Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).

## Getting Started

### Prerequisites

- **Rust** 1.75+ (install via [rustup](https://rustup.rs))
- **SBCL** (Steel Bank Common Lisp) — the agent runtime
  - macOS: `brew install sbcl`
  - Ubuntu/Debian: `sudo apt install sbcl`
  - FreeBSD: `sudo pkg install sbcl`
  - NetBSD: `sudo pkgin install sbcl`
- **Quicklisp** — Common Lisp package manager
  ```bash
  curl -O https://beta.quicklisp.org/quicklisp.lisp
  sbcl --load quicklisp.lisp \
       --eval '(quicklisp-quickstart:install)' \
       --eval '(ql:add-to-init-file)' --quit
  ```

### Development Setup

```bash
git clone https://github.com/harmoniis/harmonia.git
cd harmonia
cargo build --workspace
cargo test --workspace
```

## Coding Standards

### Rust

- Format with `cargo fmt`
- Lint with `cargo clippy -- -D warnings`
- All public functions need doc comments
- Crate types are `cdylib` + `rlib` — FFI functions are `extern "C"` with `#[no_mangle]`

### Common Lisp

- Follow existing style in `src/`
- Use `defun` with docstrings for public functions
- All I/O goes through Rust FFI — never call system functions directly from Lisp

## Branch Naming

- `fix/<description>` — bug fixes
- `feat/<description>` — new features
- `docs/<description>` — documentation
- `refactor/<description>` — restructuring

## Commit Format

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): summary

Body explaining what and why.
```

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `perf`

**Scopes:** `core`, `vault`, `browser`, `gateway`, `frontends`, `cli`, `docs`

## Pull Request Process

1. Keep PRs focused — one logical change per PR
2. Update `CHANGELOG.md` under `[Unreleased]`
3. Add tests for new behavior
4. Run the full test suite locally:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```
5. Squash-and-merge on approval

## Architecture

Read `doc/genesis/INDEX.md` for the full architecture guide. Key principles:

- **Lisp is orchestration only.** Rust handles all I/O. No exceptions.
- **Four Pillars:** `lib/core/`, `lib/backends/`, `lib/tools/`, `lib/frontends/`
- **Every response is security-wrapped.** Browser and external data always passes through security boundaries.
- **Self-improvement follows the 8 Laws of Harmonia.** See `doc/genesis/HARMONIC_THEORY.md`.
