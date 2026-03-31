//! Tree-sitter queries for Rust.
//!
//! Extracts public symbols (functions, structs, enums, traits, type aliases)
//! and use/import statements.

use super::LangQueries;

/// Rust query patterns.
pub const QUERIES: LangQueries = LangQueries {
    exports: EXPORT_QUERY,
    imports: IMPORT_QUERY,
};

/// Query for public declarations.
///
/// Captures:
/// - `@name` — the symbol name
/// - `@decl` — the full declaration node
const EXPORT_QUERY: &str = r#"
(function_item
  (visibility_modifier) @_vis
  name: (identifier) @name) @decl

(struct_item
  (visibility_modifier) @_vis
  name: (type_identifier) @name) @decl

(enum_item
  (visibility_modifier) @_vis
  name: (type_identifier) @name) @decl

(trait_item
  (visibility_modifier) @_vis
  name: (type_identifier) @name) @decl

(type_item
  (visibility_modifier) @_vis
  name: (type_identifier) @name) @decl

(impl_item
  type: (type_identifier) @name) @decl
"#;

/// Query for use statements.
///
/// Captures:
/// - `@path` — the full use path
const IMPORT_QUERY: &str = r#"
(use_declaration
  argument: (scoped_identifier) @path)

(use_declaration
  argument: (use_as_clause
    path: (scoped_identifier) @path))

(use_declaration
  argument: (scoped_use_list
    path: (scoped_identifier) @path))
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_query_captures_pub_items() {
        assert!(EXPORT_QUERY.contains("visibility_modifier"));
        assert!(EXPORT_QUERY.contains("function_item"));
        assert!(EXPORT_QUERY.contains("struct_item"));
        assert!(EXPORT_QUERY.contains("trait_item"));
    }

    #[test]
    fn import_query_captures_use() {
        assert!(IMPORT_QUERY.contains("use_declaration"));
        assert!(IMPORT_QUERY.contains("@path"));
    }
}
