//! Auto-initialize knowledge from project files.
//!
//! Scans the project to detect conventions, stack info, and patterns.

use std::path::Path;

use crate::entry::{Category, Entry, Source};
use crate::store::KnowledgeStore;

/// Scan a project and populate the knowledge store with detected conventions.
///
/// Only adds entries if the store is empty (won't overwrite manual entries).
pub fn auto_init(store: &dyn KnowledgeStore, project_root: &Path) {
    if !store.list().unwrap_or_default().is_empty() {
        return; // Already initialized.
    }

    let entries = detect_project_knowledge(project_root);
    for entry in entries {
        let _ = store.upsert(&entry);
    }
}

/// Detect knowledge entries from project files.
fn detect_project_knowledge(root: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();

    // Detect language/framework.
    if let Some(stack) = detect_stack(root) {
        entries.push(stack);
    }

    // Detect package manager.
    if let Some(pm) = detect_package_manager(root) {
        entries.push(pm);
    }

    // Detect conventions from config files.
    entries.extend(detect_conventions(root));

    entries
}

/// Detect the primary tech stack.
fn detect_stack(root: &Path) -> Option<Entry> {
    let (lang, framework) = if root.join("package.json").exists() {
        let content = std::fs::read_to_string(root.join("package.json")).ok()?;
        let has_ts = root.join("tsconfig.json").exists();
        let lang = if has_ts { "TypeScript" } else { "JavaScript" };

        let framework = if content.contains("\"react\"") {
            "React"
        } else if content.contains("\"vue\"") {
            "Vue"
        } else if content.contains("\"svelte\"") {
            "Svelte"
        } else if content.contains("\"next\"") {
            "Next.js"
        } else if content.contains("\"express\"") {
            "Express"
        } else {
            "Node.js"
        };

        (lang, framework)
    } else if root.join("Cargo.toml").exists() {
        ("Rust", "Cargo")
    } else if root.join("go.mod").exists() {
        ("Go", "Go modules")
    } else if root.join("pyproject.toml").exists() {
        ("Python", "Python")
    } else {
        return None;
    };

    Some(Entry {
        id: "stack-primary".into(),
        category: Category::StackInfo,
        title: format!("{lang} / {framework}"),
        content: format!("Primary language: {lang}. Framework: {framework}."),
        tags: vec![lang.to_lowercase(), framework.to_lowercase()],
        source: Source::Auto,
    })
}

/// Detect the package manager in use.
fn detect_package_manager(root: &Path) -> Option<Entry> {
    let pm = if root.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if root.join("yarn.lock").exists() {
        "yarn"
    } else if root.join("bun.lock").exists() || root.join("bun.lockb").exists() {
        "bun"
    } else if root.join("package-lock.json").exists() || root.join("package.json").exists() {
        "npm"
    } else {
        return None;
    };

    Some(Entry {
        id: "convention-pkg-manager".into(),
        category: Category::Convention,
        title: format!("Package manager: {pm}"),
        content: format!("This project uses {pm} for dependency management."),
        tags: vec![pm.into(), "package-manager".into()],
        source: Source::Auto,
    })
}

/// Detect conventions from config files.
fn detect_conventions(root: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();

    // Detect test framework.
    if root.join("vitest.config.ts").exists()
        || root.join("vitest.config.js").exists()
        || has_dep(root, "vitest")
    {
        entries.push(Entry {
            id: "convention-test-framework".into(),
            category: Category::Convention,
            title: "Test framework: Vitest".into(),
            content: "Tests use Vitest. Test files use .test.ts/.test.tsx suffix.".into(),
            tags: vec!["vitest".into(), "testing".into()],
            source: Source::Auto,
        });
    } else if has_dep(root, "jest") {
        entries.push(Entry {
            id: "convention-test-framework".into(),
            category: Category::Convention,
            title: "Test framework: Jest".into(),
            content: "Tests use Jest. Test files use .test.ts/.test.tsx suffix.".into(),
            tags: vec!["jest".into(), "testing".into()],
            source: Source::Auto,
        });
    }

    // Detect CSS framework.
    if has_dep(root, "tailwindcss") {
        entries.push(Entry {
            id: "convention-css".into(),
            category: Category::Convention,
            title: "CSS: Tailwind CSS".into(),
            content: "Styling uses Tailwind CSS utility classes.".into(),
            tags: vec!["tailwind".into(), "css".into()],
            source: Source::Auto,
        });
    }

    // Detect ESM vs CJS.
    if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
        if content.contains("\"type\": \"module\"") || content.contains("\"type\":\"module\"") {
            entries.push(Entry {
                id: "convention-module-system".into(),
                category: Category::Convention,
                title: "Module system: ESM".into(),
                content: "Package uses ESM (type: module). Use import/export, not require().".into(),
                tags: vec!["esm".into(), "imports".into()],
                source: Source::Auto,
            });
        }
    }

    entries
}

/// Check if a dependency exists in package.json.
fn has_dep(root: &Path, name: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(root.join("package.json")) else {
        return false;
    };
    content.contains(&format!("\"{name}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_stack_node_ts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"dependencies":{"react":"^18"}}"#).unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();

        let stack = detect_stack(dir.path()).unwrap();
        assert!(stack.title.contains("TypeScript"));
        assert!(stack.title.contains("React"));
    }

    #[test]
    fn detect_stack_rust() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();

        let stack = detect_stack(dir.path()).unwrap();
        assert!(stack.title.contains("Rust"));
    }

    #[test]
    fn detect_stack_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_stack(dir.path()).is_none());
    }

    #[test]
    fn detect_pm_npm() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();

        let pm = detect_package_manager(dir.path()).unwrap();
        assert!(pm.content.contains("npm"));
    }

    #[test]
    fn detect_pm_pnpm() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();

        let pm = detect_package_manager(dir.path()).unwrap();
        assert!(pm.content.contains("pnpm"));
    }

    #[test]
    fn detect_conventions_vitest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"devDependencies":{"vitest":"^1"}}"#).unwrap();

        let convs = detect_conventions(dir.path());
        assert!(convs.iter().any(|e| e.title.contains("Vitest")));
    }
}
