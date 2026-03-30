//! Plan validation — verify a [`RunPlan`] is well-formed before execution.

use std::collections::HashSet;
use std::path::Path;

use parton_core::RunPlan;

/// Error type for plan validation failures.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    /// Plan has no files to process.
    #[error("plan has no file tasks")]
    EmptyPlan,

    /// Duplicate file path in the plan.
    #[error("duplicate file path: {0}")]
    DuplicatePath(String),

    /// A file has an empty goal.
    #[error("file '{0}' has an empty goal")]
    EmptyGoal(String),

    /// Plan has logic files but no test files.
    #[error("plan has {0:?} logic files but no test files — tests are mandatory")]
    MissingTests(Vec<String>),

    /// An import references a file not in the plan and not on disk.
    #[error("file '{file}' imports from '{import}' which is neither in the plan nor on disk")]
    InvalidImport {
        /// The file that has the bad import.
        file: String,
        /// The import path that doesn't resolve.
        import: String,
    },
}

/// Validate that a plan is well-formed and ready for execution.
///
/// Checks:
/// - Plan has at least one file
/// - No duplicate paths
/// - All goals are non-empty
/// - All `must_import_from` paths exist in the plan or on disk
pub fn validate_plan(plan: &RunPlan, project_root: &Path) -> Result<(), ValidationError> {
    if plan.files.is_empty() {
        return Err(ValidationError::EmptyPlan);
    }

    let mut seen = HashSet::new();
    for file in &plan.files {
        if !seen.insert(&file.path) {
            return Err(ValidationError::DuplicatePath(file.path.clone()));
        }
    }

    for file in &plan.files {
        if file.goal.trim().is_empty() {
            return Err(ValidationError::EmptyGoal(file.path.clone()));
        }
    }

    // Check test file coverage: logic files must have corresponding test files.
    // Uses the planner-provided `is_test` and `scaffold_only` flags — no
    // language-specific heuristics.
    let logic_files: Vec<&str> = plan
        .files
        .iter()
        .filter(|f| !f.is_test && !f.scaffold_only)
        .map(|f| f.path.as_str())
        .collect();
    let has_tests = plan.files.iter().any(|f| f.is_test);

    if !logic_files.is_empty() && !has_tests {
        return Err(ValidationError::MissingTests(
            logic_files.iter().map(|s| s.to_string()).collect(),
        ));
    }

    let plan_paths: HashSet<&str> = plan.files.iter().map(|f| f.path.as_str()).collect();
    for file in &plan.files {
        for import in &file.must_import_from {
            let in_plan = plan_paths.contains(import.path.as_str());
            let on_disk = project_root.join(&import.path).exists();
            if !in_plan && !on_disk {
                return Err(ValidationError::InvalidImport {
                    file: file.path.clone(),
                    import: import.path.clone(),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
