use serde::{Deserialize, Serialize};

use crate::crypto::{Ciphertext, CryptoAlgorithm, KeyMaterial};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vault {
    pub name: String,
    pub entries: Vec<Entry>,
    pub key_metadata: KeyMetadata,
}

impl Vault {
    pub fn new(name: impl Into<String>, key_metadata: KeyMetadata) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
            key_metadata,
        }
    }

    pub fn add_entry(&mut self, entry: Entry) {
        self.entries.push(entry);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub label: String,
    pub ciphertext: Ciphertext,
}

impl Entry {
    pub fn new(label: impl Into<String>, ciphertext: Ciphertext) -> Self {
        Self {
            label: label.into(),
            ciphertext,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyMetadata {
    pub key_id: String,
    pub algorithm: CryptoAlgorithm,
}

impl KeyMetadata {
    pub fn new(key: &KeyMaterial, algorithm: CryptoAlgorithm) -> Self {
        Self {
            key_id: key.key_id(),
            algorithm,
        }
    }
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
}
