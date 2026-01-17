# Runic

A powerful Terminal User Interface (TUI) for interacting with smart contracts deployed via [Foundry](https://book.getfoundry.sh/) and [Hardhat](https://hardhat.org/). Execute contract functions, run deployment scripts, and manage blockchain networks and wallets all from your terminal.
<img width="1672" height="1340" alt="Screenshot 2026-01-18 at 12 46 41 AM" src="https://github.com/user-attachments/assets/ef0b8db3-769b-4900-a9ca-fcb270fab71b" /><img width="1672" height="1340" alt="Screenshot 2026-01-18 at 12 46 57 AM" src="https://github.com/user-attachments/assets/f7fc8785-7c9b-4d35-814c-41b6cade5ce0" />


## ✨ Features

- **Interactive Contract Calls**: Browse deployed contracts and call functions with real-time feedback
- **Script Execution**: Run Foundry scripts and Hardhat tasks with network/wallet selection
- **Automatic Chain Detection**: Automatically detects deployed contract chains and configures RPC endpoints
- **Network Management**: Add and manage multiple blockchain networks with secure RPC URL storage
- **Wallet Integration**: Secure wallet management using system keychain
- **Real-time Status**: Live status updates during contract interactions
- **Cross-platform**: Works on macOS, Linux, and Windows

## 🚀 Installation

### Prerequisites

- Rust 1.70+ ([install Rust](https://rustup.rs/))
- Foundry or Hardhat project with deployed contracts

### Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/runic.git
cd runic

# Build the project
cargo build --release

# Run the application
./target/release/runic
```

### Direct Download

Download pre-built binaries from the [releases page](https://github.com/yourusername/runic/releases).

## 📖 Usage

### Basic Usage

```bash
# Run in current directory (auto-detects Foundry/Hardhat)
runic

# Specify project path
runic /path/to/your/project

# Force project type
runic --project-type foundry
runic --project-type hardhat

# Skip setup wizard
runic --no-setup
```

### First Time Setup

On first run, Runic will guide you through:
1. **Project Detection**: Automatically detects Foundry or Hardhat projects
2. **Network Configuration**: Set up RPC endpoints for blockchain networks
3. **Wallet Setup**: Configure wallets for transaction signing

## 🎯 Key Bindings

### Global Navigation
- `i` - Enter **Interact** mode (contract calls)
- `s` - Enter **Scripts** mode (run deployment scripts)
- `c` - Enter **Config** mode (manage networks/wallets)
- `q` / `Ctrl+C` - Quit application

### Interact Mode
- `↑/↓` or `k/j` - Navigate contracts and functions
- `Tab` / `→` - Switch between contract and function panels
- `Enter` - Call selected function
- `Esc` - Go back

### Scripts Mode
- `↑/↓` or `k/j` - Navigate scripts
- `Enter` - Run selected script
- `Esc` - Go back

### Input Mode
- `↑/↓` or `k/j` - Navigate between parameters
- `Tab` - Cycle through parameters
- `Enter` - Submit/Next parameter
- `Esc` - Cancel input

## 🔧 Configuration

Runic stores configuration in `~/.config/runic/settings.toml`:

```toml
[defaults]
network = "mainnet"
wallet = "dev_wallet"

[networks.mainnet]
rpc_url = "keychain:mainnet"
chain_id = 1
explorer_url = "https://etherscan.io"

[networks.arbitrum]
rpc_url = "keychain:arbitrum"
chain_id = 42161
explorer_url = "https://arbiscan.io"

[wallets.dev_wallet]
keychain = "runic:dev_wallet"
label = "Development Wallet"
```

### Managing Networks

In Config mode (`c`):
- Add new networks with custom RPC URLs
- Runic automatically detects deployed contracts and prompts for missing network configurations
- RPC URLs are securely stored in your system keychain

### Managing Wallets

In Config mode (`c`):
- Add wallets using private keys (stored securely in keychain)
- Support for environment variables containing private keys
- Set default wallet for transactions

## 🎮 Interactive Mode

Browse and interact with deployed contracts:

### Contract Discovery
Runic automatically scans Foundry broadcast files (`broadcast/*/run-latest.json`) to discover:
- Deployed contract addresses
- Associated ABIs
- Deployment networks and chain IDs

### Function Calling

1. **Select Contract**: Browse deployed contracts by name and network
2. **Choose Function**: View available functions (marked `[R]` for read-only, `[W]` for write)
3. **Enter Parameters**: Input function parameters with validation
4. **Select Wallet**: For write transactions, choose signing wallet
5. **Execute**: Real-time status updates during execution

### Real-time Feedback

The result panel shows:
- Contract address and network details
- Function inputs
- Execution status (Connecting → Executing → Completed/Failed)
- Transaction hashes for write operations
- Full error messages with stack traces

## 📜 Script Execution

Run deployment scripts with network and wallet selection:

1. **Select Script**: Choose from available Foundry scripts
2. **Choose Network**: Select target blockchain network
3. **Select Wallet**: Choose signing wallet for transactions
4. **Execute**: Monitor script execution with live output

## 🔒 Security Features

- **Secure Key Storage**: Private keys and RPC URLs stored in system keychain
- **No Key Exposure**: Keys never written to disk or displayed in logs
- **Environment Variable Support**: Use environment variables for sensitive data
- **Permission-based Access**: Granular control over wallet and network access

## 🐛 Troubleshooting

### Common Issues

**"No Foundry or Hardhat project detected"**
- Ensure you're in a directory with `foundry.toml` or `hardhat.config.js`
- Use `--project-type` flag to force detection

**"No network configured"**
- Run setup wizard with `--setup` flag
- Add networks manually in Config mode

**"Wallet not found"**
- Configure wallets in Config mode
- Check keychain access permissions

**Contract calls fail**
- Verify RPC URLs are accessible
- Check network connectivity
- Ensure correct wallet selected for write operations

### Debug Mode

Enable detailed logging:

```bash
RUST_LOG=debug runic
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Commit changes: `git commit -m 'Add amazing feature'`
4. Push to branch: `git push origin feature/amazing-feature`
5. Open a Pull Request

### Development Setup

```bash
# Clone and build
git clone https://github.com/yourusername/runic.git
cd runic
cargo build

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Foundry](https://book.getfoundry.sh/) - The blazing fast Ethereum development toolkit
- [Hardhat](https://hardhat.org/) - Ethereum development environment
- [ratatui](https://github.com/tui-rs-revival/ratatui) - Rust TUI library
- [alloy](https://github.com/alloy-rs/alloy) - Rust Ethereum library

---

**Made with ❤️ for the Ethereum ecosystem**</content>
<parameter name="filePath">README.md
