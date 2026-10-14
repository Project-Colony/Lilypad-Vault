<div align="center">

# Lilypad

**Local-first password manager for desktop, terminal and scripts.**

</div>

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Colony app](https://img.shields.io/badge/Colony-security-purple)](https://github.com/Project-Colony/Colony)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20windows%20%7C%20macOS-lightgrey)](#installation)

Most password managers either keep your vault on their servers behind an
account, or keep it on one machine and leave copying it around to you.
Lilypad keeps each vault as one encrypted file on your own machine, opens it
from a desktop app, a terminal interface or the command line, and can sync it
through a private repository on your own GitHub account. Nothing leaves the
machine unless you ask for it.

> **Status:** releases are signed and published for Linux, Windows and macOS
> (Apple Silicon and Intel), and CI builds and tests every change on all three
> systems. Vaults written by earlier releases are kept as test fixtures and
> must keep opening byte for byte. The one-time move of your files from the
> folder of releases up to 0.2 is tested on all three systems, including the
> rollback when a step fails. GitHub sync works in the released builds,
> which carry Lilypad's GitHub client ID; a build from source has no sync
> until you compile in a client ID of your own. Sync is covered by unit tests
> for its safety checks, while the network round trip itself is only tested
> by hand. Windows builds are not Authenticode-signed yet, so SmartScreen may
> warn on first launch.

## Why Lilypad

- **Your vault is a file you own.** No Lilypad account and no Lilypad server:
  a vault is `vaults/<name>.lily` in your data folder, encrypted with a key
  derived from its master password (Argon2id, then XChaCha20-Poly1305).
- **Three programs, one behaviour.** The desktop app, the TUI and the CLI are
  thin frontends over one shared library, so a vault edited in the terminal
  looks the same in the desktop app, and scripts get the same checks as the
  interface.
- **Sync without a sync service.** Vaults travel through a private GitHub
  repository on your account. GitHub stores only the encrypted file, and a
  pulled copy has to decrypt with your master password before it may replace
  the local one.
- **Nothing in the background.** No telemetry, no analytics and no update
  check. The only network calls are sync and the breach check, and both run
  only when you start them.

## What it does

- Three programs over one library: **Lilypad** (`lilypad`), the desktop app;
  **Lilypad TUI** (`lilypad-tui`), a keyboard-driven terminal interface; and
  **Lilypad CLI** (`lilypad-cli`), for scripts and the shell.
- Several vaults, each with its own master password.
- Logins, notes, cards and other entry types, with tags, folders, favourites,
  colour labels, password expiry, change history and a Trash.
- Password generator, TOTP codes and TOTP backup codes.
- Password health report (weak, reused, old, expiring), and an optional check
  against Have I Been Pwned that never sends a password.
- Import from Lilypad, LastPass, Bitwarden (CSV and JSON), KeePassXC,
  1Password, Safari, Chrome and Edge, Firefox, Proton Pass and Dashlane, with
  the format detected from the file's content. Export to CSV.
- Optional sync of vaults through a private repository on your own GitHub
  account, with an entry-by-entry merge when two devices both changed a vault.
- Safety backups before deleting a vault, changing a master password,
  restoring a backup and applying a synced copy; auto-lock; a clipboard that
  clears itself.
- The desktop app and the TUI in English and French. The desktop app uses the
  Project Colony themes and accent colours, and honours text size, high
  contrast, a dyslexia-friendly font and reduced motion; click the Lilypad
  name in its sidebar, or press Ctrl+, (Cmd+, on macOS), to open Preferences.

## Installation

### Via Colony (recommended)

Search for **Lilypad** in [Colony](https://github.com/Project-Colony/Colony)
and install it. Colony installs the desktop app, checks its signature, and
offers each new release as an update.

### Direct binary download

Grab the files for your platform from the
[latest release](https://github.com/Project-Colony/Lilypad-Vault/releases/latest).
Each platform has three programs:

| Platform | Desktop app | TUI | CLI |
|---|---|---|---|
| Linux (x86_64) | `lilypad-vault-linux` | `lilypad-tui-linux` | `lilypad-cli-linux` |
| Windows (x86_64) | `lilypad-vault-windows.exe` | `lilypad-tui-windows.exe` | `lilypad-cli-windows.exe` |
| macOS (Apple Silicon) | `lilypad-vault-macos` | `lilypad-tui-macos` | `lilypad-cli-macos` |
| macOS (Intel) | `lilypad-vault-macos-x86` | `lilypad-tui-macos-x86` | `lilypad-cli-macos-x86` |

```bash
chmod +x lilypad-vault-linux && ./lilypad-vault-linux
```

The Linux builds need glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36
or later). Check a download before running it with the commands under
[Code signing policy](#code-signing-policy).

### Build from source

```bash
git clone https://github.com/Project-Colony/Lilypad-Vault
cd Lilypad-Vault
cargo build --release
```

Requires Rust 1.89 or newer and a C compiler. Nothing else: HTTPS goes
through rustls with the operating system's certificate store, so no system
OpenSSL is needed. The three programs land in `target/release/`.

GitHub sync needs the client ID of a GitHub OAuth App with device flow
enabled, compiled in through the `LILYPAD_GITHUB_CLIENT_ID` environment
variable at build time. Without it everything else works, and sync says that
no client ID was compiled in.

## Documentation

Full documentation is in [docs/](docs/). Start at
[docs/README.md](docs/README.md).

## Code signing policy

Every release asset is signed by the Project Colony organisation in CI, never
on a developer machine. Next to each asset on the release page:

| File | What it is |
|---|---|
| `<asset>.sig` | an ed25519 signature over the asset, made with the organisation's release key |
| `<asset>.meta` | three lines binding the asset to its file name, its sha256 and the release version |
| `<asset>.meta.sig` | an ed25519 signature over the `.meta` |

The private key is an organisation secret, used only by the shared
[sign-and-publish workflow](https://github.com/Project-Colony/Project-Colony-Resources/blob/main/.github/workflows/sign-and-publish.yml)
in a job that builds nothing; the jobs that compile Lilypad never see it.
Colony checks all three files before it installs or updates Lilypad, and
refuses a release older than the one installed. To check a download yourself
with OpenSSL 3 (the same commands work for every asset):

```bash
cat > colony-release.pub <<'EOF'
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEARNjg3Nn8H6/aBg1unwGjkUTcrdTxERNefVaqU8cFu0s=
-----END PUBLIC KEY-----
EOF
a=lilypad-vault-linux
openssl pkeyutl -verify -pubin -inkey colony-release.pub -rawin -in "$a" -sigfile "$a.sig"
openssl pkeyutl -verify -pubin -inkey colony-release.pub -rawin -in "$a.meta" -sigfile "$a.meta.sig"
cat "$a.meta"     # version=<tag>, asset=<file name>, sha256=<digest>
sha256sum "$a"    # the digest must equal the sha256 line
```

How releases are built, signed and published:
[design/releases.md](https://github.com/Project-Colony/Project-Colony-Resources/blob/main/design/releases.md#5-signing).

<!--
Add the following only once SignPath signs the Windows build, that is once
`signpath-project-slug` is set in .github/workflows/release.yml. Before that it
would promise a signature the .exe does not carry.

Windows releases are also Authenticode-signed. Free code signing provided by
[SignPath.io](https://about.signpath.io/), certificate by
[SignPath Foundation](https://signpath.org/).

- Committers and reviewers: [MotherSphere](https://github.com/MotherSphere)
- Approvers: [MotherSphere](https://github.com/MotherSphere)
-->

## Privacy

Lilypad sends no telemetry, no analytics and no crash reports, has no account
of its own and never checks for updates.

| Data | Stored or sent | Where, and why |
|---|---|---|
| Vaults, backups, sync state | stored | `vaults/`, `backups/` and `sync/` in the data folder; vaults and backups are encrypted with the master password, and each vault carries its own audit log inside the encryption |
| Preferences | stored | `preferences.json` in the preferences folder |
| GitHub sign-in token | stored | `oauth_tokens.json` in the preferences folder, readable only by your account on Linux and macOS; scrambled with a key derived from the machine's host name and your user name, not from a master password, so treat the file as you would the token |
| Device code and token requests | sent, when you sign in to sync | `github.com`, through GitHub's OAuth device flow |
| Encrypted vault file and its name | sent, when you push or merge | `api.github.com`, as `vaults/<vault name>.lily` in a private repository named `lilypad-vault-<your GitHub username>` that Lilypad creates on your account |
| First five characters of each password's SHA-1 hash | sent, when you run the breach check | `api.pwnedpasswords.com`, with padding enabled; the answer is compared on your machine |

The preferences folder is `~/.config/Colony/Lilypad` on Linux and the data
folder `~/.local/share/Colony/Lilypad`; on Windows both are
`%LOCALAPPDATA%\Colony\Lilypad`, and on macOS both are
`~/Library/Application Support/Colony/Lilypad`. `LILYPAD_DATA_DIR`, or
`--data-dir` for the CLI, keeps the whole profile in another folder instead,
GitHub sign-in included. Releases up to 0.2 kept everything in one folder,
which the first start of a newer release moves: see
[where Lilypad keeps your data](docs/guide/data-location.md).

Lilypad connects to no server except the ones in this table, and only for the
action named there. GitHub sees the vault's name, its size and when it
changed, never its contents. The token carries the `repo` and `read:user`
scopes; `repo` grants access to all your repositories, because GitHub OAuth
Apps have no narrower scope for a single private repository, and Lilypad
only uses it on its own vault repository. Nothing is sent to GitHub before you
sign in, and nothing after you sign out. Like any HTTPS request, both
services see your IP address: see the
[GitHub privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement)
and the [Have I Been Pwned privacy policy](https://haveibeenpwned.com/Privacy).
Opening an entry's URL hands it to your default browser; Lilypad itself does
not fetch it.

How sync works, step by step: [docs/guide/sync.md](docs/guide/sync.md).

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). By submitting a patch you agree that
it is distributed under the same terms.

The desktop app embeds the Project Colony fonts (JetBrains Mono Nerd Font,
OpenDyslexic and Font Awesome 6 Free) through the
[`colony-ui`](https://crates.io/crates/colony-ui) crate. They are third-party
assets under their own licences (the SIL Open Font License 1.1 for the
typefaces), shipped with their licence texts in that crate, and are not
covered by the GPL.
