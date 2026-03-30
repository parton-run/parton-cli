//! Execution result types.

use serde::{Deserialize, Serialize};

use crate::plan::RunPlan;

/// Result of executing a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    /// Relative file path that was generated.
    pub path: String,
    /// Generated file content.
    pub content: String,
    /// Whether generation succeeded.
    pub success: bool,
    /// Error message if generation failed.
    pub error: Option<String>,
    /// Tokens consumed for this file.
    pub tokens_used: u32,
    /// Time taken in milliseconds.
    pub elapsed_ms: u64,
}

/// Suggested action the user may want to run after execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostRunAction {
    /// What this action does (human-readable).
    pub description: String,
    /// Shell command to execute.
    pub command: String,
    /// Why this action is suggested.
    pub reason: String,
}

/// Result of a complete turbo run (one or more phases).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    /// The final plan that was executed.
    pub plan: RunPlan,
    /// Per-file execution results.
    pub file_results: Vec<FileResult>,
    /// Paths of files successfully written to disk.
    pub files_written: Vec<String>,
    /// Paths and error messages of files that failed.
    pub files_failed: Vec<(String, String)>,
    /// Whether validation commands passed.
    pub validation_passed: bool,
    /// Combined validation command output.
    pub validation_output: String,
    /// Total tokens consumed across all files and retries.
    pub total_tokens: u32,
    /// Total elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// Suggested post-run actions.
    pub suggested_actions: Vec<PostRunAction>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_result_success() {
        let result = FileResult {
            path: "src/App.tsx".into(),
            content: "export function App() {}".into(),
            success: true,
            error: None,
            tokens_used: 150,
            elapsed_ms: 2500,
        };
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn file_result_failure() {
        let result = FileResult {
            path: "src/broken.ts".into(),
            content: String::new(),
            success: false,
            error: Some("provider timeout".into()),
            tokens_used: 0,
            elapsed_ms: 30000,
        };
        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("provider timeout"));
    }

    #[test]
    fn post_run_action_serde() {
        let action = PostRunAction {
            description: "Install dependencies".into(),
            command: "npm install".into(),
            reason: "New package.json was created".into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: PostRunAction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "npm install");
    }
}
