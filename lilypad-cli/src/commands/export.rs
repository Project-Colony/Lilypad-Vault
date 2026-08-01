//! Export commands for vaults and audit logs.

use super::import::{CsvEntry, EntryExport, VaultExport};
use super::utils::{
    atomic_write, decrypt_entry_secret, entry_type_label, load_vault_key, parse_date_to_timestamp,
};
use anyhow::{anyhow, Context, Result};
use csv::WriterBuilder;
use lilypad_common::time::format_timestamp_relative;
use lilypad_storage::LocalStore;
use std::cmp::Reverse;

/// Export a vault to a file.
pub fn export_vault(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    output: &str,
    format: &str,
    allow_plaintext: bool,
    master_password: Option<&str>,
) -> Result<()> {
    let format = format.to_lowercase();
    if format == "lily" {
        let payload = store.sync_payload(vault_name)?;
        atomic_write(output, payload)?;
        println!("Encrypted vault exported to {output}.");
        return Ok(());
    }

    if !allow_plaintext {
        return Err(anyhow!(
            "plaintext export requires --allow-plaintext (format: {format})"
        ));
    }

    // Security warning for plaintext export
    eprintln!("WARNING: You are about to export credentials in PLAINTEXT format.");
    eprintln!("   All passwords, TOTP secrets, and notes will be visible in the output file.");
    eprintln!("   Ensure you store or transmit this file securely, then delete it when done.");
    eprintln!();

    let key = load_vault_key(store, config, vault_name, master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    match format.as_str() {
        "json" => {
            let entries = vault
                .entries
                .iter()
                .map(|entry| {
                    let secret = decrypt_entry_secret(&key, entry)?;
                    Ok(EntryExport {
                        label: entry.label.clone(),
                        metadata: entry.metadata.clone(),
                        secret,
                        created_at: entry.created_at,
                        updated_at: entry.updated_at,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let export = VaultExport {
                name: vault.name.clone(),
                entries,
            };
            let payload = serde_json::to_vec_pretty(&export)?;
            atomic_write(output, payload)?;
            println!("Vault exported to {output}.");
        }
        "csv" => {
            let mut writer = WriterBuilder::new().from_path(output)?;
            for entry in &vault.entries {
                let secret = decrypt_entry_secret(&key, entry)?;
                let record = CsvEntry {
                    label: entry.label.clone(),
                    entry_type: entry_type_label(&entry.metadata.entry_type).to_string(),
                    username: entry.metadata.username.clone().unwrap_or_default(),
                    url: entry.metadata.url.clone().unwrap_or_default(),
                    folder: entry.metadata.folder.clone().unwrap_or_default(),
                    tags: entry.metadata.tags.join(";"),
                    password: secret.password,
                    notes: secret.notes.unwrap_or_default(),
                    totp_secret: secret.totp_secret.unwrap_or_default(),
                };
                writer.serialize(record)?;
            }
            writer.flush()?;
            println!("Vault exported to {output}.");
        }
        _ => {
            return Err(anyhow!("unsupported export format: {format}"));
        }
    }
    Ok(())
}

/// Export audit log to a file or stdout.
#[allow(clippy::too_many_arguments)]
pub fn export_audit_log(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    output: Option<&str>,
    format: &str,
    action_filter: Option<&str>,
    entry_filter: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
    limit: Option<usize>,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_vault_key(store, config, vault_name, master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    // Parse date filters
    let after_ts = after.map(parse_date_to_timestamp).transpose()?;
    let before_ts = before.map(parse_date_to_timestamp).transpose()?;

    // Filter events
    let mut events: Vec<_> = vault
        .audit_log
        .iter()
        .filter(|event| {
            // Action filter
            if let Some(action) = action_filter {
                if !event.action.contains(action) {
                    return false;
                }
            }
            // Entry filter
            if let Some(entry) = entry_filter {
                match &event.entry_label {
                    Some(label) if label.contains(entry) => {}
                    Some(_) => return false,
                    None => return false,
                }
            }
            // Date filters
            if let Some(after) = after_ts {
                if event.timestamp < after {
                    return false;
                }
            }
            if let Some(before) = before_ts {
                if event.timestamp > before {
                    return false;
                }
            }
            true
        })
        .collect();

    // Sort by timestamp (most recent first for display)
    events.sort_by_key(|event| Reverse(event.timestamp));

    // Apply limit
    if let Some(limit) = limit {
        events.truncate(limit);
    }

    let format = format.to_lowercase();
    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&events)?,
        "csv" => {
            let mut writer = WriterBuilder::new().from_writer(Vec::new());
            writer.serialize(("timestamp", "action", "entry_label", "date"))?;
            for event in &events {
                let date = format_timestamp_relative(event.timestamp);
                writer.serialize((
                    event.timestamp,
                    &event.action,
                    event.entry_label.as_deref().unwrap_or(""),
                    date,
                ))?;
            }
            writer.flush()?;
            String::from_utf8(writer.into_inner()?)?
        }
        "text" => {
            let mut output = String::new();
            output.push_str(&format!("Audit Log for '{}'\n", vault_name));
            output.push_str(&format!("{}\n\n", "=".repeat(40)));

            if events.is_empty() {
                output.push_str("No events found matching the filters.\n");
            } else {
                for event in &events {
                    let date = format_timestamp_relative(event.timestamp);
                    let entry = event.entry_label.as_deref().unwrap_or("-");
                    output.push_str(&format!("[{}] {} (entry: {})\n", date, event.action, entry));
                }
                output.push_str(&format!("\nTotal: {} events\n", events.len()));
            }
            output
        }
        _ => {
            return Err(anyhow!(
                "unsupported format: {format} (use json, csv, or text)"
            ))
        }
    };

    if let Some(path) = output {
        atomic_write(path, &content)?;
        println!("Exported {} audit events to {path}.", events.len());
    } else {
        println!("{content}");
    }

    Ok(())
}
