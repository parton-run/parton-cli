//! Post-run learning extraction.
//!
//! After a run completes, extract lessons learned and store them
//! for future runs.

use parton_core::{ModelProvider, ProviderError};

use crate::entry::{Category, Entry, Source};
use crate::store::KnowledgeStore;

/// Extract learnings from a completed run and store them.
///
/// Sends the run summary to the LLM to extract reusable knowledge.
pub async fn extract_and_store(
    run_summary: &str,
    provider: &dyn ModelProvider,
    store: &dyn KnowledgeStore,
) -> Result<Vec<Entry>, ProviderError> {
    let response = provider.send(LEARNING_PROMPT, run_summary, false).await?;
    let learnings = parse_learnings(&response.content);

    for entry in &learnings {
        let _ = store.upsert(entry);
    }

    Ok(learnings)
}

/// Build a context string from stored knowledge for the planner.
///
/// Returns relevant knowledge entries formatted for inclusion in the planning prompt.
pub fn build_knowledge_context(store: &dyn KnowledgeStore) -> String {
    let entries = store.list().unwrap_or_default();
    if entries.is_empty() {
        return String::new();
    }

    let mut sections = vec!["# Project Knowledge".to_string()];

    // Group by category.
    let stack: Vec<&Entry> = entries.iter().filter(|e| matches!(e.category, Category::StackInfo)).collect();
    let conventions: Vec<&Entry> = entries.iter().filter(|e| matches!(e.category, Category::Convention)).collect();
    let patterns: Vec<&Entry> = entries.iter().filter(|e| matches!(e.category, Category::Pattern)).collect();
    let rules: Vec<&Entry> = entries.iter().filter(|e| matches!(e.category, Category::Rule)).collect();

    if !stack.is_empty() {
        sections.push("## Stack".into());
        for e in &stack {
            sections.push(format!("- {}", e.content));
        }
    }

    if !conventions.is_empty() {
        sections.push("## Conventions".into());
        for e in &conventions {
            sections.push(format!("- {}", e.content));
        }
    }

    if !patterns.is_empty() {
        sections.push("## Patterns".into());
        for e in &patterns {
            sections.push(format!("- {}: {}", e.title, e.content));
        }
    }

    if !rules.is_empty() {
        sections.push("## Rules".into());
        for e in &rules {
            sections.push(format!("- {}", e.content));
        }
    }

    sections.join("\n")
}

const LEARNING_PROMPT: &str = r#"You are a learning extractor. Given a summary of a code generation run, extract reusable lessons.

Return ONLY a JSON array of learnings:
[
  {
    "id": "learning-xxx",
    "category": "Convention" | "Pattern" | "Rule",
    "title": "short title",
    "content": "what was learned",
    "tags": ["tag1", "tag2"]
  }
]

RULES:
- Only extract things that would be useful for FUTURE runs on this project.
- Focus on: naming patterns, import conventions, architecture decisions, gotchas.
- Do NOT extract generic programming knowledge.
- If nothing useful was learned, return an empty array [].
- Return valid JSON only."#;

/// Parse learning entries from LLM response.
fn parse_learnings(content: &str) -> Vec<Entry> {
    #[derive(serde::Deserialize)]
    struct RawLearning {
        id: String,
        category: String,
        title: String,
        content: String,
        tags: Vec<String>,
    }

    let trimmed = content.trim();
    let json_str = if let Some(start) = trimmed.find('[') {
        &trimmed[start..]
    } else {
        trimmed
    };

    let raw: Vec<RawLearning> = serde_json::from_str(json_str).unwrap_or_default();

    raw.into_iter()
        .map(|r| Entry {
            id: r.id,
            category: match r.category.as_str() {
                "Pattern" => Category::Pattern,
                "Rule" => Category::Rule,
                _ => Category::Convention,
            },
            title: r.title,
            content: r.content,
            tags: r.tags,
            source: Source::Llm,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_learnings() {
        let json = r#"[
            {"id": "l1", "category": "Convention", "title": "Named exports", "content": "Always use named exports", "tags": ["exports"]},
            {"id": "l2", "category": "Pattern", "title": "Hook pattern", "content": "Custom hooks in hooks/", "tags": ["hooks"]}
        ]"#;
        let entries = parse_learnings(json);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].category, Category::Convention);
        assert_eq!(entries[1].category, Category::Pattern);
    }

    #[test]
    fn parse_empty_array() {
        let entries = parse_learnings("[]");
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_invalid_returns_empty() {
        let entries = parse_learnings("not json");
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_with_surrounding_text() {
        let input = r#"Here are the learnings: [{"id":"l1","category":"Rule","title":"t","content":"c","tags":[]}]"#;
        let entries = parse_learnings(input);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn build_context_empty_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::LocalStore::new(dir.path());
        let ctx = build_knowledge_context(&store);
        assert!(ctx.is_empty());
    }

    #[test]
    fn build_context_with_entries() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::LocalStore::new(dir.path());
        store.upsert(&Entry {
            id: "s1".into(),
            category: Category::StackInfo,
            title: "TypeScript".into(),
            content: "Primary language: TypeScript".into(),
            tags: vec![],
            source: Source::Auto,
        }).unwrap();
        store.upsert(&Entry {
            id: "c1".into(),
            category: Category::Convention,
            title: "Named exports".into(),
            content: "Use named exports everywhere".into(),
            tags: vec![],
            source: Source::Auto,
        }).unwrap();

        let ctx = build_knowledge_context(&store);
        assert!(ctx.contains("Project Knowledge"));
        assert!(ctx.contains("TypeScript"));
        assert!(ctx.contains("named exports"));
    }
}
