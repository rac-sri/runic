# SCRIPTS MODULE

**Purpose:** Script discovery and execution for Foundry/Hardhat projects

## STRUCTURE
```
src/scripts/
├── mod.rs            # Exports: Script, ScriptManager, ScriptOutput
└── runner.rs         # ScriptManager with scan() and run() methods
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Script discovery | `runner.rs::ScriptManager::scan()` | Finds scripts in script/ directory |
| Script execution | `runner.rs::ScriptManager::run()` | Spawns forge or hardhat CLI |
| Output handling | `runner.rs::ScriptOutput` struct | Captures stdout, stderr, exit status |

## ANTI-PATTERNS (COMPLEXITY HOTSPOTS)
- `runner.rs::run()` (97 lines) - handles script execution with multiple async operations, error handling, subprocess spawning

## NOTES
- Spawns external CLI tools via tokio::process::Command (forge script, npx hardhat run)
- Foundry: `forge script <path> --rpc-url <url> --private-key <key> -vvv`
- Hardhat: `npx hardhat run <script> --network <network>`
- Private keys passed via environment variables for security
- Script output captured and displayed in TUI
- Runs async, allows cancellation via Ctrl+C
