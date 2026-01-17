use eyre::{Result, WrapErr};
use keyring::Entry;
use zeroize::Zeroizing;

const SERVICE_NAME: &str = "runic";

/// Manager for secure credential storage using OS keychain
pub struct KeychainManager {
    service: String,
}

impl KeychainManager {
    pub fn new() -> Self {
        Self {
            service: SERVICE_NAME.to_string(),
        }
    }

    /// Store a secret in the keychain
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        let entry = Entry::new(&self.service, key)
            .wrap_err_with(|| format!("Failed to create keychain entry for '{}'. Check keychain access permissions.", key))?;

        entry
            .set_password(value)
            .wrap_err_with(|| format!("Failed to store secret for '{}'. The keychain may have denied access or the entry already exists with different permissions.", key))?;

        tracing::info!("Stored secret in keychain: service={}, key={}", self.service, key);
        Ok(())
    }

    /// Retrieve a secret from the keychain
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let entry = Entry::new(&self.service, key)
            .wrap_err_with(|| format!("Failed to access keychain entry for {}", key))?;

        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e).wrap_err_with(|| format!("Failed to retrieve secret for {}", key)),
        }
    }

    /// Retrieve a secret with zeroization for sensitive data
    pub fn get_zeroizing(&self, key: &str) -> Result<Option<Zeroizing<String>>> {
        self.get(key).map(|opt| opt.map(Zeroizing::new))
    }

    /// Delete a secret from the keychain
    pub fn delete(&self, key: &str) -> Result<()> {
        let entry = Entry::new(&self.service, key)
            .wrap_err_with(|| format!("Failed to access keychain entry for {}", key))?;

        match entry.delete_credential() {
            Ok(()) => {
                tracing::info!("Deleted secret from keychain: {}", key);
                Ok(())
            }
            Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
            Err(e) => Err(e).wrap_err_with(|| format!("Failed to delete secret for {}", key)),
        }
    }

    /// Check if a key exists in the keychain
    pub fn exists(&self, key: &str) -> Result<bool> {
        let entry = Entry::new(&self.service, key)
            .wrap_err_with(|| format!("Failed to access keychain entry for {}", key))?;

        match entry.get_password() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(e).wrap_err_with(|| format!("Failed to check secret for {}", key)),
        }
    }
}

impl Default for KeychainManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Store a private key securely
/// Automatically detects and normalizes 0x prefix
pub fn store_private_key(name: &str, key: &str) -> Result<()> {
    // Validate and normalize key format
    let clean_key = key.trim().strip_prefix("0x").unwrap_or(key.trim());
    let clean_name = name.trim();
    
    if clean_key.len() != 64 || !clean_key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(eyre::eyre!(
            "Invalid private key format: expected 64 hex characters (with or without 0x prefix)"
        ));
    }

    // Always store with 0x prefix for consistency
    let key_to_store = format!("0x{}", clean_key);

    let km = KeychainManager::new();
    km.set(clean_name, &key_to_store)
}

/// Retrieve a private key from secure storage
pub fn get_private_key(name: &str) -> Result<Option<Zeroizing<String>>> {
    let km = KeychainManager::new();
    km.get_zeroizing(name)
}

/// Store an RPC URL securely
pub fn store_rpc_url(name: &str, url: &str) -> Result<()> {
    // Validate URL format
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(eyre::eyre!(
            "Invalid RPC URL format: must start with http:// or https://"
        ));
    }

    let km = KeychainManager::new();
    let key = format!("rpc:{}", name);
    km.set(&key, url)
}

/// Retrieve an RPC URL from secure storage
pub fn get_rpc_url(name: &str) -> Result<Option<String>> {
    let km = KeychainManager::new();
    let key = format!("rpc:{}", name);
    km.get(&key)
}

/// Store an API key securely
pub fn store_api_key(service: &str, key: &str) -> Result<()> {
    let km = KeychainManager::new();
    let keychain_key = format!("api:{}", service);
    km.set(&keychain_key, key)
}

/// Retrieve an API key from secure storage
pub fn get_api_key(service: &str) -> Result<Option<String>> {
    let km = KeychainManager::new();
    let keychain_key = format!("api:{}", service);
    km.get(&keychain_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require keychain access and may prompt for permissions
    // They are marked as ignored by default

    #[test]
    #[ignore]
    fn test_keychain_roundtrip() {
        let km = KeychainManager::new();
        let key = "test_runic_key";
        let value = "test_secret_value";

        // Store
        km.set(key, value).unwrap();

        // Retrieve
        let retrieved = km.get(key).unwrap();
        assert_eq!(retrieved, Some(value.to_string()));

        // Delete
        km.delete(key).unwrap();

        // Verify deleted
        let after_delete = km.get(key).unwrap();
        assert_eq!(after_delete, None);
    }
}
