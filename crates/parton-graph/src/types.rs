//! Core data types for the code graph.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Supported programming languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    /// TypeScript (.ts, .tsx).
    TypeScript,
    /// JavaScript (.js, .jsx, .mjs).
    JavaScript,
    /// Rust (.rs).
    Rust,
    /// Python (.py).
    Python,
    /// Go (.go).
    Go,
    /// Java (.java).
    Java,
    /// C (.c, .h).
    C,
    /// C++ (.cpp, .hpp, .cc).
    Cpp,
    /// Ruby (.rb).
    Ruby,
    /// PHP (.php).
    Php,
    /// Swift (.swift).
    Swift,
    /// Kotlin (.kt).
    Kotlin,
    /// Unknown or unsupported.
    Unknown,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TypeScript => write!(f, "typescript"),
            Self::JavaScript => write!(f, "javascript"),
            Self::Rust => write!(f, "rust"),
            Self::Python => write!(f, "python"),
            Self::Go => write!(f, "go"),
            Self::Java => write!(f, "java"),
            Self::C => write!(f, "c"),
            Self::Cpp => write!(f, "cpp"),
            Self::Ruby => write!(f, "ruby"),
            Self::Php => write!(f, "php"),
            Self::Swift => write!(f, "swift"),
            Self::Kotlin => write!(f, "kotlin"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Kind of symbol in the code graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    /// A function or method.
    Function,
    /// A class.
    Class,
    /// A type alias or interface.
    Type,
    /// An interface (TS/Java/Go).
    Interface,
    /// A trait (Rust).
    Trait,
    /// A variable or constant.
    Variable,
    /// An enum.
    Enum,
    /// A struct (Rust/Go).
    Struct,
    /// A module or namespace.
    Module,
}

/// A parsed symbol from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol name.
    pub name: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// Full signature text (e.g. `fn fetch_user(id: &str) -> User`).
    pub signature: String,
    /// Start line in the file (1-indexed).
    pub line_start: u32,
    /// End line in the file (1-indexed).
    pub line_end: u32,
    /// Whether this symbol is exported/public.
    pub exported: bool,
}

/// An import edge from one file to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEdge {
    /// Path of the file being imported from.
    pub from_path: String,
    /// Imported symbol names.
    pub symbols: Vec<String>,
}

/// A parsed file with its symbols and imports.
#[derive(Debug, Clone)]
pub struct FileNode {
    /// Relative file path.
    pub path: String,
    /// Detected language.
    pub language: Language,
    /// Symbols defined in this file.
    pub symbols: Vec<Symbol>,
    /// Imports from other files.
    pub imports: Vec<ImportEdge>,
}

/// In-memory code graph.
#[derive(Debug, Default)]
pub struct CodeGraph {
    /// Files indexed by relative path.
    pub files: HashMap<String, FileNode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_display() {
        assert_eq!(Language::TypeScript.to_string(), "typescript");
        assert_eq!(Language::Rust.to_string(), "rust");
        assert_eq!(Language::Unknown.to_string(), "unknown");
    }

    #[test]
    fn symbol_debug() {
        let sym = Symbol {
            name: "fetch_user".into(),
            kind: SymbolKind::Function,
            signature: "pub async fn fetch_user(id: &str) -> User".into(),
            line_start: 10,
            line_end: 25,
            exported: true,
        };
        assert!(format!("{sym:?}").contains("fetch_user"));
    }

    #[test]
    fn code_graph_default_is_empty() {
        let graph = CodeGraph::default();
        assert!(graph.files.is_empty());
    }

    #[test]
    fn file_node_stores_symbols() {
        let node = FileNode {
            path: "src/app.ts".into(),
            language: Language::TypeScript,
            symbols: vec![Symbol {
                name: "App".into(),
                kind: SymbolKind::Function,
                signature: "export function App(): JSX.Element".into(),
                line_start: 1,
                line_end: 10,
                exported: true,
            }],
            imports: vec![ImportEdge {
                from_path: "./types".into(),
                symbols: vec!["User".into()],
            }],
        };
        assert_eq!(node.symbols.len(), 1);
        assert_eq!(node.imports.len(), 1);
    }
}
