use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use eyre::{Result, WrapErr};
use serde::Deserialize;
use serde_json::Value;

use super::abi::{ContractFunction, parse_abi};
use crate::config::load_chain_names;
use crate::project::Project;

static CHAIN_NAMES: OnceLock<std::collections::HashMap<u64, String>> = OnceLock::new();

pub fn chain_id_to_network(chain_id: u64) -> String {
    let chain_names = CHAIN_NAMES.get_or_init(|| load_chain_names().unwrap_or_default());

    chain_names
        .get(&chain_id)
        .cloned()
        .unwrap_or_else(|| format!("chain-{}", chain_id))
}

/// Represents a deployed contract
#[derive(Debug, Clone)]
pub struct Deployment {
    pub name: String,
    pub address: String,
    pub callable_address: String, // Address to use for calls (proxy if available)
    pub network: String,
    pub chain_id: u64,
    pub tx_hash: Option<String>,
    pub abi_path: Option<PathBuf>,
    pub functions: Vec<ContractFunction>,
    pub args: Option<Vec<String>>,
    pub is_proxy: bool,                        // Whether this contract is behind a proxy
    pub implementation_set: bool,              // Whether the user has confirmed/set the implementation
}

/// Manager for scanning and tracking deployed contracts
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
    arguments: Option<Vec<Value>>,
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

    fn parse_run_file(&mut self, path: &PathBuf) -> Result<()> {
        // Extract chain ID from path
        // path is like .../chain_id/run-latest.json
        let chain_id_str = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .ok_or_else(|| eyre::eyre!("Invalid path structure"))?;
        
        let chain_id = chain_id_str.parse::<u64>().wrap_err("Failed to parse chain ID")?;
        let network = chain_id_to_network(chain_id);

        let content = fs::read_to_string(path)?;
        let run: BroadcastRun = serde_json::from_str(&content)?;

        if let Some(transactions) = run.transactions {
            for tx in transactions {
                if tx.transaction_type == "CREATE" {
                    if let (Some(name), Some(address)) = (tx.contract_name, tx.contract_address) {
                        // Find ABI
                        // Typical foundry structure: out/ContractName.sol/ContractName.json
                        // Or sometimes just out/ContractName.json depending on config, but standard is nested.
                        // We'll try the nested one first.
                        let mut abi_path = self.out_dir.join(format!("{}.sol", name)).join(format!("{}.json", name));
                        
                        if !abi_path.exists() {
                             abi_path = self.out_dir.join(format!("{}.json", name));
                        }

                        let functions = if abi_path.exists() {
                             if let Ok(content) = fs::read_to_string(&abi_path) {
                                 if let Ok(json) = serde_json::from_str::<Value>(&content) {
                                     // Check if it's a Foundry artifact with "abi" field
                                     let abi_json = if let Some(abi) = json.get("abi") {
                                         abi
                                     } else {
                                         &json
                                     };
                                     parse_abi(abi_json).unwrap_or_default()
                                 } else {
                                     vec![]
                                 }
                             } else {
                                 vec![]
                             }
                        } else {
                            vec![]
                        };
                        
                        let args = tx.arguments.map(|args| {
                            args.iter()
                                .map(|arg| {
                                    if let Some(s) = arg.as_str() {
                                        s.to_string()
                                    } else {
                                        arg.to_string()
                                    }
                                })
                                .collect()
                        });

                        self.deployments.push(Deployment {
                            name: name.clone(),
                            address: address.clone(),
                            callable_address: address, // Default to address
                            network: network.clone(),
                            chain_id,
                            tx_hash: tx.hash,
                            abi_path: if abi_path.exists() { Some(abi_path) } else { None },
                            functions,
                            args,
                            is_proxy: false,
                            implementation_set: false,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Process deployments to handle proxy patterns
    /// For contracts with proxy deployments, set the callable address to the proxy
    fn process_proxy_deployments(&mut self) {
        // Map of (base_name, chain_id) -> indices
        let mut base_to_deployments: HashMap<(String, u64), Vec<usize>> = HashMap::new();
        // Map of (address, chain_id) -> index
        let mut address_to_index: HashMap<(String, u64), usize> = HashMap::new();

        for (i, deployment) in self.deployments.iter().enumerate() {
            let base_name = if deployment.name.ends_with("Proxy") {
                deployment.name.trim_end_matches("Proxy").to_string()
            } else {
                deployment.name.clone()
            };
            
            base_to_deployments
                .entry((base_name, deployment.chain_id))
                .or_insert(Vec::new())
                .push(i);
                
            address_to_index.insert((deployment.address.clone(), deployment.chain_id), i);
        }

        // 1. Name-based matching (existing logic)
        for deployments in base_to_deployments.values() {
            if deployments.len() >= 2 {
                let mut proxy_idx = None;
                let mut impl_idx = None;

                for &idx in deployments {
                    let deployment = &self.deployments[idx];
                    if deployment.name.ends_with("Proxy") {
                        proxy_idx = Some(idx);
                    } else {
                        impl_idx = Some(idx);
                    }
                }

                if let (Some(proxy_idx), Some(impl_idx)) = (proxy_idx, impl_idx) {
                    self.deployments[impl_idx].callable_address =
                        self.deployments[proxy_idx].address.clone();
                    self.deployments[impl_idx].is_proxy = true;

                    self.deployments[proxy_idx].name =
                        format!("{}_hidden", self.deployments[proxy_idx].name);
                }
            }
        }
        
        // 2. Argument-based matching (ERC1967/Transparent proxies)
        // Check if any deployment has an argument that matches another deployment's address
        let mut links = Vec::new();
        for (proxy_idx, deployment) in self.deployments.iter().enumerate() {
            // Only consider deployments that haven't been hidden yet (or even if they have, maybe they are proxies?)
            // And usually proxies have arguments.
            if let Some(args) = &deployment.args {
                if !args.is_empty() {
                    // Check first argument for implementation address
                    let potential_impl = &args[0];
                    if let Some(&impl_idx) = address_to_index.get(&(potential_impl.clone(), deployment.chain_id)) {
                        if impl_idx != proxy_idx {
                             links.push((proxy_idx, impl_idx));
                        }
                    }
                }
            }
        }
        
        for (proxy_idx, impl_idx) in links {
             // If we found a link, update the implementation to use proxy address
             // Only if we haven't already updated it (or maybe we want to overwrite?)
             // Let's assume argument-based linking is strong.

             // Check if we already handled this via name matching
             if !self.deployments[proxy_idx].name.ends_with("_hidden") {
                  self.deployments[impl_idx].callable_address =
                      self.deployments[proxy_idx].address.clone();
                  self.deployments[impl_idx].is_proxy = true;

                  self.deployments[proxy_idx].name =
                      format!("{}_hidden", self.deployments[proxy_idx].name);
             }
        }
    }

    /// Scan for deployments in the broadcast directory
    /// Returns a list of chain IDs that don't have configured networks
    pub fn scan(&mut self) -> Result<Vec<u64>> {
        self.deployments.clear();

        if !self.broadcast_dir.exists() {
            tracing::info!(
                "Broadcast directory does not exist: {:?}",
                self.broadcast_dir
            );
            return Ok(vec![]);
        }

        // Walk through broadcast directory structure:
        // broadcast/<ScriptName>.s.sol/<ChainId>/run-latest.json
        self.scan_broadcast_dir(&self.broadcast_dir.clone())?;

        // Post-process deployments to handle proxies
        self.process_proxy_deployments();

        tracing::info!("Found {} deployments", self.deployments.len());

        // Return list of unique chain IDs found in deployments
        let chain_ids: Vec<u64> = self
            .deployments
            .iter()
            .map(|d| d.chain_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        Ok(chain_ids)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_to_network() {
        assert_eq!(chain_id_to_network(99999), "chain-99999");
    }

    #[test]
    fn test_process_proxy_deployments() {
        let project_root = PathBuf::from(".");
        let mut manager = DeploymentManager {
            deployments: vec![
                Deployment {
                    name: "Counter".to_string(),
                    address: "0xImpl".to_string(),
                    callable_address: "0xImpl".to_string(),
                    network: "localhost".to_string(),
                    chain_id: 31337,
                    tx_hash: None,
                    abi_path: None,
                    functions: vec![],
                    args: None,
                    is_proxy: false,
                    implementation_set: false,
                },
                Deployment {
                    name: "CounterProxy".to_string(),
                    address: "0xProxy".to_string(),
                    callable_address: "0xProxy".to_string(),
                    network: "localhost".to_string(),
                    chain_id: 31337,
                    tx_hash: None,
                    abi_path: None,
                    functions: vec![],
                    args: None,
                    is_proxy: false,
                    implementation_set: false,
                },
            ],
            project_root: project_root.clone(),
            broadcast_dir: project_root.join("broadcast"),
            out_dir: project_root.join("out"),
        };

        manager.process_proxy_deployments();

        assert_eq!(manager.deployments[0].callable_address, "0xProxy");
        assert!(manager.deployments[0].is_proxy);
        assert_eq!(manager.deployments[1].name, "CounterProxy_hidden");
    }

    #[test]
    fn test_process_proxy_deployments_cross_chain() {
        let project_root = PathBuf::from(".");
        let mut manager = DeploymentManager {
            deployments: vec![
                Deployment {
                    name: "Counter".to_string(),
                    address: "0xImpl".to_string(),
                    callable_address: "0xImpl".to_string(),
                    network: "localhost".to_string(),
                    chain_id: 1,
                    tx_hash: None,
                    abi_path: None,
                    functions: vec![],
                    args: None,
                    is_proxy: false,
                    implementation_set: false,
                },
                Deployment {
                    name: "CounterProxy".to_string(),
                    address: "0xProxy".to_string(),
                    callable_address: "0xProxy".to_string(),
                    network: "localhost".to_string(),
                    chain_id: 2,
                    tx_hash: None,
                    abi_path: None,
                    functions: vec![],
                    args: None,
                    is_proxy: false,
                    implementation_set: false,
                },
            ],
            project_root: project_root.clone(),
            broadcast_dir: project_root.join("broadcast"),
            out_dir: project_root.join("out"),
        };

        manager.process_proxy_deployments();

        // Should NOT match across chains
        assert_eq!(manager.deployments[0].callable_address, "0xImpl");
        assert!(!manager.deployments[0].is_proxy);
        assert_eq!(manager.deployments[1].name, "CounterProxy");
    }

    #[test]
    fn test_process_proxy_deployments_with_args() {
        let project_root = PathBuf::from(".");
        let mut manager = DeploymentManager {
            deployments: vec![
                Deployment {
                    name: "Counter".to_string(),
                    address: "0xImpl".to_string(),
                    callable_address: "0xImpl".to_string(),
                    network: "localhost".to_string(),
                    chain_id: 31337,
                    tx_hash: None,
                    abi_path: None,
                    functions: vec![],
                    args: None,
                    is_proxy: false,
                    implementation_set: false,
                },
                Deployment {
                    name: "ERC1967Proxy".to_string(),
                    address: "0xProxy".to_string(),
                    callable_address: "0xProxy".to_string(),
                    network: "localhost".to_string(),
                    chain_id: 31337,
                    tx_hash: None,
                    abi_path: None,
                    functions: vec![],
                    args: Some(vec!["0xImpl".to_string(), "0xData".to_string()]),
                    is_proxy: false,
                    implementation_set: false,
                },
            ],
            project_root: project_root.clone(),
            broadcast_dir: project_root.join("broadcast"),
            out_dir: project_root.join("out"),
        };

        manager.process_proxy_deployments();

        // Should match based on argument pointing to 0xImpl
        assert_eq!(manager.deployments[0].callable_address, "0xProxy");
        assert!(manager.deployments[0].is_proxy);
        assert_eq!(manager.deployments[1].name, "ERC1967Proxy_hidden");
    }
}
