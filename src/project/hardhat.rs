use std::path::Path;

use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};

use super::{Project, ProjectConfig, ProjectType};

/// Hardhat configuration (stub for MVP - Foundry-first approach)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HardhatConfig {
    // TODO: Parse hardhat.config.js/ts when Hardhat support is added
}

/// Load a Hardhat project from the given path
pub fn load_project(path: &Path) -> Result<Project> {
    let config_js = path.join("hardhat.config.js");
    let config_ts = path.join("hardhat.config.ts");

    if !config_js.exists() && !config_ts.exists() {
        return Err(eyre!(
            "hardhat.config.js or hardhat.config.ts not found at {:?}",
            path
        ));
    }

    // Extract project name from directory name
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Hardhat uses conventional directories
    Ok(Project {
        project_type: ProjectType::Hardhat,
        root: path.to_path_buf(),
        name,
        src_dir: path.join("contracts"),
        out_dir: path.join("artifacts"),
        script_dir: path.join("scripts"),
        broadcast_dir: path.join("deployments"),
        config: ProjectConfig::Hardhat(HardhatConfig::default()),
    })
}
