#![deny(warnings)]

//! Parallel file execution engine for Parton.
//!
//! Supports three execution modes:
//! - **Scaffold** — minimal compilable stubs (fast, validates structure)
//! - **Full** — complete implementation in one pass
//! - **Final** — full implementation preserving existing scaffold structure

pub mod compliance;
pub mod output;
pub mod prompt;
pub mod runner;
pub mod scaffold;
pub mod structure_check;
pub mod writer;

pub use compliance::{check_all, check_file, ComplianceIssue, IssueType};
pub use output::clean_output;
pub use prompt::{build_file_prompt, SYSTEM_PROMPT};
pub use runner::{execute, execute_streaming, scaffold_streaming, ExecMode, ScaffoldResult};
pub use structure_check::{run_check, run_install, CheckResult};
pub use writer::write_results;
