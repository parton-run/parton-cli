//! CLI-based provider that delegates to external tools (claude, codex).

use std::process::{Command, Stdio};

use async_trait::async_trait;

use parton_core::{ModelProvider, ModelResponse, ProviderError};

/// A provider that delegates to a CLI tool (e.g. `claude --print`).
#[derive(Debug)]
pub struct CliProvider {
    command: String,
    model: Option<String>,
}

impl CliProvider {
    /// Create a new CLI provider.
    ///
    /// - `command`: CLI binary name (e.g. `claude`, `codex`)
    /// - `model`: optional model override
    pub fn new(command: String, model: Option<String>) -> Self {
        Self { command, model }
    }
}

#[async_trait]
impl ModelProvider for CliProvider {
    async fn send(
        &self,
        system: &str,
        prompt: &str,
        _stream: bool,
    ) -> Result<ModelResponse, ProviderError> {
        let command = self.command.clone();
        let model = self.model.clone();
        let full_prompt = if system.is_empty() {
            prompt.to_string()
        } else {
            format!("{system}\n\n{prompt}")
        };

        tokio::task::spawn_blocking(move || {
            send_sync(&command, model.as_deref(), &full_prompt)
        })
        .await
        .map_err(|e| ProviderError::Other(format!("spawn_blocking failed: {e}")))?
    }
}

/// Synchronous CLI execution — runs in a blocking thread for true parallelism.
fn send_sync(
    command: &str,
    model: Option<&str>,
    prompt: &str,
) -> Result<ModelResponse, ProviderError> {
    let mut cmd = Command::new(command);

    configure_cli_args(&mut cmd, command, model);

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        ProviderError::Other(format!("failed to execute '{command}': {e}"))
    })?;

    // Write prompt to stdin.
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(prompt.as_bytes());
    }

    let output = child.wait_with_output().map_err(|e| {
        ProviderError::Other(format!("failed to read output from '{command}': {e}"))
    })?;

    let content = parse_output(command, &output.stdout)?;

    // Estimate tokens from content length (no usage info from CLI).
    let prompt_tokens = (prompt.len() / 4) as u32;
    let completion_tokens = (content.len() / 4) as u32;

    Ok(ModelResponse {
        content,
        prompt_tokens,
        completion_tokens,
    })
}

/// Set up CLI-specific arguments.
fn configure_cli_args(cmd: &mut Command, command: &str, model: Option<&str>) {
    match command {
        "claude" => {
            cmd.args([
                "--print",
                "--dangerously-skip-permissions",
                "--output-format",
                "json",
            ]);
        }
        "codex" => {
            let temp_dir = std::env::temp_dir().join("parton-codex-sandbox");
            let _ = std::fs::create_dir_all(&temp_dir);
            cmd.args([
                "exec",
                "--json",
                "--ephemeral",
                "--skip-git-repo-check",
                "-C",
            ]);
            cmd.arg(temp_dir.to_string_lossy().as_ref());
            cmd.arg("-");
        }
        _ => {}
    }

    if let Some(m) = model {
        let flag = if command == "codex" { "-m" } else { "--model" };
        cmd.args([flag, m]);
    }
}

/// Parse raw CLI output into text content.
fn parse_output(command: &str, stdout: &[u8]) -> Result<String, ProviderError> {
    let raw = String::from_utf8_lossy(stdout).trim().to_string();

    if raw.is_empty() {
        return Err(ProviderError::Other(format!(
            "'{command}' returned empty output"
        )));
    }

    // Claude with --output-format json wraps response in a JSON envelope.
    if command == "claude" {
        if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(result) = envelope.get("result").and_then(|r| r.as_str()) {
                return Ok(result.to_string());
            }
        }
    }

    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_output_claude_json() {
        let json = r#"{"result":"hello world","usage":{"input_tokens":10}}"#;
        let result = parse_output("claude", json.as_bytes()).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn parse_output_plain_text() {
        let result = parse_output("some-cli", b"plain output").unwrap();
        assert_eq!(result, "plain output");
    }

    #[test]
    fn parse_output_empty_errors() {
        let result = parse_output("claude", b"");
        assert!(result.is_err());
    }

    #[test]
    fn configure_claude_args() {
        let mut cmd = Command::new("echo");
        configure_cli_args(&mut cmd, "claude", Some("sonnet"));
        // Can't easily inspect args, but verify it doesn't panic.
    }
}
