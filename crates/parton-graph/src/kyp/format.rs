//! `.parton/map` file format — parser and writer.
//!
//! Ultra-compact, AI-first project map format.
//! Concept lines + indented symbol lines with signatures.

use std::fmt;

/// The complete project map.
#[derive(Debug, Clone, Default)]
pub struct PartonMap {
    /// Format version (always 1 for now).
    pub version: u32,
    /// Git SHA at time of generation.
    pub git_sha: String,
    /// Total project file count.
    pub file_count: usize,
    /// Domain concepts.
    pub concepts: Vec<Concept>,
    /// Project-wide conventions.
    pub conventions: Vec<String>,
}

/// A domain concept — a cluster of related files.
#[derive(Debug, Clone)]
pub struct Concept {
    /// Short name (e.g. `auth`, `db`, `api`).
    pub name: String,
    /// File paths (may contain globs).
    pub paths: Vec<String>,
    /// Semantic tags from LLM enrichment (`#key:value`).
    pub tags: Vec<Tag>,
    /// All exported symbols with signatures.
    pub symbols: Vec<MapSymbol>,
    /// Dependencies (names of other concepts).
    pub deps: Vec<String>,
    /// Compact pattern description.
    pub pattern: Option<String>,
}

/// A semantic tag on a concept — `#key:value`.
#[derive(Debug, Clone)]
pub struct Tag {
    /// Tag key (e.g. `flow`, `used-by`, `tables`).
    pub key: String,
    /// Tag value (compact, no prose).
    pub value: String,
}

/// A single exported symbol with its compact signature.
#[derive(Debug, Clone)]
pub struct MapSymbol {
    /// Symbol name.
    pub name: String,
    /// Symbol type suffix.
    pub kind: SymKind,
    /// Compact signature (e.g. `(email:string):boolean`).
    pub signature: String,
}

/// Symbol type suffixes for the map format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymKind {
    /// `:f` — function.
    Function,
    /// `:t` — type/interface.
    Type,
    /// `:c` — component.
    Component,
    /// `:h` — hook.
    Hook,
    /// `:v` — variable/const.
    Variable,
    /// `:e` — enum.
    Enum,
    /// `:s` — schema/table.
    Schema,
    /// `:m` — module.
    Module,
}

impl SymKind {
    /// Suffix string for the map format.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Function => ":f",
            Self::Type => ":t",
            Self::Component => ":c",
            Self::Hook => ":h",
            Self::Variable => ":v",
            Self::Enum => ":e",
            Self::Schema => ":s",
            Self::Module => ":m",
        }
    }

    /// Parse a suffix string.
    pub fn from_suffix(s: &str) -> Option<Self> {
        match s {
            ":f" | "f" => Some(Self::Function),
            ":t" | "t" => Some(Self::Type),
            ":c" | "c" => Some(Self::Component),
            ":h" | "h" => Some(Self::Hook),
            ":v" | "v" => Some(Self::Variable),
            ":e" | "e" => Some(Self::Enum),
            ":s" | "s" => Some(Self::Schema),
            ":m" | "m" => Some(Self::Module),
            _ => None,
        }
    }
}

impl fmt::Display for PartonMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "#v{} sha={} files={}",
            self.version, self.git_sha, self.file_count
        )?;
        for concept in &self.concepts {
            // Concept header line.
            write!(f, "{}:{}", concept.name, concept.paths.join(","))?;
            if !concept.deps.is_empty() {
                write!(f, "←{}", concept.deps.join(","))?;
            }
            if let Some(ref pat) = concept.pattern {
                write!(f, "|{pat}")?;
            }
            writeln!(f)?;
            // Tag lines (LLM-enriched semantic annotations).
            for tag in &concept.tags {
                writeln!(f, "  #{}:{}", tag.key, tag.value)?;
            }
            // Indented symbol lines.
            for sym in &concept.symbols {
                if sym.signature.is_empty() {
                    writeln!(f, "  {}{}", sym.name, sym.kind.suffix())?;
                } else {
                    writeln!(f, "  {}{}{}", sym.name, sym.kind.suffix(), sym.signature)?;
                }
            }
        }
        if !self.conventions.is_empty() {
            let flags: Vec<String> = self.conventions.iter().map(|c| format!("+{c}")).collect();
            writeln!(f, "{}", flags.join(" "))?;
        }
        Ok(())
    }
}

/// Valid tag keys for `#key:value` lines.
pub const VALID_TAG_KEYS: &[&str] = &[
    "flow",
    "used-by",
    "tables",
    "deps",
    "status-flow",
    "note",
    "pattern",
    "auth",
    "stores",
    "renders",
    "calls",
    "returns",
    "triggers",
];

/// Validate a complete map string. Returns errors per line.
pub fn validate_map(content: &str) -> Vec<String> {
    let mut errors = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let num = i + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(err) = validate_map_line(trimmed) {
            errors.push(format!("line {num}: {err}"));
        }
    }
    errors
}

/// Validate a single map line. Returns None if valid, Some(error) if invalid.
fn validate_map_line(line: &str) -> Option<String> {
    // Meta line.
    if line.starts_with("#v") {
        return None;
    }
    // Convention line.
    if line.starts_with('+') {
        return None;
    }
    // Tag line (indented #key:value).
    if line.starts_with("#") {
        let after = line.trim_start_matches('#');
        if !after.contains(':') {
            return Some("tag must be #key:value".into());
        }
        let key = after.split(':').next().unwrap_or("");
        if key.is_empty() {
            return Some("tag key is empty".into());
        }
        // Allow any key — VALID_TAG_KEYS is a suggestion, not a constraint.
        let value = &after[key.len() + 1..];
        if value.trim().is_empty() {
            return Some(format!("tag #{key} has empty value"));
        }
        // Reject prose (value > 80 chars is suspicious).
        if value.len() > 120 {
            return Some(format!("tag #{key} value too long (max 120 chars)"));
        }
        return None;
    }
    // Symbol line (indented, starts with name:type).
    if line.starts_with(' ') || line.starts_with('\t') {
        let inner = line.trim();
        if inner.starts_with('#') {
            return validate_map_line(inner);
        }
        // Should contain :type suffix.
        if !inner.contains(':') {
            return Some("symbol line must have :type suffix (e.g. name:f)".into());
        }
        return None;
    }
    // Concept header line (name:paths...).
    if line.contains(':') {
        let name = line.split(':').next().unwrap_or("");
        if name.is_empty() || name.contains(' ') {
            return Some("concept name must be single word".into());
        }
        return None;
    }
    Some("unrecognized line format".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_with_symbols() {
        let map = PartonMap {
            version: 1,
            git_sha: "abc".into(),
            file_count: 10,
            concepts: vec![Concept {
                name: "auth".into(),
                paths: vec!["lib/auth/*.ts".into()],
                symbols: vec![
                    MapSymbol {
                        name: "checkAdmin".into(),
                        kind: SymKind::Function,
                        signature: "(email:string):boolean".into(),
                    },
                    MapSymbol {
                        name: "AdminUser".into(),
                        kind: SymKind::Type,
                        signature: "{email,workosId}".into(),
                    },
                ],
                deps: vec!["db".into()],
                pattern: Some("guard".into()),
            }],
            conventions: vec!["named-exports".into()],
        };
        let out = map.to_string();
        assert!(out.contains("auth:lib/auth/*.ts←db|guard"));
        assert!(out.contains("  checkAdmin:f(email:string):boolean"));
        assert!(out.contains("  AdminUser:t{email,workosId}"));
        assert!(out.contains("+named-exports"));
    }

    #[test]
    fn display_symbol_no_signature() {
        let map = PartonMap {
            version: 1,
            git_sha: "x".into(),
            file_count: 1,
            concepts: vec![Concept {
                name: "db".into(),
                paths: vec!["lib/db.ts".into()],
                symbols: vec![MapSymbol {
                    name: "db".into(),
                    kind: SymKind::Variable,
                    signature: String::new(),
                }],
                deps: vec![],
                pattern: None,
            }],
            conventions: vec![],
        };
        let out = map.to_string();
        assert!(out.contains("  db:v\n"));
    }

    #[test]
    fn display_with_tags() {
        let map = PartonMap {
            version: 1,
            git_sha: "x".into(),
            file_count: 5,
            concepts: vec![Concept {
                name: "auth".into(),
                paths: vec!["lib/auth.ts".into()],
                tags: vec![Tag {
                    key: "flow".into(),
                    value: "workos.session→adminUsers.lookup→boolean".into(),
                }],
                symbols: vec![MapSymbol {
                    name: "checkAdmin".into(),
                    kind: SymKind::Function,
                    signature: "(email:string):boolean".into(),
                }],
                deps: vec![],
                pattern: None,
            }],
            conventions: vec![],
        };
        let out = map.to_string();
        assert!(out.contains("  #flow:workos.session→adminUsers.lookup→boolean"));
        assert!(out.contains("  checkAdmin:f"));
    }

    #[test]
    fn validate_valid_map() {
        let content = "#v1 sha=abc files=10\nauth:lib/auth.ts←db\n  #flow:session→check\n  checkAdmin:f(email:string)\n+typescript";
        assert!(validate_map(content).is_empty());
    }

    #[test]
    fn validate_rejects_prose() {
        let long = "x".repeat(130);
        let content = format!("  #note:{long}");
        let errors = validate_map(&content);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("too long"));
    }

    #[test]
    fn validate_rejects_empty_tag_value() {
        let errors = validate_map("  #flow:");
        assert!(!errors.is_empty());
    }

    #[test]
    fn sym_kind_roundtrip() {
        for kind in [
            SymKind::Function,
            SymKind::Type,
            SymKind::Component,
            SymKind::Hook,
            SymKind::Variable,
            SymKind::Enum,
            SymKind::Schema,
            SymKind::Module,
        ] {
            assert_eq!(SymKind::from_suffix(kind.suffix()), Some(kind));
        }
    }
}
