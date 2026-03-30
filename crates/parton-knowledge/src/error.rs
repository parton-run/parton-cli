//! Knowledge store error types.

/// Errors from knowledge store operations.
#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    /// Entry not found.
    #[error("knowledge entry not found: {0}")]
    NotFound(String),

    /// Entry data is invalid.
    #[error("invalid knowledge entry: {0}")]
    Invalid(String),

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = KnowledgeError::NotFound("abc".into());
        assert_eq!(err.to_string(), "knowledge entry not found: abc");
    }
}
