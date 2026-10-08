# Security Policy

Lilypad is a password manager: a flaw in it can expose every secret a user
keeps in it. Security reports are handled before any other work.

## Supported versions

Only the **latest release** receives security fixes. Colony, the Project Colony
app store, offers each new release as an update, so staying current is one
update away.

| Version        | Supported |
| -------------- | --------- |
| latest release | Yes       |
| anything older | No        |

## Reporting a vulnerability

Report vulnerabilities **privately** through
[GitHub Security Advisories](https://github.com/Project-Colony/Lilypad-Vault/security/advisories/new)
("Report a vulnerability"). Do not open a public issue for an exploitable bug,
and do not attach a real vault or real credentials to a report.

What to expect:

- **Acknowledgement** within a few days.
- A fix, or a mitigation plan, before any public disclosure, coordinated with
  you. Releases are automated, so a patched release usually ships as soon as
  the fix lands.
- Credit in the release notes if you want it.

## Scope

Reports of particular interest:

- **Vault encryption**: anything that recovers vault contents without the
  master password or key file, weakens the Argon2id key derivation, reuses an
  XChaCha20-Poly1305 nonce, or lets a modified vault file open without an
  error.
- **Secrets in memory and on screen**: master passwords or entry secrets that
  reach logs, crash output, temporary files or the clipboard for longer than
  the configured clearing delay.
- **Untrusted input**: import files (CSV and JSON exports from other password
  managers) and vault files received through sync are parsed as untrusted.
  A crafted file that crashes Lilypad, corrupts an existing vault or replaces
  it without the right password is in scope.
- **GitHub sync**: anything that sends unencrypted vault data, or the sign-in
  token, anywhere other than `api.github.com`, or that lets a remote copy
  replace a local vault without first being decrypted with that vault's key.
- **Network privacy**: the breach check sending more than the first five
  characters of a SHA-1 hash, or any network request Lilypad makes without
  the user asking for it (see the privacy policy in the README).
- **Release integrity**: every release asset carries an ed25519 signature
  (`.sig`) and a signed metadata sidecar (`.meta`, `.meta.sig`). A way to ship
  a binary that passes those checks without the Project Colony release key is
  critical.

Known and documented, so not a vulnerability on its own: the GitHub sign-in
token in `oauth_tokens.json` is scrambled with a key derived from the host
name and user name, not from a master password, and the `repo` scope it
carries covers all of the user's repositories (the README explains both).
Anyone who can already read the user's files or run code as that user is out
of scope.
