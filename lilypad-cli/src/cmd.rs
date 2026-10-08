//! Command implementations - thin wrappers over `lilypad-app`.
//!
//! Every vault decision lives in the service layer; these functions only parse
//! arguments, prompt for secrets, call `lilypad-app`, and print results. In
//! particular `sync pull` now goes through `lilypad_app::sync::net::pull`, which
//! validates the remote and takes a safety backup before touching the local
//! vault - the old write-before-validate path is gone.

use anyhow::{anyhow, Result};
use base64::Engine as _;
use std::path::PathBuf;
use zeroize::Zeroizing;

use crate::io;
use crate::OutputFormat;
use lilypad_app::sync::net::{self, AuthState, PullOutcome};
use lilypad_app::{App, Attachment, EntryMetadata, EntrySecret, EntryType, Session};

/// Unlocks a vault, prompting for the master password if not supplied.
fn unlock(app: &App, vault: &str, provided_pw: Option<&str>) -> Result<Session> {
    let pw = io::resolve_master_password(provided_pw)?;
    Ok(app.unlock(vault, &pw)?)
}

fn parse_entry_type(value: &str) -> Result<EntryType> {
    match value.to_lowercase().replace('_', "-").as_str() {
        "login" => Ok(EntryType::Login),
        "card" => Ok(EntryType::Card),
        "identity" => Ok(EntryType::Identity),
        "secure-note" => Ok(EntryType::SecureNote),
        "software-license" => Ok(EntryType::SoftwareLicense),
        "wifi" => Ok(EntryType::Wifi),
        "server" => Ok(EntryType::Server),
        "custom" => Ok(EntryType::Custom),
        other => Err(anyhow!("unsupported entry type: {other}")),
    }
}

/// Copies to the clipboard and, unlike a detached background thread (which dies
/// with this short-lived process), blocks until the timeout and then actually
/// clears it - so the promised auto-clear really happens.
fn clipboard_copy_and_clear(value: &str, timeout_secs: u64) -> Result<()> {
    lilypad_common::clipboard::copy_to_clipboard(value)?;
    if timeout_secs == 0 {
        println!("Copied to clipboard (no auto-clear requested).");
        return Ok(());
    }
    println!("Copied to clipboard; clearing in {timeout_secs}s (Ctrl-C to keep it).");
    std::thread::sleep(std::time::Duration::from_secs(timeout_secs));
    let _ = lilypad_common::clipboard::copy_to_clipboard("");
    println!("Clipboard cleared.");
    Ok(())
}

// ----------------------------------------------------------------------------
// Vault lifecycle
// ----------------------------------------------------------------------------

pub fn init(app: &App, vault: &str, provided_pw: Option<&str>) -> Result<()> {
    let pw = match provided_pw {
        Some(p) => Zeroizing::new(p.to_string()),
        None => io::prompt_new_master_password()?,
    };
    app.create_vault(vault, &pw)?;
    println!("Created vault '{vault}'.");
    Ok(())
}

pub fn vaults(app: &App, format: OutputFormat) -> Result<()> {
    let names = app.list_vaults()?;
    match format {
        OutputFormat::Json => println!("{}", serde_json::json!({ "vaults": names })),
        OutputFormat::Text => {
            if names.is_empty() {
                println!("No vaults found.");
            } else {
                for name in names {
                    println!("{name}");
                }
            }
        }
    }
    Ok(())
}

pub fn rename_vault(app: &App, from: &str, to: &str) -> Result<()> {
    app.rename_vault(from, to)?;
    println!("Renamed vault '{from}' to '{to}'.");
    Ok(())
}

pub fn delete_vault(app: &App, vault: &str, force: bool) -> Result<()> {
    if !force && !io::confirm(&format!("Delete vault '{vault}'? A safety backup is kept."))? {
        println!("Aborted.");
        return Ok(());
    }
    app.delete_vault(vault)?;
    println!("Deleted vault '{vault}' (a backup was kept).");
    Ok(())
}

// ----------------------------------------------------------------------------
// Entries
// ----------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn add(
    app: &App,
    vault: &str,
    label: &str,
    value: Option<String>,
    username: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    tags: Vec<String>,
    folder: Option<String>,
    entry_type: Option<String>,
    totp_secret: Option<String>,
    attachments: Vec<PathBuf>,
    provided_pw: Option<&str>,
) -> Result<()> {
    let mut session = unlock(app, vault, provided_pw)?;
    let secret_value = io::resolve_secret(value.as_deref(), "Secret value")?;

    let mut secret = EntrySecret::new(secret_value.to_string());
    secret.notes = notes;
    secret.totp_secret = totp_secret;
    for path in &attachments {
        let bytes = std::fs::read(path)
            .map_err(|e| anyhow!("failed to read attachment {}: {e}", path.display()))?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("attachment")
            .to_string();
        secret.attachments.push(Attachment::new(filename, b64));
    }

    let mut metadata = EntryMetadata {
        username,
        url,
        tags,
        folder,
        ..EntryMetadata::default()
    };
    if let Some(t) = entry_type {
        metadata.entry_type = parse_entry_type(&t)?;
    }

    lilypad_app::add_entry(app, &mut session, label, metadata, &secret)?;
    println!("Added entry '{label}' to vault '{vault}'.");
    Ok(())
}

pub fn list(app: &App, vault: &str, format: OutputFormat, provided_pw: Option<&str>) -> Result<()> {
    let session = unlock(app, vault, provided_pw)?;
    let views = lilypad_app::list_entries(&session);
    match format {
        OutputFormat::Json => {
            let items: Vec<_> = views
                .iter()
                .map(|v| {
                    serde_json::json!({
                        "label": v.label,
                        "username": v.username,
                        "url": v.url,
                        "tags": v.tags,
                        "folder": v.folder,
                        "favorite": v.is_favorite,
                    })
                })
                .collect();
            println!("{}", serde_json::json!({ "entries": items }));
        }
        OutputFormat::Text => {
            if views.is_empty() {
                println!("Vault '{vault}' has no entries.");
            } else {
                for v in views {
                    let mut line = v.label.clone();
                    if let Some(u) = &v.username {
                        line.push_str(&format!("  ({u})"));
                    }
                    if let Some(f) = &v.folder {
                        line.push_str(&format!("  [{f}]"));
                    }
                    println!("{line}");
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn get(
    app: &App,
    vault: &str,
    label: &str,
    copy: bool,
    clipboard_timeout: u64,
    show_password: bool,
    provided_pw: Option<&str>,
) -> Result<()> {
    let session = unlock(app, vault, provided_pw)?;
    let secret = lilypad_app::reveal_secret(&session, label)?;

    if let Some(v) = lilypad_app::list_entries(&session)
        .into_iter()
        .find(|v| v.label == label)
    {
        println!("Label:    {}", v.label);
        if let Some(u) = &v.username {
            println!("Username: {u}");
        }
        if let Some(u) = &v.url {
            println!("URL:      {u}");
        }
        if !v.tags.is_empty() {
            println!("Tags:     {}", v.tags.join(", "));
        }
        if let Some(f) = &v.folder {
            println!("Folder:   {f}");
        }
    }
    if show_password {
        println!("Password: {}", secret.password);
    } else {
        println!("Password: (hidden - pass --show-password to reveal)");
    }
    if let Some(n) = &secret.notes {
        println!("Notes:    {n}");
    }
    if secret.totp_secret.is_some() {
        println!("TOTP:     configured (use the `totp` command for a code)");
    }
    if !secret.attachments.is_empty() {
        println!("Attachments: {}", secret.attachments.len());
    }

    if copy {
        clipboard_copy_and_clear(&secret.password, clipboard_timeout)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    app: &App,
    vault: &str,
    label: &str,
    value: Option<String>,
    username: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    tags: Vec<String>,
    remove_tags: Vec<String>,
    clear_tags: bool,
    folder: Option<String>,
    entry_type: Option<String>,
    totp_secret: Option<String>,
    provided_pw: Option<&str>,
) -> Result<()> {
    let mut session = unlock(app, vault, provided_pw)?;

    // Merge the passed fields over the current values, then apply through
    // edit_entry: one locked read-modify-write that preserves every field not
    // in the edit (attachments, backup codes, email, phone, custom fields) and
    // records password history when the password actually changed.
    let current = lilypad_app::list_entries(&session)
        .into_iter()
        .find(|v| v.label == label)
        .ok_or_else(|| {
            // Distinguish "in Trash" from "does not exist" so a soft-deleted
            // entry cannot be silently mutated (or mistaken for missing).
            if lilypad_app::list_trash(&session).iter().any(|v| v.label == label) {
                anyhow!("entry '{label}' is in Trash; restore it first (lilypad restore {vault} {label})")
            } else {
                anyhow!("entry '{label}' not found")
            }
        })?;
    let (cur_password, cur_notes, cur_totp) = {
        let revealed = lilypad_app::reveal_secret(&session, label)?;
        (
            revealed.password.clone(),
            revealed.notes.clone(),
            revealed.totp_secret.clone(),
        )
    };
    let entry_type = match entry_type {
        Some(t) => parse_entry_type(&t)?,
        None => current.entry_type.clone(),
    };
    lilypad_app::edit_entry(
        app,
        &mut session,
        label,
        lilypad_app::EntryEdit {
            new_label: label.to_string(),
            username: username.or_else(|| current.username.clone()),
            url: url.or_else(|| current.url.clone()),
            entry_type,
            password: value.unwrap_or(cur_password),
            notes: notes.or(cur_notes),
            totp_secret: totp_secret.or(cur_totp),
            tags: None,
            folder: None,
        },
    )?;

    // Folder and tag changes go through their dedicated locked ops.
    if folder.is_some() {
        lilypad_app::set_folder(app, &mut session, label, folder)?;
    }
    if clear_tags {
        for t in current.tags.clone() {
            let _ = lilypad_app::remove_tag(app, &mut session, label, &t);
        }
    }
    for t in tags {
        lilypad_app::add_tag(app, &mut session, label, &t)?;
    }
    for t in remove_tags {
        // Removing an absent tag is a no-op, matching the old retain semantics.
        let _ = lilypad_app::remove_tag(app, &mut session, label, &t);
    }
    println!("Updated entry '{label}'.");
    Ok(())
}

pub fn history(
    app: &App,
    vault: &str,
    label: &str,
    format: OutputFormat,
    provided_pw: Option<&str>,
) -> Result<()> {
    let session = unlock(app, vault, provided_pw)?;
    let events = lilypad_app::entry_history(&session, label)?;
    match format {
        OutputFormat::Json => {
            let items: Vec<_> = events
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "timestamp": e.timestamp,
                        "change": e.change,
                        "stores_previous_password": e.stores_previous_password,
                    })
                })
                .collect();
            println!("{}", serde_json::json!({ "history": items }));
        }
        OutputFormat::Text => {
            if events.is_empty() {
                println!("No history recorded for '{label}'.");
            } else {
                for e in &events {
                    let when = lilypad_common::format_timestamp_relative(e.timestamp);
                    let extra = if e.stores_previous_password {
                        "  (previous password kept, encrypted)"
                    } else {
                        ""
                    };
                    println!("{when}  {}{extra}", e.change);
                }
            }
        }
    }
    Ok(())
}

pub fn remove(
    app: &App,
    vault: &str,
    label: &str,
    purge: bool,
    provided_pw: Option<&str>,
) -> Result<()> {
    let mut session = unlock(app, vault, provided_pw)?;
    if purge {
        // Permanent erase (not recoverable).
        lilypad_app::delete_entry(app, &mut session, label)?;
        println!("Permanently removed entry '{label}' from vault '{vault}'.");
    } else {
        // Recoverable: move to Trash. Restore with `lilypad restore`.
        lilypad_app::soft_delete(app, &mut session, label)?;
        println!(
            "Moved entry '{label}' to Trash (restore with `lilypad restore {vault} {label}`)."
        );
    }
    Ok(())
}

pub fn trash(
    app: &App,
    vault: &str,
    format: OutputFormat,
    provided_pw: Option<&str>,
) -> Result<()> {
    let session = unlock(app, vault, provided_pw)?;
    let views = lilypad_app::list_trash(&session);
    match format {
        OutputFormat::Json => {
            let labels: Vec<_> = views.iter().map(|v| v.label.clone()).collect();
            println!("{}", serde_json::json!({ "trash": labels }));
        }
        OutputFormat::Text => {
            if views.is_empty() {
                println!("Trash is empty for vault '{vault}'.");
            } else {
                for v in views {
                    println!("{}", v.label);
                }
            }
        }
    }
    Ok(())
}

pub fn restore(app: &App, vault: &str, label: &str, provided_pw: Option<&str>) -> Result<()> {
    let mut session = unlock(app, vault, provided_pw)?;
    lilypad_app::restore(app, &mut session, label)?;
    println!("Restored entry '{label}' in vault '{vault}'.");
    Ok(())
}

pub fn audit(
    app: &App,
    vault: &str,
    format: OutputFormat,
    provided_pw: Option<&str>,
) -> Result<()> {
    let session = unlock(app, vault, provided_pw)?;
    let report = lilypad_app::vault_health(&session);
    let grade = health_grade_letter(report.score.grade);
    match format {
        OutputFormat::Json => {
            let issues: Vec<_> = report
                .issues
                .iter()
                .map(|i| {
                    serde_json::json!({
                        "severity": format!("{:?}", i.severity),
                        "title": i.title,
                        "description": i.description,
                        "recommendation": i.recommendation,
                        "affected": i.affected_entries,
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({
                    "score": report.score.score,
                    "grade": grade,
                    "issues": issues,
                })
            );
        }
        OutputFormat::Text => {
            println!("Vault health: {grade} ({}/100)", report.score.score);
            if report.issues.is_empty() {
                println!("No issues found. Nice.");
            } else {
                println!("{} issue(s):", report.issues.len());
                for i in &report.issues {
                    let sev = match i.severity {
                        lilypad_app::IssueSeverity::Critical => "CRIT",
                        lilypad_app::IssueSeverity::Warning => "WARN",
                        lilypad_app::IssueSeverity::Info => "INFO",
                    };
                    println!(
                        "  [{sev}] {} ({} affected)",
                        i.title,
                        i.affected_entries.len()
                    );
                    if !i.recommendation.is_empty() {
                        println!("         {}", i.recommendation);
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn breach_check(
    app: &App,
    vault: &str,
    format: OutputFormat,
    provided_pw: Option<&str>,
) -> Result<()> {
    let session = unlock(app, vault, provided_pw)?;
    println!(
        "Checking against Have-I-Been-Pwned (k-anonymity; passwords never leave this machine)..."
    );
    let report = lilypad_app::breach_check(&session)?;
    match format {
        OutputFormat::Json => {
            let breached: Vec<_> = report
                .breached
                .iter()
                .map(|b| serde_json::json!({ "label": b.label, "count": b.count }))
                .collect();
            println!(
                "{}",
                serde_json::json!({
                    "unique_checked": report.unique_checked,
                    "requests": report.requests,
                    "breached": breached,
                })
            );
        }
        OutputFormat::Text => {
            println!(
                "Checked {} unique password(s) in {} request(s).",
                report.unique_checked, report.requests
            );
            if report.breached.is_empty() {
                println!("No password appears in known breaches. Nice.");
            } else {
                println!(
                    "{} entr{} with breached passwords - change them:",
                    report.breached.len(),
                    if report.breached.len() == 1 {
                        "y"
                    } else {
                        "ies"
                    }
                );
                for b in &report.breached {
                    println!("  {}  (seen {} times in breaches)", b.label, b.count);
                }
            }
        }
    }
    Ok(())
}

pub fn audit_log(
    app: &App,
    vault: &str,
    format: OutputFormat,
    provided_pw: Option<&str>,
) -> Result<()> {
    let session = unlock(app, vault, provided_pw)?;
    let events = lilypad_app::audit_log(&session);
    match format {
        OutputFormat::Json => {
            let items: Vec<_> = events
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "timestamp": e.timestamp,
                        "action": e.action,
                        "entry": e.entry_label,
                    })
                })
                .collect();
            println!("{}", serde_json::json!({ "audit_log": items }));
        }
        OutputFormat::Text => {
            if events.is_empty() {
                println!("No audit events recorded for vault '{vault}'.");
            } else {
                for e in events.iter().rev() {
                    let when = lilypad_common::format_timestamp_relative(e.timestamp);
                    match &e.entry_label {
                        Some(label) => println!("{when}  {}  ({label})", e.action),
                        None => println!("{when}  {}", e.action),
                    }
                }
            }
        }
    }
    Ok(())
}

fn health_grade_letter(g: lilypad_app::HealthGrade) -> &'static str {
    use lilypad_app::HealthGrade as G;
    match g {
        G::A => "A",
        G::B => "B",
        G::C => "C",
        G::D => "D",
        G::F => "F",
    }
}

pub fn rename_entry(
    app: &App,
    vault: &str,
    label: &str,
    new_label: &str,
    provided_pw: Option<&str>,
) -> Result<()> {
    let mut session = unlock(app, vault, provided_pw)?;
    lilypad_app::rename_entry(app, &mut session, label, new_label)?;
    println!("Renamed entry '{label}' to '{new_label}'.");
    Ok(())
}

pub fn search(
    app: &App,
    vault: &str,
    query: &str,
    format: OutputFormat,
    provided_pw: Option<&str>,
) -> Result<()> {
    let session = unlock(app, vault, provided_pw)?;
    let hits = lilypad_app::search_entries(&session, query);
    match format {
        OutputFormat::Json => {
            let labels: Vec<_> = hits.iter().map(|v| v.label.clone()).collect();
            println!("{}", serde_json::json!({ "matches": labels }));
        }
        OutputFormat::Text => {
            if hits.is_empty() {
                println!("No matches for '{query}' in vault '{vault}'.");
            } else {
                for v in hits {
                    println!("{}", v.label);
                }
            }
        }
    }
    Ok(())
}

pub fn generate(
    length: usize,
    uppercase: bool,
    lowercase: bool,
    digits: bool,
    symbols: bool,
    copy: bool,
    clipboard_timeout: u64,
) -> Result<()> {
    let password = lilypad_app::generate_password(&lilypad_app::PasswordOptions {
        length,
        uppercase,
        lowercase,
        digits,
        symbols,
    })?;
    println!("{}", *password);
    if copy {
        clipboard_copy_and_clear(&password, clipboard_timeout)?;
    }
    Ok(())
}

pub fn totp(app: &App, vault: &str, label: &str, provided_pw: Option<&str>) -> Result<()> {
    let session = unlock(app, vault, provided_pw)?;
    let secret = lilypad_app::reveal_secret(&session, label)?;
    let totp_secret = secret
        .totp_secret
        .as_deref()
        .ok_or_else(|| anyhow!("entry '{label}' has no TOTP secret"))?;
    let code = lilypad_app::totp::code_for_secret(totp_secret)?;
    println!("{}", *code);
    Ok(())
}

pub fn change_master_password(app: &App, vault: &str, provided_pw: Option<&str>) -> Result<()> {
    let mut session = unlock(app, vault, provided_pw)?;
    let new_pw = io::prompt_new_master_password()?;
    lilypad_app::change_master_password(app, &mut session, &new_pw)?;
    println!("Master password changed for vault '{vault}'.");
    Ok(())
}

// ----------------------------------------------------------------------------
// Import / export
// ----------------------------------------------------------------------------

pub fn import(
    app: &App,
    vault: &str,
    file: &std::path::Path,
    dry_run: bool,
    provided_pw: Option<&str>,
) -> Result<()> {
    let bytes =
        std::fs::read(file).map_err(|e| anyhow!("failed to read {}: {e}", file.display()))?;
    let format = lilypad_app::detect_format(&bytes)?;
    println!("Detected format: {}", format.name());

    let parsed = lilypad_app::parse_import(&bytes, format)?;
    for w in &parsed.warnings {
        eprintln!("warning: {w}");
    }
    if dry_run {
        println!(
            "Dry run: {} entr{} would be imported into vault '{vault}'.",
            parsed.entries.len(),
            if parsed.entries.len() == 1 {
                "y"
            } else {
                "ies"
            }
        );
        return Ok(());
    }

    let mut session = unlock(app, vault, provided_pw)?;
    let report = lilypad_app::import_entries(app, &mut session, parsed)?;
    for (from, to) in &report.renamed {
        println!("renamed: '{from}' already existed - imported as '{to}'");
    }
    if report.skipped > 0 {
        println!(
            "skipped: {} entr{} failed validation",
            report.skipped,
            if report.skipped == 1 { "y" } else { "ies" }
        );
    }
    println!(
        "Imported {} entr{} into vault '{vault}'.",
        report.added,
        if report.added == 1 { "y" } else { "ies" }
    );
    println!(
        "Tip: the source file contains your passwords in cleartext - delete it once verified."
    );
    Ok(())
}

pub fn export(
    app: &App,
    vault: &str,
    file: &std::path::Path,
    force: bool,
    provided_pw: Option<&str>,
) -> Result<()> {
    if !force
        && !io::confirm(&format!(
            "Export vault '{vault}' to {} as PLAINTEXT CSV (every secret revealed)?",
            file.display()
        ))?
    {
        println!("Aborted.");
        return Ok(());
    }
    let session = unlock(app, vault, provided_pw)?;
    let csv = lilypad_app::export_csv(&session)?;
    std::fs::write(file, csv.as_bytes())
        .map_err(|e| anyhow!("failed to write {}: {e}", file.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600));
    }
    println!(
        "Exported vault '{vault}' to {} (PLAINTEXT - store it safely, delete it when done).",
        file.display()
    );
    Ok(())
}

// ----------------------------------------------------------------------------
// Backups
// ----------------------------------------------------------------------------

pub fn backup(app: &App, vault: &str) -> Result<()> {
    let filename = app.create_backup(vault)?;
    println!("Created backup: {filename}");
    Ok(())
}

pub fn list_backups(app: &App, vault: &str) -> Result<()> {
    let backups = app.list_backups(vault)?;
    if backups.is_empty() {
        println!("No backups for vault '{vault}'.");
    } else {
        for b in backups {
            println!("{}  ({} bytes)", b.filename, b.size_bytes);
        }
    }
    Ok(())
}

pub fn restore_backup(app: &App, backup_filename: &str, force: bool) -> Result<()> {
    if !force
        && !io::confirm(&format!(
            "Restore from '{backup_filename}'? The current vault is backed up first."
        ))?
    {
        println!("Aborted.");
        return Ok(());
    }
    let previous = app.restore_backup(backup_filename)?;
    match previous {
        Some(b) => println!("Restored. Previous vault saved as backup: {b}"),
        None => println!("Restored from '{backup_filename}'."),
    }
    Ok(())
}

pub fn prune_backups(app: &App, vault: &str, keep: usize) -> Result<()> {
    let removed = app.prune_backups(vault, keep)?;
    println!("Removed {removed} old backup(s), kept {keep} most recent.");
    Ok(())
}

// ----------------------------------------------------------------------------
// Sync / OAuth
// ----------------------------------------------------------------------------

pub fn login() -> Result<()> {
    let login = net::begin_login()?;
    println!(
        "To authorize Lilypad, open:\n  {}\nand enter the code: {}",
        login.verification_uri, login.user_code
    );
    println!("Waiting for authorization...");
    let username = net::complete_login(&login)?;
    println!("Logged in as {username}.");
    Ok(())
}

pub fn logout() -> Result<()> {
    net::logout()?;
    println!("Logged out of GitHub.");
    Ok(())
}

pub fn auth_status() -> Result<()> {
    match net::auth_status()? {
        AuthState::Authenticated { username } => {
            println!("Authenticated with GitHub as {username}.")
        }
        AuthState::NotAuthenticated => println!("Not authenticated. Run `lilypad login`."),
    }
    Ok(())
}

pub fn sync_push(app: &App, vault: &str, force: bool) -> Result<()> {
    net::push(app, vault, force)?;
    println!("Pushed vault '{vault}' to GitHub.");
    Ok(())
}

pub fn sync_pull(app: &App, vault: &str, provided_pw: Option<&str>) -> Result<()> {
    // Pull is always a validated replace (validate + safety backup happen in the
    // service layer), so no force flag is needed to make it safe.
    let pw = io::resolve_master_password(provided_pw)?;
    match net::pull(app, vault, &pw)? {
        PullOutcome::Applied => println!("Pulled and applied remote vault '{vault}'."),
        PullOutcome::NoRemote => println!("No remote vault named '{vault}' was found."),
    }
    Ok(())
}

pub fn sync_merge(app: &App, vault: &str, provided_pw: Option<&str>) -> Result<()> {
    use lilypad_app::sync::net::MergeOutcome;
    let pw = io::resolve_master_password(provided_pw)?;
    match net::sync_merge(app, vault, &pw)? {
        MergeOutcome::NoRemote => {
            println!(
                "No remote vault named '{vault}' was found; run `lilypad sync push` to create it."
            )
        }
        MergeOutcome::Merged { report, pushed } => {
            println!("Merged remote vault '{vault}': {}.", report.summary());
            if pushed {
                println!("Pushed the merged vault back (the remote was missing local data).");
            }
            println!("Sync status: in sync.");
        }
    }
    Ok(())
}

pub fn sync_status(app: &App, vault: &str) -> Result<()> {
    use lilypad_app::SyncStatusView as S;
    let status = net::remote_status(app, vault)?;
    let (label, hint) = match status {
        S::InSync => ("in sync", "nothing to do."),
        S::LocalAhead => ("local ahead", "run `lilypad sync push` to publish your changes."),
        S::RemoteAhead => ("remote ahead", "run `lilypad sync pull` to get the latest."),
        S::Conflict => (
            "conflict",
            "both sides changed; run `lilypad sync merge` to combine them entry-by-entry (a safety backup is kept).",
        ),
        S::NoRemote => ("no remote", "run `lilypad sync push` to create it."),
        S::NoBaseline => ("no baseline", "run `lilypad sync push` or `pull` to establish one."),
    };
    println!("Sync status for '{vault}': {label} - {hint}");
    Ok(())
}
