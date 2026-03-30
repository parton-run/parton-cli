//! Pre-execution environment check.
//!
//! Inspects plan commands (`check_commands`, `validation_commands`,
//! `install_command`) and verifies that the required CLI tools are
//! available on the system before any code is generated.

use std::collections::BTreeSet;

use parton_core::RunPlan;

use crate::tui::style;

/// Warn if the plan references CLI tools not found on `$PATH`.
///
/// Returns `true` if all tools are present (or the user chose to
/// continue anyway), `false` if the user wants to abort.
pub fn warn_missing_tools(plan: &RunPlan) -> bool {
    let missing = find_missing_tools(plan);
    if missing.is_empty() {
        return true;
    }

    eprintln!();
    style::print_err("Missing tools detected:");
    for tool in &missing {
        eprintln!(
            "    {}",
            style::dim(&format!("{tool} — not found on $PATH"))
        );
    }
    eprintln!();
    eprintln!(
        "  The generated code will not compile or pass tests without {}.",
        if missing.len() == 1 {
            format!("`{}`", missing.iter().next().unwrap())
        } else {
            "these tools".to_string()
        }
    );
    eprintln!("  Install them first, or continue to generate code only.\n");

    ask_continue()
}

/// Extract tool names from plan commands and return those not on `$PATH`.
fn find_missing_tools(plan: &RunPlan) -> BTreeSet<String> {
    let mut commands: Vec<&str> = Vec::new();
    for cmd in &plan.check_commands {
        commands.push(cmd);
    }
    for cmd in &plan.validation_commands {
        commands.push(cmd);
    }
    if let Some(ref cmd) = plan.install_command {
        commands.push(cmd);
    }

    let mut tools = BTreeSet::new();
    for raw in &commands {
        if let Some(tool) = extract_tool(raw) {
            tools.insert(tool);
        }
    }

    tools.into_iter().filter(|t| !is_on_path(t)).collect()
}

/// Extract the base tool name from a shell command string.
///
/// Handles common prefixes like `CI=true`, `ENV=val`, and runners
/// like `npx`, `bunx`, `pipx` which delegate to the base tool.
fn extract_tool(cmd: &str) -> Option<String> {
    let mut tokens = cmd.split_whitespace();
    loop {
        let token = tokens.next()?;
        // Skip env assignments like CI=true, NODE_ENV=production.
        if token.contains('=') && !token.starts_with('-') {
            continue;
        }
        // npx/bunx/pipx run the tool, but the tool itself (node/bun/pip)
        // is the real dependency.
        let tool = match token {
            "npx" | "node" | "npm" => "node",
            "bunx" | "bun" => "bun",
            "pipx" | "pip" | "python" | "python3" => "python3",
            "cargo" | "rustc" => "cargo",
            other => other,
        };
        return Some(tool.to_string());
    }
}

/// Check whether a tool is available on `$PATH`.
fn is_on_path(tool: &str) -> bool {
    std::process::Command::new("which")
        .arg(tool)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Ask the user whether to continue despite missing tools.
fn ask_continue() -> bool {
    eprint!("  Continue anyway? [y/N] ");
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    matches!(input.trim(), "y" | "Y" | "yes" | "Yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_tool_simple() {
        assert_eq!(extract_tool("go build ./..."), Some("go".into()));
    }

    #[test]
    fn extract_tool_with_env_prefix() {
        assert_eq!(extract_tool("CI=true go test ./..."), Some("go".into()));
    }

    #[test]
    fn extract_tool_npx_maps_to_node() {
        assert_eq!(extract_tool("npx tsc --noEmit"), Some("node".into()));
    }

    #[test]
    fn extract_tool_npm_maps_to_node() {
        assert_eq!(extract_tool("npm run build"), Some("node".into()));
    }

    #[test]
    fn extract_tool_cargo() {
        assert_eq!(extract_tool("cargo test"), Some("cargo".into()));
    }

    #[test]
    fn extract_tool_python() {
        assert_eq!(extract_tool("python3 -m pytest"), Some("python3".into()));
    }

    #[test]
    fn extract_tool_multiple_env_vars() {
        assert_eq!(
            extract_tool("CI=true NODE_ENV=test npx vitest run"),
            Some("node".into())
        );
    }

    #[test]
    fn extract_tool_empty_returns_none() {
        assert_eq!(extract_tool(""), None);
    }

    #[test]
    fn is_on_path_finds_sh() {
        assert!(is_on_path("sh"));
    }

    #[test]
    fn is_on_path_misses_fake_tool() {
        assert!(!is_on_path("this_tool_does_not_exist_12345"));
    }
}
