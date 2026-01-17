use alloy::{
    network::EthereumWallet,
    primitives::{Address, Bytes, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol_types::SolValue,
};
use eyre::{Result, WrapErr};
use zeroize::Zeroizing;

use super::abi::ContractFunction;

/// Handles contract calls via Alloy
pub struct ContractCaller {
    rpc_url: String,
    #[allow(dead_code)]
    chain_id: u64,
    signer: Option<PrivateKeySigner>,
}

/// Result of a contract call
#[derive(Debug)]
pub enum CallResult {
    /// Read call result (view/pure)
    Read(Vec<String>),
    /// Write call result (transaction hash)
    Write(String),
    /// Error during call
    #[allow(dead_code)]
    Error(String),
}

impl ContractCaller {
    pub fn new(rpc_url: &str, chain_id: u64) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            chain_id,
            signer: None,
        }
    }

    /// Set the signer for write transactions
    pub fn with_signer(mut self, private_key: Zeroizing<String>) -> Result<Self> {
        let key_str = private_key.as_str();
        let clean_key = key_str.strip_prefix("0x").unwrap_or(key_str);

        let signer: PrivateKeySigner = clean_key.parse().wrap_err("Failed to parse private key")?;

        self.signer = Some(signer);
        Ok(self)
    }

    /// Execute a read-only call (view/pure function)
    pub async fn call_read(
        &self,
        contract_address: &str,
        function: &ContractFunction,
        params: &[String],
    ) -> Result<CallResult> {
        let provider = ProviderBuilder::new()
            .connect(&self.rpc_url)
            .await
            .wrap_err("Failed to connect to RPC")?;

        let address: Address = contract_address
            .parse()
            .wrap_err("Invalid contract address")?;

        // Encode the call data
        let calldata = encode_call_data(function, params)?;

        let tx = TransactionRequest::default()
            .to(address)
            .input(calldata.into());

        let result = provider.call(tx).await.wrap_err("Call failed")?;

        // Decode the result
        let decoded = decode_result(function, &result)?;

        Ok(CallResult::Read(decoded))
    }

    /// Execute a write transaction
    pub async fn call_write(
        &self,
        contract_address: &str,
        function: &ContractFunction,
        params: &[String],
        value: Option<U256>,
    ) -> Result<CallResult> {
        let signer = self
            .signer
            .clone()
            .ok_or_else(|| eyre::eyre!("No signer configured for write transaction"))?;

        let wallet = EthereumWallet::from(signer);

        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect(&self.rpc_url)
            .await
            .wrap_err("Failed to connect to RPC")?;

        let address: Address = contract_address
            .parse()
            .wrap_err("Invalid contract address")?;

        // Encode the call data
        let calldata = encode_call_data(function, params)?;

        let mut tx = TransactionRequest::default()
            .to(address)
            .input(calldata.into());

        if let Some(v) = value {
            tx = tx.value(v);
        }

        let pending_tx = provider
            .send_transaction(tx)
            .await
            .wrap_err("Failed to send transaction")?;

        let tx_hash = format!("{:?}", pending_tx.tx_hash());

        Ok(CallResult::Write(tx_hash))
    }

    /// Determine if a function is a read or write operation
    pub fn is_read_only(function: &ContractFunction) -> bool {
        matches!(function.state_mutability.as_str(), "view" | "pure")
    }
}

/// Encode call data for a function call
fn encode_call_data(function: &ContractFunction, params: &[String]) -> Result<Vec<u8>> {
    use alloy::primitives::keccak256;

    // Calculate function selector
    let signature = super::abi::function_signature(function);
    let selector = &keccak256(signature.as_bytes())[..4];

    // Encode parameters
    let mut calldata = selector.to_vec();

    for (i, param) in function.inputs.iter().enumerate() {
        let value = params.get(i).map(|s| s.as_str()).unwrap_or("");
        let encoded = encode_param(&param.param_type, value)?;
        calldata.extend(encoded);
    }

    Ok(calldata)
}

/// Encode a single parameter value
fn encode_param(param_type: &str, value: &str) -> Result<Vec<u8>> {
    match param_type {
        "address" => {
            let addr: Address = value.parse().wrap_err("Invalid address")?;
            Ok(addr.abi_encode())
        }
        "uint256" | "uint" => {
            let num: U256 = if value.starts_with("0x") {
                value.parse().wrap_err("Invalid uint256")?
            } else {
                U256::from_str_radix(value, 10).wrap_err("Invalid uint256")?
            };
            Ok(num.abi_encode())
        }
        "bool" => {
            let b = value.to_lowercase() == "true" || value == "1";
            Ok(b.abi_encode())
        }
        "bytes32" => {
            let bytes: alloy::primitives::B256 = value.parse().wrap_err("Invalid bytes32")?;
            Ok(bytes.abi_encode())
        }
        "bytes" => {
            let hex_str = value.strip_prefix("0x").unwrap_or(value);
            let bytes = hex::decode(hex_str).wrap_err("Invalid bytes")?;
            Ok(Bytes::from(bytes).abi_encode())
        }
        "string" => Ok(value.to_string().abi_encode()),
        t if t.starts_with("uint") => {
            // Handle uint8, uint16, etc.
            let num: U256 = if value.starts_with("0x") {
                value.parse().wrap_err("Invalid uint")?
            } else {
                U256::from_str_radix(value, 10).wrap_err("Invalid uint")?
            };
            Ok(num.abi_encode())
        }
        t if t.starts_with("int") => {
            // Handle int types (simplified - treating as uint for now)
            let num: U256 = if value.starts_with("0x") {
                value.parse().wrap_err("Invalid int")?
            } else {
                U256::from_str_radix(value, 10).wrap_err("Invalid int")?
            };
            Ok(num.abi_encode())
        }
        _ => Err(eyre::eyre!("Unsupported parameter type: {}", param_type)),
    }
}

/// Decode result bytes based on function outputs
fn decode_result(function: &ContractFunction, data: &Bytes) -> Result<Vec<String>> {
    if function.outputs.is_empty() || data.is_empty() {
        return Ok(vec![]);
    }

    let mut results = Vec::new();
    let mut offset = 0;

    for output in &function.outputs {
        let (decoded, consumed) = decode_single_output(&output.param_type, data, offset)?;
        results.push(decoded);
        offset += consumed;
    }

    Ok(results)
}

fn decode_single_output(param_type: &str, data: &Bytes, offset: usize) -> Result<(String, usize)> {
    let slice = &data[offset..];

    match param_type {
        "address" => {
            if slice.len() < 32 {
                return Err(eyre::eyre!("Insufficient data for address"));
            }
            let addr = Address::from_slice(&slice[12..32]);
            Ok((format!("{:?}", addr), 32))
        }
        "uint256" | "uint" => {
            if slice.len() < 32 {
                return Err(eyre::eyre!("Insufficient data for uint256"));
            }
            let num = U256::from_be_slice(&slice[..32]);
            Ok((num.to_string(), 32))
        }
        "bool" => {
            if slice.len() < 32 {
                return Err(eyre::eyre!("Insufficient data for bool"));
            }
            let b = slice[31] != 0;
            Ok((b.to_string(), 32))
        }
        "bytes32" => {
            if slice.len() < 32 {
                return Err(eyre::eyre!("Insufficient data for bytes32"));
            }
            let hex = format!("0x{}", hex::encode(&slice[..32]));
            Ok((hex, 32))
        }
        "string" => {
            // String is dynamic - has offset, then length, then data
            if slice.len() < 32 {
                return Err(eyre::eyre!("Insufficient data for string offset"));
            }
            let str_offset = U256::from_be_slice(&slice[..32]).to::<usize>();
            let len_slice = &data[str_offset..];
            let len = U256::from_be_slice(&len_slice[..32]).to::<usize>();
            let str_data = &len_slice[32..32 + len];
            let s = String::from_utf8_lossy(str_data).to_string();
            Ok((s, 32))
        }
        t if t.starts_with("uint") => {
            if slice.len() < 32 {
                return Err(eyre::eyre!("Insufficient data for {}", t));
            }
            let num = U256::from_be_slice(&slice[..32]);
            Ok((num.to_string(), 32))
        }
        _ => {
            // Unknown type - return hex
            if slice.len() < 32 {
                return Ok(("0x".to_string(), slice.len()));
            }
            let hex = format!("0x{}", hex::encode(&slice[..32]));
            Ok((hex, 32))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_read_only() {
        let view_func = ContractFunction {
            name: "balanceOf".to_string(),
            inputs: vec![],
            outputs: vec![],
            state_mutability: "view".to_string(),
        };
        assert!(ContractCaller::is_read_only(&view_func));

        let write_func = ContractFunction {
            name: "transfer".to_string(),
            inputs: vec![],
            outputs: vec![],
            state_mutability: "nonpayable".to_string(),
        };
        assert!(!ContractCaller::is_read_only(&write_func));
    }
}
