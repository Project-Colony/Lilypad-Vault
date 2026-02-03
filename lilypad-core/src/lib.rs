pub mod crypto;
pub mod errors;
pub mod models;

pub use crypto::{
    decrypt, derive_key, encrypt, Ciphertext, CryptoAlgorithm, KeyDerivationParams, KeyMaterial,
};
pub use errors::{CoreError, Result};
pub use models::{
    Attachment, Entry, EntryChangeType, EntryHistoryRecord, EntryMetadata, EntrySecret, EntryType,
    KeyMetadata, Vault,
    // Size limit constants
    MAX_ATTACHMENT_SIZE, MAX_ATTACHMENTS_PER_ENTRY, MAX_FOLDER_LENGTH, MAX_LABEL_LENGTH,
    MAX_NOTES_SIZE, MAX_PASSWORD_SIZE, MAX_TAG_LENGTH, MAX_TAGS_PER_ENTRY,
    MAX_TOTAL_ATTACHMENTS_SIZE, MAX_URL_LENGTH, MAX_USERNAME_LENGTH,
};

mod config {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct AppConfig {
        pub environment: String,
        pub data_dir: String,
    }

    impl Default for AppConfig {
        fn default() -> Self {
            Self {
                environment: "development".to_string(),
                data_dir: ".lilypad".to_string(),
            }
        }
    }

    pub fn default_config() -> AppConfig {
        AppConfig::default()
    }

    #[cfg(test)]
    mod tests {
        use super::default_config;

        #[test]
        fn it_sets_defaults() {
            let config = default_config();
            assert_eq!(config.environment, "development");
            assert_eq!(config.data_dir, ".lilypad");
        }
    }
}

pub use config::{default_config, AppConfig};
