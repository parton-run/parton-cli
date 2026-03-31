//! Tool types for LLM tool-use (function calling).
//!
//! These types are provider-agnostic. Each provider maps them to its
//! own wire format (e.g. OpenAI function calling, Anthropic tool use).

use serde::{Deserialize, Serialize};

/// Definition of a tool the LLM can call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (e.g. `read_file`, `search_symbols`).
    pub name: String,
    /// Human-readable description shown to the LLM.
    pub description: String,
    /// JSON Schema for the tool's parameters.
    pub parameters: serde_json::Value,
}

/// A tool call requested by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique call id (provider-generated).
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// Arguments as a JSON object.
    pub arguments: serde_json::Value,
}

/// Result of executing a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Id of the tool call this result belongs to.
    pub call_id: String,
    /// Tool output as a string (will be sent back to the LLM).
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definition_serde() {
        let def = ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        };
        let json = serde_json::to_string(&def).unwrap();
        let parsed: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "read_file");
    }

    #[test]
    fn tool_call_serde() {
        let call = ToolCall {
            id: "call_1".into(),
            name: "read_file".into(),
            arguments: serde_json::json!({"path": "src/app.ts"}),
        };
        let json = serde_json::to_string(&call).unwrap();
        let parsed: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "call_1");
        assert_eq!(parsed.arguments["path"], "src/app.ts");
    }

    #[test]
    fn tool_result_serde() {
        let result = ToolResult {
            call_id: "call_1".into(),
            content: "file contents here".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: ToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.call_id, "call_1");
    }
}
