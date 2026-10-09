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
use totp_rs::{Algorithm, Builder, Secret, Totp};
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
    pub fn current_code(&self) -> Result<Zeroizing<String>> {
        Ok(Zeroizing::new(self.totp()?.generate_current().to_string()))
    }

    /// Builds the generator, rejecting parameters it cannot compute a code for.
    ///
    /// Uses `build_noncompliant` so short secrets (below RFC 6238's recommended
    /// 128-bit floor) are still accepted, matching what Google Authenticator and
    /// other apps do; many real issuers hand out 80-bit secrets, and rejecting
    /// them would make those accounts unusable in Lilypad. That skips the
    /// library's own checks, so the two that would otherwise panic at generation
    /// time (more than 9 digits, a zero period) are made here. `totp-rs` is built
    /// with its `zeroize` feature (see workspace Cargo.toml), so the decoded
    /// seed is wiped when the generator is dropped.
    fn totp(&self) -> Result<Totp> {
        let secret = Secret::try_from_base32(&self.secret)
            .map_err(|e| AppError::Validation(format!("invalid TOTP secret: {e}")))?;
        let digits = u8::try_from(self.digits)
            .ok()
            .filter(|d| (1..=9).contains(d))
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "unsupported TOTP digit count: {} (expected 1 to 9)",
                    self.digits
                ))
            })?;
        if self.period == 0 {
            return Err(AppError::Validation(
                "TOTP period must be at least one second".to_string(),
            ));
        }
        Ok(Builder::new()
            .with_algorithm(self.algorithm.to_totp_rs())
            .with_digits(digits)
            .with_step_duration(self.period)
            .with_secret(secret)
            .build_noncompliant())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6238 appendix B, T = 59: one vector per algorithm, 8 digits.
    #[test]
    fn rfc6238_vectors() {
        let cases = [
            (
                TotpAlgorithm::Sha1,
                "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
                "94287082",
            ),
            (
                TotpAlgorithm::Sha256,
                "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA",
                "46119246",
            ),
            (
                TotpAlgorithm::Sha512,
                "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNA",
                "90693936",
            ),
        ];
        for (algorithm, secret, expected) in cases {
            let cfg = TotpConfig {
                secret: secret.to_string(),
                algorithm,
                digits: 8,
                period: 30,
            };
            assert_eq!(cfg.totp().unwrap().generate(59).to_string(), expected);
        }
    }

    #[test]
    fn short_secrets_are_accepted() {
        // 80 bits, below the RFC floor, as some issuers hand out.
        let code = TotpConfig::from_secret("JBSWY3DPEHPK3PXP")
            .current_code()
            .unwrap();
        assert_eq!(code.len(), 6);
    }

    #[test]
    fn parameters_that_cannot_produce_a_code_are_errors() {
        let uri = |q: &str| format!("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&{q}");
        for q in ["digits=10", "digits=0", "period=0"] {
            let cfg = TotpConfig::from_otpauth(&uri(q)).unwrap();
            assert!(cfg.current_code().is_err(), "{q} should be rejected");
        }
        assert!(TotpConfig::from_secret("not base32!")
            .current_code()
            .is_err());
    }
}
