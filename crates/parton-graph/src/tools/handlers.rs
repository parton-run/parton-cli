//! Individual tool handler implementations.

use std::path::Path;

use parton_core::ToolCall;
use serde_json::json;

use crate::types::{CodeGraph, SymbolKind};

/// Maximum lines returned by `read_file`.
const MAX_READ_LINES: usize = 200;

/// Maximum results from `search_symbols`.
const MAX_SEARCH_RESULTS: usize = 30;

/// Maximum results from `list_files`.
const MAX_LIST_RESULTS: usize = 100;

/// Read a file from disk.
pub fn read_file(call: &ToolCall, root: &Path) -> String {
    let path = call.arguments["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return "error: missing path argument".into();
    }
    let full = root.join(path);
    match std::fs::read_to_string(&full) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().take(MAX_READ_LINES).collect();
            lines.join("\n")
        }
        Err(e) => format!("error reading {path}: {e}"),
    }
}

/// Get exported symbols from the graph.
pub fn get_exports(call: &ToolCall, graph: &CodeGraph) -> String {
    let path = call.arguments["path"].as_str().unwrap_or("");
    let exports = graph.exports_of(path);
    if exports.is_empty() {
        return format!("no exports found for {path}");
    }
    let items: Vec<serde_json::Value> = exports
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "kind": format_kind(&s.kind),
                "signature": s.signature,
            })
        })
        .collect();
    serde_json::to_string_pretty(&items).unwrap_or_default()
}

/// Get imports for a file from the graph.
pub fn get_imports(call: &ToolCall, graph: &CodeGraph) -> String {
    let path = call.arguments["path"].as_str().unwrap_or("");
    let node = match graph.get_file(path) {
        Some(n) => n,
        None => return format!("file not found in graph: {path}"),
    };
    if node.imports.is_empty() {
        return format!("no imports in {path}");
    }
    let items: Vec<serde_json::Value> = node
        .imports
        .iter()
        .map(|imp| {
            json!({
                "from_path": imp.from_path,
                "symbols": imp.symbols,
            })
        })
        .collect();
    serde_json::to_string_pretty(&items).unwrap_or_default()
}

/// Search symbols by name substring.
pub fn search_symbols(call: &ToolCall, graph: &CodeGraph) -> String {
    let query = call.arguments["query"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    if query.is_empty() {
        return "error: missing query argument".into();
    }
    let mut results: Vec<serde_json::Value> = Vec::new();
    for (path, node) in &graph.files {
        for sym in &node.symbols {
            if !sym.exported {
                continue;
            }
            if sym.name.to_lowercase().contains(&query) {
                results.push(json!({
                    "path": path,
                    "name": sym.name,
                    "kind": format_kind(&sym.kind),
                    "signature": sym.signature,
                }));
                if results.len() >= MAX_SEARCH_RESULTS {
                    return serde_json::to_string_pretty(&results).unwrap_or_default();
                }
            }
        }
    }
    if results.is_empty() {
        return format!("no symbols matching '{query}'");
    }
    serde_json::to_string_pretty(&results).unwrap_or_default()
}

/// List files matching a substring pattern.
pub fn list_files(call: &ToolCall, graph: &CodeGraph) -> String {
    let pattern = call.arguments["pattern"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    if pattern.is_empty() {
        return "error: missing pattern argument".into();
    }
    let mut matches: Vec<&str> = graph
        .files
        .keys()
        .filter(|p| p.to_lowercase().contains(&pattern))
        .map(|p| p.as_str())
        .collect();
    matches.sort();
    if matches.is_empty() {
        return format!("no files matching '{pattern}'");
    }
    if matches.len() > MAX_LIST_RESULTS {
        let total = matches.len();
        matches.truncate(MAX_LIST_RESULTS);
        matches.push("...");
        return format!(
            "{}\n({total} files total, showing first {MAX_LIST_RESULTS})",
            matches.join("\n")
        );
    }
    matches.join("\n")
}

/// Format symbol kind for JSON output.
fn format_kind(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Class => "class",
        SymbolKind::Type => "type",
        SymbolKind::Interface => "interface",
        SymbolKind::Trait => "trait",
        SymbolKind::Variable => "variable",
        SymbolKind::Enum => "enum",
        SymbolKind::Struct => "struct",
        SymbolKind::Module => "module",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn search_symbols_no_match() {
        let graph = CodeGraph::new();
        let call = ToolCall {
            id: "t".into(),
            name: "search_symbols".into(),
            arguments: json!({"query": "zzz"}),
        };
        assert!(search_symbols(&call, &graph).contains("no symbols"));
    }

    #[test]
    fn list_files_no_match() {
        let graph = CodeGraph::new();
        let call = ToolCall {
            id: "t".into(),
            name: "list_files".into(),
            arguments: json!({"pattern": "zzz"}),
        };
        assert!(list_files(&call, &graph).contains("no files"));
    }

    #[test]
    fn get_imports_missing_file() {
        let graph = CodeGraph::new();
        let call = ToolCall {
            id: "t".into(),
            name: "get_imports".into(),
            arguments: json!({"path": "nope.ts"}),
        };
        assert!(get_imports(&call, &graph).contains("not found"));
    }

    #[test]
    fn format_kind_all_variants() {
        assert_eq!(format_kind(&SymbolKind::Function), "function");
        assert_eq!(format_kind(&SymbolKind::Class), "class");
        assert_eq!(format_kind(&SymbolKind::Struct), "struct");
        assert_eq!(format_kind(&SymbolKind::Module), "module");
    }
}
