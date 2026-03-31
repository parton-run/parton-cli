//! System prompt and per-file prompt builder for the executor.
//!
//! Battle-tested prompts from production Parton — do NOT simplify.

use std::path::Path;

use parton_core::{FileAction, FilePlan, RunPlan};

/// System prompt for file executor agents.
pub const SYSTEM_PROMPT: &str = "\
You are a file writer. You receive a task and produce file content.

YOUR OUTPUT FORMAT:
Line 1: ===FILE_START===
Lines 2..N: the COMPLETE source code of the file
Last line: ===FILE_END===

That is your ENTIRE response. Nothing before ===FILE_START===. Nothing after ===FILE_END===.

If your output does not start with ===FILE_START=== the system rejects it immediately.

RULES:
1. Between the markers: ONLY source code. Zero English words. Zero explanations.
2. EDIT tasks: you receive the current file content. Output the COMPLETE updated file (every line, first to last) with your changes applied.
3. CREATE tasks: output the complete new file.
4. If no changes are needed for an EDIT: output the original file unchanged between the markers.
5. NEVER output 'no changes needed' or any commentary. Just the file.
6. NEVER use markdown fences (```). Just raw code.
7. Export ALL symbols listed in MANDATORY Exports with EXACT names. Other parallel files import these names. If you rename or omit any, the build fails.
8. Import ALL symbols listed in Import Interfaces with EXACT names from the specified paths.";

/// Build the per-file prompt from a file plan and run plan.
///
/// Includes conventions, goal, exports contract, import interfaces,
/// context files, and output instructions.
/// Build the per-file prompt with optional graph context.
///
/// When `graph_context` is provided, it is inserted between the
/// import interfaces and context files sections.
pub fn build_file_prompt_with_graph(
    file: &FilePlan,
    plan: &RunPlan,
    project_root: &Path,
    graph_context: Option<&str>,
) -> String {
    let mut prompt = build_file_prompt(file, plan, project_root);

    if let Some(ctx) = graph_context {
        if !ctx.is_empty() {
            // Insert graph context before the OUTPUT INSTRUCTION section.
            if let Some(pos) = prompt.find("## OUTPUT INSTRUCTION") {
                prompt.insert_str(pos, &format!("{ctx}\n\n"));
            } else {
                prompt.push_str(&format!("\n\n{ctx}"));
            }
        }
    }

    prompt
}

pub fn build_file_prompt(file: &FilePlan, plan: &RunPlan, project_root: &Path) -> String {
    let mut sections = Vec::new();

    // Project conventions — must be followed by every executor.
    if !plan.conventions.is_empty() {
        let rules = plan
            .conventions
            .iter()
            .map(|c| format!("- {c}"))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!(
            "## Project Conventions (MANDATORY — follow these exactly)\n{rules}"
        ));
    }

    // Action and path.
    let action_str = match file.action {
        FileAction::Create => "CREATE",
        FileAction::Edit => "EDIT",
    };
    sections.push(format!("# {action_str}: {}", file.path));

    // Goal.
    sections.push(format!("## Goal\n{}", file.goal));

    // Must-export symbols — strict contract.
    if !file.must_export.is_empty() {
        let exports = file
            .must_export
            .iter()
            .map(|s| format!("- `{s}`"))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!(
            "## MANDATORY Exports (contract — do NOT deviate)\n\
             This file MUST export ALL of these symbols with EXACT names:\n{exports}\n\n\
             Other files import these by name. If you rename or omit any, the build WILL fail."
        ));
    }

    // Import interfaces — what's available from other files.
    if !file.must_import_from.is_empty() {
        let import_lines: Vec<String> = file
            .must_import_from
            .iter()
            .map(|imp| {
                let symbols = imp.symbols.join("`, `");
                format!("- from `{}`: `{symbols}`", imp.path)
            })
            .collect();
        sections.push(format!(
            "## Import Interfaces (available from parallel files)\n\
             These symbols will exist in these files (created in parallel with yours).\n\
             Import them by EXACT name:\n{}",
            import_lines.join("\n")
        ));
    }

    // Pre-read context files from disk.
    if !file.context_files.is_empty() {
        let mut ctx_parts = Vec::new();
        for ctx_path in &file.context_files {
            let full_path = project_root.join(ctx_path);
            match std::fs::read_to_string(&full_path) {
                Ok(content) => {
                    ctx_parts.push(format!("### {ctx_path}\n```\n{content}\n```"));
                }
                Err(_) => {
                    ctx_parts.push(format!("### {ctx_path}\n(file not found on disk)"));
                }
            }
        }
        sections.push(format!(
            "## Context Files (read-only reference)\n{}",
            ctx_parts.join("\n\n")
        ));
    }

    // Current file content for edit actions.
    if file.action == FileAction::Edit {
        let full_path = project_root.join(&file.path);
        match std::fs::read_to_string(&full_path) {
            Ok(content) => {
                sections.push(format!("## Current File Content\n```\n{content}\n```"));
            }
            Err(_) => {
                sections.push(
                    "## Current File Content\n(file not found — treating as new file)".into(),
                );
            }
        }
    }

    // Remind contract + protocol at the end.
    sections.push(
        "## OUTPUT INSTRUCTION\n\
         1. Implement the goal EXACTLY as described above.\n\
         2. Export ALL symbols listed in MANDATORY Exports with EXACT names.\n\
         3. Import symbols from Import Interfaces with EXACT names.\n\
         4. Respond with EXACTLY:\n\
         ===FILE_START===\n\
         (complete file content — every line)\n\
         ===FILE_END===\n\n\
         No prose. No explanation. Just markers and code."
            .to_string(),
    );

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use parton_core::ImportRef;

    fn test_plan() -> RunPlan {
        RunPlan {
            summary: "test".into(),
            conventions: vec!["Use named exports".into()],
            files: vec![],
            install_command: None,
            check_commands: vec![],
            validation_commands: vec![],
            done: true,
            remaining_work: None,
        }
    }

    #[test]
    fn build_prompt_includes_conventions() {
        let file = FilePlan {
            path: "src/app.ts".into(),
            action: FileAction::Create,
            goal: "Create app".into(),
            must_export: vec![],
            must_import_from: vec![],
            context_files: vec![],
            scaffold_only: false,
            is_test: false,
        };
        let dir = tempfile::tempdir().unwrap();
        let prompt = build_file_prompt(&file, &test_plan(), dir.path());
        assert!(prompt.contains("MANDATORY — follow these exactly"));
        assert!(prompt.contains("Use named exports"));
    }

    #[test]
    fn build_prompt_includes_export_contract() {
        let file = FilePlan {
            path: "src/types.ts".into(),
            action: FileAction::Create,
            goal: "Create types".into(),
            must_export: vec!["Todo".into(), "Filter".into()],
            must_import_from: vec![],
            context_files: vec![],
            scaffold_only: false,
            is_test: false,
        };
        let dir = tempfile::tempdir().unwrap();
        let prompt = build_file_prompt(&file, &test_plan(), dir.path());
        assert!(prompt.contains("MANDATORY Exports (contract — do NOT deviate)"));
        assert!(prompt.contains("`Todo`"));
        assert!(prompt.contains("`Filter`"));
        assert!(prompt.contains("build WILL fail"));
    }

    #[test]
    fn build_prompt_includes_import_interfaces() {
        let file = FilePlan {
            path: "src/app.ts".into(),
            action: FileAction::Create,
            goal: "Create app".into(),
            must_export: vec![],
            must_import_from: vec![ImportRef {
                path: "src/types.ts".into(),
                symbols: vec!["Todo".into()],
            }],
            context_files: vec![],
            scaffold_only: false,
            is_test: false,
        };
        let dir = tempfile::tempdir().unwrap();
        let prompt = build_file_prompt(&file, &test_plan(), dir.path());
        assert!(prompt.contains("Import Interfaces (available from parallel files)"));
        assert!(prompt.contains("from `src/types.ts`: `Todo`"));
    }

    #[test]
    fn build_prompt_reads_context_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), r#"{"strict": true}"#).unwrap();

        let file = FilePlan {
            path: "src/app.ts".into(),
            action: FileAction::Create,
            goal: "Create app".into(),
            must_export: vec![],
            must_import_from: vec![],
            context_files: vec!["tsconfig.json".into()],
            scaffold_only: false,
            is_test: false,
        };
        let prompt = build_file_prompt(&file, &test_plan(), dir.path());
        assert!(prompt.contains(r#"{"strict": true}"#));
    }

    #[test]
    fn build_prompt_output_instruction() {
        let file = FilePlan {
            path: "src/app.ts".into(),
            action: FileAction::Create,
            goal: "test".into(),
            must_export: vec![],
            must_import_from: vec![],
            context_files: vec![],
            scaffold_only: false,
            is_test: false,
        };
        let dir = tempfile::tempdir().unwrap();
        let prompt = build_file_prompt(&file, &test_plan(), dir.path());
        assert!(prompt.contains("===FILE_START==="));
        assert!(prompt.contains("===FILE_END==="));
        assert!(prompt.contains("No prose. No explanation"));
    }
}
