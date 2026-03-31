//! OpenAI API provider.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use parton_core::{ModelProvider, ModelResponse, ProviderError};

const DEFAULT_BASE_URL: &str = "https://api.openai.com";
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Configuration for the OpenAI provider.
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    /// API key for authentication.
    pub api_key: String,
    /// Model identifier (e.g. `gpt-4o-mini`).
    pub model: String,
    /// Base URL for the API.
    pub base_url: String,
    /// Maximum tokens per response.
    pub max_tokens: u32,
}

/// OpenAI API provider.
#[derive(Debug)]
pub struct OpenAiProvider {
    config: OpenAiConfig,
    client: Client,
}

impl OpenAiProvider {
    /// Create a new provider with explicit configuration.
    pub fn new(config: OpenAiConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    /// Create a provider from environment variables.
    ///
    /// Reads `OPENAI_API_KEY` (or a custom var via `env_key`).
    pub fn from_env(model: Option<String>, env_key: Option<&str>) -> Result<Self, ProviderError> {
        let key_name = env_key.unwrap_or("OPENAI_API_KEY");
        let api_key = read_env_key(key_name)?;

        Ok(Self::new(OpenAiConfig {
            api_key,
            model: model.unwrap_or_else(|| "gpt-4o".into()),
            base_url: DEFAULT_BASE_URL.into(),
            max_tokens: DEFAULT_MAX_TOKENS,
        }))
    }
}

#[async_trait]
impl ModelProvider for OpenAiProvider {
    async fn send(
        &self,
        system: &str,
        prompt: &str,
        json_mode: bool,
    ) -> Result<ModelResponse, ProviderError> {
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let use_new_param = uses_max_completion_tokens(&self.config.model);

        let response_format = if json_mode {
            Some(ResponseFormat {
                r#type: "json_object",
            })
        } else {
            None
        };

        let body = Request {
            model: &self.config.model,
            max_tokens: if use_new_param {
                None
            } else {
                Some(self.config.max_tokens)
            },
            max_completion_tokens: if use_new_param {
                Some(self.config.max_tokens)
            } else {
                None
            },
            response_format,
            messages: vec![
                Message {
                    role: "system",
                    content: system,
                },
                Message {
                    role: "user",
                    content: prompt,
                },
            ],
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            let message = parse_error_message(&text);
            return Err(match status.as_u16() {
                401 => ProviderError::Auth(message),
                429 => ProviderError::RateLimited(message),
                500..=599 => ProviderError::Server(message),
                _ => ProviderError::Other(format!("({status}): {message}")),
            });
        }

        let api_response: ApiResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let content = api_response
            .choices
            .into_iter()
            .map(|c| c.message.content)
            .collect::<Vec<_>>()
            .join("");

        Ok(ModelResponse {
            content,
            prompt_tokens: api_response.usage.prompt_tokens,
            completion_tokens: api_response.usage.completion_tokens,
        })
    }
}

/// Newer OpenAI models require `max_completion_tokens` instead of `max_tokens`.
fn uses_max_completion_tokens(model: &str) -> bool {
    model.starts_with("o1") || model.starts_with("o3") || model.starts_with("gpt-5")
}

/// Read an API key from an environment variable.
fn read_env_key(name: &str) -> Result<String, ProviderError> {
    let key = std::env::var(name).map_err(|_| {
        ProviderError::InvalidConfig(format!("{name} environment variable not set"))
    })?;
    if key.trim().is_empty() {
        return Err(ProviderError::InvalidConfig(format!(
            "{name} is empty or whitespace"
        )));
    }
    Ok(key)
}

/// Extract error message from OpenAI error JSON.
fn parse_error_message(body: &str) -> String {
    serde_json::from_str::<ErrorResponse>(body)
        .map(|e| e.error.message)
        .unwrap_or_else(|_| body.to_string())
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat<'a>>,
    messages: Vec<Message<'a>>,
}

#[derive(Serialize)]
struct ResponseFormat<'a> {
    r#type: &'a str,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ApiResponse {
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_max_completion_tokens_for_new_models() {
        assert!(uses_max_completion_tokens("gpt-5.4"));
        assert!(uses_max_completion_tokens("o1-preview"));
        assert!(uses_max_completion_tokens("o3-mini"));
        assert!(!uses_max_completion_tokens("gpt-4o"));
        assert!(!uses_max_completion_tokens("gpt-4o-mini"));
    }

    #[test]
    fn read_env_key_missing() {
        let result = read_env_key("PARTON_TEST_NONEXISTENT_KEY_12345");
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_message_valid_json() {
        let body = r#"{"error":{"message":"Invalid API key","type":"invalid_request_error"}}"#;
        assert_eq!(parse_error_message(body), "Invalid API key");
    }

    #[test]
    fn parse_error_message_invalid_json() {
        let body = "not json";
        assert_eq!(parse_error_message(body), "not json");
    }

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
}
