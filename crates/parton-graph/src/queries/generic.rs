//! Regex-based fallback for languages without tree-sitter queries.
//!
//! Extracts basic symbol declarations using simple pattern matching.
//! Less accurate than tree-sitter but provides some context for
//! unsupported languages.

use crate::types::{ImportEdge, Symbol, SymbolKind};

/// Extract symbols from source code using regex heuristics.
///
/// Looks for common declaration patterns across languages.
pub fn extract_symbols(source: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let line_start = (line_num + 1) as u32;

        if let Some(sym) = try_extract_function(trimmed, line_start) {
            symbols.push(sym);
        } else if let Some(sym) = try_extract_class_or_struct(trimmed, line_start) {
            symbols.push(sym);
        }
    }

    symbols
}

/// Extract import-like statements using regex heuristics.
pub fn extract_imports(source: &str) -> Vec<ImportEdge> {
    let mut imports = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(edge) = try_extract_import(trimmed) {
            imports.push(edge);
        }
    }

    imports
}

/// Try to match function-like declarations.
fn try_extract_function(line: &str, line_start: u32) -> Option<Symbol> {
    let patterns = [
        ("export function ", true),
        ("export async function ", true),
        ("export const ", true),
        ("pub fn ", true),
        ("pub async fn ", true),
        ("def ", false),
        ("func ", false),
    ];

    for (pat, exported) in patterns {
        if line.starts_with(pat) || line.contains(pat) {
            let after = line.split(pat).nth(1)?;
            let name = after.split(&['(', ':', '<', ' ', '{'][..]).next()?;
            if !name.is_empty() && name.chars().next()?.is_alphabetic() {
                return Some(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    signature: line.to_string(),
                    line_start,
                    line_end: line_start,
                    exported,
                });
            }
        }
    }

    None
}

/// Try to match class/struct/interface declarations.
fn try_extract_class_or_struct(line: &str, line_start: u32) -> Option<Symbol> {
    let patterns = [
        ("export class ", SymbolKind::Class, true),
        ("export interface ", SymbolKind::Interface, true),
        ("export type ", SymbolKind::Type, true),
        ("export enum ", SymbolKind::Enum, true),
        ("pub struct ", SymbolKind::Struct, true),
        ("pub enum ", SymbolKind::Enum, true),
        ("pub trait ", SymbolKind::Trait, true),
        ("class ", SymbolKind::Class, false),
        ("struct ", SymbolKind::Struct, false),
    ];

    for (pat, kind, exported) in patterns {
        if let Some(after) = line.strip_prefix(pat) {
            let name = after.split(&['<', ' ', '{', '(', ':'][..]).next()?;
            if !name.is_empty() && name.chars().next()?.is_alphabetic() {
                return Some(Symbol {
                    name: name.to_string(),
                    kind,
                    signature: line.to_string(),
                    line_start,
                    line_end: line_start,
                    exported,
                });
            }
        }
    }

    None
}

/// Try to extract an import statement.
fn try_extract_import(line: &str) -> Option<ImportEdge> {
    // import { X, Y } from './path'
    if line.starts_with("import ") && line.contains("from ") {
        let source = extract_string_literal(line.split("from ").nth(1)?)?;
        let symbols = extract_named_imports(line);
        return Some(ImportEdge {
            from_path: source,
            symbols,
        });
    }

    // from module import x, y
    if line.starts_with("from ") && line.contains(" import ") {
        let module = line.strip_prefix("from ")?.split(" import ").next()?;
        let imports_part = line.split(" import ").nth(1)?;
        let symbols: Vec<String> = imports_part
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        return Some(ImportEdge {
            from_path: module.trim().to_string(),
            symbols,
        });
    }

    None
}

/// Extract a string literal value (removes quotes).
fn extract_string_literal(s: &str) -> Option<String> {
    let trimmed = s.trim().trim_end_with(';');
    let inner = trimmed
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
        })?;
    Some(inner.to_string())
}

/// Extract named imports from `{ X, Y }`.
fn extract_named_imports(line: &str) -> Vec<String> {
    if let Some(start) = line.find('{') {
        if let Some(end) = line.find('}') {
            return line[start + 1..end]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && !s.contains(' '))
                .collect();
        }
    }
    vec![]
}

/// Extension for trimming specific suffix patterns.
trait TrimSuffix {
    fn trim_end_with(&self, c: char) -> &str;
}

impl TrimSuffix for str {
    fn trim_end_with(&self, c: char) -> &str {
        self.strip_suffix(c).unwrap_or(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_ts_exported_function() {
        let symbols = extract_symbols("export function fetchUser(id: string): User {");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "fetchUser");
        assert!(symbols[0].exported);
    }

    #[test]
    fn extract_rust_pub_fn() {
        let symbols = extract_symbols("pub fn process(data: &[u8]) -> Result<()> {");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "process");
    }

    #[test]
    fn extract_ts_class() {
        let symbols = extract_symbols("export class UserService {");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Class);
    }

    #[test]
    fn extract_ts_import() {
        let imports = extract_imports("import { User, Filter } from './types';");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].from_path, "./types");
        assert_eq!(imports[0].symbols, vec!["User", "Filter"]);
    }

    #[test]
    fn extract_python_import() {
        let imports = extract_imports("from models import User, Post");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].from_path, "models");
        assert_eq!(imports[0].symbols, vec!["User", "Post"]);
    }

    #[test]
    fn empty_source_returns_empty() {
        assert!(extract_symbols("").is_empty());
        assert!(extract_imports("").is_empty());
    }
}
