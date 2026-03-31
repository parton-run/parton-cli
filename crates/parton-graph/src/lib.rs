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
pub mod types;

pub use detect::{detect_language, is_supported};
pub use error::GraphError;
pub use grammar::GrammarStore;
pub use query::context_for_file;
pub use types::{CodeGraph, FileNode, ImportEdge, Language, Symbol, SymbolKind};

use std::collections::HashMap;
use std::path::Path;

use parton_core::FilePlan;

/// Build graph context strings for all files in a plan.
///
/// Scans existing project files with tree-sitter, creates stub
/// nodes for planned files using their `must_import_from` data,
/// then generates targeted context for each file.
pub async fn build_graph_contexts(
    plan_files: &[FilePlan],
    project_root: &Path,
) -> Result<HashMap<String, String>, GraphError> {
    // Collect all existing files worth scanning (imports + context + edits).
    let scannable: Vec<String> = plan_files
        .iter()
        .flat_map(|f| {
            let mut paths: Vec<String> =
                f.must_import_from.iter().map(|i| i.path.clone()).collect();
            paths.extend(f.context_files.iter().cloned());
            if f.action == parton_core::FileAction::Edit {
                paths.push(f.path.clone());
            }
            paths
        })
        .filter(|p| project_root.join(p).exists())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if scannable.is_empty() {
        return Ok(HashMap::new());
    }

    let mut store = GrammarStore::with_default_cache()?;
    let file_nodes = scan::scan_files(&scannable, &mut store, project_root).await;

    let mut code_graph = CodeGraph::new();
    for node in file_nodes {
        code_graph.add_file(node);
    }

    // Create stub nodes for planned files so the graph knows their imports.
    for file in plan_files {
        if code_graph.get_file(&file.path).is_some() {
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
            code_graph.add_file(FileNode {
                path: file.path.clone(),
                language: detect_language(&file.path),
                symbols: vec![],
                imports,
            });
        }
    }

    let mut contexts = HashMap::new();
    for file in plan_files {
        let ctx = context_for_file(&code_graph, &file.path);
        if !ctx.is_empty() {
            contexts.insert(file.path.clone(), ctx);
        }
    }

    Ok(contexts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_api_exports() {
        // Verify key types are accessible.
        let _graph = CodeGraph::new();
        let _lang = Language::TypeScript;
        let _kind = SymbolKind::Function;
    }
}
