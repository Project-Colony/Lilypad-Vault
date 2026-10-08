//! Terminal input/output helpers.
//!
//! Secrets are resolved through here so the CLI never *requires* them on the
//! command line: the master password and entry values are prompted for
//! interactively (hidden) when not supplied, read from stdin when piped, and
//! accepted from a flag only with an explicit visibility warning.

use anyhow::{anyhow, Result};
use std::io::{IsTerminal, Read, Write};
use zeroize::Zeroizing;

/// Resolves the master password: an explicitly supplied value (flag/env) is used
/// as-is (a warning is printed elsewhere), otherwise it is prompted for hidden.
pub fn resolve_master_password(provided: Option<&str>) -> Result<Zeroizing<String>> {
    if let Some(p) = provided {
        return Ok(Zeroizing::new(p.to_string()));
    }
    let p = rpassword::prompt_password("Master password: ")
        .map_err(|e| anyhow!("failed to read master password: {e}"))?;
    Ok(Zeroizing::new(p))
}

/// Prompts for a new master password twice and checks they match. When stdin is
/// piped (scripted use), reads a single line from it instead of prompting the
/// terminal, so password changes can be automated.
pub fn prompt_new_master_password() -> Result<Zeroizing<String>> {
    if !std::io::stdin().is_terminal() {
        let mut s = String::new();
        std::io::stdin()
            .read_line(&mut s)
            .map_err(|e| anyhow!("failed to read new master password from stdin: {e}"))?;
        let pw = s.trim_end_matches(['\n', '\r']).to_string();
        if pw.is_empty() {
            return Err(anyhow!("master password must not be empty"));
        }
        return Ok(Zeroizing::new(pw));
    }

    let first = rpassword::prompt_password("New master password: ")
        .map_err(|e| anyhow!("failed to read password: {e}"))?;
    let confirm = rpassword::prompt_password("Confirm new master password: ")
        .map_err(|e| anyhow!("failed to read password: {e}"))?;
    if first != confirm {
        return Err(anyhow!("passwords do not match"));
    }
    if first.trim().is_empty() {
        return Err(anyhow!("master password must not be empty"));
    }
    Ok(Zeroizing::new(first))
}

/// Resolves a secret value for an entry: an explicit value is used (with a
/// visibility warning), piped stdin is read, otherwise it is prompted hidden.
pub fn resolve_secret(provided: Option<&str>, prompt: &str) -> Result<Zeroizing<String>> {
    if let Some(v) = provided {
        eprintln!(
            "WARNING: a secret passed on the command line is visible in shell history and \
             process listings; prefer piping it or entering it interactively."
        );
        return Ok(Zeroizing::new(v.to_string()));
    }
    if !std::io::stdin().is_terminal() {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| anyhow!("failed to read secret from stdin: {e}"))?;
        let trimmed = s.trim_end_matches(['\n', '\r']).to_string();
        return Ok(Zeroizing::new(trimmed));
    }
    let v = rpassword::prompt_password(format!("{prompt}: "))
        .map_err(|e| anyhow!("failed to read secret: {e}"))?;
    Ok(Zeroizing::new(v))
}

/// Asks a yes/no question on stderr; defaults to no.
pub fn confirm(prompt: &str) -> Result<bool> {
    eprint!("{prompt} [y/N]: ");
    std::io::stderr().flush().ok();
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| anyhow!("failed to read confirmation: {e}"))?;
    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}
