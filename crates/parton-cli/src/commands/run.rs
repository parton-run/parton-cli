//! The `parton run` command — pipeline.
//!
//! 0. Code graph (scan existing project)
//! 1. Clarification (with graph context)
//! 2. Skeleton plan (with graph context)
//! 3. Scaffold execution (with per-file graph context)
//! 4. Structure check + auto-fix
//! 5. Final execution (with per-file graph context)
//! 6. Validation + auto-fix

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use parton_core::{FileResult, StageKind};

/// Filter out pre-existing errors from validation output.
///
/// Compares each error line against the baseline — if a line existed
/// before our run, it's not a new failure.
fn filter_baseline_errors(errors: &str, baseline: &str) -> String {
    if baseline.is_empty() {
        return errors.to_string();
    }
    let baseline_lines: std::collections::HashSet<&str> = baseline
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    errors
        .lines()
        .filter(|line| !baseline_lines.contains(line.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Dump debug content to `.parton/debug/{name}`.
fn debug_dump(project_root: &Path, debug: bool, name: &str, content: &str) {
    if !debug {
        return;
    }
    let dir = project_root.join(".parton/debug");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(name), content);
}

use super::run_helpers::{
    create_provider, format_comments, is_greenfield_project, is_missing_tool_error, load_or_setup,
    needs_final_execution, run_validation_check,
};
use crate::tui::{clarify, plan_review, progress, spinner, style};

/// Execute the full pipeline.
pub fn run(prompt: &str, project_root: &Path, _review: bool, debug: bool) -> Result<()> {
    let start = Instant::now();
    let config = load_or_setup(project_root)?;

    let planning_provider = create_provider(StageKind::Planning, &config)?;
    let exec_provider = create_provider(StageKind::Execution, &config)
        .or_else(|_| create_provider(StageKind::Planning, &config))?;

    let knowledge_root = project_root.join(".parton");
    let knowledge_store = parton_knowledge::LocalStore::new(&knowledge_root);
    parton_knowledge::auto_init(&knowledge_store, project_root);

    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    let is_greenfield = is_greenfield_project(project_root);

    // ── Step 0: Code graph (try KYP map first, fallback to live scan) ──
    let (code_graph, graph_summary, light_summary, tool_defs, has_kyp_map, mut grammar_store) =
        if is_greenfield {
            (
                parton_graph::CodeGraph::new(),
                String::new(),
                String::new(),
                vec![],
                false,
                None,
            )
        } else {
            // Try loading .parton/map first.
            let mut kyp_map = parton_graph::kyp::load_map(project_root)
                .filter(|m| parton_graph::kyp::map_is_current(m, project_root));

            // Offer to build KYP map if it doesn't exist.
            if kyp_map.is_none() && !is_greenfield {
                eprintln!();
                style::print_header("Project map not found");
                eprintln!(
                    "  {} scans your project with tree-sitter and builds a compact\n  \
                 AI-optimized map (.parton/map). This gives the planner better\n  \
                 context with fewer tokens — faster and cheaper runs.\n",
                    style::bold("parton kyp")
                );
                eprint!("  Build project map now? [Y/n] ");
                let mut input = String::new();
                let _ = std::io::stdin().read_line(&mut input);
                let answer = input.trim().to_lowercase();
                if answer.is_empty() || answer == "y" || answer == "yes" {
                    eprintln!();
                    if let Ok(()) = super::kyp::run_kyp(project_root) {
                        kyp_map = parton_graph::kyp::load_map(project_root);
                    }
                    eprintln!();
                }
            }

            if let Some(ref map) = kyp_map {
                style::print_header("Code graph (from .parton/map)");
                style::print_ok(&format!(
                    "{} files, {} concepts (cached)",
                    map.file_count,
                    map.concepts.len()
                ));
            } else {
                style::print_header("Code graph");
            }

            // Always do live scan — needed for graph contexts, tool handlers, contract check.
            // Returns both graph AND grammar store (reused for contract verification).
            let spin = spinner::Spinner::start("Scanning project...");
            let (graph, grammar_store) = rt
                .block_on(async { parton_graph::scan_project_with_store(project_root).await })
                .unwrap_or_else(|e| {
                    tracing::warn!("graph scan failed: {e}");
                    (
                        parton_graph::CodeGraph::new(),
                        parton_graph::GrammarStore::with_default_cache()
                            .expect("grammar store init failed"),
                    )
                });
            spin.stop();

            let summary = parton_graph::build_graph_summary(&graph, project_root);

            // Use KYP map context if available, otherwise light summary.
            let light = if let Some(ref map) = kyp_map {
                parton_graph::kyp::map_to_context_light(map)
            } else {
                parton_graph::build_light_graph_summary(&graph)
            };

            let tools = parton_graph::tools::create_tool_definitions();
            if kyp_map.is_none() && graph.file_count() > 0 {
                style::print_ok(&format!(
                    "{} files scanned, {} with exports",
                    graph.file_count(),
                    graph
                        .files
                        .values()
                        .filter(|f| f.symbols.iter().any(|s| s.exported))
                        .count()
                ));
            }
            let has_map = kyp_map.is_some();
            (graph, summary, light, tools, has_map, Some(grammar_store))
        };

    // ── Step 1: Clarification ──
    style::print_header("Analyzing intent");

    // Build a tool handler closure — only read_file + list_files.
    // The KYP map is the primary context; tools are for drill-down.
    let tool_graph = &code_graph;
    let tool_root = project_root;
    let handle_tool = move |call: parton_core::ToolCall| -> parton_core::ToolResult {
        parton_graph::tools::handle_tool_call(&call, tool_graph, tool_root)
    };

    // Debug: dump map/summary context.
    debug_dump(project_root, debug, "00_light_summary.txt", &light_summary);
    debug_dump(project_root, debug, "00_graph_summary.txt", &graph_summary);

    // Use light summary + tools when available, fall back to fat summary.
    let clarify_summary = if tool_defs.is_empty() {
        &graph_summary
    } else {
        &light_summary
    };

    // With KYP map: no tool use needed — map has full context. One-shot call.
    // Without map: use tools for drill-down.
    let use_tools = !tool_defs.is_empty() && !has_kyp_map;

    let spin = spinner::Spinner::start(if use_tools {
        "Analyzing (with tool use)..."
    } else {
        "Analyzing..."
    });
    let clarification = rt
        .block_on(async {
            if use_tools {
                parton_planner::generate_questions_with_tools(
                    prompt,
                    is_greenfield,
                    clarify_summary,
                    &*planning_provider,
                    &tool_defs,
                    &handle_tool,
                )
                .await
            } else {
                parton_planner::generate_questions(
                    prompt,
                    is_greenfield,
                    clarify_summary,
                    &*planning_provider,
                )
                .await
            }
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

    debug_dump(
        project_root,
        debug,
        "01_clarify_summary.txt",
        clarify_summary,
    );
    debug_dump(
        project_root,
        debug,
        "01_enriched_prompt.txt",
        &enriched_prompt,
    );

    // Combine project context with graph summary for the planner.
    // Use light summary when tools are available; fat summary otherwise.
    let project_ctx = parton_planner::build_project_context(project_root);
    let planner_summary = if tool_defs.is_empty() {
        &graph_summary
    } else {
        &light_summary
    };
    let full_context = if planner_summary.is_empty() {
        project_ctx
    } else {
        format!("{project_ctx}\n\n{planner_summary}")
    };

    debug_dump(project_root, debug, "02_planner_context.txt", &full_context);
    debug_dump(
        project_root,
        debug,
        "02_planner_prompt.txt",
        &enriched_prompt,
    );

    // ── Step 2: Skeleton plan ──
    style::print_header("Step 2 — Plan");
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
            let mut skel = rt
                .block_on(async {
                    if use_tools {
                        parton_planner::generate_skeleton_with_tools(
                            &prompt,
                            &full_context,
                            &*planning_provider,
                            &tool_defs,
                            &handle_tool,
                        )
                        .await
                    } else {
                        parton_planner::generate_skeleton(
                            &prompt,
                            &full_context,
                            &*planning_provider,
                        )
                        .await
                    }
                })
                .map_err(|e| anyhow::anyhow!("planning failed: {e}"))?;
            spin.stop();

            // Auto-fix: if planner says Create but file exists, switch to Edit.
            for file in &mut skel.files {
                if file.action == parton_core::FileAction::Create
                    && project_root.join(&file.path).exists()
                {
                    file.action = parton_core::FileAction::Edit;
                }
            }

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
                        if use_tools {
                            parton_planner::generate_skeleton_with_tools(
                                &replan_prompt,
                                &full_context,
                                &*planning_provider,
                                &tool_defs,
                                &handle_tool,
                            )
                            .await
                        } else {
                            parton_planner::generate_skeleton(
                                &replan_prompt,
                                &full_context,
                                &*planning_provider,
                            )
                            .await
                        }
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

    // ── Baseline: capture pre-existing failures using plan's validation commands ──
    let baseline_errors = if !plan.validation_commands.is_empty() {
        let spin = spinner::Spinner::start("Checking baseline...");
        let baseline = parton_executor::run_check(&plan.validation_commands, project_root);
        spin.stop();
        if baseline.passed {
            style::print_ok("Baseline: all validations pass");
        } else {
            let count = baseline
                .errors
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count();
            style::print_err(&format!(
                "Baseline: {count} pre-existing errors (will be ignored)"
            ));
        }
        debug_dump(
            project_root,
            debug,
            "00_baseline_errors.txt",
            &baseline.errors,
        );
        baseline.errors
    } else {
        String::new()
    };

    // ── Environment check ──
    if !super::env_check::warn_missing_tools(&plan) {
        style::print_err("Aborted — install the missing tools and try again.");
        return Ok(());
    }

    // ── Build per-file graph contexts for executors ──
    let graph_contexts = parton_graph::build_file_contexts(&code_graph, &plan.files);

    // Debug: dump per-file contexts.
    if debug {
        for (path, ctx) in &graph_contexts {
            let safe_name = path.replace('/', "_");
            debug_dump(
                project_root,
                debug,
                &format!("03_file_{safe_name}.txt"),
                ctx,
            );
        }
        // Dump plan as JSON.
        if let Ok(plan_json) = serde_json::to_string_pretty(&plan) {
            debug_dump(project_root, debug, "02_plan.json", &plan_json);
        }
    }

    // ── Step 3: Scaffold+Enrich (parallel, combined) ──
    style::print_header("Step 3 — Scaffold");
    let exec_labels: Vec<String> = plan.files.iter().map(|f| f.path.clone()).collect();
    let scaffold_prog = progress::ParallelProgress::new(&exec_labels);
    let _ticker = scaffold_prog.start_ticker();

    let scaffold_results = rt.block_on(async {
        parton_executor::scaffold_streaming_with_graph(
            &plan,
            &*exec_provider,
            project_root,
            &graph_contexts,
            &|r| scaffold_prog.complete(&r.path, r.elapsed_ms, r.success),
        )
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
        if !sr.enriched_goal.is_empty() {
            if let Some(file) = enriched_plan.files.iter_mut().find(|f| f.path == sr.path) {
                file.goal = sr.enriched_goal.clone();
            }
        }
    }
    let mut scaffold_tokens: u32 = scaffold_results.iter().map(|r| r.tokens_used).sum();

    // ── Step 3.5: Contract verification (tree-sitter) ──
    // Reuse the grammar store from graph scan to avoid Wasm engine conflicts.
    let violations = if let Some(ref mut store) = grammar_store {
        rt.block_on(async {
            parton_executor::contract::check_contracts(&scaffold_results, &plan, store).await
        })
    } else {
        vec![]
    };

    // Filter out parse errors (Wasm store failures) — only act on real violations.
    let real_violations: Vec<_> = violations
        .into_iter()
        .filter(|v| !matches!(v.kind, parton_graph::verify::ViolationKind::ParseError))
        .collect();

    if real_violations.is_empty() {
        style::print_ok("Contracts: verified");
    } else {
        let violation_files: Vec<&str> = real_violations
            .iter()
            .map(|v| v.path.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        style::print_err(&format!(
            "Contract violations in {} files — re-scaffolding",
            violation_files.len()
        ));
        for v in &real_violations {
            eprintln!("    {}", style::dim(&v.details));
        }

        let re_results = rt.block_on(async {
            parton_executor::contract::re_scaffold_violations(
                &real_violations,
                &plan,
                &*exec_provider,
                project_root,
                &graph_contexts,
                &|r| {
                    if r.success {
                        style::print_ok(&format!("  re-scaffold: {}", r.path));
                    }
                },
            )
            .await
        });

        // Update scaffold results with re-scaffolded code.
        for rr in &re_results {
            if rr.success && !rr.code.is_empty() {
                let full_path = project_root.join(&rr.path);
                if let Some(parent) = full_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&full_path, &rr.code);
                if !scaffold_written.contains(&rr.path) {
                    scaffold_written.push(rr.path.clone());
                }
            }
            if !rr.enriched_goal.is_empty() {
                if let Some(file) = enriched_plan.files.iter_mut().find(|f| f.path == rr.path) {
                    file.goal = rr.enriched_goal.clone();
                }
            }
            scaffold_tokens += rr.tokens_used;
        }
    }

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
    } else if is_missing_tool_error(&check.errors) {
        style::print_err("Structure check skipped — required tools not installed");
        String::new()
    } else {
        style::print_err("Structure errors — fixing config files first:");
        for line in check.errors.lines().take(10) {
            eprintln!("    {}", style::dim(line));
        }

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

            let recheck = parton_executor::run_check(&plan.check_commands, project_root);
            if recheck.passed {
                style::print_ok("Config fix successful — structure compiles");
            }
        }

        check.errors
    };

    // ── Step 4: Final execution (only logic files) ──
    let mut final_plan = enriched_plan.clone();
    final_plan.files.retain(needs_final_execution);
    let skipped = enriched_plan.files.len() - final_plan.files.len();

    style::print_header("Step 4 — Final Execution");
    if skipped > 0 {
        style::print_ok(&format!("{skipped} files kept from scaffold"));
    }

    let final_labels: Vec<String> = final_plan.files.iter().map(|f| f.path.clone()).collect();
    let final_prog = progress::ParallelProgress::new(&final_labels);
    let _ticker = final_prog.start_ticker();

    let final_results = rt.block_on(async {
        parton_executor::execute_streaming_with_graph(
            &final_plan,
            &*exec_provider,
            project_root,
            parton_executor::ExecMode::Final,
            &structure_errors,
            &graph_contexts,
            &|r| final_prog.complete(&r.path, r.elapsed_ms, r.success),
        )
        .await
    });
    drop(_ticker);

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

    // ── Step 5: Validation (with auto-fix retry) ──
    if !plan.validation_commands.is_empty() {
        style::print_header("Step 5 — Validation");
        let val_result = run_validation_check(&plan.validation_commands, project_root);

        // Filter out pre-existing failures so we only act on NEW errors.
        let new_errors = filter_baseline_errors(&val_result.errors, &baseline_errors);
        let actually_passed = val_result.passed || new_errors.trim().is_empty();

        if !actually_passed {
            if is_missing_tool_error(&val_result.errors) {
                style::print_err("Skipping auto-fix — install the missing tools and re-run");
            } else {
                style::print_err("Validation failed — attempting auto-fix...");

                // Only re-run files that are mentioned in NEW errors.
                let mut fix_plan = final_plan.clone();
                fix_plan.files.retain(|f| new_errors.contains(&f.path));

                if fix_plan.files.is_empty() {
                    style::print_err("No planned files in error output — cannot auto-fix");
                    for line in new_errors.lines().take(10) {
                        eprintln!("    {}", style::dim(line));
                    }
                } else {
                    style::print_ok(&format!(
                        "Auto-fixing {} of {} files",
                        fix_plan.files.len(),
                        final_plan.files.len()
                    ));

                    let fix_labels: Vec<String> =
                        fix_plan.files.iter().map(|f| f.path.clone()).collect();
                    let fix_prog = progress::ParallelProgress::new(&fix_labels);
                    let _ticker = fix_prog.start_ticker();
                    let fix_results = rt.block_on(async {
                        parton_executor::execute_streaming(
                            &fix_plan,
                            &*exec_provider,
                            project_root,
                            parton_executor::ExecMode::Final,
                            &new_errors,
                            &|r| fix_prog.complete(&r.path, r.elapsed_ms, r.success),
                        )
                        .await
                    });
                    drop(_ticker);

                    let fix_ok: Vec<FileResult> =
                        fix_results.iter().filter(|r| r.success).cloned().collect();
                    let _ = parton_executor::write_results(&fix_ok, project_root);

                    let retry = run_validation_check(&plan.validation_commands, project_root);
                    let retry_new = filter_baseline_errors(&retry.errors, &baseline_errors);
                    if retry.passed || retry_new.trim().is_empty() {
                        style::print_ok("Auto-fix successful — validation passed");
                    } else {
                        style::print_err("Validation still failing after auto-fix");
                        for line in retry_new.lines().take(10) {
                            eprintln!("    {}", style::dim(line));
                        }
                    }
                }
            }
        }
    }

    // ── Summary ──
    let elapsed = start.elapsed();
    let final_tokens: u32 = final_results.iter().map(|r| r.tokens_used).sum();
    let total_tokens = scaffold_tokens + final_tokens;
    let failed = final_results.iter().filter(|r| !r.success).count();

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
