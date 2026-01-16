use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};

const CONFIG_DIR: &str = "runic";
const CONFIG_FILE: &str = "config.toml";

/// Application configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub networks: HashMap<String, NetworkConfig>,

    #[serde(default)]
    pub wallets: HashMap<String, WalletConfig>,

    #[serde(default)]
    pub api_keys: HashMap<String, String>,

    #[serde(default)]
    pub defaults: Option<Defaults>,

    #[serde(skip)]
    config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub rpc_url: String,
    pub chain_id: Option<u64>,
    pub explorer_url: Option<String>,
    pub explorer_api_key: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Reference to keychain entry (e.g., "runic:dev_wallet")
    pub keychain: Option<String>,
    /// Environment variable containing private key
    pub env_var: Option<String>,
    /// Optional label for display
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Defaults {
    pub network: Option<String>,
    pub wallet: Option<String>,
}

impl AppConfig {
    /// Load configuration from default location or create default
    pub fn load() -> Result<Self> {
        let config_path = Self::default_config_path()?;

        if config_path.exists() {
            Self::load_from(&config_path)
        } else {
            Ok(Self {
                config_path: Some(config_path),
                ..Default::default()
            })
        }
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path)
            .wrap_err_with(|| format!("Failed to read config file: {:?}", path))?;

        let mut config: AppConfig =
            toml::from_str(&content).wrap_err("Failed to parse config file")?;

        config.config_path = Some(path.clone());
        Ok(config)
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<()> {
        let path = self
            .config_path
            .clone()
            .or_else(|| Self::default_config_path().ok())
            .ok_or_else(|| eyre::eyre!("No config path available"))?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .wrap_err_with(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        let content = toml::to_string_pretty(self).wrap_err("Failed to serialize config")?;

        fs::write(&path, content)
            .wrap_err_with(|| format!("Failed to write config file: {:?}", path))?;

        Ok(())
    }

    /// Get the config file path
    pub fn config_path(&self) -> Option<PathBuf> {
        self.config_path.clone().or_else(|| Self::default_config_path().ok())
    }

    /// Get the default configuration file path
    fn default_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| eyre::eyre!("Could not determine config directory"))?;

        Ok(config_dir.join(CONFIG_DIR).join(CONFIG_FILE))
    }

    /// Get a network by name, falling back to default
    pub fn get_network(&self, name: Option<&str>) -> Option<(&String, &NetworkConfig)> {
        if let Some(name) = name {
            self.networks.get_key_value(name)
        } else if let Some(defaults) = &self.defaults {
            if let Some(default_name) = &defaults.network {
                self.networks.get_key_value(default_name)
            } else {
                self.networks.iter().next()
            }
        } else {
            self.networks.iter().next()
        }
    }

    /// Resolve an API key value (handling keychain references)
    pub fn resolve_api_key(&self, name: &str) -> Result<Option<String>> {
        let value = match self.api_keys.get(name) {
            Some(v) => v,
            None => return Ok(None),
        };

        if let Some(keychain_ref) = value.strip_prefix("keychain:") {
            use super::KeychainManager;
            let km = KeychainManager::new();
            km.get(keychain_ref)
        } else {
            Ok(Some(value.clone()))
        }
    }

    /// Resolve a wallet private key
    pub fn resolve_wallet_key(&self, name: &str) -> Result<Option<zeroize::Zeroizing<String>>> {
        let wallet = match self.wallets.get(name) {
            Some(w) => w,
            None => return Ok(None),
        };

        if let Some(keychain_ref) = &wallet.keychain {
            use super::KeychainManager;
            let km = KeychainManager::new();
            km.get(keychain_ref).map(|opt| opt.map(zeroize::Zeroizing::new))
        } else if let Some(env_var) = &wallet.env_var {
            Ok(std::env::var(env_var).ok().map(zeroize::Zeroizing::new))
        } else {
            Ok(None)
        }
    }
}

/// Create a default configuration with common networks
pub fn create_default_config() -> AppConfig {
    let mut networks = HashMap::new();

    networks.insert(
        "mainnet".to_string(),
        NetworkConfig {
            rpc_url: "https://eth.llamarpc.com".to_string(),
            chain_id: Some(1),
            explorer_url: Some("https://etherscan.io".to_string()),
            explorer_api_key: Some("keychain:etherscan_mainnet".to_string()),
        },
    );

    networks.insert(
        "sepolia".to_string(),
        NetworkConfig {
            rpc_url: "https://sepolia.drpc.org".to_string(),
            chain_id: Some(11155111),
            explorer_url: Some("https://sepolia.etherscan.io".to_string()),
            explorer_api_key: Some("keychain:etherscan_sepolia".to_string()),
        },
    );

    AppConfig {
        networks,
        wallets: HashMap::new(),
        api_keys: HashMap::new(),
        defaults: Some(Defaults {
            network: Some("sepolia".to_string()),
            wallet: None,
        }),
        config_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let content = r#"
[networks.mainnet]
rpc_url = "https://eth.llamarpc.com"
chain_id = 1

[networks.sepolia]
rpc_url = "https://sepolia.drpc.org"
chain_id = 11155111

[defaults]
network = "sepolia"

[api_keys]
etherscan = "keychain:etherscan_api"
"#;

        let config: AppConfig = toml::from_str(content).unwrap();
        assert_eq!(config.networks.len(), 2);
        assert!(config.networks.contains_key("mainnet"));
        assert_eq!(
            config.defaults.as_ref().unwrap().network,
            Some("sepolia".to_string())
        );
    }
}
