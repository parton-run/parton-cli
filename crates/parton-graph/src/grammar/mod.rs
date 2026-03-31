//! Grammar loading and caching via tree-sitter Wasm.
//!
//! Manages lazy download and runtime loading of tree-sitter
//! grammar `.wasm` files for supported languages.

pub mod download;
pub mod registry;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::GraphError;
use crate::types::Language;

/// Manages loaded tree-sitter grammars.
///
/// Grammars are downloaded lazily and cached on disk. Once loaded
/// into the Wasm runtime, they stay in memory for the process lifetime.
#[derive(Debug)]
pub struct GrammarStore {
    cache_dir: PathBuf,
    loaded: HashMap<Language, tree_sitter::Language>,
    engine: tree_sitter::wasmtime::Engine,
}

impl GrammarStore {
    /// Create a new grammar store with the given cache directory.
    pub fn new(cache_dir: impl Into<PathBuf>) -> Result<Self, GraphError> {
        let engine = tree_sitter::wasmtime::Engine::default();
        Ok(Self {
            cache_dir: cache_dir.into(),
            loaded: HashMap::new(),
            engine,
        })
    }

    /// Create a store using the default cache directory (`~/.parton/grammars/`).
    pub fn with_default_cache() -> Result<Self, GraphError> {
        Self::new(download::default_cache_dir())
    }

    /// Get a loaded grammar, or download and load it.
    pub async fn get_or_load(
        &mut self,
        lang: Language,
    ) -> Result<tree_sitter::Language, GraphError> {
        if let Some(grammar) = self.loaded.get(&lang) {
            return Ok(grammar.clone());
        }

        let wasm_path = download::ensure_grammar(lang, &self.cache_dir).await?;
        let wasm_bytes = std::fs::read(&wasm_path)?;

        let load_name = registry::grammar_load_name(lang)
            .ok_or_else(|| GraphError::UnsupportedLanguage(lang.to_string()))?;

        let mut store = tree_sitter::WasmStore::new(&self.engine)
            .map_err(|e| GraphError::Query(e.to_string()))?;

        let grammar = store
            .load_language(load_name, &wasm_bytes)
            .map_err(|e| GraphError::GrammarDownload(format!("wasm load failed: {e}")))?;

        self.loaded.insert(lang, grammar.clone());
        Ok(grammar)
    }

    /// Check if a grammar is already loaded in memory.
    pub fn is_loaded(&self, lang: Language) -> bool {
        self.loaded.contains_key(&lang)
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = GrammarStore::new(dir.path());
        assert!(store.is_ok());
    }

    #[test]
    fn not_loaded_initially() {
        let dir = tempfile::tempdir().unwrap();
        let store = GrammarStore::new(dir.path()).unwrap();
        assert!(!store.is_loaded(Language::TypeScript));
    }

    #[test]
    fn cache_dir_stored() {
        let dir = tempfile::tempdir().unwrap();
        let store = GrammarStore::new(dir.path()).unwrap();
        assert_eq!(store.cache_dir(), dir.path());
    }
}
