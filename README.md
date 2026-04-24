# sclaude

[English](./README.md) | [简体中文](./README.zh-CN.md)

`sclaude` is a Rust wrapper around Claude Code CLI for multi-account management, account import, encrypted account-pool sync, and model-pinned entrypoints.

The repository is intentionally code-only. It does not contain account pool data, cached usage, local credentials, or machine-specific config.

If you want a GUI for manual account management, see <https://github.com/murongg/ai-accounts-hub>.

## Install

Unix:

```bash
curl -fsSL https://raw.githubusercontent.com/lauzhihao/sclaude/main/install.sh | bash
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/lauzhihao/sclaude/main/install.ps1 | iex
```

Current published release targets:

- Linux: `x86_64-unknown-linux-musl`
- macOS: `x86_64-apple-darwin`, `aarch64-apple-darwin`
- Windows: `x86_64-pc-windows-msvc`

The installer:

- downloads the latest published `sclaude` release asset
- uses `SCLAUDE_HOME`, defaulting to `~/.sclaude`
- installs `sclaude` as the primary command under `$SCLAUDE_HOME/bin`
- installs `opus`, `sonnet`, and `haiku` as model entrypoints under `$SCLAUDE_HOME/bin`
- installs `sclaude-original` as a thin passthrough helper to the underlying `claude` under `$SCLAUDE_HOME/bin`
- imports the current Claude profile when it finds `~/.claude.json`, `~/.config.json`, `~/.claude/.claude.json`, or `~/.claude/.config.json`

Override `SCLAUDE_HOME` to move the full managed home. Override `INSTALL_BIN` only when you intentionally want the entrypoint binaries outside `$SCLAUDE_HOME/bin`.

Default managed layout:

```text
~/.sclaude/
  bin/
  runtime/
  tmp/
  accounts/
  state.json
  repo-sync.json
```

## Requirements

- Unix installer: `bash`, `curl`, `tar`
- Windows installer: PowerShell 5+ or PowerShell 7+
- `claude` is still required at runtime for `launch`, `login`, and passthrough commands
- if `claude` is missing, `sclaude` offers to install `@anthropic-ai/claude-code` through `npm` into `$SCLAUDE_HOME/runtime/claude-code`
- `push` and `pull` additionally require `git` plus `SCLAUDE_POOL_KEY`

Build from source:

```bash
cargo build --release
```

## Entrypoints

- `sclaude`: primary command
- `opus`: forces `--model opus`
- `sonnet`: forces `--model sonnet`
- `haiku`: forces `--model haiku`
- `sclaude-original`: passthrough helper to the underlying `claude`

All runtime entrypoints launch Claude with:

- `CLAUDE_CONFIG_DIR` pointing at the selected managed profile
- `IS_SANDBOX=1`
- `--dangerously-skip-permissions` unless you already passed it

## Command Overview

| Command | Purpose |
| --- | --- |
| `sclaude` | Default behavior; same as `sclaude launch` |
| `sclaude launch` | Pick the best account, switch to it, then launch or resume Claude |
| `sclaude auto` | Pick the best account without launching Claude |
| `sclaude login` | Add one account through official OAuth or API credentials, then switch to it |
| `sclaude add` | Add one account through the same login flow as `login`; switch only when `--switch` is passed |
| `sclaude push <repo>` | Encrypt and push the full local account pool into a Git repository |
| `sclaude pull <repo>` | Pull and decrypt an account pool from a Git repository, then overwrite local state |
| `sclaude use <label>` | Switch directly to a known account by the label shown in `list` |
| `sclaude rm <label>` | Remove a stored account by the label shown in `list` |
| `sclaude list` | Refresh current usage state, then render the account table |
| `sclaude refresh` | Refresh all known accounts and print the latest table |
| `sclaude import-auth <path>` | Import a Claude auth file or a Claude profile directory |
| `sclaude import-known` | Import the default known local Claude profile |
| `sclaude update` | Self-update `sclaude` from GitHub Releases; `upgrade` is an alias |

## Login Modes

### OAuth

```bash
sclaude login
sclaude login --oauth
sclaude login --oauth --username you@example.com
```

Actual behavior:

- runs `claude auth login --claudeai` in a temporary managed profile
- after OAuth login, runs `claude setup-token` in a PTY and tries to extract the printed `sk-ant-oat...` token automatically
- if automatic extraction fails, `sclaude` falls back to prompting you to paste the token manually
- stores the OAuth token and its creation time in local state so launches can use `CLAUDE_CODE_OAUTH_TOKEN`
- `--username` is only an email hint passed to Claude
- `--password` is kept only for compatibility and is currently ignored
- after a successful login, `sclaude login` always switches to the imported account

### API

```bash
sclaude login --api \
  --provider poe.com \
  --ANTHROPIC_BASE_URL https://example.com/api/claude \
  --ANTHROPIC_API_KEY sk-ant-xxxx
```

Actual behavior:

- stores a minimal managed Claude profile containing `ANTHROPIC_BASE_URL`, `ANTHROPIC_API_KEY`, and `providerId`
- displays the account as `key-<prefix>@<provider>`
- deduplicates by the effective `(ANTHROPIC_BASE_URL, ANTHROPIC_API_KEY)` fingerprint, so repeated imports of the same API account update the existing record instead of creating a duplicate
- multiple different providers or different key/base-url pairs can coexist

### `add`

```bash
sclaude add [--switch]
sclaude add --api --provider poe.com --ANTHROPIC_BASE_URL ... --ANTHROPIC_API_KEY ...
```

Actual behavior:

- uses the same login flow and options as `sclaude login`
- unlike `login`, it switches to the new account only when `--switch` is passed
- OAuth `add` also runs `claude setup-token` and stores the pasted token

## Command Details

### `launch`

```bash
sclaude launch [--no-import-known] [--no-login] [--dry-run] [--no-resume] [--no-launch] [<claude args...>]
```

- imports known local profiles first unless `--no-import-known` is passed
- refreshes state and keeps the current account when it still looks usable
- if `--no-login` is not passed and no account exists, it falls back to the OAuth login flow
- `--dry-run` prints the chosen account without switching or launching
- `--no-launch` switches the account without starting Claude
- extra arguments are forwarded to Claude

### `auto`

```bash
sclaude auto [--no-import-known] [--no-login] [--dry-run]
```

- same account-selection logic as `launch`
- never starts Claude

### `use`

```bash
sclaude use <label>
```

- matches the account label shown in `sclaude list`
- matching is case-insensitive

### `rm`

```bash
sclaude rm [-y|--yes] <label>
```

- removes the account from local state and deletes its managed profile
- asks for interactive confirmation unless `-y` is passed

### `list`

```bash
sclaude list
```

- refreshes all known accounts first
- always renders the account table with account label, plan, quota columns, reset time, and status, even when zero accounts are currently usable
- OAuth usage is refreshed from the managed Claude profile's short-lived access token when available; quota fields may stay `N/A` on temporary fetch failures

### `refresh`

```bash
sclaude refresh
```

- refreshes all known accounts
- prints a refreshed-count message and the latest account table
- like `list`, it keeps rendering the latest account table even when no account is currently usable

### `import-auth`

```bash
sclaude import-auth <path>
```

- `<path>` can be a Claude auth file or a parent directory containing one
- imported profiles are copied into `sclaude` state storage as managed accounts
- the profile must contain actual account credentials, currently either `userID` or `ANTHROPIC_API_KEY`; plain settings-only files are rejected

### `import-known`

```bash
sclaude import-known
```

- when `CLAUDE_CONFIG_DIR` is set, imports that live profile
- otherwise imports the default local Claude profile from:
  - `~/.claude.json`
  - `~/.config.json`
  - `~/.claude/`
- prefers `claude auth status` when available, and falls back to local auth-file parsing
- like `import-auth`, only profiles with actual account credentials are imported; plain settings-only files are ignored

### `push`

```bash
export SCLAUDE_POOL_KEY='replace-with-a-long-random-secret'
sclaude push [-i <identity_file>] [--path <repo_path>] [repo]
```

- clones the repository with your existing Git credentials
- exports account metadata and stored OAuth tokens as an encrypted bundle
- stores the bundle under `.sclaude-account-pool/bundle.enc.json` by default
- only pushes when the encrypted bundle changed
- when `[repo]` is explicitly provided once, `sclaude` remembers it under `$SCLAUDE_HOME/repo-sync.json`
- when `[repo]` is omitted, `sclaude` uses `SCLAUDE_POOL_REPO` first, then the saved repo from `$SCLAUDE_HOME`
- `--path <repo_path>` must be a relative repository subdirectory
- `-i <identity_file>` passes an SSH key to Git through `GIT_SSH_COMMAND`

### `pull`

```bash
export SCLAUDE_POOL_KEY='replace-with-the-same-secret'
sclaude pull [-i <identity_file>] [--path <repo_path>] [repo]
```

- clones the repository with your existing Git credentials
- decrypts the remote account pool bundle
- force-overwrites the local managed account pool instead of merging
- token-only accounts are restored as minimal local Claude profiles and launched with `CLAUDE_CODE_OAUTH_TOKEN`
- when `[repo]` is omitted, `sclaude` uses `SCLAUDE_POOL_REPO` first, then the saved repo from `$SCLAUDE_HOME`
- refreshes account usage after import and prints the latest table

### `update`

```bash
sclaude update [-f|--force]
sclaude upgrade [-f|--force]
```

- downloads the latest matching GitHub Releases asset from `lauzhihao/sclaude`
- replaces the current `sclaude` executable
- also updates sidecar binaries such as `opus`, `sonnet`, and `haiku`
- stages downloaded update binaries under `$SCLAUDE_HOME/tmp`
- `-f`, `--force` reinstalls even if the current version already matches the latest release

## Passthrough Behavior

If the first non-global argument is not a declared `sclaude` subcommand, `sclaude` treats it as a Claude CLI command after account selection.

Examples:

```bash
sclaude auth status
sclaude mcp list
opus auth status
```

That is why `opus auth status` works even though `auth` is not a declared `sclaude` subcommand.

## Account Storage Notes

- managed accounts live under the resolved `SCLAUDE_HOME` directory, defaulting to `~/.sclaude`
- imported profiles are stored as isolated managed Claude homes
- credential bundles are stored as local bundle files inside the managed account home
- OAuth tokens are stored in local state and shown in `list` as `sk...<last6> <YYYYmmdd-expiry>`
- temporary login profiles, Git checkouts, and update staging live under `$SCLAUDE_HOME/tmp`
- `import-known` and installer auto-import still read existing Claude profiles under `$HOME` or `CLAUDE_CONFIG_DIR` only as external import sources
