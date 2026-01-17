# CONTRACTS MODULE

**Purpose:** Ethereum contract interaction, ABI parsing, and deployment management

## STRUCTURE
```
src/contracts/
├── mod.rs            # Exports: ContractFunction, FunctionParam, parse_abi, ContractCaller, Deployment, DeploymentManager
├── abi.rs            # ABI parsing from Solidity JSON artifacts
├── caller.rs         # ContractCaller with async read/write methods (alloy)
└── deployment.rs     # DeploymentManager with chain ID to network mapping
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| ABI parsing | `abi.rs::parse_abi()` | Reads Solidity JSON artifacts from foundry/hardhat artifact dirs |
| Contract calls | `caller.rs::call_read/write()` | Async methods using alloy RPC provider |
| Deployment scanning | `deployment.rs::DeploymentManager::scan()` | Discovers deployed contracts from artifacts |
| Chain ID mapping | `deployment.rs` | Maps chain IDs to network names for display |
| Parameter encoding/decoding | `caller.rs` | Encodes inputs, decodes outputs for contract calls |

## ANTI-PATTERNS (COMPLEXITY HOTSPOTS)
- `caller.rs::decode_single_output()` (60 lines) - complex parameter decoding with deep match statements

## NOTES
- Uses alloy crate for Ethereum RPC interactions (provider, types, sol_types)
- Reads ABIs from: `broadcast/**/deployments/*` (Foundry) or `deployments/` (Hardhat)
- ContractCaller holds provider and signer for async calls
- Private keys loaded via KeychainManager, not from config files
