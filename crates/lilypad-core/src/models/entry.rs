//! Entries: labels, metadata, colours, custom fields and change history.

use serde::{Deserialize, Serialize};

use super::{
    current_timestamp, generate_id, MAX_CUSTOM_FIELD_NAME_LENGTH, MAX_CUSTOM_FIELD_VALUE_SIZE,
    MAX_FOLDER_LENGTH, MAX_TAGS_PER_ENTRY, MAX_TAG_LENGTH, MAX_URL_LENGTH, MAX_USERNAME_LENGTH,
};
use crate::crypto::Ciphertext;
use crate::error::{CoreError, CoreResult as Result};

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
    /// Unix timestamp when this entry was soft-deleted (moved to Trash);
    /// 0 means the entry is live. Kept in cleartext metadata so Trash can be
    /// listed and filtered without decrypting.
    #[serde(default)]
    pub deleted_at: u64,
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
            deleted_at: 0,
        };
        entry
            .history
            .push(EntryHistoryRecord::new(EntryChangeType::Created));
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
            self.history
                .push(EntryHistoryRecord::new(EntryChangeType::PasswordChanged));
        }
        self.updated_at = current_timestamp();
    }

    /// Gets the history of this entry, most recent first.
    pub fn get_history(&self) -> impl Iterator<Item = &EntryHistoryRecord> {
        self.history.iter().rev()
    }

    /// Gets the number of password changes recorded in history.
    pub fn password_change_count(&self) -> usize {
        self.history
            .iter()
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
        self.history
            .push(EntryHistoryRecord::new(EntryChangeType::PasswordChanged));
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

    pub(super) fn matches_query(&self, needle: &str) -> bool {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EntryType {
    #[default]
    Login,
    Card,
    Identity,
    SecureNote,
    SoftwareLicense,
    Wifi,
    Server,
    Custom,
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
