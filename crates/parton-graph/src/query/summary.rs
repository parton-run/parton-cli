//! High-level graph summary for the planner and clarifier.
//!
//! Produces a compact overview of all modules and their exports,
//! import patterns, and key file snippets so the planner/clarifier
//! understand how the project works without asking the user.

use std::path::Path;

use crate::types::{CodeGraph, SymbolKind};

/// Maximum number of files to include in the modules listing.
const MAX_FILES: usize = 100;

/// Maximum exports to show per file.
const MAX_EXPORTS_PER_FILE: usize = 10;

/// Maximum lines to include per key file snippet.
const MAX_SNIPPET_LINES: usize = 25;

/// Maximum number of key file snippets.
const MAX_SNIPPETS: usize = 15;

/// Patterns that identify key files worth showing snippets of.
const KEY_FILE_PATTERNS: &[&str] = &[
    "auth",
    "middleware",
    "permission",
    "rbac",
    "role",
    "guard",
    "session",
    "layout",
    "schema",
    "db/index",
    "database",
];

/// Build a light summary listing only module paths and export counts.
///
/// Used when tools are available so the LLM can drill into specifics
/// on demand instead of receiving the full fat summary up front.
pub fn build_light_summary(graph: &CodeGraph) -> String {
    if graph.files.is_empty() {
        return String::new();
    }

    let mut lines =
        vec!["## Existing Modules (use read_file/get_exports tools to inspect)".to_string()];

    let mut paths: Vec<&String> = graph.files.keys().collect();
    paths.sort();

    for path in &paths {
        let node = match graph.files.get(*path) {
            Some(n) => n,
            None => continue,
        };
        let export_count = node.symbols.iter().filter(|s| s.exported).count();
        if export_count > 0 {
            lines.push(format!("- {path} ({export_count} exports)"));
        } else {
            lines.push(format!("- {path}"));
        }
    }

    lines.push(format!("({} files total)", paths.len()));
    lines.join("\n")
}

/// Build a planner-friendly summary of the entire code graph.
///
/// Includes:
/// 1. Module listing with exported symbol names and kinds
/// 2. Common import patterns detected in the project
/// 3. Key file snippets (auth, middleware, schema, etc.)
pub fn build_summary(graph: &CodeGraph, project_root: &Path) -> String {
    if graph.files.is_empty() {
        return String::new();
    }

    let mut sections = vec!["## Existing Code (from project graph)".to_string()];

    let modules_section = build_modules_section(graph);
    if !modules_section.is_empty() {
        sections.push(modules_section);
    }

    let imports_section = build_import_patterns(graph);
    if !imports_section.is_empty() {
        sections.push(imports_section);
    }

    let snippets_section = build_key_snippets(graph, project_root);
    if !snippets_section.is_empty() {
        sections.push(snippets_section);
    }

    sections.join("\n\n")
}

/// Build the modules & exports listing.
fn build_modules_section(graph: &CodeGraph) -> String {
    let mut lines = vec!["### Modules & Exports".to_string()];

    let mut paths: Vec<&String> = graph.files.keys().collect();
    paths.sort();

    for (i, path) in paths.iter().enumerate() {
        if i >= MAX_FILES {
            lines.push(format!("... and {} more files", paths.len() - i));
            break;
        }

        let node = match graph.files.get(*path) {
            Some(n) => n,
            None => continue,
        };

        let exports: Vec<&crate::types::Symbol> =
            node.symbols.iter().filter(|s| s.exported).collect();

        if exports.is_empty() {
            continue;
        }

        lines.push(format!("// {path}"));

        for (j, sym) in exports.iter().enumerate() {
            if j >= MAX_EXPORTS_PER_FILE {
                lines.push(format!("  ... and {} more exports", exports.len() - j));
                break;
            }
            let kind = format_kind(&sym.kind);
            if sym.signature.is_empty() || sym.signature == sym.name {
                lines.push(format!("  export: {} ({kind})", sym.name));
            } else {
                lines.push(format!("  export: {}", sym.signature));
            }
        }
    }

    if lines.len() <= 1 {
        return String::new();
    }

    lines.join("\n")
}

/// Detect and list common import patterns used in the project.
fn build_import_patterns(graph: &CodeGraph) -> String {
    let mut patterns: Vec<(String, String)> = Vec::new();

    for node in graph.files.values() {
        for imp in &node.imports {
            let key = &imp.from_path;
            if !patterns.iter().any(|(_, p)| p == key) {
                let symbols = imp.symbols.join(", ");
                let desc = if symbols.is_empty() {
                    format!("import from \"{key}\"")
                } else {
                    format!("import {{ {symbols} }} from \"{key}\"")
                };
                patterns.push((desc, key.clone()));
            }
        }
    }

    if patterns.is_empty() {
        return String::new();
    }

    patterns.sort_by(|a, b| a.1.cmp(&b.1));

    let mut lines = vec!["### Import Patterns (how this project imports)".to_string()];
    for (desc, _) in patterns.iter().take(30) {
        lines.push(format!("- {desc}"));
    }
    if patterns.len() > 30 {
        lines.push(format!("... and {} more patterns", patterns.len() - 30));
    }

    lines.join("\n")
}

/// Build snippets of key files (auth, middleware, schema, etc.).
///
/// Shows the first ~25 lines of files matching key patterns
/// so planner/clarifier understand HOW things work, not just WHAT exists.
fn build_key_snippets(graph: &CodeGraph, project_root: &Path) -> String {
    let mut snippets: Vec<(String, String)> = Vec::new();

    let mut paths: Vec<&String> = graph.files.keys().collect();
    paths.sort();

    for path in &paths {
        if snippets.len() >= MAX_SNIPPETS {
            break;
        }
        let path_lower = path.to_lowercase();
        let is_key = KEY_FILE_PATTERNS.iter().any(|pat| path_lower.contains(pat));
        if !is_key {
            continue;
        }

        let full_path = project_root.join(path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let snippet: String = content
            .lines()
            .take(MAX_SNIPPET_LINES)
            .collect::<Vec<_>>()
            .join("\n");

        if !snippet.trim().is_empty() {
            snippets.push(((*path).clone(), snippet));
        }
    }

    if snippets.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "### Key Files (implementation details — DO NOT ask the user about these)".to_string(),
    ];
    for (path, snippet) in &snippets {
        lines.push(format!("#### {path}"));
        lines.push(format!("```\n{snippet}\n```"));
    }

    lines.join("\n")
}

/// Format a symbol kind for display.
fn format_kind(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "Function",
        SymbolKind::Class => "Class",
        SymbolKind::Type => "Type",
        SymbolKind::Interface => "Interface",
        SymbolKind::Trait => "Trait",
        SymbolKind::Variable => "Variable",
        SymbolKind::Enum => "Enum",
        SymbolKind::Struct => "Struct",
        SymbolKind::Module => "Module",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn sym(name: &str, kind: SymbolKind, sig: &str) -> Symbol {
        Symbol {
            name: name.into(),
            kind,
            signature: sig.into(),
            line_start: 1,
            line_end: 5,
            exported: true,
        }
    }

    fn file(path: &str, symbols: Vec<Symbol>, imports: Vec<ImportEdge>) -> FileNode {
        FileNode {
            path: path.into(),
            language: Language::TypeScript,
            symbols,
            imports,
        }
    }

    #[test]
    fn empty_graph_returns_empty() {
        let graph = CodeGraph::default();
        let dir = tempfile::tempdir().unwrap();
        assert!(build_summary(&graph, dir.path()).is_empty());
    }

    #[test]
    fn light_summary_empty_graph() {
        let graph = CodeGraph::default();
        assert!(build_light_summary(&graph).is_empty());
    }

    #[test]
    fn light_summary_shows_export_counts() {
        let mut graph = CodeGraph::new();
        graph.add_file(file(
            "lib/auth.ts",
            vec![
                sym("checkAdmin", SymbolKind::Function, "checkAdmin"),
                sym("isOwner", SymbolKind::Function, "isOwner"),
            ],
            vec![],
        ));
        graph.add_file(file("lib/utils.ts", vec![], vec![]));

        let summary = build_light_summary(&graph);
        assert!(summary.contains("lib/auth.ts (2 exports)"));
        assert!(summary.contains("lib/utils.ts"));
        assert!(!summary.contains("lib/utils.ts ("));
        assert!(summary.contains("2 files total"));
    }

    #[test]
    fn summary_includes_exports() {
        let mut graph = CodeGraph::new();
        graph.add_file(file(
            "lib/auth.ts",
            vec![sym(
                "checkPermission",
                SymbolKind::Function,
                "export function checkPermission(userId: string): boolean",
            )],
            vec![],
        ));

        let dir = tempfile::tempdir().unwrap();
        let summary = build_summary(&graph, dir.path());
        assert!(summary.contains("Existing Code"));
        assert!(summary.contains("lib/auth.ts"));
        assert!(summary.contains("checkPermission"));
    }

    #[test]
    fn summary_includes_import_patterns() {
        let mut graph = CodeGraph::new();
        graph.add_file(file(
            "src/app.ts",
            vec![],
            vec![ImportEdge {
                from_path: "@/lib/db".into(),
                symbols: vec!["db".into()],
            }],
        ));

        let dir = tempfile::tempdir().unwrap();
        let summary = build_summary(&graph, dir.path());
        assert!(summary.contains("Import Patterns"));
        assert!(summary.contains("@/lib/db"));
    }

    #[test]
    fn summary_includes_key_file_snippets() {
        let mut graph = CodeGraph::new();
        graph.add_file(file(
            "lib/auth/check.ts",
            vec![sym(
                "isAdmin",
                SymbolKind::Function,
                "export function isAdmin()",
            )],
            vec![],
        ));

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lib/auth")).unwrap();
        std::fs::write(
            dir.path().join("lib/auth/check.ts"),
            "export function isAdmin(email: string): boolean {\n  return ADMIN_EMAILS.includes(email);\n}\n",
        ).unwrap();

        let summary = build_summary(&graph, dir.path());
        assert!(summary.contains("Key Files"));
        assert!(summary.contains("ADMIN_EMAILS"));
    }

    #[test]
    fn summary_skips_unexported() {
        let mut graph = CodeGraph::new();
        let mut private_sym = sym("internal", SymbolKind::Function, "function internal()");
        private_sym.exported = false;
        graph.add_file(file("src/utils.ts", vec![private_sym], vec![]));

        let dir = tempfile::tempdir().unwrap();
        let summary = build_summary(&graph, dir.path());
        assert!(!summary.contains("internal"));
    }

    #[test]
    fn summary_sorted_by_path() {
        let mut graph = CodeGraph::new();
        graph.add_file(file(
            "z/last.ts",
            vec![sym("Z", SymbolKind::Variable, "Z")],
            vec![],
        ));
        graph.add_file(file(
            "a/first.ts",
            vec![sym("A", SymbolKind::Variable, "A")],
            vec![],
        ));

        let dir = tempfile::tempdir().unwrap();
        let summary = build_summary(&graph, dir.path());
        let a_pos = summary.find("a/first.ts").unwrap();
        let z_pos = summary.find("z/last.ts").unwrap();
        assert!(a_pos < z_pos);
    }
}
