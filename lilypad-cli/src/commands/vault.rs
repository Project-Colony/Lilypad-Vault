//! Vault management commands (init, list, rename, delete).

use anyhow::{anyhow, Result};
use lilypad_common::{
    keyfile::{save_key, KeyFile},
    validation::validate_password_strength,
};
use lilypad_core::{derive_key, KeyDerivationParams, KeyMaterial, KeyMetadata, Vault};
use lilypad_storage::LocalStore;
use std::io::{self, Write};

use super::utils::{key_path, OutputFormat};

/// Initialize a new vault.
pub fn init_vault(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault: &str,
    use_master_password: bool,
    master_password: Option<&str>,
) -> Result<()> {
    let kp = key_path(config);
    if kp.exists() {
        return Err(anyhow!("key already initialized at {}", kp.display()));
    }

    let (key, key_file) = if use_master_password {
        let password = master_password.ok_or_else(|| {
            anyhow!("master password required (--master-password or LILYPAD_MASTER_PASSWORD)")
        })?;
        // Warn if password is weak
        let strength = validate_password_strength(password);
        if !strength.is_acceptable() {
            eprintln!("Warning: {}", strength.feedback());
        }
        let params = KeyDerivationParams::generate();
        let key = derive_key(password, &params)?;
        (key, KeyFile::from_kdf(params))
    } else {
        let key = KeyMaterial::generate();
        let key_file = KeyFile::from_raw(&key);
        (key, key_file)
    };
    save_key(&kp, &key_file)?;

    let metadata = if use_master_password {
        let kdf_params = match &key_file {
            KeyFile::Kdf { params } => params,
            _ => unreachable!(),
        };
        KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
            .with_embedded_kdf(kdf_params)
    } else {
        KeyMetadata::new(&key, lilypad_core::CryptoAlgorithm::XChaCha20Poly1305)
    };
    let new_vault = Vault::new(vault, metadata);
    store.save_vault(&new_vault, &key)?;

    println!("Vault '{}' initialized in {}.", new_vault.name, config.data_dir);
    Ok(())
}

/// List all vaults.
pub fn list_vaults(store: &LocalStore, output_format: OutputFormat) -> Result<()> {
    let vaults = store.list_vaults()?;

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::json!({ "vaults": vaults });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            if vaults.is_empty() {
                println!("No vaults found.");
                return Ok(());
            }
            println!("Vaults:");
            for name in vaults {
                println!("- {name}");
            }
        }
    }
    Ok(())
}

/// Rename a vault.
pub fn rename_vault(store: &LocalStore, from: &str, to: &str) -> Result<()> {
    store.rename_vault(from, to)?;
    println!("Vault '{from}' renamed to '{to}'.");
    Ok(())
}

/// Delete a vault with optional confirmation.
pub fn delete_vault(store: &LocalStore, vault: &str, force: bool) -> Result<()> {
    if !force {
        print!("Are you sure you want to delete vault '{vault}'? This cannot be undone. [y/N] ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("Deletion cancelled.");
            return Ok(());
        }
    }
    store.delete_vault(vault)?;
    println!("Vault '{vault}' deleted.");
    Ok(())
}

/// Verify vault integrity.
pub fn verify_vault(store: &LocalStore, vault_name: &str) -> Result<()> {
    let vaults_dir = store
        .backup_dir()
        .parent()
        .ok_or_else(|| anyhow!("invalid backup directory structure"))?
        .join("vaults");
    let vault_path = vaults_dir.join(format!("{}.lily", vault_name));

    if !vault_path.exists() {
        // Try legacy extension
        let legacy_path = vaults_dir.join(format!("{}.json", vault_name));
        if legacy_path.exists() {
            lilypad_storage::verify_vault_integrity(&legacy_path)?;
        } else {
            return Err(anyhow!("vault '{}' not found", vault_name));
        }
    } else {
        lilypad_storage::verify_vault_integrity(&vault_path)?;
    }

    println!("Vault '{}' integrity verified successfully.", vault_name);
    Ok(())
}
