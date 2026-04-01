//! CLI-based provider that delegates to external tools (claude, codex).

pub mod tool_use;

use std::process::{Command, Stdio};

use async_trait::async_trait;

use parton_core::{
    ModelProvider, ModelResponse, ProviderError, ToolCall, ToolDefinition, ToolResult,
};

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
        _json_mode: bool,
    ) -> Result<ModelResponse, ProviderError> {
        let command = self.command.clone();
        let model = self.model.clone();
        let full_prompt = if system.is_empty() {
            prompt.to_string()
        } else {
            format!("{system}\n\n{prompt}")
        };

        tokio::task::spawn_blocking(move || send_sync(&command, model.as_deref(), &full_prompt))
            .await
            .map_err(|e| ProviderError::Other(format!("spawn_blocking failed: {e}")))?
    }

    async fn send_with_tools(
        &self,
        system: &str,
        prompt: &str,
        tools: &[ToolDefinition],
        max_turns: usize,
        handle_tool: &(dyn Fn(ToolCall) -> ToolResult + Send + Sync),
    ) -> Result<ModelResponse, ProviderError> {
        if tools.is_empty() {
            return self.send(system, prompt, true).await;
        }

        let command = self.command.clone();
        let model = self.model.clone();

        tokio::task::block_in_place(|| {
            let send_fn = |sys: &str, p: &str| -> Result<ModelResponse, ProviderError> {
                send_sync_with_system(&command, model.as_deref(), sys, p)
            };
            tool_use::run_tool_loop(&send_fn, system, prompt, tools, max_turns, handle_tool)
        })
    }
}

/// Synchronous CLI execution — sends combined system+prompt via stdin.
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

    let mut child = cmd
        .spawn()
        .map_err(|e| ProviderError::Other(format!("failed to execute '{command}': {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(prompt.as_bytes());
    }

    let output = child.wait_with_output().map_err(|e| {
        ProviderError::Other(format!("failed to read output from '{command}': {e}"))
    })?;

    let parsed = parse_output(command, &output.stdout)?;
    let prompt_tokens = parsed.prompt_tokens.unwrap_or((prompt.len() / 4) as u32);
    let completion_tokens = parsed
        .completion_tokens
        .unwrap_or((parsed.content.len() / 4) as u32);

    Ok(ModelResponse {
        content: parsed.content,
        prompt_tokens,
        completion_tokens,
    })
}

/// Synchronous CLI execution with separate system prompt.
///
/// Uses `--system-prompt` flag for CLI tools that support it (claude).
/// This avoids prompt injection detection that triggers when system
/// instructions arrive via stdin.
fn send_sync_with_system(
    command: &str,
    model: Option<&str>,
    system: &str,
    prompt: &str,
) -> Result<ModelResponse, ProviderError> {
    let mut cmd = Command::new(command);
    configure_cli_args(&mut cmd, command, model);

    // Pass system prompt via --system-prompt flag for claude.
    if !system.is_empty() && command == "claude" {
        cmd.args(["--system-prompt", system]);
    }

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let full_prompt = if system.is_empty() || command == "claude" {
        prompt.to_string()
    } else {
        format!("{system}\n\n{prompt}")
    };

    let mut child = cmd
        .spawn()
        .map_err(|e| ProviderError::Other(format!("failed to execute '{command}': {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(full_prompt.as_bytes());
    }

    let output = child.wait_with_output().map_err(|e| {
        ProviderError::Other(format!("failed to read output from '{command}': {e}"))
    })?;

    let parsed = parse_output(command, &output.stdout)?;
    let prompt_tokens = parsed
        .prompt_tokens
        .unwrap_or(((system.len() + prompt.len()) / 4) as u32);
    let completion_tokens = parsed
        .completion_tokens
        .unwrap_or((parsed.content.len() / 4) as u32);

    Ok(ModelResponse {
        content: parsed.content,
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
                "--tools",
                "",
                "--no-session-persistence",
            ]);
        }
        "codex" => {
            cmd.args([
                "exec",
                "--json",
                "--ephemeral",
                "--skip-git-repo-check",
                "--full-auto",
                "-",
            ]);
        }
        _ => {}
    }

    if let Some(m) = model {
        let flag = if command == "codex" { "-m" } else { "--model" };
        cmd.args([flag, m]);
    }
}

/// Parsed CLI output with content and optional token counts.
struct CliOutput {
    content: String,
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

/// Parse raw CLI output into text content.
fn parse_output(command: &str, stdout: &[u8]) -> Result<CliOutput, ProviderError> {
    let raw = String::from_utf8_lossy(stdout).trim().to_string();
    if raw.is_empty() {
        return Err(ProviderError::Other(format!(
            "'{command}' returned empty output"
        )));
    }

    match command {
        "claude" => parse_claude_output(&raw),
        "codex" => parse_codex_output(&raw),
        _ => Ok(CliOutput {
            content: raw,
            prompt_tokens: None,
            completion_tokens: None,
        }),
    }
}

/// Parse claude --output-format json envelope.
fn parse_claude_output(raw: &str) -> Result<CliOutput, ProviderError> {
    if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(raw) {
        let content = envelope
            .get("result")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();
        if content.is_empty() {
            return Err(ProviderError::Other("claude returned empty result".into()));
        }
        return Ok(CliOutput {
            content,
            prompt_tokens: None,
            completion_tokens: None,
        });
    }
    Ok(CliOutput {
        content: raw.to_string(),
        prompt_tokens: None,
        completion_tokens: None,
    })
}

/// Parse codex NDJSON stream output.
///
/// Codex returns one JSON object per line:
/// - `item.completed` → `item.text` has the response
/// - `turn.completed` → `usage` has token counts
fn parse_codex_output(raw: &str) -> Result<CliOutput, ProviderError> {
    let mut content = String::new();
    let mut prompt_tokens = None;
    let mut completion_tokens = None;

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Extract text from item.completed events.
        if obj.get("type").and_then(|t| t.as_str()) == Some("item.completed") {
            if let Some(text) = obj
                .get("item")
                .and_then(|i| i.get("text"))
                .and_then(|t| t.as_str())
            {
                if !text.is_empty() {
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(text);
                }
            }
        }

        // Extract usage from turn.completed.
        if obj.get("type").and_then(|t| t.as_str()) == Some("turn.completed") {
            if let Some(usage) = obj.get("usage") {
                prompt_tokens = usage
                    .get("input_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);
                completion_tokens = usage
                    .get("output_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);
            }
        }
    }

    if content.is_empty() {
        return Err(ProviderError::Other(
            "codex returned no text content".into(),
        ));
    }

    Ok(CliOutput {
        content,
        prompt_tokens,
        completion_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_output_claude_json() {
        let json = r#"{"result":"hello world","usage":{"input_tokens":10}}"#;
        let result = parse_output("claude", json.as_bytes()).unwrap();
        assert_eq!(result.content, "hello world");
    }

    #[test]
    fn parse_output_codex_ndjson() {
        let ndjson = r#"{"type":"thread.started","thread_id":"abc"}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"hello world"}}
{"type":"turn.completed","usage":{"input_tokens":100,"output_tokens":20}}"#;
        let result = parse_output("codex", ndjson.as_bytes()).unwrap();
        assert_eq!(result.content, "hello world");
        assert_eq!(result.prompt_tokens, Some(100));
        assert_eq!(result.completion_tokens, Some(20));
    }

    #[test]
    fn parse_output_codex_empty_text() {
        let ndjson = r#"{"type":"thread.started","thread_id":"abc"}
{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":0}}"#;
        assert!(parse_output("codex", ndjson.as_bytes()).is_err());
    }

    #[test]
    fn parse_output_plain_text() {
        let result = parse_output("some-cli", b"plain output").unwrap();
        assert_eq!(result.content, "plain output");
    }

    #[test]
    fn parse_output_empty_errors() {
        assert!(parse_output("claude", b"").is_err());
    }

    #[test]
    fn configure_claude_args() {
        let mut cmd = Command::new("echo");
        configure_cli_args(&mut cmd, "claude", Some("sonnet"));
    }
}
