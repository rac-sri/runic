mod detector;
mod foundry;
mod hardhat;

pub use detector::detect;
pub use foundry::FoundryConfig;
pub use hardhat::HardhatConfig;

use std::path::{Path, PathBuf};

use eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectType {
    Foundry,
    Hardhat,
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectType::Foundry => write!(f, "Foundry"),
            ProjectType::Hardhat => write!(f, "Hardhat"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    pub project_type: ProjectType,
    pub root: PathBuf,
    pub name: String,
    pub src_dir: PathBuf,
    pub out_dir: PathBuf,
    pub script_dir: PathBuf,
    pub broadcast_dir: PathBuf,
    #[allow(dead_code)]
    pub config: ProjectConfig,
}

#[derive(Debug, Clone)]
pub enum ProjectConfig {
    #[allow(dead_code)]
    Foundry(FoundryConfig),
    #[allow(dead_code)]
    Hardhat(HardhatConfig),
}

impl Project {
    pub fn new_foundry(path: &Path) -> Result<Self> {
        foundry::load_project(path)
    }

    pub fn new_hardhat(path: &Path) -> Result<Self> {
        hardhat::load_project(path)
    }

    #[allow(dead_code)]
    pub fn is_foundry(&self) -> bool {
        self.project_type == ProjectType::Foundry
    }

    #[allow(dead_code)]
    pub fn is_hardhat(&self) -> bool {
        self.project_type == ProjectType::Hardhat
    }
}
