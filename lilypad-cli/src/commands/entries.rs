//! Entry management commands (add, list, get, update, remove, search).

use anyhow::{anyhow, Context, Result};
use lilypad_common::{
    clipboard::copy_to_clipboard_with_timeout,
    time::format_timestamp_relative,
    validation::validate_password_strength,
};
use lilypad_core::{Entry, EntrySecret};
use lilypad_storage::LocalStore;
use rand::RngExt;
use std::path::PathBuf;

use super::utils::{
    apply_metadata_updates, build_metadata, decrypt_entry_secret, encrypt_entry_secret,
    entry_type_label, load_attachments, load_vault_key, OutputFormat,
};

/// Add a new entry to a vault.
#[allow(clippy::too_many_arguments)]
pub fn add_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    value: &str,
    username: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    tags: Vec<String>,
    folder: Option<String>,
    entry_type: Option<String>,
    totp_secret: Option<String>,
    attachment: Vec<PathBuf>,
    require_strong: bool,
    expires_in: u32,
    master_password: Option<&str>,
) -> Result<()> {
    // Check password strength
    let strength = validate_password_strength(value);
    if require_strong && !strength.is_acceptable() {
        return Err(anyhow!(
            "password is too weak: {}. Use a stronger password or remove --require-strong.",
            strength.feedback()
        ));
    } else if !strength.is_acceptable() {
        eprintln!("Warning: {}", strength.feedback());
    }

    let key = load_vault_key(store, config, vault_name, master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let metadata = build_metadata(username, url, tags, folder, entry_type)?;
    // Validate metadata
    metadata.validate()?;

    let mut secret = EntrySecret::new(value);
    secret.notes = notes;
    secret.totp_secret = totp_secret;
    secret.attachments = load_attachments(attachment)?;
    // Validate secret sizes
    secret.validate()?;

    let ciphertext = encrypt_entry_secret(&key, &secret)?;
    let mut entry = Entry::new_with_metadata(label, metadata, ciphertext);
    if expires_in > 0 {
        entry.set_password_expiry_days(expires_in);
    }
    vault.add_entry(entry)?;
    store.save_vault(&vault, &key)?;
    println!("Entry '{label}' added to vault '{vault_name}'.");
    if expires_in > 0 {
        println!("Password will expire in {expires_in} days.");
    }
    Ok(())
}

/// List all entries in a vault.
pub fn list_entries(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    output_format: OutputFormat,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_vault_key(store, config, vault_name, master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    match output_format {
        OutputFormat::Json => {
            let entries: Vec<_> = vault
                .entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "label": e.label,
                        "type": entry_type_label(&e.metadata.entry_type),
                        "username": e.metadata.username,
                        "url": e.metadata.url,
                        "folder": e.metadata.folder,
                        "tags": e.metadata.tags,
                        "created_at": e.created_at,
                        "updated_at": e.updated_at,
                    })
                })
                .collect();
            let json = serde_json::json!({
                "vault": vault_name,
                "entries": entries,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            if vault.entries.is_empty() {
                println!("No entries in '{vault_name}'.");
                return Ok(());
            }
            println!("Entries in '{vault_name}':");
            for entry in &vault.entries {
                let tags = if entry.metadata.tags.is_empty() {
                    "none".to_string()
                } else {
                    entry.metadata.tags.join(", ")
                };
                let entry_type = entry_type_label(&entry.metadata.entry_type);
                let folder = entry.metadata.folder.as_deref().unwrap_or("none");
                let updated = format_timestamp_relative(entry.updated_at);
                println!(
                    "- {} (type: {}, folder: {}, tags: {}, updated: {})",
                    entry.label, entry_type, folder, tags, updated
                );
            }
        }
    }
    Ok(())
}

/// Get an entry from a vault.
#[allow(clippy::too_many_arguments)]
pub fn get_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    copy: bool,
    clipboard_timeout: u64,
    show_password: bool,
    output_format: OutputFormat,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_vault_key(store, config, vault_name, master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let entry = vault
        .find_entry(label)
        .ok_or_else(|| anyhow!("entry '{label}' not found"))?;
    let secret = decrypt_entry_secret(&key, entry)?;

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "label": entry.label,
                "type": entry_type_label(&entry.metadata.entry_type),
                "username": entry.metadata.username,
                "url": entry.metadata.url,
                "folder": entry.metadata.folder,
                "tags": entry.metadata.tags,
                "password": if show_password { Some(&secret.password) } else { None },
                "notes": secret.notes,
                "totp_secret": if show_password { secret.totp_secret.as_ref() } else { None },
                "attachments_count": secret.attachments.len(),
                "created_at": entry.created_at,
                "updated_at": entry.updated_at,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            println!("Label: {}", entry.label);
            println!("Type: {}", entry_type_label(&entry.metadata.entry_type));
            if let Some(username) = &entry.metadata.username {
                println!("Username: {username}");
            }
            if let Some(url) = &entry.metadata.url {
                println!("URL: {url}");
            }
            if let Some(folder) = &entry.metadata.folder {
                println!("Folder: {folder}");
            }
            if !entry.metadata.tags.is_empty() {
                println!("Tags: {}", entry.metadata.tags.join(", "));
            }
            // Mask password by default for security
            if show_password {
                println!("Password: {}", secret.password);
            } else {
                println!("Password: ******** (use --show-password to reveal)");
            }
            if let Some(notes) = &secret.notes {
                println!("Notes: {notes}");
            }
            if let Some(totp_secret) = &secret.totp_secret {
                if show_password {
                    println!("TOTP secret: {totp_secret}");
                } else {
                    println!("TOTP secret: ******** (use --show-password to reveal)");
                }
            }
            if !secret.attachments.is_empty() {
                println!("Attachments: {}", secret.attachments.len());
            }
            println!("Updated: {}", format_timestamp_relative(entry.updated_at));
        }
    }

    if copy {
        copy_to_clipboard_with_timeout(&secret.password, clipboard_timeout)?;
        if output_format == OutputFormat::Text {
            println!("Password copied to clipboard for {clipboard_timeout} seconds.");
        }
    }
    Ok(())
}

/// Update an existing entry.
#[allow(clippy::too_many_arguments)]
pub fn update_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    value: &str,
    username: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    tags: Vec<String>,
    folder: Option<String>,
    entry_type: Option<String>,
    totp_secret: Option<String>,
    attachment: Vec<PathBuf>,
    require_strong: bool,
    expires_in: u32,
    master_password: Option<&str>,
) -> Result<()> {
    // Check password strength
    let strength = validate_password_strength(value);
    if require_strong && !strength.is_acceptable() {
        return Err(anyhow!(
            "password is too weak: {}. Use a stronger password or remove --require-strong.",
            strength.feedback()
        ));
    } else if !strength.is_acceptable() {
        eprintln!("Warning: {}", strength.feedback());
    }

    let key = load_vault_key(store, config, vault_name, master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let (mut metadata, mut secret, old_password) = {
        let existing = vault
            .find_entry(label)
            .ok_or_else(|| anyhow!("entry '{label}' not found"))?;
        let decrypted = decrypt_entry_secret(&key, existing)?;
        let old_pw = decrypted.password.clone();
        (existing.metadata.clone(), decrypted, old_pw)
    };
    apply_metadata_updates(&mut metadata, username, url, tags, folder, entry_type)?;
    // Validate metadata
    metadata.validate()?;

    let password_changed = value != old_password;
    secret.password = value.to_string();
    if notes.is_some() {
        secret.notes = notes;
    }
    if totp_secret.is_some() {
        secret.totp_secret = totp_secret;
    }
    let new_attachments = load_attachments(attachment)?;
    if !new_attachments.is_empty() {
        secret.attachments = new_attachments;
    }
    // Validate secret sizes
    secret.validate()?;

    let ciphertext = encrypt_entry_secret(&key, &secret)?;
    vault
        .update_entry(label, ciphertext)
        .with_context(|| format!("entry '{label}' not found"))?;
    if let Some(entry) = vault.entries.iter_mut().find(|entry| entry.label == label) {
        entry.metadata = metadata;
        // Update password change tracking
        if password_changed {
            entry.record_password_change();
        }
        // Update expiry if specified
        if expires_in > 0 {
            entry.set_password_expiry_days(expires_in);
        }
    }
    store.save_vault(&vault, &key)?;
    println!("Entry '{label}' updated.");
    if password_changed && expires_in > 0 {
        println!("Password will expire in {expires_in} days.");
    }
    Ok(())
}

/// Remove an entry from a vault.
pub fn remove_entry(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_vault_key(store, config, vault_name, master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    vault
        .remove_entry(label)
        .with_context(|| format!("entry '{label}' not found"))?;
    store.save_vault(&vault, &key)?;
    println!("Entry '{label}' removed.");
    Ok(())
}

/// Search entries by label.
pub fn search_entries(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    query: &str,
    output_format: OutputFormat,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_vault_key(store, config, vault_name, master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;
    let matches = vault.search_entries(query);

    match output_format {
        OutputFormat::Json => {
            let results: Vec<_> = matches
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "label": e.label,
                        "type": entry_type_label(&e.metadata.entry_type),
                        "username": e.metadata.username,
                        "url": e.metadata.url,
                        "folder": e.metadata.folder,
                        "tags": e.metadata.tags,
                        "updated_at": e.updated_at,
                    })
                })
                .collect();
            let json = serde_json::json!({
                "query": query,
                "results": results,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            if matches.is_empty() {
                println!("No entries match '{query}'.");
                return Ok(());
            }
            println!("Results for '{query}':");
            for entry in matches {
                let updated = format_timestamp_relative(entry.updated_at);
                println!("- {} (updated: {})", entry.label, updated);
            }
        }
    }
    Ok(())
}

/// Generate a random password.
pub fn generate_password(
    length: usize,
    uppercase: bool,
    lowercase: bool,
    digits: bool,
    symbols: bool,
    copy: bool,
    clipboard_timeout: u64,
) -> Result<()> {
    if length == 0 {
        return Err(anyhow!("password length must be greater than 0"));
    }
    if length > 1024 {
        return Err(anyhow!("password length must be at most 1024"));
    }
    if !uppercase && !lowercase && !digits && !symbols {
        return Err(anyhow!(
            "at least one character set must be enabled (uppercase, lowercase, digits, symbols)"
        ));
    }

    let mut charset = String::new();
    if uppercase {
        charset.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    }
    if lowercase {
        charset.push_str("abcdefghijklmnopqrstuvwxyz");
    }
    if digits {
        charset.push_str("0123456789");
    }
    if symbols {
        charset.push_str("!@#$%^&*()-_=+[]{}|;:,.<>?");
    }

    let charset_bytes = charset.as_bytes();
    let mut rng = rand::rng();
    let password: String = (0..length)
        .map(|_| {
            let idx = rng.random_range(0..charset_bytes.len());
            charset_bytes[idx] as char
        })
        .collect();

    // Display password strength
    let strength = validate_password_strength(&password);
    println!("Generated password: {password}");
    println!("Strength: {}", strength.feedback());

    if copy {
        copy_to_clipboard_with_timeout(&password, clipboard_timeout)?;
        println!("Password copied to clipboard for {clipboard_timeout} seconds.");
    }

    Ok(())
}
