//! Setup command — launches the ratatui-based setup wizard.

use std::path::Path;

use anyhow::Result;
use parton_core::{AccessType, ModelConfig, PartonConfig};

use crate::tui::{setup_wizard, style};

/// A detected provider + model combination.
#[derive(Clone)]
struct ModelOption {
    label: String,
    access: AccessType,
    command: Option<String>,
    provider: String,
    model: String,
}

/// Run the setup wizard for a project directory.
pub fn run_setup(project_root: &Path) -> Result<PartonConfig> {
    let mut config = PartonConfig::default();
    config.project.name = detect_project_name(project_root);
    config.execution.validation = detect_validation(project_root);

    // Build model options.
    let options = build_model_options();
    if options.is_empty() {
        style::print_err("No AI providers found.");
        style::print_err("Install claude or codex CLI, set OPENAI_API_KEY, or start ollama.");
        anyhow::bail!("no providers available");
    }

    let labels: Vec<String> = options.iter().map(|o| o.label.clone()).collect();

    // Launch ratatui wizard.
    let result = setup_wizard::run_wizard(labels)?;

    // Map wizard result to config.
    config.models.default = Some(to_config(&options[result.default]));
    config.models.planning = result.planning.map(|i| to_config(&options[i]));
    config.models.execution = result.execution.map(|i| to_config(&options[i]));
    config.models.judge = result.judge.map(|i| to_config(&options[i]));

    config.save(project_root)?;

    // Print summary after terminal is restored.
    eprintln!();
    style::print_header("Configuration saved");
    style::print_kv("Default", &options[result.default].label);
    if let Some(i) = result.planning {
        style::print_kv("Planning", &options[i].label);
    }
    if let Some(i) = result.execution {
        style::print_kv("Execution", &options[i].label);
    }
    if let Some(i) = result.judge {
        style::print_kv("Judge", &options[i].label);
    }
    style::print_ok("Saved to parton.toml");
    eprintln!();

    Ok(config)
}

fn to_config(opt: &ModelOption) -> ModelConfig {
    ModelConfig {
        access: opt.access.clone(),
        command: opt.command.clone(),
        provider: opt.provider.clone(),
        model: opt.model.clone(),
        env_key: None,
    }
}

/// Build all available model options.
fn build_model_options() -> Vec<ModelOption> {
    let mut options = Vec::new();

    if which("claude") {
        for model in &["claude-sonnet-4-6", "claude-opus-4-6", "claude-haiku-4-5"] {
            options.push(ModelOption {
                label: format!("Claude / {model}"),
                access: AccessType::Cli,
                command: Some("claude".into()),
                provider: "claude".into(),
                model: model.to_string(),
            });
        }
    }

    if which("codex") {
        for model in &["gpt-5.4", "gpt-5.4-mini", "o3", "o4-mini"] {
            options.push(ModelOption {
                label: format!("Codex / {model}"),
                access: AccessType::Cli,
                command: Some("codex".into()),
                provider: "codex".into(),
                model: model.to_string(),
            });
        }
    }

    if has_env("OPENAI_API_KEY") || has_env("PARTON_OPENAI_KEY") {
        for model in &[
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-4o",
            "gpt-4o-mini",
            "o3",
            "o4-mini",
        ] {
            options.push(ModelOption {
                label: format!("OpenAI API / {model}"),
                access: AccessType::Api,
                command: None,
                provider: "openai".into(),
                model: model.to_string(),
            });
        }
    }

    if let Some(models) = detect_ollama_models() {
        for model in models {
            options.push(ModelOption {
                label: format!("Ollama / {model}"),
                access: AccessType::Api,
                command: None,
                provider: "ollama".into(),
                model,
            });
        }
    }

    options
}

fn detect_ollama_models() -> Option<Vec<String>> {
    let output = std::process::Command::new("ollama")
        .arg("list")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let models: Vec<String> = text
        .lines()
        .skip(1)
        .filter_map(|l| l.split_whitespace().next())
        .map(String::from)
        .collect();
    if models.is_empty() {
        None
    } else {
        Some(models)
    }
}

fn has_env(name: &str) -> bool {
    std::env::var(name)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn detect_project_name(root: &Path) -> String {
    let pkg = root.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                return name.to_string();
            }
        }
    }
    root.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string()
}

fn detect_validation(root: &Path) -> Vec<String> {
    if root.join("Cargo.toml").exists() {
        return vec!["cargo check".into(), "cargo test".into()];
    }
    if root.join("package.json").exists() {
        let mut cmds = vec![];
        if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) {
                    if scripts.contains_key("build") {
                        cmds.push("npm run build".into());
                    }
                    if scripts.contains_key("test") {
                        cmds.push("npm test".into());
                    }
                }
            }
        }
        if cmds.is_empty() {
            cmds.push("npx tsc --noEmit".into());
        }
        return cmds;
    }
    if root.join("go.mod").exists() {
        return vec!["go build ./...".into(), "go test ./...".into()];
    }
    vec![]
}
