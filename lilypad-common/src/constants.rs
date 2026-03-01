//! Centralized constants for Lilypad.
//!
//! This module contains all configurable constants used throughout the Lilypad
//! codebase. Centralizing these values makes it easier to tune behavior and
//! ensures consistency across all components.

// =============================================================================
// SECURITY CONSTANTS
// =============================================================================

/// Maximum login attempts before lockout.
pub const MAX_LOGIN_ATTEMPTS: u32 = 5;

/// Lockout duration in seconds (5 minutes).
pub const LOCKOUT_DURATION_SECS: u64 = 300;

/// Default clipboard timeout in seconds.
pub const DEFAULT_CLIPBOARD_TIMEOUT_SECS: u64 = 30;

/// Default auto-lock timeout in minutes.
pub const DEFAULT_AUTO_LOCK_MINUTES: u32 = 10;

// =============================================================================
// CRYPTOGRAPHY CONSTANTS
// =============================================================================

/// Argon2 memory cost in KiB (64 MiB default).
pub const ARGON2_MEMORY_KIB: u32 = 64 * 1024;

/// Argon2 iteration count.
pub const ARGON2_ITERATIONS: u32 = 3;

/// Argon2 parallelism fallback (adaptive mode uses up to `min(cpu_count, 4)`).
pub const ARGON2_PARALLELISM: u32 = 1;

/// Minimum Argon2 memory in KiB (16 MiB).
pub const ARGON2_MIN_MEMORY_KIB: u32 = 16 * 1024;

/// Maximum Argon2 memory in KiB (256 MiB).
pub const ARGON2_MAX_MEMORY_KIB: u32 = 256 * 1024;

/// Salt length in bytes.
pub const SALT_LENGTH: usize = 16;

/// Nonce length in bytes for XChaCha20-Poly1305.
pub const NONCE_LENGTH: usize = 24;

/// Key length in bytes.
pub const KEY_LENGTH: usize = 32;

// =============================================================================
// SIZE LIMITS
// =============================================================================

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

/// Maximum total size for an entry (in bytes).
pub const MAX_ENTRY_SIZE: usize = 50 * 1024 * 1024; // 50 MB

/// Maximum length for vault names.
pub const MAX_VAULT_NAME_LENGTH: usize = 64;

// =============================================================================
// PASSWORD VALIDATION CONSTANTS
// =============================================================================

/// Minimum password length for strong passwords.
pub const MIN_STRONG_PASSWORD_LENGTH: usize = 12;

/// Minimum password length.
pub const MIN_PASSWORD_LENGTH: usize = 8;

/// Length threshold for very strong passwords.
pub const VERY_STRONG_PASSWORD_LENGTH: usize = 16;

// =============================================================================
// UI CONSTANTS
// =============================================================================

/// Status message display duration in seconds.
pub const STATUS_MESSAGE_DURATION_SECS: u64 = 5;

/// Default password generator length.
pub const DEFAULT_GENERATOR_LENGTH: usize = 16;

/// Minimum password generator length.
pub const MIN_GENERATOR_LENGTH: usize = 8;

/// Maximum password generator length.
pub const MAX_GENERATOR_LENGTH: usize = 64;

// =============================================================================
// VAULT FORMAT CONSTANTS
// =============================================================================

/// Current vault format version.
pub const VAULT_FORMAT_VERSION: u32 = 1;

/// Maximum supported vault format version.
pub const MAX_VAULT_FORMAT_VERSION: u32 = 1;

/// Current settings format version.
pub const SETTINGS_FORMAT_VERSION: u32 = 1;
