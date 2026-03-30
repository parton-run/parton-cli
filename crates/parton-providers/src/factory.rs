//! Provider factory — creates the right provider from configuration.

use parton_core::{AccessType, ModelConfig, ModelProvider, ProviderError, StageKind};

use crate::cli::CliProvider;
use crate::ollama::OllamaProvider;
use crate::openai::OpenAiProvider;

/// Create a provider from a [`ModelConfig`].
///
/// Resolves access type, reads API keys from environment, and returns
/// the appropriate provider implementation.
pub fn create_provider(config: &ModelConfig) -> Result<Box<dyn ModelProvider>, ProviderError> {
    match config.access {
        AccessType::Cli => {
            let command = config.command.as_deref().unwrap_or("claude");
            let model = if config.model.is_empty() {
                None
            } else {
                Some(config.model.clone())
            };
            Ok(Box::new(CliProvider::new(command.to_string(), model)))
        }
        AccessType::Api => create_api_provider(config),
    }
}

/// Resolve the model config for a specific stage.
///
/// Resolution order:
/// 1. Stage-specific config (e.g. `models.execution`)
/// 2. Default config (`models.default`)
/// 3. Returns `None` if neither is set.
pub fn resolve_stage_config(
    stage: StageKind,
    models: &parton_core::ModelsSection,
) -> Option<&ModelConfig> {
    let stage_specific = match stage {
        StageKind::Context => models.context.as_ref(),
        StageKind::Planning => models.planning.as_ref(),
        StageKind::Execution => models.execution.as_ref(),
        StageKind::Judge => models.judge.as_ref(),
    };
    stage_specific.or(models.default.as_ref())
}

/// Create a provider for a specific pipeline stage.
///
/// Uses stage-specific config if available, falls back to default.
pub fn create_stage_provider(
    stage: StageKind,
    models: &parton_core::ModelsSection,
) -> Result<Box<dyn ModelProvider>, ProviderError> {
    let config = resolve_stage_config(stage, models).ok_or_else(|| {
        ProviderError::InvalidConfig(format!(
            "no model configured for stage '{stage}' and no default set"
        ))
    })?;
    create_provider(config)
}

/// Create an API-based provider from config.
fn create_api_provider(config: &ModelConfig) -> Result<Box<dyn ModelProvider>, ProviderError> {
    let model = if config.model.is_empty() {
        None
    } else {
        Some(config.model.clone())
    };

    match config.provider.as_str() {
        "openai" => {
            let provider = OpenAiProvider::from_env(model, config.env_key.as_deref())?;
            Ok(Box::new(provider))
        }
        "ollama" => Ok(Box::new(OllamaProvider::new(model))),
        "" => Err(ProviderError::InvalidConfig(
            "provider name is empty".into(),
        )),
        other => Err(ProviderError::InvalidConfig(format!(
            "unknown provider '{other}'. supported: openai, ollama"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parton_core::ModelsSection;

    #[test]
    fn resolve_stage_config_specific() {
        let models = ModelsSection {
            execution: Some(ModelConfig {
                provider: "ollama".into(),
                model: "llama3:8b".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let config = resolve_stage_config(StageKind::Execution, &models);
        assert!(config.is_some());
        assert_eq!(config.unwrap().provider, "ollama");
    }

    #[test]
    fn resolve_stage_config_fallback_to_default() {
        let models = ModelsSection {
            default: Some(ModelConfig {
                provider: "openai".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let config = resolve_stage_config(StageKind::Planning, &models);
        assert!(config.is_some());
        assert_eq!(config.unwrap().provider, "openai");
    }

    #[test]
    fn resolve_stage_config_none() {
        let models = ModelsSection::default();
        let config = resolve_stage_config(StageKind::Judge, &models);
        assert!(config.is_none());
    }

    #[test]
    fn create_provider_cli() {
        let config = ModelConfig {
            access: AccessType::Cli,
            command: Some("claude".into()),
            model: "sonnet".into(),
            ..Default::default()
        };
        let provider = create_provider(&config);
        assert!(provider.is_ok());
    }

    #[test]
    fn create_provider_unknown_api() {
        let config = ModelConfig {
            access: AccessType::Api,
            provider: "unknown".into(),
            ..Default::default()
        };
        let result = create_provider(&config);
        assert!(result.is_err());
    }
}
