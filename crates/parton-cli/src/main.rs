#![deny(warnings)]

mod commands;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Parton — AI-powered parallel code generation CLI.
#[derive(Parser, Debug)]
#[command(name = "parton", version, about, long_about = None)]
struct Cli {
    /// Enable verbose output.
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

/// Available commands.
#[derive(Subcommand, Debug)]
enum Command {
    /// Generate code from a natural language prompt.
    Run {
        /// What to build (e.g. "create a todo app with React").
        #[arg(required = true)]
        prompt: Vec<String>,

        /// Project directory.
        #[arg(short, long, default_value = ".")]
        path: std::path::PathBuf,

        /// Review plan before executing.
        #[arg(long)]
        review: bool,
    },

    /// Configure providers and create parton.toml.
    Setup {
        /// Project directory.
        #[arg(short, long, default_value = ".")]
        path: std::path::PathBuf,
    },

    /// Show version information.
    Version,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let level = if cli.verbose { "info" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .with_target(false)
        .without_time()
        .init();

    match cli.command {
        Some(Command::Run { prompt, path, review }) => {
            let prompt_str = prompt.join(" ");
            let project_root = std::fs::canonicalize(&path)
                .unwrap_or_else(|_| path.clone());
            commands::run::run(&prompt_str, &project_root, review)
        }
        Some(Command::Setup { path }) => {
            let project_root = std::fs::canonicalize(&path)
                .unwrap_or_else(|_| path.clone());
            commands::setup::run_setup(&project_root)?;
            Ok(())
        }
        Some(Command::Version) | None => {
            println!("parton v{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
