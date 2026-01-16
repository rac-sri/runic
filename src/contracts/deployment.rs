use std::fs;
use std::path::PathBuf;

use eyre::{Result, WrapErr};
use serde::Deserialize;
use serde_json::Value;

use super::abi::{parse_abi, ContractFunction};
use crate::project::Project;

/// Represents a deployed contract
#[derive(Debug, Clone)]
pub struct Deployment {
    pub name: String,
    pub address: String,
    pub network: String,
    pub chain_id: u64,
    pub tx_hash: Option<String>,
    pub abi_path: Option<PathBuf>,
    pub functions: Vec<ContractFunction>,
}

/// Manages scanning and loading of deployments
pub struct DeploymentManager {
    pub deployments: Vec<Deployment>,
    project_root: PathBuf,
    broadcast_dir: PathBuf,
    out_dir: PathBuf,
}

/// Foundry broadcast run artifact structure
#[derive(Debug, Deserialize)]
struct BroadcastRun {
    transactions: Option<Vec<Transaction>>,
}

#[derive(Debug, Deserialize)]
struct Transaction {
    #[serde(rename = "transactionType")]
    transaction_type: String,
    #[serde(rename = "contractName")]
    contract_name: Option<String>,
    #[serde(rename = "contractAddress")]
    contract_address: Option<String>,
    hash: Option<String>,
}

impl DeploymentManager {
    pub fn new(project: &Project) -> Self {
        Self {
            deployments: Vec::new(),
            project_root: project.root.clone(),
            broadcast_dir: project.broadcast_dir.clone(),
            out_dir: project.out_dir.clone(),
        }
    }

    /// Scan for deployments in the broadcast directory
    pub fn scan(&mut self) -> Result<()> {
        self.deployments.clear();

        if !self.broadcast_dir.exists() {
            tracing::info!("Broadcast directory does not exist: {:?}", self.broadcast_dir);
            return Ok(());
        }

        // Walk through broadcast directory structure:
        // broadcast/<ScriptName>.s.sol/<ChainId>/run-latest.json
        self.scan_broadcast_dir(&self.broadcast_dir.clone())?;

        tracing::info!("Found {} deployments", self.deployments.len());
        Ok(())
    }

    fn scan_broadcast_dir(&mut self, dir: &PathBuf) -> Result<()> {
        let entries = fs::read_dir(dir).wrap_err_with(|| format!("Failed to read {:?}", dir))?;

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Recurse into subdirectories
                self.scan_broadcast_dir(&path)?;
            } else if path.file_name().is_some_and(|n| n == "run-latest.json") {
                // Parse the run file
                if let Err(e) = self.parse_run_file(&path) {
                    tracing::warn!("Failed to parse {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    fn parse_run_file(&mut self, path: &PathBuf) -> Result<()> {
        let content = fs::read_to_string(path)
            .wrap_err_with(|| format!("Failed to read {:?}", path))?;

        let run: BroadcastRun = serde_json::from_str(&content)
            .wrap_err_with(|| format!("Failed to parse {:?}", path))?;

        // Extract chain ID from path (broadcast/<script>/<chain_id>/run-latest.json)
        let chain_id = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let network = chain_id_to_network(chain_id);

        if let Some(transactions) = run.transactions {
            for tx in transactions {
                if tx.transaction_type == "CREATE" || tx.transaction_type == "CREATE2" {
                    if let (Some(name), Some(address)) = (tx.contract_name, tx.contract_address) {
                        // Try to load ABI
                        let (abi_path, functions) = self.load_abi(&name);

                        self.deployments.push(Deployment {
                            name,
                            address,
                            network: network.clone(),
                            chain_id,
                            tx_hash: tx.hash,
                            abi_path,
                            functions,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    fn load_abi(&self, contract_name: &str) -> (Option<PathBuf>, Vec<ContractFunction>) {
        // Try to find ABI in out directory: out/<ContractName>.sol/<ContractName>.json
        let abi_path = self
            .out_dir
            .join(format!("{}.sol", contract_name))
            .join(format!("{}.json", contract_name));

        if !abi_path.exists() {
            return (None, Vec::new());
        }

        let content = match fs::read_to_string(&abi_path) {
            Ok(c) => c,
            Err(_) => return (Some(abi_path), Vec::new()),
        };

        let artifact: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return (Some(abi_path), Vec::new()),
        };

        // Foundry artifact has ABI at .abi
        let abi = artifact.get("abi").unwrap_or(&artifact);

        let functions = parse_abi(abi).unwrap_or_default();

        (Some(abi_path), functions)
    }

    /// Get a deployment by index
    pub fn get(&self, index: usize) -> Option<&Deployment> {
        self.deployments.get(index)
    }

    /// Get a deployment by address
    pub fn get_by_address(&self, address: &str) -> Option<&Deployment> {
        let addr_lower = address.to_lowercase();
        self.deployments
            .iter()
            .find(|d| d.address.to_lowercase() == addr_lower)
    }
}

fn chain_id_to_network(chain_id: u64) -> String {
    match chain_id {
        1 => "mainnet".to_string(),
        5 => "goerli".to_string(),
        11155111 => "sepolia".to_string(),
        137 => "polygon".to_string(),
        80001 => "mumbai".to_string(),
        42161 => "arbitrum".to_string(),
        421613 => "arbitrum-goerli".to_string(),
        10 => "optimism".to_string(),
        420 => "optimism-goerli".to_string(),
        8453 => "base".to_string(),
        84531 => "base-goerli".to_string(),
        31337 => "anvil".to_string(),
        _ => format!("chain-{}", chain_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_to_network() {
        assert_eq!(chain_id_to_network(1), "mainnet");
        assert_eq!(chain_id_to_network(11155111), "sepolia");
        assert_eq!(chain_id_to_network(31337), "anvil");
        assert_eq!(chain_id_to_network(99999), "chain-99999");
    }
}
