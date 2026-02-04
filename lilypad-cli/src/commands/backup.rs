//! Backup management commands.

use anyhow::Result;
use lilypad_common::time::format_timestamp_relative;
use lilypad_storage::LocalStore;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};

use super::utils::format_size;

/// Create a backup of a vault.
pub fn create_backup(store: &LocalStore, vault_name: &str) -> Result<()> {
    let backup_name = store.create_backup(vault_name)?;
    println!("Backup created: {}", backup_name);
    println!(
        "  Location: {}",
        store.backup_dir().join(&backup_name).display()
    );
    Ok(())
}

/// List backups for a vault or all vaults.
pub fn list_backups(store: &LocalStore, vault_name: Option<&str>) -> Result<()> {
    let backup_dir = store.backup_dir();
    if !backup_dir.exists() {
        println!("No backups found.");
        return Ok(());
    }

    // If vault name specified, list only its backups
    if let Some(name) = vault_name {
        let backups = store.list_backups(name)?;
        if backups.is_empty() {
            println!("No backups found for vault '{}'.", name);
            return Ok(());
        }

        println!("Backups for vault '{}':\n", name);
        for backup in backups {
            let timestamp = format_timestamp_relative(backup.created_at);
            let size = format_size(backup.size_bytes);
            println!("  {} ({}, {})", backup.filename, timestamp, size);
        }
        return Ok(());
    }

    // List all backups grouped by vault - optimized to avoid redundant disk reads
    let mut all_backups: HashMap<String, Vec<lilypad_storage::BackupInfo>> = HashMap::new();
    let mut processed_vaults: HashSet<String> = HashSet::new();

    for entry in fs::read_dir(&backup_dir)? {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();
        if filename.ends_with(".backup") {
            if let Some(vault) = filename.split('_').next() {
                // Only process each vault once
                if processed_vaults.insert(vault.to_string()) {
                    if let Ok(backups) = store.list_backups(vault) {
                        all_backups.insert(vault.to_string(), backups);
                    }
                }
            }
        }
    }

    if all_backups.is_empty() {
        println!("No backups found.");
        return Ok(());
    }

    println!("All backups:\n");
    for (vault, backups) in all_backups {
        println!("Vault '{}':", vault);
        for backup in backups {
            let timestamp = format_timestamp_relative(backup.created_at);
            let size = format_size(backup.size_bytes);
            println!("  {} ({}, {})", backup.filename, timestamp, size);
        }
        println!();
    }

    Ok(())
}

/// Restore a vault from backup.
pub fn restore_backup(store: &LocalStore, backup_name: &str, force: bool) -> Result<()> {
    if !force {
        eprintln!("WARNING: This will replace the current vault with the backup.");
        eprintln!("   The current vault will be backed up first.");
        eprint!("   Continue? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Restore cancelled.");
            return Ok(());
        }
    }

    match store.restore_backup(backup_name)? {
        Some(old_backup) => {
            println!("Backup restored successfully.");
            println!("  Previous vault backed up as: {}", old_backup);
        }
        None => {
            println!("Backup restored successfully.");
        }
    }

    Ok(())
}

/// Prune old backups, keeping only the most recent ones.
pub fn prune_backups(store: &LocalStore, vault_name: &str, keep: usize) -> Result<()> {
    let deleted = store.prune_backups(vault_name, keep)?;
    if deleted == 0 {
        println!("No old backups to delete (keeping {} most recent).", keep);
    } else {
        println!(
            "Deleted {} old backup(s), kept {} most recent.",
            deleted, keep
        );
    }
    Ok(())
}
