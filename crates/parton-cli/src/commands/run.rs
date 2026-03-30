//! The `parton run` command — 6-step pipeline.
//!
//! 1. Skeleton plan (contracts)
//! 2. Enrich plan (detailed goals, parallel)
//! 3. Scaffold execution (minimal stubs, parallel)
//! 4. Structure check (compile/type-check + auto-fix)
//! 5. Final execution (full implementation, parallel)
//! 6. Validation (tests + build)

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use parton_core::{FileResult, ModelProvider, PartonConfig, StageKind};

use crate::tui::{clarify, plan_review, progress, spinner, style};

/// Execute the full pipeline.
pub fn run(prompt: &str, project_root: &Path, _review: bool) -> Result<()> {
    let start = Instant::now();
    let config = load_or_setup(project_root)?;

    let planning_provider = create_provider(StageKind::Planning, &config)?;
    let exec_provider = create_provider(StageKind::Execution, &config)
        .or_else(|_| create_provider(StageKind::Planning, &config))?;

    let knowledge_root = project_root.join(".parton");
    let knowledge_store = parton_knowledge::LocalStore::new(&knowledge_root);
    parton_knowledge::auto_init(&knowledge_store, project_root);

    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;

    // ── Step 0: Clarification ──
    style::print_header("Analyzing intent");
    let is_greenfield = is_greenfield_project(project_root);

    let spin = spinner::Spinner::start("Analyzing...");
    let clarification = rt
        .block_on(async {
            parton_planner::generate_questions(prompt, is_greenfield, &*planning_provider).await
        })
        .unwrap_or_else(|e| {
            tracing::warn!("Clarification failed: {e}");
            parton_core::ClarificationResult {
                questions: vec![],
                assumptions: vec![],
                confidence: 0.5,
                sufficient_for_planning: true,
            }
        });
    spin.stop();

    let planning_ctx = clarify::run_clarification(prompt, &clarification);
    let mut enriched_prompt = planning_ctx.to_enriched_prompt();

    let knowledge_ctx = parton_knowledge::build_knowledge_context(&knowledge_store);
    if !knowledge_ctx.is_empty() {
        enriched_prompt = format!("{knowledge_ctx}\n\n{enriched_prompt}");
    }

    let project_ctx = parton_planner::build_project_context(project_root);

    // ── Step 1: Skeleton plan (with retry on validation failure) ──
    style::print_header("Step 1 — Plan");
    let plan = {
        let mut last_error = String::new();
        let mut result = None;
        for attempt in 0..2 {
            let prompt = if attempt == 0 {
                enriched_prompt.clone()
            } else {
                format!(
                    "{enriched_prompt}\n\n## PREVIOUS PLAN WAS REJECTED\n\
                     Reason: {last_error}\n\nFix the issues and regenerate."
                )
            };
            let spin = spinner::Spinner::start(if attempt == 0 {
                "Generating plan..."
            } else {
                "Retrying..."
            });
            let skel = rt
                .block_on(async {
                    parton_planner::generate_skeleton(&prompt, &project_ctx, &*planning_provider)
                        .await
                })
                .map_err(|e| anyhow::anyhow!("planning failed: {e}"))?;
            spin.stop();

            match parton_planner::validate_plan(&skel, project_root) {
                Ok(()) => {
                    result = Some(skel);
                    break;
                }
                Err(e) => {
                    style::print_err(&format!("Validation: {e}"));
                    last_error = e.to_string();
                }
            }
        }
        result.ok_or_else(|| anyhow::anyhow!("plan failed: {last_error}"))?
    };
    style::print_ok(&format!("{} files planned", plan.files.len()));

    // ── Plan review ──
    let mut plan = plan;
    loop {
        match plan_review::run_review(plan.clone())? {
            plan_review::ReviewDecision::Approve => {
                eprintln!("  Plan approved!");
                break;
            }
            plan_review::ReviewDecision::Replan(comments) => {
                let comment_text = format_comments(&comments);
                let replan_prompt = format!("{enriched_prompt}\n\n## Feedback\n{comment_text}");
                let spin = spinner::Spinner::start("Replanning...");
                plan = rt
                    .block_on(async {
                        parton_planner::generate_skeleton(
                            &replan_prompt,
                            &project_ctx,
                            &*planning_provider,
                        )
                        .await
                    })
                    .map_err(|e| anyhow::anyhow!("replan failed: {e}"))?;
                spin.stop();
                style::print_ok(&format!("Revised: {} files", plan.files.len()));
            }
            plan_review::ReviewDecision::Reject => {
                style::print_err("Plan rejected.");
                return Ok(());
            }
        }
    }

    // ── Step 2: Scaffold+Enrich (parallel, combined) ──
    style::print_header("Step 2 — Scaffold");
    let exec_labels: Vec<String> = plan.files.iter().map(|f| f.path.clone()).collect();
    let scaffold_prog = progress::ParallelProgress::new(&exec_labels);
    let _ticker = scaffold_prog.start_ticker();

    let scaffold_results = rt.block_on(async {
        parton_executor::scaffold_streaming(&plan, &*exec_provider, project_root, &|r| {
            scaffold_prog.complete(&r.path, r.elapsed_ms, r.success)
        })
        .await
    });
    drop(_ticker);

    // Write scaffold code to disk + update plan with enriched goals.
    let mut enriched_plan = plan.clone();
    let mut scaffold_written = Vec::new();
    for sr in &scaffold_results {
        if sr.success && !sr.code.is_empty() {
            let full_path = project_root.join(&sr.path);
            if let Some(parent) = full_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&full_path, &sr.code);
            scaffold_written.push(sr.path.clone());
        }
        // Update enriched goal if provided.
        if !sr.enriched_goal.is_empty() {
            if let Some(file) = enriched_plan.files.iter_mut().find(|f| f.path == sr.path) {
                file.goal = sr.enriched_goal.clone();
            }
        }
    }
    let scaffold_tokens: u32 = scaffold_results.iter().map(|r| r.tokens_used).sum();

    // Install deps.
    if let Some(ref cmd) = plan.install_command {
        let spin = spinner::Spinner::start(&format!("Installing deps ({cmd})..."));
        let ok = parton_executor::run_install(cmd, project_root);
        spin.stop();
        if ok {
            style::print_ok("Dependencies installed");
        } else {
            style::print_err("Install failed");
        }
    }

    // Structure check.
    let spin = spinner::Spinner::start("Checking structure...");
    let check = parton_executor::run_check(&plan.check_commands, project_root);
    spin.stop();

    let structure_errors = if check.passed {
        style::print_ok("Structure compiles");
        String::new()
    } else {
        style::print_err("Structure errors — fixing config files first:");
        for line in check.errors.lines().take(10) {
            eprintln!("    {}", style::dim(line));
        }

        // Re-scaffold ONLY config files that have errors.
        let config_fix_plan = {
            let mut p = enriched_plan.clone();
            p.files.retain(|f| {
                f.scaffold_only && check.errors.lines().any(|line| line.contains(&f.path))
            });
            p
        };
        if !config_fix_plan.files.is_empty() {
            let fix_labels: Vec<String> = config_fix_plan
                .files
                .iter()
                .map(|f| f.path.clone())
                .collect();
            let fix_prog = progress::ParallelProgress::new(&fix_labels);
            let _t = fix_prog.start_ticker();
            let fix_results = rt.block_on(async {
                parton_executor::execute_streaming(
                    &config_fix_plan,
                    &*exec_provider,
                    project_root,
                    parton_executor::ExecMode::Final,
                    &check.errors,
                    &|r| fix_prog.complete(&r.path, r.elapsed_ms, r.success),
                )
                .await
            });
            drop(_t);
            let fix_ok: Vec<FileResult> =
                fix_results.iter().filter(|r| r.success).cloned().collect();
            let _ = parton_executor::write_results(&fix_ok, project_root);

            // Re-check.
            let recheck = parton_executor::run_check(&plan.check_commands, project_root);
            if recheck.passed {
                style::print_ok("Config fix successful — structure compiles");
            }
        }

        check.errors
    };

    // ── Step 3: Final execution (only logic files) ──
    let mut final_plan = enriched_plan.clone();
    final_plan.files.retain(needs_final_execution);
    let skipped = enriched_plan.files.len() - final_plan.files.len();

    style::print_header("Step 3 — Final Execution");
    if skipped > 0 {
        style::print_ok(&format!("{skipped} files kept from scaffold"));
    }

    let final_labels: Vec<String> = final_plan.files.iter().map(|f| f.path.clone()).collect();
    let final_prog = progress::ParallelProgress::new(&final_labels);
    let _ticker = final_prog.start_ticker();

    let final_results = rt.block_on(async {
        parton_executor::execute_streaming(
            &final_plan,
            &*exec_provider,
            project_root,
            parton_executor::ExecMode::Final,
            &structure_errors,
            &|r| final_prog.complete(&r.path, r.elapsed_ms, r.success),
        )
        .await
    });
    drop(_ticker);

    // Compliance check (only on files that went through final execution).
    let issues = parton_executor::check_all(&final_plan, &final_results);
    if issues.is_empty() {
        style::print_ok(&format!(
            "Compliance: {}/{} OK",
            final_plan.files.len(),
            final_plan.files.len()
        ));
    } else {
        for issue in &issues {
            style::print_err(&format!("{}: {}", issue.file_path, issue.message));
        }
    }

    // Write final files to disk (only logic files — config already written by scaffold).
    let final_ok: Vec<FileResult> = final_results
        .iter()
        .filter(|r| r.success)
        .cloned()
        .collect();
    let written_final =
        parton_executor::write_results(&final_ok, project_root).context("failed to write files")?;
    let total_written = scaffold_written.len() + written_final.len();
    style::print_ok(&format!(
        "{total_written} files total ({} scaffold + {} final)",
        scaffold_written.len(),
        written_final.len()
    ));

    // ── Step 6: Validation (with auto-fix retry) ──
    if !plan.validation_commands.is_empty() {
        style::print_header("Step 6 — Validation");
        let val_result = run_validation_check(&plan.validation_commands, project_root);

        if !val_result.passed {
            style::print_err("Validation failed — attempting auto-fix...");

            // Re-run final execution with validation errors as context.
            let fix_prog = progress::ParallelProgress::new(&final_labels);
            let _ticker = fix_prog.start_ticker();
            let fix_results = rt.block_on(async {
                parton_executor::execute_streaming(
                    &final_plan,
                    &*exec_provider,
                    project_root,
                    parton_executor::ExecMode::Final,
                    &val_result.errors,
                    &|r| fix_prog.complete(&r.path, r.elapsed_ms, r.success),
                )
                .await
            });
            drop(_ticker);

            let fix_ok: Vec<FileResult> =
                fix_results.iter().filter(|r| r.success).cloned().collect();
            let _ = parton_executor::write_results(&fix_ok, project_root);

            // Re-validate.
            let retry = run_validation_check(&plan.validation_commands, project_root);
            if retry.passed {
                style::print_ok("Auto-fix successful — validation passed");
            } else {
                style::print_err("Validation still failing after auto-fix");
                for line in retry.errors.lines().take(10) {
                    eprintln!("    {}", style::dim(line));
                }
            }
        }
    }

    // ── Summary ──
    let elapsed = start.elapsed();
    let final_tokens: u32 = final_results.iter().map(|r| r.tokens_used).sum();
    let total_tokens = scaffold_tokens + final_tokens;
    let failed = final_results.iter().filter(|r| !r.success).count();

    // Extract learnings.
    let summary = format!(
        "Prompt: {prompt}\nFiles: {}\nFailed: {failed}",
        written_final.join(", "),
    );
    let _ = rt.block_on(parton_knowledge::extract_and_store(
        &summary,
        &*planning_provider,
        &knowledge_store,
    ));

    eprintln!();
    style::print_header("Done");
    style::print_kv(
        "Files",
        &format!("{total_written} written, {failed} failed"),
    );
    style::print_kv(
        "Tokens",
        &format!("{total_tokens} (scaffold: {scaffold_tokens}, final: {final_tokens})"),
    );
    style::print_kv("Time", &format!("{:.1}s", elapsed.as_secs_f64()));
    eprintln!();

    if failed > 0 {
        anyhow::bail!("{failed} files failed");
    }

    Ok(())
}

// ── Helpers ──

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

fn load_or_setup(project_root: &Path) -> Result<PartonConfig> {
    let config = PartonConfig::load(project_root).context("failed to load parton.toml")?;
    if config.models.default.is_none() {
        eprintln!("  No provider configured. Starting setup...\n");
        return crate::commands::setup::run_setup(project_root).context("setup failed");
    }
    Ok(config)
}

fn create_provider(stage: StageKind, config: &PartonConfig) -> Result<Box<dyn ModelProvider>> {
    parton_providers::create_stage_provider(stage, &config.models)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Determine if a file needs final execution.
///
/// Respects the planner's `scaffold_only` flag — the planner decides
/// which files are config/static (scaffold is final) vs logic (needs implementation).
fn needs_final_execution(file: &parton_core::FilePlan) -> bool {
    !file.scaffold_only
}

fn is_greenfield_project(root: &Path) -> bool {
    !root.join("package.json").exists()
        && !root.join("Cargo.toml").exists()
        && !root.join("go.mod").exists()
        && !root.join("pyproject.toml").exists()
}

fn run_validation_check(commands: &[String], project_root: &Path) -> parton_executor::CheckResult {
    let result = parton_executor::run_check(commands, project_root);
    for cmd in commands {
        // Check each individually for display.
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

