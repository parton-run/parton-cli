//! Map lock file — tracks per-file state for incremental updates.
//!
//! Format: `path:lines:hash` per line, with a header for git sha.

use std::collections::HashMap;
use std::path::Path;

/// Lock file state.
#[derive(Debug, Default)]
pub struct MapLock {
    /// Git sha at time of last full build.
    pub git_sha: String,
    /// Per-file state: path → (line_count, content_hash).
    pub files: HashMap<String, FileState>,
}

/// State of a single file at scan time.
#[derive(Debug, Clone)]
pub struct FileState {
    /// Number of lines.
    pub lines: usize,
    /// Simple content hash (first 8 hex chars of hash).
    pub hash: String,
}

/// What changed since the last lock.
#[derive(Debug)]
pub struct LockDiff {
    /// Files that are new or have changed hash.
    pub changed: Vec<String>,
    /// Files that were in the lock but no longer exist.
    pub removed: Vec<String>,
}

/// Save a lock file to `.parton/map.lock`.
pub fn save_lock(lock: &MapLock, project_root: &Path) -> Result<(), std::io::Error> {
    let dir = project_root.join(".parton");
    std::fs::create_dir_all(&dir)?;

    let mut lines = vec![format!("sha={}", lock.git_sha)];
    let mut paths: Vec<&String> = lock.files.keys().collect();
    paths.sort();
    for path in paths {
        let state = &lock.files[path];
        lines.push(format!("{}:{}:{}", path, state.lines, state.hash));
    }
    std::fs::write(dir.join("map.lock"), lines.join("\n"))
}

/// Load a lock file from `.parton/map.lock`.
pub fn load_lock(project_root: &Path) -> Option<MapLock> {
    let content = std::fs::read_to_string(project_root.join(".parton/map.lock")).ok()?;
    let mut lock = MapLock::default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(sha) = line.strip_prefix("sha=") {
            lock.git_sha = sha.to_string();
        } else {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() == 3 {
                lock.files.insert(
                    parts[0].to_string(),
                    FileState {
                        lines: parts[1].parse().unwrap_or(0),
                        hash: parts[2].to_string(),
                    },
                );
            }
        }
    }

    if lock.git_sha.is_empty() {
        return None;
    }
    Some(lock)
}

/// Build a lock from the current project state.
pub fn build_lock(file_paths: &[String], project_root: &Path, git_sha: &str) -> MapLock {
    let mut files = HashMap::new();
    for path in file_paths {
        let full = project_root.join(path);
        if let Ok(content) = std::fs::read_to_string(&full) {
            files.insert(
                path.clone(),
                FileState {
                    lines: content.lines().count(),
                    hash: quick_hash(&content),
                },
            );
        }
    }
    MapLock {
        git_sha: git_sha.to_string(),
        files,
    }
}

/// Diff current project state against a lock to find changes.
pub fn diff_lock(lock: &MapLock, file_paths: &[String], project_root: &Path) -> LockDiff {
    let mut changed = Vec::new();
    let mut current_set: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for path in file_paths {
        current_set.insert(path.as_str());
        let full = project_root.join(path);
        let content = match std::fs::read_to_string(&full) {
            Ok(c) => c,
            Err(_) => {
                changed.push(path.clone());
                continue;
            }
        };
        let hash = quick_hash(&content);
        match lock.files.get(path) {
            Some(state) if state.hash == hash => {} // Unchanged.
            _ => changed.push(path.clone()),
        }
    }

    let removed = lock
        .files
        .keys()
        .filter(|p| !current_set.contains(p.as_str()))
        .cloned()
        .collect();

    LockDiff { changed, removed }
}

/// Quick non-cryptographic hash of file content.
fn quick_hash(content: &str) -> String {
    // Simple djb2 hash — fast, good enough for change detection.
    let mut hash: u64 = 5381;
    for byte in content.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_save_lock() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ts"), "const x = 1;\n").unwrap();
        let lock = build_lock(&["a.ts".into()], dir.path(), "abc123");
        assert_eq!(lock.files.len(), 1);
        assert_eq!(lock.files["a.ts"].lines, 1);
        save_lock(&lock, dir.path()).unwrap();
        assert!(dir.path().join(".parton/map.lock").exists());
    }

    #[test]
    fn load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ts"), "const x = 1;\n").unwrap();
        let lock = build_lock(&["a.ts".into()], dir.path(), "abc123");
        save_lock(&lock, dir.path()).unwrap();
        let loaded = load_lock(dir.path()).unwrap();
        assert_eq!(loaded.git_sha, "abc123");
        assert_eq!(loaded.files.len(), 1);
        assert_eq!(loaded.files["a.ts"].hash, lock.files["a.ts"].hash);
    }

    #[test]
    fn diff_detects_changes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ts"), "v1").unwrap();
        let lock = build_lock(&["a.ts".into()], dir.path(), "abc");

        // Change file.
        std::fs::write(dir.path().join("a.ts"), "v2").unwrap();
        let diff = diff_lock(&lock, &["a.ts".into()], dir.path());
        assert_eq!(diff.changed, vec!["a.ts"]);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn diff_detects_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let lock = MapLock {
            git_sha: "abc".into(),
            files: HashMap::new(),
        };
        std::fs::write(dir.path().join("new.ts"), "x").unwrap();
        let diff = diff_lock(&lock, &["new.ts".into()], dir.path());
        assert_eq!(diff.changed, vec!["new.ts"]);
    }

    #[test]
    fn diff_detects_removed_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = HashMap::new();
        files.insert(
            "old.ts".into(),
            FileState {
                lines: 1,
                hash: "x".into(),
            },
        );
        let lock = MapLock {
            git_sha: "abc".into(),
            files,
        };
        let diff = diff_lock(&lock, &[], dir.path());
        assert_eq!(diff.removed, vec!["old.ts"]);
    }

    #[test]
    fn diff_unchanged_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ts"), "same").unwrap();
        let lock = build_lock(&["a.ts".into()], dir.path(), "abc");
        let diff = diff_lock(&lock, &["a.ts".into()], dir.path());
        assert!(diff.changed.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn quick_hash_deterministic() {
        assert_eq!(quick_hash("hello"), quick_hash("hello"));
        assert_ne!(quick_hash("hello"), quick_hash("world"));
    }
}
