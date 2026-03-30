//! Knowledge entry types.

use serde::{Deserialize, Serialize};

/// Category of a knowledge entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    /// Code convention or style rule.
    Convention,
    /// Architecture decision record.
    ArchitectureDecision,
    /// Technology stack information.
    StackInfo,
    /// Enforced project rule.
    Rule,
    /// Recurring code pattern.
    Pattern,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Convention => write!(f, "convention"),
            Self::ArchitectureDecision => write!(f, "architecture-decision"),
            Self::StackInfo => write!(f, "stack-info"),
            Self::Rule => write!(f, "rule"),
            Self::Pattern => write!(f, "pattern"),
        }
    }
}

/// How a knowledge entry was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// Manually added by a user.
    Manual,
    /// Extracted by an LLM.
    Llm,
    /// Auto-detected from code analysis.
    Auto,
}

/// A single knowledge entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Unique identifier.
    pub id: String,
    /// Entry category.
    pub category: Category,
    /// Short title.
    pub title: String,
    /// Full content.
    pub content: String,
    /// Searchable tags.
    pub tags: Vec<String>,
    /// How this entry was created.
    pub source: Source,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_display() {
        assert_eq!(Category::Convention.to_string(), "convention");
        assert_eq!(
            Category::ArchitectureDecision.to_string(),
            "architecture-decision"
        );
    }

    #[test]
    fn entry_serde_roundtrip() {
        let entry = Entry {
            id: "conv-01".into(),
            category: Category::Convention,
            title: "Use named exports".into(),
            content: "Always use named exports, never default".into(),
            tags: vec!["typescript".into(), "exports".into()],
            source: Source::Auto,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: Entry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "conv-01");
        assert_eq!(parsed.tags.len(), 2);
    }
}
