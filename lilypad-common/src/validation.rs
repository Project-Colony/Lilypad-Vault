//! Validation utilities for Lilypad.
//!
//! Provides input validation for vault names, passwords, and entry sizes
//! to prevent security issues and ensure data integrity.

use std::fmt;

// Re-export size constants from lilypad-core for convenience
pub use lilypad_core::{
    MAX_ATTACHMENT_SIZE, MAX_NOTES_SIZE, MAX_PASSWORD_SIZE, MAX_TOTAL_ATTACHMENTS_SIZE,
};

/// Maximum total size for an entry (in bytes).
pub const MAX_ENTRY_SIZE: usize = 50 * 1024 * 1024; // 50 MB

/// Maximum length for vault names.
pub const MAX_VAULT_NAME_LENGTH: usize = 64;

/// Minimum password length for strong passwords.
pub const MIN_STRONG_PASSWORD_LENGTH: usize = 12;

/// Validation error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Value is empty or whitespace only.
    Empty(String),
    /// Value exceeds maximum allowed size.
    TooLarge { field: String, max: usize, actual: usize },
    /// Value contains invalid characters.
    InvalidCharacters { field: String, details: String },
    /// Password is too weak.
    WeakPassword(PasswordStrength),
    /// Generic validation failure.
    Invalid(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(field) => write!(f, "{field} cannot be empty"),
            Self::TooLarge { field, max, actual } => {
                write!(f, "{field} is too large ({actual} bytes, max {max} bytes)")
            }
            Self::InvalidCharacters { field, details } => {
                write!(f, "{field} contains invalid characters: {details}")
            }
            Self::WeakPassword(strength) => {
                write!(f, "password is too weak: {}", strength.feedback())
            }
            Self::Invalid(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Result type for validation operations.
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Validates a vault name for safety and correctness.
///
/// Vault names must:
/// - Not be empty
/// - Contain only alphanumeric characters, underscores, and hyphens
/// - Not exceed 64 characters
/// - Not start with a dot (hidden files)
/// - Not contain path traversal sequences
///
/// # Arguments
/// * `name` - The vault name to validate
///
/// # Returns
/// Ok(()) if valid, or a ValidationError describing the issue.
pub fn validate_vault_name(name: &str) -> ValidationResult<()> {
    let name = name.trim();

    if name.is_empty() {
        return Err(ValidationError::Empty("vault name".to_string()));
    }

    if name.len() > MAX_VAULT_NAME_LENGTH {
        return Err(ValidationError::TooLarge {
            field: "vault name".to_string(),
            max: MAX_VAULT_NAME_LENGTH,
            actual: name.len(),
        });
    }

    // Check for path traversal attempts
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(ValidationError::InvalidCharacters {
            field: "vault name".to_string(),
            details: "path separators and '..' are not allowed".to_string(),
        });
    }

    // Check for hidden files (starting with dot)
    if name.starts_with('.') {
        return Err(ValidationError::InvalidCharacters {
            field: "vault name".to_string(),
            details: "cannot start with a dot".to_string(),
        });
    }

    // Only allow safe characters
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ValidationError::InvalidCharacters {
            field: "vault name".to_string(),
            details: "only alphanumeric characters, underscores, and hyphens are allowed"
                .to_string(),
        });
    }

    Ok(())
}

/// Validates entry sizes to prevent DoS through oversized data.
///
/// # Arguments
/// * `password_len` - Length of the password in bytes
/// * `notes_len` - Length of the notes in bytes
/// * `attachments_total` - Total size of all attachments in bytes
///
/// # Returns
/// Ok(()) if all sizes are within limits, or a ValidationError.
pub fn validate_entry_size(
    password_len: usize,
    notes_len: usize,
    attachments_total: usize,
) -> ValidationResult<()> {
    if password_len > MAX_PASSWORD_SIZE {
        return Err(ValidationError::TooLarge {
            field: "password".to_string(),
            max: MAX_PASSWORD_SIZE,
            actual: password_len,
        });
    }

    if notes_len > MAX_NOTES_SIZE {
        return Err(ValidationError::TooLarge {
            field: "notes".to_string(),
            max: MAX_NOTES_SIZE,
            actual: notes_len,
        });
    }

    let total = password_len + notes_len + attachments_total;
    if total > MAX_ENTRY_SIZE {
        return Err(ValidationError::TooLarge {
            field: "entry".to_string(),
            max: MAX_ENTRY_SIZE,
            actual: total,
        });
    }

    Ok(())
}

/// Password strength assessment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordStrength {
    /// Very weak password (less than 8 characters).
    VeryWeak,
    /// Weak password (8-11 characters, missing character types).
    Weak,
    /// Fair password (12+ characters but missing diversity).
    Fair,
    /// Strong password (12+ characters with good diversity).
    Strong,
    /// Very strong password (16+ characters with full diversity).
    VeryStrong,
}

impl PasswordStrength {
    /// Returns a human-readable feedback message.
    pub fn feedback(&self) -> &'static str {
        match self {
            Self::VeryWeak => "password is too short (minimum 8 characters)",
            Self::Weak => "password should be at least 12 characters with mixed case",
            Self::Fair => "consider adding numbers and special characters",
            Self::Strong => "good password strength",
            Self::VeryStrong => "excellent password strength",
        }
    }

    /// Returns true if the password strength is acceptable.
    pub fn is_acceptable(&self) -> bool {
        matches!(self, Self::Fair | Self::Strong | Self::VeryStrong)
    }

    /// Returns a numeric score (0-4).
    pub fn score(&self) -> u8 {
        match self {
            Self::VeryWeak => 0,
            Self::Weak => 1,
            Self::Fair => 2,
            Self::Strong => 3,
            Self::VeryStrong => 4,
        }
    }
}

/// Validates password strength and returns the assessment.
///
/// Checks for:
/// - Minimum length
/// - Character diversity (lowercase, uppercase, digits, symbols)
/// - Common patterns (not implemented yet)
///
/// # Arguments
/// * `password` - The password to validate
///
/// # Returns
/// The password strength assessment.
pub fn validate_password_strength(password: &str) -> PasswordStrength {
    let len = password.len();
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_symbol = password
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && !c.is_whitespace());

    let diversity = [has_lower, has_upper, has_digit, has_symbol]
        .iter()
        .filter(|&&x| x)
        .count();

    if len < 8 {
        return PasswordStrength::VeryWeak;
    }

    if len < 12 {
        return if diversity >= 3 {
            PasswordStrength::Fair
        } else {
            PasswordStrength::Weak
        };
    }

    if len < 16 {
        return if diversity >= 3 {
            PasswordStrength::Strong
        } else {
            PasswordStrength::Fair
        };
    }

    // 16+ characters
    if diversity == 4 {
        PasswordStrength::VeryStrong
    } else if diversity >= 3 {
        PasswordStrength::Strong
    } else {
        PasswordStrength::Fair
    }
}

/// Validates a label (entry name, tag, folder name).
///
/// # Arguments
/// * `label` - The label to validate
/// * `field_name` - Name of the field for error messages
/// * `max_length` - Maximum allowed length
///
/// # Returns
/// Ok(()) if valid, or a ValidationError.
pub fn validate_label(label: &str, field_name: &str, max_length: usize) -> ValidationResult<()> {
    let label = label.trim();

    if label.is_empty() {
        return Err(ValidationError::Empty(field_name.to_string()));
    }

    if label.len() > max_length {
        return Err(ValidationError::TooLarge {
            field: field_name.to_string(),
            max: max_length,
            actual: label.len(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_vault_name_valid() {
        assert!(validate_vault_name("primary").is_ok());
        assert!(validate_vault_name("my-vault").is_ok());
        assert!(validate_vault_name("vault_2024").is_ok());
        assert!(validate_vault_name("MyVault123").is_ok());
    }

    #[test]
    fn test_validate_vault_name_empty() {
        assert!(matches!(
            validate_vault_name(""),
            Err(ValidationError::Empty(_))
        ));
        assert!(matches!(
            validate_vault_name("   "),
            Err(ValidationError::Empty(_))
        ));
    }

    #[test]
    fn test_validate_vault_name_path_traversal() {
        assert!(matches!(
            validate_vault_name("../etc/passwd"),
            Err(ValidationError::InvalidCharacters { .. })
        ));
        assert!(matches!(
            validate_vault_name(".."),
            Err(ValidationError::InvalidCharacters { .. })
        ));
        assert!(matches!(
            validate_vault_name("foo/bar"),
            Err(ValidationError::InvalidCharacters { .. })
        ));
        assert!(matches!(
            validate_vault_name("foo\\bar"),
            Err(ValidationError::InvalidCharacters { .. })
        ));
    }

    #[test]
    fn test_validate_vault_name_hidden() {
        assert!(matches!(
            validate_vault_name(".hidden"),
            Err(ValidationError::InvalidCharacters { .. })
        ));
    }

    #[test]
    fn test_validate_vault_name_special_chars() {
        assert!(matches!(
            validate_vault_name("vault@home"),
            Err(ValidationError::InvalidCharacters { .. })
        ));
        assert!(matches!(
            validate_vault_name("vault name"),
            Err(ValidationError::InvalidCharacters { .. })
        ));
    }

    #[test]
    fn test_validate_vault_name_too_long() {
        let long_name = "a".repeat(65);
        assert!(matches!(
            validate_vault_name(&long_name),
            Err(ValidationError::TooLarge { .. })
        ));
    }

    #[test]
    fn test_validate_entry_size_valid() {
        assert!(validate_entry_size(100, 1000, 100000).is_ok());
    }

    #[test]
    fn test_validate_entry_size_password_too_large() {
        assert!(matches!(
            validate_entry_size(MAX_PASSWORD_SIZE + 1, 0, 0),
            Err(ValidationError::TooLarge { field, .. }) if field == "password"
        ));
    }

    #[test]
    fn test_validate_entry_size_notes_too_large() {
        assert!(matches!(
            validate_entry_size(0, MAX_NOTES_SIZE + 1, 0),
            Err(ValidationError::TooLarge { field, .. }) if field == "notes"
        ));
    }

    #[test]
    fn test_password_strength_very_weak() {
        assert_eq!(validate_password_strength("short"), PasswordStrength::VeryWeak);
        assert_eq!(validate_password_strength("1234567"), PasswordStrength::VeryWeak);
    }

    #[test]
    fn test_password_strength_weak() {
        assert_eq!(validate_password_strength("password12"), PasswordStrength::Weak);
    }

    #[test]
    fn test_password_strength_fair() {
        assert_eq!(
            validate_password_strength("Password123!"),
            PasswordStrength::Strong
        );
    }

    #[test]
    fn test_password_strength_strong() {
        assert_eq!(
            validate_password_strength("MyStr0ng!Pass"),
            PasswordStrength::Strong
        );
    }

    #[test]
    fn test_password_strength_very_strong() {
        assert_eq!(
            validate_password_strength("MyV3ryStr0ng!Password"),
            PasswordStrength::VeryStrong
        );
    }

    #[test]
    fn test_password_strength_is_acceptable() {
        assert!(!PasswordStrength::VeryWeak.is_acceptable());
        assert!(!PasswordStrength::Weak.is_acceptable());
        assert!(PasswordStrength::Fair.is_acceptable());
        assert!(PasswordStrength::Strong.is_acceptable());
        assert!(PasswordStrength::VeryStrong.is_acceptable());
    }
}
