//! Shared utilities for Lilypad CLI and Desktop applications.
//!
//! This crate provides common functionality used by both the CLI and Desktop
//! interfaces, including key file management, clipboard operations, timestamp
//! formatting, and validation utilities.

pub mod clipboard;
pub mod constants;
pub mod keyfile;
pub mod time;
pub mod validation;

pub use clipboard::{clear_clipboard_after, copy_to_clipboard};
pub use constants::*;
pub use keyfile::{load_key, save_key, KeyFile};
pub use time::{current_timestamp, format_timestamp, format_timestamp_relative};
pub use validation::{
    has_weak_patterns, validate_entry_size, validate_password_strength, validate_vault_name,
    PasswordStrength, ValidationError,
};
