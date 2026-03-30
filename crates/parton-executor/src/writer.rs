//! File writer — atomically write executor results to disk.

use std::path::Path;

use parton_core::FileResult;

/// Write successful file results to disk.
///
/// Creates parent directories as needed. Returns the list of paths written.
pub fn write_results(
    results: &[FileResult],
    project_root: &Path,
) -> Result<Vec<String>, std::io::Error> {
    let mut written = Vec::new();

    for result in results {
        if !result.success {
            continue;
        }

        let full_path = project_root.join(&result.path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, &result.content)?;
        written.push(result.path.clone());
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn success_result(path: &str, content: &str) -> FileResult {
        FileResult {
            path: path.into(),
            content: content.into(),
            success: true,
            error: None,
            tokens_used: 10,
            elapsed_ms: 100,
        }
    }

    fn failed_result(path: &str) -> FileResult {
        FileResult {
            path: path.into(),
            content: String::new(),
            success: false,
            error: Some("failed".into()),
            tokens_used: 0,
            elapsed_ms: 0,
        }
    }

    #[test]
    fn writes_successful_files() {
        let dir = tempfile::tempdir().unwrap();
        let results = vec![success_result("src/app.ts", "const x = 1;")];
        let written = write_results(&results, dir.path()).unwrap();

        assert_eq!(written, vec!["src/app.ts"]);
        let content = std::fs::read_to_string(dir.path().join("src/app.ts")).unwrap();
        assert_eq!(content, "const x = 1;");
    }

    #[test]
    fn skips_failed_files() {
        let dir = tempfile::tempdir().unwrap();
        let results = vec![failed_result("src/broken.ts")];
        let written = write_results(&results, dir.path()).unwrap();

        assert!(written.is_empty());
        assert!(!dir.path().join("src/broken.ts").exists());
    }

    #[test]
    fn creates_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        let results = vec![success_result("src/deep/nested/file.ts", "ok")];
        let written = write_results(&results, dir.path()).unwrap();

        assert_eq!(written.len(), 1);
        assert!(dir.path().join("src/deep/nested/file.ts").exists());
    }

    #[test]
    fn writes_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        let results = vec![
            success_result("a.ts", "a"),
            failed_result("b.ts"),
            success_result("c.ts", "c"),
        ];
        let written = write_results(&results, dir.path()).unwrap();

        assert_eq!(written, vec!["a.ts", "c.ts"]);
    }
}
