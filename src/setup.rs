use std::io::{self, Write};

use eyre::{Result, WrapErr};

use crate::config::{AppConfig, KeychainManager, NetworkConfig, WalletConfig};

/// Run interactive setup if configuration is missing or incomplete
pub fn run_setup_if_needed(config: &mut AppConfig) -> Result<bool> {
    // Check if we have minimum required config
    let has_network = !config.networks.is_empty();
    let has_wallet = !config.wallets.is_empty();

    if has_network && has_wallet {
        return Ok(false); // No setup needed
    }

    println!("\n┌─────────────────────────────────────────┐");
    println!("│       runic - First Time Setup          │");
    println!("└─────────────────────────────────────────┘\n");

    if !has_network {
        println!("No networks configured. Let's add one.\n");
        setup_network(config)?;
    }

    if !has_wallet {
        println!("\nNo wallets configured. Let's add one.\n");
        setup_wallet(config)?;
    }

    // Optionally setup etherscan API key
    println!("\n── Optional: Etherscan API Key ──");
    println!("An Etherscan API key enables contract verification.");
    if prompt_yes_no("Add Etherscan API key?")? {
        setup_etherscan(config)?;
    }

    // Save configuration
    config.save().wrap_err("Failed to save configuration")?;

    println!("\n✓ Configuration saved successfully!");
    println!(
        "  Config file: {}",
        config
            .config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
    println!("\nYou can modify settings anytime in the Config view (press 'c').\n");

    Ok(true)
}

fn setup_network(config: &mut AppConfig) -> Result<()> {
    println!("── Network Configuration ──\n");

    // Network name
    let name = prompt_string("Network name (e.g., sepolia, mainnet, localhost)")?;

    // RPC URL
    println!("\nEnter the RPC URL for {}:", name);
    println!("  Examples:");
    println!("    - https://eth-sepolia.g.alchemy.com/v2/YOUR_KEY");
    println!("    - https://sepolia.infura.io/v3/YOUR_KEY");
    println!("    - http://localhost:8545");
    let rpc_url = prompt_string("RPC URL")?;

    // Chain ID
    let chain_id = prompt_chain_id(&name)?;

    // Explorer URL (optional)
    let explorer_url = match name.to_lowercase().as_str() {
        "mainnet" => Some("https://etherscan.io".to_string()),
        "sepolia" => Some("https://sepolia.etherscan.io".to_string()),
        "goerli" => Some("https://goerli.etherscan.io".to_string()),
        "polygon" => Some("https://polygonscan.com".to_string()),
        "arbitrum" => Some("https://arbiscan.io".to_string()),
        "optimism" => Some("https://optimistic.etherscan.io".to_string()),
        "base" => Some("https://basescan.org".to_string()),
        _ => None,
    };

    config.networks.insert(
        name.clone(),
        NetworkConfig {
            rpc_url,
            chain_id: Some(chain_id),
            explorer_url,
            explorer_api_key: None,
        },
    );

    // Set as default if it's the first network
    if config.defaults.is_none() {
        config.defaults = Some(crate::config::Defaults {
            network: Some(name.clone()),
            wallet: None,
        });
    }

    println!("\n✓ Network '{}' configured", name);
    Ok(())
}

fn setup_wallet(config: &mut AppConfig) -> Result<()> {
    println!("── Wallet Configuration ──\n");
    println!("Your private key will be stored securely in the system keychain.");
    println!("It will NEVER be written to any config file.\n");

    // Wallet name
    let name = prompt_string("Wallet name (e.g., dev, deployer, main)")?;

    // Private key
    println!("\nEnter your private key (with or without 0x prefix):");
    let private_key = prompt_secret("Private key")?;

    // Validate private key format
    let clean_key = private_key
        .trim()
        .strip_prefix("0x")
        .unwrap_or(private_key.trim());
    if clean_key.len() != 64 || !clean_key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(eyre::eyre!(
            "Invalid private key format. Expected 64 hex characters."
        ));
    }

    // Store in keychain
    let keychain_key = format!("wallet_{}", name);
    let km = KeychainManager::new();
    km.set(&keychain_key, clean_key)
        .wrap_err("Failed to store private key in keychain")?;

    config.wallets.insert(
        name.clone(),
        WalletConfig {
            keychain: Some(format!("runic:{}", keychain_key)),
            env_var: None,
            label: Some(format!("{} wallet", name)),
        },
    );

    // Set as default if it's the first wallet
    if let Some(defaults) = &mut config.defaults {
        if defaults.wallet.is_none() {
            defaults.wallet = Some(name.clone());
        }
    }

    println!(
        "\n✓ Wallet '{}' configured and stored in system keychain",
        name
    );
    Ok(())
}

fn setup_etherscan(config: &mut AppConfig) -> Result<()> {
    println!("\nEnter your Etherscan API key:");
    println!("  Get one at: https://etherscan.io/myapikey");
    let api_key = prompt_secret("API key")?;

    // Store in keychain
    let km = KeychainManager::new();
    km.set("etherscan_api", api_key.trim())
        .wrap_err("Failed to store API key in keychain")?;

    config.api_keys.insert(
        "etherscan".to_string(),
        "keychain:etherscan_api".to_string(),
    );

    println!("\n✓ Etherscan API key stored in system keychain");
    Ok(())
}

fn prompt_string(prompt: &str) -> Result<String> {
    print!("{}: ", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        return Err(eyre::eyre!("Input cannot be empty"));
    }

    Ok(trimmed)
}

fn prompt_secret(prompt: &str) -> Result<String> {
    print!("{}: ", prompt);
    io::stdout().flush()?;

    // Try to disable echo for password input
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let stdin_fd = io::stdin().as_raw_fd();

        // Get current terminal settings
        let mut termios = unsafe {
            let mut t = std::mem::zeroed();
            if libc::tcgetattr(stdin_fd, &mut t) == 0 {
                Some(t)
            } else {
                None
            }
        };

        // Disable echo
        if let Some(ref mut t) = termios {
            let original = *t;
            t.c_lflag &= !libc::ECHO;
            unsafe { libc::tcsetattr(stdin_fd, libc::TCSANOW, t) };

            let mut input = String::new();
            let result = io::stdin().read_line(&mut input);

            // Restore terminal
            unsafe { libc::tcsetattr(stdin_fd, libc::TCSANOW, &original) };
            println!(); // New line after hidden input

            result?;
            return Ok(input.trim().to_string());
        }
    }

    // Fallback: read normally (echo visible)
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_yes_no(prompt: &str) -> Result<bool> {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn prompt_chain_id(network_name: &str) -> Result<u64> {
    // Try to guess chain ID from network name
    let suggested = match network_name.to_lowercase().as_str() {
        "mainnet" | "ethereum" => Some(1),
        "goerli" => Some(5),
        "sepolia" => Some(11155111),
        "polygon" | "matic" => Some(137),
        "mumbai" => Some(80001),
        "arbitrum" | "arbitrum-one" => Some(42161),
        "arbitrum-goerli" => Some(421613),
        "optimism" => Some(10),
        "optimism-goerli" => Some(420),
        "base" => Some(8453),
        "base-goerli" => Some(84531),
        "localhost" | "anvil" | "hardhat" => Some(31337),
        _ => None,
    };

    if let Some(id) = suggested {
        print!("Chain ID [{}]: ", id);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().is_empty() {
            return Ok(id);
        }

        input.trim().parse().wrap_err("Invalid chain ID")
    } else {
        print!("Chain ID: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        input.trim().parse().wrap_err("Invalid chain ID")
    }
}

/// Check if configuration is complete for running scripts
pub fn is_config_complete(config: &AppConfig) -> bool {
    !config.networks.is_empty() && !config.wallets.is_empty()
}
