//! The encrypted part of an entry: password, notes, TOTP, backup codes and attachments.

use chacha20poly1305::aead::Generate;
use serde::{Deserialize, Serialize};

use super::{
    current_timestamp, CustomField, MAX_ATTACHMENTS_PER_ENTRY, MAX_ATTACHMENT_SIZE,
    MAX_CUSTOM_FIELDS_PER_ENTRY, MAX_LABEL_LENGTH, MAX_NOTES_SIZE, MAX_PASSWORD_SIZE,
    MAX_TOTAL_ATTACHMENTS_SIZE,
};
use crate::error::{CoreError, CoreResult as Result};

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
        let bytes: [u8; TOTP_BACKUP_CODE_LENGTH] = Generate::generate();
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
        self.totp_backup_codes
            .iter()
            .map(|c| c.code.clone())
            .collect()
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
    let cleaned: String = phone
        .chars()
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
        if self.filename.contains("..")
            || self.filename.contains('/')
            || self.filename.contains('\\')
        {
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
                self.filename, decoded_size, MAX_ATTACHMENT_SIZE
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
