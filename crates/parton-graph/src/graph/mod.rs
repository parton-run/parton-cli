//! In-memory code graph construction and querying.

pub mod edges;

use crate::types::{CodeGraph, FileNode, Symbol};

impl CodeGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file node to the graph.
    pub fn add_file(&mut self, node: FileNode) {
        self.files.insert(node.path.clone(), node);
    }

    /// Get a file node by path.
    pub fn get_file(&self, path: &str) -> Option<&FileNode> {
        self.files.get(path)
    }

    /// Get all exported symbols from a file.
    pub fn exports_of(&self, path: &str) -> Vec<&Symbol> {
        self.files
            .get(path)
            .map(|f| f.symbols.iter().filter(|s| s.exported).collect())
            .unwrap_or_default()
    }

    /// Find files that import from the given path.
    ///
    /// Returns `(importer_path, imported_symbol_names)` pairs.
    pub fn dependents_of(&self, path: &str) -> Vec<(&str, Vec<&str>)> {
        self.files
            .values()
            .filter_map(|node| {
                let edge = node.imports.iter().find(|e| e.from_path == path)?;
                let symbols: Vec<&str> = edge.symbols.iter().map(|s| s.as_str()).collect();
                Some((node.path.as_str(), symbols))
            })
            .collect()
    }

    /// Get resolved imports for a file.
    ///
    /// For each import edge, looks up the target file and returns
    /// its exported symbols that match the imported names.
    pub fn resolved_imports_of(&self, path: &str) -> Vec<(&str, Vec<&Symbol>)> {
        let node = match self.files.get(path) {
            Some(n) => n,
            None => return vec![],
        };

        node.imports
            .iter()
            .filter_map(|edge| {
                let target = self.files.get(&edge.from_path)?;
                let matched: Vec<&Symbol> = target
                    .symbols
                    .iter()
                    .filter(|s| s.exported && edge.symbols.contains(&s.name))
                    .collect();
                if matched.is_empty() {
                    None
                } else {
                    Some((target.path.as_str(), matched))
                }
            })
            .collect()
    }

    /// Number of files in the graph.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ImportEdge, Language, SymbolKind};

    fn make_symbol(name: &str, exported: bool) -> Symbol {
        Symbol {
            name: name.into(),
            kind: SymbolKind::Function,
            signature: format!("export function {name}()"),
            line_start: 1,
            line_end: 5,
            exported,
        }
    }

    fn make_file(path: &str, symbols: Vec<Symbol>, imports: Vec<ImportEdge>) -> FileNode {
        FileNode {
            path: path.into(),
            language: Language::TypeScript,
            symbols,
            imports,
        }
    }

    #[test]
    fn add_and_get_file() {
        let mut graph = CodeGraph::new();
        graph.add_file(make_file("src/app.ts", vec![], vec![]));
        assert!(graph.get_file("src/app.ts").is_some());
        assert!(graph.get_file("missing.ts").is_none());
    }

    #[test]
    fn exports_of_file() {
        let mut graph = CodeGraph::new();
        graph.add_file(make_file(
            "src/types.ts",
            vec![make_symbol("User", true), make_symbol("internal", false)],
            vec![],
        ));
        let exports = graph.exports_of("src/types.ts");
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "User");
    }

    #[test]
    fn exports_of_missing_file() {
        let graph = CodeGraph::new();
        assert!(graph.exports_of("nope.ts").is_empty());
    }

    #[test]
    fn dependents_of_file() {
        let mut graph = CodeGraph::new();
        graph.add_file(make_file(
            "src/types.ts",
            vec![make_symbol("User", true)],
            vec![],
        ));
        graph.add_file(make_file(
            "src/app.ts",
            vec![],
            vec![ImportEdge {
                from_path: "src/types.ts".into(),
                symbols: vec!["User".into()],
            }],
        ));
        let deps = graph.dependents_of("src/types.ts");
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].0, "src/app.ts");
        assert_eq!(deps[0].1, vec!["User"]);
    }

    #[test]
    fn resolved_imports() {
        let mut graph = CodeGraph::new();
        graph.add_file(make_file(
            "src/types.ts",
            vec![make_symbol("User", true), make_symbol("Admin", true)],
            vec![],
        ));
        graph.add_file(make_file(
            "src/app.ts",
            vec![],
            vec![ImportEdge {
                from_path: "src/types.ts".into(),
                symbols: vec!["User".into()],
            }],
        ));
        let imports = graph.resolved_imports_of("src/app.ts");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].1.len(), 1);
        assert_eq!(imports[0].1[0].name, "User");
    }

    #[test]
    fn resolved_imports_missing_file() {
        let graph = CodeGraph::new();
        assert!(graph.resolved_imports_of("nope.ts").is_empty());
    }

    #[test]
    fn file_count() {
        let mut graph = CodeGraph::new();
        assert_eq!(graph.file_count(), 0);
        graph.add_file(make_file("a.ts", vec![], vec![]));
        graph.add_file(make_file("b.ts", vec![], vec![]));
        assert_eq!(graph.file_count(), 2);
    }
}
