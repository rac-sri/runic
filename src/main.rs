mod app;
mod config;
mod contracts;
mod project;
mod scripts;
mod setup;
mod ui;

use std::path::PathBuf;

use clap::Parser;
use color_eyre::Result;

use crate::config::AppConfig;

#[derive(Parser, Debug)]
#[command(name = "runic")]
#[command(about = "TUI for Foundry and Hardhat smart contract interaction")]
#[command(version)]
struct Cli {
    /// Path to the project directory
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Skip project detection and force a specific project type
    #[arg(long, value_parser = ["foundry", "hardhat"])]
    project_type: Option<String>,

    /// Force re-run setup wizard
    #[arg(long)]
    setup: bool,

    /// Skip setup wizard even if config is incomplete
    #[arg(long)]
    no_setup: bool,

    /// Test keychain functionality
    #[arg(long)]
    test_keychain: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let project_path = cli.path.canonicalize().unwrap_or(cli.path);

    // Initialize tracing for debug logs (only in debug builds)
    #[cfg(debug_assertions)]
    {
        use tracing_subscriber::{EnvFilter, fmt, prelude::*};
        tracing_subscriber::registry()
            .with(fmt::layer().with_target(false))
            .with(EnvFilter::from_default_env())
            .init();
    }

    // Test keychain if requested
    if cli.test_keychain {
        return test_keychain();
    }

    // Load or create configuration
    let mut config = AppConfig::load()?;

    #[allow(unused_assignments)]
    // Run setup if needed or forced
    if cli.setup || (!cli.no_setup && !setup::is_config_complete(&config)) {
        setup::run_setup_if_needed(&mut config)?;
        // Reload config after setup
        config = AppConfig::load()?;
    }

    // Detect project type
    let project = match cli.project_type.as_deref() {
        Some("foundry") => project::Project::new_foundry(&project_path)?,
        Some("hardhat") => project::Project::new_hardhat(&project_path)?,
        _ => project::detect(&project_path)?,
    };

    // Run the TUI application
    app::run(project).await
}

fn test_keychain() -> Result<()> {
    use config::{KeychainManager, get_private_key, store_private_key};

    println!("Testing keychain functionality...\n");

    let km = KeychainManager::new();
    let test_key = "__runic_test__";
    let test_value = "test_secret_value_12345";

    // Test 1: Store a value
    println!("1. Storing test value...");
    match km.set(test_key, test_value) {
        Ok(()) => println!("   ✓ Successfully stored value"),
        Err(e) => {
            println!("   ✗ Failed to store: {}", e);
            return Err(e);
        }
    }

    // Test 2: Retrieve the value
    println!("2. Retrieving test value...");
    match km.get(test_key) {
        Ok(Some(v)) if v == test_value => {
            println!("   ✓ Successfully retrieved and verified value")
        }
        Ok(Some(v)) => println!(
            "   ✗ Retrieved value doesn't match: got '{}', expected '{}'",
            v, test_value
        ),
        Ok(None) => println!("   ✗ Value not found in keychain"),
        Err(e) => println!("   ✗ Failed to retrieve: {}", e),
    }

    // Test 3: Delete the value
    println!("3. Deleting test value...");
    match km.delete(test_key) {
        Ok(()) => println!("   ✓ Successfully deleted value"),
        Err(e) => println!("   ✗ Failed to delete: {}", e),
    }

    // Test 4: Verify deletion
    println!("4. Verifying deletion...");
    match km.get(test_key) {
        Ok(None) => println!("   ✓ Value correctly deleted"),
        Ok(Some(_)) => println!("   ✗ Value still exists after deletion"),
        Err(e) => println!("   ✗ Error checking: {}", e),
    }

    // Test 5: Test private key storage
    println!("\n5. Testing private key storage...");
    let test_wallet = "__runic_test_wallet__";
    let test_pk = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    match store_private_key(test_wallet, test_pk) {
        Ok(()) => println!("   ✓ Successfully stored private key"),
        Err(e) => {
            println!("   ✗ Failed to store private key: {}", e);
            return Err(e);
        }
    }

    println!("6. Retrieving private key...");
    match get_private_key(test_wallet) {
        Ok(Some(pk)) => {
            if pk.starts_with("0x") && pk.len() == 66 {
                println!(
                    "   ✓ Successfully retrieved private key (correctly formatted with 0x prefix)"
                );
            } else {
                println!(
                    "   ✗ Private key format issue: len={}, starts_with_0x={}",
                    pk.len(),
                    pk.starts_with("0x")
                );
            }
        }
        Ok(None) => println!("   ✗ Private key not found"),
        Err(e) => println!("   ✗ Failed to retrieve: {}", e),
    }

    // Cleanup
    println!("7. Cleaning up test wallet...");
    let _ = km.delete(test_wallet);
    println!("   ✓ Cleanup complete");

    println!("\n✓ Keychain tests completed!");
    Ok(())
}
