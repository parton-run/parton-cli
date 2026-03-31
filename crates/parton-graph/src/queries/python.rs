//! Tree-sitter queries for Python.
//!
//! Extracts top-level definitions (functions, classes) and import statements.
//! Python has no explicit export — top-level defs are treated as public.

use super::LangQueries;

/// Python query patterns.
pub const QUERIES: LangQueries = LangQueries {
    exports: EXPORT_QUERY,
    imports: IMPORT_QUERY,
};

/// Query for top-level definitions (treated as exports).
///
/// Captures:
/// - `@name` — the symbol name
/// - `@decl` — the full declaration node
const EXPORT_QUERY: &str = r#"
(module
  (function_definition
    name: (identifier) @name) @decl)

(module
  (class_definition
    name: (identifier) @name) @decl)

(module
  (expression_statement
    (assignment
      left: (identifier) @name)) @decl)
"#;

/// Query for import statements.
///
/// Captures:
/// - `@source` — the module path
/// - `@symbol` — imported name (for `from X import Y`)
const IMPORT_QUERY: &str = r#"
(import_from_statement
  module_name: (dotted_name) @source
  name: (dotted_name
    (identifier) @symbol))

(import_from_statement
  module_name: (dotted_name) @source
  name: (aliased_import
    name: (dotted_name
      (identifier) @symbol)))

(import_statement
  name: (dotted_name) @source)
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_query_captures_defs() {
        assert!(EXPORT_QUERY.contains("function_definition"));
        assert!(EXPORT_QUERY.contains("class_definition"));
    }

    #[test]
    fn import_query_captures_from_imports() {
        assert!(IMPORT_QUERY.contains("import_from_statement"));
        assert!(IMPORT_QUERY.contains("@source"));
        assert!(IMPORT_QUERY.contains("@symbol"));
    }
}
