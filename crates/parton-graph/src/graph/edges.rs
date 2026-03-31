//! Edge resolution utilities for the code graph.
//!
//! Normalizes import paths to match file paths in the graph.

use std::path::Path;

/// Normalize an import path relative to the importing file.
///
/// Resolves relative paths like `./types` or `../utils` against the
/// directory of the importing file. Handles common extensions.
pub fn resolve_import_path(import_from: &str, importer_path: &str) -> String {
    if !import_from.starts_with('.') {
        return import_from.to_string();
    }

    let importer_dir = Path::new(importer_path).parent().unwrap_or(Path::new(""));

    let resolved = importer_dir.join(import_from);
    normalize_path(&resolved)
}

/// Normalize a path by resolving `.` and `..` components.
fn normalize_path(path: &Path) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::Normal(s) => {
                if let Some(s) = s.to_str() {
                    parts.push(s);
                }
            }
            _ => {}
        }
    }
    parts.join("/")
}

/// Try to match an import path against known file paths.
///
/// Attempts the path as-is, then with common extensions appended.
pub fn find_matching_file<'a>(import_path: &str, known_paths: &'a [&str]) -> Option<&'a str> {
    if known_paths.contains(&import_path) {
        return known_paths.iter().find(|&&p| p == import_path).copied();
    }

    let extensions = [
        ".ts",
        ".tsx",
        ".js",
        ".jsx",
        ".rs",
        ".py",
        ".go",
        "/index.ts",
        "/index.js",
    ];

    for ext in &extensions {
        let candidate = format!("{import_path}{ext}");
        if let Some(&path) = known_paths.iter().find(|&&p| p == candidate) {
            return Some(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_same_dir() {
        assert_eq!(resolve_import_path("./types", "src/app.ts"), "src/types");
    }

    #[test]
    fn resolve_relative_parent_dir() {
        assert_eq!(
            resolve_import_path("../utils", "src/components/app.ts"),
            "src/utils"
        );
    }

    #[test]
    fn resolve_absolute_path_unchanged() {
        assert_eq!(
            resolve_import_path("@/lib/utils", "src/app.ts"),
            "@/lib/utils"
        );
    }

    #[test]
    fn resolve_package_import_unchanged() {
        assert_eq!(resolve_import_path("react", "src/app.ts"), "react");
    }

    #[test]
    fn find_matching_exact() {
        let paths = vec!["src/types.ts", "src/app.ts"];
        assert_eq!(
            find_matching_file("src/types.ts", &paths),
            Some("src/types.ts")
        );
    }

    #[test]
    fn find_matching_with_extension() {
        let paths = vec!["src/types.ts", "src/app.ts"];
        assert_eq!(
            find_matching_file("src/types", &paths),
            Some("src/types.ts")
        );
    }

    #[test]
    fn find_matching_index_file() {
        let paths = vec!["src/components/index.ts"];
        assert_eq!(
            find_matching_file("src/components", &paths),
            Some("src/components/index.ts")
        );
    }

    #[test]
    fn find_matching_not_found() {
        let paths = vec!["src/types.ts"];
        assert_eq!(find_matching_file("src/missing", &paths), None);
    }
}
