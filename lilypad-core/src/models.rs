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
pub const MAX_FOLDER_LENGTH: usize = 512;

/// Maximum folder nesting depth.
pub const MAX_FOLDER_DEPTH: usize = 10;

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

/// Maximum number of custom fields per entry.
pub const MAX_CUSTOM_FIELDS_PER_ENTRY: usize = 50;

/// Maximum size for custom field names (in characters).
pub const MAX_CUSTOM_FIELD_NAME_LENGTH: usize = 128;

/// Maximum size for custom field values (in bytes).
pub const MAX_CUSTOM_FIELD_VALUE_SIZE: usize = 10 * 1024; // 10 KB

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
            Self::validate_folder_path(f)?;
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

    /// Returns all favorite entries.
    pub fn favorites(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.is_favorite)
            .collect()
    }

    /// Returns recently used entries, sorted by last access time (most recent first).
    pub fn recently_used(&self, limit: usize) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self.entries
            .iter()
            .filter(|entry| entry.last_accessed_at.is_some())
            .collect();
        entries.sort_by(|a, b| {
            b.last_accessed_at.unwrap_or(0).cmp(&a.last_accessed_at.unwrap_or(0))
        });
        entries.into_iter().take(limit).collect()
    }

    /// Returns most frequently used entries, sorted by access count (highest first).
    pub fn most_used(&self, limit: usize) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self.entries
            .iter()
            .filter(|entry| entry.access_count > 0)
            .collect();
        entries.sort_by(|a, b| b.access_count.cmp(&a.access_count));
        entries.into_iter().take(limit).collect()
    }

    /// Sets an entry as favorite or not.
    pub fn set_entry_favorite(&mut self, label: &str, favorite: bool) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.set_favorite(favorite);
        self.touch();
        Ok(())
    }

    /// Records that an entry was accessed.
    pub fn record_entry_access(&mut self, label: &str) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.record_access();
        Ok(())
    }

    /// Sets the color for an entry.
    pub fn set_entry_color(&mut self, label: &str, color: Option<EntryColor>) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.set_color(color);
        self.touch();
        Ok(())
    }

    /// Sets the icon for an entry.
    pub fn set_entry_icon(&mut self, label: &str, icon: Option<String>) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.set_icon(icon);
        self.touch();
        Ok(())
    }

    /// Lists all unique parent folders in the folder tree.
    /// For nested folders like "Work/Email/Google", this returns:
    /// ["Work", "Work/Email", "Work/Email/Google"]
    pub fn list_folder_tree(&self) -> Vec<String> {
        let mut folders = BTreeSet::new();
        for entry in &self.entries {
            if let Some(folder) = entry.metadata.folder.as_deref() {
                if !folder.trim().is_empty() {
                    // Add the full path and all parent paths
                    let parts: Vec<&str> = folder.split('/').collect();
                    let mut path = String::new();
                    for (i, part) in parts.iter().enumerate() {
                        if i > 0 {
                            path.push('/');
                        }
                        path.push_str(part);
                        folders.insert(path.clone());
                    }
                }
            }
        }
        folders.into_iter().collect()
    }

    /// Returns entries in a folder, including entries in nested subfolders.
    pub fn entries_in_folder_recursive(&self, folder: &str) -> Vec<&Entry> {
        let prefix = if folder.ends_with('/') {
            folder.to_string()
        } else {
            format!("{}/", folder)
        };
        self.entries
            .iter()
            .filter(|entry| {
                if let Some(f) = &entry.metadata.folder {
                    f == folder || f.starts_with(&prefix)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Returns only entries directly in a folder (not in subfolders).
    pub fn entries_in_folder_direct(&self, folder: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.metadata.folder.as_deref() == Some(folder))
            .collect()
    }

    /// Returns immediate child folders of a given folder.
    pub fn child_folders(&self, parent: &str) -> Vec<String> {
        let mut children = BTreeSet::new();
        let prefix = if parent.is_empty() {
            String::new()
        } else if parent.ends_with('/') {
            parent.to_string()
        } else {
            format!("{}/", parent)
        };

        for entry in &self.entries {
            if let Some(folder) = &entry.metadata.folder {
                let relative = if parent.is_empty() {
                    folder.as_str()
                } else if folder.starts_with(&prefix) {
                    &folder[prefix.len()..]
                } else {
                    continue;
                };

                // Get the first component of the relative path
                if let Some(child) = relative.split('/').next() {
                    if !child.is_empty() {
                        let full_path = if parent.is_empty() {
                            child.to_string()
                        } else {
                            format!("{}/{}", parent.trim_end_matches('/'), child)
                        };
                        children.insert(full_path);
                    }
                }
            }
        }
        children.into_iter().collect()
    }

    /// Returns entries that have no folder assigned.
    pub fn entries_without_folder(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.metadata.folder.is_none() || entry.metadata.folder.as_deref() == Some(""))
            .collect()
    }

    /// Validates a folder path for nested folder support.
    pub fn validate_folder_path(path: &str) -> Result<()> {
        if path.is_empty() {
            return Ok(());
        }
        if path.len() > MAX_FOLDER_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "folder path too long ({} chars, max {} chars)",
                path.len(),
                MAX_FOLDER_LENGTH
            )));
        }
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() > MAX_FOLDER_DEPTH {
            return Err(CoreError::InvalidInput(format!(
                "folder path too deep ({} levels, max {} levels)",
                parts.len(),
                MAX_FOLDER_DEPTH
            )));
        }
        for part in &parts {
            if part.is_empty() {
                return Err(CoreError::InvalidInput(
                    "folder path contains empty components".to_string(),
                ));
            }
            if part.contains("..") {
                return Err(CoreError::InvalidInput(
                    "folder path contains invalid characters".to_string(),
                ));
            }
        }
        Ok(())
    }

    // ==================== Bulk Operations ====================

    /// Deletes multiple entries at once. Returns the number of entries deleted.
    pub fn bulk_delete(&mut self, labels: &[&str]) -> Result<usize> {
        let mut deleted = 0;
        for label in labels {
            if let Some(pos) = self.entries.iter().position(|e| e.label == *label) {
                self.entries.remove(pos);
                self.record_event(AuditEvent::new("entry_removed", Some(label)));
                deleted += 1;
            }
        }
        if deleted > 0 {
            self.touch();
        }
        Ok(deleted)
    }

    /// Moves multiple entries to a folder. Returns the number of entries moved.
    pub fn bulk_move_to_folder(&mut self, labels: &[&str], folder: Option<String>) -> Result<usize> {
        if let Some(ref f) = folder {
            Self::validate_folder_path(f)?;
        }
        let mut moved = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str()) {
                entry.metadata.folder = folder.clone();
                entry.updated_at = current_timestamp();
                moved += 1;
            }
        }
        if moved > 0 {
            self.touch();
            self.record_event(AuditEvent::new("bulk_folder_updated", None));
        }
        Ok(moved)
    }

    /// Adds a tag to multiple entries. Returns the number of entries modified.
    pub fn bulk_add_tag(&mut self, labels: &[&str], tag: impl Into<String>) -> Result<usize> {
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
        let mut modified = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str()) {
                if entry.metadata.tags.len() < MAX_TAGS_PER_ENTRY
                    && !entry.metadata.tags.iter().any(|t| t == &tag)
                {
                    entry.metadata.tags.push(tag.clone());
                    entry.updated_at = current_timestamp();
                    modified += 1;
                }
            }
        }
        if modified > 0 {
            self.touch();
            self.record_event(AuditEvent::new("bulk_tag_added", None));
        }
        Ok(modified)
    }

    /// Removes a tag from multiple entries. Returns the number of entries modified.
    pub fn bulk_remove_tag(&mut self, labels: &[&str], tag: &str) -> Result<usize> {
        let mut modified = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str()) {
                if let Some(pos) = entry.metadata.tags.iter().position(|t| t == tag) {
                    entry.metadata.tags.remove(pos);
                    entry.updated_at = current_timestamp();
                    modified += 1;
                }
            }
        }
        if modified > 0 {
            self.touch();
            self.record_event(AuditEvent::new("bulk_tag_removed", None));
        }
        Ok(modified)
    }

    /// Sets favorite status for multiple entries. Returns the number of entries modified.
    pub fn bulk_set_favorite(&mut self, labels: &[&str], favorite: bool) -> Result<usize> {
        let mut modified = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str()) && entry.is_favorite != favorite {
                entry.is_favorite = favorite;
                entry.updated_at = current_timestamp();
                modified += 1;
            }
        }
        if modified > 0 {
            self.touch();
        }
        Ok(modified)
    }

    /// Sets color for multiple entries. Returns the number of entries modified.
    pub fn bulk_set_color(&mut self, labels: &[&str], color: Option<EntryColor>) -> Result<usize> {
        let mut modified = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str()) && entry.color != color {
                entry.color = color;
                entry.updated_at = current_timestamp();
                modified += 1;
            }
        }
        if modified > 0 {
            self.touch();
        }
        Ok(modified)
    }

    /// Returns entries matching the given labels.
    pub fn get_entries(&self, labels: &[&str]) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| labels.contains(&e.label.as_str()))
            .collect()
    }

    /// Returns mutable entries matching the given labels.
    pub fn get_entries_mut(&mut self, labels: &[&str]) -> Vec<&mut Entry> {
        self.entries
            .iter_mut()
            .filter(|e| labels.contains(&e.label.as_str()))
            .collect()
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

/// Record of a change made to an entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryHistoryRecord {
    /// Unix timestamp when the change was made.
    pub timestamp: u64,
    /// Type of change that was made.
    pub change_type: EntryChangeType,
    /// Optional description of what changed.
    #[serde(default)]
    pub description: Option<String>,
    /// Previous ciphertext (for password history - encrypted).
    #[serde(default)]
    pub previous_ciphertext: Option<Ciphertext>,
}

/// Types of changes that can be made to an entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntryChangeType {
    /// Entry was created.
    Created,
    /// Password was changed.
    PasswordChanged,
    /// Metadata was updated (username, URL, tags, etc.).
    MetadataUpdated,
    /// Notes were modified.
    NotesUpdated,
    /// Entry was renamed.
    Renamed,
    /// TOTP secret was added or modified.
    TotpUpdated,
    /// Attachments were modified.
    AttachmentsUpdated,
}

impl EntryHistoryRecord {
    /// Creates a new history record with the current timestamp.
    pub fn new(change_type: EntryChangeType) -> Self {
        Self {
            timestamp: current_timestamp(),
            change_type,
            description: None,
            previous_ciphertext: None,
        }
    }

    /// Creates a new history record with a description.
    pub fn with_description(change_type: EntryChangeType, description: impl Into<String>) -> Self {
        Self {
            timestamp: current_timestamp(),
            change_type,
            description: Some(description.into()),
            previous_ciphertext: None,
        }
    }

    /// Creates a password change record that stores the previous password.
    pub fn password_change(previous_ciphertext: Ciphertext) -> Self {
        Self {
            timestamp: current_timestamp(),
            change_type: EntryChangeType::PasswordChanged,
            description: None,
            previous_ciphertext: Some(previous_ciphertext),
        }
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
    /// Unix timestamp when the password was last changed (0 = same as created_at)
    #[serde(default)]
    pub password_changed_at: u64,
    /// Optional Unix timestamp when the password should be rotated (0 = no expiry)
    #[serde(default)]
    pub password_expires_at: u64,
    /// History of changes made to this entry.
    #[serde(default)]
    pub history: Vec<EntryHistoryRecord>,
    /// Whether this entry is marked as favorite.
    #[serde(default)]
    pub is_favorite: bool,
    /// Unix timestamp when this entry was last accessed/used.
    #[serde(default)]
    pub last_accessed_at: Option<u64>,
    /// Number of times this entry has been accessed.
    #[serde(default)]
    pub access_count: u64,
    /// Icon identifier for custom entry icon.
    #[serde(default)]
    pub icon: Option<String>,
    /// Color label for visual categorization.
    #[serde(default)]
    pub color: Option<EntryColor>,
}

/// Color labels for visual categorization of entries.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntryColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Gray,
}

impl EntryColor {
    /// Returns the hex color code for this color.
    pub fn to_hex(&self) -> &'static str {
        match self {
            EntryColor::Red => "#EF4444",
            EntryColor::Orange => "#F97316",
            EntryColor::Yellow => "#EAB308",
            EntryColor::Green => "#22C55E",
            EntryColor::Blue => "#3B82F6",
            EntryColor::Purple => "#A855F7",
            EntryColor::Pink => "#EC4899",
            EntryColor::Gray => "#6B7280",
        }
    }

    /// Returns all available colors.
    pub fn all() -> &'static [EntryColor] {
        &[
            EntryColor::Red,
            EntryColor::Orange,
            EntryColor::Yellow,
            EntryColor::Green,
            EntryColor::Blue,
            EntryColor::Purple,
            EntryColor::Pink,
            EntryColor::Gray,
        ]
    }
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
        let mut entry = Self {
            id: generate_id(),
            label: label.into(),
            metadata,
            ciphertext,
            created_at: now,
            updated_at: now,
            password_changed_at: now,
            password_expires_at: 0,
            history: Vec::new(),
            is_favorite: false,
            last_accessed_at: None,
            access_count: 0,
            icon: None,
            color: None,
        };
        entry.history.push(EntryHistoryRecord::new(EntryChangeType::Created));
        entry
    }

    /// Marks this entry as favorite or not.
    pub fn set_favorite(&mut self, favorite: bool) {
        self.is_favorite = favorite;
        self.updated_at = current_timestamp();
    }

    /// Records that this entry was accessed (copy password, view, etc.).
    pub fn record_access(&mut self) {
        self.last_accessed_at = Some(current_timestamp());
        self.access_count = self.access_count.saturating_add(1);
    }

    /// Sets the icon identifier for this entry.
    pub fn set_icon(&mut self, icon: Option<String>) {
        self.icon = icon;
        self.updated_at = current_timestamp();
    }

    /// Sets the color label for this entry.
    pub fn set_color(&mut self, color: Option<EntryColor>) {
        self.color = color;
        self.updated_at = current_timestamp();
    }

    /// Records a change in the entry's history.
    pub fn record_change(&mut self, record: EntryHistoryRecord) {
        self.history.push(record);
        self.updated_at = current_timestamp();
    }

    /// Records a password change, optionally storing the previous password.
    pub fn record_password_change_with_history(&mut self, previous_ciphertext: Option<Ciphertext>) {
        self.password_changed_at = current_timestamp();
        if let Some(prev) = previous_ciphertext {
            self.history.push(EntryHistoryRecord::password_change(prev));
        } else {
            self.history.push(EntryHistoryRecord::new(EntryChangeType::PasswordChanged));
        }
        self.updated_at = current_timestamp();
    }

    /// Gets the history of this entry, most recent first.
    pub fn get_history(&self) -> impl Iterator<Item = &EntryHistoryRecord> {
        self.history.iter().rev()
    }

    /// Gets the number of password changes recorded in history.
    pub fn password_change_count(&self) -> usize {
        self.history.iter()
            .filter(|h| matches!(h.change_type, EntryChangeType::PasswordChanged))
            .count()
    }

    /// Sets the password expiry date (as Unix timestamp).
    /// Pass 0 to disable expiry.
    pub fn set_password_expiry(&mut self, expires_at: u64) {
        self.password_expires_at = expires_at;
    }

    /// Sets password expiry to N days from now.
    /// Pass 0 to disable expiry.
    pub fn set_password_expiry_days(&mut self, days: u32) {
        if days == 0 {
            self.password_expires_at = 0;
        } else {
            let now = current_timestamp();
            self.password_expires_at = now + (days as u64 * 24 * 60 * 60);
        }
    }

    /// Records that the password was changed (updates password_changed_at and adds to history).
    pub fn record_password_change(&mut self) {
        self.password_changed_at = current_timestamp();
        self.history.push(EntryHistoryRecord::new(EntryChangeType::PasswordChanged));
        self.updated_at = current_timestamp();
    }

    /// Returns true if the password has expired.
    pub fn is_password_expired(&self) -> bool {
        if self.password_expires_at == 0 {
            return false;
        }
        current_timestamp() > self.password_expires_at
    }

    /// Returns the number of days until password expires, or None if no expiry set.
    pub fn days_until_password_expires(&self) -> Option<i64> {
        if self.password_expires_at == 0 {
            return None;
        }
        let now = current_timestamp();
        let diff = self.password_expires_at as i64 - now as i64;
        Some(diff / (24 * 60 * 60))
    }

    /// Returns the age of the password in days.
    pub fn password_age_days(&self) -> u64 {
        let changed = if self.password_changed_at > 0 {
            self.password_changed_at
        } else {
            self.created_at
        };
        let now = current_timestamp();
        (now.saturating_sub(changed)) / (24 * 60 * 60)
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

/// Custom field for storing additional data with entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomField {
    /// Name/label of the custom field.
    pub name: String,
    /// Value of the custom field.
    pub value: String,
    /// Type of the custom field (affects how it's displayed/handled).
    #[serde(default)]
    pub field_type: CustomFieldType,
}

/// Types of custom fields.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CustomFieldType {
    /// Plain text field (visible).
    #[default]
    Text,
    /// Hidden/masked field (like a password).
    Hidden,
    /// Boolean toggle field.
    Boolean,
    /// URL field (can be clicked/opened).
    Url,
    /// Date field (stored as Unix timestamp string).
    Date,
}

impl CustomField {
    /// Creates a new text custom field.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            field_type: CustomFieldType::Text,
        }
    }

    /// Creates a new hidden (password-like) custom field.
    pub fn hidden(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            field_type: CustomFieldType::Hidden,
        }
    }

    /// Creates a new boolean custom field.
    pub fn boolean(name: impl Into<String>, value: bool) -> Self {
        Self {
            name: name.into(),
            value: value.to_string(),
            field_type: CustomFieldType::Boolean,
        }
    }

    /// Creates a new URL custom field.
    pub fn url(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            field_type: CustomFieldType::Url,
        }
    }

    /// Returns the value as a boolean (for Boolean type fields).
    pub fn as_bool(&self) -> bool {
        self.value.to_lowercase() == "true" || self.value == "1"
    }

    /// Validates the custom field against size limits.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(CoreError::InvalidInput(
                "custom field name cannot be empty".to_string(),
            ));
        }
        if self.name.len() > MAX_CUSTOM_FIELD_NAME_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "custom field name too long ({} chars, max {} chars)",
                self.name.len(),
                MAX_CUSTOM_FIELD_NAME_LENGTH
            )));
        }
        if self.value.len() > MAX_CUSTOM_FIELD_VALUE_SIZE {
            return Err(CoreError::InvalidInput(format!(
                "custom field value too large ({} bytes, max {} bytes)",
                self.value.len(),
                MAX_CUSTOM_FIELD_VALUE_SIZE
            )));
        }
        Ok(())
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
    /// TOTP backup codes (one-time use recovery codes)
    #[serde(default)]
    pub totp_backup_codes: Vec<TotpBackupCode>,
    /// Email address (for Identity entries or validation)
    #[serde(default)]
    pub email: Option<String>,
    /// Phone number (for Identity entries)
    #[serde(default)]
    pub phone: Option<String>,
    /// Custom fields for additional data.
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
}

/// TOTP backup code for recovery when 2FA device is unavailable
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TotpBackupCode {
    /// The backup code (typically 8-10 alphanumeric characters)
    pub code: String,
    /// Whether this code has been used
    pub used: bool,
    /// Timestamp when the code was used (0 if not used)
    #[serde(default)]
    pub used_at: u64,
}

/// Number of backup codes to generate
pub const TOTP_BACKUP_CODE_COUNT: usize = 10;
/// Length of each backup code
pub const TOTP_BACKUP_CODE_LENGTH: usize = 8;

impl TotpBackupCode {
    /// Creates a new unused backup code.
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            used: false,
            used_at: 0,
        }
    }

    /// Marks the code as used.
    pub fn mark_used(&mut self) {
        self.used = true;
        self.used_at = current_timestamp();
    }

    /// Generates a random backup code.
    pub fn generate() -> Self {
        // Use alphanumeric characters (excluding confusing ones like 0/O, 1/l/I)
        const CHARSET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
        let mut code = String::with_capacity(TOTP_BACKUP_CODE_LENGTH);
        let mut bytes = [0u8; TOTP_BACKUP_CODE_LENGTH];
        OsRng.fill_bytes(&mut bytes);
        for byte in bytes {
            let idx = (byte as usize) % CHARSET.len();
            code.push(CHARSET[idx] as char);
        }
        Self::new(code)
    }

    /// Generates a set of backup codes.
    pub fn generate_set(count: usize) -> Vec<Self> {
        (0..count).map(|_| Self::generate()).collect()
    }
}

impl EntrySecret {
    pub fn new(password: impl Into<String>) -> Self {
        Self {
            password: password.into(),
            notes: None,
            totp_secret: None,
            attachments: Vec::new(),
            totp_backup_codes: Vec::new(),
            email: None,
            phone: None,
            custom_fields: Vec::new(),
        }
    }

    /// Adds a custom field to this entry.
    pub fn add_custom_field(&mut self, field: CustomField) -> Result<()> {
        if self.custom_fields.len() >= MAX_CUSTOM_FIELDS_PER_ENTRY {
            return Err(CoreError::InvalidInput(format!(
                "too many custom fields ({}, max {})",
                self.custom_fields.len(),
                MAX_CUSTOM_FIELDS_PER_ENTRY
            )));
        }
        field.validate()?;
        self.custom_fields.push(field);
        Ok(())
    }

    /// Removes a custom field by name.
    pub fn remove_custom_field(&mut self, name: &str) -> Option<CustomField> {
        if let Some(pos) = self.custom_fields.iter().position(|f| f.name == name) {
            Some(self.custom_fields.remove(pos))
        } else {
            None
        }
    }

    /// Gets a custom field by name.
    pub fn get_custom_field(&self, name: &str) -> Option<&CustomField> {
        self.custom_fields.iter().find(|f| f.name == name)
    }

    /// Updates or adds a custom field.
    pub fn set_custom_field(&mut self, field: CustomField) -> Result<()> {
        field.validate()?;
        if let Some(existing) = self.custom_fields.iter_mut().find(|f| f.name == field.name) {
            *existing = field;
        } else {
            self.add_custom_field(field)?;
        }
        Ok(())
    }

    /// Generates TOTP backup codes for this entry.
    /// Returns the generated codes (should be shown to user once).
    pub fn generate_backup_codes(&mut self) -> Vec<String> {
        self.totp_backup_codes = TotpBackupCode::generate_set(TOTP_BACKUP_CODE_COUNT);
        self.totp_backup_codes.iter().map(|c| c.code.clone()).collect()
    }

    /// Verifies and consumes a backup code. Returns true if valid.
    pub fn use_backup_code(&mut self, code: &str) -> bool {
        let code_upper = code.to_uppercase().replace("-", "").replace(" ", "");
        for backup in &mut self.totp_backup_codes {
            if !backup.used && backup.code == code_upper {
                backup.mark_used();
                return true;
            }
        }
        false
    }

    /// Returns the number of unused backup codes.
    pub fn unused_backup_codes_count(&self) -> usize {
        self.totp_backup_codes.iter().filter(|c| !c.used).count()
    }

    /// Returns true if backup codes are running low (less than 3 remaining).
    pub fn backup_codes_low(&self) -> bool {
        self.unused_backup_codes_count() < 3
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
            // Use decoded size for accurate total calculation
            total_attachment_size += attachment.decoded_size();
        }

        if total_attachment_size > MAX_TOTAL_ATTACHMENTS_SIZE {
            return Err(CoreError::InvalidInput(format!(
                "total attachments size exceeds limit ({} bytes, max {} bytes)",
                total_attachment_size, MAX_TOTAL_ATTACHMENTS_SIZE
            )));
        }

        // Validate email format if present
        if let Some(email) = &self.email {
            if !is_valid_email(email) {
                return Err(CoreError::InvalidInput(format!(
                    "invalid email format: {}",
                    email
                )));
            }
        }

        // Validate phone format if present
        if let Some(phone) = &self.phone {
            if !is_valid_phone(phone) {
                return Err(CoreError::InvalidInput(format!(
                    "invalid phone format: {}",
                    phone
                )));
            }
        }

        // Validate custom fields
        if self.custom_fields.len() > MAX_CUSTOM_FIELDS_PER_ENTRY {
            return Err(CoreError::InvalidInput(format!(
                "too many custom fields ({}, max {})",
                self.custom_fields.len(),
                MAX_CUSTOM_FIELDS_PER_ENTRY
            )));
        }

        for field in &self.custom_fields {
            field.validate()?;
        }

        Ok(())
    }
}

/// Basic email validation (RFC 5322 simplified)
fn is_valid_email(email: &str) -> bool {
    if email.is_empty() || email.len() > 254 {
        return false;
    }
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }
    let local = parts[0];
    let domain = parts[1];

    // Local part validation
    if local.is_empty() || local.len() > 64 {
        return false;
    }

    // Domain validation
    if domain.is_empty() || !domain.contains('.') {
        return false;
    }

    // Check for valid characters
    let valid_local_chars = |c: char| c.is_alphanumeric() || "!#$%&'*+/=?^_`{|}~.-".contains(c);
    let valid_domain_chars = |c: char| c.is_alphanumeric() || c == '.' || c == '-';

    local.chars().all(valid_local_chars) && domain.chars().all(valid_domain_chars)
}

/// Basic phone validation (international format)
fn is_valid_phone(phone: &str) -> bool {
    if phone.is_empty() {
        return false;
    }
    // Remove common formatting characters
    let cleaned: String = phone.chars()
        .filter(|c| !matches!(c, ' ' | '-' | '(' | ')' | '.'))
        .collect();

    // Must be at least 7 digits, at most 15 (E.164 standard)
    if cleaned.len() < 7 || cleaned.len() > 16 {
        return false;
    }

    // First char can be +, rest must be digits
    let mut chars = cleaned.chars();
    match chars.next() {
        Some('+') | Some('0'..='9') => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_digit())
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
    ///
    /// The size limit is checked against the decoded (actual) data size,
    /// not the base64-encoded string length.
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

        // Calculate the decoded size from base64 length
        // Base64 encoding: 4 chars encode 3 bytes, so decoded_size ≈ encoded_size * 3 / 4
        // We account for padding by ignoring trailing '=' characters
        let base64_len = self.data_base64.trim_end_matches('=').len();
        let decoded_size = base64_len * 3 / 4;

        if decoded_size > MAX_ATTACHMENT_SIZE {
            return Err(CoreError::InvalidInput(format!(
                "attachment '{}' exceeds maximum size ({} bytes, max {} bytes)",
                self.filename,
                decoded_size,
                MAX_ATTACHMENT_SIZE
            )));
        }

        Ok(())
    }

    /// Returns the estimated decoded size of the attachment in bytes.
    pub fn decoded_size(&self) -> usize {
        let base64_len = self.data_base64.trim_end_matches('=').len();
        base64_len * 3 / 4
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
