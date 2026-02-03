use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use lilypad_common::validation::validate_vault_name;
use lilypad_core::{decrypt, encrypt, AppConfig, KeyMaterial, KeyMetadata, Vault};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

const VAULT_EXTENSION: &str = "lily";
const LEGACY_EXTENSION: &str = "json";
const LEGACY_VAULT_HEADER: &[u8] = b"LILYPAD_VAULT_V1\n";
const VAULT_HEADER: &[u8] = b"LILYPAD_VAULT_V1\n# Lilypad vault (encrypted)\n";

/// Current vault format version.
const CURRENT_VERSION: u32 = 1;

/// Maximum supported vault format version.
const MAX_SUPPORTED_VERSION: u32 = 1;

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
            version: CURRENT_VERSION,
            key_metadata: vault.key_metadata.clone(),
            ciphertext,
        };
        let serialized = serde_json::to_vec_pretty(&stored)?;
        let mut bytes = Vec::with_capacity(VAULT_HEADER.len() + serialized.len());
        bytes.extend_from_slice(VAULT_HEADER);
        bytes.extend_from_slice(&serialized);
        let path = self.vault_path(&vault.name)?;
        self.write_vault_file_atomic(&path, &bytes)?;
        Ok(())
    }

    pub fn load_vault(&self, name: &str, key: &KeyMaterial) -> Result<Vault> {
        let path = self.vault_path(name)?;
        let bytes = self.read_vault_file_locked(&path)?;
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

        // Validate vault version
        if stored.version > MAX_SUPPORTED_VERSION {
            return Err(anyhow!(
                "vault format version {} is not supported (max supported: {}). \
                 Please upgrade Lilypad to open this vault.",
                stored.version,
                MAX_SUPPORTED_VERSION
            ));
        }

        // Check for version that may need migration
        if stored.version < CURRENT_VERSION {
            eprintln!(
                "Note: vault '{}' uses format version {}. Saving will upgrade it to version {}.",
                name, stored.version, CURRENT_VERSION
            );
        }

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
        validate_vault_name(name).context("invalid vault name")?;
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
        self.root.join("vaults").join(format!("{name}.{extension}"))
    }

    pub fn sync_backend(&self) -> Option<&dyn SyncBackend> {
        self.sync.as_deref()
    }

    pub fn list_vaults(&self) -> Result<Vec<String>> {
        let vaults_dir = self.root.join("vaults");
        if !vaults_dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(vaults_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
                if extension == VAULT_EXTENSION || extension == LEGACY_EXTENSION {
                    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
        names.sort();
        names.dedup();
        Ok(names)
    }

    pub fn delete_vault(&self, name: &str) -> Result<()> {
        let path = self.vault_path(name)?;
        if !path.exists() {
            return Err(anyhow!("vault '{name}' does not exist"));
        }
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn rename_vault(&self, from: &str, to: &str) -> Result<()> {
        validate_vault_name(to).context("invalid new vault name")?;
        let from_path = self.vault_path(from)?;
        if !from_path.exists() {
            return Err(anyhow!("vault '{from}' does not exist"));
        }
        let to_path = self.vault_path_with_extension(to, VAULT_EXTENSION);
        if to_path.exists() {
            return Err(anyhow!("vault '{to}' already exists"));
        }
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(from_path, to_path)?;
        Ok(())
    }

    pub fn sync_payload(&self, name: &str) -> Result<Vec<u8>> {
        let path = self.vault_path(name)?;
        self.read_vault_file_locked(&path)
    }

    pub fn apply_sync_payload(&self, name: &str, payload: &[u8]) -> Result<()> {
        let path = self.vault_path(name)?;
        self.write_vault_file_atomic(&path, payload)?;
        Ok(())
    }

    /// Reads a vault file with a shared lock to prevent concurrent write issues.
    fn read_vault_file_locked(&self, path: &PathBuf) -> Result<Vec<u8>> {
        let file = File::open(path)
            .with_context(|| format!("failed to open vault file: {}", path.display()))?;

        // Try to acquire a shared lock (allows multiple readers)
        file.lock_shared().with_context(|| {
            format!(
                "failed to acquire lock on vault file: {} (another process may be writing)",
                path.display()
            )
        })?;

        let mut bytes = Vec::new();
        let mut reader = std::io::BufReader::new(&file);
        reader.read_to_end(&mut bytes)?;

        // Lock is automatically released when file is dropped
        Ok(bytes)
    }

    /// Writes a vault file atomically using a temporary file and rename.
    ///
    /// This ensures that the vault file is never in an inconsistent state:
    /// - If the write fails, the original file remains unchanged
    /// - If the process crashes during write, the temp file is left behind (not the corrupted vault)
    fn write_vault_file_atomic(&self, path: &PathBuf, bytes: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Create a temporary file in the same directory for atomic rename
        let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let temp_file = tempfile::NamedTempFile::new_in(parent)
            .context("failed to create temporary file for atomic write")?;

        // Acquire an exclusive lock on the temp file
        temp_file.as_file().lock_exclusive().with_context(|| {
            format!(
                "failed to acquire exclusive lock for writing: {}",
                path.display()
            )
        })?;

        // Write to the temp file
        temp_file
            .as_file()
            .write_all(bytes)
            .context("failed to write vault data")?;
        temp_file
            .as_file()
            .sync_all()
            .context("failed to sync vault data to disk")?;

        // Set secure permissions before persisting
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(temp_file.path(), fs::Permissions::from_mode(0o600))?;
        }

        // Atomically rename temp file to target path
        // This is atomic on most filesystems (POSIX guarantees it)
        temp_file
            .persist(path)
            .with_context(|| format!("failed to persist vault file: {}", path.display()))?;

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
