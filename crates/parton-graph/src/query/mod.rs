//! Query API for extracting targeted context from the code graph.

pub mod context;
pub mod summary;

use crate::types::CodeGraph;

/// Build a formatted context string for a specific file.
///
/// Returns a markdown section with import signatures and dependents,
/// ready to inject into the executor prompt. Returns an empty string
/// if no relevant context exists.
pub fn context_for_file(graph: &CodeGraph, file_path: &str) -> String {
    context::build_context(graph, file_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileNode, ImportEdge, Language, Symbol, SymbolKind};

    #[test]
    fn context_for_file_delegates() {
        let mut graph = CodeGraph::new();
        graph.add_file(FileNode {
            path: "src/types.ts".into(),
            language: Language::TypeScript,
            symbols: vec![Symbol {
                name: "User".into(),
                kind: SymbolKind::Interface,
                signature: "export interface User { id: string }".into(),
                line_start: 1,
                line_end: 3,
                exported: true,
            }],
            imports: vec![],
        });
        graph.add_file(FileNode {
            path: "src/app.ts".into(),
            language: Language::TypeScript,
            symbols: vec![],
            imports: vec![ImportEdge {
                from_path: "src/types.ts".into(),
                symbols: vec!["User".into()],
            }],
        });

        let ctx = context_for_file(&graph, "src/app.ts");
        assert!(ctx.contains("User"));
    }

    #[test]
    fn context_for_missing_file_is_empty() {
        let graph = CodeGraph::new();
        assert!(context_for_file(&graph, "missing.ts").is_empty());
    }
}
