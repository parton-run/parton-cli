//! OpenAI API provider.

pub mod helpers;
pub mod tool_use;
pub mod types;

use async_trait::async_trait;
use reqwest::Client;

use parton_core::{
    ModelProvider, ModelResponse, ProviderError, ToolCall, ToolDefinition, ToolResult,
};

use helpers::{parse_error_message, read_env_key, status_to_error, uses_max_completion_tokens};
use types::{ApiResponse, Message, Request, ResponseFormat};

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
    pub(crate) config: OpenAiConfig,
    pub(crate) client: Client,
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
            return Err(status_to_error(status.as_u16(), message));
        }

        let api_response: ApiResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let content = api_response
            .choices
            .into_iter()
            .filter_map(|c| c.message.content)
            .collect::<Vec<_>>()
            .join("");

        Ok(ModelResponse {
            content,
            prompt_tokens: api_response.usage.prompt_tokens,
            completion_tokens: api_response.usage.completion_tokens,
        })
    }

    async fn send_with_tools(
        &self,
        system: &str,
        prompt: &str,
        tools: &[ToolDefinition],
        max_turns: usize,
        handle_tool: &(dyn Fn(ToolCall) -> ToolResult + Send + Sync),
    ) -> Result<ModelResponse, ProviderError> {
        if tools.is_empty() {
            return self.send(system, prompt, true).await;
        }
        tool_use::send_with_tools_loop(self, system, prompt, tools, max_turns, handle_tool).await
    }
}
