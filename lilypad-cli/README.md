# lilypad-cli

Command-line interface for interacting with Lilypad.

## Usage

```bash
cargo run -p lilypad-cli -- --help
```

## Commands

```bash
# Initialize a vault and generate a local key
cargo run -p lilypad-cli -- init primary

# Add an encrypted entry
cargo run -p lilypad-cli -- add primary email "password"

# List entries
cargo run -p lilypad-cli -- list primary

# Read an entry
cargo run -p lilypad-cli -- get primary email
```

## Options

```bash
# Change the storage directory (default: $LILYPAD_DATA_DIR, else the platform
# Colony/Lilypad directory shared with the desktop app, e.g. ~/.config/Colony/Lilypad)
cargo run -p lilypad-cli -- --data-dir .data init primary

# Vaults made by older CLI builds live in ./.lilypad; open them with
cargo run -p lilypad-cli -- --data-dir .lilypad vaults
```
