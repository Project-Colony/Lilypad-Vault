//! Import commands with support for multiple password manager formats.
//!
//! This module uses a generic import strategy to eliminate code duplication
//! across different password manager import functions.

use anyhow::{anyhow, Result};
use csv::ReaderBuilder;
use lilypad_common::keyfile::load_key;
use lilypad_core::{CryptoAlgorithm, EntrySecret, KeyMaterial, KeyMetadata, Vault};
use lilypad_storage::LocalStore;
use serde::Deserialize;
use std::fs;

use super::utils::{
    build_metadata, key_path, parse_tags, upsert_entry, validate_import_entry,
};

// ============== CSV Record Definitions ==============

/// Lilypad native CSV format
#[derive(Debug, Deserialize, serde::Serialize)]
pub struct CsvEntry {
    #[serde(rename = "Label", default)]
    pub label: String,
    #[serde(rename = "Type", default)]
    pub entry_type: String,
    #[serde(rename = "Username", default)]
    pub username: String,
    #[serde(rename = "URL", default)]
    pub url: String,
    #[serde(rename = "Folder", default)]
    pub folder: String,
    #[serde(rename = "Tags", default)]
    pub tags: String,
    #[serde(rename = "Password", default)]
    pub password: String,
    #[serde(rename = "Notes", default)]
    pub notes: String,
    #[serde(rename = "TOTP", default)]
    pub totp_secret: String,
}

/// LastPass CSV format
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LastPassEntry {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub totp: String,
    #[serde(default)]
    pub extra: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub grouping: String,
    #[serde(default)]
    pub fav: String,
}

/// Bitwarden CSV format
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BitwardenEntry {
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub favorite: String,
    #[serde(rename = "type", default)]
    pub entry_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub fields: String,
    #[serde(default)]
    pub reprompt: String,
    #[serde(default)]
    pub login_uri: String,
    #[serde(default)]
    pub login_username: String,
    #[serde(default)]
    pub login_password: String,
    #[serde(default)]
    pub login_totp: String,
}

/// 1Password CSV format
#[derive(Debug, Deserialize)]
pub struct OnePasswordEntry {
    #[serde(rename = "Title", default)]
    pub title: String,
    #[serde(rename = "Url", alias = "URL", default)]
    pub url: String,
    #[serde(rename = "Username", default)]
    pub username: String,
    #[serde(rename = "Password", default)]
    pub password: String,
    #[serde(rename = "Notes", default)]
    pub notes: String,
    #[serde(rename = "OTPAuth", alias = "OTPauth", default)]
    pub otp_auth: String,
}

/// Chrome CSV format
#[derive(Debug, Deserialize)]
pub struct ChromeEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub note: String,
}

/// Firefox CSV format
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct FirefoxEntry {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(rename = "httpRealm", default)]
    pub http_realm: String,
    #[serde(rename = "formActionOrigin", default)]
    pub form_action_origin: String,
    #[serde(default)]
    pub guid: String,
    #[serde(rename = "timeCreated", default)]
    pub time_created: String,
    #[serde(rename = "timeLastUsed", default)]
    pub time_last_used: String,
    #[serde(rename = "timePasswordChanged", default)]
    pub time_password_changed: String,
}

/// Dashlane CSV format
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DashlaneEntry {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub username2: String,
    #[serde(default)]
    pub username3: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub category: String,
    #[serde(rename = "otpSecret", default)]
    pub otp_secret: String,
}

/// KeePass CSV format
#[derive(Debug, Deserialize)]
pub struct KeePassEntry {
    #[serde(rename = "Group", default)]
    pub group: String,
    #[serde(rename = "Title", default)]
    pub title: String,
    #[serde(rename = "Username", default)]
    pub username: String,
    #[serde(rename = "Password", default)]
    pub password: String,
    #[serde(rename = "URL", default)]
    pub url: String,
    #[serde(rename = "Notes", default)]
    pub notes: String,
    #[serde(rename = "TOTP", default)]
    pub totp: String,
}

// ============== Import Trait ==============

/// Trait for converting CSV records into Lilypad entries.
pub trait ImportRecord {
    /// Returns the label for the entry.
    fn label(&self) -> String;
    /// Returns the username, if any.
    fn username(&self) -> Option<String>;
    /// Returns the URL, if any.
    fn url(&self) -> Option<String>;
    /// Returns the folder, if any.
    fn folder(&self) -> Option<String>;
    /// Returns tags as a vector.
    fn tags(&self) -> Vec<String>;
    /// Returns the entry type, if any.
    fn entry_type(&self) -> Option<String>;
    /// Returns the password.
    fn password(&self) -> String;
    /// Returns notes, if any.
    fn notes(&self) -> Option<String>;
    /// Returns the TOTP secret, if any.
    fn totp_secret(&self) -> Option<String>;
}

// ============== ImportRecord Implementations ==============

impl ImportRecord for CsvEntry {
    fn label(&self) -> String {
        self.label.clone()
    }
    fn username(&self) -> Option<String> {
        if self.username.is_empty() { None } else { Some(self.username.clone()) }
    }
    fn url(&self) -> Option<String> {
        if self.url.is_empty() { None } else { Some(self.url.clone()) }
    }
    fn folder(&self) -> Option<String> {
        if self.folder.is_empty() { None } else { Some(self.folder.clone()) }
    }
    fn tags(&self) -> Vec<String> {
        parse_tags(&self.tags)
    }
    fn entry_type(&self) -> Option<String> {
        if self.entry_type.is_empty() { None } else { Some(self.entry_type.clone()) }
    }
    fn password(&self) -> String {
        self.password.clone()
    }
    fn notes(&self) -> Option<String> {
        if self.notes.is_empty() { None } else { Some(self.notes.clone()) }
    }
    fn totp_secret(&self) -> Option<String> {
        if self.totp_secret.is_empty() { None } else { Some(self.totp_secret.clone()) }
    }
}

impl ImportRecord for LastPassEntry {
    fn label(&self) -> String {
        if self.name.is_empty() {
            extract_domain_from_url(&self.url).unwrap_or_else(|| "Unnamed Entry".to_string())
        } else {
            self.name.clone()
        }
    }
    fn username(&self) -> Option<String> {
        if self.username.is_empty() { None } else { Some(self.username.clone()) }
    }
    fn url(&self) -> Option<String> {
        if self.url.is_empty() { None } else { Some(self.url.clone()) }
    }
    fn folder(&self) -> Option<String> {
        if self.grouping.is_empty() { None } else { Some(self.grouping.clone()) }
    }
    fn tags(&self) -> Vec<String> {
        Vec::new() // LastPass uses grouping, not tags
    }
    fn entry_type(&self) -> Option<String> {
        None
    }
    fn password(&self) -> String {
        self.password.clone()
    }
    fn notes(&self) -> Option<String> {
        if self.extra.is_empty() { None } else { Some(self.extra.clone()) }
    }
    fn totp_secret(&self) -> Option<String> {
        if self.totp.is_empty() { None } else { Some(extract_totp_secret(&self.totp)) }
    }
}

impl ImportRecord for BitwardenEntry {
    fn label(&self) -> String {
        self.name.clone()
    }
    fn username(&self) -> Option<String> {
        if self.login_username.is_empty() { None } else { Some(self.login_username.clone()) }
    }
    fn url(&self) -> Option<String> {
        if self.login_uri.is_empty() { None } else { Some(self.login_uri.clone()) }
    }
    fn folder(&self) -> Option<String> {
        if self.folder.is_empty() { None } else { Some(self.folder.clone()) }
    }
    fn tags(&self) -> Vec<String> {
        Vec::new()
    }
    fn entry_type(&self) -> Option<String> {
        Some(self.entry_type.clone())
    }
    fn password(&self) -> String {
        self.login_password.clone()
    }
    fn notes(&self) -> Option<String> {
        if self.notes.is_empty() { None } else { Some(self.notes.clone()) }
    }
    fn totp_secret(&self) -> Option<String> {
        if self.login_totp.is_empty() { None } else { Some(extract_totp_secret(&self.login_totp)) }
    }
}

impl ImportRecord for OnePasswordEntry {
    fn label(&self) -> String {
        self.title.clone()
    }
    fn username(&self) -> Option<String> {
        if self.username.is_empty() { None } else { Some(self.username.clone()) }
    }
    fn url(&self) -> Option<String> {
        if self.url.is_empty() { None } else { Some(self.url.clone()) }
    }
    fn folder(&self) -> Option<String> {
        None
    }
    fn tags(&self) -> Vec<String> {
        Vec::new()
    }
    fn entry_type(&self) -> Option<String> {
        None
    }
    fn password(&self) -> String {
        self.password.clone()
    }
    fn notes(&self) -> Option<String> {
        if self.notes.is_empty() { None } else { Some(self.notes.clone()) }
    }
    fn totp_secret(&self) -> Option<String> {
        if self.otp_auth.is_empty() { None } else { Some(extract_totp_secret(&self.otp_auth)) }
    }
}

impl ImportRecord for ChromeEntry {
    fn label(&self) -> String {
        if self.name.is_empty() {
            extract_domain_from_url(&self.url).unwrap_or_else(|| "Unnamed Entry".to_string())
        } else {
            self.name.clone()
        }
    }
    fn username(&self) -> Option<String> {
        if self.username.is_empty() { None } else { Some(self.username.clone()) }
    }
    fn url(&self) -> Option<String> {
        if self.url.is_empty() { None } else { Some(self.url.clone()) }
    }
    fn folder(&self) -> Option<String> {
        None
    }
    fn tags(&self) -> Vec<String> {
        Vec::new()
    }
    fn entry_type(&self) -> Option<String> {
        None
    }
    fn password(&self) -> String {
        self.password.clone()
    }
    fn notes(&self) -> Option<String> {
        if self.note.is_empty() { None } else { Some(self.note.clone()) }
    }
    fn totp_secret(&self) -> Option<String> {
        None
    }
}

impl ImportRecord for FirefoxEntry {
    fn label(&self) -> String {
        extract_domain_from_url(&self.url).unwrap_or_else(|| "Unnamed Entry".to_string())
    }
    fn username(&self) -> Option<String> {
        if self.username.is_empty() { None } else { Some(self.username.clone()) }
    }
    fn url(&self) -> Option<String> {
        if self.url.is_empty() { None } else { Some(self.url.clone()) }
    }
    fn folder(&self) -> Option<String> {
        None
    }
    fn tags(&self) -> Vec<String> {
        Vec::new()
    }
    fn entry_type(&self) -> Option<String> {
        None
    }
    fn password(&self) -> String {
        self.password.clone()
    }
    fn notes(&self) -> Option<String> {
        None
    }
    fn totp_secret(&self) -> Option<String> {
        None
    }
}

impl ImportRecord for DashlaneEntry {
    fn label(&self) -> String {
        self.title.clone()
    }
    fn username(&self) -> Option<String> {
        if self.username.is_empty() { None } else { Some(self.username.clone()) }
    }
    fn url(&self) -> Option<String> {
        if self.url.is_empty() { None } else { Some(self.url.clone()) }
    }
    fn folder(&self) -> Option<String> {
        if self.category.is_empty() { None } else { Some(self.category.clone()) }
    }
    fn tags(&self) -> Vec<String> {
        Vec::new()
    }
    fn entry_type(&self) -> Option<String> {
        None
    }
    fn password(&self) -> String {
        self.password.clone()
    }
    fn notes(&self) -> Option<String> {
        if self.note.is_empty() { None } else { Some(self.note.clone()) }
    }
    fn totp_secret(&self) -> Option<String> {
        if self.otp_secret.is_empty() { None } else { Some(self.otp_secret.clone()) }
    }
}

impl ImportRecord for KeePassEntry {
    fn label(&self) -> String {
        self.title.clone()
    }
    fn username(&self) -> Option<String> {
        if self.username.is_empty() { None } else { Some(self.username.clone()) }
    }
    fn url(&self) -> Option<String> {
        if self.url.is_empty() { None } else { Some(self.url.clone()) }
    }
    fn folder(&self) -> Option<String> {
        if self.group.is_empty() { None } else { Some(self.group.clone()) }
    }
    fn tags(&self) -> Vec<String> {
        Vec::new()
    }
    fn entry_type(&self) -> Option<String> {
        None
    }
    fn password(&self) -> String {
        self.password.clone()
    }
    fn notes(&self) -> Option<String> {
        if self.notes.is_empty() { None } else { Some(self.notes.clone()) }
    }
    fn totp_secret(&self) -> Option<String> {
        if self.totp.is_empty() { None } else { Some(extract_totp_secret(&self.totp)) }
    }
}

// ============== Generic Import Function ==============

/// Generic CSV import function that works with any record type implementing ImportRecord.
pub fn import_csv<T>(vault: &mut Vault, key: &KeyMaterial, input: &str) -> Result<usize>
where
    T: for<'de> Deserialize<'de> + ImportRecord,
{
    let mut reader = ReaderBuilder::new().from_path(input)?;
    let mut count = 0;

    for result in reader.deserialize() {
        let record: T = result?;
        let label = record.label();

        // Skip entries without passwords (for Bitwarden non-login entries)
        if record.password().is_empty() {
            continue;
        }

        let metadata = build_metadata(
            record.username(),
            record.url(),
            record.tags(),
            record.folder(),
            record.entry_type(),
        )?;

        let mut secret = EntrySecret::new(record.password());
        secret.notes = record.notes();
        secret.totp_secret = record.totp_secret();

        validate_import_entry(&label, &secret, &metadata)?;
        upsert_entry(vault, key, &label, metadata, secret)?;
        count += 1;
    }

    Ok(count)
}

// ============== Helper Functions ==============

/// Extract domain from URL for use as label.
pub fn extract_domain_from_url(url: &str) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    let url = url.trim();
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);

    let domain = without_scheme.split('/').next()?;
    let domain = domain.split(':').next()?; // Remove port if present

    if domain.is_empty() {
        None
    } else {
        Some(domain.to_string())
    }
}

/// Extract TOTP secret from various formats (otpauth:// URL or raw secret).
pub fn extract_totp_secret(totp_value: &str) -> String {
    let totp_value = totp_value.trim();

    // If it's an otpauth:// URL, extract the secret parameter
    if totp_value.starts_with("otpauth://") {
        if let Some(secret_start) = totp_value.find("secret=") {
            let secret_part = &totp_value[secret_start + 7..];
            let secret_end = secret_part.find('&').unwrap_or(secret_part.len());
            return secret_part[..secret_end].to_string();
        }
    }

    // Otherwise, return as-is (it's already a raw secret)
    totp_value.to_string()
}

// ============== Main Import Command ==============

/// Import entries into a vault from various formats.
pub fn import_vault(
    store: &LocalStore,
    config: &lilypad_core::AppConfig,
    vault_name: &str,
    input: &str,
    format: &str,
    source: &str,
    master_password: Option<&str>,
) -> Result<()> {
    let format = format.to_lowercase();
    let source = source.to_lowercase();

    if format == "lily" {
        let payload = fs::read(input)?;
        store.apply_sync_payload(vault_name, &payload)?;
        println!("Encrypted vault imported from {input}.");
        return Ok(());
    }

    let (key, _) = load_key(&key_path(config), master_password)?;
    let mut vault = match store.load_vault(vault_name, &key) {
        Ok(vault) => vault,
        Err(_) => {
            let metadata = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
            Vault::new(vault_name, metadata)
        }
    };

    let imported_count = match format.as_str() {
        "json" => {
            let payload = fs::read(input)?;
            let import: VaultExport = serde_json::from_slice(&payload)?;
            let mut count = 0;
            for entry in import.entries {
                validate_import_entry(&entry.label, &entry.secret, &entry.metadata)?;
                upsert_entry(&mut vault, &key, &entry.label, entry.metadata, entry.secret)?;
                count += 1;
            }
            count
        }
        "csv" => match source.as_str() {
            "lilypad" => import_csv::<CsvEntry>(&mut vault, &key, input)?,
            "lastpass" => import_csv::<LastPassEntry>(&mut vault, &key, input)?,
            "bitwarden" => import_csv::<BitwardenEntry>(&mut vault, &key, input)?,
            "1password" | "onepassword" => import_csv::<OnePasswordEntry>(&mut vault, &key, input)?,
            "chrome" | "google" => import_csv::<ChromeEntry>(&mut vault, &key, input)?,
            "firefox" => import_csv::<FirefoxEntry>(&mut vault, &key, input)?,
            "dashlane" => import_csv::<DashlaneEntry>(&mut vault, &key, input)?,
            "keepass" => import_csv::<KeePassEntry>(&mut vault, &key, input)?,
            _ => {
                return Err(anyhow!(
                    "Unsupported source: {source}. Supported sources: lilypad, lastpass, bitwarden, 1password, chrome, firefox, dashlane, keepass"
                ));
            }
        },
        _ => {
            return Err(anyhow!("unsupported import format: {format}"));
        }
    };

    store.save_vault(&vault, &key)?;
    println!("Imported {imported_count} entries from {source} into vault '{vault_name}'.");
    Ok(())
}

// ============== Export Types (for JSON import) ==============

use serde::Serialize;
use lilypad_core::EntryMetadata;

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultExport {
    pub name: String,
    pub entries: Vec<EntryExport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryExport {
    pub label: String,
    pub metadata: EntryMetadata,
    pub secret: EntrySecret,
    pub created_at: u64,
    pub updated_at: u64,
}
