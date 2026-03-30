//! Two-phase planning: skeleton (contracts) → enrich (detailed goals) in parallel.

use parton_core::{FilePlan, ModelProvider, ProviderError, RunPlan};

/// System prompt for skeleton planning — full contracts, minimal goals.
const SKELETON_PROMPT: &str = r#"You are a software architect producing a JSON execution plan SKELETON.

You MUST return a single valid JSON object. No text before or after. No markdown fences.

THE JSON SCHEMA:
{
  "summary": "string — one line describing the feature",
  "conventions": ["string — project-wide rules ALL files must follow"],
  "files": [
    {
      "path": "string — relative file path",
      "action": "Create" or "Edit",
      "goal": "string — ONE SENTENCE summary only (details come later)",
      "must_export": ["string — EXACT symbol names this file MUST export"],
      "must_import_from": [
        {"path": "string — EXACT path matching another file's path field", "symbols": ["string"]}
      ],
      "context_files": ["string — existing files the executor needs"],
      "scaffold_only": true or false
    }
  ],
  "install_command": "string or null — shell command to install dependencies (e.g. 'npm install', 'cargo fetch', 'pip install -r requirements.txt')",
  "check_commands": ["string — fast commands to verify structure compiles (e.g. 'npx tsc --noEmit', 'cargo check', 'go build ./...')"],
  "validation_commands": ["string — full validation: build + tests (e.g. 'npm run build', 'npm test', 'cargo test')"],
  "done": true or false,
  "remaining_work": "string or null"
}

THIS IS A SKELETON — the "goal" field must define the file's COMPLETE INTERFACE in compact form.
Include: exact function/method names, exact prop names, exact callback names, exact return type shapes.
Do NOT include implementation details — just the interface contract.

Example goals:
- "Export function useTodos(): { todos: Todo[], addTodo(text: string): void, toggleTodo(id: string): void, deleteTodo(id: string): void, filter: FilterStatus, setFilter(f: FilterStatus): void }"
- "Export function TodoItem({ todo, onToggle, onDelete }: { todo: Todo, onToggle(id: string): void, onDelete(id: string): void }): JSX.Element — renders checkbox, text, delete button"
- "Export function App(): JSX.Element — uses useTodos(), renders TodoInput({ onAdd: addTodo }), TodoList({ todos, onToggle: toggleTodo, onDelete: deleteTodo }), FilterBar({ filter, onFilterChange: setFilter })"
- "Export type Todo = { id: string, text: string, completed: boolean, createdAt: number }"

The goal is the CONTRACT between parallel agents. If you write onToggle in TodoItem but toggleTodo in App, the build breaks. BE PRECISE.

SCAFFOLD_ONLY FLAG:
Set "scaffold_only": true for files where the scaffold IS the final version:
- Config files (package.json, tsconfig.json, vite.config.*, tailwind.config.*, etc.)
- CSS files, HTML entry points, setup files
- Any file that doesn't contain application logic

Set "scaffold_only": false for files that need full implementation:
- Source files with business logic, components, hooks, utilities
- Test files that need real test cases

Files with scaffold_only=true will NOT go through the final implementation step.

CRITICAL — INTERFACE PRECISION:
When file A passes data to file B (e.g. component props, function parameters), the skeleton goal MUST specify the EXACT prop/parameter names. Example:
- BAD goal: "TodoItem component that can toggle and delete"
- GOOD goal: "TodoItem({ todo, onToggle, onDelete }: Props) — receives todo object, onToggle callback, onDelete callback"
If you don't specify exact names, parallel agents will use different names and the code won't compile.

COMPLETENESS CHECK — CRITICAL:
Every file that is imported by another file in the plan MUST exist in the plan.
If main.tsx imports from "./components/App", then src/components/App.tsx MUST be in the plan.
If App.tsx renders TodoItem, then the file containing TodoItem MUST be in the plan.
Missing files = broken imports = build failure. Trace ALL imports and ensure every target exists.

EXISTING CODE: If the Project Context shows existing files, do NOT recreate them.
Use context_files to reference existing files the executor needs to see.
If an existing file already exports a symbol you need, import it — don't create a duplicate.

ALSO — must_export and must_import_from MUST be COMPLETE and PRECISE:
- Every exported symbol name must be exact
- Every import must reference the EXACT path of the source file in this plan
- If file B needs symbol X from file A, then A's must_export MUST include X

CONVENTIONS must be complete — import style, export style, naming, testing framework, etc.

TESTING IS MANDATORY. For EVERY source file that contains logic you MUST include a corresponding test file in the SAME plan. No exceptions.
  Example: if you create src/hooks/useTodos.ts, you MUST also create src/hooks/useTodos.test.ts.
  Example: if you create src/components/TodoItem.tsx, you MUST also create src/components/TodoItem.test.tsx.
  Example: if you create lib/auth/roles.ts, you MUST also create lib/auth/roles.test.ts.
  Config files (package.json, tsconfig.json, vite.config.ts, index.html, CSS files) do NOT need tests.
  A plan with 8 logic files and only 1 test file is WRONG. It should have 8 test files.

Use NAMED exports everywhere (export function X, export const Y). NEVER use default exports. This prevents import mismatches between parallel files.

PHASING: max 15 files per phase. Set done=false if more needed.

MINIMIZE FILE COUNT: Use the FEWEST files possible. Combine related logic.
BAD: separate types.ts, types.test.ts, storage.ts, storage.test.ts, useTodos.ts, useTodos.test.ts = 6 files for todo logic
GOOD: types.ts (types), hooks/useTodos.ts (hook with storage), hooks/useTodos.test.ts (tests) = 3 files
Don't create utility files unless they have 3+ consumers. Inline small helpers.

ALL scripts must be non-interactive and terminate on their own (CI=true, no stdin).
CRITICAL: if the test runner defaults to watch mode you MUST configure it to run once and exit.

VERSIONS AND CONFIGURATION — CRITICAL:
- Use LATEST STABLE versions of ALL dependencies, tools, and frameworks. Not outdated, not canary, not beta.
- Config files (tsconfig, build config, etc.) MUST be compatible with the dependency versions you specify.
- If you add a dependency, the config files MUST support it. Example: if using React 19, the TypeScript config must support JSX transform for React 19. If using Tailwind v4, the config must use v4 syntax.
- The build command in validation_commands MUST pass with the exact config and dependency versions you chose.
- THINK about version compatibility: new major versions often have breaking config changes. Use config patterns that match the versions.
- Test that your mental model is consistent: dependency versions ↔ config files ↔ build commands ↔ source code patterns must all agree.

Return valid JSON only."#;

/// System prompt for enriching a single file's goal.
const ENRICH_PROMPT: &str = r#"You are a code specification writer. Given a file's skeleton goal (which already defines the interface contract), EXPAND it with implementation details.

The skeleton goal already has: exact function names, prop names, return types.
You must ADD:
- Behavior details: what each function actually does, edge cases to handle
- State management: what state is kept, how it's updated
- UI details: what elements to render, layout, interactions
- Test specifics: what scenarios to test, expected inputs and outputs
- Error handling: what can go wrong and how to handle it

Your response must be ONLY the expanded goal text — a plain string. No JSON, no markdown, no code blocks.

CRITICAL: Do NOT change any function names, prop names, or type names from the skeleton goal.
The skeleton goal is the interface contract. You are adding implementation details, not changing the interface."#;

/// Generate a skeleton plan (contracts only, minimal goals).
pub async fn generate_skeleton(
    prompt: &str,
    project_context: &str,
    provider: &dyn ModelProvider,
) -> Result<RunPlan, ProviderError> {
    let system = if project_context.is_empty() {
        SKELETON_PROMPT.to_string()
    } else {
        format!("{SKELETON_PROMPT}\n\n# Project Context\n{project_context}")
    };

    let response = provider.send(&system, prompt, false).await?;

    crate::parse_plan(&response.content)
        .map_err(|e| ProviderError::Other(format!("failed to parse skeleton plan: {e}")))
}

/// Enrich all file goals in parallel.
///
/// Takes a skeleton plan, sends each file to the LLM for detailed goal
/// enrichment, and returns the plan with enriched goals.
pub async fn enrich_plan(
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    on_enriched: &dyn Fn(&str),
) -> Result<RunPlan, ProviderError> {
    use futures_util::stream::{FuturesUnordered, StreamExt};

    let conventions_text = if plan.conventions.is_empty() {
        String::new()
    } else {
        format!(
            "Project conventions:\n{}",
            plan.conventions
                .iter()
                .map(|c| format!("- {c}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    // Only enrich files that need final execution — skip scaffold_only files.
    let files_to_enrich: Vec<&parton_core::FilePlan> =
        plan.files.iter().filter(|f| !f.scaffold_only).collect();

    // Immediately mark scaffold_only files as enriched.
    for file in plan.files.iter().filter(|f| f.scaffold_only) {
        on_enriched(&file.path);
    }

    let futures: FuturesUnordered<_> = files_to_enrich
        .iter()
        .map(|file| enrich_single_file(file, plan, &conventions_text, provider))
        .collect();

    let enriched_goals: Vec<(String, String)> = futures
        .filter_map(|result| async move { result.ok() })
        .inspect(|(path, _)| on_enriched(path))
        .collect()
        .await;

    // Merge enriched goals back into the plan.
    let mut enriched_plan = plan.clone();
    for file in &mut enriched_plan.files {
        if let Some((_, goal)) = enriched_goals.iter().find(|(p, _)| p == &file.path) {
            file.goal = goal.clone();
        }
    }

    Ok(enriched_plan)
}

/// Enrich a single file's goal via LLM.
async fn enrich_single_file(
    file: &FilePlan,
    plan: &RunPlan,
    conventions: &str,
    provider: &dyn ModelProvider,
) -> Result<(String, String), ProviderError> {
    let mut context_parts = vec![];

    if !conventions.is_empty() {
        context_parts.push(conventions.to_string());
    }

    context_parts.push(format!(
        "File: {} ({})",
        file.path,
        match file.action {
            parton_core::FileAction::Create => "Create",
            parton_core::FileAction::Edit => "Edit",
        }
    ));

    context_parts.push(format!("Current goal: {}", file.goal));

    if !file.must_export.is_empty() {
        context_parts.push(format!("Must export: {}", file.must_export.join(", ")));
    }

    if !file.must_import_from.is_empty() {
        let imports: Vec<String> = file
            .must_import_from
            .iter()
            .map(|imp| format!("from {}: {}", imp.path, imp.symbols.join(", ")))
            .collect();
        context_parts.push(format!("Imports: {}", imports.join("; ")));
    }

    // Include sibling file summaries so enrich knows the full picture.
    let siblings: Vec<String> = plan
        .files
        .iter()
        .filter(|f| f.path != file.path)
        .map(|f| {
            let exports = if f.must_export.is_empty() {
                String::new()
            } else {
                format!(" exports [{}]", f.must_export.join(", "))
            };
            format!("- {}{}", f.path, exports)
        })
        .collect();

    if !siblings.is_empty() {
        context_parts.push(format!("Other files in plan:\n{}", siblings.join("\n")));
    }

    let user_prompt = context_parts.join("\n\n");
    let response = provider.send(ENRICH_PROMPT, &user_prompt, false).await?;

    Ok((file.path.clone(), response.content.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_prompt_has_key_rules() {
        assert!(SKELETON_PROMPT.contains("COMPLETE INTERFACE"));
        assert!(SKELETON_PROMPT.contains("must_export"));
        assert!(SKELETON_PROMPT.contains("COMPLETE and PRECISE"));
        assert!(SKELETON_PROMPT.contains("TESTING IS MANDATORY"));
    }

    #[test]
    fn enrich_prompt_has_key_rules() {
        assert!(ENRICH_PROMPT.contains("implementation details"));
        assert!(ENRICH_PROMPT.contains("interface contract"));
    }
}
