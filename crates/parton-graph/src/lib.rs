#![deny(warnings)]

//! SQLite-backed graph store for Parton.
//!
//! Stores code relationships (files, symbols, imports/exports)
//! for context-aware code generation.

pub mod error;
pub mod schema;
pub mod store;
pub mod types;

pub use error::GraphError;
pub use store::GraphStore;
pub use types::{FileId, RelationshipKind, Symbol, SymbolId, SymbolKind};
