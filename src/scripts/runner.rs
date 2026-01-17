use std::fs;
use std::path::PathBuf;
use std::process::Stdio;

use eyre::{Result, WrapErr};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::AppConfig;
use crate::project::Project;

/// Type of script
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptType {
    Foundry,
    Hardhat,
}

/// Represents a Foundry or Hardhat script
#[derive(Debug, Clone)]
pub struct Script {
    pub name: String,
    pub path: PathBuf,
    pub description: Option<String>,
    pub contract_name: Option<String>,
    pub script_type: ScriptType,
}

/// Output from running a script
#[derive(Debug, Clone)]
pub struct ScriptOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl From<ScriptOutput> for String {
    fn from(output: ScriptOutput) -> Self {
        output.stdout
    }
}

/// Manages script discovery and execution
pub struct ScriptManager {
    pub scripts: Vec<Script>,
    script_dir: PathBuf,
    project_root: PathBuf,
}

impl ScriptManager {
    pub fn new(project: &Project) -> Self {
        Self {
            scripts: Vec::new(),
            script_dir: project.script_dir.clone(),
            project_root: project.root.clone(),
        }
    }

    /// Scan for scripts in the script directory and standard 'scripts' directory
    pub fn scan(&mut self) -> Result<()> {
        self.scripts.clear();

        // Scan configured script directory (usually 'script' for Foundry)
        if self.script_dir.exists() {
            self.scan_dir(&self.script_dir.clone())?;
        }

        // Also scan 'scripts' directory if it exists and is different (common for Hardhat)
        let hardhat_scripts = self.project_root.join("scripts");
        if hardhat_scripts.exists() && hardhat_scripts != self.script_dir {
            self.scan_dir(&hardhat_scripts)?;
        }

        // Sort scripts by name
        self.scripts.sort_by(|a, b| a.name.cmp(&b.name));

        tracing::info!("Found {} scripts", self.scripts.len());
        Ok(())
    }

    fn scan_dir(&mut self, dir: &PathBuf) -> Result<()> {
        let entries = fs::read_dir(dir).wrap_err_with(|| format!("Failed to read {:?}", dir))?;

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                self.scan_dir(&path)?;
            } else {
                if let Some(script) = self.parse_script(&path) {
                    self.scripts.push(script);
                }
            }
        }

        Ok(())
    }

    fn parse_script(&self, path: &PathBuf) -> Option<Script> {
        let file_name = path.file_name()?.to_string_lossy().to_string();

        if file_name.ends_with(".s.sol") {
            // Foundry script
            let name = file_name.strip_suffix(".s.sol")?.to_string();
            let content = fs::read_to_string(path).ok()?;
            let description = extract_natspec_description(&content);
            let contract_name = extract_contract_name(&content);

            return Some(Script {
                name,
                path: path.clone(),
                description,
                contract_name,
                script_type: ScriptType::Foundry,
            });
        } else if file_name.ends_with(".js") || file_name.ends_with(".ts") {
            // Hardhat script (exclude config files)
            if file_name.contains("hardhat.config") {
                return None;
            }

            let name = if let Some(stripped) = file_name.strip_suffix(".js") {
                stripped.to_string()
            } else {
                file_name.strip_suffix(".ts")?.to_string()
            };

            return Some(Script {
                name,
                path: path.clone(),
                description: None, // TODO: Parse JS comments
                contract_name: None,
                script_type: ScriptType::Hardhat,
            });
        }

        None
    }

    /// Run a script
    pub async fn run(
        &self,
        script: &Script,
        network: &str,
        rpc_url: &str,
        broadcast: bool,
        verify: bool,
        private_key: Option<&str>,
        tx: Option<UnboundedSender<String>>,
    ) -> Result<ScriptOutput> {
        match script.script_type {
            ScriptType::Foundry => {
                self.run_foundry(script, rpc_url, broadcast, verify, private_key, tx)
                    .await
            }
            ScriptType::Hardhat => {
                self.run_hardhat(script, network, rpc_url, private_key, tx)
                    .await
            }
        }
    }

    async fn run_foundry(
        &self,
        script: &Script,
        rpc_url: &str,
        broadcast: bool,
        verify: bool,
        private_key: Option<&str>,
        tx: Option<UnboundedSender<String>>,
    ) -> Result<ScriptOutput> {
        let script_path = script
            .path
            .strip_prefix(&self.project_root)
            .unwrap_or(&script.path);

        let contract_target = if let Some(contract) = &script.contract_name {
            format!("{}:{}", script_path.display(), contract)
        } else {
            script_path.display().to_string()
        };

        let mut cmd = Command::new("forge");
        cmd.arg("script")
            .arg(&contract_target)
            .arg("--rpc-url")
            .arg(rpc_url)
            .current_dir(&self.project_root);

        if broadcast {
            cmd.arg("--broadcast");
        }

        if verify {
            cmd.arg("--verify");
        }

        // Pass private key directly and via env var
        if let Some(pk) = private_key {
            cmd.env("PRIVATE_KEY", pk);
            cmd.arg("--private-key").arg(pk);
        }

        self.execute_command(cmd, &script.name, tx).await
    }

    async fn run_hardhat(
        &self,
        script: &Script,
        network: &str,
        rpc_url: &str,
        private_key: Option<&str>,
        tx: Option<UnboundedSender<String>>,
    ) -> Result<ScriptOutput> {
        let script_path = script
            .path
            .strip_prefix(&self.project_root)
            .unwrap_or(&script.path);

        let mut cmd = Command::new("npx");
        cmd.arg("hardhat")
            .arg("run")
            .arg(script_path)
            .current_dir(&self.project_root);

        // Special handling: if network is "custom" or "env", skip --network flag
        // and let script use RPC_URL env var directly
        if network != "custom" && network != "env" {
            cmd.arg("--network").arg(network);
        }

        // Pass RPC URL and private key as environment variables
        // Scripts can use these directly with:
        //   const provider = new ethers.JsonRpcProvider(process.env.RPC_URL)
        //   const wallet = new ethers.Wallet(process.env.PRIVATE_KEY, provider)
        cmd.env("RPC_URL", rpc_url);
        cmd.env("NETWORK_URL", rpc_url); // Alternative name some scripts use

        if let Some(pk) = private_key {
            cmd.env("PRIVATE_KEY", pk);
        }

        self.execute_command(cmd, &script.name, tx).await
    }

    async fn execute_command(
        &self,
        mut cmd: Command,
        script_name: &str,
        tx: Option<UnboundedSender<String>>,
    ) -> Result<ScriptOutput> {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        tracing::info!("Running script: {:?}", script_name);

        let mut child = cmd.spawn().wrap_err("Failed to spawn command")?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        let mut stdout_lines = stdout_reader.lines();
        let mut stderr_lines = stderr_reader.lines();

        let mut stdout_output = String::new();
        let mut stderr_output = String::new();

        // Read output concurrently
        loop {
            tokio::select! {
                line = stdout_lines.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            if let Some(tx) = &tx {
                                let _ = tx.send(l.clone());
                            }
                            stdout_output.push_str(&l);
                            stdout_output.push('\n');
                        }
                        Ok(None) => break,
                        Err(e) => {
                            tracing::warn!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
                line = stderr_lines.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            if let Some(tx) = &tx {
                                let _ = tx.send(l.clone());
                            }
                            stderr_output.push_str(&l);
                            stderr_output.push('\n');
                        }
                        Ok(None) => {} // stderr might close before stdout
                        Err(e) => {
                            tracing::warn!("Error reading stderr: {}", e);
                        }
                    }
                }
            }
        }

        let status = child.wait().await.wrap_err("Failed to wait for command")?;

        Ok(ScriptOutput {
            success: status.success(),
            stdout: stdout_output,
            stderr: stderr_output,
        })
    }

    /// Get a script by index
    pub fn get(&self, index: usize) -> Option<&Script> {
        self.scripts.get(index)
    }

    /// Get a script by name
    pub fn get_by_name(&self, name: &str) -> Option<&Script> {
        self.scripts.iter().find(|s| s.name == name)
    }

    /// Run a script with secure credential resolution from config
    pub async fn run_with_secure_config(
        &self,
        script: &Script,
        network_name: &str,
        config: &AppConfig,
        broadcast: bool,
        verify: bool,
        tx: Option<UnboundedSender<String>>,
    ) -> Result<ScriptOutput> {
        let rpc_url = match config.resolve_rpc_url(network_name)? {
            Some(url) => url,
            None => {
                return Err(eyre::eyre!(
                    "Network '{}' not found in config",
                    network_name
                ));
            }
        };

        // Try to resolve private key from:
        // 1. Configured default wallet in config
        // 2. PRIVATE_KEY environment variable as fallback
        let private_key = if let Some(default_wallet) =
            config.defaults.as_ref().and_then(|d| d.wallet.as_ref())
        {
            match config.resolve_wallet_key(default_wallet)? {
                Some(key) => normalize_private_key(&key),
                None => {
                    tracing::warn!("No private key found for wallet: {}", default_wallet);
                    // Fall back to PRIVATE_KEY env var
                    std::env::var("PRIVATE_KEY")
                        .ok()
                        .and_then(|k| normalize_private_key(&k))
                }
            }
        } else {
            // No default wallet configured, try PRIVATE_KEY env var
            std::env::var("PRIVATE_KEY")
                .ok()
                .and_then(|k| normalize_private_key(&k))
        };

        if private_key.is_none() {
            let msg = "WARNING: No private key available. Set PRIVATE_KEY env var or configure a default wallet with 'k' in config screen.";
            tracing::warn!("{}", msg);
            if let Some(ref tx) = tx {
                let _ = tx.send(msg.to_string());
            }
        }

        self.run(
            script,
            network_name,
            &rpc_url,
            broadcast,
            verify,
            private_key.as_deref(),
            tx,
        )
        .await
    }

    /// Run a script with explicit wallet selection
    /// If wallet_name is None, uses PRIVATE_KEY environment variable
    pub async fn run_with_wallet(
        &self,
        script: &Script,
        network_name: &str,
        wallet_name: Option<&str>,
        config: &AppConfig,
        broadcast: bool,
        verify: bool,
        tx: Option<UnboundedSender<String>>,
    ) -> Result<ScriptOutput> {
        let rpc_url = match config.resolve_rpc_url(network_name)? {
            Some(url) => url,
            None => {
                return Err(eyre::eyre!(
                    "Network '{}' not found in config",
                    network_name
                ));
            }
        };

        // Resolve private key based on wallet selection
        let private_key = if let Some(wallet) = wallet_name {
            // User selected a specific wallet
            let wallet_config = config.wallets.get(wallet);

            match config.resolve_wallet_key(wallet)? {
                Some(key) => normalize_private_key(&key),
                None => {
                    // Provide specific error message based on wallet configuration
                    let reason = if let Some(wc) = wallet_config {
                        if let Some(keychain_ref) = &wc.keychain {
                            format!(
                                "Keychain entry '{}' not found. Re-add the wallet with 'k' in config screen.",
                                keychain_ref
                            )
                        } else if let Some(env_var) = &wc.env_var {
                            format!("Environment variable '{}' is not set.", env_var)
                        } else {
                            "Wallet has no keychain or env_var configured.".to_string()
                        }
                    } else {
                        format!("Wallet '{}' not found in config.", wallet)
                    };

                    return Err(eyre::eyre!(
                        "No private key for wallet '{}': {}",
                        wallet,
                        reason
                    ));
                }
            }
        } else {
            // User chose to use PRIVATE_KEY env var
            match std::env::var("PRIVATE_KEY") {
                Ok(key) => match normalize_private_key(&key) {
                    Some(k) => Some(k),
                    None => {
                        return Err(eyre::eyre!(
                            "PRIVATE_KEY environment variable is set but invalid. Expected 64 hex characters."
                        ));
                    }
                },
                Err(_) => {
                    return Err(eyre::eyre!("PRIVATE_KEY environment variable is not set."));
                }
            }
        };

        self.run(
            script,
            network_name,
            &rpc_url,
            broadcast,
            verify,
            private_key.as_deref(),
            tx,
        )
        .await
    }
}

/// Extract description from NatSpec @notice or @title
fn extract_natspec_description(content: &str) -> Option<String> {
    // Look for @title or @notice in NatSpec comments
    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(desc) = trimmed.strip_prefix("/// @title") {
            return Some(desc.trim().to_string());
        }
        if let Some(desc) = trimmed.strip_prefix("/// @notice") {
            return Some(desc.trim().to_string());
        }
        if let Some(desc) = trimmed.strip_prefix("* @title") {
            return Some(desc.trim().to_string());
        }
        if let Some(desc) = trimmed.strip_prefix("* @notice") {
            return Some(desc.trim().to_string());
        }
    }

    None
}

/// Extract the main contract name from the script
fn extract_contract_name(content: &str) -> Option<String> {
    // Look for "contract <Name> is Script" pattern
    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("contract ") && trimmed.contains(" is ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                return Some(parts[1].to_string());
            }
        }
    }

    None
}

/// Normalize a private key to the expected format (0x + 64 hex chars)
/// Returns None if the key is invalid
fn normalize_private_key(key: &str) -> Option<String> {
    let trimmed = key.trim();

    // Strip 0x prefix if present to get raw hex
    let raw_hex = trimmed.strip_prefix("0x").unwrap_or(trimmed);

    // Validate: must be exactly 64 hex characters (32 bytes)
    if raw_hex.len() != 64 {
        tracing::warn!(
            "Private key has invalid length ({} hex chars). Expected 64 hex chars (32 bytes).",
            raw_hex.len()
        );
        return None;
    }

    if !raw_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        tracing::warn!("Private key contains non-hex characters.");
        return None;
    }

    // Return with 0x prefix
    Some(format!("0x{}", raw_hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_natspec() {
        let content = r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// @title Deploy Token Contract
/// @notice Deploys the main token
contract DeployToken is Script {
    function run() external {
    }
}
"#;

        assert_eq!(
            extract_natspec_description(content),
            Some("Deploy Token Contract".to_string())
        );
    }

    #[test]
    fn test_extract_contract_name() {
        let content = r#"contract DeployToken is Script {
    function run() external {
    }
}
"#;

        assert_eq!(
            extract_contract_name(content),
            Some("DeployToken".to_string())
        );
    }

    #[test]
    fn test_normalize_private_key() {
        // Valid key with 0x prefix
        let key_with_prefix = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        assert_eq!(
            normalize_private_key(key_with_prefix),
            Some(key_with_prefix.to_string())
        );

        // Valid key without 0x prefix
        let key_without_prefix = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        assert_eq!(
            normalize_private_key(key_without_prefix),
            Some(format!("0x{}", key_without_prefix))
        );

        // Key with whitespace
        let key_with_whitespace =
            "  0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80  ";
        assert!(normalize_private_key(key_with_whitespace).is_some());

        // Invalid: too short
        let short_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478";
        assert!(normalize_private_key(short_key).is_none());

        // Invalid: too long
        let long_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80aa";
        assert!(normalize_private_key(long_key).is_none());
    }
}
