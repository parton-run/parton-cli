//! Tree-sitter parsing and symbol extraction.
//!
//! Parses source code with a tree-sitter grammar and extracts
//! symbols and imports using query patterns.

use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use crate::error::GraphError;
use crate::types::{ImportEdge, Symbol, SymbolKind};

/// Parse a source file and extract exported symbols.
pub fn extract_exports(
    source: &str,
    ts_lang: &Language,
    query_str: &str,
    lang: crate::types::Language,
) -> Result<Vec<Symbol>, GraphError> {
    let tree = parse_source(source, ts_lang)?;
    let query = compile_query(ts_lang, query_str)?;
    let root = tree.root_node();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, source.as_bytes());

    let name_idx = capture_index(&query, "name");
    let decl_idx = capture_index(&query, "decl");

    let mut symbols = Vec::new();

    while let Some(m) = matches.next() {
        let name = match name_idx {
            Some(idx) => match capture_text(m, idx, source) {
                Some(n) => n,
                None => continue,
            },
            None => continue,
        };

        let (signature, start, end, kind) = match decl_idx {
            Some(idx) => match m.captures.iter().find(|c| c.index == idx) {
                Some(cap) => {
                    let node = cap.node;
                    let sig = extract_signature(source, &node);
                    let kind = infer_kind_from_node(node.kind());
                    let start = node.start_position().row + 1;
                    let end = node.end_position().row + 1;
                    (sig, start, end, kind)
                }
                None => (name.clone(), 0, 0, SymbolKind::Variable),
            },
            None => (name.clone(), 0, 0, SymbolKind::Variable),
        };

        let exported = is_exported_symbol(&name, lang);

        symbols.push(Symbol {
            name,
            kind,
            signature,
            line_start: start as u32,
            line_end: end as u32,
            exported,
        });
    }

    Ok(symbols)
}

/// Parse a source file and extract import edges.
pub fn extract_imports(
    source: &str,
    ts_lang: &Language,
    query_str: &str,
) -> Result<Vec<ImportEdge>, GraphError> {
    let tree = parse_source(source, ts_lang)?;
    let query = compile_query(ts_lang, query_str)?;
    let root = tree.root_node();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, source.as_bytes());

    let source_idx = capture_index(&query, "source");
    let symbol_idx = capture_index(&query, "symbol");
    let path_idx = capture_index(&query, "path");

    let mut edges: Vec<ImportEdge> = Vec::new();

    while let Some(m) = matches.next() {
        let from_path = source_idx
            .and_then(|idx| capture_text(m, idx, source))
            .or_else(|| path_idx.and_then(|idx| capture_text(m, idx, source)));

        let from_path = match from_path {
            Some(p) => strip_quotes(&p),
            None => continue,
        };

        let symbol = symbol_idx.and_then(|idx| capture_text(m, idx, source));

        if let Some(existing) = edges.iter_mut().find(|e| e.from_path == from_path) {
            if let Some(sym) = symbol {
                if !existing.symbols.contains(&sym) {
                    existing.symbols.push(sym);
                }
            }
        } else {
            edges.push(ImportEdge {
                from_path,
                symbols: symbol.into_iter().collect(),
            });
        }
    }

    Ok(edges)
}

/// Parse source with tree-sitter and return the syntax tree.
fn parse_source(source: &str, ts_lang: &Language) -> Result<tree_sitter::Tree, GraphError> {
    let mut parser = Parser::new();
    parser
        .set_language(ts_lang)
        .map_err(|e| GraphError::Query(format!("set language: {e}")))?;

    parser
        .parse(source, None)
        .ok_or_else(|| GraphError::ParseFailed {
            path: "<source>".into(),
            reason: "tree-sitter returned no tree".into(),
        })
}

/// Compile a tree-sitter query string.
fn compile_query(ts_lang: &Language, query_str: &str) -> Result<Query, GraphError> {
    Query::new(ts_lang, query_str).map_err(|e| GraphError::Query(format!("{e}")))
}

/// Get the index of a named capture.
fn capture_index(query: &Query, name: &str) -> Option<u32> {
    query
        .capture_names()
        .iter()
        .position(|n| *n == name)
        .map(|i| i as u32)
}

/// Get the text of a capture by index.
fn capture_text(m: &tree_sitter::QueryMatch, idx: u32, source: &str) -> Option<String> {
    m.captures
        .iter()
        .find(|c| c.index == idx)
        .and_then(|c| c.node.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())
}

/// Extract the first line of a declaration as its signature.
fn extract_signature(source: &str, node: &tree_sitter::Node) -> String {
    let start = node.start_byte();
    let end = node.end_byte().min(source.len());
    let text = &source[start..end];

    text.lines()
        .next()
        .unwrap_or(text)
        .trim_end_matches('{')
        .trim()
        .to_string()
}

/// Infer symbol kind from tree-sitter node type.
fn infer_kind_from_node(node_type: &str) -> SymbolKind {
    match node_type {
        s if s.contains("function") => SymbolKind::Function,
        s if s.contains("class") => SymbolKind::Class,
        s if s.contains("interface") => SymbolKind::Interface,
        s if s.contains("type_alias") || s.contains("type_item") => SymbolKind::Type,
        s if s.contains("enum") => SymbolKind::Enum,
        s if s.contains("struct") => SymbolKind::Struct,
        s if s.contains("trait") => SymbolKind::Trait,
        s if s.contains("impl") => SymbolKind::Struct,
        s if s.contains("method") => SymbolKind::Function,
        _ => SymbolKind::Variable,
    }
}

/// Determine if a symbol is exported based on language conventions.
fn is_exported_symbol(name: &str, lang: crate::types::Language) -> bool {
    match lang {
        crate::types::Language::Go => name.starts_with(|c: char| c.is_uppercase()),
        crate::types::Language::Python => !name.starts_with('_'),
        _ => true,
    }
}

/// Strip surrounding quotes from a string.
fn strip_quotes(s: &str) -> String {
    s.trim_matches(|c| c == '"' || c == '\'').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_function_kind() {
        assert_eq!(
            infer_kind_from_node("function_declaration"),
            SymbolKind::Function
        );
        assert_eq!(infer_kind_from_node("function_item"), SymbolKind::Function);
    }

    #[test]
    fn infer_class_kind() {
        assert_eq!(infer_kind_from_node("class_declaration"), SymbolKind::Class);
    }

    #[test]
    fn infer_interface_kind() {
        assert_eq!(
            infer_kind_from_node("interface_declaration"),
            SymbolKind::Interface
        );
    }

    #[test]
    fn infer_unknown_kind() {
        assert_eq!(
            infer_kind_from_node("some_random_node"),
            SymbolKind::Variable
        );
    }

    #[test]
    fn strip_quotes_double() {
        assert_eq!(strip_quotes("\"./types\""), "./types");
    }

    #[test]
    fn strip_quotes_single() {
        assert_eq!(strip_quotes("'./types'"), "./types");
    }

    #[test]
    fn go_export_convention() {
        assert!(is_exported_symbol("FetchUser", crate::types::Language::Go));
        assert!(!is_exported_symbol("fetchUser", crate::types::Language::Go));
    }

    #[test]
    fn python_export_convention() {
        assert!(is_exported_symbol(
            "fetch_user",
            crate::types::Language::Python
        ));
        assert!(!is_exported_symbol(
            "_internal",
            crate::types::Language::Python
        ));
    }
}
