# Where Lilypad keeps your data

The desktop app, the TUI and the CLI share one data folder, so a vault created
in one of them shows up in the other two.

## The default folder

| System | Folder |
|---|---|
| Linux | `~/.config/Colony/Lilypad` |
| macOS | `~/Library/Application Support/Colony/Lilypad` |
| Windows | `%APPDATA%\Colony\Lilypad\config` |

Inside it:

```
vaults/<name>.lily                 one encrypted file per vault
backups/<name>_<timestamp>.backup  safety copies of a vault, as encrypted as the vault itself
sync/<name>.json                   the sync baseline of a vault (see sync.md)
locks/                             lock files, so two programs never write a vault at once
settings.json                      preferences shared by the three programs
oauth_tokens.json                  the GitHub sign-in token, once you sign in to sync
key.json                           only for vaults from the oldest releases (see below)
```

Lilypad takes a safety backup before it deletes a vault, changes a master
password, restores a backup or applies a synced copy. `lilypad-cli backup`,
`list-backups`, `restore-backup` and `prune-backups` manage them by hand.

`key.json` holds the key parameters of vaults written in the first vault
format (V1). Newer vaults carry their parameters inside the file, so a
data folder that only holds new vaults has no `key.json`.

## Moving the vaults elsewhere

Two overrides move the vaults, the backups, the sync baselines and
`settings.json` to another folder:

- the `LILYPAD_DATA_DIR` environment variable, read by all three programs;
- `--data-dir <DIR>`, a CLI option that wins over the variable.

The GitHub sign-in token does not move: it stays in the default folder, so
one sign-in serves every data folder.

## Vaults from older CLI builds

CLI builds older than the shared service layer kept their vaults in
`./.lilypad`, relative to the folder the CLI was started from. Lilypad cannot
find those on its own. When the data folder holds no vault and
`./.lilypad/vaults` does, the CLI prints a hint; open them with:

```bash
lilypad-cli --data-dir .lilypad vaults
```

## What a vault file reveals

A vault file is encrypted as a whole with a key derived from its master
password (Argon2id, then XChaCha20-Poly1305), and each entry's secrets
(password, notes, TOTP secret, backup codes, attachments) are encrypted a
second time inside it. Outside the encryption, the file carries only its
format version, a key identifier, the algorithm names, the key derivation
parameters and salt, a checksum of the ciphertext and an encrypted password
verifier. Without the master password, someone holding the file learns the
vault's name (the file name), its size and when it changed.
