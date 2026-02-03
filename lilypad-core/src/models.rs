use serde::{Deserialize, Serialize};

use crate::crypto::{Ciphertext, CryptoAlgorithm, KeyMaterial};
use crate::errors::{CoreError, Result};
use rand_core::{OsRng, RngCore};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum size for entry labels (in characters).
pub const MAX_LABEL_LENGTH: usize = 256;

/// Maximum size for tags (in characters).
pub const MAX_TAG_LENGTH: usize = 64;

/// Maximum number of tags per entry.
pub const MAX_TAGS_PER_ENTRY: usize = 50;

/// Maximum size for folder names (in characters).
pub const MAX_FOLDER_LENGTH: usize = 128;

/// Maximum size for URLs (in characters).
pub const MAX_URL_LENGTH: usize = 2048;

/// Maximum size for usernames (in characters).
pub const MAX_USERNAME_LENGTH: usize = 256;

/// Maximum size for passwords (in bytes).
pub const MAX_PASSWORD_SIZE: usize = 10 * 1024; // 10 KB

/// Maximum size for notes (in bytes).
pub const MAX_NOTES_SIZE: usize = 100 * 1024; // 100 KB

/// Maximum size for a single attachment (in bytes).
pub const MAX_ATTACHMENT_SIZE: usize = 10 * 1024 * 1024; // 10 MB

/// Maximum total size for all attachments (in bytes).
pub const MAX_TOTAL_ATTACHMENTS_SIZE: usize = 50 * 1024 * 1024; // 50 MB

/// Maximum number of attachments per entry.
pub const MAX_ATTACHMENTS_PER_ENTRY: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vault {
    pub name: String,
    pub entries: Vec<Entry>,
    pub key_metadata: KeyMetadata,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub last_accessed_at: Option<u64>,
    #[serde(default)]
    pub audit_log: Vec<AuditEvent>,
}

impl Vault {
    pub fn new(name: impl Into<String>, key_metadata: KeyMetadata) -> Self {
        let now = current_timestamp();
        Self {
            name: name.into(),
            entries: Vec::new(),
            key_metadata,
            description: None,
            created_at: now,
            updated_at: now,
            last_accessed_at: None,
            audit_log: Vec::new(),
        }
    }

    pub fn set_description(&mut self, description: Option<String>) {
        self.description = description;
        self.touch();
        self.record_event(AuditEvent::new("vault_description_updated", None));
    }

    pub fn record_access(&mut self) {
        self.last_accessed_at = Some(current_timestamp());
        self.record_event(AuditEvent::new("vault_accessed", None));
    }

    pub fn add_entry(&mut self, entry: Entry) -> Result<()> {
        if entry.label.trim().is_empty() {
            return Err(CoreError::InvalidInput(
                "entry label cannot be empty".to_string(),
            ));
        }
        if entry.label.len() > MAX_LABEL_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "entry label too long ({} chars, max {} chars)",
                entry.label.len(),
                MAX_LABEL_LENGTH
            )));
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
        // Validate entry metadata
        entry.metadata.validate()?;
        self.entries.push(entry);
        self.touch();
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
        self.touch();
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
        self.touch();
        self.record_event(AuditEvent::new("entry_removed", Some(label)));
        Ok(removed)
    }

    pub fn rename_entry(&mut self, label: &str, new_label: impl Into<String>) -> Result<()> {
        let new_label = new_label.into();
        if new_label.trim().is_empty() {
            return Err(CoreError::InvalidInput(
                "entry label cannot be empty".to_string(),
            ));
        }
        if new_label.len() > MAX_LABEL_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "entry label too long ({} chars, max {} chars)",
                new_label.len(),
                MAX_LABEL_LENGTH
            )));
        }
        if label == new_label {
            return Ok(());
        }
        if self
            .entries
            .iter()
            .any(|existing| existing.label == new_label)
        {
            return Err(CoreError::AlreadyExists(format!(
                "entry label '{}' already exists",
                new_label
            )));
        }
        let event_label = {
            let entry = self
                .entries
                .iter_mut()
                .find(|entry| entry.label == label)
                .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
            entry.label = new_label;
            entry.updated_at = current_timestamp();
            entry.label.clone()
        };
        self.touch();
        self.record_event(AuditEvent::new("entry_renamed", Some(&event_label)));
        Ok(())
    }

    pub fn update_entry_metadata(&mut self, label: &str, metadata: EntryMetadata) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.metadata = metadata;
        entry.updated_at = current_timestamp();
        self.touch();
        self.record_event(AuditEvent::new("entry_metadata_updated", Some(label)));
        Ok(())
    }

    pub fn set_entry_folder(&mut self, label: &str, folder: Option<String>) -> Result<()> {
        if let Some(ref f) = folder {
            if f.len() > MAX_FOLDER_LENGTH {
                return Err(CoreError::InvalidInput(format!(
                    "folder name too long ({} chars, max {} chars)",
                    f.len(),
                    MAX_FOLDER_LENGTH
                )));
            }
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.metadata.folder = folder;
        entry.updated_at = current_timestamp();
        self.touch();
        self.record_event(AuditEvent::new("entry_folder_updated", Some(label)));
        Ok(())
    }

    pub fn add_entry_tag(&mut self, label: &str, tag: impl Into<String>) -> Result<()> {
        let tag = tag.into();
        if tag.trim().is_empty() {
            return Err(CoreError::InvalidInput("tag cannot be empty".to_string()));
        }
        if tag.len() > MAX_TAG_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "tag too long ({} chars, max {} chars)",
                tag.len(),
                MAX_TAG_LENGTH
            )));
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        if entry.metadata.tags.len() >= MAX_TAGS_PER_ENTRY {
            return Err(CoreError::InvalidInput(format!(
                "too many tags ({}, max {})",
                entry.metadata.tags.len(),
                MAX_TAGS_PER_ENTRY
            )));
        }
        if !entry.metadata.tags.iter().any(|existing| existing == &tag) {
            entry.metadata.tags.push(tag);
            entry.updated_at = current_timestamp();
            self.touch();
            self.record_event(AuditEvent::new("entry_tag_added", Some(label)));
        }
        Ok(())
    }

    pub fn remove_entry_tag(&mut self, label: &str, tag: &str) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        let index = entry
            .metadata
            .tags
            .iter()
            .position(|existing| existing == tag)
            .ok_or_else(|| CoreError::NotFound(format!("tag '{tag}'")))?;
        entry.metadata.tags.remove(index);
        entry.updated_at = current_timestamp();
        self.touch();
        self.record_event(AuditEvent::new("entry_tag_removed", Some(label)));
        Ok(())
    }

    pub fn find_entry(&self, label: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.label == label)
    }

    pub fn search_entries(&self, query: &str) -> Vec<&Entry> {
        let needle = query.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| entry.matches_query(&needle))
            .collect()
    }

    pub fn entries_by_folder(&self, folder: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.metadata.folder.as_deref() == Some(folder))
            .collect()
    }

    pub fn entries_by_tag(&self, tag: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.metadata.tags.iter().any(|value| value == tag))
            .collect()
    }

    pub fn list_folders(&self) -> Vec<String> {
        let mut folders = BTreeSet::new();
        for entry in &self.entries {
            if let Some(folder) = entry.metadata.folder.as_deref() {
                if !folder.trim().is_empty() {
                    folders.insert(folder.to_string());
                }
            }
        }
        folders.into_iter().collect()
    }

    pub fn list_tags(&self) -> Vec<String> {
        let mut tags = BTreeSet::new();
        for entry in &self.entries {
            for tag in &entry.metadata.tags {
                if !tag.trim().is_empty() {
                    tags.insert(tag.to_string());
                }
            }
        }
        tags.into_iter().collect()
    }

    fn record_event(&mut self, mut event: AuditEvent) {
        if event.timestamp == 0 {
            event.timestamp = current_timestamp();
        }
        self.audit_log.push(event);
    }

    fn touch(&mut self) {
        self.updated_at = current_timestamp();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    #[serde(default)]
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub metadata: EntryMetadata,
    pub ciphertext: Ciphertext,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
}

impl Entry {
    pub fn new(label: impl Into<String>, ciphertext: Ciphertext) -> Self {
        Self::new_with_metadata(label, EntryMetadata::default(), ciphertext)
    }

    pub fn new_with_metadata(
        label: impl Into<String>,
        metadata: EntryMetadata,
        ciphertext: Ciphertext,
    ) -> Self {
        let now = current_timestamp();
        Self {
            id: generate_id(),
            label: label.into(),
            metadata,
            ciphertext,
            created_at: now,
            updated_at: now,
        }
    }

    fn matches_query(&self, needle: &str) -> bool {
        let label_match = self.label.to_lowercase().contains(needle);
        let username_match = self
            .metadata
            .username
            .as_ref()
            .map(|value| value.to_lowercase().contains(needle))
            .unwrap_or(false);
        let url_match = self
            .metadata
            .url
            .as_ref()
            .map(|value| value.to_lowercase().contains(needle))
            .unwrap_or(false);
        let tag_match = self
            .metadata
            .tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(needle));
        label_match || username_match || url_match || tag_match
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryMetadata {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub entry_type: EntryType,
}

impl Default for EntryMetadata {
    fn default() -> Self {
        Self {
            username: None,
            url: None,
            tags: Vec::new(),
            folder: None,
            entry_type: EntryType::Login,
        }
    }
}

impl EntryMetadata {
    /// Validates the entry metadata against size and count limits.
    pub fn validate(&self) -> Result<()> {
        if let Some(username) = &self.username {
            if username.len() > MAX_USERNAME_LENGTH {
                return Err(CoreError::InvalidInput(format!(
                    "username too long ({} chars, max {} chars)",
                    username.len(),
                    MAX_USERNAME_LENGTH
                )));
            }
        }

        if let Some(url) = &self.url {
            if url.len() > MAX_URL_LENGTH {
                return Err(CoreError::InvalidInput(format!(
                    "URL too long ({} chars, max {} chars)",
                    url.len(),
                    MAX_URL_LENGTH
                )));
            }
        }

        if let Some(folder) = &self.folder {
            if folder.len() > MAX_FOLDER_LENGTH {
                return Err(CoreError::InvalidInput(format!(
                    "folder name too long ({} chars, max {} chars)",
                    folder.len(),
                    MAX_FOLDER_LENGTH
                )));
            }
        }

        if self.tags.len() > MAX_TAGS_PER_ENTRY {
            return Err(CoreError::InvalidInput(format!(
                "too many tags ({}, max {})",
                self.tags.len(),
                MAX_TAGS_PER_ENTRY
            )));
        }

        for tag in &self.tags {
            if tag.len() > MAX_TAG_LENGTH {
                return Err(CoreError::InvalidInput(format!(
                    "tag '{}' too long ({} chars, max {} chars)",
                    tag,
                    tag.len(),
                    MAX_TAG_LENGTH
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntryType {
    Login,
    Card,
    Identity,
    SecureNote,
    SoftwareLicense,
    Wifi,
    Server,
    Custom,
}

impl Default for EntryType {
    fn default() -> Self {
        EntryType::Login
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntrySecret {
    pub password: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub totp_secret: Option<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

impl EntrySecret {
    pub fn new(password: impl Into<String>) -> Self {
        Self {
            password: password.into(),
            notes: None,
            totp_secret: None,
            attachments: Vec::new(),
        }
    }

    /// Validates the entry secret against size limits.
    ///
    /// Returns an error if any field exceeds the maximum allowed size.
    pub fn validate(&self) -> Result<()> {
        if self.password.len() > MAX_PASSWORD_SIZE {
            return Err(CoreError::InvalidInput(format!(
                "password exceeds maximum size ({} bytes, max {} bytes)",
                self.password.len(),
                MAX_PASSWORD_SIZE
            )));
        }

        if let Some(notes) = &self.notes {
            if notes.len() > MAX_NOTES_SIZE {
                return Err(CoreError::InvalidInput(format!(
                    "notes exceed maximum size ({} bytes, max {} bytes)",
                    notes.len(),
                    MAX_NOTES_SIZE
                )));
            }
        }

        if self.attachments.len() > MAX_ATTACHMENTS_PER_ENTRY {
            return Err(CoreError::InvalidInput(format!(
                "too many attachments ({}, max {})",
                self.attachments.len(),
                MAX_ATTACHMENTS_PER_ENTRY
            )));
        }

        let mut total_attachment_size = 0usize;
        for attachment in &self.attachments {
            attachment.validate()?;
            total_attachment_size += attachment.data_base64.len();
        }

        if total_attachment_size > MAX_TOTAL_ATTACHMENTS_SIZE {
            return Err(CoreError::InvalidInput(format!(
                "total attachments size exceeds limit ({} bytes, max {} bytes)",
                total_attachment_size, MAX_TOTAL_ATTACHMENTS_SIZE
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    pub filename: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    pub data_base64: String,
}

impl Attachment {
    /// Creates a new attachment.
    pub fn new(filename: impl Into<String>, data_base64: impl Into<String>) -> Self {
        Self {
            filename: filename.into(),
            mime_type: None,
            data_base64: data_base64.into(),
        }
    }

    /// Creates a new attachment with MIME type.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Validates the attachment against size limits.
    pub fn validate(&self) -> Result<()> {
        if self.filename.trim().is_empty() {
            return Err(CoreError::InvalidInput(
                "attachment filename cannot be empty".to_string(),
            ));
        }

        if self.filename.len() > MAX_LABEL_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "attachment filename too long ({} chars, max {} chars)",
                self.filename.len(),
                MAX_LABEL_LENGTH
            )));
        }

        // Check for path traversal in filename
        if self.filename.contains("..") || self.filename.contains('/') || self.filename.contains('\\') {
            return Err(CoreError::InvalidInput(
                "attachment filename contains invalid characters".to_string(),
            ));
        }

        if self.data_base64.len() > MAX_ATTACHMENT_SIZE {
            return Err(CoreError::InvalidInput(format!(
                "attachment '{}' exceeds maximum size ({} bytes, max {} bytes)",
                self.filename,
                self.data_base64.len(),
                MAX_ATTACHMENT_SIZE
            )));
        }

        Ok(())
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

    use super::{Entry, EntryMetadata, EntrySecret, EntryType, KeyMetadata, Vault};

    #[test]
    fn vault_starts_empty() {
        let key = KeyMaterial::generate();
        let metadata = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
        let vault = Vault::new("primary", metadata);
        assert_eq!(vault.entries.len(), 0);
        assert!(vault.created_at > 0);
        assert_eq!(vault.created_at, vault.updated_at);
    }

    #[test]
    fn entry_roundtrip() {
        let key = KeyMaterial::generate();
        let secret = EntrySecret::new("vault-secret");
        let payload = serde_json::to_vec(&secret).expect("serialize");
        let ciphertext = encrypt(&key, &payload).expect("encrypt");
        let metadata = EntryMetadata {
            username: Some("name".to_string()),
            url: Some("https://example.com".to_string()),
            tags: vec!["personal".to_string()],
            folder: Some("accounts".to_string()),
            entry_type: EntryType::Login,
        };
        let entry = Entry::new_with_metadata("email", metadata, ciphertext);
        assert_eq!(entry.label, "email");
        assert_eq!(entry.metadata.username.as_deref(), Some("name"));
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
        vault
            .add_entry_tag("email", "personal")
            .expect("tag add");
        vault
            .set_entry_folder("email", Some("accounts".to_string()))
            .expect("folder update");
        vault.rename_entry("email", "primary").expect("rename");
        assert_eq!(vault.list_tags(), vec!["personal".to_string()]);
        assert_eq!(vault.list_folders(), vec!["accounts".to_string()]);
        let removed = vault.remove_entry("primary").expect("remove");
        assert_eq!(removed.label, "primary");
        assert!(vault.find_entry("primary").is_none());
    }
}
