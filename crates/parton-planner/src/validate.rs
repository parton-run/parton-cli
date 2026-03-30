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
mod tests {
    use super::*;
    use parton_core::{FileAction, FilePlan, ImportRef};

    fn make_plan(files: Vec<FilePlan>) -> RunPlan {
        RunPlan {
            summary: "test".into(),
            conventions: vec![],
            files,
            validation_commands: vec![],
            done: true,
            remaining_work: None,
        }
    }

    fn simple_file(path: &str) -> FilePlan {
        FilePlan {
            path: path.into(),
            action: FileAction::Create,
            goal: "do something".into(),
            must_export: vec![],
            must_import_from: vec![],
            context_files: vec![],
        }
    }

    #[test]
    fn valid_plan_passes() {
        let plan = make_plan(vec![simple_file("src/app.ts")]);
        let dir = tempfile::tempdir().unwrap();
        assert!(validate_plan(&plan, dir.path()).is_ok());
    }

    #[test]
    fn empty_plan_fails() {
        let plan = make_plan(vec![]);
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            validate_plan(&plan, dir.path()),
            Err(ValidationError::EmptyPlan)
        ));
    }

    #[test]
    fn duplicate_path_fails() {
        let plan = make_plan(vec![
            simple_file("src/app.ts"),
            simple_file("src/app.ts"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            validate_plan(&plan, dir.path()),
            Err(ValidationError::DuplicatePath(_))
        ));
    }

    #[test]
    fn empty_goal_fails() {
        let mut file = simple_file("src/app.ts");
        file.goal = "  ".into();
        let plan = make_plan(vec![file]);
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            validate_plan(&plan, dir.path()),
            Err(ValidationError::EmptyGoal(_))
        ));
    }

    #[test]
    fn import_in_plan_passes() {
        let mut file_b = simple_file("src/b.ts");
        file_b.must_import_from = vec![ImportRef {
            path: "src/a.ts".into(),
            symbols: vec!["Foo".into()],
        }];
        let plan = make_plan(vec![simple_file("src/a.ts"), file_b]);
        let dir = tempfile::tempdir().unwrap();
        assert!(validate_plan(&plan, dir.path()).is_ok());
    }

    #[test]
    fn import_on_disk_passes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/existing.ts"), "export const X = 1;").unwrap();

        let mut file = simple_file("src/new.ts");
        file.must_import_from = vec![ImportRef {
            path: "src/existing.ts".into(),
            symbols: vec!["X".into()],
        }];
        let plan = make_plan(vec![file]);
        assert!(validate_plan(&plan, dir.path()).is_ok());
    }

    #[test]
    fn import_missing_fails() {
        let mut file = simple_file("src/app.ts");
        file.must_import_from = vec![ImportRef {
            path: "src/missing.ts".into(),
            symbols: vec!["Foo".into()],
        }];
        let plan = make_plan(vec![file]);
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            validate_plan(&plan, dir.path()),
            Err(ValidationError::InvalidImport { .. })
        ));
    }
}
