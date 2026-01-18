use anyhow::{anyhow, Result};
use lilypad_core::{decrypt, encrypt, AppConfig, KeyMaterial, KeyMetadata, Vault};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const VAULT_EXTENSION: &str = "lily";
const LEGACY_EXTENSION: &str = "json";
const LEGACY_VAULT_HEADER: &[u8] = b"LILYPAD_VAULT_V1\n";
const VAULT_HEADER: &[u8] = b"LILYPAD_VAULT_V1\n# Lilypad vault (encrypted)\n";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreStatus {
    pub root: String,
    pub has_sync: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredVault {
    pub version: u32,
    pub key_metadata: KeyMetadata,
    pub ciphertext: lilypad_core::Ciphertext,
}

pub trait SyncBackend: Send + Sync {
    fn push(&self, vault_name: &str, payload: &[u8]) -> Result<()>;
    fn pull(&self, vault_name: &str) -> Result<Option<Vec<u8>>>;
}

pub struct NoopSync;

impl SyncBackend for NoopSync {
    fn push(&self, _vault_name: &str, _payload: &[u8]) -> Result<()> {
        Err(anyhow!("sync backend not configured"))
    }

    fn pull(&self, _vault_name: &str) -> Result<Option<Vec<u8>>> {
        Err(anyhow!("sync backend not configured"))
    }
}

pub struct LocalStore {
    root: PathBuf,
    sync: Option<Box<dyn SyncBackend>>,
}

impl LocalStore {
    pub fn new(config: &AppConfig) -> Result<Self> {
        Ok(Self {
            root: PathBuf::from(&config.data_dir),
            sync: None,
        })
    }

    pub fn status(&self) -> StoreStatus {
        StoreStatus {
            root: self.root.display().to_string(),
            has_sync: self.sync.is_some(),
        }
    }

    pub fn with_sync(mut self, backend: Box<dyn SyncBackend>) -> Self {
        self.sync = Some(backend);
        self
    }

    pub fn save_vault(&self, vault: &Vault, key: &KeyMaterial) -> Result<()> {
        let payload = serde_json::to_vec(vault)?;
        let ciphertext = encrypt(key, &payload)?;
        let stored = StoredVault {
            version: 1,
            key_metadata: vault.key_metadata.clone(),
            ciphertext,
        };
        let serialized = serde_json::to_vec_pretty(&stored)?;
        let mut bytes = Vec::with_capacity(VAULT_HEADER.len() + serialized.len());
        bytes.extend_from_slice(VAULT_HEADER);
        bytes.extend_from_slice(&serialized);
        let path = self.vault_path(&vault.name)?;
        self.write_vault_file(&path, &bytes)?;
        Ok(())
    }

    pub fn load_vault(&self, name: &str, key: &KeyMaterial) -> Result<Vault> {
        let path = self.vault_path(name)?;
        let bytes = fs::read(&path)?;
        let stored_bytes = if bytes.starts_with(VAULT_HEADER) {
            &bytes[VAULT_HEADER.len()..]
        } else if bytes.starts_with(LEGACY_VAULT_HEADER) {
            &bytes[LEGACY_VAULT_HEADER.len()..]
        } else if bytes.starts_with(b"{") {
            bytes.as_slice()
        } else {
            return Err(anyhow!(
                "vault file '{}' has an invalid header",
                path.display()
            ));
        };
        let stored: StoredVault = serde_json::from_slice(stored_bytes)?;
        if stored.key_metadata.key_id != key.key_id() {
            return Err(anyhow!(
                "key id mismatch: expected {}, got {}",
                stored.key_metadata.key_id,
                key.key_id()
            ));
        }
        let decrypted = decrypt(key, &stored.ciphertext)?;
        let vault: Vault = serde_json::from_slice(&decrypted)?;
        Ok(vault)
    }

    fn vault_path(&self, name: &str) -> Result<PathBuf> {
        if name.trim().is_empty() {
            return Err(anyhow!("vault name cannot be empty"));
        }
        let lily_path = self.vault_path_with_extension(name, VAULT_EXTENSION);
        if lily_path.exists() {
            return Ok(lily_path);
        }
        let legacy_path = self.vault_path_with_extension(name, LEGACY_EXTENSION);
        if legacy_path.exists() {
            return Ok(legacy_path);
        }
        Ok(lily_path)
    }

    fn vault_path_with_extension(&self, name: &str, extension: &str) -> PathBuf {
        self.root
            .join("vaults")
            .join(format!("{name}.{extension}"))
    }

    pub fn sync_backend(&self) -> Option<&dyn SyncBackend> {
        self.sync.as_deref()
    }

    pub fn sync_payload(&self, name: &str) -> Result<Vec<u8>> {
        let path = self.vault_path(name)?;
        Ok(fs::read(path)?)
    }

    pub fn apply_sync_payload(&self, name: &str, payload: &[u8]) -> Result<()> {
        let path = self.vault_path(name)?;
        self.write_vault_file(&path, payload)?;
        Ok(())
    }

    fn write_vault_file(&self, path: &PathBuf, bytes: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::LocalStore;
    use lilypad_core::AppConfig;

    #[test]
    fn it_reports_status() {
        let config = AppConfig::default();
        let store = LocalStore::new(&config).expect("store");
        let status = store.status();
        assert_eq!(status.root, ".lilypad");
        assert!(!status.has_sync);
    }
}
