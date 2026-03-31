//! Async grammar download with disk cache.
//!
//! Downloads `.wasm` grammar files on first use and caches them
//! in `~/.parton/grammars/` for subsequent runs.

use std::path::{Path, PathBuf};

use crate::error::GraphError;
use crate::grammar::registry;
use crate::types::Language;

/// Default cache directory for grammar files.
pub fn default_cache_dir() -> PathBuf {
    dirs_path().join("grammars")
}

/// Ensure a grammar `.wasm` file is available locally.
///
/// Returns the path to the cached `.wasm` file. Downloads from
/// the tree-sitter GitHub registry if not already cached.
pub async fn ensure_grammar(lang: Language, cache_dir: &Path) -> Result<PathBuf, GraphError> {
    let filename = registry::grammar_filename(lang)
        .ok_or_else(|| GraphError::UnsupportedLanguage(lang.to_string()))?;

    let cached_path = cache_dir.join(&filename);

    if cached_path.exists() {
        tracing::debug!("grammar cache hit: {filename}");
        return Ok(cached_path);
    }

    let url = registry::grammar_url(lang)
        .ok_or_else(|| GraphError::UnsupportedLanguage(lang.to_string()))?;

    tracing::info!("downloading grammar: {url}");
    let bytes = download_bytes(&url).await?;

    std::fs::create_dir_all(cache_dir)?;
    std::fs::write(&cached_path, &bytes)?;
    tracing::info!("cached grammar: {filename} ({} bytes)", bytes.len());

    Ok(cached_path)
}

/// Download raw bytes from a URL.
async fn download_bytes(url: &str) -> Result<Vec<u8>, GraphError> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| GraphError::GrammarDownload(e.to_string()))?;

    if !response.status().is_success() {
        return Err(GraphError::GrammarDownload(format!(
            "http {}",
            response.status()
        )));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| GraphError::GrammarDownload(e.to_string()))
}

/// Platform home-based `.parton` directory.
fn dirs_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".parton")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cache_dir_ends_with_grammars() {
        let dir = default_cache_dir();
        assert!(dir.ends_with("grammars"));
    }

    #[test]
    fn cache_hit_returns_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let wasm_path = dir.path().join("tree-sitter-python.wasm");
        std::fs::write(&wasm_path, b"fake wasm").unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(ensure_grammar(Language::Python, dir.path()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), wasm_path);
    }

    #[test]
    fn unsupported_language_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(ensure_grammar(Language::Unknown, dir.path()));
        assert!(result.is_err());
    }
}
