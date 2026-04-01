//! Multi-turn tool-use loop for CLI providers.
//!
//! Injects tool definitions into the prompt and parses structured
//! JSON responses to simulate a tool-use conversation loop via
//! one-shot CLI invocations (claude --print, codex exec, etc.).

use parton_core::{ModelResponse, ProviderError, ToolCall, ToolDefinition, ToolResult};
use tracing::debug;

/// Build the tool-use system prompt suffix.
///
/// Tells the model about available tools and the expected JSON format.
pub fn build_tool_prompt(tools: &[ToolDefinition]) -> String {
    let tool_list: Vec<String> = tools
        .iter()
        .map(|t| {
            format!(
                "- {}({}): {}",
                t.name,
                compact_params(&t.parameters),
                t.description
            )
        })
        .collect();

    format!(
        r#"

## Available Tools
You have access to these tools to inspect the codebase:
{}

## Response Format
You MUST respond with ONLY a JSON object (no markdown fences).

If you need to call tools first, respond with:
{{"action":"tool_call","calls":[{{"name":"tool_name","arguments":{{"param":"value"}}}}]}}

When you have enough information, respond with your final answer:
{{"action":"final","content":"your actual response here"}}

You can call multiple tools at once. Always call tools BEFORE giving your final answer if you need more context."#,
        tool_list.join("\n")
    )
}

/// Build a follow-up prompt with tool results.
pub fn build_followup_prompt(original_prompt: &str, tool_results: &[(String, String)]) -> String {
    let results: Vec<String> = tool_results
        .iter()
        .map(|(name, content)| format!("## Result of {name}\n{content}"))
        .collect();

    format!(
        "{original_prompt}\n\n## Tool Results\n{}\n\n\
         Use the tool results above. If you need more tools, respond with \
         {{\"action\":\"tool_call\",...}}. Otherwise respond with {{\"action\":\"final\",\"content\":\"...\"}}.",
        results.join("\n\n")
    )
}

/// Parse a CLI response and extract either tool calls or final content.
pub fn parse_tool_response(raw: &str) -> Result<ToolResponse, ProviderError> {
    // Strip control characters (except \n \r \t) that Claude CLI sometimes injects.
    let sanitized: String = raw
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\r' && c != '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let trimmed = sanitized.trim();

    // Extract the first JSON object, ignoring trailing text.
    let json_str = extract_json_object(trimmed)
        .ok_or_else(|| ProviderError::Other("no JSON object in tool response".into()))?;

    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| ProviderError::Other(format!("failed to parse tool response: {e}")))?;

    match parsed.get("action").and_then(|a| a.as_str()) {
        Some("tool_call") => {
            let calls = parsed
                .get("calls")
                .and_then(|c| c.as_array())
                .ok_or_else(|| ProviderError::Other("tool_call missing calls array".into()))?;

            let tool_calls: Vec<ToolCall> = calls
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    // Accept both {"name":"x","arguments":{}} and {"tool":"x","path":"y"}.
                    let name = c["name"]
                        .as_str()
                        .or_else(|| c["tool"].as_str())
                        .unwrap_or("")
                        .into();
                    let arguments = if let Some(args) = c.get("arguments") {
                        args.clone()
                    } else {
                        // Fallback: treat all non-meta keys as arguments.
                        let mut args = serde_json::Map::new();
                        for (k, v) in c.as_object().into_iter().flatten() {
                            if k != "name" && k != "tool" {
                                args.insert(k.clone(), v.clone());
                            }
                        }
                        serde_json::Value::Object(args)
                    };
                    ToolCall {
                        id: format!("cli_{i}"),
                        name,
                        arguments,
                    }
                })
                .collect();
            Ok(ToolResponse::ToolCalls(tool_calls))
        }
        Some("final") => {
            let content = parsed
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            Ok(ToolResponse::Final(content))
        }
        _ => {
            // Model didn't follow format — treat raw output as final.
            Ok(ToolResponse::Final(raw.to_string()))
        }
    }
}

/// Run the multi-turn tool-use loop via repeated CLI invocations.
pub fn run_tool_loop(
    send_fn: &dyn Fn(&str, &str) -> Result<ModelResponse, ProviderError>,
    system: &str,
    prompt: &str,
    tools: &[ToolDefinition],
    max_turns: usize,
    handle_tool: &(dyn Fn(ToolCall) -> ToolResult + Send + Sync),
) -> Result<ModelResponse, ProviderError> {
    let tool_suffix = build_tool_prompt(tools);
    let augmented_system = format!("{system}{tool_suffix}");

    let mut current_prompt = prompt.to_string();
    let mut total_prompt_tokens = 0u32;
    let mut total_completion_tokens = 0u32;

    for turn in 0..max_turns {
        let resp = send_fn(&augmented_system, &current_prompt)?;
        total_prompt_tokens += resp.prompt_tokens;
        total_completion_tokens += resp.completion_tokens;

        debug!(turn, raw_len = resp.content.len(), "cli tool use response");
        debug!(raw_content = %resp.content.chars().take(500).collect::<String>(), "response preview");

        match parse_tool_response(&resp.content)? {
            ToolResponse::Final(content) => {
                return Ok(ModelResponse {
                    content,
                    prompt_tokens: total_prompt_tokens,
                    completion_tokens: total_completion_tokens,
                });
            }
            ToolResponse::ToolCalls(calls) => {
                let results: Vec<(String, String)> = calls
                    .into_iter()
                    .map(|call| {
                        let name = call.name.clone();
                        let result = handle_tool(call);
                        (name, result.content)
                    })
                    .collect();
                current_prompt = build_followup_prompt(prompt, &results);
            }
        }
    }

    // max_turns exhausted — do one final call without tools.
    let resp = send_fn(system, &current_prompt)?;
    Ok(ModelResponse {
        content: resp.content,
        prompt_tokens: total_prompt_tokens + resp.prompt_tokens,
        completion_tokens: total_completion_tokens + resp.completion_tokens,
    })
}

/// Parsed tool response from CLI output.
pub enum ToolResponse {
    /// Model wants to call tools.
    ToolCalls(Vec<ToolCall>),
    /// Model's final answer.
    Final(String),
}

/// Extract the first balanced JSON object from a string.
///
/// Handles markdown fences, leading text, and trailing text after
/// the closing brace. Returns the substring containing the JSON object.
fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if escape_next {
            escape_next = false;
            continue;
        }
        match b {
            b'\\' if in_string => escape_next = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Compact parameter display for the tool prompt.
fn compact_params(schema: &serde_json::Value) -> String {
    schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|props| {
            props
                .keys()
                .map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_final_response() {
        let raw = r#"{"action":"final","content":"here is the plan"}"#;
        let resp = parse_tool_response(raw).unwrap();
        assert!(matches!(resp, ToolResponse::Final(c) if c == "here is the plan"));
    }

    #[test]
    fn parse_tool_call_response() {
        let raw =
            r#"{"action":"tool_call","calls":[{"name":"read_file","arguments":{"path":"a.ts"}}]}"#;
        let resp = parse_tool_response(raw).unwrap();
        match resp {
            ToolResponse::ToolCalls(calls) => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].name, "read_file");
                assert_eq!(calls[0].arguments["path"], "a.ts");
            }
            _ => panic!("expected tool calls"),
        }
    }

    #[test]
    fn parse_with_markdown_fences() {
        let raw = "```json\n{\"action\":\"final\",\"content\":\"ok\"}\n```";
        let resp = parse_tool_response(raw).unwrap();
        assert!(matches!(resp, ToolResponse::Final(c) if c == "ok"));
    }

    #[test]
    fn parse_unknown_action_is_final() {
        let raw = "just plain text, no JSON";
        let resp = parse_tool_response(raw);
        assert!(resp.is_err());
    }

    #[test]
    fn parse_with_trailing_text() {
        let raw = r#"{"action":"final","content":"ok"}

I also want to mention something else."#;
        let resp = parse_tool_response(raw).unwrap();
        assert!(matches!(resp, ToolResponse::Final(c) if c == "ok"));
    }

    #[test]
    fn parse_with_leading_text() {
        let raw = r#"Here is my response:
{"action":"tool_call","calls":[{"name":"read_file","arguments":{"path":"a.ts"}}]}"#;
        let resp = parse_tool_response(raw).unwrap();
        assert!(matches!(resp, ToolResponse::ToolCalls(c) if c.len() == 1));
    }

    #[test]
    fn extract_json_handles_nested() {
        let s = r#"text {"a":{"b":1}} more"#;
        assert_eq!(extract_json_object(s), Some(r#"{"a":{"b":1}}"#));
    }

    #[test]
    fn extract_json_handles_strings_with_braces() {
        let s = r#"{"content":"hello {world}"}"#;
        assert_eq!(extract_json_object(s), Some(s));
    }

    #[test]
    fn build_tool_prompt_includes_tools() {
        let tools = vec![ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}}}),
        }];
        let prompt = build_tool_prompt(&tools);
        assert!(prompt.contains("read_file"));
        assert!(prompt.contains("path"));
        assert!(prompt.contains("tool_call"));
    }

    #[test]
    fn compact_params_extracts_keys() {
        let schema = serde_json::json!({"type":"object","properties":{"path":{},"query":{}}});
        let result = compact_params(&schema);
        assert!(result.contains("path"));
        assert!(result.contains("query"));
    }

    #[test]
    fn build_followup_includes_results() {
        let results = vec![("read_file".into(), "file contents".into())];
        let followup = build_followup_prompt("original", &results);
        assert!(followup.contains("original"));
        assert!(followup.contains("file contents"));
        assert!(followup.contains("Result of read_file"));
    }

    #[test]
    fn run_tool_loop_with_tool_call_then_final() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let call_count = AtomicUsize::new(0);

        let send_fn = |_sys: &str, _p: &str| -> Result<ModelResponse, ProviderError> {
            let n = call_count.fetch_add(1, Ordering::SeqCst);
            let content = if n == 0 {
                // First call: model wants to call a tool.
                r#"{"action":"tool_call","calls":[{"name":"read_file","arguments":{"path":"src/app.ts"}}]}"#
            } else {
                // Second call: model gives final answer.
                r#"{"action":"final","content":"{\"summary\":\"the plan\"}"}"#
            };
            Ok(ModelResponse {
                content: content.into(),
                prompt_tokens: 100,
                completion_tokens: 50,
            })
        };

        let tools = vec![ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: serde_json::json!({"type":"object","properties":{"path":{}}}),
        }];

        let handle = |call: ToolCall| -> ToolResult {
            assert_eq!(call.name, "read_file");
            ToolResult {
                call_id: call.id,
                content: "export const App = () => {}".into(),
            }
        };

        let result = run_tool_loop(&send_fn, "system", "prompt", &tools, 5, &handle).unwrap();
        assert!(result.content.contains("the plan"));
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
        assert_eq!(result.prompt_tokens, 200);
    }

    #[test]
    fn run_tool_loop_direct_final() {
        let send_fn = |_sys: &str, _p: &str| -> Result<ModelResponse, ProviderError> {
            Ok(ModelResponse {
                content: r#"{"action":"final","content":"done"}"#.into(),
                prompt_tokens: 50,
                completion_tokens: 20,
            })
        };
        let tools = vec![ToolDefinition {
            name: "x".into(),
            description: "x".into(),
            parameters: serde_json::json!({}),
        }];
        let handle = |call: ToolCall| -> ToolResult {
            ToolResult {
                call_id: call.id,
                content: String::new(),
            }
        };
        let result = run_tool_loop(&send_fn, "sys", "p", &tools, 5, &handle).unwrap();
        assert_eq!(result.content, "done");
    }

    #[test]
    fn run_tool_loop_model_ignores_format() {
        // Model returns plain text without JSON — should still work.
        let send_fn = |_sys: &str, _p: &str| -> Result<ModelResponse, ProviderError> {
            Ok(ModelResponse {
                content: "Here is the plan without JSON format".into(),
                prompt_tokens: 50,
                completion_tokens: 20,
            })
        };
        let tools = vec![ToolDefinition {
            name: "x".into(),
            description: "x".into(),
            parameters: serde_json::json!({}),
        }];
        let handle = |call: ToolCall| -> ToolResult {
            ToolResult {
                call_id: call.id,
                content: String::new(),
            }
        };
        let result = run_tool_loop(&send_fn, "sys", "p", &tools, 5, &handle);
        // Should error since no JSON found.
        assert!(result.is_err());
    }

    #[test]
    fn parse_tool_call_with_tool_key() {
        // Claude sometimes returns "tool" instead of "name".
        let raw = r#"{"action":"tool_call","calls":[{"tool":"read_file","path":"lib/auth.ts"}]}"#;
        let resp = parse_tool_response(raw).unwrap();
        match resp {
            ToolResponse::ToolCalls(calls) => {
                assert_eq!(calls[0].name, "read_file");
                assert_eq!(calls[0].arguments["path"], "lib/auth.ts");
            }
            _ => panic!("expected tool calls"),
        }
    }

    #[test]
    fn parse_claude_style_multiline_response() {
        // Simulate what claude CLI might actually return — JSON followed
        // by trailing explanation text.
        let raw = r#"{"action":"tool_call","calls":[{"name":"get_exports","arguments":{"path":"lib/auth.ts"}},{"name":"read_file","arguments":{"path":"lib/db/schema.ts"}}]}

I want to inspect the auth module and database schema to understand the existing permission system before planning."#;
        let resp = parse_tool_response(raw).unwrap();
        match resp {
            ToolResponse::ToolCalls(calls) => {
                assert_eq!(calls.len(), 2);
                assert_eq!(calls[0].name, "get_exports");
                assert_eq!(calls[1].name, "read_file");
            }
            _ => panic!("expected tool calls"),
        }
    }
}
