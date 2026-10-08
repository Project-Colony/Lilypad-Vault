//! TOTP configuration and code generation.
//!
//! The old frontends hardcoded `Algorithm::SHA1, 6 digits, 30s` at three call
//! sites, so any issuer using non-default settings produced wrong codes, and an
//! imported `otpauth://` URI silently lost its parameters. This module keeps a
//! [`TotpConfig`] that carries the algorithm/digits/period, parses `otpauth://`
//! URIs, and generates codes centrally. (Persisting per-entry parameters is an
//! additive change to `EntrySecret` handled separately; the default remains
//! SHA1/6/30 so existing `totp_secret`-only entries keep working.)

use crate::error::{AppError, Result};
use totp_rs::{Algorithm, Secret, TOTP};
use zeroize::Zeroizing;

/// Hash algorithm for a TOTP entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TotpAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

impl TotpAlgorithm {
    fn to_totp_rs(self) -> Algorithm {
        match self {
            TotpAlgorithm::Sha1 => Algorithm::SHA1,
            TotpAlgorithm::Sha256 => Algorithm::SHA256,
            TotpAlgorithm::Sha512 => Algorithm::SHA512,
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "SHA1" => Some(TotpAlgorithm::Sha1),
            "SHA256" => Some(TotpAlgorithm::Sha256),
            "SHA512" => Some(TotpAlgorithm::Sha512),
            _ => None,
        }
    }
}

/// A fully specified TOTP configuration.
#[derive(Debug, Clone)]
pub struct TotpConfig {
    /// The shared secret, base32-encoded (as issuers present it).
    pub secret: String,
    pub algorithm: TotpAlgorithm,
    pub digits: usize,
    pub period: u64,
}

impl TotpConfig {
    /// A config from a bare base32 secret using the ubiquitous SHA1/6/30 defaults.
    pub fn from_secret(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
        }
    }

    /// Parses an `otpauth://totp/...` URI, preserving algorithm/digits/period.
    pub fn from_otpauth(uri: &str) -> Result<Self> {
        let query = uri
            .split_once('?')
            .map(|(_, q)| q)
            .ok_or_else(|| AppError::Validation("otpauth URI has no parameters".to_string()))?;
        let mut secret = None;
        let mut algorithm = TotpAlgorithm::Sha1;
        let mut digits = 6usize;
        let mut period = 30u64;
        for pair in query.split('&') {
            let Some((k, v)) = pair.split_once('=') else {
                continue;
            };
            match k.to_ascii_lowercase().as_str() {
                "secret" => secret = Some(v.to_string()),
                "algorithm" => algorithm = TotpAlgorithm::parse(v).unwrap_or(TotpAlgorithm::Sha1),
                "digits" => digits = v.parse().unwrap_or(6),
                "period" => period = v.parse().unwrap_or(30),
                _ => {}
            }
        }
        let secret =
            secret.ok_or_else(|| AppError::Validation("otpauth URI has no secret".to_string()))?;
        Ok(Self {
            secret,
            algorithm,
            digits,
            period,
        })
    }

    /// Generates the current TOTP code. The result is zeroized on drop.
    ///
    /// Uses `new_unchecked` so short secrets (below RFC 6238's recommended
    /// 128-bit floor) are still accepted, matching what Google Authenticator and
    /// other apps do; many real issuers hand out 80-bit secrets, and rejecting
    /// them would make those accounts unusable in Lilypad.
    pub fn current_code(&self) -> Result<Zeroizing<String>> {
        // Decode the base32 seed into a zeroizing buffer so the raw seed bytes
        // are wiped when this call returns. `totp-rs` is built with its
        // `zeroize` feature (see workspace Cargo.toml), so the `TOTP`'s own copy
        // is wiped on drop as well.
        let decoded = Secret::Encoded(self.secret.clone())
            .to_bytes()
            .map_err(|e| AppError::Validation(format!("invalid TOTP secret: {e}")))?;
        let seed = Zeroizing::new(decoded);
        let totp = TOTP::new_unchecked(
            self.algorithm.to_totp_rs(),
            self.digits,
            1,
            self.period,
            seed.to_vec(),
        );
        let code = totp
            .generate_current()
            .map_err(|e| AppError::Crypto(format!("TOTP generation failed: {e}")))?;
        Ok(Zeroizing::new(code))
    }
}

/// Convenience: current code for a stored TOTP secret.
///
/// Accepts both encodings found in the wild (and produced by imports): a full
/// `otpauth://` URI (algorithm/digits/period preserved) or a bare base32 seed
/// (ubiquitous SHA1/6/30 defaults).
pub fn code_for_secret(secret: &str) -> Result<Zeroizing<String>> {
    let secret = secret.trim();
    if secret.starts_with("otpauth://") {
        TotpConfig::from_otpauth(secret)?.current_code()
    } else {
        TotpConfig::from_secret(secret).current_code()
    }
}
