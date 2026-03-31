//! Project file scanning.
//!
//! Scans source files using tree-sitter grammars to extract symbols
//! and import relationships, building `FileNode` entries for the graph.

pub mod parser;
pub mod walker;

use std::path::Path;

use crate::detect;
use crate::error::GraphError;
use crate::grammar::GrammarStore;
use crate::queries;
use crate::queries::generic;
use crate::types::{FileNode, Language};

/// Scan a set of files and extract their symbols and imports.
///
/// For each file:
/// 1. Detect language from extension
/// 2. Load grammar (download if needed)
/// 3. Parse with tree-sitter and extract symbols/imports
/// 4. Fall back to regex heuristics if grammar unavailable
pub async fn scan_files(
    paths: &[String],
    store: &mut GrammarStore,
    project_root: &Path,
) -> Vec<FileNode> {
    let mut nodes = Vec::new();

    for rel_path in paths {
        let lang = detect::detect_language(rel_path);
        if matches!(lang, Language::Unknown) {
            continue;
        }

        let full_path = project_root.join(rel_path);
        let source = match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!("skip {rel_path}: {e}");
                continue;
            }
        };

        match scan_single(rel_path, &source, lang, store).await {
            Ok(node) => nodes.push(node),
            Err(e) => {
                tracing::debug!("tree-sitter failed for {rel_path}: {e}, using fallback");
                nodes.push(scan_fallback(rel_path, &source, lang));
            }
        }
    }

    nodes
}

/// Scan a single file with tree-sitter.
async fn scan_single(
    path: &str,
    source: &str,
    lang: Language,
    store: &mut GrammarStore,
) -> Result<FileNode, GraphError> {
    let ts_lang = store.get_or_load(lang).await?;
    let lang_queries = queries::queries_for(lang)
        .ok_or_else(|| GraphError::UnsupportedLanguage(lang.to_string()))?;

    let symbols = parser::extract_exports(source, &ts_lang, lang_queries.exports, lang)?;
    let imports = parser::extract_imports(source, &ts_lang, lang_queries.imports)?;

    Ok(FileNode {
        path: path.to_string(),
        language: lang,
        symbols,
        imports,
    })
}

/// Fallback: scan with regex heuristics when tree-sitter is unavailable.
fn scan_fallback(path: &str, source: &str, lang: Language) -> FileNode {
    FileNode {
        path: path.to_string(),
        language: lang,
        symbols: generic::extract_symbols(source),
        imports: generic::extract_imports(source),
    }
}

/// Collect all scannable file paths from a plan's context.
///
/// Gathers files from: plan file paths, import sources, and context files.
pub fn collect_scan_paths(
    plan_files: &[String],
    import_sources: &[String],
    context_files: &[String],
) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();

    for p in plan_files.iter().chain(import_sources).chain(context_files) {
        if !paths.contains(p) && detect::is_supported(detect::detect_language(p)) {
            paths.push(p.clone());
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_extracts_ts_symbols() {
        let source = r#"
export function fetchUser(id: string): User {
    return db.get(id);
}

export class UserService {
    constructor() {}
}
"#;
        let node = scan_fallback("src/user.ts", source, Language::TypeScript);
        assert_eq!(node.path, "src/user.ts");
        assert!(node.symbols.len() >= 2);
    }

    #[test]
    fn fallback_extracts_imports() {
        let source = "import { User } from './types';\n";
        let node = scan_fallback("src/app.ts", source, Language::TypeScript);
        assert_eq!(node.imports.len(), 1);
        assert_eq!(node.imports[0].from_path, "./types");
    }

    #[test]
    fn collect_scan_paths_deduplicates() {
        let paths = collect_scan_paths(
            &["src/app.ts".into(), "src/types.ts".into()],
            &["src/types.ts".into()],
            &["src/config.ts".into()],
        );
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn collect_scan_paths_skips_unsupported() {
        let paths = collect_scan_paths(&["README.md".into(), "src/app.ts".into()], &[], &[]);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], "src/app.ts");
    }
}
