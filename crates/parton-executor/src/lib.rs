#![deny(warnings)]

//! Parallel file execution engine for Parton.
//!
//! Takes a [`RunPlan`](parton_core::RunPlan) and executes all files in parallel
//! via LLM providers. Includes compliance checking, output parsing, and file writing.

pub mod compliance;
pub mod output;
pub mod prompt;
pub mod runner;
pub mod writer;

pub use compliance::{check_all, check_file, ComplianceIssue, IssueType};
pub use output::clean_output;
pub use prompt::{build_file_prompt, SYSTEM_PROMPT};
pub use runner::{execute, execute_streaming};
pub use writer::write_results;
