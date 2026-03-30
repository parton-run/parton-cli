#![deny(warnings)]

//! LLM provider implementations for Parton.
//!
//! Supports multiple providers:
//! - **OpenAI API** — GPT-4o, GPT-5.x, etc.
//! - **Ollama** — local LLM inference
//! - **CLI wrappers** — claude, codex, or any CLI tool
//!
//! Use [`create_provider`] to build a provider from config,
//! or [`create_stage_provider`] to resolve per-stage overrides.

pub mod cli;
pub mod factory;
pub mod ollama;
pub mod openai;

pub use cli::CliProvider;
pub use factory::{create_provider, create_stage_provider, resolve_stage_config};
pub use ollama::OllamaProvider;
pub use openai::{OpenAiConfig, OpenAiProvider};
