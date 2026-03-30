//! Configuration types parsed from `parton.toml`.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// How a model is accessed.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessType {
    /// Access via local CLI binary (e.g. `claude`, `codex`).
    Cli,
    /// Access via remote API using an API key.
    #[default]
    Api,
}

/// Per-stage model configuration.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// How this model is accessed: `cli` or `api`.
    #[serde(default)]
    pub access: AccessType,
    /// CLI command to invoke (e.g. `claude`). Used when `access = cli`.
    #[serde(default)]
    pub command: Option<String>,
    /// Provider name (e.g. `anthropic`, `openai`, `ollama`).
    #[serde(default)]
    pub provider: String,
    /// Model identifier (e.g. `claude-sonnet-4-6`, `gpt-4o-mini`).
    #[serde(default)]
    pub model: String,
    /// Environment variable holding the API key.
    /// Allows using a custom var (e.g. `PARTON_OPENAI_KEY`) to avoid
    /// collisions with other tools.
    #[serde(default)]
    pub env_key: Option<String>,
}

/// Per-stage model overrides.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ModelsSection {
    /// Default model for all stages.
    pub default: Option<ModelConfig>,
    /// Override for context stage.
    pub context: Option<ModelConfig>,
    /// Override for planning stage.
    pub planning: Option<ModelConfig>,
    /// Override for execution stage.
    pub execution: Option<ModelConfig>,
    /// Override for judge stage.
    pub judge: Option<ModelConfig>,
}

/// Execution settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSection {
    /// Branch name prefix for execution runs.
    #[serde(default = "default_branch_prefix")]
    pub branch_prefix: String,
    /// Commands to run for validation after each phase.
    #[serde(default)]
    pub validation: Vec<String>,
    /// Maximum number of steps per run.
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    /// Token budget per step for context building.
    #[serde(default = "default_step_budget")]
    pub step_budget: usize,
}

impl Default for ExecutionSection {
    fn default() -> Self {
        Self {
            branch_prefix: default_branch_prefix(),
            validation: Vec::new(),
            max_steps: default_max_steps(),
            step_budget: default_step_budget(),
        }
    }
}

/// Project metadata.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProjectSection {
    /// Project name.
    #[serde(default)]
    pub name: String,
}

/// Top-level Parton configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartonConfig {
    /// Config format version.
    #[serde(default = "default_config_version")]
    pub config_version: String,
    /// Project metadata.
    #[serde(default)]
    pub project: ProjectSection,
    /// Execution settings.
    #[serde(default)]
    pub execution: ExecutionSection,
    /// Per-stage model configuration.
    #[serde(default)]
    pub models: ModelsSection,
}

impl Default for PartonConfig {
    fn default() -> Self {
        Self {
            config_version: default_config_version(),
            project: ProjectSection::default(),
            execution: ExecutionSection::default(),
            models: ModelsSection::default(),
        }
    }
}

impl PartonConfig {
    /// Load config from a TOML string.
    pub fn from_toml(content: &str) -> Result<Self, CoreError> {
        toml::from_str(content).map_err(|e| CoreError::Parse(e.to_string()))
    }

    /// Load config from `parton.toml` in the given directory.
    ///
    /// Returns default config if no file exists.
    pub fn load(project_root: &std::path::Path) -> Result<Self, CoreError> {
        let path = project_root.join("parton.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Self::from_toml(&content)
    }

    /// Save config to `parton.toml` in the given directory.
    pub fn save(&self, project_root: &std::path::Path) -> Result<(), CoreError> {
        let path = project_root.join("parton.toml");
        let content = toml::to_string_pretty(self)
            .map_err(|e| CoreError::Parse(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

fn default_config_version() -> String {
    "0.1.0".into()
}

fn default_branch_prefix() -> String {
    "parton".into()
}

fn default_max_steps() -> usize {
    10
}

fn default_step_budget() -> usize {
    6000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_type_defaults_to_api() {
        let access = AccessType::default();
        assert_eq!(access, AccessType::Api);
    }

    #[test]
    fn access_type_serde() {
        let json = serde_json::to_string(&AccessType::Cli).unwrap();
        assert_eq!(json, r#""cli""#);
        let parsed: AccessType = serde_json::from_str(r#""api""#).unwrap();
        assert_eq!(parsed, AccessType::Api);
    }

    #[test]
    fn model_config_with_env_key() {
        let config = ModelConfig {
            access: AccessType::Api,
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            env_key: Some("PARTON_OPENAI_KEY".into()),
            ..Default::default()
        };
        assert_eq!(config.env_key.as_deref(), Some("PARTON_OPENAI_KEY"));
    }

    #[test]
    fn parton_config_defaults() {
        let config = PartonConfig::default();
        assert_eq!(config.config_version, "0.1.0");
        assert_eq!(config.execution.branch_prefix, "parton");
        assert_eq!(config.execution.max_steps, 10);
        assert!(config.models.default.is_none());
    }

    #[test]
    fn parton_config_from_toml() {
        let toml = r#"
            config_version = "0.1.0"

            [project]
            name = "my-app"

            [models.execution]
            provider = "ollama"
            model = "qwen2.5-coder:7b"
            access = "api"
        "#;
        let config = PartonConfig::from_toml(toml).unwrap();
        assert_eq!(config.project.name, "my-app");
        let exec = config.models.execution.unwrap();
        assert_eq!(exec.provider, "ollama");
        assert_eq!(exec.access, AccessType::Api);
    }

    #[test]
    fn models_section_stage_overrides() {
        let toml = r#"
            [models.default]
            access = "cli"
            command = "claude"
            model = "claude-sonnet-4-6"

            [models.execution]
            provider = "openai"
            model = "gpt-4o-mini"
            access = "api"
            env_key = "PARTON_OPENAI_KEY"
        "#;
        let config: PartonConfig = toml::from_str(toml).unwrap();
        assert!(config.models.default.is_some());
        assert!(config.models.execution.is_some());
        assert!(config.models.planning.is_none());
    }
}
