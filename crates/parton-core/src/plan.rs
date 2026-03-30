//! Turbo execution plan types.
//!
//! These types define the contract between the planner and executor.
//! The planner produces a [`TurboRunPlan`], the executor consumes it.

use serde::{Deserialize, Serialize};

/// A reference to symbols imported from another file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRef {
    /// File path to import from (must match a file's `path` in the plan).
    pub path: String,
    /// Exact symbol names to import.
    pub symbols: Vec<String>,
}

/// Whether a file should be created or edited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileAction {
    /// Create a new file from scratch.
    Create,
    /// Edit an existing file.
    Edit,
}

/// Execution plan for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePlan {
    /// Relative file path (e.g. `src/types/todo.ts`).
    pub path: String,
    /// Whether to create or edit this file.
    pub action: FileAction,
    /// Precise description of what to implement, including exact signatures.
    pub goal: String,
    /// Symbols this file MUST export.
    #[serde(default)]
    pub must_export: Vec<String>,
    /// Symbols this file expects from other files in the plan.
    #[serde(default)]
    pub must_import_from: Vec<ImportRef>,
    /// Read-only files the executor needs for context.
    #[serde(default)]
    pub context_files: Vec<String>,
    /// If true, scaffold output is final — skip enrichment and final execution.
    /// Use for config files, CSS, HTML, and other static files.
    #[serde(default)]
    pub scaffold_only: bool,
}

/// A complete execution plan for one phase of work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPlan {
    /// One-line summary of what this plan does.
    pub summary: String,
    /// Project-wide conventions all executors must follow.
    #[serde(default)]
    pub conventions: Vec<String>,
    /// Files to generate in this phase.
    pub files: Vec<FilePlan>,
    /// Shell command to install dependencies (e.g. "npm install", "cargo fetch").
    #[serde(default)]
    pub install_command: Option<String>,
    /// Shell commands to check structure compiles (e.g. "npx tsc --noEmit").
    #[serde(default)]
    pub check_commands: Vec<String>,
    /// Shell commands for full validation — build + tests (e.g. "npm run build", "npm test").
    #[serde(default)]
    pub validation_commands: Vec<String>,
    /// True if this plan completes the entire task.
    #[serde(default = "default_done")]
    pub done: bool,
    /// If not done, what remains for the next phase.
    #[serde(default)]
    pub remaining_work: Option<String>,
}

fn default_done() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_ref_serde_roundtrip() {
        let import = ImportRef {
            path: "src/types/todo.ts".into(),
            symbols: vec!["Todo".into(), "FilterStatus".into()],
        };
        let json = serde_json::to_string(&import).unwrap();
        let parsed: ImportRef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, import);
    }

    #[test]
    fn file_action_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&FileAction::Create).unwrap(),
            r#""Create""#
        );
        assert_eq!(
            serde_json::to_string(&FileAction::Edit).unwrap(),
            r#""Edit""#
        );
    }

    #[test]
    fn run_plan_defaults() {
        let json = r#"{
            "summary": "test plan",
            "files": [],
            "validation_commands": []
        }"#;
        let plan: RunPlan = serde_json::from_str(json).unwrap();
        assert!(plan.done);
        assert!(plan.remaining_work.is_none());
        assert!(plan.conventions.is_empty());
    }

    #[test]
    fn file_plan_with_all_fields() {
        let plan = FilePlan {
            path: "src/App.tsx".into(),
            action: FileAction::Create,
            goal: "Export function App()".into(),
            must_export: vec!["App".into()],
            must_import_from: vec![ImportRef {
                path: "src/types/todo.ts".into(),
                symbols: vec!["Todo".into()],
            }],
            context_files: vec!["tsconfig.json".into()],
            scaffold_only: false,
        };
        let json = serde_json::to_string(&plan).unwrap();
        let parsed: FilePlan = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, "src/App.tsx");
        assert_eq!(parsed.must_export.len(), 1);
        assert_eq!(parsed.must_import_from.len(), 1);
    }
}
