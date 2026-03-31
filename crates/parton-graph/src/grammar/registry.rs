//! Grammar URL registry for lazy download.
//!
//! Maps supported languages to their pre-built `.wasm` grammar URLs
//! hosted on the tree-sitter GitHub organization site.

use crate::types::Language;

/// Base URL for pre-built tree-sitter `.wasm` grammars.
const BASE_URL: &str = "https://raw.githubusercontent.com/tree-sitter/tree-sitter.github.io/master";

/// Get the download URL for a language grammar `.wasm` file.
pub fn grammar_url(lang: Language) -> Option<String> {
    let name = grammar_name(lang)?;
    Some(format!("{BASE_URL}/tree-sitter-{name}.wasm"))
}

/// Get the local cache filename for a grammar.
pub fn grammar_filename(lang: Language) -> Option<String> {
    let name = grammar_name(lang)?;
    Some(format!("tree-sitter-{name}.wasm"))
}

/// Get the tree-sitter grammar name used in `WasmStore::load_language`.
pub fn grammar_load_name(lang: Language) -> Option<&'static str> {
    match lang {
        Language::TypeScript => Some("typescript"),
        Language::JavaScript => Some("javascript"),
        Language::Rust => Some("rust"),
        Language::Python => Some("python"),
        Language::Go => Some("go"),
        Language::Java => Some("java"),
        Language::C => Some("c"),
        Language::Cpp => Some("cpp"),
        Language::Ruby => Some("ruby"),
        Language::Php => Some("php"),
        Language::Swift => None,
        Language::Kotlin => Some("kotlin"),
        Language::Unknown => None,
    }
}

/// Internal name mapping for download URLs.
fn grammar_name(lang: Language) -> Option<&'static str> {
    match lang {
        Language::TypeScript => Some("typescript"),
        Language::JavaScript => Some("javascript"),
        Language::Rust => Some("rust"),
        Language::Python => Some("python"),
        Language::Go => Some("go"),
        Language::Java => Some("java"),
        Language::C => Some("c"),
        Language::Cpp => Some("cpp"),
        Language::Ruby => Some("ruby"),
        Language::Php => Some("php"),
        Language::Kotlin => Some("kotlin"),
        Language::Swift | Language::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_for_typescript() {
        let url = grammar_url(Language::TypeScript).unwrap();
        assert!(url.contains("tree-sitter-typescript.wasm"));
        assert!(url.starts_with("https://"));
    }

    #[test]
    fn url_for_all_supported() {
        let supported = [
            Language::TypeScript,
            Language::JavaScript,
            Language::Rust,
            Language::Python,
            Language::Go,
            Language::Java,
            Language::C,
            Language::Cpp,
            Language::Ruby,
            Language::Php,
            Language::Kotlin,
        ];
        for lang in supported {
            assert!(grammar_url(lang).is_some(), "missing URL for {lang}");
            assert!(
                grammar_filename(lang).is_some(),
                "missing filename for {lang}"
            );
            assert!(
                grammar_load_name(lang).is_some(),
                "missing load name for {lang}"
            );
        }
    }

    #[test]
    fn url_none_for_unknown() {
        assert!(grammar_url(Language::Unknown).is_none());
        assert!(grammar_url(Language::Swift).is_none());
    }

    #[test]
    fn filename_format() {
        let name = grammar_filename(Language::Python).unwrap();
        assert_eq!(name, "tree-sitter-python.wasm");
    }

    #[test]
    fn load_name_matches_tree_sitter() {
        assert_eq!(grammar_load_name(Language::TypeScript), Some("typescript"));
        assert_eq!(grammar_load_name(Language::Rust), Some("rust"));
    }
}
