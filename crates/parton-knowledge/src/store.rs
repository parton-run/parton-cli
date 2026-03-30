//! Knowledge store trait and filesystem implementation.

use std::path::{Path, PathBuf};

use crate::entry::{Category, Entry};
use crate::error::KnowledgeError;

/// Abstract knowledge storage backend.
pub trait KnowledgeStore {
    /// List all entries.
    fn list(&self) -> Result<Vec<Entry>, KnowledgeError>;

    /// Get a single entry by ID.
    fn get(&self, id: &str) -> Result<Entry, KnowledgeError>;

    /// Insert or update an entry.
    fn upsert(&self, entry: &Entry) -> Result<(), KnowledgeError>;

    /// Delete an entry by ID.
    fn delete(&self, id: &str) -> Result<(), KnowledgeError>;

    /// Search entries by text query and/or tags.
    fn search(&self, query: &str, tags: &[String]) -> Result<Vec<Entry>, KnowledgeError>;
}

/// Filesystem-backed knowledge store.
///
/// Stores entries as JSON files under `{root}/knowledge/{category}/{id}.json`.
#[derive(Debug)]
pub struct LocalStore {
    root: PathBuf,
}

impl LocalStore {
    /// Create a new store rooted at the given directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Path to the knowledge directory.
    fn knowledge_dir(&self) -> PathBuf {
        self.root.join("knowledge")
    }

    /// Path to a specific entry file.
    fn entry_path(&self, category: Category, id: &str) -> PathBuf {
        self.knowledge_dir()
            .join(category.to_string())
            .join(format!("{id}.json"))
    }

    /// Read all JSON files from a directory.
    fn read_entries_from_dir(&self, dir: &Path) -> Vec<Entry> {
        let read_dir = match std::fs::read_dir(dir) {
            Ok(d) => d,
            Err(_) => return vec![],
        };

        read_dir
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|e| {
                let content = std::fs::read_to_string(e.path()).ok()?;
                serde_json::from_str::<Entry>(&content).ok()
            })
            .collect()
    }
}

impl KnowledgeStore for LocalStore {
    fn list(&self) -> Result<Vec<Entry>, KnowledgeError> {
        let dir = self.knowledge_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut entries = Vec::new();
        let subdirs = std::fs::read_dir(&dir)?;
        for subdir in subdirs.filter_map(|e| e.ok()) {
            if subdir.path().is_dir() {
                entries.extend(self.read_entries_from_dir(&subdir.path()));
            }
        }
        Ok(entries)
    }

    fn get(&self, id: &str) -> Result<Entry, KnowledgeError> {
        // Search across all category directories.
        let dir = self.knowledge_dir();
        if !dir.exists() {
            return Err(KnowledgeError::NotFound(id.into()));
        }

        for subdir in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
            let path = subdir.path().join(format!("{id}.json"));
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                return serde_json::from_str(&content)
                    .map_err(|e| KnowledgeError::Serialization(e.to_string()));
            }
        }

        Err(KnowledgeError::NotFound(id.into()))
    }

    fn upsert(&self, entry: &Entry) -> Result<(), KnowledgeError> {
        let path = self.entry_path(entry.category, &entry.id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(entry)
            .map_err(|e| KnowledgeError::Serialization(e.to_string()))?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), KnowledgeError> {
        let dir = self.knowledge_dir();
        if !dir.exists() {
            return Err(KnowledgeError::NotFound(id.into()));
        }

        for subdir in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
            let path = subdir.path().join(format!("{id}.json"));
            if path.exists() {
                std::fs::remove_file(&path)?;
                return Ok(());
            }
        }

        Err(KnowledgeError::NotFound(id.into()))
    }

    fn search(&self, query: &str, tags: &[String]) -> Result<Vec<Entry>, KnowledgeError> {
        let all = self.list()?;
        let query_lower = query.to_lowercase();

        Ok(all
            .into_iter()
            .filter(|e| {
                let text_match = query.is_empty()
                    || e.title.to_lowercase().contains(&query_lower)
                    || e.content.to_lowercase().contains(&query_lower);
                let tag_match =
                    tags.is_empty() || e.tags.iter().any(|t| tags.contains(t));
                text_match && tag_match
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::Source;

    fn test_entry(id: &str) -> Entry {
        Entry {
            id: id.into(),
            category: Category::Convention,
            title: format!("Test {id}"),
            content: "test content".into(),
            tags: vec!["rust".into()],
            source: Source::Auto,
        }
    }

    #[test]
    fn upsert_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::new(dir.path());
        let entry = test_entry("conv-01");

        store.upsert(&entry).unwrap();
        let retrieved = store.get("conv-01").unwrap();
        assert_eq!(retrieved.title, "Test conv-01");
    }

    #[test]
    fn list_entries() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::new(dir.path());
        store.upsert(&test_entry("a")).unwrap();
        store.upsert(&test_entry("b")).unwrap();

        let entries = store.list().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn list_empty_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::new(dir.path());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn get_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::new(dir.path());
        assert!(matches!(
            store.get("missing"),
            Err(KnowledgeError::NotFound(_))
        ));
    }

    #[test]
    fn delete_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::new(dir.path());
        store.upsert(&test_entry("del")).unwrap();
        store.delete("del").unwrap();
        assert!(store.get("del").is_err());
    }

    #[test]
    fn delete_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::new(dir.path());
        assert!(store.delete("nope").is_err());
    }

    #[test]
    fn search_by_text() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::new(dir.path());
        store.upsert(&test_entry("a")).unwrap();

        let results = store.search("Test a", &[]).unwrap();
        assert_eq!(results.len(), 1);

        let results = store.search("nonexistent", &[]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_by_tags() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalStore::new(dir.path());
        store.upsert(&test_entry("a")).unwrap();

        let results = store.search("", &["rust".into()]).unwrap();
        assert_eq!(results.len(), 1);

        let results = store.search("", &["python".into()]).unwrap();
        assert!(results.is_empty());
    }
}
