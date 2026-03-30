//! Parallel file execution engine.

use std::path::Path;
use std::time::Instant;

use parton_core::{FilePlan, FileResult, ModelProvider, RunPlan};

use crate::output::clean_output;
use crate::prompt::build_file_prompt;
use crate::scaffold;

/// Execution mode determines which system prompt is used.
#[derive(Clone, Copy, Debug)]
pub enum ExecMode {
    /// Combined scaffold+enrich: returns goal + minimal compilable code.
    Scaffold,
    /// Full implementation (single-pass, no scaffold phase).
    Full,
    /// Final implementation — preserve existing scaffold structure.
    Final,
}

/// Result of a scaffold+enrich execution for one file.
#[derive(Debug, Clone)]
pub struct ScaffoldResult {
    /// File path.
    pub path: String,
    /// Enriched goal (from ===GOAL_START=== / ===GOAL_END===).
    pub enriched_goal: String,
    /// Scaffold code (from ===FILE_START=== / ===FILE_END===).
    pub code: String,
    /// Whether execution succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
    /// Tokens consumed.
    pub tokens_used: u32,
    /// Time in ms.
    pub elapsed_ms: u64,
}

/// Execute scaffold+enrich in parallel. Returns ScaffoldResults with enriched goals + code.
pub async fn scaffold_streaming(
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &Path,
    on_result: &dyn Fn(&ScaffoldResult),
) -> Vec<ScaffoldResult> {
    use futures_util::stream::{FuturesUnordered, StreamExt};

    let mut futures: FuturesUnordered<_> = plan
        .files
        .iter()
        .map(|file| scaffold_single(file, plan, provider, project_root))
        .collect();

    let mut results = Vec::with_capacity(plan.files.len());
    while let Some(result) = futures.next().await {
        on_result(&result);
        results.push(result);
    }
    results
}

/// Scaffold+enrich a single file.
async fn scaffold_single(
    file: &FilePlan,
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &Path,
) -> ScaffoldResult {
    let prompt = build_file_prompt(file, plan, project_root);
    let start = Instant::now();

    match provider
        .send(scaffold::SCAFFOLD_PROMPT, &prompt, false)
        .await
    {
        Ok(response) => {
            let (goal, code) = scaffold::parse_scaffold_output(&response.content);
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let tokens = response.prompt_tokens + response.completion_tokens;
            let success = !code.is_empty();

            ScaffoldResult {
                path: file.path.clone(),
                enriched_goal: goal,
                code,
                success,
                error: if success {
                    None
                } else {
                    Some("empty scaffold output".into())
                },
                tokens_used: tokens,
                elapsed_ms,
            }
        }
        Err(e) => ScaffoldResult {
            path: file.path.clone(),
            enriched_goal: String::new(),
            code: String::new(),
            success: false,
            error: Some(e.to_string()),
            tokens_used: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// Execute all files in parallel, streaming results as they complete.
///
/// `structure_errors`: if non-empty, appended to Final mode prompts
/// so the executor knows what to fix.
pub async fn execute_streaming(
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &Path,
    mode: ExecMode,
    structure_errors: &str,
    on_result: &dyn Fn(&FileResult),
) -> Vec<FileResult> {
    use futures_util::stream::{FuturesUnordered, StreamExt};

    let mut futures: FuturesUnordered<_> = plan
        .files
        .iter()
        .map(|file| execute_file(file, plan, provider, project_root, mode, structure_errors))
        .collect();

    let mut results = Vec::with_capacity(plan.files.len());
    while let Some(result) = futures.next().await {
        on_result(&result);
        results.push(result);
    }
    results
}

/// Execute all files (no streaming callback).
pub async fn execute(
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &Path,
    mode: ExecMode,
) -> Vec<FileResult> {
    execute_streaming(plan, provider, project_root, mode, "", &|_| {}).await
}

/// Execute a single file.
async fn execute_file(
    file: &FilePlan,
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &Path,
    mode: ExecMode,
    structure_errors: &str,
) -> FileResult {
    let system_prompt = match mode {
        ExecMode::Scaffold => scaffold::SCAFFOLD_PROMPT,
        ExecMode::Full => crate::prompt::SYSTEM_PROMPT,
        ExecMode::Final => scaffold::FINAL_PROMPT,
    };

    let mut prompt = match mode {
        ExecMode::Final => build_final_prompt(file, plan, project_root),
        _ => build_file_prompt(file, plan, project_root),
    };

    // Inject structure check errors into final execution.
    if matches!(mode, ExecMode::Final) && !structure_errors.is_empty() {
        let relevant = extract_relevant_errors(structure_errors, &file.path);
        if !relevant.is_empty() {
            prompt.push_str(&format!(
                "\n\n## STRUCTURE CHECK ERRORS (must fix)\n\
                 The scaffold had these compilation errors. You MUST fix them:\n```\n{relevant}\n```"
            ));
        }
    }

    let start = Instant::now();

    match provider.send(system_prompt, &prompt, false).await {
        Ok(response) => {
            let content = clean_output(&response.content);
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let tokens = response.prompt_tokens + response.completion_tokens;
            let success = !content.is_empty();

            FileResult {
                path: file.path.clone(),
                content,
                success,
                error: if success {
                    None
                } else {
                    Some("empty output".into())
                },
                tokens_used: tokens,
                elapsed_ms,
            }
        }
        Err(e) => FileResult {
            path: file.path.clone(),
            content: String::new(),
            success: false,
            error: Some(e.to_string()),
            tokens_used: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// Build prompt for final execution — includes existing scaffold content.
fn build_final_prompt(file: &FilePlan, plan: &RunPlan, project_root: &Path) -> String {
    let base_prompt = build_file_prompt(file, plan, project_root);

    let scaffold_path = project_root.join(&file.path);
    let scaffold_content = std::fs::read_to_string(&scaffold_path).unwrap_or_default();

    if scaffold_content.is_empty() {
        return base_prompt;
    }

    format!(
        "{base_prompt}\n\n\
         ## EXISTING SCAFFOLD (imports and exports are VERIFIED working)\n\
         ```\n{scaffold_content}\n```\n\n\
         IMPORTANT: Keep ALL import and export statements EXACTLY as shown above.\n\
         Replace only the stub implementations with real code."
    )
}

/// Extract error lines relevant to a specific file path.
fn extract_relevant_errors(all_errors: &str, file_path: &str) -> String {
    all_errors
        .lines()
        .filter(|line| line.contains(file_path))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use parton_core::{FileAction, ModelResponse, ProviderError};

    struct MockProvider {
        response: String,
    }

    #[async_trait]
    impl ModelProvider for MockProvider {
        async fn send(&self, _: &str, _: &str, _: bool) -> Result<ModelResponse, ProviderError> {
            Ok(ModelResponse {
                content: self.response.clone(),
                prompt_tokens: 10,
                completion_tokens: 5,
            })
        }
    }

    fn test_plan() -> RunPlan {
        RunPlan {
            summary: "test".into(),
            conventions: vec![],
            files: vec![FilePlan {
                path: "src/app.ts".into(),
                action: FileAction::Create,
                goal: "Create app".into(),
                must_export: vec![],
                must_import_from: vec![],
                context_files: vec![],
                scaffold_only: false,
            }],
            install_command: None,
            check_commands: vec![],
            validation_commands: vec![],
            done: true,
            remaining_work: None,
        }
    }

    #[tokio::test]
    async fn execute_success() {
        let provider = MockProvider {
            response: "===FILE_START===\nconst x = 1;\n===FILE_END===".into(),
        };
        let dir = tempfile::tempdir().unwrap();
        let results = execute(&test_plan(), &provider, dir.path(), ExecMode::Full).await;
        assert!(results[0].success);
        assert_eq!(results[0].content, "const x = 1;");
    }

    #[tokio::test]
    async fn execute_empty_output() {
        let provider = MockProvider {
            response: "no markers".into(),
        };
        let dir = tempfile::tempdir().unwrap();
        let results = execute(&test_plan(), &provider, dir.path(), ExecMode::Full).await;
        assert!(!results[0].success);
    }

    #[test]
    fn extract_relevant_errors_filters() {
        let errors = "src/App.tsx(14,20): error TS2322: bad type\nsrc/main.tsx(1,8): unused import\nother stuff";
        let relevant = extract_relevant_errors(errors, "src/App.tsx");
        assert!(relevant.contains("TS2322"));
        assert!(!relevant.contains("main.tsx"));
    }

    #[test]
    fn extract_relevant_errors_empty() {
        let relevant = extract_relevant_errors("no matching errors", "src/missing.ts");
        assert!(relevant.is_empty());
    }
}
