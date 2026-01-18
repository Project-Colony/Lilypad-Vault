use anyhow::Result;
use lilypad_core::default_config;
use lilypad_storage::LocalStore;

fn main() -> Result<()> {
    let config = default_config();
    let store = LocalStore::new(&config)?;
    let status = store.status();

    println!(
        "Lilypad CLI ready (env: {}, data dir: {}).",
        config.environment, status.root
    );

    Ok(())
}
