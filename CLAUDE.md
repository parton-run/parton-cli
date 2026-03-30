# Parton CLI — Agent Instructions

## What is Parton?

Parton is an AI-powered code generation CLI that executes tasks in parallel.
It plans what files to create/edit, then generates them simultaneously using LLM providers.

## Tech Stack

- **Language:** Rust (2021 edition)
- **Async:** Tokio (only in providers and executor — core stays sync)
- **Database:** SQLite via rusqlite
- **CLI:** clap (derive)

## Workspace Crates

```
parton-core       # Shared types, config, contracts. No provider specifics.
parton-providers  # LLM providers: Anthropic API, OpenAI API, Ollama, CLI wrappers
parton-planner    # Turbo planner: generates JSON execution plans via LLM
parton-executor   # Parallel file execution via provider + compliance checking
parton-knowledge  # Local knowledge store (learnings, conventions)
parton-graph      # SQLite graph store for code relationships
parton-cli        # CLI entry point, TUI, commands
```

## Code Quality Rules — MANDATORY

These are non-negotiable. Every PR must pass all of these.

### File Size
- **Maximum 200 lines per file.** If a file exceeds this, split it.
- No exceptions. Prefer many small, focused files over few large ones.

### Testing
- **Every public function has at least one test.**
- Tests live in `#[cfg(test)] mod tests` at the bottom of the file, or in a separate `tests/` directory for integration tests.
- Use `assert!`, `assert_eq!`, `assert_matches!` — no `println!` debugging.

### Type Safety
- **No `unwrap()` in library code.** Use `?`, `map_err`, or `unwrap_or`.
- `unwrap()` is allowed only in tests and in `main()`.
- Use enums over stringly-typed values. If there are 3+ string variants, make an enum.
- All public types must derive or implement `Debug`.

### Error Handling
- Library crates use `thiserror` with typed error enums.
- CLI crate uses `anyhow` for ergonomic error propagation.
- Error messages must be lowercase, no trailing period: `"failed to parse config"`.

### Documentation
- Every `pub` type, trait, and function has a `///` doc comment.
- Doc comments describe *what* and *why*, not *how*.

### Warnings
- `#![deny(warnings)]` in every crate's `lib.rs` or `main.rs`.
- `cargo clippy -- -D warnings` must pass.
- Zero `#[allow(dead_code)]` — if it's dead, delete it.

### Naming
- Files: `snake_case.rs`
- Types: `PascalCase`
- Functions: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`

### Imports
- Group imports: std → external crates → workspace crates → local modules.
- Prefer re-exports from `parton-core` for shared types.

## Architecture Rules

- Providers, planner, executor, knowledge, and graph MUST remain separate crates.
- LLMs must NEVER be used for graph construction or deterministic operations.
- Core logic stays sync — async only at the provider/executor boundary.
- No provider-specific logic leaks into core or planner.

## Do NOT

- Do NOT commit .env files or API keys
- Do NOT use `sqlx` — rusqlite is the right choice for local SQLite
- Do NOT add unnecessary async — keep core sync
- Do NOT use `unwrap()` in library code
- Do NOT create files over 200 lines
- Do NOT skip tests for public functions
- Do NOT add features beyond what was asked
