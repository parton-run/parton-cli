//! Ollama local LLM provider.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use parton_core::{ModelProvider, ModelResponse, ProviderError};

const DEFAULT_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "qwen2.5-coder:7b";

/// Ollama provider for local LLM inference.
#[derive(Debug)]
pub struct OllamaProvider {
    base_url: String,
    model: String,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    ///
    /// If `model` is `None`, defaults to `qwen2.5-coder:7b`.
    /// Reads `OLLAMA_BASE_URL` from env or defaults to `localhost:11434`.
    pub fn new(model: Option<String>) -> Self {
        let base_url = std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());
        let model = model.unwrap_or_else(|| {
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into())
        });
        Self { base_url, model }
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    async fn send(
        &self,
        system: &str,
        prompt: &str,
        _stream: bool,
    ) -> Result<ModelResponse, ProviderError> {
        let full_prompt = if system.is_empty() {
            prompt.to_string()
        } else {
            format!("{system}\n\n{prompt}")
        };

        let client = reqwest::Client::new();
        let url = format!("{}/api/generate", self.base_url);

        let request = GenerateRequest {
            model: &self.model,
            prompt: &full_prompt,
            stream: false,
        };

        let resp = client.post(&url).json(&request).send().await.map_err(|e| {
            ProviderError::Network(format!(
                "ollama request failed (is ollama running at {}?): {}",
                self.base_url, e
            ))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Other(format!(
                "ollama returned {status}: {body}"
            )));
        }

        let result: GenerateResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::Other(format!("failed to parse ollama response: {e}")))?;

        Ok(ModelResponse {
            content: result.response,
            prompt_tokens: result.prompt_eval_count,
            completion_tokens: result.eval_count,
        })
    }
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
    #[serde(default)]
    prompt_eval_count: u32,
    #[serde(default)]
    eval_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model() {
        let provider = OllamaProvider::new(None);
        // Either env var or default
        assert!(!provider.model.is_empty());
    }

    #[test]
    fn custom_model() {
        let provider = OllamaProvider::new(Some("llama3:8b".into()));
        assert_eq!(provider.model, "llama3:8b");
    }

    #[test]
    fn request_serialization() {
        let req = GenerateRequest {
            model: "qwen2.5-coder:7b",
            prompt: "hello",
            stream: false,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["model"], "qwen2.5-coder:7b");
        assert_eq!(json["stream"], false);
    }
}
