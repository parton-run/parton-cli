//! Multi-turn tool-use loop for the OpenAI provider.

use parton_core::{ModelResponse, ProviderError, ToolCall, ToolDefinition, ToolResult};
use serde::Serialize;

use super::helpers::{parse_error_message, status_to_error, uses_max_completion_tokens};
use super::types::{ApiResponse, ApiToolCall};
use super::OpenAiProvider;

#[derive(Clone, Serialize)]
struct ApiToolDef {
    r#type: &'static str,
    function: ApiFunction,
}

#[derive(Clone, Serialize)]
struct ApiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
enum ChatMsg {
    Text {
        role: String,
        content: String,
    },
    Assistant {
        role: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ApiToolCall>>,
    },
    ToolResult {
        role: String,
        tool_call_id: String,
        content: String,
    },
}

#[derive(Serialize)]
struct ToolRequest {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    messages: Vec<ChatMsg>,
    tools: Vec<ApiToolDef>,
}

fn to_api_tools(tools: &[ToolDefinition]) -> Vec<ApiToolDef> {
    tools
        .iter()
        .map(|t| ApiToolDef {
            r#type: "function",
            function: ApiFunction {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            },
        })
        .collect()
}

fn to_domain_call(api: &ApiToolCall) -> Result<ToolCall, ProviderError> {
    let arguments: serde_json::Value = serde_json::from_str(&api.function.arguments)
        .map_err(|e| ProviderError::Other(format!("bad tool call arguments: {e}")))?;
    Ok(ToolCall {
        id: api.id.clone(),
        name: api.function.name.clone(),
        arguments,
    })
}

/// Run a multi-turn tool-use conversation loop.
pub async fn send_with_tools_loop(
    provider: &OpenAiProvider,
    system: &str,
    prompt: &str,
    tools: &[ToolDefinition],
    max_turns: usize,
    handle_tool: &(dyn Fn(ToolCall) -> ToolResult + Send + Sync),
) -> Result<ModelResponse, ProviderError> {
    let url = format!("{}/v1/chat/completions", provider.config.base_url);
    let use_new = uses_max_completion_tokens(&provider.config.model);
    let api_tools = to_api_tools(tools);

    let mut messages = vec![
        ChatMsg::Text {
            role: "system".into(),
            content: system.into(),
        },
        ChatMsg::Text {
            role: "user".into(),
            content: prompt.into(),
        },
    ];
    let mut total_prompt = 0u32;
    let mut total_completion = 0u32;

    for _ in 0..max_turns {
        let body = ToolRequest {
            model: provider.config.model.clone(),
            max_tokens: if use_new {
                None
            } else {
                Some(provider.config.max_tokens)
            },
            max_completion_tokens: if use_new {
                Some(provider.config.max_tokens)
            } else {
                None
            },
            messages: messages.clone(),
            tools: api_tools.clone(),
        };

        let resp = provider
            .client
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", provider.config.api_key),
            )
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(status_to_error(status.as_u16(), parse_error_message(&text)));
        }

        let api: ApiResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        total_prompt += api.usage.prompt_tokens;
        total_completion += api.usage.completion_tokens;

        let choice = api
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::Other("no choices in response".into()))?;

        let tool_calls = choice.message.tool_calls.unwrap_or_default();
        if tool_calls.is_empty() {
            return Ok(ModelResponse {
                content: choice.message.content.unwrap_or_default(),
                prompt_tokens: total_prompt,
                completion_tokens: total_completion,
            });
        }

        messages.push(ChatMsg::Assistant {
            role: "assistant".into(),
            content: choice.message.content.clone(),
            tool_calls: Some(tool_calls.clone()),
        });

        for api_call in &tool_calls {
            let result = handle_tool(to_domain_call(api_call)?);
            messages.push(ChatMsg::ToolResult {
                role: "tool".into(),
                tool_call_id: result.call_id.clone(),
                content: result.content,
            });
        }
    }

    Ok(ModelResponse {
        content: String::new(),
        prompt_tokens: total_prompt,
        completion_tokens: total_completion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_api_tools_converts() {
        let defs = vec![ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: serde_json::json!({"type": "object"}),
        }];
        let api = to_api_tools(&defs);
        assert_eq!(api.len(), 1);
        assert_eq!(api[0].r#type, "function");
        assert_eq!(api[0].function.name, "read_file");
    }

    #[test]
    fn to_domain_call_parses() {
        let api_call = ApiToolCall {
            id: "call_1".into(),
            r#type: "function".into(),
            function: super::super::types::ApiFunctionCall {
                name: "read_file".into(),
                arguments: r#"{"path":"src/app.ts"}"#.into(),
            },
        };
        let call = to_domain_call(&api_call).unwrap();
        assert_eq!(call.name, "read_file");
        assert_eq!(call.arguments["path"], "src/app.ts");
    }

    #[test]
    fn to_domain_call_bad_json_errors() {
        let api_call = ApiToolCall {
            id: "call_2".into(),
            r#type: "function".into(),
            function: super::super::types::ApiFunctionCall {
                name: "read_file".into(),
                arguments: "not json".into(),
            },
        };
        assert!(to_domain_call(&api_call).is_err());
    }
}
