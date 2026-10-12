//! Every key the code asks for exists, and every key in the locale files is
//! asked for somewhere: a scan of the sources of every crate.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn sources(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).unwrap();
            out.push((path, text));
        }
    }
}

/// The string literal right after each `t(` or `tf(` call.
fn requested_keys(text: &str) -> Vec<&str> {
    let mut keys = Vec::new();
    for call in ["t(\"", "tf(\""] {
        for (at, _) in text.match_indices(call) {
            let before = text[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue; // `print(`, `assert(`, ...
            }
            let rest = &text[at + call.len()..];
            if let Some(end) = rest.find('"') {
                keys.push(&rest[..end]);
            }
        }
    }
    keys
}

fn lilypad_keys() -> BTreeSet<String> {
    let path = crates_dir().join("../config/i18n/en.json");
    let table: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    table.keys().cloned().collect()
}

#[test]
fn every_requested_key_exists() {
    let ours = lilypad_keys();
    let shared: BTreeSet<&str> = colony_ui::i18n::all(colony_ui::i18n::Locale::En)
        .map(|(k, _)| k)
        .collect();
    let mut files = Vec::new();
    sources(&crates_dir(), &mut files);
    let mut missing = Vec::new();
    for (path, text) in &files {
        for key in requested_keys(text) {
            if !ours.contains(key) && !shared.contains(key) {
                missing.push(format!("{}: {key}", path.display()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "keys not in en.json:\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_key_is_used() {
    let mut files = Vec::new();
    sources(&crates_dir(), &mut files);
    let unused: Vec<String> = lilypad_keys()
        .into_iter()
        .filter(|key| {
            let quoted = format!("\"{key}\"");
            !files.iter().any(|(_, text)| text.contains(&quoted))
        })
        .collect();
    assert!(unused.is_empty(), "keys nothing uses: {unused:?}");
}
