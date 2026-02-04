//! Shared utilities for CLI commands.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use lilypad_core::{decrypt, encrypt, Attachment, Entry, EntryMetadata, EntrySecret, EntryType, KeyMaterial, Vault};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use zeroize::Zeroize;

/// Output format for CLI commands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

/// A String wrapper that zeroizes its contents on drop.
#[derive(Clone)]
pub struct SecureString(String);

impl SecureString {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Writes data to a file atomically using a temporary file and rename.
/// This ensures the file is never in a partially-written state.
pub fn atomic_write(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    let parent = path.parent().unwrap_or(Path::new("."));

    // Create temp file in the same directory to ensure same filesystem for rename
    let mut temp_file = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;

    temp_file
        .write_all(data.as_ref())
        .with_context(|| "failed to write to temp file")?;

    temp_file
        .flush()
        .with_context(|| "failed to flush temp file")?;

    // Persist the temp file by renaming it to the target path
    temp_file
        .persist(path)
        .with_context(|| format!("failed to persist file to {}", path.display()))?;

    Ok(())
}

/// Returns the path to the key file.
pub fn key_path(config: &lilypad_core::AppConfig) -> PathBuf {
    PathBuf::from(&config.data_dir).join("key.json")
}

/// Builds entry metadata from optional fields.
pub fn build_metadata(
    username: Option<String>,
    url: Option<String>,
    tags: Vec<String>,
    folder: Option<String>,
    entry_type: Option<String>,
) -> Result<EntryMetadata> {
    let mut metadata = EntryMetadata::default();
    apply_metadata_updates(&mut metadata, username, url, tags, folder, entry_type)?;
    Ok(metadata)
}

/// Applies metadata updates to an existing metadata struct.
pub fn apply_metadata_updates(
    metadata: &mut EntryMetadata,
    username: Option<String>,
    url: Option<String>,
    tags: Vec<String>,
    folder: Option<String>,
    entry_type: Option<String>,
) -> Result<()> {
    if username.is_some() {
        metadata.username = username;
    }
    if url.is_some() {
        metadata.url = url;
    }
    if !tags.is_empty() {
        metadata.tags = tags;
    }
    if folder.is_some() {
        metadata.folder = folder;
    }
    if let Some(entry_type) = entry_type {
        metadata.entry_type = parse_entry_type(&entry_type)?;
    }
    Ok(())
}

/// Parses entry type string into EntryType enum.
pub fn parse_entry_type(value: &str) -> Result<EntryType> {
    let normalized = value.to_lowercase().replace('_', "-");
    match normalized.as_str() {
        "login" => Ok(EntryType::Login),
        "card" => Ok(EntryType::Card),
        "identity" => Ok(EntryType::Identity),
        "secure-note" => Ok(EntryType::SecureNote),
        "software-license" => Ok(EntryType::SoftwareLicense),
        "wifi" => Ok(EntryType::Wifi),
        "server" => Ok(EntryType::Server),
        "custom" => Ok(EntryType::Custom),
        _ => Err(anyhow!("unsupported entry type: {value}")),
    }
}

/// Returns the label string for an entry type.
pub fn entry_type_label(entry_type: &EntryType) -> &'static str {
    match entry_type {
        EntryType::Login => "login",
        EntryType::Card => "card",
        EntryType::Identity => "identity",
        EntryType::SecureNote => "secure-note",
        EntryType::SoftwareLicense => "software-license",
        EntryType::Wifi => "wifi",
        EntryType::Server => "server",
        EntryType::Custom => "custom",
    }
}

/// Parses tags from a semicolon-separated string.
pub fn parse_tags(tags: &str) -> Vec<String> {
    tags.split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

/// Loads attachments from file paths.
pub fn load_attachments(paths: Vec<PathBuf>) -> Result<Vec<Attachment>> {
    let mut attachments = Vec::new();
    for path in paths {
        let data = fs::read(&path)
            .with_context(|| format!("failed to read attachment {}", path.display()))?;
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow!("attachment has invalid filename"))?
            .to_string();
        let data_base64 = STANDARD.encode(data);
        attachments.push(Attachment {
            filename,
            mime_type: None,
            data_base64,
        });
    }
    Ok(attachments)
}

/// Encrypts an entry secret.
pub fn encrypt_entry_secret(
    key: &KeyMaterial,
    secret: &EntrySecret,
) -> Result<lilypad_core::Ciphertext> {
    let payload = serde_json::to_vec(secret)?;
    Ok(encrypt(key, &payload)?)
}

/// Decrypts an entry secret.
pub fn decrypt_entry_secret(key: &KeyMaterial, entry: &Entry) -> Result<EntrySecret> {
    let plaintext = decrypt(key, &entry.ciphertext)?;
    // Try to deserialize as structured JSON first
    if let Ok(secret) = serde_json::from_slice::<EntrySecret>(&plaintext) {
        return Ok(secret);
    }
    // Legacy format: plaintext is just the password as UTF-8 string
    let password = String::from_utf8(plaintext.clone()).map_err(|_| {
        anyhow!(
            "entry '{}' has corrupted data: invalid UTF-8 in legacy format",
            entry.label
        )
    })?;
    // Warn about legacy format so user knows to re-save the entry
    eprintln!(
        "Warning: entry '{}' is in legacy format. Consider updating it to migrate to new format.",
        entry.label
    );
    Ok(EntrySecret::new(password))
}

/// Upserts an entry into a vault (updates if exists, adds if not).
pub fn upsert_entry(
    vault: &mut Vault,
    key: &KeyMaterial,
    label: &str,
    metadata: EntryMetadata,
    secret: EntrySecret,
) -> Result<()> {
    let ciphertext = encrypt_entry_secret(key, &secret)?;
    if vault.find_entry(label).is_some() {
        vault.update_entry(label, ciphertext)?;
        if let Some(entry) = vault.entries.iter_mut().find(|entry| entry.label == label) {
            entry.metadata = metadata;
        }
    } else {
        vault.add_entry(Entry::new_with_metadata(label, metadata, ciphertext))?;
    }
    Ok(())
}

/// Validates that a value is not empty.
pub fn non_empty_value(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("value cannot be empty".to_string());
    }
    Ok(value.to_string())
}

/// Formats file size in human-readable format.
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Parse a date string (YYYY-MM-DD) or Unix timestamp to a Unix timestamp.
pub fn parse_date_to_timestamp(date_str: &str) -> Result<u64> {
    // Try parsing as Unix timestamp first
    if let Ok(ts) = date_str.parse::<u64>() {
        return Ok(ts);
    }

    // Try parsing as date (YYYY-MM-DD)
    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        let datetime = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow!("invalid date: failed to create datetime from {}", date_str))?;
        return Ok(datetime.and_utc().timestamp() as u64);
    }

    Err(anyhow!(
        "invalid date format: '{}'. Use YYYY-MM-DD or Unix timestamp.",
        date_str
    ))
}

/// Validates URL format (basic validation).
pub fn validate_url_format(url: &str) -> Result<()> {
    // Check for valid URL schemes
    let valid_schemes = ["http://", "https://", "ftp://", "ftps://", "ssh://", "file://"];
    let has_valid_scheme = valid_schemes
        .iter()
        .any(|scheme| url.to_lowercase().starts_with(scheme));

    if !has_valid_scheme && !url.contains("://") {
        // Allow URLs without scheme (will be treated as https)
        // But must have at least a domain-like structure
        if !url.contains('.') && !url.starts_with("localhost") {
            return Err(anyhow!("URL '{}' appears malformed (no domain)", url));
        }
    }

    // Check for dangerous URL schemes
    let dangerous_schemes = ["javascript:", "data:", "vbscript:"];
    for scheme in dangerous_schemes {
        if url.to_lowercase().starts_with(scheme) {
            return Err(anyhow!("URL '{}' uses a potentially dangerous scheme", url));
        }
    }

    // Check for control characters
    if url.chars().any(|c| c.is_control()) {
        return Err(anyhow!("URL contains control characters"));
    }

    Ok(())
}

/// Validates TOTP secret format (should be valid base32).
pub fn validate_totp_secret(secret: &str) -> Result<()> {
    use totp_rs::Secret;

    // TOTP secrets should be base32 encoded
    let clean_secret = secret.to_uppercase().replace([' ', '-'], "");

    if clean_secret.is_empty() {
        return Err(anyhow!("TOTP secret is empty"));
    }

    // Check for valid base32 characters
    for c in clean_secret.chars() {
        if !matches!(c, 'A'..='Z' | '2'..='7' | '=') {
            return Err(anyhow!(
                "TOTP secret contains invalid character '{}' (must be base32: A-Z, 2-7)",
                c
            ));
        }
    }

    // Try to decode to verify it's valid base32
    let secret_obj = Secret::Encoded(clean_secret);
    secret_obj
        .to_bytes()
        .map_err(|e| anyhow!("invalid TOTP secret: {}", e))?;

    Ok(())
}

/// Validates imported entry data against size limits and format requirements.
pub fn validate_import_entry(
    label: &str,
    secret: &EntrySecret,
    metadata: &EntryMetadata,
) -> Result<()> {
    use lilypad_core::{MAX_NOTES_SIZE, MAX_PASSWORD_SIZE};

    // Validate password size
    if secret.password.len() > MAX_PASSWORD_SIZE {
        return Err(anyhow!(
            "entry '{}': password exceeds maximum size ({} bytes, max {} bytes)",
            label,
            secret.password.len(),
            MAX_PASSWORD_SIZE
        ));
    }

    // Validate notes size
    if let Some(notes) = &secret.notes {
        if notes.len() > MAX_NOTES_SIZE {
            return Err(anyhow!(
                "entry '{}': notes exceed maximum size ({} bytes, max {} bytes)",
                label,
                notes.len(),
                MAX_NOTES_SIZE
            ));
        }
    }

    // Validate URL format if present
    if let Some(url) = &metadata.url {
        if !url.is_empty() {
            validate_url_format(url)
                .with_context(|| format!("entry '{}' has invalid URL", label))?;
        }
    }

    // Validate TOTP secret format if present
    if let Some(totp_secret) = &secret.totp_secret {
        if !totp_secret.is_empty() {
            validate_totp_secret(totp_secret)
                .with_context(|| format!("entry '{}' has invalid TOTP secret", label))?;
        }
    }

    // Validate metadata
    metadata
        .validate()
        .with_context(|| format!("entry '{}' has invalid metadata", label))?;

    // Validate secret (including attachments)
    secret
        .validate()
        .with_context(|| format!("entry '{}' has invalid secret data", label))?;

    Ok(())
}

/// Reads a password securely from stdin (hidden input).
pub fn read_password_secure(prompt: &str) -> Result<String> {
    eprint!("{}", prompt);
    std::io::stderr().flush()?;
    rpassword::read_password().map_err(|e| anyhow!("failed to read password: {}", e))
}

/// Reads a password with confirmation.
pub fn read_password_with_confirmation(prompt: &str, confirm_prompt: &str) -> Result<String> {
    let password = read_password_secure(prompt)?;
    if password.is_empty() {
        return Err(anyhow!("password cannot be empty"));
    }
    let confirm = read_password_secure(confirm_prompt)?;
    if password != confirm {
        return Err(anyhow!("passwords do not match"));
    }
    Ok(password)
}
