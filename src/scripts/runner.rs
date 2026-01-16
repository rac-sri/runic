use std::fs;
use std::path::PathBuf;
use std::process::Stdio;

use eyre::{Result, WrapErr};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::project::Project;

/// Represents a Foundry script
#[derive(Debug, Clone)]
pub struct Script {
    pub name: String,
    pub path: PathBuf,
    pub description: Option<String>,
    pub contract_name: Option<String>,
}

/// Output from running a script
#[derive(Debug, Clone)]
pub struct ScriptOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
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

    /// Scan for scripts in the script directory
    pub fn scan(&mut self) -> Result<()> {
        self.scripts.clear();

        if !self.script_dir.exists() {
            tracing::info!("Script directory does not exist: {:?}", self.script_dir);
            return Ok(());
        }

        self.scan_dir(&self.script_dir.clone())?;

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
            } else if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".s.sol"))
            {
                if let Some(script) = self.parse_script(&path) {
                    self.scripts.push(script);
                }
            }
        }

        Ok(())
    }

    fn parse_script(&self, path: &PathBuf) -> Option<Script> {
        let file_name = path.file_name()?.to_string_lossy().to_string();
        let name = file_name.strip_suffix(".s.sol")?.to_string();

        // Try to extract description from NatSpec comments
        let content = fs::read_to_string(path).ok()?;
        let description = extract_natspec_description(&content);
        let contract_name = extract_contract_name(&content);

        Some(Script {
            name,
            path: path.clone(),
            description,
            contract_name,
        })
    }

    /// Run a script using forge
    pub async fn run(
        &self,
        script: &Script,
        network: &str,
        rpc_url: &str,
        broadcast: bool,
        verify: bool,
        private_key: Option<&str>,
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

        // Pass private key via env var for security (not on command line)
        if let Some(pk) = private_key {
            cmd.env("PRIVATE_KEY", pk);
            cmd.arg("--private-key").arg("$PRIVATE_KEY");
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        tracing::info!("Running script: {:?}", script.name);

        let mut child = cmd.spawn().wrap_err("Failed to spawn forge command")?;

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
                            stderr_output.push_str(&l);
                            stderr_output.push('\n');
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!("Error reading stderr: {}", e);
                        }
                    }
                }
            }
        }

        let status = child.wait().await.wrap_err("Failed to wait for forge")?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_natspec() {
        let content = r#"
// SPDX-License-Identifier: MIT
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
        let content = r#"
contract DeployToken is Script {
    function run() external {
    }
}
"#;

        assert_eq!(
            extract_contract_name(content),
            Some("DeployToken".to_string())
        );
    }
}
