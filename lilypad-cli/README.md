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
# Change the storage directory (default: .lilypad)
cargo run -p lilypad-cli -- --data-dir .data init primary
```
