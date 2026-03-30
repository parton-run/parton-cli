//! The `parton run` command — end-to-end orchestration.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use parton_core::{FileResult, ModelProvider, PartonConfig, RunPlan, StageKind};

use crate::tui::{clarify, plan_review, progress, spinner, style};

/// Execute a turbo run: clarify → plan → review → execute → validate → report.
pub fn run(prompt: &str, project_root: &Path, _review: bool) -> Result<()> {
    let start = Instant::now();

    // 1. Load or create config.
    let config = load_or_setup(project_root)?;

    // 2. Resolve providers.
    let planning_provider = create_provider(StageKind::Planning, &config)?;
    let exec_provider = create_provider(StageKind::Execution, &config)
        .or_else(|_| create_provider(StageKind::Planning, &config))?;

    // 3. Initialize knowledge store.
    let knowledge_root = project_root.join(".parton");
    let knowledge_store = parton_knowledge::LocalStore::new(&knowledge_root);
    parton_knowledge::auto_init(&knowledge_store, project_root);

    // 4. Create tokio runtime.
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;

    // 5. Clarification flow.
    style::print_header("Analyzing intent");

    let is_greenfield = !project_root.join("package.json").exists()
        && !project_root.join("Cargo.toml").exists()
        && !project_root.join("go.mod").exists();

    // Generate clarification questions (with spinner).
    let spin = spinner::Spinner::start("Analyzing...");
    let clarification_result = rt.block_on(async {
        parton_planner::generate_questions(prompt, is_greenfield, &*planning_provider).await
    }).unwrap_or_else(|e| {
        tracing::warn!("Clarification failed: {e}");
        parton_core::ClarificationResult {
            questions: vec![],
            assumptions: vec![],
            confidence: 0.5,
            sufficient_for_planning: true,
        }
    });
    spin.stop();

    // Interactive TUI questions (no spinner — user is interacting).
    let planning_ctx = clarify::run_clarification(prompt, &clarification_result);

    let mut enriched_prompt = planning_ctx.to_enriched_prompt();

    // Inject project knowledge into the prompt.
    let knowledge_ctx = parton_knowledge::build_knowledge_context(&knowledge_store);
    if !knowledge_ctx.is_empty() {
        enriched_prompt = format!("{knowledge_ctx}\n\n{enriched_prompt}");
    }

    // 6. Plan.
    style::print_header("Planning");
    let spin = spinner::Spinner::start("Generating plan...");
    let mut plan = rt.block_on(generate_plan(&enriched_prompt, project_root, &*planning_provider))?;
    spin.stop();
    style::print_ok(&format!("{} files to process", plan.files.len()));

    // 6. Interactive plan review (with comments + replan loop).
    loop {
        match plan_review::run_review(plan.clone())? {
            plan_review::ReviewDecision::Approve => {
                eprintln!("  Plan approved! Executing...");
                break;
            }
            plan_review::ReviewDecision::Replan(comments) => {
                style::print_header("Replanning");
                let comment_text = format_comments(&comments);
                let replan_prompt = format!(
                    "{enriched_prompt}\n\n## Review Feedback\n\
                     The previous plan was reviewed and these comments were made:\n{comment_text}\n\n\
                     Generate a revised plan that addresses this feedback."
                );
                let spin = spinner::Spinner::start("Replanning...");
                plan = rt.block_on(generate_plan(&replan_prompt, project_root, &*planning_provider))?;
                spin.stop();
                style::print_ok(&format!("Revised plan: {} files", plan.files.len()));
            }
            plan_review::ReviewDecision::Reject => {
                style::print_err("Plan rejected.");
                return Ok(());
            }
        }
    }

    // 7. Execute all files in parallel (per-file progress, no spinner).
    eprintln!();
    let results = rt.block_on(execute_plan(&plan, &*exec_provider, project_root))?;

    // 8. Check compliance.
    let issues = parton_executor::check_all(&plan, &results);
    if issues.is_empty() {
        style::print_ok(&format!("Compliance: {}/{} files OK", plan.files.len(), plan.files.len()));
    } else {
        for issue in &issues {
            style::print_err(&format!("{}: {}", issue.file_path, issue.message));
        }
    }

    // 9. Write files to disk.
    let successful: Vec<FileResult> = results.iter().filter(|r| r.success).cloned().collect();
    let written = parton_executor::write_results(&successful, project_root)
        .context("failed to write files")?;
    style::print_ok(&format!("{} files written", written.len()));

    // 10. Auto-install dependencies if a manifest was written.
    auto_install_deps(project_root, &written);

    // 11. Run validation commands.
    if !config.execution.validation.is_empty() {
        run_validation(&config.execution.validation, project_root);
    }

    // 12. Extract learnings (background, don't block on failure).
    let run_summary = format!(
        "Prompt: {prompt}\nFiles written: {}\nFiles failed: {}\nValidation: {}",
        written.join(", "),
        results.iter().filter(|r| !r.success).map(|r| r.path.as_str()).collect::<Vec<_>>().join(", "),
        if config.execution.validation.is_empty() { "none" } else { "ran" },
    );
    let _ = rt.block_on(parton_knowledge::extract_and_store(
        &run_summary, &*planning_provider, &knowledge_store,
    ));

    // 13. Summary.
    let elapsed = start.elapsed();
    let total_tokens: u32 = results.iter().map(|r| r.tokens_used).sum();
    let failed = results.iter().filter(|r| !r.success).count();

    eprintln!();
    style::print_header("Done");
    style::print_kv("Files", &format!("{} written, {} failed", written.len(), failed));
    style::print_kv("Tokens", &format!("{total_tokens}"));
    style::print_kv("Time", &format!("{:.1}s", elapsed.as_secs_f64()));
    eprintln!();

    if failed > 0 {
        anyhow::bail!("{failed} files failed to generate");
    }

    Ok(())
}

/// Format review comments into text for the replan prompt.
fn format_comments(comments: &[plan_review::ReviewComment]) -> String {
    comments
        .iter()
        .map(|c| match &c.file {
            Some(path) => format!("- [{}] {}", path, c.text),
            None => format!("- [GENERAL] {}", c.text),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Load config or run setup.
fn load_or_setup(project_root: &Path) -> Result<PartonConfig> {
    let config = PartonConfig::load(project_root).context("failed to load parton.toml")?;
    if config.models.default.is_none() {
        eprintln!("  No provider configured. Starting setup...\n");
        return crate::commands::setup::run_setup(project_root).context("setup failed");
    }
    Ok(config)
}

/// Create provider for a stage.
fn create_provider(stage: StageKind, config: &PartonConfig) -> Result<Box<dyn ModelProvider>> {
    parton_providers::create_stage_provider(stage, &config.models)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Generate a turbo plan via LLM.
async fn generate_plan(prompt: &str, project_root: &Path, provider: &dyn ModelProvider) -> Result<RunPlan> {
    let response = provider
        .send(parton_planner::SYSTEM_PROMPT, prompt, false)
        .await
        .map_err(|e| anyhow::anyhow!("planning failed: {e}"))?;

    let plan = parton_planner::parse_plan(&response.content)
        .map_err(|e| anyhow::anyhow!("failed to parse plan: {e}"))?;

    parton_planner::validate_plan(&plan, project_root)
        .map_err(|e| anyhow::anyhow!("plan validation failed: {e}"))?;

    Ok(plan)
}

/// Execute all files in parallel with progress.
async fn execute_plan(
    plan: &RunPlan,
    provider: &dyn ModelProvider,
    project_root: &Path,
) -> Result<Vec<FileResult>> {
    let mut prog = progress::TurboProgress::new(plan.files.len());
    let overall = prog.overall_bar(plan.files.len());

    let indices: Vec<(String, usize)> = plan
        .files
        .iter()
        .map(|f| {
            let idx = prog.add_file(&f.path);
            (f.path.clone(), idx)
        })
        .collect();

    let results = parton_executor::execute_streaming(plan, provider, project_root, &|result| {
        if let Some((_, idx)) = indices.iter().find(|(p, _)| p == &result.path) {
            prog.complete_file(*idx, &result.path, result.elapsed_ms, result.success);
        }
        overall.inc(1);
    })
    .await;

    overall.finish_and_clear();
    Ok(results)
}

/// Auto-install dependencies if a manifest file was written.
///
/// Language-agnostic: detects JS/TS, Rust, Go, Python manifests.
fn auto_install_deps(project_root: &Path, written_files: &[String]) {
    let manifests = [
        ("package.json", detect_js_install_cmd(project_root)),
        ("Cargo.toml", Some(("cargo fetch", "cargo"))),
        ("go.mod", Some(("go mod download", "go"))),
        ("pyproject.toml", Some(("pip install -e .", "pip"))),
        ("requirements.txt", Some(("pip install -r requirements.txt", "pip"))),
    ];

    for (manifest, install) in &manifests {
        if written_files.iter().any(|f| f.ends_with(manifest)) {
            if let Some((cmd, label)) = install {
                run_install(project_root, cmd, label);
            }
        }
    }
}

/// Detect the right JS install command based on lockfile.
fn detect_js_install_cmd(root: &Path) -> Option<(&'static str, &'static str)> {
    if root.join("pnpm-lock.yaml").exists() {
        Some(("pnpm install", "pnpm"))
    } else if root.join("yarn.lock").exists() {
        Some(("yarn install", "yarn"))
    } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
        Some(("bun install", "bun"))
    } else {
        Some(("npm install", "npm"))
    }
}

/// Run an install command with spinner.
fn run_install(project_root: &Path, cmd: &str, label: &str) {
    let spin = spinner::Spinner::start(&format!("Installing dependencies ({label})..."));
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(project_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();
    spin.stop();

    match output {
        Ok(o) if o.status.success() => style::print_ok(&format!("Dependencies installed ({label})")),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            style::print_err(&format!("{label} install failed: {}", stderr.lines().next().unwrap_or("")));
        }
        Err(e) => style::print_err(&format!("{label} install failed: {e}")),
    }
}

/// Run validation commands.
fn run_validation(commands: &[String], project_root: &Path) {
    eprintln!();
    style::print_header("Validation");

    for cmd in commands {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(project_root)
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
}
