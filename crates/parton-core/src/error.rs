//! Core error types shared across workspace crates.

use thiserror::Error;

/// Errors that can occur in the Parton core layer.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Configuration is invalid or missing required fields.
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// Failed to parse a file or data format.
    #[error("parse error: {0}")]
    Parse(String),

    /// I/O error reading or writing files.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_config_displays_message() {
        let err = CoreError::InvalidConfig("missing api key".into());
        assert_eq!(err.to_string(), "invalid config: missing api key");
    }

    #[test]
    fn parse_error_displays_message() {
        let err = CoreError::Parse("unexpected token".into());
        assert_eq!(err.to_string(), "parse error: unexpected token");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err = CoreError::from(io_err);
        assert!(err.to_string().contains("not found"));
    }
}
