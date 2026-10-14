# Where Lilypad keeps your data

The desktop app, the TUI and the CLI share one profile, so a vault created in
one of them shows up in the other two. A profile lives in two folders: one for
what you chose (preferences, the GitHub sign-in) and one for what Lilypad
produced (vaults, backups, sync state).

## The default folders

| System | Preferences and GitHub sign-in | Vaults, backups and sync state |
|---|---|---|
| Linux | `~/.config/Colony/Lilypad` | `~/.local/share/Colony/Lilypad` |
| Windows | `%LOCALAPPDATA%\Colony\Lilypad` | `%LOCALAPPDATA%\Colony\Lilypad` |
| macOS | `~/Library/Application Support/Colony/Lilypad` | `~/Library/Application Support/Colony/Lilypad` |

On Windows and macOS both are the same folder; no file name appears in both,
so they never collide. On Linux, `$XDG_CONFIG_HOME` and `$XDG_DATA_HOME`
replace `~/.config` and `~/.local/share` when they are set.

In the preferences folder:

```
preferences.json                   preferences shared by the three programs
oauth_tokens.json                  the GitHub sign-in token, once you sign in to sync
```

In the data folder:

```
vaults/<name>.lily                 one encrypted file per vault
backups/<name>_<timestamp>.backup  safety copies of a vault, as encrypted as the vault itself
sync/<name>.json                   the sync baseline of a vault (see sync.md)
locks/                             lock files, so two programs never write a vault at once
key.json                           only for vaults from the oldest releases (see below)
.migrated                          written once the files of an older release have moved here
```

Lilypad takes a safety backup before it deletes a vault, changes a master
password, restores a backup or applies a synced copy. `lilypad-cli backup`,
`list-backups`, `restore-backup` and `prune-backups` manage them by hand.

`key.json` holds the key parameters of vaults written in the first vault
format (V1). Newer vaults carry their parameters inside the file, so a data
folder that only holds new vaults has no `key.json`.

## Keeping the profile somewhere else

Two overrides put the whole profile in one folder of your choice: vaults,
backups, sync state, `preferences.json` and the GitHub sign-in token.

- `--data-dir <DIR>`, a CLI option;
- the `LILYPAD_DATA_DIR` environment variable, read by all three programs.

The CLI option wins over the variable. Each folder has its own sign-in, so
signing in to sync with one profile does not sign in the others. The folder
is created if it does not exist; an empty value is refused rather than read
as the current folder.

## Moving from releases up to 0.2

Releases up to 0.2 kept everything in one folder:

| System | Folder |
|---|---|
| Linux | `~/.config/Colony/Lilypad` |
| Windows | `%APPDATA%\Colony\Lilypad\config` |
| macOS | `~/Library/Application Support/Colony/Lilypad` |

The first start of a newer release moves those files where they now belong
and turns `settings.json` into `preferences.json`:

- On Linux, the old folder becomes the preferences folder: `vaults/`,
  `backups/`, `sync/` and `key.json` move to `~/.local/share/Colony/Lilypad`,
  and `oauth_tokens.json` stays.
- On Windows, everything moves from the roaming `%APPDATA%` folder to
  `%LOCALAPPDATA%\Colony\Lilypad`.
- On macOS, the folder does not change; only `settings.json` is converted.

How the move keeps your vaults safe:

- An entry moves only if nothing of that name is at its destination yet, so
  the move never overwrites a file.
- A move that cannot be a rename (another drive) copies the entry, checks
  every copied file reads back the same, and leaves the original in place.
- If any step fails, every step already taken is undone and Lilypad keeps
  using the old folder for that run, so nothing goes missing. It tries again
  at the next start.
- Once everything has moved, Lilypad writes `.migrated` in the data folder
  and never checks the old folder again. A lock keeps the desktop app, the TUI
  and the CLI from moving the files at the same time.
- The old `settings.json` stays where it was, and so does anything that had to
  be copied rather than moved. Delete them by hand once you are happy with the
  new location.

A folder given with `--data-dir` or `LILYPAD_DATA_DIR` that an older release
used keeps its files where they are; only its `settings.json` is turned into
`preferences.json`.

## Vaults from older CLI builds

CLI builds older than the shared service layer kept their vaults in
`./.lilypad`, relative to the folder the CLI was started from. Lilypad does
not move those. When no override is set, the data folder holds no vault and
`./.lilypad/vaults` holds a `.lily` file, the CLI prints a hint; open them
with:

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
