# GitHub sync

Sync keeps a vault the same on several machines by storing its encrypted file
in a private repository on your own GitHub account. There is no Lilypad
server: Lilypad talks to GitHub directly, and only when you ask it to.

The desktop app and the CLI can sync; the TUI cannot. Sync needs a build that
carries Lilypad's GitHub client ID. The released builds do. A build from
source does only if `LILYPAD_GITHUB_CLIENT_ID` was set when it was compiled
(see [Build from source](../../README.md#build-from-source)).

## Signing in

Desktop: **Sign in with GitHub** under **Preferences > Sync** (click the
Lilypad name in the sidebar, or press Ctrl+, or Cmd+, on macOS, to open
Preferences). CLI: `lilypad-cli login`.

Lilypad uses GitHub's OAuth device flow. It shows a short code and the address
<https://github.com/login/device>; you enter the code there and approve
Lilypad. GitHub then hands Lilypad a token with two scopes:

- `repo`, to create and write the vault repository. GitHub OAuth Apps have no
  scope for a single private repository, so this one covers all your
  repositories; Lilypad only ever uses it on its own vault repository.
- `read:user`, to learn your GitHub username, which names the repository.

The token is saved in `oauth_tokens.json` in the preferences folder, or in
the folder given with `--data-dir` or `LILYPAD_DATA_DIR`, so each profile
signs in on its own (see [data-location.md](data-location.md)). On Linux and
macOS the file is readable only by your account. Its content is scrambled
with a key derived from the machine's host name and your user name, not from
a master password: anyone who can read your files can recover the token, so
treat the file as you would the token itself. When GitHub expires the token,
Lilypad refreshes it with the same client ID.

`lilypad-cli auth-status` shows whether you are signed in. Signing out
(`lilypad-cli logout`, or **Sign out** in the desktop app) removes the token
from that file. To cut access from GitHub's side as well, revoke Lilypad under
**GitHub Settings > Applications > Authorized OAuth Apps**.

## What goes to GitHub

On the first push Lilypad creates a private repository named
`lilypad-vault-<your GitHub username>`, described as "Lilypad encrypted vault
storage (do not edit manually)". Each vault is one file in it,
`vaults/<vault name>.lily`, byte for byte the encrypted file from your data
folder. GitHub sees the vault's name, its size and when it changed, never its
contents; a master password never leaves your machine.

## The four operations

| Desktop | CLI | What it does |
|---|---|---|
| Sync now | `lilypad-cli sync merge <vault>` | merges the remote copy into the local vault entry by entry, then pushes the result if GitHub lacks anything |
| Push to GitHub | `lilypad-cli sync push <vault>` | uploads the local vault |
| Pull from GitHub | `lilypad-cli sync pull <vault>` | replaces the local vault with the remote copy |
| Check status | `lilypad-cli sync status <vault>` | compares the two without changing either |

**Merge is the one to use day to day.** For each entry the newest version
wins, deletions and renames carry over, and a safety backup of the local vault
is taken first. It ends in sync from any starting state, including a conflict.

**Push** refuses to overwrite a remote copy that changed since this machine
last synced, and refuses when a remote copy exists but this machine has never
synced it: pull or merge first. `--force` (CLI only) overwrites the remote
copy anyway.

**Pull** downloads the remote copy and proves it decrypts with your master
password before touching anything. It refuses a copy in an older vault
format than the local one, takes a safety backup of the local vault, and only
then replaces it. A remote copy that is corrupt or was made with another
password never overwrites your vault.

## Status

Lilypad remembers, per vault, the remote commit and the local checksum of the
last successful sync (`sync/<vault>.json` in the data folder), plus a device
identifier generated once. Comparing both sides against that baseline gives:

| Status | Meaning |
|---|---|
| In sync | nothing changed on either side |
| Local ahead | only this machine changed: push or merge |
| Remote ahead | only GitHub changed: pull or merge |
| Conflict | both changed: merge combines them |
| No remote | the vault is not on GitHub yet: push to create it |
| No baseline | this machine never synced this vault: merge, or push or pull once |
