//! Graph store error types.

/// Errors from graph store operations.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    /// SQLite error.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Entry not found.
    #[error("not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = GraphError::NotFound("file abc".into());
        assert_eq!(err.to_string(), "not found: file abc");
    }
}
