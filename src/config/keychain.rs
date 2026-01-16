use eyre::{Result, WrapErr};
use keyring::Entry;
use zeroize::Zeroizing;

const SERVICE_NAME: &str = "runic";

/// Manager for secure credential storage using the OS keychain
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
            .wrap_err_with(|| format!("Failed to create keychain entry for {}", key))?;

        entry
            .set_password(value)
            .wrap_err_with(|| format!("Failed to store secret for {}", key))?;

        tracing::info!("Stored secret in keychain: {}", key);
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
/// The key should be a hex string (with or without 0x prefix)
pub fn store_private_key(name: &str, key: &str) -> Result<()> {
    // Validate key format
    let clean_key = key.strip_prefix("0x").unwrap_or(key);
    if clean_key.len() != 64 || !clean_key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(eyre::eyre!(
            "Invalid private key format: expected 64 hex characters"
        ));
    }

    let km = KeychainManager::new();
    km.set(name, key)
}

/// Retrieve a private key from secure storage
pub fn get_private_key(name: &str) -> Result<Option<Zeroizing<String>>> {
    let km = KeychainManager::new();
    km.get_zeroizing(name)
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
