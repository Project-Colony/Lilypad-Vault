# lilypad-cli

Interface en ligne de commande pour interagir avec Lilypad.

## Usage

```bash
cargo run -p lilypad-cli -- --help
```

## Commandes

```bash
# Initialiser un coffre et générer une clé locale
cargo run -p lilypad-cli -- init primary

# Ajouter une entrée chiffrée
cargo run -p lilypad-cli -- add primary email "mot-de-passe"

# Lister les entrées
cargo run -p lilypad-cli -- list primary

# Lire une entrée
cargo run -p lilypad-cli -- get primary email
```

## Options

```bash
# Changer le dossier de stockage (par défaut: .lilypad)
cargo run -p lilypad-cli -- --data-dir .data init primary
```
