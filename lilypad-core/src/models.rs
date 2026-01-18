use serde::{Deserialize, Serialize};

use crate::crypto::{Ciphertext, CryptoAlgorithm, KeyMaterial};
use crate::errors::{CoreError, Result};
use rand_core::{OsRng, RngCore};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vault {
    pub name: String,
    pub entries: Vec<Entry>,
    pub key_metadata: KeyMetadata,
    #[serde(default)]
    pub audit_log: Vec<AuditEvent>,
}

impl Vault {
    pub fn new(name: impl Into<String>, key_metadata: KeyMetadata) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
            key_metadata,
            audit_log: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: Entry) -> Result<()> {
        if entry.label.trim().is_empty() {
            return Err(CoreError::InvalidInput(
                "entry label cannot be empty".to_string(),
            ));
        }
        if self
            .entries
            .iter()
            .any(|existing| existing.label == entry.label)
        {
            return Err(CoreError::AlreadyExists(format!(
                "entry label '{}' already exists",
                entry.label
            )));
        }
        self.entries.push(entry);
        self.record_event(AuditEvent::new("entry_added", None));
        Ok(())
    }

    pub fn update_entry(&mut self, label: &str, ciphertext: Ciphertext) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.ciphertext = ciphertext;
        entry.updated_at = current_timestamp();
        self.record_event(AuditEvent::new("entry_updated", Some(label)));
        Ok(())
    }

    pub fn remove_entry(&mut self, label: &str) -> Result<Entry> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        let removed = self.entries.remove(index);
        self.record_event(AuditEvent::new("entry_removed", Some(label)));
        Ok(removed)
    }

    pub fn find_entry(&self, label: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.label == label)
    }

    pub fn search_entries(&self, query: &str) -> Vec<&Entry> {
        let needle = query.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| entry.label.to_lowercase().contains(&needle))
            .collect()
    }

    fn record_event(&mut self, mut event: AuditEvent) {
        if event.timestamp == 0 {
            event.timestamp = current_timestamp();
        }
        self.audit_log.push(event);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    #[serde(default)]
    pub id: String,
    pub label: String,
    pub ciphertext: Ciphertext,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
}

impl Entry {
    pub fn new(label: impl Into<String>, ciphertext: Ciphertext) -> Self {
        let now = current_timestamp();
        Self {
            id: generate_id(),
            label: label.into(),
            ciphertext,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyMetadata {
    pub key_id: String,
    pub algorithm: CryptoAlgorithm,
    #[serde(default)]
    pub kdf: Option<String>,
}

impl KeyMetadata {
    pub fn new(key: &KeyMaterial, algorithm: CryptoAlgorithm) -> Self {
        Self {
            key_id: key.key_id(),
            algorithm,
            kdf: None,
        }
    }

    pub fn with_kdf(mut self, kdf: impl Into<String>) -> Self {
        self.kdf = Some(kdf.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub timestamp: u64,
    pub action: String,
    #[serde(default)]
    pub entry_label: Option<String>,
}

impl AuditEvent {
    pub fn new(action: impl Into<String>, entry_label: Option<&str>) -> Self {
        Self {
            timestamp: current_timestamp(),
            action: action.into(),
            entry_label: entry_label.map(|label| label.to_string()),
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn generate_id() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use crate::crypto::{encrypt, CryptoAlgorithm, KeyMaterial};

    use super::{Entry, KeyMetadata, Vault};

    #[test]
    fn vault_starts_empty() {
        let key = KeyMaterial::generate();
        let metadata = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
        let vault = Vault::new("primary", metadata);
        assert_eq!(vault.entries.len(), 0);
    }

    #[test]
    fn entry_roundtrip() {
        let key = KeyMaterial::generate();
        let ciphertext = encrypt(&key, b"vault-secret").expect("encrypt");
        let entry = Entry::new("email", ciphertext);
        assert_eq!(entry.label, "email");
    }

    #[test]
    fn vault_crud() {
        let key = KeyMaterial::generate();
        let metadata = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
        let mut vault = Vault::new("primary", metadata);
        let ciphertext = encrypt(&key, b"value").expect("encrypt");
        vault
            .add_entry(Entry::new("email", ciphertext))
            .expect("add");
        assert!(vault.find_entry("email").is_some());
        let updated = encrypt(&key, b"new").expect("encrypt");
        vault.update_entry("email", updated).expect("update");
        let removed = vault.remove_entry("email").expect("remove");
        assert_eq!(removed.label, "email");
        assert!(vault.find_entry("email").is_none());
    }
}
