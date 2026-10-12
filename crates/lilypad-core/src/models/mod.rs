//! The vault and entry data model.

mod entry;
mod keys;
mod secret;
mod vault;

#[cfg(test)]
mod tests;

use chacha20poly1305::aead::Generate;
use std::time::{SystemTime, UNIX_EPOCH};

pub use entry::{
    CustomField, CustomFieldType, Entry, EntryChangeType, EntryColor, EntryHistoryRecord,
    EntryMetadata, EntryType,
};
pub use keys::{EmbeddedKdfParams, KeyMetadata};
pub use secret::{
    Attachment, EntrySecret, TotpBackupCode, TOTP_BACKUP_CODE_COUNT, TOTP_BACKUP_CODE_LENGTH,
};
pub use vault::{AuditEvent, MergeResult, Tombstone, Vault, MAX_CLOCK_SKEW_SECS};

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

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn generate_id() -> String {
    let bytes: [u8; 16] = Generate::generate();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
