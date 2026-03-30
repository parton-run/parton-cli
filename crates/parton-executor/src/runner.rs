//! Parallel file execution engine.

use std::path::Path;
use std::time::Instant;

use parton_core::{FileResult, FilePlan, ModelProvider, RunPlan};

use crate::output::clean_output;
use crate::prompt::{build_file_prompt, SYSTEM_PROMPT};

/// Execute all files in a plan in parallel, streaming results as they complete.
///
/// Calls `on_result` for each file as it finishes (order is non-deterministic).
pub async fn execute_streaming(
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &Path,
    on_result: &dyn Fn(&FileResult),
) -> Vec<FileResult> {
    use futures_util::stream::{FuturesUnordered, StreamExt};

    let mut futures: FuturesUnordered<_> = plan
        .files
        .iter()
        .map(|file| execute_file(file, plan, provider, project_root))
        .collect();

    let mut results = Vec::with_capacity(plan.files.len());
    while let Some(result) = futures.next().await {
        on_result(&result);
        results.push(result);
    }
    results
}

/// Execute all files in a plan in parallel (no streaming callback).
pub async fn execute(
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &Path,
) -> Vec<FileResult> {
    execute_streaming(plan, provider, project_root, &|_| {}).await
}

/// Execute a single file from the plan.
async fn execute_file(
    file: &FilePlan,
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &Path,
) -> FileResult {
    let prompt = build_file_prompt(file, plan, project_root);
    let start = Instant::now();

    match provider.send(SYSTEM_PROMPT, &prompt, false).await {
        Ok(response) => {
            let content = clean_output(&response.content);
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let tokens = response.prompt_tokens + response.completion_tokens;
            let success = !content.is_empty();

            FileResult {
                path: file.path.clone(),
                content,
                success,
                error: if success { None } else { Some("empty output".into()) },
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

    struct FailingProvider;

    #[async_trait]
    impl ModelProvider for FailingProvider {
        async fn send(&self, _: &str, _: &str, _: bool) -> Result<ModelResponse, ProviderError> {
            Err(ProviderError::Other("test error".into()))
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
            }],
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
        let results = execute(&test_plan(), &provider, dir.path()).await;

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(results[0].content, "const x = 1;");
        assert_eq!(results[0].tokens_used, 15);
    }

    #[tokio::test]
    async fn execute_empty_output() {
        let provider = MockProvider {
            response: "no markers here".into(),
        };
        let dir = tempfile::tempdir().unwrap();
        let results = execute(&test_plan(), &provider, dir.path()).await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert_eq!(results[0].error.as_deref(), Some("empty output"));
    }

    #[tokio::test]
    async fn execute_provider_error() {
        let dir = tempfile::tempdir().unwrap();
        let results = execute(&test_plan(), &FailingProvider, dir.path()).await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].error.as_deref().unwrap().contains("test error"));
    }
}
