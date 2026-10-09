# Contributing to Lilypad

Bug reports, fixes and improvements are welcome. Lilypad is a password
manager, so changes to cryptography, vault storage or sync get a careful
review.

## Security issues

Do not open a public issue or pull request for a vulnerability. Report it
privately as described in [`SECURITY.md`](SECURITY.md).

## Build and test

The toolchain and system packages are listed under
[Build from source](README.md#build-from-source) in the README. From the
repository root:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

CI runs the same commands on Linux, macOS and Windows, checks the minimum Rust
version (`rust-version` in `Cargo.toml`) and audits dependencies. A pull
request merges only when all of it is green.

To run a program during development:

```bash
cargo run -p lilypad-desktop
cargo run -p lilypad-tui
cargo run -p lilypad-cli -- --help
```

Never commit a real vault, password or token, including in test fixtures.

## Pull requests

- Open the pull request against `main`. Keep it to one change.
- The title is a [Conventional Commit](https://www.conventionalcommits.org/)
  that reads as the changelog entry: `fix: ...`, `feat: ...`, `docs: ...`,
  `chore: ...`, and so on. Pull requests are squash-merged, so the title
  becomes the commit on `main`, and release-please builds the version and the
  changelog from it.
- Do not edit `CHANGELOG.md` or the version in `Cargo.toml`: release-please
  writes both.
- Code, comments, documentation, commit messages and pull requests are in
  English.
- Add tests for new logic, and update the README or `docs/` in the same pull
  request when behaviour, paths or commands change.

By submitting a pull request you agree that your contribution is distributed
under [GPL-3.0-or-later](LICENSE).
