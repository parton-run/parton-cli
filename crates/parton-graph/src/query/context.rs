//! Format graph data as executor prompt context.
//!
//! Converts graph query results into markdown sections suitable
//! for injection into the executor's per-file prompt.

use crate::types::{CodeGraph, Symbol};

/// Build a markdown context section for a file being generated.
///
/// Includes:
/// 1. Signatures of symbols imported from other files
/// 2. List of dependent files that import from this file
pub fn build_context(graph: &CodeGraph, file_path: &str) -> String {
    let imports_section = build_imports_section(graph, file_path);
    let dependents_section = build_dependents_section(graph, file_path);

    if imports_section.is_empty() && dependents_section.is_empty() {
        return String::new();
    }

    let mut sections = vec!["## Related Code (from project graph)".to_string()];

    if !imports_section.is_empty() {
        sections.push(imports_section);
    }
    if !dependents_section.is_empty() {
        sections.push(dependents_section);
    }

    sections.join("\n\n")
}

/// Build the "Imports" subsection with resolved signatures.
fn build_imports_section(graph: &CodeGraph, file_path: &str) -> String {
    let resolved = graph.resolved_imports_of(file_path);
    if resolved.is_empty() {
        return String::new();
    }

    let mut lines = vec!["### Imports (signatures you depend on)".to_string()];

    for (source_path, symbols) in &resolved {
        lines.push(format!("// from {source_path}"));
        for sym in symbols {
            lines.push(format_symbol(sym));
        }
    }

    lines.join("\n")
}

/// Build the "Dependents" subsection listing files that import from this one.
fn build_dependents_section(graph: &CodeGraph, file_path: &str) -> String {
    let dependents = graph.dependents_of(file_path);
    if dependents.is_empty() {
        return String::new();
    }

    let mut lines = vec!["### Dependents (don't break these imports)".to_string()];

    for (dep_path, symbols) in &dependents {
        let sym_list = symbols.join(", ");
        lines.push(format!("- {dep_path} imports: {sym_list}"));
    }

    lines.join("\n")
}

/// Format a symbol as a signature line.
fn format_symbol(sym: &Symbol) -> String {
    if sym.signature.is_empty() {
        sym.name.clone()
    } else {
        sym.signature.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileNode, ImportEdge, Language, SymbolKind};

    fn sym(name: &str, sig: &str) -> Symbol {
        Symbol {
            name: name.into(),
            kind: SymbolKind::Function,
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
        assert!(build_context(&graph, "src/app.ts").is_empty());
    }

    #[test]
    fn context_with_imports() {
        let mut graph = CodeGraph::new();
        graph.add_file(file(
            "src/types.ts",
            vec![
                sym("User", "export interface User { id: string }"),
                sym("Filter", "export type Filter = 'all' | 'active'"),
            ],
            vec![],
        ));
        graph.add_file(file(
            "src/app.ts",
            vec![],
            vec![ImportEdge {
                from_path: "src/types.ts".into(),
                symbols: vec!["User".into()],
            }],
        ));

        let ctx = build_context(&graph, "src/app.ts");
        assert!(ctx.contains("Related Code"));
        assert!(ctx.contains("Imports"));
        assert!(ctx.contains("export interface User"));
        assert!(!ctx.contains("Filter"));
    }

    #[test]
    fn context_with_dependents() {
        let mut graph = CodeGraph::new();
        graph.add_file(file(
            "src/types.ts",
            vec![sym("User", "export interface User { id: string }")],
            vec![],
        ));
        graph.add_file(file(
            "src/app.ts",
            vec![],
            vec![ImportEdge {
                from_path: "src/types.ts".into(),
                symbols: vec!["User".into()],
            }],
        ));

        let ctx = build_context(&graph, "src/types.ts");
        assert!(ctx.contains("Dependents"));
        assert!(ctx.contains("src/app.ts imports: User"));
    }

    #[test]
    fn context_with_both() {
        let mut graph = CodeGraph::new();
        graph.add_file(file(
            "src/types.ts",
            vec![sym("User", "export interface User { id: string }")],
            vec![],
        ));
        graph.add_file(file(
            "src/app.ts",
            vec![sym("App", "export function App()")],
            vec![ImportEdge {
                from_path: "src/types.ts".into(),
                symbols: vec!["User".into()],
            }],
        ));
        graph.add_file(file(
            "src/main.ts",
            vec![],
            vec![ImportEdge {
                from_path: "src/app.ts".into(),
                symbols: vec!["App".into()],
            }],
        ));

        let ctx = build_context(&graph, "src/app.ts");
        assert!(ctx.contains("Imports"));
        assert!(ctx.contains("Dependents"));
        assert!(ctx.contains("export interface User"));
        assert!(ctx.contains("src/main.ts imports: App"));
    }

    #[test]
    fn no_dependents_no_section() {
        let mut graph = CodeGraph::new();
        graph.add_file(file("src/leaf.ts", vec![], vec![]));
        let ctx = build_context(&graph, "src/leaf.ts");
        assert!(ctx.is_empty());
    }
}
