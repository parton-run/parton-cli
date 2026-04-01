//! Contract verification for scaffold output.
//!
//! Parses generated code with tree-sitter and verifies that
//! exports and imports match the execution plan contracts.

pub mod check;

use parton_core::FilePlan;

use crate::detect;
use crate::grammar::GrammarStore;
use crate::queries;
use crate::scan::parser;

/// A contract violation found in scaffold output.
#[derive(Debug, Clone)]
pub struct ContractViolation {
    /// File path from the plan.
    pub path: String,
    /// What kind of violation.
    pub kind: ViolationKind,
    /// Human-readable details for the re-scaffold prompt.
    pub details: String,
}

/// Types of contract violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationKind {
    /// A required export is missing from the generated code.
    MissingExport,
    /// A required import is missing from the generated code.
    MissingImport,
    /// Tree-sitter failed to parse the generated code.
    ParseError,
    /// No code was generated (empty output).
    EmptyOutput,
}

/// Verify a single scaffold output against its plan contract.
///
/// Parses `code` with tree-sitter, extracts symbols and imports,
/// and compares them with the plan's `must_export` and `must_import_from`.
pub async fn verify_contract(
    code: &str,
    file: &FilePlan,
    store: &mut GrammarStore,
) -> Vec<ContractViolation> {
    let mut violations = Vec::new();

    if code.trim().is_empty() {
        violations.push(ContractViolation {
            path: file.path.clone(),
            kind: ViolationKind::EmptyOutput,
            details: "no code generated".into(),
        });
        return violations;
    }

    // Skip contract check if plan has no contracts.
    if file.must_export.is_empty() && file.must_import_from.is_empty() {
        return violations;
    }

    let lang = detect::detect_language(&file.path);
    let lang_queries = match queries::queries_for(lang) {
        Some(q) => q,
        None => return violations, // Unsupported language — skip.
    };

    let ts_lang = match store.get_or_load(lang).await {
        Ok(l) => l,
        Err(_) => return violations, // Grammar unavailable — skip.
    };

    // Check exports.
    if !file.must_export.is_empty() {
        match parser::extract_exports(code, &ts_lang, lang_queries.exports, lang) {
            Ok(symbols) => {
                let missing = check::find_missing_exports(&file.must_export, &symbols);
                for name in missing {
                    violations.push(ContractViolation {
                        path: file.path.clone(),
                        kind: ViolationKind::MissingExport,
                        details: format!("must_export '{name}' not found in generated code"),
                    });
                }
            }
            Err(e) => {
                violations.push(ContractViolation {
                    path: file.path.clone(),
                    kind: ViolationKind::ParseError,
                    details: format!("failed to parse exports: {e}"),
                });
            }
        }
    }

    // Check imports.
    if !file.must_import_from.is_empty() {
        match parser::extract_imports(code, &ts_lang, lang_queries.imports) {
            Ok(imports) => {
                let missing = check::find_missing_imports(&file.must_import_from, &imports);
                for (path, sym) in missing {
                    violations.push(ContractViolation {
                        path: file.path.clone(),
                        kind: ViolationKind::MissingImport,
                        details: format!("missing import '{sym}' from '{path}'"),
                    });
                }
            }
            Err(e) => {
                violations.push(ContractViolation {
                    path: file.path.clone(),
                    kind: ViolationKind::ParseError,
                    details: format!("failed to parse imports: {e}"),
                });
            }
        }
    }

    violations
}

/// Format violations into a string for the re-scaffold prompt.
pub fn format_violations(violations: &[ContractViolation]) -> String {
    violations
        .iter()
        .map(|v| format!("- {}: {}", v.path, v.details))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_violations_output() {
        let violations = vec![
            ContractViolation {
                path: "src/app.ts".into(),
                kind: ViolationKind::MissingExport,
                details: "must_export 'App' not found".into(),
            },
            ContractViolation {
                path: "src/app.ts".into(),
                kind: ViolationKind::MissingImport,
                details: "missing import 'User' from 'types'".into(),
            },
        ];
        let formatted = format_violations(&violations);
        assert!(formatted.contains("must_export 'App'"));
        assert!(formatted.contains("missing import 'User'"));
    }

    #[test]
    fn violation_kind_eq() {
        assert_eq!(ViolationKind::MissingExport, ViolationKind::MissingExport);
        assert_ne!(ViolationKind::MissingExport, ViolationKind::ParseError);
    }
}
