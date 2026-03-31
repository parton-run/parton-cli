//! Language detection from file extensions.

use std::path::Path;

use crate::types::Language;

/// Detect the programming language from a file path.
pub fn detect_language(path: &str) -> Language {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "ts" | "tsx" | "mts" | "cts" => Language::TypeScript,
        "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
        "rs" => Language::Rust,
        "py" | "pyi" => Language::Python,
        "go" => Language::Go,
        "java" => Language::Java,
        "c" | "h" => Language::C,
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Language::Cpp,
        "rb" => Language::Ruby,
        "php" => Language::Php,
        "swift" => Language::Swift,
        "kt" | "kts" => Language::Kotlin,
        _ => Language::Unknown,
    }
}

/// Check whether a language is supported for tree-sitter parsing.
pub fn is_supported(lang: Language) -> bool {
    !matches!(lang, Language::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_typescript() {
        assert_eq!(detect_language("src/app.ts"), Language::TypeScript);
        assert_eq!(detect_language("src/App.tsx"), Language::TypeScript);
        assert_eq!(detect_language("lib/index.mts"), Language::TypeScript);
    }

    #[test]
    fn detect_javascript() {
        assert_eq!(detect_language("src/app.js"), Language::JavaScript);
        assert_eq!(detect_language("src/App.jsx"), Language::JavaScript);
        assert_eq!(detect_language("lib/index.mjs"), Language::JavaScript);
        assert_eq!(detect_language("lib/config.cjs"), Language::JavaScript);
    }

    #[test]
    fn detect_rust() {
        assert_eq!(detect_language("src/main.rs"), Language::Rust);
    }

    #[test]
    fn detect_python() {
        assert_eq!(detect_language("app.py"), Language::Python);
        assert_eq!(detect_language("types.pyi"), Language::Python);
    }

    #[test]
    fn detect_go() {
        assert_eq!(detect_language("main.go"), Language::Go);
    }

    #[test]
    fn detect_java() {
        assert_eq!(detect_language("App.java"), Language::Java);
    }

    #[test]
    fn detect_c_cpp() {
        assert_eq!(detect_language("main.c"), Language::C);
        assert_eq!(detect_language("lib.h"), Language::C);
        assert_eq!(detect_language("main.cpp"), Language::Cpp);
        assert_eq!(detect_language("lib.hpp"), Language::Cpp);
    }

    #[test]
    fn detect_ruby() {
        assert_eq!(detect_language("app.rb"), Language::Ruby);
    }

    #[test]
    fn detect_php() {
        assert_eq!(detect_language("index.php"), Language::Php);
    }

    #[test]
    fn detect_swift() {
        assert_eq!(detect_language("App.swift"), Language::Swift);
    }

    #[test]
    fn detect_kotlin() {
        assert_eq!(detect_language("App.kt"), Language::Kotlin);
        assert_eq!(detect_language("build.kts"), Language::Kotlin);
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(detect_language("readme.md"), Language::Unknown);
        assert_eq!(detect_language("Makefile"), Language::Unknown);
        assert_eq!(detect_language(".gitignore"), Language::Unknown);
    }

    #[test]
    fn is_supported_known_languages() {
        assert!(is_supported(Language::TypeScript));
        assert!(is_supported(Language::Rust));
        assert!(!is_supported(Language::Unknown));
    }
}
