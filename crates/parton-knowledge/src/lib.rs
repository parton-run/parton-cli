#![deny(warnings)]

//! Local knowledge store for Parton.
//!
//! Stores and retrieves project-specific learnings, conventions,
//! and patterns extracted from previous runs.

pub mod entry;
pub mod error;
pub mod init;
pub mod learning;
pub mod store;

pub use entry::{Category, Entry, Source};
pub use error::KnowledgeError;
pub use init::auto_init;
pub use learning::{build_knowledge_context, extract_and_store};
pub use store::{KnowledgeStore, LocalStore};
