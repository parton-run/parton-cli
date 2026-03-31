#![deny(warnings)]

//! Turbo planner for Parton.
//!
//! Generates JSON execution plans via LLM, defining which files to create/edit,
//! their exports, imports, and conventions.
//!
//! The planner produces a [`RunPlan`](parton_core::RunPlan) that the executor consumes.

pub mod clarify;
pub mod context;
pub mod parse;
pub mod prompt;
pub mod skeleton;
pub mod validate;

pub use clarify::{generate_questions, generate_questions_with_tools};
pub use context::build_project_context;
pub use parse::{parse_plan, ParseError};
pub use prompt::SYSTEM_PROMPT;
pub use skeleton::{enrich_plan, generate_skeleton, generate_skeleton_with_tools};
pub use validate::{validate_plan, ValidationError};
