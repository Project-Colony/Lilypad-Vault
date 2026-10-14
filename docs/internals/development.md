# Development

How to build, run and test Lilypad, what CI checks, and how a release gets
out. How to open a pull request is in [CONTRIBUTING.md](../../CONTRIBUTING.md).

## Toolchain

Rust 1.89 or newer (`rust-version` in `Cargo.toml`, checked by the MSRV job)
and a C compiler, which builds aws-lc, the cryptography behind rustls. No
system library is needed on any platform.

## The dev loop

```bash
cargo run -p lilypad                    # the desktop app
cargo run -p lilypad-tui                # the TUI
cargo run -p lilypad-cli -- --help      # the CLI
```

Point a development run at a throwaway data folder so it never touches your
real vaults:

```bash
LILYPAD_DATA_DIR=/tmp/lilypad-dev cargo run -p lilypad
```

`LILYPAD_DATA_DIR` holds the whole profile, preferences and GitHub sign-in
included (see [data-location.md](../guide/data-location.md)), so a
development run starts signed out of GitHub and never reads or changes your
real sign-in. The CLI also takes `--data-dir`.

Sync needs a GitHub OAuth App client ID compiled in. Set
`LILYPAD_GITHUB_CLIENT_ID` when you build, to the client ID of an OAuth App of
your own with device flow enabled. It is read at compile time only; the
variable means nothing to a running program.

## Tests

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Unit tests sit next to the code they test, integration tests in each crate's
`tests/`. Two suites guard compatibility and deserve care:

- the vaults under `crates/lilypad-core/tests/fixtures/pre-crypto-upgrade/`
  were written before the RustCrypto upgrade, and must keep opening with the
  same content;
- the CLI integration tests drive the real `lilypad-cli` binary end to end.

The interface strings are checked too: `en.json` and `fr.json` in
`config/i18n/` must have the same keys and placeholders (unit tests in
`crates/lilypad-core/src/i18n.rs`), and `crates/lilypad-core/tests/i18n_keys.rs`
fails when the code asks for a key that does not exist or a key goes unused.

The network side of sync (sign-in, push, pull against GitHub) has no
automated test: it needs real credentials. The safety logic around it
(validate before overwrite, the sync baseline, the entry merge) is unit
tested. Never commit a real vault, password or token, including in fixtures.

`cargo bench -p lilypad-core` measures key derivation and encryption.

## What CI checks

Every pull request runs, and must pass, before it can merge:

| Job | What it runs |
|---|---|
| Check | `cargo check --workspace` |
| Build All Binaries | `cargo build --workspace --all-targets` |
| Test (Linux, macOS, Windows) | `cargo test --workspace`; on Linux the OAuth tests again with a client ID compiled in; on Windows a check of each `.exe`'s version resource |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format | `cargo fmt --all -- --check` |
| MSRV | `cargo check --workspace --all-targets --locked` on Rust 1.89 |
| Dependency Check | `cargo deny check` against `deny.toml`: licences, advisories, sources |
| Security Audit | RustSec advisories |
| Code Coverage | `cargo llvm-cov`, uploaded to Codecov |

Warnings are errors in every job (`RUSTFLAGS=-Dwarnings`).

## Dependencies

Every dependency is pinned in `[workspace.dependencies]` of the root
`Cargo.toml` to its full latest version, and the crates refer to it with
`workspace = true`. Dependabot proposes updates for Cargo and GitHub Actions
every week; actions are pinned to commit SHAs, with the version in a comment.
A new licence in the dependency tree fails the Dependency Check until it is
added to `deny.toml`, with a comment naming the crate that brings it.

## Releases

Releases are made by [release-please](https://github.com/googleapis/release-please),
never by hand:

1. Every merge to `main` updates an open release pull request: the next
   version, computed from the Conventional Commit titles since the last
   release, and the matching `CHANGELOG.md` entry. Merge it untouched; do not
   edit `CHANGELOG.md` or the version in `Cargo.toml` yourself.
2. Merging it tags `vX.Y.Z` and creates the release as a draft.
3. The release workflow builds the three programs for Linux, Windows, macOS
   (Apple Silicon) and macOS (Intel), runs each one's `--version`, checks the
   Windows version resources, and validates `colony.json` against the asset
   names with the Colony binary.
4. The shared sign-and-publish workflow of Project-Colony-Resources signs
   every asset (`.sig`, `.meta`, `.meta.sig`), verifies the signatures on the
   release, and only then publishes it. Colony offers it as an update from
   that moment.

If a release run fails after the tag exists, rerun it from the tag rather
than tagging again:

```bash
gh workflow run release.yml --ref vX.Y.Z -f tag=vX.Y.Z
```

How signing works and how to check a download:
[Code signing policy](../../README.md#code-signing-policy).
