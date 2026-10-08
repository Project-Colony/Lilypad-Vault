# Lilypad

Lilypad is a local-first password manager written in Rust, part of
[Project Colony](https://github.com/Project-Colony). It comes as three programs
that share one service layer, so they read and write vaults the same way:

- **Lilypad** (`lilypad-desktop`), the desktop app, built with iced;
- **Lilypad TUI** (`lilypad-tui`), a keyboard-driven terminal interface;
- **Lilypad CLI** (`lilypad-cli`), for scripts and the shell.

## What it does

- Several vaults, each with its own master password (Argon2id key derivation,
  XChaCha20-Poly1305 encryption).
- Logins, notes, cards and other entry types, with tags, folders, favourites,
  colour labels, password expiry, change history and a Trash.
- Password generator, TOTP codes and TOTP backup codes.
- Password health report (weak, reused, old, expiring), and an optional check
  against Have I Been Pwned that never sends a password (see the privacy policy
  below).
- Import from Lilypad, LastPass, Bitwarden (CSV and JSON), KeePassXC,
  1Password, Safari, Chrome and Edge, Firefox, Proton Pass and Dashlane, with
  the format detected from the file's content. Export to CSV.
- Optional sync of vaults through a private repository on your own GitHub
  account, with an entry-by-entry merge when two devices both changed a vault.
- Safety backups before deleting a vault, changing a master password,
  restoring a backup and applying a synced copy; auto-lock; a clipboard that
  clears itself.

## Install

From [Colony](https://github.com/Project-Colony/Colony), the Project Colony
app store, or from the [releases page](https://github.com/Project-Colony/Lilypad-Vault/releases/latest):

| Platform | Desktop app | CLI | TUI |
| --- | --- | --- | --- |
| Linux (x86_64) | `lilypad-linux` | `lilypad-cli-linux` | `lilypad-tui-linux` |
| Windows (x86_64) | `lilypad-windows.exe` | `lilypad-cli-windows.exe` | `lilypad-tui-windows.exe` |
| macOS (Apple Silicon) | `lilypad-macos` | `lilypad-cli-macos` | `lilypad-tui-macos` |
| macOS (Intel) | `lilypad-macos-x86` | `lilypad-cli-macos-x86` | `lilypad-tui-macos-x86` |

Every file has a detached ed25519 signature (`.sig`) and a signed metadata
sidecar (`.meta`, `.meta.sig`) made with the Project Colony release key, which
Colony checks before installing.

## Where your data lives

Vaults, settings and the GitHub sign-in token live in one directory:
`~/.config/Colony/Lilypad` on Linux, `~/Library/Application Support/Colony/Lilypad`
on macOS and `%APPDATA%\Colony\Lilypad\config` on Windows. The
`LILYPAD_DATA_DIR` environment variable (all three programs) or `--data-dir`
(CLI) moves the vaults and settings elsewhere; the GitHub token stays in the
platform directory.

A vault file is encrypted as a whole with a key derived from its master
password (Argon2id, then XChaCha20-Poly1305), and each entry's secrets
(password, notes, TOTP secret, backup codes, attachments) are encrypted a
second time inside it. Outside the encryption, the file carries only its
format version, a key identifier, the algorithm names, the key derivation
parameters and salt, a checksum of the ciphertext and an encrypted
password verifier. What someone holding the file can learn without the master
password is the vault's name (the file name), its size and when it changed.

## Build from source

Rust 1.89 or newer and a C toolchain for your platform. On Linux, also
`pkg-config` and the OpenSSL development package (`libssl-dev` on Debian and
Ubuntu, `openssl` on Arch, `openssl-devel` on Fedora): HTTPS goes through the
system OpenSSL.

```bash
cargo build --release -p lilypad-desktop -p lilypad-cli -p lilypad-tui
cargo test --workspace
```

GitHub sync needs the client ID of a GitHub OAuth App with device flow
enabled, compiled in through the `LILYPAD_GITHUB_CLIENT_ID` environment
variable at build time. Without it, everything else works and sync says that
no client ID was compiled in.

## Development

The workspace has eight crates: `lilypad-core` (cryptography and models),
`lilypad-storage` (vault files, backups, locking), `lilypad-common`
(validation, health checks, search), `lilypad-oauth` (GitHub sign-in and the
GitHub API), `lilypad-app` (the service layer the three programs share),
`lilypad-cli`, `ui/tui` and `ui/desktop`. See [`doc/doc.md`](doc/doc.md) for
the architecture.

Code, comments, documentation and commit messages are in English. Commits
follow [Conventional Commits](https://www.conventionalcommits.org/): releases,
their version and their changelog are made by release-please from them.

## Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

Windows builds are Authenticode-signed this way once the SignPath Foundation
has accepted the project; until then they ship without Authenticode. Every
release asset, on every platform, is always signed with the Project-Colony
ed25519 release key, and Colony verifies that signature before installing.

Team roles and members:

- Committers and reviewers: [MotherSphere](https://github.com/MotherSphere)
- Approvers: [MotherSphere](https://github.com/MotherSphere)

### Privacy policy

Lilypad has no telemetry, no analytics, no account of its own and no update
check. It contacts two networked services, each only when you ask it to.

- **GitHub, for sync.** Signing in (`lilypad-cli login`, or Settings > Sync in
  the desktop app) uses GitHub's OAuth device flow: Lilypad shows a code, you
  approve it on github.com, and GitHub returns a token with the `repo` and
  `read:user` scopes. `repo` grants access to all your repositories, because
  GitHub OAuth Apps have no narrower scope for a single private repository;
  Lilypad only uses it on its own vault repository. The token is stored
  in `oauth_tokens.json` in the platform directory above, readable only by
  your account on Linux and macOS. It is scrambled with a key derived from the
  machine's host name and your user name, not from a master password, so
  treat that file as you would the token itself. On push, pull,
  merge or status, Lilypad talks to `api.github.com` to find or create a
  private repository named `lilypad-vault-<your GitHub username>` and to read or
  write the vault files in it. What it uploads is the encrypted vault file,
  as `vaults/<vault name>.lily`: GitHub sees the vault's name, its size and
  when it changed, not its contents (see
  [Where your data lives](#where-your-data-lives)).
  Nothing is sent to GitHub until you sign in, and nothing after `logout`.
- **Have I Been Pwned, for the breach check.** Only when you run it
  (`lilypad-cli breach-check`, or the button in the desktop app's vault health
  settings). Lilypad hashes each password with SHA-1 on your machine and sends
  only the first five characters of each hash to
  `https://api.pwnedpasswords.com/range/`, with padding enabled. The service
  never receives a password or a full hash, and the answer is compared
  locally.

Opening an entry's URL hands it to your default browser; Lilypad itself does
not fetch it. Like any HTTPS request, both services see your IP address.

## License

Lilypad is licensed under the **GNU General Public License, version 3 or (at
your option) any later version** (`GPL-3.0-or-later`). The full text is in
[`LICENSE`](LICENSE).

Contributions are accepted under the same terms: by submitting a patch you
agree that it may be distributed under GPL-3.0-or-later.

The bundled JetBrains Mono fonts under `ui/Assets/Fonts/` and
`ui/desktop/assets/fonts/` are third-party assets distributed under the SIL
Open Font License 1.1 and are not covered by the GPL.
