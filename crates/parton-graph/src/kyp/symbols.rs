//! Per-cluster symbol extraction with full signatures.

use crate::kyp::format::{MapSymbol, SymKind};
use crate::types::{CodeGraph, SymbolKind};

/// Maximum symbols per concept before truncation.
const MAX_SYMBOLS_PER_CONCEPT: usize = 25;

/// Extract exported symbols from a cluster's files, filtered and ranked.
///
/// Filters out noise (Props types, empty sigs, test helpers) and
/// caps at MAX_SYMBOLS_PER_CONCEPT. The map is the primary context
/// but shouldn't be bloated with derivative types.
pub fn extract_cluster_symbols(file_paths: &[String], graph: &CodeGraph) -> Vec<MapSymbol> {
    let import_counts = count_imports(graph);
    let mut symbols: Vec<(MapSymbol, u32)> = Vec::new();

    for path in file_paths {
        let node = match graph.get_file(path) {
            Some(n) => n,
            None => continue,
        };
        for sym in &node.symbols {
            if !sym.exported {
                continue;
            }
            if should_skip_symbol(&sym.name, &sym.signature) {
                continue;
            }
            // Deduplicate by name.
            if symbols.iter().any(|(s, _)| s.name == sym.name) {
                continue;
            }
            let count = import_counts.get(&sym.name).copied().unwrap_or(0);
            symbols.push((
                MapSymbol {
                    name: sym.name.clone(),
                    kind: map_kind(&sym.kind),
                    signature: compact_signature(&sym.signature, &sym.name),
                },
                count,
            ));
        }
    }

    // Sort by import count desc, then kind order, then alphabetical.
    symbols.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(kind_order(&a.0.kind).cmp(&kind_order(&b.0.kind)))
            .then(a.0.name.cmp(&b.0.name))
    });

    // Truncate to cap.
    symbols.truncate(MAX_SYMBOLS_PER_CONCEPT);
    symbols.into_iter().map(|(s, _)| s).collect()
}

/// Count how many files import each symbol name across the project.
fn count_imports(graph: &CodeGraph) -> std::collections::HashMap<String, u32> {
    let mut counts = std::collections::HashMap::new();
    for node in graph.files.values() {
        for edge in &node.imports {
            for sym in &edge.symbols {
                *counts.entry(sym.clone()).or_default() += 1;
            }
        }
    }
    counts
}

/// Filter out symbols that add noise without value for the planner.
fn should_skip_symbol(name: &str, signature: &str) -> bool {
    // Skip *Props types — derivable from component names.
    if name.ends_with("Props") {
        return true;
    }
    // Skip *Schema suffixed (zod schemas — implementation detail).
    if name.ends_with("Schema") || name.ends_with("schema") {
        return true;
    }
    // Skip SCREAMING_CASE constants (config values, not API surface).
    if name.len() > 3 && name == name.to_uppercase() && name.contains('_') {
        return true;
    }
    // Skip test helpers leaking from #[test] modules.
    if signature.contains("#[test]") || signature.contains("#[cfg(test)]") {
        return true;
    }
    // Skip very short names that are likely re-exports or noise.
    if name.len() <= 1 {
        return true;
    }
    false
}

/// Compact a full signature into the map format.
///
/// `export async function checkAdmin(email: string): Promise<boolean>`
/// → `(email:string):Promise<boolean>`
fn compact_signature(sig: &str, name: &str) -> String {
    if sig.is_empty() || sig == name {
        return String::new();
    }

    // Extract params + return type from function signature.
    if let Some(open) = sig.find('(') {
        let from_open = &sig[open..];
        // Find matching close paren.
        let params_end = find_matching_paren(from_open);
        let params = &from_open[..params_end];

        // Compact params: remove spaces after colons/commas.
        let compact_params = params.replace(": ", ":").replace(", ", ",");

        // Extract return type.
        let after_params = &from_open[params_end..];
        let ret = extract_return_type(after_params);

        if ret.is_empty() {
            compact_params
        } else {
            format!("{compact_params}:{ret}")
        }
    } else if sig.contains('{') {
        // Type/interface with fields: extract field names.
        extract_fields(sig)
    } else {
        String::new()
    }
}

/// Find the position after the matching `)` for an opening `(`.
fn find_matching_paren(s: &str) -> usize {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    s.len()
}

/// Extract return type from after the params.
fn extract_return_type(s: &str) -> String {
    // Look for `: ReturnType` or `=> ReturnType`
    let trimmed = s.trim();
    let after = if let Some(rest) = trimmed.strip_prefix(':') {
        rest.trim()
    } else if let Some(rest) = trimmed.strip_prefix("=>") {
        rest.trim()
    } else {
        return String::new();
    };

    // Take until `{` or end of line.
    after
        .split('{')
        .next()
        .unwrap_or(after)
        .trim()
        .trim_end_matches(';')
        .to_string()
}

/// Extract field names from a type/interface signature.
fn extract_fields(sig: &str) -> String {
    if let Some(open) = sig.find('{') {
        if let Some(close) = sig.rfind('}') {
            let inner = &sig[open..=close];
            // Compact: remove spaces, keep just field names.
            let compact = inner
                .replace(": ", ":")
                .replace("; ", ",")
                .replace(", ", ",");
            return compact;
        }
    }
    String::new()
}

/// Map tree-sitter SymbolKind to map SymKind.
fn map_kind(kind: &SymbolKind) -> SymKind {
    match kind {
        SymbolKind::Function => SymKind::Function,
        SymbolKind::Class => SymKind::Component,
        SymbolKind::Type | SymbolKind::Interface => SymKind::Type,
        SymbolKind::Trait => SymKind::Type,
        SymbolKind::Variable => SymKind::Variable,
        SymbolKind::Enum => SymKind::Enum,
        SymbolKind::Struct => SymKind::Type,
        SymbolKind::Module => SymKind::Module,
    }
}

/// Sort order for symbol kinds (functions first).
fn kind_order(kind: &SymKind) -> u8 {
    match kind {
        SymKind::Function => 0,
        SymKind::Component => 1,
        SymKind::Hook => 2,
        SymKind::Type => 3,
        SymKind::Enum => 4,
        SymKind::Schema => 5,
        SymKind::Variable => 6,
        SymKind::Module => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_graph() -> CodeGraph {
        let mut g = CodeGraph::new();
        g.add_file(FileNode {
            path: "lib/auth.ts".into(),
            language: Language::TypeScript,
            symbols: vec![
                Symbol {
                    name: "checkAdmin".into(),
                    kind: SymbolKind::Function,
                    signature: "export function checkAdmin(email: string): boolean".into(),
                    line_start: 1,
                    line_end: 5,
                    exported: true,
                },
                Symbol {
                    name: "AdminUser".into(),
                    kind: SymbolKind::Type,
                    signature: "export interface AdminUser { email: string; id: number }".into(),
                    line_start: 7,
                    line_end: 10,
                    exported: true,
                },
                Symbol {
                    name: "internal".into(),
                    kind: SymbolKind::Function,
                    signature: String::new(),
                    line_start: 12,
                    line_end: 15,
                    exported: false,
                },
            ],
            imports: vec![],
        });
        g
    }

    #[test]
    fn extracts_all_exported() {
        let g = make_graph();
        let syms = extract_cluster_symbols(&["lib/auth.ts".into()], &g);
        assert_eq!(syms.len(), 2);
        assert!(syms.iter().any(|s| s.name == "checkAdmin"));
        assert!(syms.iter().any(|s| s.name == "AdminUser"));
        assert!(!syms.iter().any(|s| s.name == "internal"));
    }

    #[test]
    fn compact_function_signature() {
        let sig = compact_signature(
            "export function checkAdmin(email: string): boolean",
            "checkAdmin",
        );
        assert_eq!(sig, "(email:string):boolean");
    }

    #[test]
    fn compact_async_signature() {
        let sig = compact_signature(
            "export async function GET(req: Request): Promise<Response>",
            "GET",
        );
        assert!(sig.contains("(req:Request)"));
        assert!(sig.contains("Promise<Response>"));
    }

    #[test]
    fn compact_interface_signature() {
        let sig = compact_signature(
            "export interface User { email: string; id: number }",
            "User",
        );
        assert!(sig.contains("email"));
        assert!(sig.contains("id"));
    }

    #[test]
    fn compact_empty_signature() {
        assert_eq!(compact_signature("", "x"), "");
        assert_eq!(compact_signature("x", "x"), "");
    }

    #[test]
    fn skips_props_types() {
        let mut g = CodeGraph::new();
        g.add_file(FileNode {
            path: "ui.tsx".into(),
            language: Language::TypeScript,
            symbols: vec![
                Symbol {
                    name: "Button".into(),
                    kind: SymbolKind::Function,
                    signature: "export function Button()".into(),
                    line_start: 1,
                    line_end: 5,
                    exported: true,
                },
                Symbol {
                    name: "ButtonProps".into(),
                    kind: SymbolKind::Type,
                    signature: "export type ButtonProps".into(),
                    line_start: 7,
                    line_end: 10,
                    exported: true,
                },
            ],
            imports: vec![],
        });
        let syms = extract_cluster_symbols(&["ui.tsx".into()], &g);
        assert!(syms.iter().any(|s| s.name == "Button"));
        assert!(!syms.iter().any(|s| s.name == "ButtonProps"));
    }

    #[test]
    fn skips_screaming_constants() {
        assert!(should_skip_symbol("FREE_DAILY_LIMIT", ""));
        assert!(should_skip_symbol("CHUNK_OVERLAP_TOKENS", ""));
        assert!(!should_skip_symbol("checkAdmin", ""));
        assert!(!should_skip_symbol("db", ""));
    }

    #[test]
    fn deduplicates_symbols() {
        let mut g = CodeGraph::new();
        g.add_file(FileNode {
            path: "a.ts".into(),
            language: Language::TypeScript,
            symbols: vec![Symbol {
                name: "fetchData".into(),
                kind: SymbolKind::Function,
                signature: "fetchData".into(),
                line_start: 1,
                line_end: 1,
                exported: true,
            }],
            imports: vec![],
        });
        g.add_file(FileNode {
            path: "b.ts".into(),
            language: Language::TypeScript,
            symbols: vec![Symbol {
                name: "fetchData".into(),
                kind: SymbolKind::Function,
                signature: "fetchData".into(),
                line_start: 1,
                line_end: 1,
                exported: true,
            }],
            imports: vec![],
        });
        let syms = extract_cluster_symbols(&["a.ts".into(), "b.ts".into()], &g);
        assert_eq!(syms.len(), 1);
    }
}
