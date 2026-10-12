# Architecture

Lilypad ships three programs, the desktop app, the TUI and the CLI, over one
library that knows everything about vaults. The programs only collect input
and render results; every decision about a vault (unlocking, writing,
locking, syncing, clipboard policy) is made in the library, so the three can
never disagree about how a vault is read or written.

```
crates/
├── lilypad/        the desktop app (iced), binary `lilypad`
├── lilypad-tui/    the terminal interface (ratatui), binary `lilypad-tui`
├── lilypad-cli/    the command line (clap), binary `lilypad-cli`
└── lilypad-core/   the library the three share
```

Each binary has its own `build.rs`, which writes the Windows version resource
(product name `Lilypad`, the workspace version) that code signing requires.

## Data flow

```
 lilypad / lilypad-tui / lilypad-cli
        │  App::open, App::unlock, add_entry, sync::net::push, ...
        ▼
 lilypad-core service layer ── AppError (one typed error)
        │                 │
        ▼                 ▼
     storage            oauth ──► api.github.com
   (vault files,     (device-flow sign-in,
    backups, keys)    token store, GitHub API)
        │
        ▼
   models + crypto
 (Vault, Entry, Argon2id, XChaCha20-Poly1305)
```

A frontend opens an `App` for a data directory, unlocks a vault into a
`Session` and passes that session to the operation it wants. Unlocking only
reads: a wrong master password can never overwrite a vault. Every write is a
read-modify-write under an exclusive per-vault lock (`locking`), so two
programs working on the same vault cannot lose each other's changes.

## lilypad-core

| Module | What lives there |
|---|---|
| `models/` | `Vault` and its merge logic (`vault.rs`), `Entry` with metadata, colours, custom fields and history (`entry.rs`), the encrypted `EntrySecret` with TOTP backup codes and attachments (`secret.rs`), key metadata (`keys.rs`) |
| `crypto` | Argon2id key derivation and XChaCha20-Poly1305 encryption |
| `error` | `CoreError` for the models and the crypto, `AppError` for everything the frontends call |
| `storage` | `LocalStore`: the `.lily` file format, atomic writes, key files, backups, the audit log |
| `oauth/` | GitHub device-flow sign-in (`device_flow.rs`), the token store, the GitHub API client and the sync backend that stores a vault in a private repository |
| `sync/` | validated pull, push and entry-level merge over `oauth` (`net.rs`), and the per-vault sync state (`state.rs`) |
| `vault` | `App` and `OpenOptions`: opening a data directory, creating, unlocking, renaming and deleting vaults |
| `session` | `Session`: an unlocked vault and its key, with the idle auto-lock policy |
| `entries` | every entry operation: add, edit (field-preserving), tags, folders, Trash, history, backup codes |
| `secret` | the one place an `EntrySecret` is encrypted or decrypted, with zeroizing guards |
| `password` | master-password normalization (trim, then Unicode NFC), applied once before every key derivation |
| `crypto_ops` | changing a master password |
| `locking` | the cross-process per-vault lock |
| `management` | backups: create, list, restore, prune |
| `import` | the importer (format detected from content) and the CSV export |
| `breach` | the Have I Been Pwned k-anonymity check |
| `health` | the password health analysis and the report for an unlocked vault |
| `search` | the fuzzy and advanced search |
| `validation` | vault names, entry sizes, password strength |
| `generator`, `totp` | the password generator and TOTP codes |
| `keyfile`, `clipboard`, `time` | the legacy `key.json` key file, the clipboard, timestamp formatting |
| `i18n` | the interface strings of the desktop app and the TUI (see below) |
| `paths` | the config and data directories, and the one-time move from where earlier releases kept their files |
| `settings` | `preferences.json`: the Colony standard preferences plus Lilypad's own, shared by every frontend |

### Where files live

`paths` resolves two roots with `colony_ui::paths`: the config root holds
`preferences.json` and `oauth_tokens.json`; the data root holds `vaults/`,
`backups/`, `sync/`, `locks/` and the legacy `key.json`. They are the same
directory on Windows and macOS, so no file name is used in both.
`--data-dir` or `LILYPAD_DATA_DIR` replaces both roots with one directory.

Releases up to 0.2 wrote everything to
`ProjectDirs::from_path("Colony/Lilypad").config_dir()`, the Roaming profile
on Windows. The first `App::open` of a process moves what it finds there:
each entry is renamed (or copied and checked byte for byte across volumes)
only if its destination is absent, `settings.json` is converted to
`preferences.json` (the old theme index maps to a Colony theme), and a
`.migrated` marker in the data root makes later starts skip the check. An
exclusive lock on `locks/migrate.lock` keeps the desktop app, the CLI and the
TUI from migrating at the same time. On any error every step is undone and
that run uses the old directory, so nobody starts on an empty profile. The
old directory is never deleted.

### The vault file

A vault is `vaults/<name>.lily` in the data directory. The file starts with a
`LILYPAD_VAULT_V2` header line and a comment saying the file is an encrypted
vault, followed by JSON: the format version, the key metadata (key identifier,
algorithm and, from V2, the Argon2id parameters and salt), a checksum of the
ciphertext, an encrypted password verifier and the ciphertext itself. The
ciphertext holds the whole vault (name, entries, metadata); each entry's
secrets are encrypted a second time inside it. A V1 vault has no embedded key
parameters and needs the `key.json` key file beside it. Legacy `.json` vault
files are still read; new vaults are always written as `.lily`.

### Interface strings

`config/i18n/en.json` and `fr.json` hold every string the desktop app and the
TUI show, embedded at build time. `i18n::t` looks a key up in the active
locale, then in English, then in the labels every Colony program shares
(`colony_ui::i18n`: the Preferences skeleton, themes, accents). The locale is
colony-ui's, set once at startup from the `language` preference or the
system's. Errors shown in a window go through `AppError::localized`; the CLI
prints `Display`, which stays English. Tests check that both files have the
same keys and placeholders, that every key the code asks for exists and that
every key is used.

## lilypad (desktop)

```
src/
├── main.rs        entry point: --version, logging, fonts, the iced application
├── state.rs       LilypadApp, the screens and overlays, shared helpers
├── message.rs     every message the app handles
├── update.rs      the update loop and the background tasks (Argon2, sync)
├── view/          mod.rs (shell, notices, dialogs), sidebar.rs, unlock.rs, vault.rs, entry.rs
├── preferences/   mod.rs (page, General, Appearance, About), security.rs, vault.rs, sync.rs
├── theme.rs       widget styles and the interface density
└── icons.rs       the Nerd Font glyphs it draws
```

The app is built on `colony-ui`, the Project Colony design system: every
colour comes from the active Colony palette (theme, accent, high contrast),
every font from its accessors (OpenDyslexic when the dyslexia font is on),
every text size through `sz()` (font size times reading size), and Appearance
and Accessibility are the kit's own sections. iced fixes its default font
when the window opens, so every text names its font, which is how the
dyslexia font reaches all of it without a restart. The Lilypad name at the top
of the sidebar, or Ctrl+, (Cmd+, on macOS), opens and closes Preferences,
which replaces the content area while the sidebar stays. Tests reject a
hand-written colour, a raw text size or a text without a font in `src/`.

Argon2 and every network call run off the UI thread (`Task` plus
`spawn_blocking`). Buffers holding secrets (form fields, master-password
fields, the sync password) are zeroized on drop, on lock and when
Preferences closes.

## lilypad-tui

`main.rs` sets up the terminal and runs the event loop; `app.rs` holds the
state, `input.rs` the keyboard handling of every screen and `ui.rs` the
rendering. It follows the same `language` preference as the desktop app.

## lilypad-cli

`main.rs` parses the arguments and dispatches; `args.rs` declares every flag
and subcommand; `cmd.rs` implements the commands and `io.rs` the prompts and
output. The CLI output is English only: it is a scripting interface.

CLI builds older than the shared service layer kept vaults in `./.lilypad`,
relative to the working directory. Lilypad cannot move those automatically;
when the data directory is empty and `./.lilypad/vaults` holds vaults, the CLI
prints a hint to open them with `--data-dir .lilypad`.

## Tests

- Unit tests sit next to the code they test.
- `crates/lilypad-core/tests/` holds the integration tests: vaults written
  before the RustCrypto upgrade must keep opening byte for byte
  (`pre_crypto_upgrade_vaults.rs`, with fixtures), the store round-trip, the
  OAuth configuration seen from outside the crate, and the interface strings
  against the code of every crate (`i18n_keys.rs`).
- `crates/lilypad-cli/tests/integration.rs` drives the CLI binary end to end.
- The desktop and TUI test their state machines without a window or a
  terminal (`crates/lilypad/src/tests.rs`, `crates/lilypad-tui/src/tests.rs`).
- `cargo bench -p lilypad-core` measures key derivation and encryption.
