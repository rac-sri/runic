use std::collections::HashMap;
use std::fs;
use std::path::Path;

use eyre::{Result, WrapErr, eyre};
use serde::{Deserialize, Serialize};

use super::{Project, ProjectConfig, ProjectType};

/// Foundry configuration parsed from foundry.toml
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FoundryConfig {
    #[serde(default)]
    pub profile: HashMap<String, ProfileConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub src: Option<String>,
    pub out: Option<String>,
    pub libs: Option<Vec<String>>,
    pub script: Option<String>,
    pub broadcast: Option<String>,
    pub solc: Option<String>,
    pub optimizer: Option<bool>,
    pub optimizer_runs: Option<u32>,
    pub evm_version: Option<String>,
    pub remappings: Option<Vec<String>>,
    #[serde(default)]
    pub rpc_endpoints: HashMap<String, String>,
    #[serde(default)]
    pub etherscan: HashMap<String, EtherscanConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EtherscanConfig {
    pub key: Option<String>,
    pub url: Option<String>,
    pub chain: Option<u64>,
}

impl FoundryConfig {
    pub fn default_profile(&self) -> &ProfileConfig {
        use std::sync::LazyLock;
        static DEFAULT: LazyLock<ProfileConfig> = LazyLock::new(|| ProfileConfig {
            src: None,
            out: None,
            libs: None,
            script: None,
            broadcast: None,
            solc: None,
            optimizer: None,
            optimizer_runs: None,
            evm_version: None,
            remappings: None,
            rpc_endpoints: HashMap::new(),
            etherscan: HashMap::new(),
        });
        self.profile.get("default").unwrap_or(&DEFAULT)
    }

    pub fn src_dir(&self) -> &str {
        self.default_profile().src.as_deref().unwrap_or("src")
    }

    pub fn out_dir(&self) -> &str {
        self.default_profile().out.as_deref().unwrap_or("out")
    }

    pub fn script_dir(&self) -> &str {
        self.default_profile().script.as_deref().unwrap_or("script")
    }

    pub fn broadcast_dir(&self) -> &str {
        self.default_profile()
            .broadcast
            .as_deref()
            .unwrap_or("broadcast")
    }
}

/// Load a Foundry project from the given path
pub fn load_project(path: &Path) -> Result<Project> {
    let config_path = path.join("foundry.toml");

    if !config_path.exists() {
        return Err(eyre!("foundry.toml not found at {:?}", path));
    }

    let config_content = fs::read_to_string(&config_path)
        .wrap_err_with(|| format!("Failed to read {:?}", config_path))?;

    let config: FoundryConfig =
        toml::from_str(&config_content).wrap_err("Failed to parse foundry.toml")?;

    // Extract project name from directory name
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(Project {
        project_type: ProjectType::Foundry,
        root: path.to_path_buf(),
        name,
        src_dir: path.join(config.src_dir()),
        out_dir: path.join(config.out_dir()),
        script_dir: path.join(config.script_dir()),
        broadcast_dir: path.join(config.broadcast_dir()),
        config: ProjectConfig::Foundry(config),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_foundry_toml() {
        let content = r#"
[profile.default]
src = "src"
out = "out"
"#;
        let config: FoundryConfig = toml::from_str(content).unwrap();
        assert_eq!(config.src_dir(), "src");
        assert_eq!(config.out_dir(), "out");
    }

    #[test]
    fn test_parse_foundry_toml_with_rpc() {
        let content = r#"
[profile.default]
src = "src"
out = "out"

[profile.default.rpc_endpoints]
mainnet = "https://eth-mainnet.g.alchemy.com/v2/xxx"
sepolia = "https://eth-sepolia.g.alchemy.com/v2/xxx"
"#;
        let config: FoundryConfig = toml::from_str(content).unwrap();
        assert!(
            config
                .default_profile()
                .rpc_endpoints
                .contains_key("mainnet")
        );
    }
}
