//! Vault import from other password managers, and the native CSV export.
//!
//! Formats are auto-detected from file CONTENT (never the extension): binary
//! magics first, then a JSON probe, then an ordered list of CSV header rules -
//! exact-header rules before subset rules, so a generic header never shadows a
//! specific one. The rules and quirks here were verified against the official
//! docs and the open-source importers of Bitwarden and Proton Pass (which parse
//! competitors' real exports, fixtures included).
//!
//! Everything parses into [`ImportedEntry`] (a zeroize-on-drop superset), and
//! [`import_entries`] commits the batch in ONE locked read-modify-write with a
//! non-destructive conflict policy: an existing label is never overwritten, the
//! incoming entry is renamed with an `(imported)` suffix instead.

use crate::error::{AppError, Result};
use crate::locking::with_vault_locked;
use crate::secret::seal_secret;
use crate::session::Session;
use crate::vault::App;
use lilypad_core::{CustomField, Entry, EntryMetadata, EntrySecret, EntryType};
use zeroize::{Zeroize, Zeroizing};

/// A supported import source, as detected from the file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    LilypadCsv,
    LastPassCsv,
    BitwardenCsv,
    BitwardenJson,
    KeePassXcCsv,
    OnePasswordCsv,
    SafariCsv,
    ChromeCsv,
    FirefoxCsv,
    ProtonPassCsv,
    DashlaneCsv,
    DashlaneNotesCsv,
}

impl ImportFormat {
    pub fn name(&self) -> &'static str {
        match self {
            ImportFormat::LilypadCsv => "Lilypad CSV",
            ImportFormat::LastPassCsv => "LastPass CSV",
            ImportFormat::BitwardenCsv => "Bitwarden CSV",
            ImportFormat::BitwardenJson => "Bitwarden JSON",
            ImportFormat::KeePassXcCsv => "KeePassXC CSV",
            ImportFormat::OnePasswordCsv => "1Password CSV",
            ImportFormat::SafariCsv => "Safari/Apple Passwords CSV",
            ImportFormat::ChromeCsv => "Chrome/Edge CSV",
            ImportFormat::FirefoxCsv => "Firefox CSV",
            ImportFormat::ProtonPassCsv => "Proton Pass CSV",
            ImportFormat::DashlaneCsv => "Dashlane credentials CSV",
            ImportFormat::DashlaneNotesCsv => "Dashlane secure notes CSV",
        }
    }
}

/// One parsed entry, format-independent. Sensitive fields are wiped on drop.
#[derive(Debug, Default)]
pub struct ImportedEntry {
    pub label: String,
    pub kind: EntryType,
    pub username: Option<String>,
    pub email: Option<String>,
    pub url: Option<String>,
    pub folder: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub password: String,
    pub notes: Option<String>,
    /// Stored verbatim: either a bare base32 seed or a full `otpauth://` URI
    /// (both are accepted by `totp::code_for_secret`).
    pub totp: Option<String>,
    pub custom_fields: Vec<CustomField>,
}

impl ImportedEntry {
    /// Wipes the sensitive fields in place. (Not a `Drop` impl: that would
    /// forbid the struct-update syntax the parsers use; the owning
    /// [`ParsedImport`] wipes every entry on drop instead.)
    fn wipe(&mut self) {
        self.password.zeroize();
        if let Some(n) = self.notes.as_mut() {
            n.zeroize();
        }
        if let Some(t) = self.totp.as_mut() {
            t.zeroize();
        }
        if let Some(e) = self.email.as_mut() {
            e.zeroize();
        }
        for f in self.custom_fields.iter_mut() {
            f.value.zeroize();
        }
    }
}

/// The parse stage's output: entries plus non-fatal warnings (skipped rows,
/// suspected corruption) the frontend should surface. Every entry's sensitive
/// fields are wiped when this drops.
#[derive(Debug, Default)]
pub struct ParsedImport {
    pub entries: Vec<ImportedEntry>,
    pub warnings: Vec<String>,
}

impl Drop for ParsedImport {
    fn drop(&mut self) {
        for e in self.entries.iter_mut() {
            e.wipe();
        }
    }
}

/// The commit stage's report.
#[derive(Debug, Default)]
pub struct ImportReport {
    pub added: usize,
    /// (original label, stored-as label) for entries renamed by the conflict policy.
    pub renamed: Vec<(String, String)>,
    pub skipped: usize,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Detects the import format from raw file bytes.
///
/// Unsupported-but-recognized inputs return a typed error with a concrete hint
/// (e.g. "export CSV from KeePassXC instead") rather than a generic failure.
pub fn detect_format(bytes: &[u8]) -> Result<ImportFormat> {
    // Binary magics first.
    if bytes.starts_with(b"PK\x03\x04") {
        return Err(AppError::Validation(
            "this is a ZIP archive (1Password 1PUX or Proton Pass export); \
             export as CSV instead and import that"
                .to_string(),
        ));
    }
    if bytes.starts_with(&[0x03, 0xD9, 0xA2, 0x9A]) || bytes.starts_with(&[0x65, 0xFB, 0x4B, 0xB5])
    {
        return Err(AppError::Validation(
            "this is a binary KeePass database (.kdbx); open it in KeePassXC and \
             use Database > Export > CSV, then import that file"
                .to_string(),
        ));
    }

    let text = std::str::from_utf8(strip_bom(bytes))
        .map_err(|_| AppError::Validation("file is not valid UTF-8 text".to_string()))?;
    let trimmed = text.trim_start();

    // JSON probe.
    if trimmed.starts_with('{') {
        let value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| AppError::Validation(format!("unrecognized JSON file: {e}")))?;
        if value.get("encrypted").and_then(|v| v.as_bool()) == Some(true) {
            return Err(AppError::Validation(
                "this Bitwarden JSON export is encrypted; re-export it as \
                 'unencrypted .json' and import that"
                    .to_string(),
            ));
        }
        if value.get("items").map(|i| i.is_array()) == Some(true) {
            return Ok(ImportFormat::BitwardenJson);
        }
        return Err(AppError::Validation(
            "unrecognized JSON structure (expected a Bitwarden unencrypted export)".to_string(),
        ));
    }
    if trimmed.starts_with("<?xml") || trimmed.starts_with('<') {
        return Err(AppError::Validation(
            "XML imports are not supported yet; export as CSV instead (KeePassXC: \
             Database > Export > CSV)"
                .to_string(),
        ));
    }

    // CSV header rules, most specific first. Headers never contain quoted
    // newlines in any of these formats, so the first line is the header.
    let header_line = text.lines().next().unwrap_or("").trim_end_matches('\r');
    let headers: Vec<String> = parse_header_row(header_line);
    let has = |name: &str| headers.iter().any(|h| h == name);
    let has_ci = |name: &str| headers.iter().any(|h| h.eq_ignore_ascii_case(name));
    let exact = |expect: &[&str]| {
        headers.len() == expect.len() && headers.iter().zip(expect).all(|(h, e)| h == e)
    };

    // LastPass form-fill profile export: reject with guidance, never fall through.
    if has("profilename") && has("profilelanguage") {
        return Err(AppError::Validation(
            "this is a LastPass form-fill profiles export; export your vault \
             (passwords) instead"
                .to_string(),
        ));
    }
    // Lilypad native.
    if exact(&[
        "Label", "Type", "Username", "URL", "Folder", "Tags", "Password", "Notes", "TOTP",
    ]) {
        return Ok(ImportFormat::LilypadCsv);
    }
    // Bitwarden CSV: the login_* prefix is unique among mainstream managers.
    if has("login_uri") && has("login_username") {
        return Ok(ImportFormat::BitwardenCsv);
    }
    // Proton Pass CSV.
    if headers.first().map(String::as_str) == Some("type")
        && has("name")
        && has("totp")
        && has("createTime")
        && has("modifyTime")
    {
        return Ok(ImportFormat::ProtonPassCsv);
    }
    // KeePassXC CSV (10-column current, or the 6-column legacy prefix).
    if exact(&[
        "Group",
        "Title",
        "Username",
        "Password",
        "URL",
        "Notes",
        "TOTP",
        "Icon",
        "Last Modified",
        "Created",
    ]) || exact(&["Group", "Title", "Username", "Password", "URL", "Notes"])
    {
        return Ok(ImportFormat::KeePassXcCsv);
    }
    // Dashlane credentials.csv (otpUrl on current exports, otpSecret on older).
    if has("username2") && has("username3") && has("title") && has("password") {
        return Ok(ImportFormat::DashlaneCsv);
    }
    // 1Password 8 CSV - must precede Safari (both TitleCase Title+OTPAuth).
    if exact(&[
        "Title", "Url", "Username", "Password", "OTPAuth", "Favorite", "Archived", "Tags", "Notes",
    ]) {
        return Ok(ImportFormat::OnePasswordCsv);
    }
    // Safari / Apple Passwords.
    if headers.first().map(String::as_str) == Some("Title")
        && has("OTPAuth")
        && !has("Favorite")
        && !has("Archived")
    {
        return Ok(ImportFormat::SafariCsv);
    }
    // Firefox: distinctive metadata columns.
    if has_ci("url") && has_ci("username") && has_ci("password") && has("httpRealm") && has("guid")
    {
        return Ok(ImportFormat::FirefoxCsv);
    }
    // LastPass vault CSV: 7-name subset rule (totp optional: 8-col current,
    // 7-col legacy/lastpass-cli). Placed after all exact rules.
    if [
        "url", "username", "password", "extra", "name", "grouping", "fav",
    ]
    .iter()
    .all(|n| has(n))
    {
        return Ok(ImportFormat::LastPassCsv);
    }
    // Chrome/Chromium/Edge/Brave: short and generic, so exact-equality, late.
    if exact(&["name", "url", "username", "password", "note"])
        || exact(&["name", "url", "username", "password"])
    {
        return Ok(ImportFormat::ChromeCsv);
    }
    // Dashlane secure notes: the most ambiguous header in the set - last.
    if exact(&["title", "note"]) {
        return Ok(ImportFormat::DashlaneNotesCsv);
    }

    Err(AppError::Validation(format!(
        "unrecognized file format (CSV header seen: {header_line:?}); supported: \
         Lilypad, LastPass, Bitwarden (CSV/JSON), KeePassXC, 1Password, Safari, \
         Chrome/Edge, Firefox, Proton Pass, Dashlane"
    )))
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parses file bytes in the given (detected) format into entries + warnings.
pub fn parse_import(bytes: &[u8], format: ImportFormat) -> Result<ParsedImport> {
    let text = std::str::from_utf8(strip_bom(bytes))
        .map_err(|_| AppError::Validation("file is not valid UTF-8 text".to_string()))?;
    let mut parsed = match format {
        ImportFormat::BitwardenJson => parse_bitwarden_json(text)?,
        _ => {
            // LastPass exports exist with pathological \r\r\n row terminators;
            // normalize line endings OUTSIDE quoted regions before parsing.
            let normalized = Zeroizing::new(normalize_newlines_quote_aware(text));
            parse_csv(&normalized, format)?
        }
    };

    // Suspected HTML-entity corruption (historic LastPass web-vault exports
    // HTML-encoded special characters). Warn - never silently decode, a
    // password can legitimately contain '&amp;'.
    if format == ImportFormat::LastPassCsv {
        let sus = parsed.entries.iter().any(|e| {
            let inb = |s: &str| {
                s.contains("&amp;")
                    || s.contains("&lt;")
                    || s.contains("&gt;")
                    || s.contains("&#39;")
            };
            inb(&e.password) || e.notes.as_deref().map(inb).unwrap_or(false)
        });
        if sus {
            parsed.warnings.push(
                "some values look HTML-encoded (&amp;, &lt;, ...): this LastPass export \
                 may be corrupted by the old web-vault bug; verify affected passwords"
                    .to_string(),
            );
        }
    }
    Ok(parsed)
}

fn parse_csv(text: &str, format: ImportFormat) -> Result<ParsedImport> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| AppError::Validation(format!("invalid CSV header: {e}")))?
        .iter()
        .map(|h| h.to_string())
        .collect();
    let idx = |name: &str| headers.iter().position(|h| h == name);
    let idx_ci = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));

    let mut out = ParsedImport::default();

    // Collect records up front so broken LastPass rows can be salvaged: real
    // exports exist where a multiline note was written UNQUOTED, splitting one
    // logical row across several physical lines (and shifting columns, which
    // puts passwords into the folder column). Rejoin fragments until the field
    // count matches the header again, like Proton Pass's importer does.
    let mut records: Vec<csv::StringRecord> = Vec::new();
    for (row_no, record) in reader.records().enumerate() {
        match record {
            Ok(r) => records.push(r),
            Err(e) => out
                .warnings
                .push(format!("row {}: unparseable, skipped ({e})", row_no + 2)),
        }
    }
    if format == ImportFormat::LastPassCsv {
        records = salvage_broken_rows(records, headers.len(), &mut out.warnings);
    }

    for (row_no, record) in records.iter().enumerate() {
        if record.len() != headers.len() {
            out.warnings.push(format!(
                "row {}: has {} fields, expected {} - skipped (a note with unquoted \
                 line breaks in the source export can cause this)",
                row_no + 2,
                record.len(),
                headers.len()
            ));
            continue;
        }
        let get = |i: Option<usize>| -> String {
            i.and_then(|i| record.get(i)).unwrap_or("").to_string()
        };
        let opt = |s: String| if s.is_empty() { None } else { Some(s) };

        let entry = match format {
            ImportFormat::LilypadCsv => ImportedEntry {
                label: get(idx("Label")),
                kind: parse_entry_type_name(&get(idx("Type"))),
                username: opt(get(idx("Username"))),
                url: opt(get(idx("URL"))),
                folder: opt(get(idx("Folder"))),
                tags: split_tags(&get(idx("Tags"))),
                password: get(idx("Password")),
                notes: opt(get(idx("Notes"))),
                totp: opt(get(idx("TOTP"))),
                ..Default::default()
            },
            ImportFormat::LastPassCsv => {
                let url = get(idx("url"));
                if url == "http://group" {
                    // Empty-folder placeholder row; lastpass-cli skips these too.
                    continue;
                }
                let extra = get(idx("extra"));
                let is_note = url == "http://sn";
                let (kind, notes) = if is_note {
                    lastpass_note_kind_and_body(&extra)
                } else {
                    (EntryType::Login, opt(extra))
                };
                ImportedEntry {
                    label: get(idx("name")),
                    kind,
                    username: opt(get(idx("username"))),
                    // `http://` alone is LastPass's password-only placeholder.
                    url: if is_note || url == "http://" {
                        None
                    } else {
                        opt(url)
                    },
                    // "(none)" is LastPass's legacy "no folder" literal.
                    folder: match get(idx("grouping")) {
                        g if g.is_empty() || g == "(none)" => None,
                        g => Some(g.replace('\\', "/")),
                    },
                    favorite: get(idx("fav")) == "1",
                    password: get(idx("password")),
                    notes,
                    totp: opt(get(idx("totp"))),
                    ..Default::default()
                }
            }
            ImportFormat::BitwardenCsv => {
                let ty = get(idx("type"));
                ImportedEntry {
                    label: get(idx("name")),
                    kind: if ty == "note" {
                        EntryType::SecureNote
                    } else {
                        EntryType::Login
                    },
                    username: opt(get(idx("login_username"))),
                    url: opt(get(idx("login_uri"))),
                    folder: opt(get(idx("folder")).or_empty_from(get(idx("collections")))),
                    favorite: get(idx("favorite")) == "1",
                    password: get(idx("login_password")),
                    notes: opt(get(idx("notes"))),
                    totp: opt(get(idx("login_totp"))),
                    ..Default::default()
                }
            }
            ImportFormat::KeePassXcCsv => ImportedEntry {
                label: get(idx("Title")),
                kind: EntryType::Login,
                username: opt(get(idx("Username"))),
                url: opt(get(idx("URL"))),
                // The first path component is the localized database root
                // ("Root", "Passwords", ...): strip it.
                folder: strip_keepass_root(&get(idx("Group"))),
                password: get(idx("Password")),
                notes: opt(get(idx("Notes"))),
                totp: opt(get(idx("TOTP"))),
                ..Default::default()
            },
            ImportFormat::OnePasswordCsv => ImportedEntry {
                label: get(idx("Title")),
                kind: EntryType::Login,
                username: opt(get(idx("Username"))),
                url: opt(get(idx("Url"))),
                tags: split_tags(&get(idx("Tags"))),
                favorite: get(idx("Favorite")).eq_ignore_ascii_case("true"),
                password: get(idx("Password")),
                notes: opt(get(idx("Notes"))),
                totp: opt(get(idx("OTPAuth"))),
                ..Default::default()
            },
            ImportFormat::SafariCsv => ImportedEntry {
                label: get(idx("Title")),
                kind: EntryType::Login,
                username: opt(get(idx("Username"))),
                url: opt(get(idx("URL")).or_empty_from(get(idx("Url")))),
                password: get(idx("Password")),
                notes: opt(get(idx("Notes"))),
                totp: opt(get(idx("OTPAuth"))),
                ..Default::default()
            },
            ImportFormat::ChromeCsv => {
                let url = get(idx("url"));
                let name = get(idx("name"));
                ImportedEntry {
                    label: if name.is_empty() {
                        host_of(&url).unwrap_or_else(|| "Imported entry".to_string())
                    } else {
                        name
                    },
                    kind: EntryType::Login,
                    username: opt(get(idx("username"))),
                    url: opt(url),
                    password: get(idx("password")),
                    notes: opt(get(idx("note"))),
                    ..Default::default()
                }
            }
            ImportFormat::FirefoxCsv => {
                let url = get(idx_ci("url").or_else(|| idx_ci("hostname")));
                if url == "chrome://FirefoxAccounts" {
                    continue;
                }
                ImportedEntry {
                    label: host_of(&url).unwrap_or_else(|| "Imported entry".to_string()),
                    kind: EntryType::Login,
                    username: opt(get(idx_ci("username"))),
                    url: opt(url),
                    password: get(idx_ci("password")),
                    ..Default::default()
                }
            }
            ImportFormat::ProtonPassCsv => {
                let ty = get(idx("type"));
                ImportedEntry {
                    label: get(idx("name")),
                    kind: if ty == "note" {
                        EntryType::SecureNote
                    } else {
                        EntryType::Login
                    },
                    username: opt(get(idx("username"))),
                    email: opt(get(idx("email"))),
                    url: opt(get(idx("url"))),
                    folder: opt(get(idx("vault"))),
                    password: get(idx("password")),
                    notes: opt(get(idx("note"))),
                    totp: opt(get(idx("totp"))),
                    ..Default::default()
                }
            }
            ImportFormat::DashlaneCsv => {
                let title = get(idx("title"));
                let url = get(idx("url"));
                ImportedEntry {
                    label: if title.is_empty() {
                        host_of(&url).unwrap_or_else(|| "Imported entry".to_string())
                    } else {
                        title
                    },
                    kind: EntryType::Login,
                    username: opt(get(idx("username"))),
                    url: opt(url),
                    folder: opt(get(idx("category"))),
                    password: get(idx("password")),
                    notes: opt(get(idx("note"))),
                    totp: opt(get(idx("otpUrl")).or_empty_from(get(idx("otpSecret")))),
                    ..Default::default()
                }
            }
            ImportFormat::DashlaneNotesCsv => ImportedEntry {
                label: get(idx("title")),
                kind: EntryType::SecureNote,
                notes: opt(get(idx("note"))),
                ..Default::default()
            },
            ImportFormat::BitwardenJson => unreachable!("handled by parse_bitwarden_json"),
        };

        push_if_meaningful(&mut out, entry, row_no + 2);
    }
    Ok(out)
}

fn parse_bitwarden_json(text: &str) -> Result<ParsedImport> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| AppError::Validation(format!("invalid Bitwarden JSON: {e}")))?;
    let mut out = ParsedImport::default();

    // folderId -> folder name.
    let mut folders = std::collections::HashMap::new();
    if let Some(arr) = value.get("folders").and_then(|f| f.as_array()) {
        for f in arr {
            if let (Some(id), Some(name)) = (
                f.get("id").and_then(|v| v.as_str()),
                f.get("name").and_then(|v| v.as_str()),
            ) {
                folders.insert(id.to_string(), name.to_string());
            }
        }
    }

    let items = value
        .get("items")
        .and_then(|i| i.as_array())
        .ok_or_else(|| AppError::Validation("Bitwarden JSON has no items array".to_string()))?;
    for (i, item) in items.iter().enumerate() {
        let s = |v: Option<&serde_json::Value>| -> Option<String> {
            v.and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
        };
        let ty = item.get("type").and_then(|v| v.as_u64()).unwrap_or(1);
        let login = item.get("login");
        let kind = match ty {
            1 => EntryType::Login,
            2 => EntryType::SecureNote,
            3 => EntryType::Card,
            4 => EntryType::Identity,
            _ => EntryType::Custom,
        };

        // Custom fields (type 0 text / 1 hidden / 2 boolean; 3 linked skipped).
        let mut custom_fields = Vec::new();
        if let Some(fields) = item.get("fields").and_then(|f| f.as_array()) {
            for f in fields {
                let name = s(f.get("name")).unwrap_or_else(|| "field".to_string());
                let val = s(f.get("value")).unwrap_or_default();
                let field = match f.get("type").and_then(|v| v.as_u64()).unwrap_or(0) {
                    1 => CustomField::hidden(name, val),
                    2 => CustomField::boolean(name, val == "true"),
                    3 => continue,
                    _ => CustomField::new(name, val),
                };
                custom_fields.push(field);
            }
        }
        // Card/Identity structured payloads become custom fields (lossless-ish).
        for (obj_key, prefix) in [("card", "card"), ("identity", "identity")] {
            if let Some(obj) = item.get(obj_key).and_then(|c| c.as_object()) {
                for (k, v) in obj {
                    if let Some(v) = v.as_str().filter(|v| !v.is_empty()) {
                        let name = format!("{prefix}:{k}");
                        let f = if k == "number" || k == "code" {
                            CustomField::hidden(name, v)
                        } else {
                            CustomField::new(name, v)
                        };
                        custom_fields.push(f);
                    }
                }
            }
        }

        let entry = ImportedEntry {
            label: s(item.get("name")).unwrap_or_else(|| "Imported entry".to_string()),
            kind,
            username: s(login.and_then(|l| l.get("username"))),
            url: login
                .and_then(|l| l.get("uris"))
                .and_then(|u| u.as_array())
                .and_then(|a| a.first())
                .and_then(|u| u.get("uri"))
                .and_then(|v| v.as_str())
                .map(String::from),
            folder: s(item.get("folderId"))
                .and_then(|id| folders.get(&id).cloned())
                .or_else(|| s(item.get("folderId"))),
            favorite: item
                .get("favorite")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            password: s(login.and_then(|l| l.get("password"))).unwrap_or_default(),
            notes: s(item.get("notes")),
            totp: s(login.and_then(|l| l.get("totp"))),
            custom_fields,
            ..Default::default()
        };
        push_if_meaningful(&mut out, entry, i + 1);
    }
    Ok(out)
}

/// Skips entries that carry nothing of value; everything else is kept.
fn push_if_meaningful(out: &mut ParsedImport, mut entry: ImportedEntry, row: usize) {
    if entry.password.is_empty()
        && entry.notes.is_none()
        && entry.totp.is_none()
        && entry.custom_fields.is_empty()
    {
        entry.wipe();
        out.warnings.push(format!(
            "row {row}: no password, notes, TOTP or fields - skipped"
        ));
        return;
    }
    if entry.label.trim().is_empty() {
        entry.label = entry
            .url
            .as_deref()
            .and_then(host_of_ref)
            .unwrap_or_else(|| "Imported entry".to_string());
    }
    out.entries.push(entry);
}

// ---------------------------------------------------------------------------
// Committing
// ---------------------------------------------------------------------------

/// Commits parsed entries into the vault in ONE locked read-modify-write.
///
/// Non-destructive: an incoming label that collides with any existing entry
/// (live or trashed) is stored under `label (imported)` / `label (imported N)`
/// instead of overwriting. Records an `entries_imported` audit event.
pub fn import_entries(
    app: &App,
    session: &mut Session,
    parsed: ParsedImport,
) -> Result<ImportReport> {
    let key = session.key().clone();
    let name = session.name().to_string();
    let lock_dir = app.lock_dir();

    let mut report = ImportReport {
        warnings: parsed.warnings.clone(),
        ..Default::default()
    };

    let updated = with_vault_locked(&lock_dir, &name, || {
        let mut vault = app
            .store()
            .load_vault(&name, &key)
            .map_err(AppError::from)?;
        let mut taken: std::collections::HashSet<String> =
            vault.entries.iter().map(|e| e.label.clone()).collect();

        for imported in &parsed.entries {
            let mut secret = EntrySecret::new(imported.password.clone());
            secret.notes = imported.notes.clone();
            secret.totp_secret = imported.totp.clone();
            secret.email = imported.email.clone();
            secret.custom_fields = imported.custom_fields.clone();
            let metadata = EntryMetadata {
                username: imported.username.clone(),
                url: imported.url.clone(),
                tags: imported.tags.clone(),
                folder: imported.folder.clone(),
                entry_type: imported.kind.clone(),
            };
            if secret.validate().is_err() || metadata.validate().is_err() {
                report.skipped += 1;
                report
                    .warnings
                    .push(format!("'{}': failed validation - skipped", imported.label));
                continue;
            }

            // Conflict policy: rename, never overwrite.
            let base = imported.label.trim().to_string();
            let mut label = base.clone();
            let mut n = 1usize;
            while taken.contains(&label) {
                n += 1;
                label = if n == 2 {
                    format!("{base} (imported)")
                } else {
                    format!("{base} (imported {n})")
                };
            }
            if label != base {
                report.renamed.push((base, label.clone()));
            }
            taken.insert(label.clone());

            let ciphertext = seal_secret(&key, &secret)?;
            wipe_entry_secret(&mut secret);
            let mut entry = Entry::new_with_metadata(&label, metadata, ciphertext);
            entry.is_favorite = imported.favorite;
            vault.add_entry(entry).map_err(AppError::from)?;
            report.added += 1;
        }

        vault.record_mutation("entries_imported", None);
        app.store()
            .save_vault(&vault, &key)
            .map_err(AppError::from)?;
        Ok(vault)
    })?;

    *session.vault_mut() = updated;
    session.touch();
    Ok(report)
}

fn wipe_entry_secret(secret: &mut EntrySecret) {
    secret.password.zeroize();
    if let Some(n) = secret.notes.as_mut() {
        n.zeroize();
    }
    if let Some(t) = secret.totp_secret.as_mut() {
        t.zeroize();
    }
    if let Some(e) = secret.email.as_mut() {
        e.zeroize();
    }
    for f in secret.custom_fields.iter_mut() {
        f.value.zeroize();
    }
}

// ---------------------------------------------------------------------------
// Native export
// ---------------------------------------------------------------------------

/// Exports the vault's live entries as Lilypad-native CSV (PLAINTEXT: every
/// secret is decrypted). The returned buffer zeroizes on drop; the caller
/// decides where it is written and must warn the user.
pub fn export_csv(session: &Session) -> Result<Zeroizing<String>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "Label", "Type", "Username", "URL", "Folder", "Tags", "Password", "Notes", "TOTP",
        ])
        .map_err(|e| AppError::Other(format!("CSV write failed: {e}")))?;
    for entry in session.vault().entries.iter().filter(|e| e.deleted_at == 0) {
        let revealed = crate::secret::open_secret(session.key(), entry)?;
        writer
            .write_record([
                entry.label.as_str(),
                entry_type_name(&entry.metadata.entry_type),
                entry.metadata.username.as_deref().unwrap_or(""),
                entry.metadata.url.as_deref().unwrap_or(""),
                entry.metadata.folder.as_deref().unwrap_or(""),
                &entry.metadata.tags.join(","),
                revealed.password.as_str(),
                revealed.notes.as_deref().unwrap_or(""),
                revealed.totp_secret.as_deref().unwrap_or(""),
            ])
            .map_err(|e| AppError::Other(format!("CSV write failed: {e}")))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|e| AppError::Other(format!("CSV write failed: {e}")))?;
    String::from_utf8(bytes)
        .map(Zeroizing::new)
        .map_err(|_| AppError::Other("CSV output was not UTF-8".to_string()))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Rejoins logical rows that a source export split across physical lines
/// (LastPass writes multiline notes UNQUOTED in some real exports). A fragment
/// with fewer fields than the header starts a pending row; each following
/// fragment's first field is glued to the pending row's last field with a
/// newline (it is the continuation of the broken cell) and the rest appended,
/// until the field count reaches the header's. Overshoot drops the row with a
/// warning rather than importing shifted columns (which would put passwords in
/// the folder column).
fn salvage_broken_rows(
    records: Vec<csv::StringRecord>,
    want: usize,
    warnings: &mut Vec<String>,
) -> Vec<csv::StringRecord> {
    let mut out = Vec::with_capacity(records.len());
    let mut pending: Option<Vec<String>> = None;
    for record in records {
        let fields: Vec<String> = record.iter().map(String::from).collect();
        match pending.take() {
            None => {
                if fields.len() >= want {
                    out.push(record); // normal (or overlong: caught by the caller)
                } else {
                    pending = Some(fields);
                }
            }
            Some(mut row) => {
                let mut it = fields.into_iter();
                if let Some(first) = it.next() {
                    if let Some(last) = row.last_mut() {
                        last.push('\n');
                        last.push_str(&first);
                    }
                }
                row.extend(it);
                match row.len().cmp(&want) {
                    std::cmp::Ordering::Equal => {
                        warnings.push(format!(
                            "salvaged a row split across lines by unquoted line breaks \
                             ('{}')",
                            row.get(5).map(String::as_str).unwrap_or("?")
                        ));
                        out.push(csv::StringRecord::from(row));
                    }
                    std::cmp::Ordering::Less => pending = Some(row),
                    std::cmp::Ordering::Greater => {
                        warnings.push(
                            "dropped an unrecoverable broken row (fields would have \
                             shifted columns)"
                                .to_string(),
                        );
                    }
                }
            }
        }
    }
    if pending.is_some() {
        warnings.push("dropped an incomplete trailing row".to_string());
    }
    out
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

/// Parses the (never-multiline) header row, tolerating quoted names.
fn parse_header_row(line: &str) -> Vec<String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(line.as_bytes());
    reader
        .records()
        .next()
        .and_then(|r| r.ok())
        .map(|r| r.iter().map(|f| f.to_string()).collect())
        .unwrap_or_default()
}

/// Normalizes `\r\r\n` and `\r\n` to `\n` OUTSIDE quoted regions (LastPass
/// ships exports with `\r\r\n` row terminators; notes cells legitimately
/// contain raw newlines inside quotes and must not be touched).
fn normalize_newlines_quote_aware(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_quotes = false;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '"' => {
                in_quotes = !in_quotes;
                out.push(c);
                i += 1;
            }
            '\r' if !in_quotes => {
                // Collapse \r{1,2}\n to \n; a lone \r becomes \n.
                let mut j = i;
                while j < bytes.len() && bytes[j] == b'\r' {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'\n' {
                    j += 1;
                }
                out.push('\n');
                i = j;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// LastPass typed secure notes: `extra` starts with `NoteType:<Type>`, then
/// `Language:xx` (skipped), then `Key:Value` lines; everything from the
/// `Notes:` key onward is the free-text body. The full body is preserved as
/// the entry's notes; the NoteType maps the entry kind.
fn lastpass_note_kind_and_body(extra: &str) -> (EntryType, Option<String>) {
    let Some(rest) = extra.strip_prefix("NoteType:") else {
        return (
            EntryType::SecureNote,
            if extra.is_empty() {
                None
            } else {
                Some(extra.to_string())
            },
        );
    };
    let note_type = rest.lines().next().unwrap_or("").trim();
    let kind = match note_type {
        "Credit Card" => EntryType::Card,
        "Address" => EntryType::Identity,
        "Wi-Fi Password" => EntryType::Wifi,
        "Server" | "Database" | "SSH Key" => EntryType::Server,
        "Software License" => EntryType::SoftwareLicense,
        _ => EntryType::SecureNote,
    };
    (kind, Some(extra.to_string()))
}

/// Strips the first (localized root) component of a KeePass group path.
fn strip_keepass_root(group: &str) -> Option<String> {
    let group = group.trim_matches('/');
    if group.is_empty() {
        return None;
    }
    match group.split_once('/') {
        Some((_root, rest)) if !rest.is_empty() => Some(rest.to_string()),
        _ => None, // only the root group: no folder
    }
}

fn split_tags(s: &str) -> Vec<String> {
    s.split([',', ';'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

fn host_of(url: &str) -> Option<String> {
    host_of_ref(url)
}

fn host_of_ref(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    let without = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host = without.split('/').next()?.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn parse_entry_type_name(s: &str) -> EntryType {
    match s.to_lowercase().replace('_', "-").as_str() {
        "card" => EntryType::Card,
        "identity" => EntryType::Identity,
        "secure-note" | "securenote" | "note" => EntryType::SecureNote,
        "software-license" | "softwarelicense" => EntryType::SoftwareLicense,
        "wifi" => EntryType::Wifi,
        "server" => EntryType::Server,
        "custom" => EntryType::Custom,
        _ => EntryType::Login,
    }
}

fn entry_type_name(t: &EntryType) -> &'static str {
    match t {
        EntryType::Login => "login",
        EntryType::Card => "card",
        EntryType::Identity => "identity",
        EntryType::SecureNote => "secure-note",
        EntryType::SoftwareLicense => "software-license",
        EntryType::Wifi => "wifi",
        EntryType::Server => "server",
        EntryType::Custom => "custom",
    }
}

/// `a.or_empty_from(b)`: `a` if non-empty, else `b`.
trait OrEmptyFrom {
    fn or_empty_from(self, other: String) -> String;
}
impl OrEmptyFrom for String {
    fn or_empty_from(self, other: String) -> String {
        if self.is_empty() {
            other
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::{App, OpenOptions};
    use tempfile::TempDir;

    fn test_app(dir: &TempDir) -> App {
        App::open(OpenOptions {
            data_dir: Some(dir.path().to_path_buf()),
            auto_lock_after: None,
        })
        .unwrap()
    }

    #[test]
    fn detects_every_supported_header() {
        let cases: [(&[u8], ImportFormat); 10] = [
            (b"url,username,password,totp,extra,name,grouping,fav\n" as &[u8], ImportFormat::LastPassCsv),
            (b"url,username,password,extra,name,grouping,fav\n", ImportFormat::LastPassCsv),
            (b"folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n", ImportFormat::BitwardenCsv),
            (b"Group,Title,Username,Password,URL,Notes,TOTP,Icon,Last Modified,Created\n", ImportFormat::KeePassXcCsv),
            (b"Title,Url,Username,Password,OTPAuth,Favorite,Archived,Tags,Notes\n", ImportFormat::OnePasswordCsv),
            (b"Title,URL,Username,Password,Notes,OTPAuth\n", ImportFormat::SafariCsv),
            (b"name,url,username,password,note\n", ImportFormat::ChromeCsv),
            (b"url,username,password,httpRealm,formActionOrigin,guid,timeCreated,timeLastUsed,timePasswordChanged\n", ImportFormat::FirefoxCsv),
            (b"type,name,url,email,username,password,note,totp,createTime,modifyTime,vault\n", ImportFormat::ProtonPassCsv),
            (b"Label,Type,Username,URL,Folder,Tags,Password,Notes,TOTP\n", ImportFormat::LilypadCsv),
        ];
        for (header, expected) in cases {
            assert_eq!(
                detect_format(header).unwrap(),
                expected,
                "header: {:?}",
                std::str::from_utf8(header)
            );
        }
        // A UTF-8 BOM must not break exact-header rules.
        assert_eq!(
            detect_format(b"\xEF\xBB\xBFname,url,username,password,note\n").unwrap(),
            ImportFormat::ChromeCsv
        );
        // Recognized-but-unsupported inputs give actionable errors.
        assert!(detect_format(&[0x03, 0xD9, 0xA2, 0x9A, 0x67])
            .unwrap_err()
            .to_string()
            .contains("KeePassXC"));
        assert!(detect_format(b"PK\x03\x04zzz")
            .unwrap_err()
            .to_string()
            .contains("CSV"));
        assert!(
            detect_format(br#"{"encrypted":true,"passwordProtected":true}"#)
                .unwrap_err()
                .to_string()
                .contains("unencrypted")
        );
        assert!(detect_format(b"profilename,profilelanguage,title\n")
            .unwrap_err()
            .to_string()
            .contains("form-fill"));
        assert!(detect_format(b"colonnes,inconnues\n").is_err());
    }

    #[test]
    fn lastpass_parses_notes_groups_and_quirks() {
        // \r\r\n row terminators (real-world LastPass corruption) + a secure
        // note + an empty-folder placeholder row + a favorite.
        let csv = "url,username,password,totp,extra,name,grouping,fav\r\r\n\
                   https://ex.com,alice,pw1,GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ,\"line1\nline2\",Example,Work\\Email,1\r\r\n\
                   http://group,,,,,placeholder,Empty\\Folder,0\r\r\n\
                   http://sn,,,,\"NoteType:Credit Card\nLanguage:en-US\nNumber:4111\nNotes:my card\",Visa,,0\r\r\n";
        let parsed = parse_import(csv.as_bytes(), ImportFormat::LastPassCsv).unwrap();
        assert_eq!(parsed.entries.len(), 2, "placeholder row must be skipped");

        let login = &parsed.entries[0];
        assert_eq!(login.label, "Example");
        assert_eq!(login.kind, EntryType::Login);
        assert_eq!(login.folder.as_deref(), Some("Work/Email"));
        assert!(login.favorite);
        assert_eq!(login.notes.as_deref(), Some("line1\nline2"));
        assert_eq!(
            login.totp.as_deref(),
            Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ")
        );

        let note = &parsed.entries[1];
        assert_eq!(
            note.kind,
            EntryType::Card,
            "NoteType:Credit Card maps to Card"
        );
        assert!(note.url.is_none(), "http://sn is a sentinel, not a URL");
        assert!(note.notes.as_deref().unwrap().contains("Number:4111"));
    }

    #[test]
    fn lastpass_none_grouping_means_no_folder() {
        let csv = "url,username,password,totp,extra,name,grouping,fav\n\
                   https://a.com,u,p,,,A,(none),0\n";
        let parsed = parse_import(csv.as_bytes(), ImportFormat::LastPassCsv).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert!(parsed.entries[0].folder.is_none(), "(none) is not a folder");
    }

    #[test]
    fn lastpass_salvages_rows_broken_by_unquoted_multiline_notes() {
        // The note "line1\nline2" was written WITHOUT quotes by the exporter,
        // splitting one logical row across two physical lines. Naively parsed,
        // the columns shift and the password would land in `grouping` (shown as
        // a folder). The salvage pass must rejoin it.
        let csv = "url,username,password,totp,extra,name,grouping,fav\n\
                   https://ok.com,u1,p1,,fine,OK,Work,0\n\
                   https://broken.com,u2,p2,,line1\n\
                   line2,Broken,Personal,0\n";
        let parsed = parse_import(csv.as_bytes(), ImportFormat::LastPassCsv).unwrap();
        assert_eq!(parsed.entries.len(), 2, "the broken row must be recovered");
        let broken = parsed.entries.iter().find(|e| e.label == "Broken").unwrap();
        assert_eq!(broken.password, "p2");
        assert_eq!(broken.notes.as_deref(), Some("line1\nline2"));
        assert_eq!(broken.folder.as_deref(), Some("Personal"));
        assert!(
            parsed.warnings.iter().any(|w| w.contains("salvaged")),
            "the salvage must be surfaced as a warning"
        );
        // No password-looking garbage may end up as a folder.
        assert!(parsed
            .entries
            .iter()
            .all(|e| e.folder.as_deref() != Some("p2")));
    }

    #[test]
    fn bitwarden_json_maps_folders_types_and_custom_fields() {
        let json = r#"{
          "folders": [{"id": "f1", "name": "Work"}],
          "items": [
            {"type": 1, "name": "GitHub", "folderId": "f1", "favorite": true,
             "notes": "hi",
             "login": {"username": "alice", "password": "pw",
                       "totp": "otpauth://totp/gh?secret=ABC&period=60",
                       "uris": [{"uri": "https://github.com"}]},
             "fields": [{"name": "PIN", "value": "1234", "type": 1}]},
            {"type": 2, "name": "Note", "secureNote": {"type": 0}, "notes": "body"}
          ]
        }"#;
        let parsed = parse_import(json.as_bytes(), ImportFormat::BitwardenJson).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        let gh = &parsed.entries[0];
        assert_eq!(gh.folder.as_deref(), Some("Work"));
        assert!(gh.favorite);
        assert_eq!(
            gh.totp.as_deref(),
            Some("otpauth://totp/gh?secret=ABC&period=60")
        );
        assert_eq!(gh.custom_fields.len(), 1);
        assert_eq!(parsed.entries[1].kind, EntryType::SecureNote);
    }

    #[test]
    fn import_commits_renames_conflicts_and_round_trips() {
        let dir = TempDir::new().unwrap();
        let app = test_app(&dir);
        let mut session = app.create_vault("v", "pw").unwrap();
        // Existing entry that will collide with the import.
        crate::entries::add_entry(
            &app,
            &mut session,
            "GitHub",
            lilypad_core::EntryMetadata::default(),
            &EntrySecret::new("existing"),
        )
        .unwrap();

        let csv = "name,url,username,password,note\n\
                   GitHub,https://github.com,alice,imported-pw,\n\
                   Mail,https://mail.com,bob,pw2,a note\n";
        let parsed = parse_import(csv.as_bytes(), ImportFormat::ChromeCsv).unwrap();
        let report = import_entries(&app, &mut session, parsed).unwrap();
        assert_eq!(report.added, 2);
        assert_eq!(report.renamed.len(), 1);
        assert_eq!(report.renamed[0].0, "GitHub");
        assert_eq!(report.renamed[0].1, "GitHub (imported)");

        // The pre-existing entry was NOT overwritten.
        let existing = crate::entries::reveal_secret(&session, "GitHub").unwrap();
        assert_eq!(existing.password, "existing");
        let imported = crate::entries::reveal_secret(&session, "GitHub (imported)").unwrap();
        assert_eq!(imported.password, "imported-pw");

        // Round-trip: our export re-imports through detection unchanged.
        let exported = export_csv(&session).unwrap();
        assert_eq!(
            detect_format(exported.as_bytes()).unwrap(),
            ImportFormat::LilypadCsv
        );
        let reparsed = parse_import(exported.as_bytes(), ImportFormat::LilypadCsv).unwrap();
        assert_eq!(reparsed.entries.len(), 3);
        assert!(reparsed
            .entries
            .iter()
            .any(|e| e.label == "Mail" && e.notes.as_deref() == Some("a note")));

        // The import shows up in the audit log.
        let actions: Vec<String> = crate::entries::audit_log(&session)
            .into_iter()
            .map(|e| e.action)
            .collect();
        assert!(actions.iter().any(|a| a == "entries_imported"));
    }
}
