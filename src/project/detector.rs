use std::path::Path;

use eyre::{eyre, Result};

use super::{foundry, hardhat, Project, ProjectType};

/// Detect the project type based on configuration files present in the directory
pub fn detect(path: &Path) -> Result<Project> {
    let foundry_config = path.join("foundry.toml");
    let hardhat_config_js = path.join("hardhat.config.js");
    let hardhat_config_ts = path.join("hardhat.config.ts");

    // Check for Foundry first (foundry.toml)
    if foundry_config.exists() {
        tracing::info!("Detected Foundry project at {:?}", path);
        return foundry::load_project(path);
    }

    // Check for Hardhat (hardhat.config.js or hardhat.config.ts)
    if hardhat_config_js.exists() || hardhat_config_ts.exists() {
        tracing::info!("Detected Hardhat project at {:?}", path);
        return hardhat::load_project(path);
    }

    Err(eyre!(
        "No Foundry or Hardhat project detected at {:?}\n\
         Expected: foundry.toml, hardhat.config.js, or hardhat.config.ts",
        path
    ))
}

/// Check if a path contains a valid project
#[allow(dead_code)]
pub fn is_valid_project(path: &Path) -> Option<ProjectType> {
    if path.join("foundry.toml").exists() {
        Some(ProjectType::Foundry)
    } else if path.join("hardhat.config.js").exists() || path.join("hardhat.config.ts").exists() {
        Some(ProjectType::Hardhat)
    } else {
        None
    }
}
