//! Graph error types.

/// Errors from graph operations.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    /// Language not supported for parsing.
    #[error("language not supported: {0}")]
    UnsupportedLanguage(String),

    /// Grammar download failed.
    #[error("grammar download failed: {0}")]
    GrammarDownload(String),

    /// File parsing failed.
    #[error("parse failed for {path}: {reason}")]
    ParseFailed {
        /// File path that failed.
        path: String,
        /// Reason for failure.
        reason: String,
    },

    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Tree-sitter query error.
    #[error("query error: {0}")]
    Query(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_unsupported() {
        let err = GraphError::UnsupportedLanguage("brainfuck".into());
        assert_eq!(err.to_string(), "language not supported: brainfuck");
    }

    #[test]
    fn error_display_parse_failed() {
        let err = GraphError::ParseFailed {
            path: "src/app.ts".into(),
            reason: "syntax error".into(),
        };
        assert_eq!(err.to_string(), "parse failed for src/app.ts: syntax error");
    }

    #[test]
    fn error_display_download() {
        let err = GraphError::GrammarDownload("404 not found".into());
        assert_eq!(err.to_string(), "grammar download failed: 404 not found");
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err = GraphError::from(io_err);
        assert!(err.to_string().contains("missing"));
    }
}
