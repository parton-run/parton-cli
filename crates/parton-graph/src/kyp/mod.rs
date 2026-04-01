//! KYP (Know Your Project) — AI-first project index.
//!
//! Generates a `.parton/map` file with ultra-compact project representation.
//! The map encodes domain concepts, key symbols, dependencies, patterns,
//! and conventions in ~100-200 tokens.

pub mod cluster;
pub mod compress;
pub mod enrich;
pub mod format;
pub mod lock;
pub mod pattern;
pub mod symbols;

use std::path::Path;

use format::{Concept, PartonMap};

use crate::types::CodeGraph;

/// Build a PartonMap from a code graph.
///
/// Pipeline: cluster → symbols → compress → pattern → assemble.
pub fn build_map(graph: &CodeGraph, git_sha: &str) -> PartonMap {
    let clusters = cluster::cluster_files(graph);

    let concepts: Vec<Concept> = clusters
        .iter()
        .map(|c| {
            let syms = symbols::extract_cluster_symbols(&c.files, graph);
            let paths = compress::compress_paths(&c.files);
            let deps = find_cluster_deps(&c.files, &clusters, graph);
            let pat = pattern::detect_pattern(&c.files, graph);

            Concept {
                name: c.name.clone(),
                paths,
                tags: vec![], // Filled by LLM enrichment.
                symbols: syms,
                deps,
                pattern: pat,
            }
        })
        .collect();

    PartonMap {
        version: 1,
        git_sha: git_sha.to_string(),
        file_count: graph.file_count(),
        concepts,
        conventions: detect_conventions(graph),
    }
}

/// Load a PartonMap from `.parton/map` file.
pub fn load_map(project_root: &Path) -> Option<PartonMap> {
    let path = project_root.join(".parton/map");
    let content = std::fs::read_to_string(&path).ok()?;
    parse_map(&content)
}

/// Save a PartonMap and lock file to `.parton/`.
pub fn save_map(
    map: &PartonMap,
    graph: &CodeGraph,
    project_root: &Path,
) -> Result<(), std::io::Error> {
    let dir = project_root.join(".parton");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("map"), map.to_string())?;

    // Build and save lock from current file state.
    let file_paths: Vec<String> = graph.files.keys().cloned().collect();
    let map_lock = lock::build_lock(&file_paths, project_root, &map.git_sha);
    lock::save_lock(&map_lock, project_root)
}

/// Convert a PartonMap to a full context string (all symbols).
pub fn map_to_context(map: &PartonMap) -> String {
    format!(
        "## Project Map ({} files)\n{}",
        map.file_count,
        map.to_string().trim()
    )
}

/// Convert a PartonMap to a light context string (headers + tags only, no symbols).
///
/// ~2KB instead of ~33KB. Planner gets concept overview, can use
/// `read_file` tool for details. Switch to `map_to_context` if
/// planner needs full symbol list.
pub fn map_to_context_light(map: &PartonMap) -> String {
    use std::fmt::Write;
    let mut out = format!("## Project Map ({} files)\n", map.file_count);
    let _ = writeln!(
        out,
        "#v{} sha={} files={}",
        map.version, map.git_sha, map.file_count
    );
    for concept in &map.concepts {
        let _ = write!(out, "{}:{}", concept.name, concept.paths.join(","));
        if !concept.deps.is_empty() {
            let _ = write!(out, "←{}", concept.deps.join(","));
        }
        if let Some(ref pat) = concept.pattern {
            let _ = write!(out, "|{pat}");
        }
        let _ = writeln!(out);
        for tag in &concept.tags {
            let _ = writeln!(out, "  #{}:{}", tag.key, tag.value);
        }
    }
    if !map.conventions.is_empty() {
        let flags: Vec<String> = map.conventions.iter().map(|c| format!("+{c}")).collect();
        let _ = writeln!(out, "{}", flags.join(" "));
    }
    out
}

/// Check if an incremental update is possible and return changed files.
///
/// Returns `None` if no lock exists (full rebuild needed).
/// Returns `Some(diff)` with the list of changed/removed files.
pub fn check_for_updates(project_root: &Path, current_files: &[String]) -> Option<lock::LockDiff> {
    let existing = lock::load_lock(project_root)?;
    Some(lock::diff_lock(&existing, current_files, project_root))
}

/// Incrementally update a map with only changed files.
///
/// Re-scans only `changed_files` with tree-sitter, updates affected
/// concepts, and saves the new map + lock.
pub async fn update_map(
    _existing_map: &PartonMap,
    changed_files: &[String],
    removed_files: &[String],
    project_root: &Path,
) -> Result<PartonMap, crate::error::GraphError> {
    // Re-scan only changed files.
    let mut store = crate::grammar::GrammarStore::with_default_cache()?;
    let new_nodes = crate::scan::scan_files(changed_files, &mut store, project_root).await;

    // Build updated graph: start from scratch for simplicity,
    // but only the changed concepts need rebuilding.
    let full_files = crate::scan::walker::collect_source_files(project_root);
    let mut graph = CodeGraph::new();

    // Add unchanged files from a quick rescan (we need the full graph for deps).
    let all_nodes = crate::scan::scan_files(&full_files, &mut store, project_root).await;
    for node in all_nodes {
        graph.add_file(node);
    }

    // Overwrite with freshly scanned changed files.
    for node in new_nodes {
        graph.add_file(node);
    }

    // Remove deleted files.
    for path in removed_files {
        graph.files.remove(path);
    }

    let sha = get_git_sha(project_root);
    let map = build_map(&graph, &sha);
    save_map(&map, &graph, project_root)?;

    Ok(map)
}

/// Get the git HEAD sha (short form).
pub fn get_git_sha(project_root: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(project_root)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".into())
}

/// Check if a loaded map is still valid (sha matches HEAD).
pub fn map_is_current(map: &PartonMap, project_root: &Path) -> bool {
    let head = get_git_sha(project_root);
    map.git_sha == head
}

/// Find which other clusters a cluster depends on.
fn find_cluster_deps(
    file_paths: &[String],
    all_clusters: &[cluster::RawCluster],
    graph: &CodeGraph,
) -> Vec<String> {
    let mut dep_names: Vec<String> = Vec::new();

    for path in file_paths {
        let node = match graph.get_file(path) {
            Some(n) => n,
            None => continue,
        };
        for edge in &node.imports {
            // Find which cluster the import target belongs to.
            for cluster in all_clusters {
                if cluster.files.contains(&edge.from_path)
                    && !dep_names.contains(&cluster.name)
                    && !file_paths.contains(&edge.from_path)
                {
                    dep_names.push(cluster.name.clone());
                }
            }
        }
    }

    dep_names.sort();
    dep_names
}

/// Detect project conventions from code patterns.
fn detect_conventions(graph: &CodeGraph) -> Vec<String> {
    let mut conventions = Vec::new();

    let has_ts = graph
        .files
        .keys()
        .any(|p| p.ends_with(".ts") || p.ends_with(".tsx"));
    let has_jsx = graph
        .files
        .keys()
        .any(|p| p.ends_with(".tsx") || p.ends_with(".jsx"));
    let has_rs = graph.files.keys().any(|p| p.ends_with(".rs"));

    if has_ts {
        conventions.push("typescript".into());
    }
    if has_jsx {
        conventions.push("react".into());
    }
    if has_rs {
        conventions.push("rust".into());
    }

    // Check for named vs default exports.
    let has_default = graph
        .files
        .values()
        .any(|n| n.symbols.iter().any(|s| s.name == "default" && s.exported));
    if !has_default && !graph.files.is_empty() {
        conventions.push("named-exports".into());
    }

    conventions
}

/// Parse a `.parton/map` string into a PartonMap.
///
/// Simple line-based parser — not a full grammar implementation,
/// but enough for our own writer's output.
fn parse_map(content: &str) -> Option<PartonMap> {
    let mut map = PartonMap::default();
    for raw_line in content.lines() {
        // Indented lines are tags/symbols — belong to the last concept.
        if raw_line.starts_with(' ') || raw_line.starts_with('\t') {
            // Skip for now — tags/symbols are not loaded back into PartonMap.
            continue;
        }
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#v") {
            parse_meta_line(line, &mut map);
        } else if line.starts_with('+') {
            map.conventions = line
                .split_whitespace()
                .map(|w| w.trim_start_matches('+').to_string())
                .collect();
        } else if let Some(colon) = line.find(':') {
            let name = &line[..colon];
            if let Some(concept) = parse_concept_line(name, &line[colon + 1..]) {
                map.concepts.push(concept);
            }
        }
    }
    if map.version == 0 {
        return None;
    }
    Some(map)
}

/// Parse the `#v1 sha=abc files=424` meta line.
fn parse_meta_line(line: &str, map: &mut PartonMap) {
    for part in line.split_whitespace() {
        if let Some(ver) = part.strip_prefix("#v") {
            map.version = ver.parse().unwrap_or(1);
        } else if let Some(sha) = part.strip_prefix("sha=") {
            map.git_sha = sha.to_string();
        } else if let Some(n) = part.strip_prefix("files=") {
            map.file_count = n.parse().unwrap_or(0);
        }
    }
}

/// Parse a concept line after the name.
fn parse_concept_line(name: &str, rest: &str) -> Option<Concept> {
    // Split by operators: → ← |
    let (paths_str, after_arrow) = split_at_char(rest, '→');
    let (exports_str, after_exports) = match after_arrow {
        Some(a) => split_at_char(a, '←'),
        None => (None, split_at_char(rest, '←').1),
    };
    let (deps_str, pattern_str) = match after_exports {
        Some(a) => split_at_char(a, '|'),
        None => {
            let (_, p) = split_at_char(paths_str.unwrap_or(rest), '|');
            (None, p)
        }
    };

    let paths: Vec<String> = paths_str
        .unwrap_or(
            rest.split('→')
                .next()
                .unwrap_or(rest)
                .split('←')
                .next()
                .unwrap_or(rest)
                .split('|')
                .next()
                .unwrap_or(rest),
        )
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Some(Concept {
        name: name.to_string(),
        paths,
        tags: vec![],
        symbols: exports_str.map(|_| vec![]).unwrap_or_default(),
        deps: deps_str
            .map(|d| {
                d.split('|')
                    .next()
                    .unwrap_or(d)
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
        pattern: pattern_str.map(|s| s.to_string()),
    })
}

/// Split string at first occurrence of a char.
fn split_at_char(s: &str, c: char) -> (Option<&str>, Option<&str>) {
    match s.find(c) {
        Some(i) => (Some(&s[..i]), Some(&s[i + c.len_utf8()..])),
        None => (Some(s), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn test_graph() -> CodeGraph {
        let mut g = CodeGraph::new();
        g.add_file(FileNode {
            path: "lib/auth/admin.ts".into(),
            language: Language::TypeScript,
            symbols: vec![Symbol {
                name: "checkAdmin".into(),
                kind: SymbolKind::Function,
                signature: "export function checkAdmin()".into(),
                line_start: 1,
                line_end: 5,
                exported: true,
            }],
            imports: vec![ImportEdge {
                from_path: "lib/db/schema.ts".into(),
                symbols: vec!["users".into()],
            }],
        });
        g.add_file(FileNode {
            path: "lib/auth/guard.ts".into(),
            language: Language::TypeScript,
            symbols: vec![Symbol {
                name: "withGuard".into(),
                kind: SymbolKind::Function,
                signature: "export function withGuard()".into(),
                line_start: 1,
                line_end: 5,
                exported: true,
            }],
            imports: vec![],
        });
        g.add_file(FileNode {
            path: "lib/db/schema.ts".into(),
            language: Language::TypeScript,
            symbols: vec![Symbol {
                name: "users".into(),
                kind: SymbolKind::Variable,
                signature: "export const users".into(),
                line_start: 1,
                line_end: 5,
                exported: true,
            }],
            imports: vec![],
        });
        g.add_file(FileNode {
            path: "lib/db/index.ts".into(),
            language: Language::TypeScript,
            symbols: vec![Symbol {
                name: "db".into(),
                kind: SymbolKind::Variable,
                signature: "export const db".into(),
                line_start: 1,
                line_end: 3,
                exported: true,
            }],
            imports: vec![],
        });
        g
    }

    #[test]
    fn build_map_produces_concepts() {
        let g = test_graph();
        let map = build_map(&g, "abc123");
        assert!(!map.concepts.is_empty());
        assert_eq!(map.git_sha, "abc123");
        assert_eq!(map.file_count, 4);
    }

    #[test]
    fn map_roundtrip_display_parse() {
        let g = test_graph();
        let map = build_map(&g, "abc123");
        let text = map.to_string();
        let parsed = parse_map(&text);
        assert!(parsed.is_some());
        let p = parsed.unwrap();
        assert_eq!(p.version, 1);
        assert_eq!(p.git_sha, "abc123");
        assert_eq!(p.file_count, 4);
        assert!(!p.concepts.is_empty());
    }

    #[test]
    fn save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        // Create files on disk so lock can hash them.
        std::fs::create_dir_all(dir.path().join("lib/auth")).unwrap();
        std::fs::create_dir_all(dir.path().join("lib/db")).unwrap();
        std::fs::write(
            dir.path().join("lib/auth/admin.ts"),
            "export function checkAdmin(){}",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("lib/auth/guard.ts"),
            "export function withGuard(){}",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("lib/db/schema.ts"),
            "export const users = pgTable()",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("lib/db/index.ts"),
            "export const db = drizzle()",
        )
        .unwrap();

        let g = test_graph();
        let map = build_map(&g, "test123");
        save_map(&map, &g, dir.path()).unwrap();

        assert!(dir.path().join(".parton/map").exists());
        assert!(dir.path().join(".parton/map.lock").exists());

        let loaded = load_map(dir.path());
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().git_sha, "test123");
    }

    #[test]
    fn detect_conventions_typescript() {
        let g = test_graph();
        let convs = detect_conventions(&g);
        assert!(convs.contains(&"typescript".into()));
        assert!(convs.contains(&"named-exports".into()));
    }
}
