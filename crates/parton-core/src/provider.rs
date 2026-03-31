//! Provider trait and response types.
//!
//! The [`ModelProvider`] trait is the core abstraction for LLM communication.
//! It lives in `parton-core` so all crates can reference it without
//! depending on provider implementations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Response from a model provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    /// Generated text content.
    pub content: String,
    /// Number of input tokens consumed.
    pub prompt_tokens: u32,
    /// Number of output tokens generated.
    pub completion_tokens: u32,
}

/// Error type for provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Authentication failed (invalid or missing API key).
    #[error("authentication failed: {0}")]
    Auth(String),

    /// Rate limited by the provider.
    #[error("rate limited: {0}")]
    RateLimited(String),

    /// Provider returned a server error.
    #[error("server error: {0}")]
    Server(String),

    /// Network or connection error.
    #[error("network error: {0}")]
    Network(String),

    /// Invalid configuration (missing env var, bad model name, etc).
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// Generic provider error.
    #[error("provider error: {0}")]
    Other(String),
}

/// Trait for sending prompts to an LLM provider.
///
/// Implementations exist for Anthropic API, OpenAI API, Ollama, and CLI wrappers.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Send a prompt to the model.
    ///
    /// - `system`: system-level instructions or context
    /// - `prompt`: the user prompt
    /// - `json_mode`: if true, force the model to return valid JSON
    async fn send(
        &self,
        system: &str,
        prompt: &str,
        json_mode: bool,
    ) -> Result<ModelResponse, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_response_serde() {
        let response = ModelResponse {
            content: "hello world".into(),
            prompt_tokens: 10,
            completion_tokens: 5,
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: ModelResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, "hello world");
        assert_eq!(parsed.prompt_tokens, 10);
        assert_eq!(parsed.completion_tokens, 5);
    }

    #[test]
    fn provider_error_display() {
        let err = ProviderError::Auth("bad key".into());
        assert_eq!(err.to_string(), "authentication failed: bad key");

        let err = ProviderError::RateLimited("retry after 30s".into());
        assert_eq!(err.to_string(), "rate limited: retry after 30s");
    }
}
