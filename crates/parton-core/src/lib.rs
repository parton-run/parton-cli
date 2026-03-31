#![deny(warnings)]

//! Core types and contracts for the Parton CLI.
//!
//! This crate defines shared types used across the workspace:
//! execution plans, results, configuration, and the provider trait.

pub mod clarification;
pub mod config;
pub mod error;
pub mod plan;
pub mod provider;
pub mod result;
pub mod stage;
pub mod tool;

// Re-exports for convenience.
pub use clarification::{ClarificationResult, PlanningContext, Question, QuestionType};
pub use config::{AccessType, ModelConfig, ModelsSection, PartonConfig};
pub use error::CoreError;
pub use plan::{FileAction, FilePlan, ImportRef, RunPlan};
pub use provider::{ModelProvider, ModelResponse, ProviderError};
pub use result::{FileResult, PostRunAction, RunResult};
pub use stage::StageKind;
pub use tool::{ToolCall, ToolDefinition, ToolResult};
