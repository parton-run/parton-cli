//! LLM enrichment for KYP maps.
//!
//! Enriches a tree-sitter draft map with semantic #tags by running
//! parallel per-concept LLM calls (10 at a time).

use std::path::Path;

use futures_util::stream::{FuturesUnordered, StreamExt};
use parton_core::{ModelProvider, ProviderError};

use super::format::{Concept, PartonMap, Tag};

/// Maximum lines to read from each file for context.
const MAX_CONTEXT_LINES: usize = 40;

/// System prompt for per-concept enrichment.
const ENRICH_SYSTEM: &str = "\
You are a project indexer. You receive a concept (cluster of files) with source code snippets.
Return 1-3 semantic tags in EXACTLY this format, one per line:

#key:value

RULES:
- Each line starts with # followed by key:value
- Max 120 chars per value
- No sentences. Telegraphic style. Compact.
- No markdown, no explanation, ONLY #key:value lines

GOOD EXAMPLES:
#flow:workos.session.email→adminUsers.lookup→boolean
#used-by:all /admin/* routes via withAdminApi
#stores:s3+postgres
#status-flow:created→approved|rejected

BAD — DO NOT WRITE:
#flow:This function checks if the user is admin  ← TOO VERBOSE
This is a comment  ← MISSING #key:value FORMAT";

/// Enrich a draft map with semantic tags via parallel LLM calls.
///
/// Calls `on_enriched` with `(completed, total, concept_name)` after each.
pub async fn enrich_map(
    map: &PartonMap,
    provider: &dyn ModelProvider,
    project_root: &Path,
    on_enriched: &dyn Fn(usize, usize, &str),
) -> PartonMap {
    let mut enriched = map.clone();
    let total = enriched.concepts.len();

    let futures: FuturesUnordered<_> = enriched
        .concepts
        .iter()
        .enumerate()
        .map(|(i, concept)| enrich_concept(i, concept, provider, project_root))
        .collect();

    let mut done = 0usize;
    let mut results: Vec<(usize, Vec<Tag>)> = Vec::new();

    futures
        .for_each(|r| {
            done += 1;
            if let Ok((idx, tags)) = r {
                let name = enriched
                    .concepts
                    .get(idx)
                    .map(|c| c.name.as_str())
                    .unwrap_or("?");
                on_enriched(done, total, name);
                results.push((idx, tags));
            } else {
                on_enriched(done, total, "?");
            }
            async {}
        })
        .await;

    for (idx, tags) in results {
        if idx < enriched.concepts.len() {
            enriched.concepts[idx].tags = tags;
        }
    }

    enriched
}

/// Enrich a single concept — one LLM call.
async fn enrich_concept(
    idx: usize,
    concept: &Concept,
    provider: &dyn ModelProvider,
    project_root: &Path,
) -> Result<(usize, Vec<Tag>), ProviderError> {
    let prompt = build_concept_prompt(concept, project_root);
    let response = provider.send(ENRICH_SYSTEM, &prompt, false).await?;
    let tags = parse_tags(&response.content);
    Ok((idx, tags))
}

/// Build the prompt for enriching a single concept.
fn build_concept_prompt(concept: &Concept, project_root: &Path) -> String {
    let mut parts = vec![format!("Concept: {}", concept.name)];
    parts.push(format!("Files: {}", concept.paths.join(", ")));

    if !concept.deps.is_empty() {
        parts.push(format!("Depends on: {}", concept.deps.join(", ")));
    }

    // Include symbol names.
    if !concept.symbols.is_empty() {
        let syms: Vec<String> = concept
            .symbols
            .iter()
            .take(15)
            .map(|s| format!("{}{}", s.name, s.kind.suffix()))
            .collect();
        parts.push(format!("Key symbols: {}", syms.join(", ")));
    }

    // Read first N lines of up to 3 key files for context.
    let real_paths = expand_concept_paths(concept, project_root);
    for path in real_paths.iter().take(3) {
        let full = project_root.join(path);
        if let Ok(content) = std::fs::read_to_string(&full) {
            let snippet: String = content
                .lines()
                .take(MAX_CONTEXT_LINES)
                .collect::<Vec<_>>()
                .join("\n");
            if !snippet.trim().is_empty() {
                parts.push(format!("--- {path} ---\n{snippet}"));
            }
        }
    }

    parts.push("Return ONLY #key:value lines (1-3 tags):".into());
    parts.join("\n\n")
}

/// Expand concept paths (which may contain globs) to real file paths.
fn expand_concept_paths(concept: &Concept, project_root: &Path) -> Vec<String> {
    let mut result = Vec::new();
    for path_pattern in &concept.paths {
        if path_pattern.contains('*') || path_pattern.contains('[') {
            // Glob — try to find first matching file.
            let base = path_pattern
                .split('*')
                .next()
                .unwrap_or(path_pattern)
                .split('[')
                .next()
                .unwrap_or(path_pattern);
            let dir = project_root.join(base);
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten().take(3) {
                    if entry.path().is_file() {
                        if let Ok(rel) = entry.path().strip_prefix(project_root) {
                            result.push(rel.to_string_lossy().to_string());
                        }
                    }
                }
            }
        } else {
            result.push(path_pattern.clone());
        }
    }
    result
}

/// Parse #key:value tags from LLM response.
fn parse_tags(content: &str) -> Vec<Tag> {
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_start_matches("- ");
            let after_hash = trimmed.strip_prefix('#')?;
            let colon = after_hash.find(':')?;
            let key = after_hash[..colon].trim();
            let value = after_hash[colon + 1..].trim();
            if key.is_empty() || value.is_empty() || value.len() > 120 {
                return None;
            }
            Some(Tag {
                key: key.to_string(),
                value: value.to_string(),
            })
        })
        .take(3)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_tags() {
        let input = "#flow:a→b→c\n#used-by:admin routes\n#note:important";
        let tags = parse_tags(input);
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0].key, "flow");
        assert_eq!(tags[0].value, "a→b→c");
    }

    #[test]
    fn parse_ignores_prose() {
        let input = "Here are the tags:\n#flow:a→b\nSome explanation\n#note:ok";
        let tags = parse_tags(input);
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn parse_ignores_empty_value() {
        let input = "#flow:\n#note:ok";
        let tags = parse_tags(input);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].key, "note");
    }

    #[test]
    fn parse_strips_markdown_bullets() {
        let input = "- #flow:a→b\n- #note:ok";
        let tags = parse_tags(input);
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn parse_caps_at_3() {
        let input = "#a:1\n#b:2\n#c:3\n#d:4\n#e:5";
        let tags = parse_tags(input);
        assert_eq!(tags.len(), 3);
    }
}
