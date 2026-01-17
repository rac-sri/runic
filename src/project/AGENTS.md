# PROJECT MODULE

**Purpose:** Foundry/Hardhat project detection and configuration parsing

## STRUCTURE
```
src/project/
├── mod.rs            # Exports: detect(), FoundryConfig, HardhatConfig; defines ProjectType enum
├── detector.rs       # Project type detection logic
├── foundry.rs        # FoundryConfig parsing from foundry.toml
└── hardhat.rs       # HardhatConfig parsing from hardhat.config.js
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Auto-detection | `detector.rs::detect()` | Checks for foundry.toml vs hardhat.config.js in project root |
| Foundry config | `foundry.rs::FoundryConfig` | Parsed from toml file (solc_version, src, test dirs) |
| Hardhat config | `hardhat.rs::HardhatConfig` | Parsed from JS config (networks, solidity version) |
| Project type enum | `mod.rs::ProjectType` | Foundry or Hardhat variant, affects artifact paths |
| Project struct | `mod.rs::Project` | Holds project_type, root path, and specific config |

## NOTES
- Project type determined by config file presence: `foundry.toml` → Foundry, `hardhat.config.js` → Hardhat
- Stores absolute project root path for resolving relative file paths
- No complexity hotspots (all functions under 50 lines)
