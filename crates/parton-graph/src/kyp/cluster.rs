//! Directory-based clustering with import affinity.

use std::collections::HashMap;

use crate::types::CodeGraph;

/// A raw cluster before symbol ranking and compression.
#[derive(Debug)]
pub struct RawCluster {
    /// Cluster name (derived from directory).
    pub name: String,
    /// File paths in this cluster.
    pub files: Vec<String>,
}

/// Cluster project files into domain groups.
///
/// Algorithm:
/// 1. Group by second-level directory (`lib/auth/*` → `auth`)
/// 2. Merge small clusters (< 2 files) into parent
/// 3. Cap cluster names to reasonable length
pub fn cluster_files(graph: &CodeGraph) -> Vec<RawCluster> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for path in graph.files.keys() {
        let name = derive_cluster_name(path);
        groups.entry(name).or_default().push(path.clone());
    }

    // Sort files within each cluster.
    for files in groups.values_mut() {
        files.sort();
    }

    // Merge tiny clusters (1 file) into an "other" cluster.
    let mut clusters: Vec<RawCluster> = Vec::new();
    let mut other_files: Vec<String> = Vec::new();

    for (name, files) in &groups {
        if files.len() < 2 {
            other_files.extend(files.iter().cloned());
        } else {
            clusters.push(RawCluster {
                name: name.clone(),
                files: files.clone(),
            });
        }
    }

    if !other_files.is_empty() {
        other_files.sort();
        clusters.push(RawCluster {
            name: "other".into(),
            files: other_files,
        });
    }

    clusters.sort_by(|a, b| a.name.cmp(&b.name));
    clusters
}

/// Derive a cluster name from a file path.
///
/// Uses the most semantically meaningful directory segment:
/// - `lib/auth/admin.ts` → `auth`
/// - `app/api/admin/roles/route.ts` → `api-admin`
/// - `components/admin/Sidebar.tsx` → `admin-ui`
/// - `crates/parton-core/src/provider.rs` → `core`
/// - `packages/auth/src/index.ts` → `auth`
fn derive_cluster_name(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() <= 1 {
        return "root".into();
    }

    // Monorepo / workspace: strip prefix and recurse on the inner path.
    if matches!(parts[0], "crates" | "packages" | "apps" | "services") && parts.len() > 2 {
        let pkg = strip_common_prefix(parts[1]);
        // If package has deep structure (src/kyp/mod.rs), use sub-module.
        if parts.len() > 4 && parts[2] == "src" {
            return format!("{pkg}-{}", parts[3]);
        }
        return pkg;
    }

    match parts[0] {
        "lib" | "src" => {
            if parts.len() > 2 {
                parts[1].to_string()
            } else {
                parts[1].split('.').next().unwrap_or(parts[1]).to_string()
            }
        }
        "app" => {
            if parts.len() > 2 && parts[1] == "api" {
                if parts.len() > 3 {
                    format!("api-{}", parts[2])
                } else {
                    "api".into()
                }
            } else if parts.len() > 2 {
                format!("pages-{}", parts[1])
            } else {
                "pages".into()
            }
        }
        "components" => {
            if parts.len() > 2 {
                format!("{}-ui", parts[1])
            } else {
                "ui".into()
            }
        }
        "test" | "tests" | "__tests__" => "tests".into(),
        dir => dir.to_string(),
    }
}

/// Strip common monorepo prefixes from package names.
///
/// `parton-core` → `core`, `@scope/auth` → `auth`
fn strip_common_prefix(name: &str) -> String {
    let stripped = name
        .strip_prefix("parton-")
        .or_else(|| name.strip_prefix("@"))
        .unwrap_or(name);
    stripped
        .split('/')
        .next_back()
        .unwrap_or(stripped)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn graph_with_files(paths: &[&str]) -> CodeGraph {
        let mut g = CodeGraph::new();
        for p in paths {
            g.add_file(FileNode {
                path: p.to_string(),
                language: Language::TypeScript,
                symbols: vec![],
                imports: vec![],
            });
        }
        g
    }

    #[test]
    fn clusters_by_directory() {
        let g = graph_with_files(&[
            "lib/auth/admin.ts",
            "lib/auth/guard.ts",
            "lib/db/schema.ts",
            "lib/db/index.ts",
        ]);
        let clusters = cluster_files(&g);
        assert!(clusters.iter().any(|c| c.name == "auth"));
        assert!(clusters.iter().any(|c| c.name == "db"));
    }

    #[test]
    fn merges_tiny_clusters() {
        let g = graph_with_files(&["lib/auth/admin.ts", "lib/auth/guard.ts", "lib/config.ts"]);
        let clusters = cluster_files(&g);
        let other = clusters.iter().find(|c| c.name == "other");
        assert!(other.is_some());
        assert!(other.unwrap().files.contains(&"lib/config.ts".to_string()));
    }

    #[test]
    fn derive_name_lib() {
        assert_eq!(derive_cluster_name("lib/auth/admin.ts"), "auth");
        assert_eq!(derive_cluster_name("lib/db/schema.ts"), "db");
    }

    #[test]
    fn derive_name_app_api() {
        assert_eq!(
            derive_cluster_name("app/api/admin/roles/route.ts"),
            "api-admin"
        );
    }

    #[test]
    fn derive_name_components() {
        assert_eq!(
            derive_cluster_name("components/admin/Sidebar.tsx"),
            "admin-ui"
        );
    }

    #[test]
    fn derive_name_root_file() {
        assert_eq!(derive_cluster_name("config.ts"), "root");
    }

    #[test]
    fn derive_name_rust_workspace() {
        assert_eq!(
            derive_cluster_name("crates/parton-core/src/provider.rs"),
            "core"
        );
        assert_eq!(
            derive_cluster_name("crates/parton-graph/src/kyp/mod.rs"),
            "graph-kyp"
        );
        assert_eq!(derive_cluster_name("crates/parton-cli/src/main.rs"), "cli");
    }

    #[test]
    fn derive_name_monorepo_packages() {
        assert_eq!(derive_cluster_name("packages/auth/src/index.ts"), "auth");
        assert_eq!(derive_cluster_name("apps/web/src/app.tsx"), "web");
    }

    #[test]
    fn strip_prefix_removes_parton() {
        assert_eq!(strip_common_prefix("parton-core"), "core");
        assert_eq!(strip_common_prefix("parton-graph"), "graph");
        assert_eq!(strip_common_prefix("my-lib"), "my-lib");
    }
}
