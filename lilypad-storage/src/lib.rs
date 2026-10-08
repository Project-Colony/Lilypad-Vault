use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use lilypad_common::validation::validate_vault_name;
use lilypad_core::{
    decrypt, encrypt, AppConfig, EmbeddedKdfParams, KeyMaterial, KeyMetadata, Vault,
};
use rand::Rng as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Number of overwrite passes for secure deletion
const SECURE_DELETE_PASSES: usize = 3;

const VAULT_EXTENSION: &str = "lily";
const LEGACY_EXTENSION: &str = "json";
const LEGACY_VAULT_HEADER: &[u8] = b"LILYPAD_VAULT_V1\n";
const VAULT_HEADER_V1: &[u8] = b"LILYPAD_VAULT_V1\n# Lilypad vault (encrypted)\n";
const VAULT_HEADER_V2: &[u8] = b"LILYPAD_VAULT_V2\n# Lilypad vault (encrypted)\n";

/// Maximum supported vault format version.
const MAX_SUPPORTED_VERSION: u32 = 2;

/// Magic string used for password verification.
const VERIFIER_MAGIC: &str = "LILYPAD_VERIFY_OK";

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

/// A small encrypted blob used to verify the master password is correct
/// before attempting full vault decryption (V2+).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PasswordVerifier {
    /// The known plaintext marker, encrypted with the derived key.
    pub ciphertext: lilypad_core::Ciphertext,
    /// The expected plaintext (a constant magic string).
    pub magic: String,
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
    /// Encrypted password verifier (V2+). Allows password validation
    /// without decrypting the entire vault.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_verifier: Option<PasswordVerifier>,
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

/// Local filesystem storage backend for Lilypad vaults.
///
/// `LocalStore` manages encrypted vault files, key files, backups,
/// and audit logs on the local filesystem.
///
/// # Examples
///
/// ```no_run
/// use lilypad_core::{default_config, Vault};
/// use lilypad_storage::LocalStore;
///
/// let config = default_config();
/// let store = LocalStore::new(&config).expect("open store");
///
/// // List available vaults
/// let vaults = store.list_vaults().expect("list vaults");
/// println!("Found {} vaults", vaults.len());
///
/// // Check if a vault exists
/// if store.vault_exists("primary").unwrap_or(false) {
///     println!("primary vault exists");
/// }
/// ```
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

        // If vault has embedded KDF params, write V2 with password verifier
        let (version, header, password_verifier) = if vault.key_metadata.kdf_params.is_some() {
            let verifier = create_password_verifier(key)?;
            (2, VAULT_HEADER_V2, Some(verifier))
        } else {
            (1, VAULT_HEADER_V1, None)
        };

        let stored = StoredVault {
            version,
            key_metadata: vault.key_metadata.clone(),
            ciphertext,
            checksum: Some(checksum),
            password_verifier,
        };
        let serialized = serde_json::to_vec_pretty(&stored)?;
        let mut bytes = Vec::with_capacity(header.len() + serialized.len());
        bytes.extend_from_slice(header);
        bytes.extend_from_slice(&serialized);
        let path = self.vault_path(&vault.name)?;
        self.write_vault_file_atomic(&path, &bytes)?;
        Ok(())
    }

    pub fn load_vault(&self, name: &str, key: &KeyMaterial) -> Result<Vault> {
        let path = self.vault_path(name)?;
        let bytes = self.read_vault_file_locked(&path)?;
        self.load_vault_from_bytes(&bytes, key)
    }

    /// Parses, integrity-checks, verifies the password against, and decrypts a
    /// vault from raw file bytes already held in memory, without touching any
    /// file on disk. This lets a caller prove that a candidate payload (e.g. a
    /// freshly pulled remote vault) decrypts with a given key *before* it is
    /// allowed to overwrite the live vault file.
    pub fn load_vault_from_bytes(&self, bytes: &[u8], key: &KeyMaterial) -> Result<Vault> {
        let stored_bytes = strip_header(bytes)?;
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

        // Password verification
        if let Some(ref verifier) = stored.password_verifier {
            // V2: use password verifier
            match decrypt(key, &verifier.ciphertext) {
                Ok(plaintext) if plaintext == verifier.magic.as_bytes() => {
                    // Password correct, proceed
                }
                _ => {
                    return Err(anyhow!("incorrect master password"));
                }
            }
        } else {
            // V1: use key_id comparison
            if stored.key_metadata.key_id != key.key_id() {
                return Err(anyhow!(
                    "key id mismatch: expected {}, got {}",
                    stored.key_metadata.key_id,
                    key.key_id()
                ));
            }
        }

        let decrypted = decrypt(key, &stored.ciphertext)?;
        let vault: Vault = serde_json::from_slice(&decrypted)?;
        Ok(vault)
    }

    /// Checks whether a vault file exists on disk.
    pub fn vault_exists(&self, name: &str) -> Result<bool> {
        let lily_path = self.vault_path_with_extension(name, VAULT_EXTENSION);
        if lily_path.exists() {
            return Ok(true);
        }
        let legacy_path = self.vault_path_with_extension(name, LEGACY_EXTENSION);
        Ok(legacy_path.exists())
    }

    /// Loads KDF params from a stored vault file without decrypting.
    /// Returns None if the vault doesn't have embedded KDF params (V1 vault).
    pub fn load_vault_kdf_params(&self, name: &str) -> Result<Option<EmbeddedKdfParams>> {
        let path = self.vault_path(name)?;
        if !path.exists() {
            return Ok(None);
        }
        let bytes = self.read_vault_file_locked(&path)?;
        Self::kdf_params_from_bytes(&bytes)
    }

    /// Reads the embedded KDF params from raw vault bytes in memory, without
    /// touching disk. Lets a caller (e.g. validated sync) derive the exact key a
    /// future `load_vault` will need for a candidate payload before committing it.
    pub fn kdf_params_from_bytes(bytes: &[u8]) -> Result<Option<EmbeddedKdfParams>> {
        let stored_bytes = strip_header(bytes)?;
        let stored: StoredVault = serde_json::from_slice(stored_bytes)?;
        Ok(stored.key_metadata.kdf_params.clone())
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

        // Generate backup filename with a sub-second timestamp so two backups of
        // the same vault within one wall-clock second do not collide (fs::copy
        // would otherwise overwrite the earlier one, e.g. the pre-restore safety
        // backup). Format: <vault>_<YYYYMMDD>_<HHMMSS>_<microseconds>.backup
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%6f");
        let backup_name = format!("{vault_name}_{timestamp}.backup");
        let backup_path = backup_dir.join(&backup_name);

        // Write the backup atomically (temp + rename + fsync, 0600), matching
        // restore_backup: a crash mid-backup must not leave a truncated .backup
        // that only fails when someone tries to restore from it.
        let bytes = self.read_vault_file_locked(&vault_path)?;
        self.write_vault_file_atomic(&backup_path, &bytes)?;

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

            // Match by parsing the fixed `<vault>_<YYYYMMDD>_<HHMMSS>.backup`
            // suffix, NOT a `starts_with(prefix)` test: vault names may contain
            // underscores, so a prefix test would let "work" match backups of
            // "work_stuff" and vice versa (wrong-vault restore / prune).
            let _ = &prefix;
            if backup_vault_name(&filename).as_deref() == Some(vault_name) {
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
        backups.sort_by_key(|backup| Reverse(backup.created_at));
        Ok(backups)
    }

    /// Restores a vault from a backup. Returns the old vault filename if one existed.
    pub fn restore_backup(&self, backup_name: &str) -> Result<Option<String>> {
        let backup_path = self.backup_dir().join(backup_name);
        if !backup_path.exists() {
            return Err(anyhow!("backup '{backup_name}' not found"));
        }

        // Extract vault name by stripping the fixed timestamp suffix from the
        // right, so vault names containing underscores restore to themselves
        // rather than to a truncated prefix.
        let vault_name = backup_vault_name(backup_name)
            .ok_or_else(|| anyhow!("invalid backup filename: {backup_name}"))?;
        let vault_name = vault_name.as_str();

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

        // Restore atomically (temp file + rename + fsync) rather than a plain
        // copy: a crash mid-restore leaves the previous vault intact instead of
        // a half-written, unopenable file.
        let bytes = fs::read(&backup_path)
            .with_context(|| format!("failed to read backup: {}", backup_path.display()))?;
        self.write_vault_file_atomic(&vault_path, &bytes)?;

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

        // fsync the parent directory so the rename itself is durable: without
        // this, a crash right after a write returns can, on some filesystems,
        // leave the directory entry pointing at the old file (a just-completed
        // password change or save could silently revert).
        #[cfg(unix)]
        {
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
        }

        Ok(())
    }
}

/// Strips the vault file header, returning only the JSON payload.
/// Parses a backup filename of the form `<vault>_<YYYYMMDD>_<HHMMSS>.backup`
/// and returns the vault name. The timestamp suffix is a fixed shape (8 digits,
/// underscore, 6 digits), so it is stripped from the RIGHT, which keeps vault
/// names that themselves contain underscores intact. Returns `None` if the name
/// does not match the expected shape. Public so callers (e.g. the service
/// layer) can resolve which vault a restore will touch and lock it first.
pub fn backup_vault_name(filename: &str) -> Option<String> {
    fn all_digits(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    }
    let stem = filename.strip_suffix(".backup")?;
    // Two accepted timestamp shapes, stripped from the right:
    //   new: <vault>_<YYYYMMDD>_<HHMMSS>_<microseconds>
    //   old: <vault>_<YYYYMMDD>_<HHMMSS>
    // Disambiguated by whether the group before the final one is the 8-digit
    // date (old) or the 6-digit time (new).
    let (rest, last) = stem.rsplit_once('_')?;
    if !all_digits(last) {
        return None;
    }
    let (rest2, prev) = rest.rsplit_once('_')?;
    if prev.len() == 8 && all_digits(prev) {
        // old 2-group: prev = date, last = time. vault = rest2.
        return (!rest2.is_empty()).then(|| rest2.to_string());
    }
    if prev.len() == 6 && all_digits(prev) {
        // new 3-group: prev = time, last = microseconds. One more group = date.
        let (vault, ymd) = rest2.rsplit_once('_')?;
        if ymd.len() == 8 && all_digits(ymd) && !vault.is_empty() {
            return Some(vault.to_string());
        }
    }
    None
}

fn strip_header(bytes: &[u8]) -> Result<&[u8]> {
    if bytes.starts_with(VAULT_HEADER_V2) {
        Ok(&bytes[VAULT_HEADER_V2.len()..])
    } else if bytes.starts_with(VAULT_HEADER_V1) {
        Ok(&bytes[VAULT_HEADER_V1.len()..])
    } else if bytes.starts_with(LEGACY_VAULT_HEADER) {
        Ok(&bytes[LEGACY_VAULT_HEADER.len()..])
    } else if bytes.starts_with(b"{") {
        Ok(bytes)
    } else {
        Err(anyhow!("vault file has an invalid header"))
    }
}

/// Creates a password verifier by encrypting a known magic string.
fn create_password_verifier(key: &KeyMaterial) -> Result<PasswordVerifier> {
    let ciphertext = encrypt(key, VERIFIER_MAGIC.as_bytes())
        .map_err(|e| anyhow!("failed to create password verifier: {}", e))?;
    Ok(PasswordVerifier {
        ciphertext,
        magic: VERIFIER_MAGIC.to_string(),
    })
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
    let bytes =
        fs::read(path).with_context(|| format!("failed to read vault file: {}", path.display()))?;

    let stored_bytes = strip_header(&bytes)?;

    let stored: StoredVault =
        serde_json::from_slice(stored_bytes).context("failed to parse vault JSON")?;

    if let Some(ref expected_checksum) = stored.checksum {
        let actual_checksum = compute_checksum(&stored.ciphertext);
        if &actual_checksum != expected_checksum {
            return Err(anyhow!("checksum mismatch: vault may be corrupted"));
        }
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
    let mut file = OpenOptions::new().write(true).open(path).with_context(|| {
        format!(
            "failed to open file for secure deletion: {}",
            path.display()
        )
    })?;

    // Acquire exclusive lock
    file.lock_exclusive().with_context(|| {
        format!(
            "failed to lock file for secure deletion: {}",
            path.display()
        )
    })?;

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
                rand::rng().fill_bytes(&mut buffer[..to_write]);
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
    fs::remove_file(path).with_context(|| {
        format!(
            "failed to remove file after secure deletion: {}",
            path.display()
        )
    })?;

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
    use super::{backup_vault_name, secure_delete, LocalStore};
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
            file.write_all(b"super secret password 12345")
                .expect("write");
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

    // ===================== HELPER FUNCTIONS =====================

    fn make_test_store(dir: &std::path::Path) -> LocalStore {
        let config = lilypad_core::AppConfig {
            environment: "test".to_string(),
            data_dir: dir.to_string_lossy().to_string(),
        };
        LocalStore::new(&config).expect("create store")
    }

    fn make_test_vault(name: &str, key: &lilypad_core::KeyMaterial) -> lilypad_core::Vault {
        let metadata =
            lilypad_core::KeyMetadata::new(key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305);
        lilypad_core::Vault::new(name, metadata)
    }

    fn make_test_entry(label: &str, key: &lilypad_core::KeyMaterial) -> lilypad_core::Entry {
        let secret = lilypad_core::EntrySecret::new("test-password");
        let plaintext = serde_json::to_vec(&secret).expect("serialize secret");
        let ciphertext = lilypad_core::encrypt(key, &plaintext).expect("encrypt secret");
        lilypad_core::Entry::new_with_metadata(
            label,
            lilypad_core::EntryMetadata::default(),
            ciphertext,
        )
    }

    // ===================== SAVE / LOAD VAULT TESTS =====================

    #[test]
    fn test_save_and_load_vault() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        let mut vault = make_test_vault("test-vault", &key);
        vault
            .add_entry(make_test_entry("Entry1", &key))
            .expect("add entry 1");
        vault
            .add_entry(make_test_entry("Entry2", &key))
            .expect("add entry 2");

        store.save_vault(&vault, &key).expect("save vault");
        let loaded = store.load_vault("test-vault", &key).expect("load vault");

        assert_eq!(loaded.name, "test-vault");
        assert_eq!(loaded.entries.len(), 2);
        assert!(loaded.find_entry("Entry1").is_some());
        assert!(loaded.find_entry("Entry2").is_some());
    }

    #[test]
    fn test_load_nonexistent_vault() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        let result = store.load_vault("no-such-vault", &key);
        assert!(result.is_err(), "loading a nonexistent vault should fail");
    }

    #[test]
    fn test_save_overwrites_existing() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        // Save a vault with one entry
        let mut vault = make_test_vault("overwrite-vault", &key);
        vault
            .add_entry(make_test_entry("OriginalEntry", &key))
            .expect("add original");
        store.save_vault(&vault, &key).expect("first save");

        // Modify and save again
        vault
            .add_entry(make_test_entry("NewEntry", &key))
            .expect("add new");
        store.save_vault(&vault, &key).expect("second save");

        // Load and verify the updated state
        let loaded = store
            .load_vault("overwrite-vault", &key)
            .expect("load after overwrite");
        assert_eq!(loaded.entries.len(), 2);
        assert!(loaded.find_entry("OriginalEntry").is_some());
        assert!(loaded.find_entry("NewEntry").is_some());
    }

    // ===================== MULTI-VAULT TESTS =====================

    #[test]
    fn test_list_vaults_empty() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());

        let vaults = store.list_vaults().expect("list vaults");
        assert!(vaults.is_empty(), "fresh store should have no vaults");
    }

    #[test]
    fn test_list_vaults_multiple() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        for name in &["alpha", "bravo", "charlie"] {
            let vault = make_test_vault(name, &key);
            store.save_vault(&vault, &key).expect("save vault");
        }

        let mut vaults = store.list_vaults().expect("list vaults");
        vaults.sort();
        assert_eq!(vaults, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn test_vault_isolation() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        // Create vault A with entry "SecretA"
        let mut vault_a = make_test_vault("vault-a", &key);
        vault_a
            .add_entry(make_test_entry("SecretA", &key))
            .expect("add SecretA");
        store.save_vault(&vault_a, &key).expect("save vault-a");

        // Create vault B with entry "SecretB"
        let mut vault_b = make_test_vault("vault-b", &key);
        vault_b
            .add_entry(make_test_entry("SecretB", &key))
            .expect("add SecretB");
        store.save_vault(&vault_b, &key).expect("save vault-b");

        // Load each vault and verify isolation
        let loaded_a = store.load_vault("vault-a", &key).expect("load vault-a");
        let loaded_b = store.load_vault("vault-b", &key).expect("load vault-b");

        assert_eq!(loaded_a.entries.len(), 1);
        assert!(loaded_a.find_entry("SecretA").is_some());
        assert!(loaded_a.find_entry("SecretB").is_none());

        assert_eq!(loaded_b.entries.len(), 1);
        assert!(loaded_b.find_entry("SecretB").is_some());
        assert!(loaded_b.find_entry("SecretA").is_none());
    }

    // ===================== BACKUP TESTS =====================

    #[test]
    fn test_create_backup() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        let vault = make_test_vault("backup-test", &key);
        store.save_vault(&vault, &key).expect("save vault");

        let backup_name = store.create_backup("backup-test").expect("create backup");

        // Verify backup file exists on disk
        let backup_path = store.backup_dir().join(&backup_name);
        assert!(
            backup_path.exists(),
            "backup file should exist at {}",
            backup_path.display()
        );
        assert!(
            backup_name.starts_with("backup-test_"),
            "backup name should start with vault name"
        );
        assert!(
            backup_name.ends_with(".backup"),
            "backup name should end with .backup"
        );
    }

    #[test]
    fn test_list_backups() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        let vault = make_test_vault("list-bk", &key);
        store.save_vault(&vault, &key).expect("save vault");

        // Create 3 backups with short delays to get distinct timestamps
        for _ in 0..3 {
            store.create_backup("list-bk").expect("create backup");
            // Small sleep to ensure distinct timestamps in filenames
            std::thread::sleep(std::time::Duration::from_millis(1100));
        }

        let backups = store.list_backups("list-bk").expect("list backups");
        assert_eq!(backups.len(), 3, "should have 3 backups");
        for info in &backups {
            assert_eq!(info.vault_name, "list-bk");
            assert!(info.size_bytes > 0, "backup should have non-zero size");
        }
    }

    #[test]
    fn backup_vault_name_parses_underscored_names() {
        // A vault whose name contains underscores must round-trip: the fixed
        // `_<8 digits>_<6 digits>` timestamp is stripped from the right.
        // Old 2-group format.
        assert_eq!(
            backup_vault_name("work_stuff_20260714_120000.backup").as_deref(),
            Some("work_stuff")
        );
        assert_eq!(
            backup_vault_name("work_20260714_120000.backup").as_deref(),
            Some("work")
        );
        // New 3-group (sub-second) format.
        assert_eq!(
            backup_vault_name("work_stuff_20260714_120000_123456.backup").as_deref(),
            Some("work_stuff")
        );
        assert_eq!(
            backup_vault_name("work_20260714_120000_000001.backup").as_deref(),
            Some("work")
        );
        // Malformed / non-backup names are rejected.
        assert_eq!(backup_vault_name("work.backup"), None);
        assert_eq!(backup_vault_name("work_2026_120000.backup"), None);
        assert_eq!(backup_vault_name("_20260714_120000.backup"), None);
    }

    #[test]
    fn backups_do_not_cross_contaminate_underscored_vaults() {
        // Regression guard for the audited data-loss bug: with the old
        // `starts_with("work_")` matching, "work" would have matched
        // "work_stuff"'s backups (wrong-vault restore / prune).
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        store
            .save_vault(&make_test_vault("work", &key), &key)
            .expect("save work");
        store
            .save_vault(&make_test_vault("work_stuff", &key), &key)
            .expect("save work_stuff");
        store.create_backup("work").expect("backup work");
        store
            .create_backup("work_stuff")
            .expect("backup work_stuff");

        let work = store.list_backups("work").expect("list work");
        let work_stuff = store.list_backups("work_stuff").expect("list work_stuff");
        assert_eq!(work.len(), 1, "'work' must see only its own backup");
        assert_eq!(work_stuff.len(), 1, "'work_stuff' must see only its own");
        assert!(work.iter().all(|b| b.vault_name == "work"));
        assert!(work_stuff.iter().all(|b| b.vault_name == "work_stuff"));
    }

    #[test]
    fn test_restore_backup() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        // Save original vault with one entry
        let mut vault = make_test_vault("restore-test", &key);
        vault
            .add_entry(make_test_entry("OriginalOnly", &key))
            .expect("add original entry");
        store.save_vault(&vault, &key).expect("save original");

        // Create a backup of the original state
        let backup_name = store.create_backup("restore-test").expect("create backup");

        // Modify the vault (add another entry, remove original)
        vault
            .add_entry(make_test_entry("AddedLater", &key))
            .expect("add new entry");
        vault.remove_entry("OriginalOnly").expect("remove original");
        store.save_vault(&vault, &key).expect("save modified");

        // Verify the modified state
        let loaded_modified = store
            .load_vault("restore-test", &key)
            .expect("load modified");
        assert!(loaded_modified.find_entry("OriginalOnly").is_none());
        assert!(loaded_modified.find_entry("AddedLater").is_some());

        // Sleep to ensure the pre-restore backup created inside restore_backup
        // gets a distinct timestamp from the original backup (second-level precision)
        std::thread::sleep(std::time::Duration::from_millis(1100));

        // Restore from backup
        store.restore_backup(&backup_name).expect("restore backup");

        // Verify original state is restored
        let loaded_restored = store
            .load_vault("restore-test", &key)
            .expect("load restored");
        assert!(
            loaded_restored.find_entry("OriginalOnly").is_some(),
            "original entry should be restored"
        );
        assert!(
            loaded_restored.find_entry("AddedLater").is_none(),
            "later addition should not be present after restore"
        );
    }

    #[test]
    fn test_prune_backups() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        let vault = make_test_vault("prune-test", &key);
        store.save_vault(&vault, &key).expect("save vault");

        // Create 5 backups
        for _ in 0..5 {
            store.create_backup("prune-test").expect("create backup");
            std::thread::sleep(std::time::Duration::from_millis(1100));
        }

        let before = store.list_backups("prune-test").expect("list before");
        assert_eq!(before.len(), 5);

        // Prune to keep only 2
        let deleted = store.prune_backups("prune-test", 2).expect("prune backups");
        assert_eq!(deleted, 3, "should have pruned 3 backups");

        let after = store.list_backups("prune-test").expect("list after");
        assert_eq!(after.len(), 2, "should have 2 backups remaining");
    }

    // ===================== SYNC PAYLOAD TESTS =====================

    #[test]
    fn test_sync_payload_roundtrip() {
        let dir_src = tempdir().expect("create source dir");
        let dir_dst = tempdir().expect("create destination dir");
        let store_src = make_test_store(dir_src.path());
        let store_dst = make_test_store(dir_dst.path());
        let key = lilypad_core::KeyMaterial::generate();

        // Save a vault in the source store
        let mut vault = make_test_vault("sync-vault", &key);
        vault
            .add_entry(make_test_entry("SyncEntry", &key))
            .expect("add entry");
        store_src.save_vault(&vault, &key).expect("save in source");

        // Get sync payload from source
        let payload = store_src
            .sync_payload("sync-vault")
            .expect("get sync payload");

        // Apply sync payload to destination
        store_dst
            .apply_sync_payload("sync-vault", &payload)
            .expect("apply sync payload");

        // Load from destination and verify
        let loaded = store_dst
            .load_vault("sync-vault", &key)
            .expect("load from destination");
        assert_eq!(loaded.name, "sync-vault");
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.find_entry("SyncEntry").is_some());
    }

    #[test]
    fn test_apply_sync_payload_overwrites() {
        let dir_src = tempdir().expect("create source dir");
        let dir_dst = tempdir().expect("create destination dir");
        let store_src = make_test_store(dir_src.path());
        let store_dst = make_test_store(dir_dst.path());
        let key = lilypad_core::KeyMaterial::generate();

        // Save an initial vault in destination
        let mut initial_vault = make_test_vault("sync-overwrite", &key);
        initial_vault
            .add_entry(make_test_entry("OldEntry", &key))
            .expect("add old entry");
        store_dst
            .save_vault(&initial_vault, &key)
            .expect("save initial in dst");

        // Prepare a different vault in source
        let mut new_vault = make_test_vault("sync-overwrite", &key);
        new_vault
            .add_entry(make_test_entry("NewEntry", &key))
            .expect("add new entry");
        store_src
            .save_vault(&new_vault, &key)
            .expect("save in source");

        // Get payload from source and apply to destination
        let payload = store_src
            .sync_payload("sync-overwrite")
            .expect("get payload");
        store_dst
            .apply_sync_payload("sync-overwrite", &payload)
            .expect("apply payload");

        // Verify destination now has the source's data
        let loaded = store_dst
            .load_vault("sync-overwrite", &key)
            .expect("load overwritten");
        assert_eq!(loaded.entries.len(), 1);
        assert!(
            loaded.find_entry("NewEntry").is_some(),
            "should have the new entry from sync"
        );
        assert!(
            loaded.find_entry("OldEntry").is_none(),
            "old entry should be replaced"
        );
    }

    // ===================== EDGE CASE TESTS =====================

    #[test]
    fn test_vault_name_validation() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        // Empty name
        let empty_vault = make_test_vault("", &key);
        assert!(
            store.save_vault(&empty_vault, &key).is_err(),
            "empty vault name should be rejected"
        );

        // Path traversal
        let traversal_vault = make_test_vault("../etc/passwd", &key);
        assert!(
            store.save_vault(&traversal_vault, &key).is_err(),
            "path traversal vault name should be rejected"
        );

        // Special characters
        let special_vault = make_test_vault("vault@home!", &key);
        assert!(
            store.save_vault(&special_vault, &key).is_err(),
            "special chars in vault name should be rejected"
        );

        // Hidden file prefix
        let hidden_vault = make_test_vault(".hidden", &key);
        assert!(
            store.save_vault(&hidden_vault, &key).is_err(),
            "dot-prefixed vault name should be rejected"
        );

        // Name with spaces
        let space_vault = make_test_vault("my vault", &key);
        assert!(
            store.save_vault(&space_vault, &key).is_err(),
            "vault name with spaces should be rejected"
        );

        // Name with forward slash
        let slash_vault = make_test_vault("foo/bar", &key);
        assert!(
            store.save_vault(&slash_vault, &key).is_err(),
            "vault name with slash should be rejected"
        );

        // Valid names should succeed
        let valid_vault = make_test_vault("my-vault_01", &key);
        assert!(
            store.save_vault(&valid_vault, &key).is_ok(),
            "valid vault name should be accepted"
        );
    }

    #[test]
    fn test_concurrent_access_safety() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        // Save initial vault
        let mut vault = make_test_vault("concurrent-vault", &key);
        vault
            .add_entry(make_test_entry("Entry1", &key))
            .expect("add entry 1");
        store.save_vault(&vault, &key).expect("save initial");

        // Simulate two sequential saves (file locking prevents corruption)
        vault
            .add_entry(make_test_entry("Entry2", &key))
            .expect("add entry 2");
        store.save_vault(&vault, &key).expect("save update 1");

        vault
            .add_entry(make_test_entry("Entry3", &key))
            .expect("add entry 3");
        store.save_vault(&vault, &key).expect("save update 2");

        // Verify the final state is consistent
        let loaded = store
            .load_vault("concurrent-vault", &key)
            .expect("load final state");
        assert_eq!(loaded.entries.len(), 3);
        assert!(loaded.find_entry("Entry1").is_some());
        assert!(loaded.find_entry("Entry2").is_some());
        assert!(loaded.find_entry("Entry3").is_some());

        // Verify that a second store instance pointing at the same directory
        // can also read the vault (shared lock)
        let store2 = make_test_store(dir.path());
        let loaded2 = store2
            .load_vault("concurrent-vault", &key)
            .expect("load from second store");
        assert_eq!(loaded2.entries.len(), 3);
    }

    // ===================== V2 VAULT FORMAT TESTS =====================

    /// Helper: create a V2 vault with embedded KDF params and password verifier.
    fn make_v2_test_vault(
        name: &str,
        password: &str,
    ) -> (
        lilypad_core::Vault,
        lilypad_core::KeyMaterial,
        lilypad_core::KeyDerivationParams,
    ) {
        let params = lilypad_core::KeyDerivationParams::generate();
        let key = lilypad_core::derive_key(password, &params).expect("derive key");
        let metadata =
            lilypad_core::KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
                .with_embedded_kdf(&params);
        let vault = lilypad_core::Vault::new(name, metadata);
        (vault, key, params)
    }

    #[test]
    fn test_v2_vault_roundtrip() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let password = "correct-horse-battery-staple!";

        let (mut vault, key, _params) = make_v2_test_vault("v2-test", password);
        vault
            .add_entry(make_test_entry("V2Entry", &key))
            .expect("add entry");

        // Save (should write V2 format with verifier)
        store.save_vault(&vault, &key).expect("save V2 vault");

        // Load with the correct key
        let loaded = store.load_vault("v2-test", &key).expect("load V2 vault");
        assert_eq!(loaded.name, "v2-test");
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.find_entry("V2Entry").is_some());
        // Verify embedded KDF params survived the roundtrip
        assert!(loaded.key_metadata.kdf_params.is_some());
    }

    #[test]
    fn test_v2_rejects_wrong_password() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let password = "correct-password-123!";
        let wrong_password = "wrong-password-456!";

        let (vault, key, params) = make_v2_test_vault("v2-reject", password);
        store.save_vault(&vault, &key).expect("save V2 vault");

        // Try to load with a wrong password (different derived key)
        let wrong_key =
            lilypad_core::derive_key(wrong_password, &params).expect("derive wrong key");
        let result = store.load_vault("v2-reject", &wrong_key);
        assert!(result.is_err(), "V2 vault should reject wrong password");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("incorrect master password"),
            "error should mention incorrect password, got: {err_msg}"
        );
    }

    #[test]
    fn test_v1_vault_still_loads() {
        // V1 vaults (without embedded KDF) should still load fine
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        // Create a V1 vault (no kdf_params = V1 format)
        let mut vault = make_test_vault("v1-compat", &key);
        vault
            .add_entry(make_test_entry("V1Entry", &key))
            .expect("add entry");
        store.save_vault(&vault, &key).expect("save V1 vault");

        // Load should succeed
        let loaded = store.load_vault("v1-compat", &key).expect("load V1 vault");
        assert_eq!(loaded.name, "v1-compat");
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.key_metadata.kdf_params.is_none());
    }

    #[test]
    fn test_vault_exists() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        assert!(!store
            .vault_exists("nonexistent")
            .expect("check nonexistent"));

        let vault = make_test_vault("exists-test", &key);
        store.save_vault(&vault, &key).expect("save vault");

        assert!(store.vault_exists("exists-test").expect("check existing"));
    }

    #[test]
    fn test_load_vault_kdf_params() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let password = "test-kdf-params-read!";

        // Create V2 vault
        let (vault, key, params) = make_v2_test_vault("kdf-read", password);
        store.save_vault(&vault, &key).expect("save V2 vault");

        // Read KDF params without full decryption
        let embedded = store
            .load_vault_kdf_params("kdf-read")
            .expect("load kdf params")
            .expect("should have embedded params");

        assert_eq!(embedded.algorithm, "argon2id");
        assert_eq!(embedded.salt, params.salt);
        assert_eq!(embedded.memory_kib, params.memory_kib);
        assert_eq!(embedded.iterations, params.iterations);
        assert_eq!(embedded.parallelism, params.parallelism);
    }

    #[test]
    fn test_load_vault_kdf_params_v1_returns_none() {
        let dir = tempdir().expect("create temp dir");
        let store = make_test_store(dir.path());
        let key = lilypad_core::KeyMaterial::generate();

        // Create V1 vault
        let vault = make_test_vault("v1-no-kdf", &key);
        store.save_vault(&vault, &key).expect("save V1 vault");

        let result = store
            .load_vault_kdf_params("v1-no-kdf")
            .expect("load kdf params");
        assert!(
            result.is_none(),
            "V1 vault should not have embedded KDF params"
        );
    }

    #[test]
    fn test_v2_sync_roundtrip() {
        let dir_a = tempdir().expect("create dir A");
        let dir_b = tempdir().expect("create dir B");
        let store_a = make_test_store(dir_a.path());
        let store_b = make_test_store(dir_b.path());
        let password = "sync-test-password!";

        // Device A: create V2 vault with entries
        let (mut vault, key, _params) = make_v2_test_vault("sync-v2", password);
        vault
            .add_entry(make_test_entry("SyncedEntry", &key))
            .expect("add entry");
        store_a.save_vault(&vault, &key).expect("save on device A");

        // Simulate GitHub sync: extract payload, apply to device B
        let payload = store_a.sync_payload("sync-v2").expect("get payload");
        store_b
            .apply_sync_payload("sync-v2", &payload)
            .expect("apply payload on device B");

        // Device B: read KDF params from vault and derive key from password
        let embedded = store_b
            .load_vault_kdf_params("sync-v2")
            .expect("load kdf on B")
            .expect("should have KDF params");
        let kdf_params = embedded.to_kdf_params();
        let key_b = lilypad_core::derive_key(password, &kdf_params).expect("derive key on B");

        // Device B: load vault with derived key
        let loaded = store_b.load_vault("sync-v2", &key_b).expect("load on B");
        assert_eq!(loaded.name, "sync-v2");
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.find_entry("SyncedEntry").is_some());
    }
}
