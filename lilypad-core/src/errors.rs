use std::fmt;

#[derive(Debug)]
pub enum CoreError {
    InvalidKeyLength { expected: usize, actual: usize },
    Crypto(String),
    Serialization(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKeyLength { expected, actual } => write!(
                f,
                "invalid key length: expected {expected} bytes, got {actual} bytes"
            ),
            Self::Crypto(message) => write!(f, "crypto error: {message}"),
            Self::Serialization(message) => write!(f, "serialization error: {message}"),
        }
    }
}

impl std::error::Error for CoreError {}

pub type Result<T> = std::result::Result<T, CoreError>;
