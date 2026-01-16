mod app;
mod config;
mod contracts;
mod project;
mod scripts;
mod ui;

use std::path::PathBuf;

use clap::Parser;
use color_eyre::Result;

#[derive(Parser, Debug)]
#[command(name = "runic")]
#[command(about = "TUI for Foundry and Hardhat smart contract interaction")]
#[command(version)]
struct Cli {
    /// Path to the project directory
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Skip project detection and force a specific project type
    #[arg(long, value_parser = ["foundry", "hardhat"])]
    project_type: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let project_path = cli.path.canonicalize().unwrap_or(cli.path);

    // Initialize tracing for debug logs (only in debug builds)
    #[cfg(debug_assertions)]
    {
        use tracing_subscriber::{fmt, prelude::*, EnvFilter};
        tracing_subscriber::registry()
            .with(fmt::layer().with_target(false))
            .with(EnvFilter::from_default_env())
            .init();
    }

    // Detect project type
    let project = match cli.project_type.as_deref() {
        Some("foundry") => project::Project::new_foundry(&project_path)?,
        Some("hardhat") => project::Project::new_hardhat(&project_path)?,
        _ => project::detect(&project_path)?,
    };

    // Run the TUI application
    app::run(project).await
}
