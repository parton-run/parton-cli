//! Serialization types for the OpenAI chat completions API.

use serde::{Deserialize, Serialize};

/// Chat completion request body.
#[derive(Serialize)]
pub struct Request<'a> {
    pub model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat<'a>>,
    pub messages: Vec<Message<'a>>,
}

/// Response format override.
#[derive(Serialize)]
pub struct ResponseFormat<'a> {
    pub r#type: &'a str,
}

/// A simple chat message (system/user).
#[derive(Serialize)]
pub struct Message<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

/// Chat completion response.
#[derive(Deserialize)]
pub struct ApiResponse {
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

/// A single completion choice.
#[derive(Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
}

/// Message returned by the model.
#[derive(Deserialize)]
pub struct ResponseMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ApiToolCall>>,
}

/// A tool call in the API response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ApiFunctionCall,
}

/// Function call details.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiFunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Token usage stats.
#[derive(Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// API error envelope.
#[derive(Deserialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

/// API error details.
#[derive(Deserialize)]
pub struct ErrorDetail {
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serialization_old_model() {
        let req = Request {
            model: "gpt-4o",
            max_tokens: Some(4096),
            max_completion_tokens: None,
            response_format: None,
            messages: vec![Message {
                role: "user",
                content: "hi",
            }],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["max_tokens"], 4096);
        assert!(json.get("max_completion_tokens").is_none());
        assert!(json.get("response_format").is_none());
    }

    #[test]
    fn request_serialization_json_mode() {
        let req = Request {
            model: "gpt-4o",
            max_tokens: Some(4096),
            max_completion_tokens: None,
            response_format: Some(ResponseFormat {
                r#type: "json_object",
            }),
            messages: vec![Message {
                role: "user",
                content: "hi",
            }],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["response_format"]["type"], "json_object");
    }

    #[test]
    fn response_message_without_tools() {
        let json = r#"{"content": "hello", "tool_calls": null}"#;
        let msg: ResponseMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.unwrap(), "hello");
        assert!(msg.tool_calls.is_none());
    }

    #[test]
    fn response_message_with_tools() {
        let json = r#"{
            "content": null,
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {"name": "read_file", "arguments": "{\"path\":\"a.ts\"}"}
            }]
        }"#;
        let msg: ResponseMessage = serde_json::from_str(json).unwrap();
        assert!(msg.content.is_none());
        let calls = msg.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "read_file");
    }
}
