//! Password and passphrase generation.
//!
//! All three frontends shipped their own `rand`-based generator with slightly
//! different charsets; this is the single shared implementation. Character
//! selection uses `rand::Rng::random_range`, which is unbiased (rejection
//! sampling under the hood), so no modulo skew is introduced.

use crate::error::{AppError, Result};
use rand::RngExt;
use zeroize::Zeroizing;

const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{}|;:,.<>?";

/// Options for [`generate_password`].
#[derive(Debug, Clone)]
pub struct PasswordOptions {
    pub length: usize,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub symbols: bool,
}

impl Default for PasswordOptions {
    fn default() -> Self {
        Self {
            length: 20,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
        }
    }
}

/// Generates a random password. The result is wrapped in [`Zeroizing`] so it is
/// wiped when dropped.
pub fn generate_password(opts: &PasswordOptions) -> Result<Zeroizing<String>> {
    if opts.length == 0 || opts.length > 1024 {
        return Err(AppError::Validation(
            "password length must be between 1 and 1024".to_string(),
        ));
    }
    let mut charset = String::new();
    if opts.uppercase {
        charset.push_str(UPPER);
    }
    if opts.lowercase {
        charset.push_str(LOWER);
    }
    if opts.digits {
        charset.push_str(DIGITS);
    }
    if opts.symbols {
        charset.push_str(SYMBOLS);
    }
    if charset.is_empty() {
        return Err(AppError::Validation(
            "at least one character set must be enabled".to_string(),
        ));
    }
    let bytes = charset.as_bytes();
    let mut rng = rand::rng();
    let password: String = (0..opts.length)
        .map(|_| bytes[rng.random_range(0..bytes.len())] as char)
        .collect();
    Ok(Zeroizing::new(password))
}
