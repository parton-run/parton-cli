//! Post-scaffold contract verification and targeted re-scaffold.
//!
//! After scaffold (Step 3), verifies each file's exports/imports match
//! the plan contracts using tree-sitter. Re-scaffolds only violators.

use std::collections::HashMap;

use parton_core::{ModelProvider, RunPlan};
use parton_graph::grammar::GrammarStore;
use parton_graph::verify::{self, ContractViolation, ViolationKind};

use crate::runner::ScaffoldResult;

/// Run contract verification on scaffold results.
///
/// Returns a list of violations grouped by file path.
pub async fn check_contracts(
    results: &[ScaffoldResult],
    plan: &RunPlan,
    store: &mut GrammarStore,
) -> Vec<ContractViolation> {
    let mut all_violations = Vec::new();

    for sr in results {
        if !sr.success || sr.code.is_empty() {
            continue;
        }
        let file = match plan.files.iter().find(|f| f.path == sr.path) {
            Some(f) => f,
            None => continue,
        };

        let violations = verify::verify_contract(&sr.code, file, store).await;
        all_violations.extend(violations);
    }

    all_violations
}

/// Re-scaffold files that have contract violations.
///
/// Only re-scaffolds files with `MissingExport` or `MissingImport`
/// violations. Parse errors and empty outputs are not retryable.
pub async fn re_scaffold_violations(
    violations: &[ContractViolation],
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &std::path::Path,
    graph_contexts: &HashMap<String, String>,
    on_result: &dyn Fn(&ScaffoldResult),
) -> Vec<ScaffoldResult> {
    use futures_util::stream::{FuturesUnordered, StreamExt};

    // Collect unique file paths with retryable violations.
    let retry_paths: Vec<&str> = violations
        .iter()
        .filter(|v| {
            matches!(
                v.kind,
                ViolationKind::MissingExport | ViolationKind::MissingImport
            )
        })
        .map(|v| v.path.as_str())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if retry_paths.is_empty() {
        return vec![];
    }

    // Build a sub-plan with only the violating files.
    let mut retry_plan = plan.clone();
    retry_plan
        .files
        .retain(|f| retry_paths.contains(&f.path.as_str()));

    // Build error context from violations.
    let error_context = build_violation_context(violations);

    let futures: FuturesUnordered<_> = retry_plan
        .files
        .iter()
        .map(|file| {
            let graph_ctx = graph_contexts.get(&file.path).map(|s| s.as_str());
            re_scaffold_single(
                file,
                &retry_plan,
                provider,
                project_root,
                graph_ctx,
                &error_context,
            )
        })
        .collect();

    let mut results = Vec::new();
    futures
        .for_each(|result| {
            on_result(&result);
            results.push(result);
            async {}
        })
        .await;
    results
}

/// Re-scaffold a single file with violation context.
async fn re_scaffold_single(
    file: &parton_core::FilePlan,
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &std::path::Path,
    graph_context: Option<&str>,
    error_context: &str,
) -> ScaffoldResult {
    let mut prompt =
        crate::prompt::build_file_prompt_with_graph(file, plan, project_root, graph_context);

    // Inject contract violation errors.
    prompt.push_str(&format!(
        "\n\n## CONTRACT VIOLATIONS (must fix)\n\
         The previous scaffold had these contract violations:\n{error_context}\n\n\
         You MUST fix all violations. Export ALL required symbols. Import ALL required symbols."
    ));

    let start = std::time::Instant::now();
    match provider
        .send(crate::scaffold::SCAFFOLD_PROMPT, &prompt, false)
        .await
    {
        Ok(response) => {
            let (goal, code) = crate::scaffold::parse_scaffold_output(&response.content);
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let tokens = response.prompt_tokens + response.completion_tokens;
            ScaffoldResult {
                path: file.path.clone(),
                enriched_goal: goal,
                code: code.clone(),
                success: !code.is_empty(),
                error: None,
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

/// Format violations into a compact error string for the prompt.
fn build_violation_context(violations: &[ContractViolation]) -> String {
    verify::format_violations(violations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_violation_context_formats() {
        let violations = vec![ContractViolation {
            path: "src/app.ts".into(),
            kind: ViolationKind::MissingExport,
            details: "must_export 'App' not found".into(),
        }];
        let ctx = build_violation_context(&violations);
        assert!(ctx.contains("App"));
    }
}
