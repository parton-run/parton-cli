//! Project context builder for the planner.
//!
//! Scans the project directory to build a high-level summary
//! (directory tree, dependencies) without including file contents.

use std::collections::BTreeMap;
use std::path::Path;

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
];

const MAX_TREE_DEPTH: usize = 3;
const MAX_TREE_ENTRIES: usize = 80;

/// Build a planning context string from the project directory.
///
/// Includes repo summary, directory tree with file counts, and dependency summary.
/// Does NOT include file contents.
pub fn build_project_context(project_root: &Path) -> String {
    let mut sections = Vec::new();

    sections.push(build_repo_summary(project_root));
    sections.push(build_directory_tree(project_root));

    if let Some(deps) = build_dependency_summary(project_root) {
        sections.push(deps);
    }

    sections.join("\n\n")
}

fn build_repo_summary(root: &Path) -> String {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".into());
    let total = count_files(root);

    format!("## Repository Summary\n- Project: {name}\n- Total files: {total}")
}

fn build_directory_tree(root: &Path) -> String {
    let mut tree = BTreeMap::new();
    collect_dir_counts(root, root, &mut tree, 0);

    let mut lines = vec!["## Project Structure".to_string()];
    for (i, (dir, count)) in tree.iter().enumerate() {
        if i >= MAX_TREE_ENTRIES {
            lines.push(format!("  ... ({} more directories)", tree.len() - i));
            break;
        }
        let display = if dir.is_empty() { "." } else { dir.as_str() };
        lines.push(format!("  {display}/ ({count} files)"));
    }

    lines.join("\n")
}

fn collect_dir_counts(dir: &Path, root: &Path, counts: &mut BTreeMap<String, usize>, depth: usize) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut file_count = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            if IGNORED_DIRS.contains(&name_str.as_ref()) || name_str.starts_with('.') {
                continue;
            }
            if depth < MAX_TREE_DEPTH {
                collect_dir_counts(&path, root, counts, depth + 1);
            } else {
                let deep = count_files(&path);
                if deep > 0 {
                    let rel = relative_path(&path, root);
                    counts.insert(format!("{rel}/..."), deep);
                }
            }
        } else {
            file_count += 1;
        }
    }

    let rel = relative_path(dir, root);
    if file_count > 0 || !rel.is_empty() {
        counts.insert(rel, file_count);
    }
}

fn build_dependency_summary(root: &Path) -> Option<String> {
    // package.json
    if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
        return Some(summarize_package_json(&content));
    }
    // Cargo.toml
    if let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) {
        return Some(summarize_cargo(&content));
    }
    // go.mod
    if root.join("go.mod").exists() {
        return Some("## Dependencies\nGo modules project".into());
    }
    None
}

fn summarize_package_json(content: &str) -> String {
    let mut lines = vec!["## Dependencies (package.json)".to_string()];

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
        for section in ["dependencies", "devDependencies"] {
            if let Some(deps) = val.get(section).and_then(|v| v.as_object()) {
                let label = if section == "dependencies" {
                    "Dependencies"
                } else {
                    "Dev"
                };
                lines.push(format!("{label}:"));
                for (key, ver) in deps {
                    lines.push(format!("  - {key}: {}", ver.as_str().unwrap_or("*")));
                }
            }
        }
    }

    lines.join("\n")
}

fn summarize_cargo(content: &str) -> String {
    let mut lines = vec!["## Dependencies (Cargo.toml)".to_string()];
    let mut in_deps = false;

    for line in content.lines() {
        let t = line.trim();
        if t == "[dependencies]" || t == "[workspace.dependencies]" {
            in_deps = true;
            continue;
        }
        if t.starts_with('[') {
            in_deps = false;
            continue;
        }
        if in_deps {
            if let Some(key) = t.split('=').next() {
                let key = key.trim();
                if !key.is_empty() {
                    lines.push(format!("  - {key}"));
                }
            }
        }
    }

    lines.join("\n")
}

fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn count_files(dir: &Path) -> usize {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if !IGNORED_DIRS.contains(&name.to_string_lossy().as_ref()) {
                count += count_files(&path);
            }
        } else {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_context() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main(){}").unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();

        let ctx = build_project_context(dir.path());
        assert!(ctx.contains("Repository Summary"));
        assert!(ctx.contains("Project Structure"));
        assert!(ctx.contains("src/"));
        assert!(!ctx.contains("fn main")); // No file contents.
    }

    #[test]
    fn ignores_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/foo")).unwrap();
        std::fs::write(dir.path().join("node_modules/foo/bar.js"), "x").unwrap();
        std::fs::write(dir.path().join("index.js"), "x").unwrap();

        let ctx = build_project_context(dir.path());
        assert!(!ctx.contains("node_modules"));
    }

    #[test]
    fn package_json_deps() {
        let content = r#"{"dependencies":{"react":"^18"},"devDependencies":{"vitest":"^1"}}"#;
        let summary = summarize_package_json(content);
        assert!(summary.contains("react"));
        assert!(summary.contains("vitest"));
    }
}
