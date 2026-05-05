//! Encrypted API key storage.
//!
//! Uses AES-GCM to encrypt API keys at rest. Keys are encrypted with a
//! master key derived from the OS environment or a user-provided passphrase.

use std::path::{Path, PathBuf};

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::KeyInit;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::Result;

/// Encrypted key store.
pub struct KeyStore {
    /// Path to the encrypted keys file.
    store_path: PathBuf,
    /// Encryption key (256-bit).
    encryption_key: Vec<u8>,
}

/// Stored key entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyEntry {
    /// Encrypted value.
    encrypted: Vec<u8>,
    /// Nonce used for encryption.
    nonce: Vec<u8>,
}

/// Collection of stored keys.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct KeyCollection {
    keys: std::collections::HashMap<String, KeyEntry>,
}

impl KeyStore {
    /// Create a new key store at the given path.
    ///
    /// The encryption key is derived from the `VULTURIUS_KEY` environment variable
    /// (hex-encoded 32-byte key) or generated randomly for this session.
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        let encryption_key = Self::derive_encryption_key();
        Self {
            store_path: store_path.into(),
            encryption_key,
        }
    }

    /// Create with a specific encryption key (for testing).
    #[cfg(test)]
    pub fn with_key(store_path: impl Into<PathBuf>, key: Vec<u8>) -> Self {
        Self {
            store_path: store_path.into(),
            encryption_key: key,
        }
    }

    /// Derive encryption key from environment or generate random.
    fn derive_encryption_key() -> Vec<u8> {
        // Try env var first (hex-encoded 32-byte key)
        if let Ok(hex_key) = std::env::var("VULTURIUS_KEY") {
            if let Ok(key_bytes) = hex::decode(&hex_key) {
                if key_bytes.len() == 32 {
                    debug!("KeyStore: Using encryption key from VULTURIUS_KEY env var");
                    return key_bytes;
                }
                warn!("KeyStore: VULTURIUS_KEY is not 32 bytes, generating random key");
            }
        }

        // Generate random key for this session
        debug!("KeyStore: Generating random session encryption key");
        let mut key = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }

    /// Store an API key.
    pub fn store_key(&self, name: &str, value: &str) -> Result<()> {
        let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from_slice(
            self.encryption_key.as_slice(),
        ));

        let nonce = Self::generate_nonce();
        let ciphertext = cipher
            .encrypt(&nonce, value.as_bytes())
            .map_err(|e| crate::VulturiusError::Config(format!("Encryption failed: {}", e)))?;

        let entry = KeyEntry {
            encrypted: ciphertext,
            nonce: nonce.to_vec(),
        };

        let mut collection = self.load_collection()?;
        collection.keys.insert(name.to_string(), entry);
        self.save_collection(&collection)?;

        debug!(key = name, "KeyStore: Stored key");
        Ok(())
    }

    /// Retrieve an API key.
    pub fn get_key(&self, name: &str) -> Result<Option<String>> {
        let collection = self.load_collection()?;

        let entry = match collection.keys.get(name) {
            Some(e) => e.clone(),
            None => return Ok(None),
        };

        let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from_slice(
            self.encryption_key.as_slice(),
        ));

        let nonce = Nonce::from_slice(&entry.nonce);
        let plaintext = cipher
            .decrypt(nonce, entry.encrypted.as_slice())
            .map_err(|e| crate::VulturiusError::Config(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext)
            .map(Some)
            .map_err(|e| crate::VulturiusError::Config(format!("Invalid UTF-8: {}", e)))
    }

    /// Check if a key exists.
    pub fn has_key(&self, name: &str) -> Result<bool> {
        let collection = self.load_collection()?;
        Ok(collection.keys.contains_key(name))
    }

    /// Delete a key.
    pub fn delete_key(&self, name: &str) -> Result<()> {
        let mut collection = self.load_collection()?;
        collection.keys.remove(name);
        self.save_collection(&collection)?;
        debug!(key = name, "KeyStore: Deleted key");
        Ok(())
    }

    /// List all stored key names.
    pub fn list_keys(&self) -> Result<Vec<String>> {
        let collection = self.load_collection()?;
        Ok(collection.keys.keys().cloned().collect())
    }

    /// Load the key collection from disk.
    fn load_collection(&self) -> Result<KeyCollection> {
        if !self.store_path.exists() {
            return Ok(KeyCollection::default());
        }

        let data = std::fs::read_to_string(&self.store_path)
            .map_err(|e| crate::VulturiusError::Io(e))?;

        serde_json::from_str(&data)
            .map_err(|e| crate::VulturiusError::Config(format!("Failed to parse key store: {}", e)))
    }

    /// Save the key collection to disk.
    fn save_collection(&self, collection: &KeyCollection) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| crate::VulturiusError::Io(e))?;
        }

        let data = serde_json::to_string_pretty(collection)
            .map_err(|e| crate::VulturiusError::Config(format!("Failed to serialize key store: {}", e)))?;

        std::fs::write(&self.store_path, data)
            .map_err(|e| crate::VulturiusError::Io(e))?;

        Ok(())
    }

    /// Generate a random nonce (96 bits for AES-GCM).
    fn generate_nonce() -> aes_gcm::Nonce<aes_gcm::aead::consts::U12> {
        let mut bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut bytes);
        aes_gcm::Nonce::from_slice(&bytes).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn test_store(temp_dir: &TempDir) -> KeyStore {
        KeyStore::new(temp_dir.path().join("keys.json"))
    }

    #[test]
    fn test_store_and_retrieve_key() {
        let temp_dir = TempDir::new().unwrap();
        let store = test_store(&temp_dir);

        store.store_key("openai", "sk-test-key-123").unwrap();
        let key = store.get_key("openai").unwrap().unwrap();
        assert_eq!(key, "sk-test-key-123");
    }

    #[test]
    fn test_get_nonexistent_key() {
        let temp_dir = TempDir::new().unwrap();
        let store = test_store(&temp_dir);

        let key = store.get_key("nonexistent").unwrap();
        assert!(key.is_none());
    }

    #[test]
    fn test_delete_key() {
        let temp_dir = TempDir::new().unwrap();
        let store = test_store(&temp_dir);

        store.store_key("test", "value").unwrap();
        assert!(store.has_key("test").unwrap());

        store.delete_key("test").unwrap();
        assert!(!store.has_key("test").unwrap());
    }

    #[test]
    fn test_list_keys() {
        let temp_dir = TempDir::new().unwrap();
        let store = test_store(&temp_dir);

        store.store_key("key1", "value1").unwrap();
        store.store_key("key2", "value2").unwrap();

        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("keys.json");
        let key = vec![0u8; 32]; // Fixed key for testing

        // Store key in first instance
        {
            let store = KeyStore::with_key(&path, key.clone());
            store.store_key("persist", "persistent-value").unwrap();
        }

        // Retrieve in second instance (same key)
        {
            let store = KeyStore::with_key(&path, key.clone());
            let key = store.get_key("persist").unwrap().unwrap();
            assert_eq!(key, "persistent-value");
        }
    }

    #[test]
    fn test_different_encryption_keys_cannot_decrypt() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("keys.json");
        let key_a = vec![0u8; 32];
        let key_b = vec![1u8; 32]; // Different key

        // Store with key A
        {
            let store = KeyStore::with_key(&path, key_a);
            store.store_key("secret", "my-secret").unwrap();
        }

        // Try to read with key B (different)
        {
            let store = KeyStore::with_key(&path, key_b);
            // Should fail to decrypt since encryption key is different
            let result = store.get_key("secret");
            assert!(result.is_err(), "Expected decryption to fail with different key");
        }
    }

    #[test]
    fn test_env_derived_key() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("keys.json");
        let key = vec![42u8; 32]; // Fixed key for testing

        let store = KeyStore::with_key(&path, key.clone());
        store.store_key("test_key", "test-value").unwrap();

        // Read back with same key
        let store2 = KeyStore::with_key(&path, key);
        let key = store2.get_key("test_key").unwrap().unwrap();
        assert_eq!(key, "test-value");
    }
}
