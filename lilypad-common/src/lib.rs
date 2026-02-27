//! Shared utilities for Lilypad CLI and Desktop applications.
//!
//! This crate provides common functionality used by both the CLI and Desktop
//! interfaces, including key file management, clipboard operations, timestamp
//! formatting, and validation utilities.

pub mod clipboard;
pub mod constants;
pub mod health;
pub mod keyfile;
pub mod search;
pub mod time;
pub mod validation;

pub use clipboard::{clear_clipboard_after, copy_to_clipboard};
pub use constants::*;
pub use health::{
    analyze_vault_health, detect_duplicates, get_expiring_entries, EntryHealthData,
    ExpiryNotification, ExpirySeverity, HealthGrade, HealthIssue, HealthReport, HealthScore,
    IssueCategory, IssueSeverity, ScoreBreakdown, VaultStats,
};
pub use keyfile::{load_key, save_key, KeyFile};
pub use search::{
    fuzzy_match, AdvancedSearch, FuzzyMatchResult, SearchFilter, SearchResult, SortField, SortOrder,
};
pub use time::{current_timestamp, format_timestamp, format_timestamp_relative};
pub use validation::{
    has_weak_patterns, validate_entry_size, validate_password_strength, validate_vault_name,
    PasswordStrength, ValidationError,
};
