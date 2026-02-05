use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use lilypad_common::validation::validate_vault_name;
use lilypad_core::{decrypt, encrypt, AppConfig, KeyMaterial, KeyMetadata, Vault};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Number of overwrite passes for secure deletion
const SECURE_DELETE_PASSES: usize = 3;

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

/// Information about a vault backup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupInfo {
    pub filename: String,
    pub vault_name: String,
    pub created_at: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredVault {
    pub version: u32,
    pub key_metadata: KeyMetadata,
    pub ciphertext: lilypad_core::Ciphertext,
    /// SHA-256 checksum of the ciphertext for integrity verification
    /// (allows detecting corruption without needing the decryption key)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
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

        // Compute SHA-256 checksum of the ciphertext for integrity verification
        let checksum = compute_checksum(&ciphertext);

        let stored = StoredVault {
            version: CURRENT_VERSION,
            key_metadata: vault.key_metadata.clone(),
            ciphertext,
            checksum: Some(checksum),
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

        // Verify integrity checksum if present
        if let Some(ref expected_checksum) = stored.checksum {
            let actual_checksum = compute_checksum(&stored.ciphertext);
            if &actual_checksum != expected_checksum {
                return Err(anyhow!(
                    "vault integrity check failed: checksum mismatch. \
                     The vault file may be corrupted or tampered with."
                ));
            }
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
        // Use secure deletion to prevent data recovery
        secure_delete(&path)?;
        Ok(())
    }

    /// Securely deletes a file by overwriting its contents with random data
    /// multiple times before unlinking it.
    pub fn secure_delete_file(&self, path: &Path) -> Result<()> {
        secure_delete(path)
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

    // ============== BACKUP METHODS ==============

    /// Creates a backup of a vault. Returns the backup filename.
    pub fn create_backup(&self, vault_name: &str) -> Result<String> {
        let vault_path = self.vault_path(vault_name)?;
        if !vault_path.exists() {
            return Err(anyhow!("vault '{vault_name}' does not exist"));
        }

        let backup_dir = self.backup_dir();
        fs::create_dir_all(&backup_dir)?;

        // Generate backup filename with timestamp
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("{vault_name}_{timestamp}.backup");
        let backup_path = backup_dir.join(&backup_name);

        // Copy the vault file to the backup location
        fs::copy(&vault_path, &backup_path)
            .with_context(|| format!("failed to create backup: {}", backup_path.display()))?;

        // Set secure permissions on backup
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&backup_path, fs::Permissions::from_mode(0o600))?;
        }

        Ok(backup_name)
    }

    /// Lists all backups for a vault (most recent first).
    pub fn list_backups(&self, vault_name: &str) -> Result<Vec<BackupInfo>> {
        let backup_dir = self.backup_dir();
        if !backup_dir.exists() {
            return Ok(Vec::new());
        }

        let prefix = format!("{vault_name}_");
        let mut backups = Vec::new();

        for entry in fs::read_dir(&backup_dir)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().to_string();

            if filename.starts_with(&prefix) && filename.ends_with(".backup") {
                let metadata = entry.metadata()?;
                let created = metadata
                    .created()
                    .or_else(|_| metadata.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                backups.push(BackupInfo {
                    filename: filename.clone(),
                    vault_name: vault_name.to_string(),
                    created_at: created,
                    size_bytes: metadata.len(),
                });
            }
        }

        // Sort by creation time, most recent first
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(backups)
    }

    /// Restores a vault from a backup. Returns the old vault filename if one existed.
    pub fn restore_backup(&self, backup_name: &str) -> Result<Option<String>> {
        let backup_path = self.backup_dir().join(backup_name);
        if !backup_path.exists() {
            return Err(anyhow!("backup '{backup_name}' not found"));
        }

        // Extract vault name from backup filename
        let vault_name = backup_name
            .split('_')
            .next()
            .ok_or_else(|| anyhow!("invalid backup filename"))?;

        validate_vault_name(vault_name)?;

        let vault_path = self.vault_path_with_extension(vault_name, VAULT_EXTENSION);
        let old_backup = if vault_path.exists() {
            // Create a backup of the current vault before restoring
            Some(self.create_backup(vault_name)?)
        } else {
            None
        };

        // Ensure parent directory exists
        if let Some(parent) = vault_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Copy backup to vault location
        fs::copy(&backup_path, &vault_path)
            .with_context(|| format!("failed to restore backup to: {}", vault_path.display()))?;

        Ok(old_backup)
    }

    /// Deletes old backups, keeping only the most recent N backups for each vault.
    pub fn prune_backups(&self, vault_name: &str, keep_count: usize) -> Result<usize> {
        let backups = self.list_backups(vault_name)?;
        let to_delete: Vec<_> = backups.into_iter().skip(keep_count).collect();
        let deleted_count = to_delete.len();

        for backup in to_delete {
            let path = self.backup_dir().join(&backup.filename);
            secure_delete(&path)?;
        }

        Ok(deleted_count)
    }

    /// Returns the backup directory path.
    pub fn backup_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    // ============== END BACKUP METHODS ==============

    /// Reads a vault file with a shared lock to prevent concurrent write issues.
    fn read_vault_file_locked(&self, path: &std::path::Path) -> Result<Vec<u8>> {
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
    fn write_vault_file_atomic(&self, path: &std::path::Path, bytes: &[u8]) -> Result<()> {
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

/// Computes a SHA-256 checksum of the ciphertext for integrity verification.
fn compute_checksum(ciphertext: &lilypad_core::Ciphertext) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ciphertext.nonce);
    hasher.update(&ciphertext.data); // Vec<u8> needs borrow
    let result = hasher.finalize();
    hex::encode(result)
}

/// Verifies a vault file's integrity without decrypting it.
/// Returns Ok(()) if the vault passes integrity checks, or an error describing the issue.
pub fn verify_vault_integrity(path: &Path) -> Result<()> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read vault file: {}", path.display()))?;

    let stored_bytes = if bytes.starts_with(VAULT_HEADER) {
        &bytes[VAULT_HEADER.len()..]
    } else if bytes.starts_with(LEGACY_VAULT_HEADER) {
        &bytes[LEGACY_VAULT_HEADER.len()..]
    } else if bytes.starts_with(b"{") {
        bytes.as_slice()
    } else {
        return Err(anyhow!("invalid vault header"));
    };

    let stored: StoredVault = serde_json::from_slice(stored_bytes)
        .context("failed to parse vault JSON")?;

    if let Some(ref expected_checksum) = stored.checksum {
        let actual_checksum = compute_checksum(&stored.ciphertext);
        if &actual_checksum != expected_checksum {
            return Err(anyhow!("checksum mismatch: vault may be corrupted"));
        }
    } else {
        // No checksum present (older vault format)
        return Ok(());
    }

    Ok(())
}

/// Securely deletes a file by overwriting its contents multiple times
/// with random data before removing it from the filesystem.
///
/// This implements a simplified version of secure deletion that:
/// 1. Opens the file with write access
/// 2. Gets the file size
/// 3. Overwrites the entire file with random data (multiple passes)
/// 4. Syncs to disk to ensure overwrites are persisted
/// 5. Truncates the file to zero length
/// 6. Removes the file from the filesystem
///
/// Note: This may not be fully effective on SSDs with wear leveling,
/// journaling filesystems, or copy-on-write filesystems. For maximum
/// security, use full-disk encryption.
fn secure_delete(path: &Path) -> Result<()> {
    // Get file size
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to get metadata for: {}", path.display()))?;
    let file_size = metadata.len() as usize;

    if file_size == 0 {
        // Empty file, just remove it
        fs::remove_file(path)?;
        return Ok(());
    }

    // Open file for writing
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .with_context(|| format!("failed to open file for secure deletion: {}", path.display()))?;

    // Acquire exclusive lock
    file.lock_exclusive()
        .with_context(|| format!("failed to lock file for secure deletion: {}", path.display()))?;

    // Buffer for random data (use chunks for large files)
    let chunk_size = 64 * 1024; // 64KB chunks
    let mut buffer = vec![0u8; chunk_size.min(file_size)];

    for pass in 0..SECURE_DELETE_PASSES {
        // Seek to beginning of file
        file.seek(SeekFrom::Start(0))?;

        let mut remaining = file_size;
        while remaining > 0 {
            let to_write = remaining.min(buffer.len());

            // Fill buffer with random data for passes 0 and 2
            // Fill with zeros for pass 1 (Gutmann-lite pattern)
            if pass == 1 {
                buffer[..to_write].fill(0x00);
            } else {
                rand::thread_rng().fill_bytes(&mut buffer[..to_write]);
            }

            file.write_all(&buffer[..to_write])?;
            remaining -= to_write;
        }

        // Ensure data is written to disk
        file.sync_all()?;
    }

    // Final pass: overwrite with zeros and truncate
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    file.sync_all()?;

    // Release lock and close file
    drop(file);

    // Remove the file from the filesystem
    fs::remove_file(path)
        .with_context(|| format!("failed to remove file after secure deletion: {}", path.display()))?;

    Ok(())
}

/// Securely deletes a directory and all its contents.
pub fn secure_delete_dir(path: &Path) -> Result<()> {
    if !path.is_dir() {
        return Err(anyhow!("path is not a directory: {}", path.display()));
    }

    // First, securely delete all files
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            secure_delete_dir(&entry_path)?;
        } else {
            secure_delete(&entry_path)?;
        }
    }

    // Then remove the empty directory
    fs::remove_dir(path)
        .with_context(|| format!("failed to remove directory: {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{secure_delete, LocalStore};
    use lilypad_core::AppConfig;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn it_reports_status() {
        let config = AppConfig::default();
        let store = LocalStore::new(&config).expect("store");
        let status = store.status();
        assert_eq!(status.root, ".lilypad");
        assert!(!status.has_sync);
    }

    #[test]
    fn test_secure_delete() {
        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("test_secret.txt");

        // Create a file with some content
        {
            let mut file = fs::File::create(&file_path).expect("create file");
            file.write_all(b"super secret password 12345").expect("write");
        }

        assert!(file_path.exists());

        // Securely delete it
        secure_delete(&file_path).expect("secure delete");

        // Verify file is gone
        assert!(!file_path.exists());
    }

    #[test]
    fn test_secure_delete_large_file() {
        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("test_large.bin");

        // Create a larger file (256KB)
        {
            let mut file = fs::File::create(&file_path).expect("create file");
            let data = vec![0xABu8; 256 * 1024];
            file.write_all(&data).expect("write");
        }

        assert!(file_path.exists());

        // Securely delete it
        secure_delete(&file_path).expect("secure delete");

        // Verify file is gone
        assert!(!file_path.exists());
    }
}
