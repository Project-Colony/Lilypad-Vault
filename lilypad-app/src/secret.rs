//! Encrypting and decrypting an [`EntrySecret`] - the one place that owns the
//! `serialize -> encrypt` / `decrypt -> deserialize` round-trip.
//!
//! `lilypad-core` deliberately does not depend on `serde_json`, so turning an
//! `EntrySecret` into an [`Entry`]'s ciphertext is the caller's job. The old
//! frontends each reimplemented it (with a legacy-plaintext fallback), and none
//! wiped the transient plaintext buffer. Here we do it once, zeroize the
//! intermediate JSON, and hand the caller a [`RevealedSecret`] guard that wipes
//! the decrypted fields when it drops.

use crate::error::{AppError, Result};
use lilypad_core::{decrypt, encrypt, Ciphertext, Entry, EntrySecret, KeyMaterial};
use std::fmt;
use std::ops::Deref;
use zeroize::Zeroize;

/// A decrypted [`EntrySecret`] whose sensitive string fields are zeroized on
/// drop. Access the inner value through `Deref`; hold it only as long as needed.
pub struct RevealedSecret(EntrySecret);

impl RevealedSecret {
    /// Borrows the inner secret.
    pub fn get(&self) -> &EntrySecret {
        &self.0
    }
}

impl Deref for RevealedSecret {
    type Target = EntrySecret;
    fn deref(&self) -> &EntrySecret {
        &self.0
    }
}

// `EntrySecret` derives `Debug`, so a redacting `Debug` here keeps `{:?}` on a
// `RevealedSecret` from printing plaintext. (Callers should never `{:?}` the
// deref'd inner value.)
impl fmt::Debug for RevealedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RevealedSecret")
            .field("password", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl Drop for RevealedSecret {
    fn drop(&mut self) {
        self.0.password.zeroize();
        if let Some(n) = self.0.notes.as_mut() {
            n.zeroize();
        }
        if let Some(t) = self.0.totp_secret.as_mut() {
            t.zeroize();
        }
        if let Some(e) = self.0.email.as_mut() {
            e.zeroize();
        }
        if let Some(p) = self.0.phone.as_mut() {
            p.zeroize();
        }
        for field in self.0.custom_fields.iter_mut() {
            field.value.zeroize();
        }
        // Attachments carry the full decrypted file body; backup codes bypass
        // 2FA. Both are as sensitive as the password and must be wiped too.
        for att in self.0.attachments.iter_mut() {
            att.data_base64.zeroize();
            att.filename.zeroize();
            if let Some(mime) = att.mime_type.as_mut() {
                mime.zeroize();
            }
        }
        for code in self.0.totp_backup_codes.iter_mut() {
            code.code.zeroize();
        }
    }
}

/// Serializes and encrypts an [`EntrySecret`] into an entry ciphertext, wiping
/// the transient JSON buffer afterward.
pub fn seal_secret(key: &KeyMaterial, secret: &EntrySecret) -> Result<Ciphertext> {
    let mut payload = serde_json::to_vec(secret)?;
    let ciphertext = encrypt(key, &payload).map_err(AppError::from)?;
    payload.zeroize();
    Ok(ciphertext)
}

/// Decrypts and deserializes an entry's secret, returning a zeroizing guard.
///
/// Falls back to the legacy format (ciphertext is a bare UTF-8 password) so
/// vaults written by pre-structured builds still open.
pub fn open_secret(key: &KeyMaterial, entry: &Entry) -> Result<RevealedSecret> {
    let mut plaintext = decrypt(key, &entry.ciphertext).map_err(AppError::from)?;
    let secret = match serde_json::from_slice::<EntrySecret>(&plaintext) {
        Ok(s) => s,
        Err(_) => {
            // Legacy: the ciphertext held the password directly as UTF-8. Borrow
            // (never clone/move) the buffer so the plaintext copy is not handed
            // to an error type that outlives our zeroize.
            match std::str::from_utf8(&plaintext) {
                Ok(s) => EntrySecret::new(s.to_string()),
                Err(_) => {
                    plaintext.zeroize();
                    return Err(AppError::Validation(format!(
                        "entry '{}' has corrupted data (invalid UTF-8)",
                        entry.label
                    )));
                }
            }
        }
    };
    plaintext.zeroize();
    Ok(RevealedSecret(secret))
}
