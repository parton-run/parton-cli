# Parton

AI-powered parallel code generation from natural language.

Describe what you want. Parton plans the work, generates all files in parallel, validates the result, and writes it to disk. One command, working code.

```
$ parton run "create a React todo app with TypeScript and Tailwind CSS"
```

## How it works

```
Prompt → Clarify → Plan → Execute (parallel) → Validate → Done
```

1. **Clarify** — AI asks targeted questions to fill in gaps (framework, styling, storage...)
2. **Plan** — Generates a JSON execution plan: which files to create, their exports, imports, and contracts
3. **Execute** — All files are generated simultaneously by separate LLM agents running in parallel
4. **Validate** — Compliance checking, dependency install, build & test verification

Each file agent is blind to other agents' output. The plan's interface contracts (exports, imports, conventions) are the only coordination mechanism. This is what makes parallel execution possible without conflicts.

## Quick start

```bash
# Install (macOS)
curl -fsSL https://parton.run/install.sh | sh

# Or build from source
cargo build --release
cp target/release/parton /usr/local/bin/

# Run
mkdir my-app && cd my-app
parton run "create a REST API with Express and Prisma"
```

On first run, Parton detects available AI providers and walks you through setup.

## Features

- **Parallel execution** — 15 files generated simultaneously, not one at a time
- **Multi-provider** — Claude CLI, Codex CLI, OpenAI API, Ollama (local), or any combination
- **Per-stage models** — Use Opus for planning, Haiku for execution, GPT for judge
- **Interactive TUI** — Arrow-key model selection, clarification questions, plan review with comments
- **Plan review** — Inspect every file before execution. Add comments to trigger replanning
- **Knowledge system** — Learns project conventions and applies them to future runs
- **Language-agnostic** — Works with any language: TypeScript, Rust, Go, Python, and more
- **Auto dep install** — Detects and runs npm/yarn/pnpm/cargo/go/pip automatically
- **Validation** — Runs your build and test commands after generation

## Configuration

Parton creates a `parton.toml` in your project root:

```toml
[project]
name = "my-app"

[execution]
validation = ["npm run build", "npm test"]

[models.default]
access = "cli"
command = "claude"
model = "claude-sonnet-4-6"

# Use a different model for execution (optional)
[models.execution]
provider = "openai"
model = "gpt-4o-mini"
access = "api"
env_key = "PARTON_OPENAI_KEY"  # Custom env var to avoid collisions
```

### Supported providers

| Provider | Access | Setup |
|----------|--------|-------|
| Claude | CLI | `brew install claude` |
| Codex | CLI | `npm i -g @openai/codex` |
| OpenAI API | API key | `export OPENAI_API_KEY=sk-...` |
| Ollama | Local | `brew install ollama && ollama pull qwen2.5-coder:7b` |

Mix and match — use Claude for planning (better reasoning) and a fast API model for parallel file generation.

## Commands

```
parton run <prompt>     Generate code from a prompt
parton setup            Configure providers and models
parton version          Show version
```

### Flags

```
parton run "prompt" --review     Always show plan review before executing
parton run "prompt" --verbose    Enable debug logging
```

## Plan review

Every run shows the plan before executing. For larger plans (10+ files), review is automatic:

```
┌─────────────────────────────────────────────────────────────┐
│ Plan Review — 14 files  │  0 comments  │  Enter = approve   │
├───────────────────┬─────────────────────────────────────────┤
│ Files             │ Detail                                  │
│                   │                                         │
│❯+ package.json    │ Path: package.json                      │
│ + tsconfig.json   │ Action: Create                          │
│ + src/App.tsx     │                                         │
│ + src/types.ts    │ Goal: Create package.json with...       │
│ + src/hooks/...   │                                         │
├───────────────────┴─────────────────────────────────────────┤
│ Comments (0)                                                │
├─────────────────────────────────────────────────────────────┤
│ ↑↓ navigate  c file comment  g general comment  Enter approve│
└─────────────────────────────────────────────────────────────┘
```

- **Enter** with no comments = approve and execute
- **c** = add comment to selected file
- **g** = add general comment
- **Enter** with comments = replan (AI revises the plan based on your feedback)
- **r** = reject

## Architecture

```
crates/
├── parton-core/        # Shared types, config, provider trait
├── parton-providers/   # LLM providers: OpenAI, Ollama, CLI wrappers
├── parton-planner/     # Plan generation, parsing, validation, clarification
├── parton-executor/    # Parallel execution, compliance, output parsing, file writer
├── parton-knowledge/   # Convention detection, learning extraction
├── parton-graph/       # SQLite code graph (files, symbols, relationships)
└── parton-cli/         # CLI entry point, TUI (ratatui), commands
```

Core principles:

- **Crates are separate concerns** — providers know nothing about planning, planner knows nothing about execution
- **Core stays sync** — async only at the provider boundary
- **No LLM for deterministic work** — graph building, validation, file writing are all deterministic
- **Language-agnostic** — no hardcoded language logic in core; detection is pattern-based

## Development

```bash
# Check everything
cargo check && cargo clippy -- -D warnings && cargo test

# Build release
./scripts/build-release.sh macos-arm64

# Run from source
cargo run -- run "create a hello world app"
```

Quality rules enforced across the codebase:

- `#![deny(warnings)]` in every crate
- Every public function has tests
- No `unwrap()` in library code
- Zero clippy lints

## License

MIT
