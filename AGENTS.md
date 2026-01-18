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

## DEVELOPMENT WORKFLOW

### Setup
1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Clone and build: `cargo build --release`
3. Run: `cargo run -- --path .`

### Daily Development
- Use `cargo check` for fast feedback
- Format code: `cargo fmt`
- Lint: `cargo clippy -- -D warnings`
- Test: `cargo test`

## BUILD AND TEST COMMANDS

### Building
```bash
cargo build                    # Debug build
cargo build --release         # Optimized release build
cargo check                   # Check without building
```

### Testing
```bash
cargo test                    # Run all tests
cargo test -- --nocapture     # Show test output
cargo test test_function_name # Run specific test function
cargo test --lib              # Run library tests only
cargo test --doc              # Run doc tests
```

### Linting and Formatting
```bash
cargo fmt                     # Format code
cargo clippy                  # Lint with Clippy
cargo clippy -- -D warnings   # Treat warnings as errors
```

### Running the Application
```bash
cargo run -- --path /path/to/project  # Run with specific project
cargo run -- --setup                 # Force setup wizard
```

## CODE STYLE GUIDELINES

### General Principles
- Follow Rust standard conventions (rustfmt, clippy)
- Prefer clarity over brevity
- Use meaningful names
- Avoid magic numbers/strings
- Document complex logic with comments

### Imports
```rust
// Order: std, external crates, local modules
use std::collections::HashMap;
use eyre::Result;
use crate::config::AppConfig;
```

- Group imports logically
- Use `use` for commonly used types
- Avoid wildcard imports (`use::*`)

### Naming Conventions
- **Functions/Methods**: `snake_case`
- **Types/Structs/Enums**: `PascalCase`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Fields/Variables**: `snake_case`
- **Modules**: `snake_case`

### Types and Safety
- Use strong typing with `#[derive(Debug, Clone)]` where appropriate
- Prefer `&str` over `String` for parameters when possible
- Use `zeroize::Zeroizing<String>` for sensitive data
- Avoid `as` casts; use `try_into()` or explicit conversions
- Never use `unwrap()` in production code; use `?` or `expect("reason")`

### Error Handling
- Use `eyre::Result<T>` for all fallible operations
- Propagate errors with `?`
- Use `eyre::eyre!()` for custom error messages
- Wrap errors with context: `.wrap_err("context")`

### Async Code
- Use `tokio` runtime
- Mark I/O functions as `async fn`
- Use `await` for async operations
- Prefer `tokio::spawn` for concurrent tasks

### Testing
- Write unit tests in `#[cfg(test)]` modules
- Use descriptive test names
- Test both success and error paths
- Mock external dependencies when possible

### Security
- Never log or store private keys
- Use OS keychain for sensitive data
- Validate user inputs
- Use secure random generation

### Code Organization
- One concept per function (max 50 lines)
- Use modules for logical grouping
- Export via `mod.rs` with `pub use`
- Keep dependencies minimal

### Comments and Documentation
- Use `///` for public API documentation
- Explain "why" not "what" in comments
- Avoid obvious comments
- Document assumptions and edge cases

## TESTING GUIDELINES

### Unit Tests
- Place in `#[cfg(test)] mod tests { ... }`
- Test functions: `fn test_something()`
- Use `assert_eq!`, `assert!`, etc.
- Test error conditions

### Integration Tests
- Place in `tests/` directory
- Test full workflows
- Use real dependencies when safe

### Running Specific Tests
```bash
# Run all tests in a module
cargo test mod_name::

# Run specific test
cargo test test_function_name

# Run tests matching pattern
cargo test pattern
```

### Test Organization
- Test both happy path and error cases
- Use fixtures for complex setup
- Clean up after tests
- Avoid flaky tests

## NOTES
- Project type auto-detected from foundry.toml or hardhat.config.js
- CLI args: `--path <dir>`, `--project-type foundry|hardhat`, `--setup`, `--no-setup`
- TUI navigation: `i` (interact), `s` (scripts), `c` (config), `q` / `Ctrl+C` (quit), vim-style `j/k` for nav
- Keychain references use prefix `runic:` (e.g., `keychain: "runic:dev_wallet"`)
- Use `color-eyre` for error reporting
- Config stored in `~/.config/runic/` (or equivalent)
- Use `tracing` for logging