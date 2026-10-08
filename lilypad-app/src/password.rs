//! Master-password normalization - applied exactly once, here.
//!
//! The old frontends disagreed: the desktop and TUI called `.trim()` on the
//! master password before deriving the key, while the CLI passed it verbatim.
//! A vault created in one frontend could therefore be un-openable in another,
//! because a different byte string reached Argon2. `lilypad-app` funnels every
//! derivation through [`normalize_master_password`] so the key is byte-identical
//! no matter which frontend typed the password.
//!
//! Policy: trim leading/trailing ASCII whitespace once, then apply Unicode NFC
//! (NIST SP 800-63B allows normalization of memorized secrets; NFC is chosen so
//! that visually identical accented input on different platforms - NFD on macOS,
//! NFC on Linux - derives the same key).

use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

/// Normalizes a raw master password into the exact bytes fed to the KDF.
///
/// Trims leading/trailing ASCII whitespace once, then applies Unicode NFC, so a
/// password typed with the same characters derives the same key regardless of
/// how the platform encodes accents (macOS emits NFD, Linux NFC). The returned
/// value is wrapped in [`Zeroizing`] so the normalized copy is wiped on drop.
pub fn normalize_master_password(raw: &str) -> Zeroizing<String> {
    Zeroizing::new(raw.trim().nfc().collect::<String>())
}

/// The pre-NFC normalization (trim only), kept so vaults whose key was derived
/// before NFC landed remain openable: the unlock path retries with this form
/// when the NFC-normalized key does not open the vault.
pub(crate) fn normalize_master_password_legacy(raw: &str) -> Zeroizing<String> {
    Zeroizing::new(raw.trim().to_string())
}

/// Returns true if the password is empty after normalization.
pub fn is_blank(raw: &str) -> bool {
    normalize_master_password(raw).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_surrounding_whitespace_once() {
        assert_eq!(&*normalize_master_password("  hunter2 \n"), "hunter2");
        // Interior whitespace is preserved (it is part of the secret).
        assert_eq!(&*normalize_master_password("two words"), "two words");
    }

    #[test]
    fn nfc_makes_decomposed_and_composed_accents_derive_the_same_key() {
        // "café": composed (NFC, U+00E9) vs decomposed (NFD, "e" + U+0301).
        let composed = "caf\u{00e9}";
        let decomposed = "cafe\u{0301}";
        assert_ne!(composed, decomposed, "inputs differ byte-for-byte");
        assert_eq!(
            &*normalize_master_password(composed),
            &*normalize_master_password(decomposed),
            "NFC must unify the two encodings so the KDF sees identical bytes"
        );
        // And the unified form is the composed one.
        assert_eq!(&*normalize_master_password(decomposed), composed);
    }

    #[test]
    fn blank_detection_uses_the_normalized_form() {
        assert!(is_blank("   "));
        assert!(!is_blank(" x "));
    }
}
