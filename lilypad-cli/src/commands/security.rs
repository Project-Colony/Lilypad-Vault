//! Security-related commands (key rotation, password change, breach check, TOTP, audit).

use anyhow::{anyhow, Context, Result};
use lilypad_common::{
    keyfile::{load_key, save_key, KeyFile},
    time::format_timestamp_relative,
    validation::{validate_password_strength, PasswordStrength},
};
use lilypad_core::{
    decrypt, derive_key, encrypt, EntryChangeType, KeyDerivationParams, KeyMaterial, KeyMetadata,
};
use lilypad_storage::LocalStore;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
#[cfg(unix)]
use std::fs::Permissions;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use totp_rs::{Algorithm, Secret, TOTP};

use super::utils::{
    decrypt_entry_secret, encrypt_entry_secret, key_path, load_vault_key,
    read_password_with_confirmation, OutputFormat,
};

/// Rotate the vault encryption key.
#[allow(clippy::too_many_arguments)]
pub fn rotate_key(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    use_master_password: bool,
    new_master_password: Option<&str>,
    skip_backup: bool,
    master_password: Option<&str>,
) -> Result<()> {
    let old_key = load_vault_key(store, config, vault_name, master_password)?;
    // Also load the old key file for backup purposes (if it exists)
    let old_key_file = load_key(&key_path(config), master_password).ok();
    let mut vault = store
        .load_vault(vault_name, &old_key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    // Create backup before rotation (unless --skip-backup is specified)
    if !skip_backup {
        let backup_dir = std::path::PathBuf::from(&config.data_dir).join("backups");
        fs::create_dir_all(&backup_dir)?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Backup vault file
        let vault_backup_path = backup_dir.join(format!("{vault_name}.{timestamp}.lily.bak"));
        let vault_payload = store.sync_payload(vault_name)?;
        fs::write(&vault_backup_path, &vault_payload)?;
        #[cfg(unix)]
        fs::set_permissions(&vault_backup_path, Permissions::from_mode(0o600))?;

        // Backup key file (if it exists)
        if let Some((_, ref old_kf)) = old_key_file {
            let key_backup_path = backup_dir.join(format!("key.{timestamp}.json.bak"));
            save_key(&key_backup_path, old_kf)?;
            #[cfg(unix)]
            fs::set_permissions(&key_backup_path, Permissions::from_mode(0o600))?;

            eprintln!(
                "Backup created (contains encrypted vault and key file):\n  - {}\n  - {}",
                vault_backup_path.display(),
                key_backup_path.display()
            );
        } else {
            eprintln!(
                "Backup created (contains encrypted vault):\n  - {}",
                vault_backup_path.display()
            );
        }
        eprintln!("   Store these backups securely or delete them after rotation is verified.");
    }

    let (new_key, key_file, metadata) = if use_master_password {
        let password = new_master_password
            .ok_or_else(|| anyhow!("new master password required (--new-master-password)"))?;
        // Warn if new password is weak
        let strength = validate_password_strength(password);
        if !strength.is_acceptable() {
            eprintln!("Warning: {}", strength.feedback());
        }
        let params = KeyDerivationParams::generate_adaptive();
        let key = derive_key(password, &params)?;
        let metadata = KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
            .with_embedded_kdf(&params);
        (key, KeyFile::from_kdf(params), metadata)
    } else {
        let key = KeyMaterial::generate();
        let metadata = KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305);
        let key_file = KeyFile::from_raw(&key);
        (key, key_file, metadata)
    };

    for entry in &mut vault.entries {
        let secret = decrypt_entry_secret(&old_key, entry)?;
        entry.ciphertext = encrypt_entry_secret(&new_key, &secret)?;
    }
    vault.key_metadata = metadata;
    store.save_vault(&vault, &new_key)?;
    save_key(&key_path(config), &key_file)?;
    println!("Vault '{vault_name}' re-encrypted with a new key.");
    Ok(())
}

/// Change the master password for a vault.
pub fn change_master_password(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    current_master_password: Option<&str>,
) -> Result<()> {
    let current_password = current_master_password.ok_or_else(|| {
        anyhow!("current master password required (--master-password or LILYPAD_MASTER_PASSWORD)")
    })?;

    // Load and verify current key
    let key = load_vault_key(store, config, vault_name, Some(current_password))?;

    // Verify vault exists and can be loaded
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    // Prompt for new password securely (hidden input)
    let new_password = read_password_with_confirmation(
        "Enter new master password: ",
        "Confirm new master password: ",
    )?;

    // Check password strength
    let strength = validate_password_strength(&new_password);
    if !strength.is_acceptable() {
        eprintln!("Warning: {}", strength.feedback());
    }

    // Generate new KDF parameters and derive key
    let new_params = KeyDerivationParams::generate_adaptive();
    let new_key = derive_key(&new_password, &new_params)?;

    // Re-encrypt all entries with the new key
    let mut updated_vault = vault.clone();
    for entry in &mut updated_vault.entries {
        let plaintext = decrypt(&key, &entry.ciphertext)?;
        entry.ciphertext = encrypt(&new_key, &plaintext)?;
    }

    // Update vault metadata with embedded KDF (V2)
    let new_metadata = KeyMetadata::new(&new_key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
        .with_embedded_kdf(&new_params);
    updated_vault.key_metadata = new_metadata;

    // Save the updated key file and vault
    let kp = key_path(config);
    let new_key_file = KeyFile::from_kdf(new_params);
    save_key(&kp, &new_key_file)?;
    store.save_vault(&updated_vault, &new_key)?;

    println!("Master password changed successfully for vault '{vault_name}'.");
    Ok(())
}

/// Check passwords against the HaveIBeenPwned database.
pub fn breach_check(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    entry_label: Option<&str>,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_vault_key(store, config, vault_name, master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    let entries_to_check: Vec<_> = if let Some(label) = entry_label {
        let entry = vault
            .find_entry(label)
            .ok_or_else(|| anyhow!("entry '{label}' not found"))?;
        vec![entry]
    } else {
        vault.entries.iter().collect()
    };

    if entries_to_check.is_empty() {
        println!("No entries to check.");
        return Ok(());
    }

    println!(
        "Checking {} entries against HaveIBeenPwned database...",
        entries_to_check.len()
    );
    println!("(This uses k-anonymity - only partial hashes are sent)\n");

    let client = reqwest::blocking::Client::builder()
        .user_agent("Lilypad-Password-Manager/0.1")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let mut breached_entries = Vec::new();
    let mut checked = 0;
    let total = entries_to_check.len();

    for entry in &entries_to_check {
        let secret = decrypt_entry_secret(&key, entry)?;

        match check_password_hibp(&client, &secret.password) {
            Ok(Some(count)) => {
                breached_entries.push((entry.label.clone(), count));
            }
            Ok(None) => {
                // Password not found in breaches
            }
            Err(e) => {
                eprintln!("Warning: Could not check '{}': {}", entry.label, e);
            }
        }

        checked += 1;
        if checked % 10 == 0 {
            eprint!("\rChecked {}/{} entries...", checked, total);
        }
    }

    if checked >= 10 {
        eprintln!(); // Clear progress line
    }

    println!("\nBreach check results for '{vault_name}':");
    println!("Entries checked: {}", checked);

    if breached_entries.is_empty() {
        println!("\nNo breached passwords found!");
    } else {
        println!("\nBREACHED PASSWORDS DETECTED:");
        println!("The following entries use passwords found in known data breaches:\n");
        for (label, count) in &breached_entries {
            println!("  - {} (seen {} times in breaches)", label, count);
        }
        println!("\nThese passwords should be changed immediately.");
    }

    Ok(())
}

/// Check a single password against HaveIBeenPwned using k-anonymity.
fn check_password_hibp(client: &reqwest::blocking::Client, password: &str) -> Result<Option<u64>> {
    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:X}", hash);

    let prefix = &hash_hex[..5];
    let suffix = &hash_hex[5..];

    let url = format!("https://api.pwnedpasswords.com/range/{}", prefix);
    let response = client.get(&url).send()?;

    if !response.status().is_success() {
        return Err(anyhow!("HIBP API returned status {}", response.status()));
    }

    let body = response.text()?;

    for line in body.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() == 2 && parts[0].eq_ignore_ascii_case(suffix) {
            let count: u64 = parts[1].parse().unwrap_or(0);
            return Ok(Some(count));
        }
    }

    Ok(None)
}

/// Show TOTP code for an entry.
pub fn show_totp(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
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
    let totp_secret = secret
        .totp_secret
        .ok_or_else(|| anyhow!("entry '{label}' does not have a TOTP secret"))?;
    let secret = Secret::Encoded(totp_secret);
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().map_err(|err| anyhow!("{err}"))?,
    )?;
    let code = totp.generate_current()?;
    println!("{code}");
    Ok(())
}

/// Manage TOTP backup codes.
pub fn backup_codes(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    generate: bool,
    verify: Option<&str>,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_vault_key(store, config, vault_name, master_password)?;
    let mut vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    let entry = vault
        .find_entry(label)
        .ok_or_else(|| anyhow!("entry '{label}' not found"))?;

    let mut secret = decrypt_entry_secret(&key, entry)?;

    if secret.totp_secret.is_none() {
        return Err(anyhow!(
            "entry '{label}' does not have TOTP enabled. Backup codes require TOTP."
        ));
    }

    if generate {
        eprintln!("WARNING: This will replace any existing backup codes!");
        eprintln!("   Store these codes in a safe place. They can only be shown once.");
        eprintln!();

        let codes = secret.generate_backup_codes();

        println!("===============================================");
        println!("   TOTP Backup Codes for '{label}'");
        println!("===============================================");
        println!();
        for (i, code) in codes.iter().enumerate() {
            let formatted = format!("{}-{}", &code[0..4], &code[4..8]);
            println!("   {:2}. {}", i + 1, formatted);
        }
        println!();
        println!("===============================================");
        println!("   Each code can only be used ONCE.");
        println!("   Store these codes securely offline.");
        println!("===============================================");

        let payload = serde_json::to_vec(&secret)?;
        let ciphertext = encrypt(&key, &payload)?;
        vault.update_entry(label, ciphertext)?;
        store.save_vault(&vault, &key)?;

        println!();
        println!("{} backup codes generated and saved.", codes.len());
    } else if let Some(code) = verify {
        if secret.use_backup_code(code) {
            println!("Backup code verified and consumed.");
            println!(
                "  Remaining unused codes: {}",
                secret.unused_backup_codes_count()
            );

            let payload = serde_json::to_vec(&secret)?;
            let ciphertext = encrypt(&key, &payload)?;
            vault.update_entry(label, ciphertext)?;
            store.save_vault(&vault, &key)?;

            if secret.backup_codes_low() {
                eprintln!();
                eprintln!(
                    "WARNING: Only {} backup codes remaining. Consider generating new codes.",
                    secret.unused_backup_codes_count()
                );
            }
        } else {
            return Err(anyhow!("Invalid or already used backup code."));
        }
    } else {
        let total = secret.totp_backup_codes.len();
        let unused = secret.unused_backup_codes_count();
        let used = total - unused;

        if total == 0 {
            println!("No backup codes generated for '{label}'.");
            println!("Use --generate to create backup codes.");
        } else {
            println!("Backup codes for '{label}':");
            println!("  Total:  {}", total);
            println!("  Used:   {}", used);
            println!("  Unused: {}", unused);

            if secret.backup_codes_low() {
                eprintln!();
                eprintln!("WARNING: Backup codes running low! Consider generating new codes.");
            }
        }
    }

    Ok(())
}

/// Audit vault for security issues.
pub fn audit_vault(
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

    let mut weak = Vec::new();
    let mut very_weak = Vec::new();
    let mut expired = Vec::new();
    let mut expiring_soon = Vec::new();
    let mut old_passwords = Vec::new();
    let mut duplicates: HashMap<String, Vec<String>> = HashMap::new();

    for entry in &vault.entries {
        let secret = decrypt_entry_secret(&key, entry)?;
        let strength = validate_password_strength(&secret.password);
        match strength {
            PasswordStrength::VeryWeak => very_weak.push(entry.label.clone()),
            PasswordStrength::Weak => weak.push(entry.label.clone()),
            _ => {}
        }
        duplicates
            .entry(secret.password)
            .or_default()
            .push(entry.label.clone());

        if entry.is_password_expired() {
            expired.push(entry.label.clone());
        } else if let Some(days) = entry.days_until_password_expires() {
            if (0..=7).contains(&days) {
                expiring_soon.push((entry.label.clone(), days));
            }
        }

        if entry.password_expires_at == 0 && entry.password_age_days() > 90 {
            old_passwords.push((entry.label.clone(), entry.password_age_days()));
        }
    }

    let reused: Vec<Vec<String>> = duplicates
        .into_iter()
        .filter(|(_, labels)| labels.len() > 1)
        .map(|(_, labels)| labels)
        .collect();

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "vault": vault_name,
                "total_entries": vault.entries.len(),
                "very_weak_passwords": very_weak,
                "weak_passwords": weak,
                "expired_passwords": expired,
                "expiring_soon": expiring_soon.iter().map(|(label, days)| {
                    serde_json::json!({ "label": label, "days_remaining": days })
                }).collect::<Vec<_>>(),
                "old_passwords": old_passwords.iter().map(|(label, days)| {
                    serde_json::json!({ "label": label, "age_days": days })
                }).collect::<Vec<_>>(),
                "reused_passwords": reused,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            println!("Audit results for '{vault_name}':");
            println!("Total entries: {}", vault.entries.len());
            println!();

            if very_weak.is_empty() && weak.is_empty() {
                println!("Password strength: All passwords meet minimum requirements.");
            } else {
                if !very_weak.is_empty() {
                    println!("Very weak passwords (<8 chars): {}", very_weak.join(", "));
                }
                if !weak.is_empty() {
                    println!(
                        "Weak passwords (8-11 chars, missing diversity): {}",
                        weak.join(", ")
                    );
                }
            }

            if !expired.is_empty() {
                println!("\nEXPIRED passwords: {}", expired.join(", "));
            }
            if !expiring_soon.is_empty() {
                println!("\nExpiring within 7 days:");
                for (label, days) in &expiring_soon {
                    println!("  - {} ({} days remaining)", label, days);
                }
            }
            if !old_passwords.is_empty() {
                println!("\nOld passwords (>90 days, no expiry set):");
                for (label, days) in &old_passwords {
                    println!("  - {} ({} days old)", label, days);
                }
            }

            if reused.is_empty() {
                println!("\nPassword reuse: No reused passwords detected.");
            } else {
                println!("\nPassword reuse detected:");
                for group in &reused {
                    println!("  - {}", group.join(", "));
                }
            }
        }
    }
    Ok(())
}

/// Show entry change history.
pub fn show_entry_history(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    label: &str,
    limit: usize,
    master_password: Option<&str>,
) -> Result<()> {
    let key = load_vault_key(store, config, vault_name, master_password)?;
    let vault = store
        .load_vault(vault_name, &key)
        .with_context(|| format!("vault not found: {vault_name}"))?;

    let entry = vault
        .find_entry(label)
        .ok_or_else(|| anyhow!("entry '{label}' not found"))?;

    println!("History for '{}' (showing up to {} records):\n", label, limit);

    if entry.history.is_empty() {
        println!("No history records found.");
        return Ok(());
    }

    for (i, record) in entry.get_history().take(limit).enumerate() {
        let timestamp = format_timestamp_relative(record.timestamp);
        let change_type = match &record.change_type {
            EntryChangeType::Created => "Created",
            EntryChangeType::PasswordChanged => "Password changed",
            EntryChangeType::MetadataUpdated => "Metadata updated",
            EntryChangeType::NotesUpdated => "Notes updated",
            EntryChangeType::Renamed => "Renamed",
            EntryChangeType::TotpUpdated => "TOTP updated",
            EntryChangeType::AttachmentsUpdated => "Attachments updated",
        };

        print!("{}. {} - {}", i + 1, timestamp, change_type);
        if let Some(desc) = &record.description {
            print!(" ({})", desc);
        }
        if record.previous_ciphertext.is_some() {
            print!(" [previous password stored]");
        }
        println!();
    }

    let total = entry.history.len();
    if total > limit {
        println!(
            "\n... and {} more records (use --limit to see more)",
            total - limit
        );
    }

    println!("\nTotal history records: {}", total);
    println!("Password changes: {}", entry.password_change_count());

    Ok(())
}
