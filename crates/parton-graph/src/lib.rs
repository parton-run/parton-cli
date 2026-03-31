#![deny(warnings)]

//! In-memory code graph for Parton.
//!
//! Parses project source files with tree-sitter, builds an in-memory
//! graph of symbols and relationships, and provides targeted context
//! for parallel code generation executors.

pub mod detect;
pub mod error;
pub mod grammar;
pub mod graph;
pub mod queries;
pub mod query;
pub mod scan;
pub mod tools;
pub mod types;

pub use detect::{detect_language, is_supported};
pub use error::GraphError;
pub use grammar::GrammarStore;
pub use query::context_for_file;
pub use types::{CodeGraph, FileNode, ImportEdge, Language, Symbol, SymbolKind};

use std::collections::HashMap;
use std::path::Path;

use parton_core::FilePlan;

/// Scan an entire project and build the code graph.
///
/// Walks the project tree, parses all supported source files with
/// tree-sitter, and returns the in-memory graph. This should run
/// once at the start of the pipeline.
pub async fn scan_project(project_root: &Path) -> Result<CodeGraph, GraphError> {
    let source_files = scan::walker::collect_source_files(project_root);

    if source_files.is_empty() {
        return Ok(CodeGraph::new());
    }

    let mut store = GrammarStore::with_default_cache()?;
    let file_nodes = scan::scan_files(&source_files, &mut store, project_root).await;

    let mut graph = CodeGraph::new();
    for node in file_nodes {
        graph.add_file(node);
    }

    Ok(graph)
}

/// Build a high-level summary of the graph for the planner/clarifier.
///
/// Returns a compact markdown overview of existing modules, their
/// exports, import patterns, and key file snippets.
pub fn build_graph_summary(graph: &CodeGraph, project_root: &Path) -> String {
    query::summary::build_summary(graph, project_root)
}

/// Build a light summary listing modules and export counts only.
///
/// Used when tools are available for drill-down, so the LLM gets a
/// compact overview and can request details on demand.
pub fn build_light_graph_summary(graph: &CodeGraph) -> String {
    query::summary::build_light_summary(graph)
}

/// Build per-file context strings for executor prompts.
///
/// Creates stub nodes for planned files using their `must_import_from`
/// data, then generates targeted context for each file.
pub fn build_file_contexts(graph: &CodeGraph, plan_files: &[FilePlan]) -> HashMap<String, String> {
    // Clone graph so we can add stub nodes for planned files.
    let mut exec_graph = graph.clone();

    for file in plan_files {
        if exec_graph.get_file(&file.path).is_some() {
            continue;
        }
        let imports: Vec<ImportEdge> = file
            .must_import_from
            .iter()
            .map(|i| ImportEdge {
                from_path: i.path.clone(),
                symbols: i.symbols.clone(),
            })
            .collect();
        if !imports.is_empty() {
            exec_graph.add_file(FileNode {
                path: file.path.clone(),
                language: detect_language(&file.path),
                symbols: vec![],
                imports,
            });
        }
    }

    let mut contexts = HashMap::new();
    for file in plan_files {
        let ctx = context_for_file(&exec_graph, &file.path);
        if !ctx.is_empty() {
            contexts.insert(file.path.clone(), ctx);
        }
    }

    contexts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_api_exports() {
        let _graph = CodeGraph::new();
        let _lang = Language::TypeScript;
        let _kind = SymbolKind::Function;
    }

    #[test]
    fn build_summary_on_empty_graph() {
        let dir = tempfile::tempdir().unwrap();
        let graph = CodeGraph::new();
        assert!(build_graph_summary(&graph, dir.path()).is_empty());
    }

    #[test]
    fn build_light_summary_on_empty_graph() {
        let graph = CodeGraph::new();
        assert!(build_light_graph_summary(&graph).is_empty());
    }
}
