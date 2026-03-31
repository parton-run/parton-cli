//! Tree-sitter queries for Go.
//!
//! Extracts exported symbols (capitalized names are public in Go)
//! and import statements.

use super::LangQueries;

/// Go query patterns.
pub const QUERIES: LangQueries = LangQueries {
    exports: EXPORT_QUERY,
    imports: IMPORT_QUERY,
};

/// Query for top-level declarations.
///
/// Go uses capitalization for visibility — we capture all top-level
/// declarations and filter by name casing in the parser.
///
/// Captures:
/// - `@name` — the symbol name
/// - `@decl` — the full declaration node
const EXPORT_QUERY: &str = r#"
(function_declaration
  name: (identifier) @name) @decl

(method_declaration
  name: (field_identifier) @name) @decl

(type_declaration
  (type_spec
    name: (type_identifier) @name)) @decl
"#;

/// Query for import statements.
///
/// Captures:
/// - `@source` — the import path string
const IMPORT_QUERY: &str = r#"
(import_declaration
  (import_spec
    path: (interpreted_string_literal) @source))

(import_declaration
  (import_spec_list
    (import_spec
      path: (interpreted_string_literal) @source)))
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_query_captures_funcs_and_types() {
        assert!(EXPORT_QUERY.contains("function_declaration"));
        assert!(EXPORT_QUERY.contains("type_declaration"));
        assert!(EXPORT_QUERY.contains("method_declaration"));
    }

    #[test]
    fn import_query_captures_paths() {
        assert!(IMPORT_QUERY.contains("import_declaration"));
        assert!(IMPORT_QUERY.contains("@source"));
    }
}
