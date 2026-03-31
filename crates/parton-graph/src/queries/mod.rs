//! Tree-sitter query patterns per language.
//!
//! Each language module defines S-expression queries for extracting
//! exports, imports, and symbol signatures.

pub mod generic;
pub mod go_lang;
pub mod python;
pub mod rust_lang;
pub mod typescript;

use crate::types::Language;

/// Query strings for a language.
#[derive(Debug)]
pub struct LangQueries {
    /// Query for exported/public symbol declarations.
    pub exports: &'static str,
    /// Query for import statements.
    pub imports: &'static str,
}

/// Get the query patterns for a language.
pub fn queries_for(lang: Language) -> Option<LangQueries> {
    match lang {
        Language::TypeScript | Language::JavaScript => Some(typescript::QUERIES),
        Language::Rust => Some(rust_lang::QUERIES),
        Language::Python => Some(python::QUERIES),
        Language::Go => Some(go_lang::QUERIES),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_exist_for_major_languages() {
        assert!(queries_for(Language::TypeScript).is_some());
        assert!(queries_for(Language::JavaScript).is_some());
        assert!(queries_for(Language::Rust).is_some());
        assert!(queries_for(Language::Python).is_some());
        assert!(queries_for(Language::Go).is_some());
    }

    #[test]
    fn queries_none_for_unsupported() {
        assert!(queries_for(Language::Unknown).is_none());
    }
}
