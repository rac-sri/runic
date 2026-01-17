# CONFIG MODULE

**Purpose:** Application configuration, network/wallet settings, and secure key storage

## STRUCTURE
```
src/config/
├── mod.rs            # Exports: KeychainManager, AppConfig, Defaults, NetworkConfig, WalletConfig
├── settings.rs       # AppConfig with load() and save() methods
└── keychain.rs       # KeychainManager wrapping OS keyring
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Config loading | `settings.rs::AppConfig::load()` | Reads from `$XDG_CONFIG_HOME/runic/settings.toml` |
| Config saving | `settings.rs::AppConfig::save()` | Persists to TOML file |
| Secure storage | `keychain.rs::KeychainManager` | OS keychain with get(), set(), delete() methods |
| Default values | `settings.rs::Defaults` struct | Default network and wallet selection |
| Network config | `settings.rs::NetworkConfig` | RPC URL, chain ID per network |
| Wallet config | `settings.rs::WalletConfig` | Wallet name, optional keychain reference |

## NOTES
- Config format: TOML in `$XDG_CONFIG_HOME/runic/settings.toml`
- Secure storage: OS keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- Keychain service prefix: `runic:` (e.g., `runic:dev_wallet`)
- **Private keys NEVER written to config files** - only keychain references stored
- Nested HashMap structure: `HashMap<String, NetworkConfig>` for networks, `HashMap<String, WalletConfig>` for wallets
- No complexity hotspots (all functions under 50 lines)
