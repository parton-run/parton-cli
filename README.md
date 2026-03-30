<p align="center">
  <a href="https://parton.run">
    <img src="assets/logo.svg" alt="Parton" height="60" />
  </a>
</p>

<p align="center">
  AI-powered parallel code generation from natural language.
</p>

<p align="center">
  <a href="https://parton.run">Website</a> &middot;
  <a href="https://parton.run/docs">Docs</a> &middot;
  <a href="https://github.com/parton-run/parton-cli/releases">Releases</a>
</p>

---

Describe what you want. Parton plans the work, generates all files in parallel, validates the result, and writes it to disk. One command, working code.

```
$ parton run "create a React todo app with TypeScript and Tailwind CSS"
```

## How it works

```
Prompt → Clarify → Plan → Scaffold → Check → Execute → Validate
```

1. **Clarify** — AI asks targeted questions to fill in gaps (framework, styling, storage...)
2. **Plan** — Generates a skeleton plan with full interface contracts: exports, imports, prop names, function signatures
3. **Scaffold** — All files generated in parallel as minimal compilable stubs. Dependencies installed. Structure verified.
4. **Execute** — Logic files re-generated in parallel with full implementation, preserving verified imports/exports from scaffold
5. **Validate** — Build and test commands run. If they fail, auto-fix re-generates with error context.

Each file agent is blind to other agents' output. The plan's interface contracts are the only coordination mechanism. Scaffold verifies the structure compiles before investing tokens in full implementation.

## Quick start

```bash
# Install
curl -fsSL https://parton.run/install.sh | sh

# Or build from source
cargo build --release
cp target/release/parton /usr/local/bin/

# Run
mkdir my-app && cd my-app
parton run "create a REST API with Express and Prisma"
```

On first run, Parton detects available AI providers and walks you through an interactive setup.

## Features

- **Parallel execution** — all files generated simultaneously, not one at a time
- **3-step pipeline** — plan → scaffold+verify → implement. Catches structural errors before spending tokens on implementation
- **Multi-provider** — Claude, Codex, OpenAI API, Ollama (local), or any combination
- **Per-stage models** — Opus for planning, Sonnet for execution, GPT for speed, Ollama for free
- **Interactive TUI** — ratatui-based: tab navigation for model selection, arrow-key clarification, fullscreen plan review
- **Plan review with comments** — inspect files, add per-file or general comments, replan with feedback
- **Knowledge system** — auto-detects project conventions, extracts learnings after each run
- **Language-agnostic** — TypeScript, Rust, Go, Python, and any language. No hardcoded language logic.
- **Auto-fix** — if build or tests fail, re-generates with error context and retries validation
- **Structure verification** — scaffold phase catches config mismatches, missing imports, and broken types before full execution

## Setup wizard

Interactive TUI with tab-based model selection:

```
┌─────────────────────────────────────────────────────┐
│  Parton Setup — Model Configuration                 │
├─────────────────────────────────────────────────────┤
│  ✓ Default │  · Planning │  ✓ Execution │  · Judge  │
├─────────────────────────────────────────────────────┤
│  Execution — Select model                           │
│                                                     │
│    Claude / claude-sonnet-4-6                        │
│  ● Claude / claude-opus-4-6                          │
│    Codex / gpt-5.4                                   │
│❯   OpenAI API / gpt-4o-mini                          │
│    Ollama / qwen2.5-coder:7b                         │
├─────────────────────────────────────────────────────┤
│  Default: sonnet │ Plan: default │ Exec: gpt-4o-mini │
└─────────────────────────────────────────────────────┘
```

## Configuration

Parton creates a `parton.toml` in your project root:

```toml
[project]
name = "my-app"

[models.default]
access = "cli"
command = "claude"
model = "claude-sonnet-4-6"

[models.execution]
provider = "openai"
model = "gpt-4o-mini"
access = "api"
env_key = "PARTON_OPENAI_KEY"
```

### Supported providers

| Provider | Access | Setup |
|----------|--------|-------|
| Claude | CLI | Install [Claude Code](https://claude.ai/code) |
| Codex | CLI | `npm i -g @openai/codex` |
| OpenAI API | API key | `export OPENAI_API_KEY=sk-...` |
| Ollama | Local | `brew install ollama && ollama pull qwen2.5-coder:7b` |

Mix and match — Opus for planning, API for fast parallel execution, Ollama for free local inference.

## Commands

```
parton run <prompt>     Generate code from a prompt
parton setup            Configure providers and models
parton version          Show version
```

## Plan review

Fullscreen ratatui view with file browser, detail pane, and inline comments:

```
┌─────────────────────────────────────────────────────────────┐
│ Plan Review — 14 files  │  2 comments  │  Enter = replan    │
├───────────────────┬─────────────────────────────────────────┤
│ Files             │ Detail                                  │
│                   │                                         │
│❯+ src/App.tsx     │ Path: src/App.tsx                       │
│ + src/types.ts    │ Action: Create                          │
│ + src/hooks/...   │                                         │
│ 💬 src/TodoItem   │ Exports: App                            │
│ + package.json    │ Imports: useTodos from hooks/useTodos   │
├───────────────────┴─────────────────────────────────────────┤
│ Comments (2)                                                │
│   [src/TodoItem.tsx] Add edit functionality                  │
│   [GENERAL] Use Tailwind v4 syntax                          │
├─────────────────────────────────────────────────────────────┤
│ ↑↓ navigate  c file comment  g general  Enter replan        │
└─────────────────────────────────────────────────────────────┘
```

- **Enter** with no comments → approve and execute
- **c** → comment on selected file
- **g** → general comment on the whole plan
- **Enter** with comments → replan (AI revises based on your feedback)
- **r** → reject

## Architecture

```
crates/
├── parton-core/        Shared types, config, provider trait
├── parton-providers/   OpenAI API, Ollama, CLI wrappers (claude, codex)
├── parton-planner/     Skeleton planning, enrichment, clarification, validation
├── parton-executor/    Parallel scaffold+execution, compliance, structure check
├── parton-knowledge/   Convention detection, learning extraction
├── parton-graph/       SQLite code graph (files, symbols, relationships)
└── parton-cli/         CLI entry point, ratatui TUI, commands
```

**Principles:**
- Crates are separate concerns — providers don't know about planning, planner doesn't know about execution
- Core stays sync — async only at the provider/executor boundary
- No LLM for deterministic work — graph building, compliance checking, file writing are all deterministic
- Fully language-agnostic — zero hardcoded language logic anywhere

## Development

```bash
cargo check && cargo clippy -- -D warnings && cargo test

# Build release binaries
./scripts/build-release.sh macos       # macOS arm64 + x86_64
./scripts/build-release.sh linux       # Linux x86_64 via Docker
./scripts/build-release.sh all         # All targets

# Run from source
cargo run -- run "create a hello world app"
```

## License

MIT
