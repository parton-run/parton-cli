//! Shared helper functions for the OpenAI provider.

use parton_core::ProviderError;

use super::types::ErrorResponse;

/// Newer OpenAI models require `max_completion_tokens` instead of `max_tokens`.
pub fn uses_max_completion_tokens(model: &str) -> bool {
    model.starts_with("o1") || model.starts_with("o3") || model.starts_with("gpt-5")
}

/// Read an API key from an environment variable.
pub fn read_env_key(name: &str) -> Result<String, ProviderError> {
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
pub fn parse_error_message(body: &str) -> String {
    serde_json::from_str::<ErrorResponse>(body)
        .map(|e| e.error.message)
        .unwrap_or_else(|_| body.to_string())
}

/// Map an HTTP status code to a ProviderError.
pub fn status_to_error(status: u16, message: String) -> ProviderError {
    match status {
        401 => ProviderError::Auth(message),
        429 => ProviderError::RateLimited(message),
        500..=599 => ProviderError::Server(message),
        _ => ProviderError::Other(format!("({status}): {message}")),
    }
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
        assert_eq!(parse_error_message("not json"), "not json");
    }

    #[test]
    fn status_to_error_maps_correctly() {
        assert!(matches!(
            status_to_error(401, "bad".into()),
            ProviderError::Auth(_)
        ));
        assert!(matches!(
            status_to_error(429, "slow".into()),
            ProviderError::RateLimited(_)
        ));
        assert!(matches!(
            status_to_error(500, "oops".into()),
            ProviderError::Server(_)
        ));
        assert!(matches!(
            status_to_error(400, "bad req".into()),
            ProviderError::Other(_)
        ));
    }
}
