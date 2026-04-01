//! Tool definitions and handler for LLM tool-use.
//!
//! Provides tools for the planner/clarifier to drill into specific
//! files. The KYP map (.parton/map) is the primary context source;
//! tools are for reading individual files when more detail is needed.

mod handlers;

use std::path::Path;

use parton_core::{ToolCall, ToolDefinition, ToolResult};
use serde_json::json;

use crate::types::CodeGraph;

/// Create tool definitions for the planner/clarifier.
///
/// Only file-level tools — the KYP map provides the overview.
pub fn create_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "read_file".into(),
            description: "Read an existing file from disk (max 200 lines).".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Relative file path"}
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "list_files".into(),
            description: "List project files matching a pattern (e.g. 'lib/auth').".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Substring to match file paths"}
                },
                "required": ["pattern"]
            }),
        },
    ]
}

/// Handle a tool call using the code graph and project root.
pub fn handle_tool_call(call: &ToolCall, graph: &CodeGraph, root: &Path) -> ToolResult {
    let content = match call.name.as_str() {
        "read_file" => handlers::read_file(call, root),
        "list_files" => handlers::list_files(call, graph),
        _ => format!("unknown tool: {}", call.name),
    };
    ToolResult {
        call_id: call.id.clone(),
        content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn test_graph() -> CodeGraph {
        let mut g = CodeGraph::new();
        g.add_file(FileNode {
            path: "lib/auth.ts".into(),
            language: Language::TypeScript,
            symbols: vec![],
            imports: vec![],
        });
        g
    }

    fn make_call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "test".into(),
            name: name.into(),
            arguments: args,
        }
    }

    #[test]
    fn tool_definitions_count() {
        assert_eq!(create_tool_definitions().len(), 2);
    }

    #[test]
    fn list_files_matches() {
        let g = test_graph();
        let call = make_call("list_files", json!({"pattern": "auth"}));
        let result = handle_tool_call(&call, &g, Path::new("/tmp"));
        assert!(result.content.contains("lib/auth.ts"));
    }

    #[test]
    fn unknown_tool_handled() {
        let g = test_graph();
        let call = make_call("nope", json!({}));
        let result = handle_tool_call(&call, &g, Path::new("/tmp"));
        assert!(result.content.contains("unknown tool"));
    }

    #[test]
    fn read_file_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello\nworld").unwrap();
        let g = CodeGraph::new();
        let call = make_call("read_file", json!({"path": "test.txt"}));
        let result = handle_tool_call(&call, &g, dir.path());
        assert!(result.content.contains("hello"));
    }

    #[test]
    fn read_file_missing() {
        let g = CodeGraph::new();
        let call = make_call("read_file", json!({"path": "nope.txt"}));
        let result = handle_tool_call(&call, &g, Path::new("/tmp"));
        assert!(result.content.contains("error reading"));
    }
}
