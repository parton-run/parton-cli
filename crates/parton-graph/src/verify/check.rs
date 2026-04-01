//! Export/import comparison logic for contract verification.

use crate::types::{ImportEdge, Symbol};

/// Check that all required exports are present in actual symbols.
///
/// Returns names of exports that are missing from the actual symbols.
pub fn find_missing_exports(required: &[String], actual: &[Symbol]) -> Vec<String> {
    required
        .iter()
        .filter(|name| !actual.iter().any(|s| s.exported && s.name == **name))
        .cloned()
        .collect()
}

/// Check that all required imports are present in actual imports.
///
/// Returns `(from_path, symbol)` pairs for missing imports.
pub fn find_missing_imports(
    required: &[parton_core::ImportRef],
    actual: &[ImportEdge],
) -> Vec<(String, String)> {
    let mut missing = Vec::new();

    for req in required {
        let edge = actual.iter().find(|e| paths_match(&e.from_path, &req.path));

        match edge {
            None => {
                // Entire import source is missing.
                for sym in &req.symbols {
                    missing.push((req.path.clone(), sym.clone()));
                }
            }
            Some(e) => {
                // Source found but some symbols may be missing.
                for sym in &req.symbols {
                    if !e.symbols.contains(sym) {
                        missing.push((req.path.clone(), sym.clone()));
                    }
                }
            }
        }
    }

    missing
}

/// Fuzzy path matching for imports.
///
/// Import paths in source code may use relative paths (`./types`),
/// aliases (`@/lib/db`), or omit extensions. This does a suffix match.
fn paths_match(actual: &str, required: &str) -> bool {
    if actual == required {
        return true;
    }
    // Strip leading ./ or @/ prefixes and compare stems.
    let a = actual
        .trim_start_matches("./")
        .trim_start_matches("../")
        .trim_start_matches("@/");
    let r = required
        .trim_start_matches("./")
        .trim_start_matches("../")
        .trim_start_matches("@/");

    // Remove extensions for comparison.
    let a_stem = a
        .trim_end_matches(".ts")
        .trim_end_matches(".tsx")
        .trim_end_matches(".js")
        .trim_end_matches(".jsx");
    let r_stem = r
        .trim_end_matches(".ts")
        .trim_end_matches(".tsx")
        .trim_end_matches(".js")
        .trim_end_matches(".jsx");

    a_stem == r_stem || a_stem.ends_with(r_stem) || r_stem.ends_with(a_stem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SymbolKind;

    fn sym(name: &str, exported: bool) -> Symbol {
        Symbol {
            name: name.into(),
            kind: SymbolKind::Function,
            signature: String::new(),
            line_start: 0,
            line_end: 0,
            exported,
        }
    }

    #[test]
    fn all_exports_present() {
        let required = vec!["foo".into(), "bar".into()];
        let actual = vec![sym("foo", true), sym("bar", true), sym("baz", true)];
        assert!(find_missing_exports(&required, &actual).is_empty());
    }

    #[test]
    fn missing_export_detected() {
        let required = vec!["foo".into(), "bar".into()];
        let actual = vec![sym("foo", true)];
        let missing = find_missing_exports(&required, &actual);
        assert_eq!(missing, vec!["bar"]);
    }

    #[test]
    fn unexported_symbol_counts_as_missing() {
        let required = vec!["foo".into()];
        let actual = vec![sym("foo", false)];
        let missing = find_missing_exports(&required, &actual);
        assert_eq!(missing, vec!["foo"]);
    }

    #[test]
    fn all_imports_present() {
        let required = vec![parton_core::ImportRef {
            path: "lib/db".into(),
            symbols: vec!["db".into()],
        }];
        let actual = vec![ImportEdge {
            from_path: "@/lib/db".into(),
            symbols: vec!["db".into()],
        }];
        assert!(find_missing_imports(&required, &actual).is_empty());
    }

    #[test]
    fn missing_import_source() {
        let required = vec![parton_core::ImportRef {
            path: "lib/auth".into(),
            symbols: vec!["checkAdmin".into()],
        }];
        let actual = vec![];
        let missing = find_missing_imports(&required, &actual);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].1, "checkAdmin");
    }

    #[test]
    fn paths_match_with_alias() {
        assert!(paths_match("@/lib/db", "lib/db"));
        assert!(paths_match("./types", "types"));
        assert!(paths_match("lib/db/schema.ts", "lib/db/schema"));
    }

    #[test]
    fn paths_no_match() {
        assert!(!paths_match("lib/auth", "lib/db"));
    }
}
