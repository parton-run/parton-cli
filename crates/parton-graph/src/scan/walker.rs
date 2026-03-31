//! Project file walker.
//!
//! Walks a project directory tree collecting source files,
//! respecting common ignore patterns.

use std::path::Path;

use crate::detect;

/// Directories to skip during scanning.
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    "__pycache__",
    ".venv",
    "vendor",
    ".parton",
    ".cache",
    ".turbo",
    ".vercel",
    "coverage",
    ".svelte-kit",
    ".nuxt",
    ".output",
];

/// Maximum directory depth for scanning.
const MAX_DEPTH: usize = 10;

/// Walk a project directory and collect all scannable source file paths.
///
/// Returns relative paths (e.g., `src/app.ts`, `lib/utils.py`).
/// Skips hidden directories, build output, and vendor directories.
pub fn collect_source_files(project_root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    walk_dir(project_root, project_root, &mut files, 0);
    files
}

/// Recursive directory walker.
fn walk_dir(dir: &Path, root: &Path, files: &mut Vec<String>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            if IGNORED_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            walk_dir(&path, root, files, depth + 1);
        } else if path.is_file() {
            let rel = match path.strip_prefix(root) {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => continue,
            };
            if detect::is_supported(detect::detect_language(&rel)) {
                files.push(rel);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_ts_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/app.ts"), "export const x = 1;").unwrap();
        std::fs::write(dir.path().join("src/utils.ts"), "export const y = 2;").unwrap();
        std::fs::write(dir.path().join("README.md"), "# Hi").unwrap();

        let files = collect_source_files(dir.path());
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.ends_with("app.ts")));
        assert!(files.iter().any(|f| f.ends_with("utils.ts")));
    }

    #[test]
    fn skips_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/foo")).unwrap();
        std::fs::write(dir.path().join("node_modules/foo/index.js"), "x").unwrap();
        std::fs::write(dir.path().join("index.ts"), "export {}").unwrap();

        let files = collect_source_files(dir.path());
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("index.ts"));
    }

    #[test]
    fn skips_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/config"), "x").unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main(){}").unwrap();

        let files = collect_source_files(dir.path());
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(collect_source_files(dir.path()).is_empty());
    }

    #[test]
    fn nested_files_collected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b/c")).unwrap();
        std::fs::write(dir.path().join("a/b/c/deep.py"), "def f(): pass").unwrap();

        let files = collect_source_files(dir.path());
        assert_eq!(files.len(), 1);
        assert!(files[0].contains("deep.py"));
    }
}
