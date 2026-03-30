use super::*;
use parton_core::{FileAction, FilePlan, ImportRef};

fn make_plan(files: Vec<FilePlan>) -> RunPlan {
    RunPlan {
        summary: "test".into(),
        conventions: vec![],
        files,
        install_command: None,
        check_commands: vec![],
        validation_commands: vec![],
        done: true,
        remaining_work: None,
    }
}

fn logic_file(path: &str) -> FilePlan {
    FilePlan {
        path: path.into(),
        action: FileAction::Create,
        goal: "do something".into(),
        must_export: vec![],
        must_import_from: vec![],
        context_files: vec![],
        scaffold_only: false,
        is_test: false,
    }
}

fn test_file(path: &str) -> FilePlan {
    FilePlan {
        is_test: true,
        ..logic_file(path)
    }
}

fn scaffold_file(path: &str) -> FilePlan {
    FilePlan {
        scaffold_only: true,
        ..logic_file(path)
    }
}

#[test]
fn valid_plan_passes() {
    let plan = make_plan(vec![
        logic_file("src/app.ts"),
        test_file("src/app.test.ts"),
    ]);
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
        logic_file("src/app.ts"),
        logic_file("src/app.ts"),
    ]);
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        validate_plan(&plan, dir.path()),
        Err(ValidationError::DuplicatePath(_))
    ));
}

#[test]
fn empty_goal_fails() {
    let mut file = logic_file("src/app.ts");
    file.goal = "  ".into();
    let plan = make_plan(vec![file]);
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        validate_plan(&plan, dir.path()),
        Err(ValidationError::EmptyGoal(_))
    ));
}

#[test]
fn missing_tests_fails() {
    let plan = make_plan(vec![logic_file("main.go")]);
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        validate_plan(&plan, dir.path()),
        Err(ValidationError::MissingTests(_))
    ));
}

#[test]
fn scaffold_only_files_skip_test_requirement() {
    let plan = make_plan(vec![scaffold_file("go.mod")]);
    let dir = tempfile::tempdir().unwrap();
    assert!(validate_plan(&plan, dir.path()).is_ok());
}

#[test]
fn any_language_test_file_works() {
    // The validator doesn't care about filenames — only is_test matters.
    let plan = make_plan(vec![
        logic_file("main.go"),
        scaffold_file("go.mod"),
        test_file("main_test.go"),
    ]);
    let dir = tempfile::tempdir().unwrap();
    assert!(validate_plan(&plan, dir.path()).is_ok());
}

#[test]
fn import_in_plan_passes() {
    let mut file_b = logic_file("src/b.ts");
    file_b.must_import_from = vec![ImportRef {
        path: "src/a.ts".into(),
        symbols: vec!["Foo".into()],
    }];
    let plan = make_plan(vec![
        logic_file("src/a.ts"),
        file_b,
        test_file("src/a.test.ts"),
    ]);
    let dir = tempfile::tempdir().unwrap();
    assert!(validate_plan(&plan, dir.path()).is_ok());
}

#[test]
fn import_on_disk_passes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/existing.ts"),
        "export const X = 1;",
    )
    .unwrap();

    let mut file = logic_file("src/new.ts");
    file.must_import_from = vec![ImportRef {
        path: "src/existing.ts".into(),
        symbols: vec!["X".into()],
    }];
    let plan = make_plan(vec![file, test_file("src/new.test.ts")]);
    assert!(validate_plan(&plan, dir.path()).is_ok());
}

#[test]
fn import_missing_fails() {
    let mut file = logic_file("src/app.ts");
    file.must_import_from = vec![ImportRef {
        path: "src/missing.ts".into(),
        symbols: vec!["Foo".into()],
    }];
    let plan = make_plan(vec![file, test_file("src/app.test.ts")]);
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        validate_plan(&plan, dir.path()),
        Err(ValidationError::InvalidImport { .. })
    ));
}
