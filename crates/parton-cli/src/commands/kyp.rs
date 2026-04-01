//! `parton kyp` command — generate `.parton/map` project index.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use parton_core::PartonConfig;

use crate::tui::{spinner, style};

/// Run the KYP indexer for a project.
pub fn run_kyp(project_root: &Path) -> Result<()> {
    let start = Instant::now();

    style::print_header("KYP — Know Your Project");

    // Ensure provider is configured (needed for LLM enrichment).
    let config = PartonConfig::load(project_root).context("failed to load config")?;
    if config.models.default.is_none() {
        eprintln!("  No provider configured. Starting setup...\n");
        crate::commands::setup::run_setup(project_root).context("setup failed")?;
    }

    // Step 1: Scan with tree-sitter.
    let spin = spinner::Spinner::start("Scanning project...");
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    let graph = rt
        .block_on(async { parton_graph::scan_project(project_root).await })
        .unwrap_or_else(|e| {
            tracing::warn!("graph scan failed: {e}");
            parton_graph::CodeGraph::new()
        });
    spin.stop();

    if graph.file_count() == 0 {
        style::print_err("No source files found");
        return Ok(());
    }
    style::print_ok(&format!("{} files scanned", graph.file_count()));

    // Step 2: Build draft map (tree-sitter only).
    let spin = spinner::Spinner::start("Building project map...");
    let git_sha = parton_graph::kyp::get_git_sha(project_root);
    let map = parton_graph::kyp::build_map(&graph, &git_sha);
    spin.stop();

    style::print_ok(&format!(
        "{} concepts, {} conventions",
        map.concepts.len(),
        map.conventions.len()
    ));

    // Step 3: LLM enrichment — add semantic #tags.
    let final_map = enrich_with_llm(&map, project_root, &rt).unwrap_or(map);

    // Step 4: Save.
    let final_content = final_map.to_string();
    let dir = project_root.join(".parton");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("map"), &final_content).context("failed to write .parton/map")?;

    // Save lock.
    let file_paths: Vec<String> = graph.files.keys().cloned().collect();
    let lock = parton_graph::kyp::lock::build_lock(&file_paths, project_root, &git_sha);
    parton_graph::kyp::lock::save_lock(&lock, project_root)
        .context("failed to write .parton/map.lock")?;

    let elapsed = start.elapsed();
    style::print_ok(&format!(
        "Saved to .parton/map ({:.1}s)",
        elapsed.as_secs_f64()
    ));

    // Print preview.
    eprintln!();
    for line in final_content.lines().take(30) {
        eprintln!("  {}", style::dim(line));
    }
    if final_content.lines().count() > 30 {
        eprintln!(
            "  {}",
            style::dim(&format!(
                "... ({} more lines)",
                final_content.lines().count() - 30
            ))
        );
    }
    eprintln!();
    style::print_kv("Size", &format!("{} bytes", final_content.len()));
    style::print_kv("Concepts", &format!("{}", final_map.concepts.len()));

    Ok(())
}

/// LLM enrichment — parallel per-concept calls.
fn enrich_with_llm(
    map: &parton_graph::kyp::format::PartonMap,
    project_root: &Path,
    rt: &tokio::runtime::Runtime,
) -> Option<parton_graph::kyp::format::PartonMap> {
    let config = PartonConfig::load(project_root).ok()?;
    let provider =
        parton_providers::create_stage_provider(parton_core::StageKind::Planning, &config.models)
            .ok()?;

    let total = map.concepts.len();
    eprintln!("  Enriching {total} concepts with LLM...");
    let enriched = rt.block_on(async {
        parton_graph::kyp::enrich::enrich_map(
            map,
            &*provider,
            project_root,
            &|done, total, name| {
                eprint!("\r  \x1b[2K  \x1b[1;32m✓\x1b[0m {done}/{total} — {name}");
            },
        )
        .await
    });
    eprintln!();

    let tagged = enriched
        .concepts
        .iter()
        .filter(|c| !c.tags.is_empty())
        .count();
    if tagged > 0 {
        style::print_ok(&format!(
            "{tagged}/{} concepts enriched",
            enriched.concepts.len()
        ));
    } else {
        style::print_err("LLM enrichment produced no tags");
    }
    Some(enriched)
}
