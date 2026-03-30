//! Helper functions for the `parton run` pipeline.

use std::path::Path;

use anyhow::{Context, Result};
use parton_core::{ModelProvider, PartonConfig, StageKind};

use crate::tui::{plan_review, style};

/// Format review comments for re-planning prompt.
pub fn format_comments(comments: &[plan_review::ReviewComment]) -> String {
    comments
        .iter()
        .map(|c| match &c.file {
            Some(path) => format!("- [{}] {}", path, c.text),
            None => format!("- [GENERAL] {}", c.text),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Load config or launch interactive setup if unconfigured.
pub fn load_or_setup(project_root: &Path) -> Result<PartonConfig> {
    let config = PartonConfig::load(project_root).context("failed to load parton.toml")?;
    if config.models.default.is_none() {
        eprintln!("  No provider configured. Starting setup...\n");
        return crate::commands::setup::run_setup(project_root).context("setup failed");
    }
    Ok(config)
}

/// Create a provider for a given pipeline stage.
pub fn create_provider(stage: StageKind, config: &PartonConfig) -> Result<Box<dyn ModelProvider>> {
    parton_providers::create_stage_provider(stage, &config.models)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Determine if a file needs final execution.
///
/// Respects the planner's `scaffold_only` flag — the planner decides
/// which files are config/static (scaffold is final) vs logic (needs
/// implementation).
pub fn needs_final_execution(file: &parton_core::FilePlan) -> bool {
    !file.scaffold_only
}

/// Detect "command not found" errors (exit code 127).
pub fn is_missing_tool_error(errors: &str) -> bool {
    errors.contains("command not found") || errors.contains("not found in PATH")
}

/// Check if the project directory has no recognizable project files.
pub fn is_greenfield_project(root: &Path) -> bool {
    !root.join("package.json").exists()
        && !root.join("Cargo.toml").exists()
        && !root.join("go.mod").exists()
        && !root.join("pyproject.toml").exists()
}

/// Run validation commands and print per-command results.
pub fn run_validation_check(
    commands: &[String],
    project_root: &Path,
) -> parton_executor::CheckResult {
    let result = parton_executor::run_check(commands, project_root);
    for cmd in commands {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(project_root)
            .env("CI", "true")
            .output();
        match output {
            Ok(o) if o.status.success() => style::print_ok(cmd),
            Ok(o) => {
                style::print_err(&format!("{cmd} (exit {})", o.status.code().unwrap_or(-1)));
                let stderr = String::from_utf8_lossy(&o.stderr);
                for line in stderr.lines().take(5) {
                    eprintln!("    {}", style::dim(line));
                }
            }
            Err(e) => style::print_err(&format!("{cmd}: {e}")),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tool_error_detected() {
        assert!(is_missing_tool_error("sh: go: command not found"));
        assert!(is_missing_tool_error("cargo: not found in PATH"));
    }

    #[test]
    fn normal_error_not_flagged() {
        assert!(!is_missing_tool_error("error[E0308]: mismatched types"));
        assert!(!is_missing_tool_error("FAIL src/app.test.ts"));
    }

    #[test]
    fn greenfield_detects_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_greenfield_project(dir.path()));
    }

    #[test]
    fn greenfield_detects_cargo_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert!(!is_greenfield_project(dir.path()));
    }
}
