//! Tree-sitter queries for TypeScript and JavaScript.
//!
//! Extracts exported symbols (functions, classes, interfaces, types,
//! enums) and import statements.

use super::LangQueries;

/// TypeScript/JavaScript query patterns.
pub const QUERIES: LangQueries = LangQueries {
    exports: EXPORT_QUERY,
    imports: IMPORT_QUERY,
};

/// Query for exported declarations.
///
/// Captures:
/// - `@name` — the symbol name
/// - `@decl` — the full declaration node (for signature extraction)
const EXPORT_QUERY: &str = r#"
(export_statement
  declaration: (function_declaration
    name: (identifier) @name) @decl)

(export_statement
  declaration: (class_declaration
    name: (type_identifier) @name) @decl)

(export_statement
  declaration: (interface_declaration
    name: (type_identifier) @name) @decl)

(export_statement
  declaration: (type_alias_declaration
    name: (type_identifier) @name) @decl)

(export_statement
  declaration: (enum_declaration
    name: (identifier) @name) @decl)

(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (identifier) @name)) @decl)
"#;

/// Query for import statements.
///
/// Captures:
/// - `@source` — the module path string
/// - `@symbol` — each imported symbol name
const IMPORT_QUERY: &str = r#"
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @symbol)))
  source: (string) @source)

(import_statement
  (import_clause
    (identifier) @symbol)
  source: (string) @source)
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_query_not_empty() {
        assert!(!QUERIES.exports.trim().is_empty());
    }

    #[test]
    fn import_query_not_empty() {
        assert!(!QUERIES.imports.trim().is_empty());
    }

    #[test]
    fn export_query_captures_name_and_decl() {
        assert!(EXPORT_QUERY.contains("@name"));
        assert!(EXPORT_QUERY.contains("@decl"));
    }

    #[test]
    fn import_query_captures_source_and_symbol() {
        assert!(IMPORT_QUERY.contains("@source"));
        assert!(IMPORT_QUERY.contains("@symbol"));
    }
}
