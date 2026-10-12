# The command line

`lilypad-cli` does everything the desktop app does with vaults, from a shell
or a script. It reads and writes the same data folder as the desktop app and
the TUI (see [data-location.md](data-location.md)).

`lilypad-cli --help` lists every command, and `lilypad-cli <command> --help`
every option of one command.

## Master passwords

A command that opens a vault needs its master password. The CLI takes it from,
in order:

1. `--master-password <PASSWORD>`. Avoid it: the value shows up in `ps` and in
   your shell history, and the CLI prints a warning when you use it.
2. The `LILYPAD_MASTER_PASSWORD` environment variable.
3. A hidden prompt.

A new master password is asked for twice, or read from the first line of
standard input when it is piped. `init` also accepts it through the option or
the variable above; `change-master-password` uses those for the current
password and always asks for the new one. Entry values
work the same way: a value given on the command line is used as is, otherwise
it is read from piped standard input or prompted for hidden.

## Global options

| Option | Effect |
|---|---|
| `--data-dir <DIR>` | use another data folder (wins over `LILYPAD_DATA_DIR`) |
| `--master-password <PASSWORD>` | see above |
| `--output-format text\|json` | JSON output for `vaults`, `list`, `search`, `history`, `trash`, `audit`, `audit-log` and `breach-check` |

## Commands

| Area | Commands |
|---|---|
| Vaults | `init <vault>`, `vaults`, `rename-vault <from> <to>`, `delete-vault <vault>`, `change-master-password <vault>` |
| Entries | `add <vault> <label> [value]`, `list <vault>`, `get <vault> <label>`, `update <vault> <label> [value]`, `rename-entry <vault> <label> <new-label>`, `search <vault> <query>`, `history <vault> <label>` |
| Trash | `remove <vault> <label>` (moves it to the Trash; `--purge` erases it for good), `trash <vault>`, `restore <vault> <label>` |
| Health | `audit <vault>` (weak, reused, old and expiring passwords), `breach-check <vault>` (Have I Been Pwned), `audit-log <vault>` |
| TOTP | `totp <vault> <label>`, `backup-codes <vault> <label>` (`--generate` for a new set, `--verify <CODE>` to use one) |
| Import and export | `import <vault> <file>` (`--dry-run` to preview), `export <vault> <file>` (plain-text CSV, asks first unless `--force`) |
| Generator | `generate` (`--length`, `--uppercase`, `--lowercase`, `--digits`, `--symbols`, `--copy`) |
| Backups | `backup <vault>`, `list-backups <vault>`, `restore-backup <file>`, `prune-backups <vault>` (`--keep`, 5 by default) |
| Sync | `login`, `logout`, `auth-status`, `sync push`, `sync pull`, `sync merge`, `sync status` (see [sync.md](sync.md)) |
| Shell | `completions <shell>` |

`add` and `update` take `--username`, `--url`, `--notes`, `--tag` (repeatable),
`--folder`, `--entry-type` (`login`, `card`, `identity`, `secure-note`,
`software-license`, `wifi`, `server` or `custom`) and `--totp-secret`; `add`
also takes `--attachment <FILE>` (repeatable), and `update` takes
`--remove-tag` and `--clear-tags`. `update` changes only the fields you pass.

`get --copy` and `generate --copy` put the password on the clipboard and clear
it after `--clipboard-timeout` seconds (15 by default); the command waits until
then, and Ctrl-C keeps the value on the clipboard. `get` hides the password
unless you pass `--show-password`.

`import` detects the format from the file's content: Lilypad, LastPass,
Bitwarden (CSV and JSON), KeePassXC, 1Password, Safari, Chrome and Edge,
Firefox, Proton Pass and Dashlane.

## Examples

```bash
# Create a vault (asks for a new master password twice)
lilypad-cli init personal

# Add a login; the password is prompted for, hidden
lilypad-cli add personal github --username octocat --url https://github.com

# Copy it to the clipboard for 15 seconds
lilypad-cli get personal github --copy

# A 32-character password, straight to the clipboard
lilypad-cli generate --length 32 --copy

# Every entry, as JSON, with the password from the environment
LILYPAD_MASTER_PASSWORD=... lilypad-cli --output-format json list personal
```

## Shell completions

`lilypad-cli completions <shell>` prints a completion script for `bash`,
`zsh`, `fish`, `elvish` or `powershell`. The script completes the command name
`lilypad`, the name the CLI gives itself in `--help`. For example, with bash:

```bash
lilypad-cli completions bash > ~/.local/share/bash-completion/completions/lilypad
```
