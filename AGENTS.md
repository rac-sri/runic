# PROJECT KNOWLEDGE BASE

**Generated:** 2025-01-17
**Commit:** fea68d5
**Branch:** main

## OVERVIEW
TUI for Foundry and Hardhat smart contract interaction - Rust app using ratatui + tokio.

## STRUCTURE
```
./
├── src/
│   ├── app.rs          # Core TUI app state and event loop
│   ├── main.rs         # CLI entry (clap) and TUI launcher
│   ├── setup.rs        # Initial config wizard
│   ├── ui/             # All rendering and view logic (8 files, 806 lines)
│   ├── contracts/      # Ethereum interaction, ABI parsing (4 files, 701 lines)
│   ├── config/          # Settings, keychain, network/wallet config (3 files, 369 lines)
│   ├── project/        # Foundry/Hardhat detection and config (4 files, 308 lines)
│   └── scripts/        # Script execution (2 files, 292 lines)
└── Cargo.toml          # Dependencies: ratatui, tokio, alloy, keyring
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Entry points | `main.rs`, `app.rs::run()` | CLI parsing → TUI init |
| View routing | `app.rs::View` enum + handlers | Home/Interact/Scripts/Config |
| UI rendering | `ui/mod.rs::draw()` | Dispatches to view-specific draw fn |
| Contract calls | `contracts/caller.rs::call_read/write()` | Uses alloy for RPC |
| ABI parsing | `contracts/abi.rs::parse_abi()` | From Solidity JSON artifacts |
| Project detection | `project/mod.rs::detect()` | Foundry (foundry.toml) or Hardhat (hardhat.config.js) |
| Config loading | `config/settings.rs::AppConfig::load()` | Stores in XDG config dir |
| Secure storage | `config/keychain.rs::KeychainManager` | OS keyring (service prefix: `runic:`) |
| Script execution | `scripts/runner.rs::run()` | Spawns `forge script` or `npx hardhat run` |

## CONVENTIONS
- **Error handling**: `Result<T, eyre::Report>` with `?` propagation (uses color-eyre)
- **Async**: tokio runtime, `async fn` for I/O, `await` everywhere
- **Config**: TOML format in `$XDG_CONFIG_HOME/runic/settings.toml`
- **Testing**: Inline `#[cfg(test)]` modules, 10 unit tests total
- **State**: `App` struct holds all state; `View` enum routes to screen-specific handlers
- **Modules**: Standard `mod.rs` pattern with `pub use` re-exports

## ANTI-PATTERNS (THIS PROJECT)
- No CI/build automation (no .github/workflows, no Makefile)
- No integration tests (only inline unit tests)
- Complex hotspots: `scripts/runner.rs:run()` (97 lines), `app.rs:handle_scripts_input()` (76 lines)

## UNIQUE STYLES
- Private keys NEVER written to config (only OS keychain via `keychain: "runic:wallet_name"`)
- View-specific state nested inside `View` enum (`InteractState`, `ScriptsState`)
- Script execution spawns external CLI tools (forge, npx hardhat) via tokio::process::Command

## COMMANDS
```bash
# Development
cargo run -- --path .              # Run TUI
cargo run -- --setup              # Force setup wizard
cargo test                        # Run 10 inline unit tests

# Build
cargo build --release             # LTO + strip enabled in profile.release
```

## NOTES
- Project type auto-detected from foundry.toml or hardhat.config.js
- CLI args: `--path <dir>`, `--project-type foundry|hardhat`, `--setup`, `--no-setup`
- TUI navigation: `i` (interact), `s` (scripts), `c` (config), `q` / `Ctrl+C` (quit), vim-style `j/k` for nav
- Keychain references use prefix `runic:` (e.g., `keychain: "runic:dev_wallet"`)
