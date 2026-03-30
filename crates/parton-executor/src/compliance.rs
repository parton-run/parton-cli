//! Compliance checking — verify executor output matches plan contracts.

use serde::{Deserialize, Serialize};

use parton_core::{FileResult, FilePlan, RunPlan};

/// Type of compliance issue found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueType {
    /// A required export symbol was not found in the output.
    MissingExport,
    /// The file content is empty or near-empty.
    EmptyFile,
    /// The executor reported an error.
    ExecutorError,
}

/// A compliance issue found during post-execution checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceIssue {
    /// Path of the file with the issue.
    pub file_path: String,
    /// Type of issue.
    pub issue_type: IssueType,
    /// The symbol that's missing (if applicable).
    pub symbol: Option<String>,
    /// Human-readable description.
    pub message: String,
}

/// Check a single file's output against its plan contract.
pub fn check_file(plan: &FilePlan, content: &str) -> Vec<ComplianceIssue> {
    let mut issues = Vec::new();

    if content.trim().len() < 10 {
        issues.push(ComplianceIssue {
            file_path: plan.path.clone(),
            issue_type: IssueType::EmptyFile,
            symbol: None,
            message: format!("file is empty ({} chars)", content.trim().len()),
        });
        return issues;
    }

    for symbol in &plan.must_export {
        if !symbol_present(content, symbol, &plan.path) {
            issues.push(ComplianceIssue {
                file_path: plan.path.clone(),
                issue_type: IssueType::MissingExport,
                symbol: Some(symbol.clone()),
                message: format!("required symbol '{symbol}' not found"),
            });
        }
    }

    issues
}

/// Check all file results against the plan.
pub fn check_all(plan: &RunPlan, results: &[FileResult]) -> Vec<ComplianceIssue> {
    let mut issues = Vec::new();

    for file_plan in &plan.files {
        match results.iter().find(|r| r.path == file_plan.path) {
            Some(result) if !result.success => {
                issues.push(ComplianceIssue {
                    file_path: file_plan.path.clone(),
                    issue_type: IssueType::ExecutorError,
                    symbol: None,
                    message: result.error.clone().unwrap_or_else(|| "unknown".into()),
                });
            }
            Some(result) => {
                issues.extend(check_file(file_plan, &result.content));
            }
            None => {
                issues.push(ComplianceIssue {
                    file_path: file_plan.path.clone(),
                    issue_type: IssueType::EmptyFile,
                    symbol: None,
                    message: "no result found for planned file".into(),
                });
            }
        }
    }

    issues
}

/// Check if a symbol is present in file content.
///
/// Uses language-specific patterns for JS/TS, falls back to substring match.
fn symbol_present(content: &str, symbol: &str, path: &str) -> bool {
    let is_js_ts = matches!(
        path.rsplit('.').next(),
        Some("ts" | "tsx" | "js" | "jsx" | "mts" | "mjs")
    );

    if is_js_ts {
        // "default" export: match `export default`, `module.exports`
        if symbol == "default" {
            return content.contains("export default")
                || content.contains("module.exports");
        }

        let patterns = [
            format!("export function {symbol}"),
            format!("export const {symbol}"),
            format!("export type {symbol}"),
            format!("export interface {symbol}"),
            format!("export class {symbol}"),
            format!("export enum {symbol}"),
            format!("export async function {symbol}"),
            format!("export default function {symbol}"),
            format!("export default class {symbol}"),
        ];

        for pattern in &patterns {
            if content.contains(pattern.as_str()) {
                return true;
            }
        }

        // Check re-exports: export { Symbol }
        content.lines().any(|line| {
            let t = line.trim();
            t.starts_with("export") && t.contains('{') && t.contains(symbol)
        })
    } else {
        content.contains(symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parton_core::FileAction;

    fn make_file_plan(path: &str, exports: Vec<&str>) -> FilePlan {
        FilePlan {
            path: path.into(),
            action: FileAction::Create,
            goal: "test".into(),
            must_export: exports.into_iter().map(String::from).collect(),
            must_import_from: vec![],
            context_files: vec![],
            scaffold_only: false,
        }
    }

    #[test]
    fn empty_file_detected() {
        let plan = make_file_plan("src/app.ts", vec![]);
        let issues = check_file(&plan, "");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].issue_type, IssueType::EmptyFile);
    }

    #[test]
    fn missing_export_detected() {
        let plan = make_file_plan("src/types.ts", vec!["Todo"]);
        let issues = check_file(&plan, "export const Filter = {};");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].issue_type, IssueType::MissingExport);
    }

    #[test]
    fn present_export_passes() {
        let plan = make_file_plan("src/types.ts", vec!["Todo"]);
        let issues = check_file(&plan, "export type Todo = { id: string };");
        assert!(issues.is_empty());
    }

    #[test]
    fn re_export_detected() {
        let plan = make_file_plan("src/index.ts", vec!["Todo"]);
        let issues = check_file(&plan, "export { Todo } from './types';");
        assert!(issues.is_empty());
    }

    #[test]
    fn non_js_uses_substring() {
        let plan = make_file_plan("src/main.rs", vec!["MyStruct"]);
        let issues = check_file(&plan, "pub struct MyStruct {}");
        assert!(issues.is_empty());
    }

    #[test]
    fn check_all_missing_result() {
        let plan = RunPlan {
            summary: "test".into(),
            conventions: vec![],
            files: vec![make_file_plan("src/app.ts", vec![])],
            install_command: None,
            check_commands: vec![],
            validation_commands: vec![],
            done: true,
            remaining_work: None,
        };
        let issues = check_all(&plan, &[]);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].issue_type, IssueType::EmptyFile);
    }
}
