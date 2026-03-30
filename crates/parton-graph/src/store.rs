//! SQLite-backed graph store.

use std::path::Path;

use rusqlite::Connection;

use crate::error::GraphError;
use crate::types::{FileId, RelationshipKind, Symbol, SymbolId, SymbolKind};

/// SQLite-backed code graph store.
#[derive(Debug)]
pub struct GraphStore {
    conn: Connection,
}

impl GraphStore {
    /// Open (or create) a graph database at the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, GraphError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        crate::schema::initialize(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory graph database (for testing).
    pub fn open_in_memory() -> Result<Self, GraphError> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        crate::schema::initialize(&conn)?;
        Ok(Self { conn })
    }

    /// Insert a file into the graph. Returns its ID.
    pub fn insert_file(&self, path: &str, content_hash: &str) -> Result<FileId, GraphError> {
        self.conn.execute(
            "INSERT INTO files (path, content_hash) VALUES (?1, ?2)",
            [path, content_hash],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get a file by path. Returns `(id, path, content_hash)`.
    pub fn get_file_by_path(&self, path: &str) -> Result<Option<(FileId, String)>, GraphError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, content_hash FROM files WHERE path = ?1")?;
        let result = stmt
            .query_row([path], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()?;
        Ok(result)
    }

    /// Get all files.
    pub fn get_all_files(&self) -> Result<Vec<(FileId, String, String)>, GraphError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, content_hash FROM files")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(GraphError::from)
    }

    /// Insert a symbol. Returns its ID.
    pub fn insert_symbol(
        &self,
        name: &str,
        kind: &SymbolKind,
        file_id: FileId,
        line_start: u32,
        line_end: u32,
        content_hash: &str,
    ) -> Result<SymbolId, GraphError> {
        self.conn.execute(
            "INSERT INTO symbols (name, kind, file_id, line_start, line_end, content_hash) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                name,
                kind.to_string(),
                file_id,
                line_start,
                line_end,
                content_hash,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get all symbols in a file.
    pub fn get_symbols_by_file(&self, file_id: FileId) -> Result<Vec<Symbol>, GraphError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, kind, file_id, line_start, line_end FROM symbols WHERE file_id = ?1",
        )?;
        let rows = stmt.query_map([file_id], |row| {
            Ok(Symbol {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: parse_symbol_kind(&row.get::<_, String>(2)?),
                file_id: row.get(3)?,
                line_start: row.get(4)?,
                line_end: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(GraphError::from)
    }

    /// Insert a relationship between two symbols.
    pub fn insert_relationship(
        &self,
        source: SymbolId,
        target: SymbolId,
        kind: &RelationshipKind,
    ) -> Result<i64, GraphError> {
        self.conn.execute(
            "INSERT INTO relationships (source_id, target_id, kind) VALUES (?1, ?2, ?3)",
            rusqlite::params![source, target, kind.to_string()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get outgoing relationships from a symbol.
    pub fn get_relationships_from(
        &self,
        symbol_id: SymbolId,
    ) -> Result<Vec<(SymbolId, SymbolId, String)>, GraphError> {
        let mut stmt = self
            .conn
            .prepare("SELECT source_id, target_id, kind FROM relationships WHERE source_id = ?1")?;
        let rows = stmt.query_map([symbol_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(GraphError::from)
    }
}

/// Parse a symbol kind string back to enum.
fn parse_symbol_kind(s: &str) -> SymbolKind {
    match s {
        "function" => SymbolKind::Function,
        "class" => SymbolKind::Class,
        "type" => SymbolKind::Type,
        "variable" => SymbolKind::Variable,
        "enum" => SymbolKind::Enum,
        "module" => SymbolKind::Module,
        _ => SymbolKind::Variable,
    }
}

/// Extension trait for optional query results.
trait OptionalExt<T> {
    /// Convert a "no rows" error to `None`.
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory() {
        let store = GraphStore::open_in_memory().unwrap();
        let files = store.get_all_files().unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn insert_and_get_file() {
        let store = GraphStore::open_in_memory().unwrap();
        let id = store.insert_file("src/app.ts", "abc123").unwrap();
        assert!(id > 0);

        let result = store.get_file_by_path("src/app.ts").unwrap();
        assert!(result.is_some());
        let (fid, hash) = result.unwrap();
        assert_eq!(fid, id);
        assert_eq!(hash, "abc123");
    }

    #[test]
    fn get_file_not_found() {
        let store = GraphStore::open_in_memory().unwrap();
        let result = store.get_file_by_path("missing.ts").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn insert_and_get_symbols() {
        let store = GraphStore::open_in_memory().unwrap();
        let file_id = store.insert_file("src/lib.ts", "hash").unwrap();
        let sym_id = store
            .insert_symbol("MyFunc", &SymbolKind::Function, file_id, 1, 10, "shash")
            .unwrap();
        assert!(sym_id > 0);

        let symbols = store.get_symbols_by_file(file_id).unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyFunc");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn insert_and_get_relationships() {
        let store = GraphStore::open_in_memory().unwrap();
        let fid = store.insert_file("a.ts", "h").unwrap();
        let s1 = store
            .insert_symbol("A", &SymbolKind::Function, fid, 1, 5, "h1")
            .unwrap();
        let s2 = store
            .insert_symbol("B", &SymbolKind::Function, fid, 6, 10, "h2")
            .unwrap();
        store
            .insert_relationship(s1, s2, &RelationshipKind::Calls)
            .unwrap();

        let rels = store.get_relationships_from(s1).unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].2, "calls");
    }

    #[test]
    fn get_all_files() {
        let store = GraphStore::open_in_memory().unwrap();
        store.insert_file("a.ts", "h1").unwrap();
        store.insert_file("b.ts", "h2").unwrap();
        let files = store.get_all_files().unwrap();
        assert_eq!(files.len(), 2);
    }
}
