//! Individual tool handler implementations.

use std::path::Path;

use parton_core::ToolCall;

use crate::types::CodeGraph;

/// Maximum lines returned by `read_file`.
const MAX_READ_LINES: usize = 200;

/// Maximum results from `list_files`.
const MAX_LIST_RESULTS: usize = 100;

/// Read a file from disk.
pub fn read_file(call: &ToolCall, root: &Path) -> String {
    let path = call.arguments["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return "error: missing path argument".into();
    }
    let full = root.join(path);
    match std::fs::read_to_string(&full) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().take(MAX_READ_LINES).collect();
            lines.join("\n")
        }
        Err(e) => format!("error reading {path}: {e}"),
    }
}

/// List files matching a substring pattern.
pub fn list_files(call: &ToolCall, graph: &CodeGraph) -> String {
    let pattern = call.arguments["pattern"]
        .as_str()
        .unwrap_or("")
        .to_lowercase();
    if pattern.is_empty() {
        return "error: missing pattern argument".into();
    }
    let mut matches: Vec<&str> = graph
        .files
        .keys()
        .filter(|p| p.to_lowercase().contains(&pattern))
        .map(|p| p.as_str())
        .collect();
    matches.sort();
    if matches.is_empty() {
        return format!("no files matching '{pattern}'");
    }
    if matches.len() > MAX_LIST_RESULTS {
        let total = matches.len();
        matches.truncate(MAX_LIST_RESULTS);
        return format!(
            "{}\n({total} files total, showing first {MAX_LIST_RESULTS})",
            matches.join("\n")
        );
    }
    matches.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn list_files_no_match() {
        let graph = CodeGraph::new();
        let call = ToolCall {
            id: "t".into(),
            name: "list_files".into(),
            arguments: serde_json::json!({"pattern": "zzz"}),
        };
        assert!(list_files(&call, &graph).contains("no files"));
    }

    #[test]
    fn read_file_empty_path() {
        let call = ToolCall {
            id: "t".into(),
            name: "read_file".into(),
            arguments: serde_json::json!({"path": ""}),
        };
        assert!(read_file(&call, Path::new("/tmp")).contains("error"));
    }
}
