use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Represents a contract function from the ABI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractFunction {
    pub name: String,
    pub inputs: Vec<FunctionParam>,
    pub outputs: Vec<FunctionParam>,
    pub state_mutability: String,
}

/// Represents a function parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionParam {
    pub name: String,
    pub param_type: String,
    pub components: Option<Vec<FunctionParam>>,
}

/// Parse an ABI JSON and extract callable functions
pub fn parse_abi(abi_json: &Value) -> Result<Vec<ContractFunction>> {
    let abi_array = abi_json
        .as_array()
        .ok_or_else(|| eyre::eyre!("ABI must be a JSON array"))?;

    let functions: Vec<ContractFunction> = abi_array
        .iter()
        .filter_map(|item| {
            let item_type = item.get("type")?.as_str()?;
            if item_type != "function" {
                return None;
            }

            let name = item.get("name")?.as_str()?.to_string();
            let state_mutability = item
                .get("stateMutability")
                .and_then(|v| v.as_str())
                .unwrap_or("nonpayable")
                .to_string();

            let inputs = parse_params(item.get("inputs"));
            let outputs = parse_params(item.get("outputs"));

            Some(ContractFunction {
                name,
                inputs,
                outputs,
                state_mutability,
            })
        })
        .collect();

    Ok(functions)
}

fn parse_params(params: Option<&Value>) -> Vec<FunctionParam> {
    params
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|param| {
                    let name = param
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();

                    let param_type = param.get("type").and_then(|t| t.as_str())?.to_string();

                    let components = param.get("components").and_then(|c| {
                        if c.is_array() {
                            Some(parse_params(Some(c)))
                        } else {
                            None
                        }
                    });

                    Some(FunctionParam {
                        name,
                        param_type,
                        components,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse ABI from a string
pub fn parse_abi_string(abi_str: &str) -> Result<Vec<ContractFunction>> {
    let abi_json: Value =
        serde_json::from_str(abi_str).wrap_err("Failed to parse ABI as JSON")?;
    parse_abi(&abi_json)
}

/// Get function signature string (for selector calculation)
#[allow(dead_code)]
pub fn function_signature(func: &ContractFunction) -> String {
    let params: Vec<String> = func.inputs.iter().map(encode_param_type).collect();
    format!("{}({})", func.name, params.join(","))
}

fn encode_param_type(param: &FunctionParam) -> String {
    if let Some(components) = &param.components {
        // Tuple type
        let inner: Vec<String> = components.iter().map(encode_param_type).collect();
        format!("({})", inner.join(","))
    } else {
        param.param_type.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_abi() {
        let abi_str = r#"[
            {
                "type": "function",
                "name": "balanceOf",
                "inputs": [{"name": "account", "type": "address"}],
                "outputs": [{"name": "", "type": "uint256"}],
                "stateMutability": "view"
            },
            {
                "type": "function",
                "name": "transfer",
                "inputs": [
                    {"name": "to", "type": "address"},
                    {"name": "amount", "type": "uint256"}
                ],
                "outputs": [{"name": "", "type": "bool"}],
                "stateMutability": "nonpayable"
            }
        ]"#;

        let functions = parse_abi_string(abi_str).unwrap();
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].name, "balanceOf");
        assert_eq!(functions[0].state_mutability, "view");
        assert_eq!(functions[1].name, "transfer");
        assert_eq!(functions[1].inputs.len(), 2);
    }

    #[test]
    fn test_function_signature() {
        let func = ContractFunction {
            name: "transfer".to_string(),
            inputs: vec![
                FunctionParam {
                    name: "to".to_string(),
                    param_type: "address".to_string(),
                    components: None,
                },
                FunctionParam {
                    name: "amount".to_string(),
                    param_type: "uint256".to_string(),
                    components: None,
                },
            ],
            outputs: vec![],
            state_mutability: "nonpayable".to_string(),
        };

        assert_eq!(function_signature(&func), "transfer(address,uint256)");
    }
}
