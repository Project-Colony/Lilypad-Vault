use anyhow::Result;
use lilypad_core::AppConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreStatus {
    pub root: String,
}

pub struct LocalStore {
    root: String,
}

impl LocalStore {
    pub fn new(config: &AppConfig) -> Result<Self> {
        Ok(Self {
            root: config.data_dir.clone(),
        })
    }

    pub fn status(&self) -> StoreStatus {
        StoreStatus {
            root: self.root.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LocalStore;
    use lilypad_core::AppConfig;

    #[test]
    fn it_reports_status() {
        let config = AppConfig::default();
        let store = LocalStore::new(&config).expect("store");
        let status = store.status();
        assert_eq!(status.root, ".lilypad");
    }
}
