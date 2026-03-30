//! Graph data types.

use serde::{Deserialize, Serialize};

/// Unique file identifier.
pub type FileId = i64;

/// Unique symbol identifier.
pub type SymbolId = i64;

/// Kind of symbol in the code graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    /// A function or method.
    Function,
    /// A class.
    Class,
    /// A type alias or interface.
    Type,
    /// A variable or constant.
    Variable,
    /// An enum.
    Enum,
    /// A module or namespace.
    Module,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Class => write!(f, "class"),
            Self::Type => write!(f, "type"),
            Self::Variable => write!(f, "variable"),
            Self::Enum => write!(f, "enum"),
            Self::Module => write!(f, "module"),
        }
    }
}

/// Kind of relationship between symbols.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipKind {
    /// Symbol A imports/uses symbol B.
    Imports,
    /// Symbol A extends/inherits from B.
    Extends,
    /// Symbol A implements interface/trait B.
    Implements,
    /// Symbol A calls function B.
    Calls,
}

impl std::fmt::Display for RelationshipKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Imports => write!(f, "imports"),
            Self::Extends => write!(f, "extends"),
            Self::Implements => write!(f, "implements"),
            Self::Calls => write!(f, "calls"),
        }
    }
}

/// A symbol in the code graph.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Unique ID.
    pub id: SymbolId,
    /// Symbol name.
    pub name: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// File this symbol belongs to.
    pub file_id: FileId,
    /// Start line in the file.
    pub line_start: u32,
    /// End line in the file.
    pub line_end: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_kind_display() {
        assert_eq!(SymbolKind::Function.to_string(), "function");
        assert_eq!(SymbolKind::Class.to_string(), "class");
    }

    #[test]
    fn relationship_kind_display() {
        assert_eq!(RelationshipKind::Imports.to_string(), "imports");
        assert_eq!(RelationshipKind::Extends.to_string(), "extends");
    }
}
