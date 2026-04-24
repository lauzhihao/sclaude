use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng, rand_core::RngCore};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::credentials::{
    ClaudeCredentialBundle, capture_credential_bundle, credential_bundle_key_for_id,
    restore_credential_bundle, save_credential_bundle,
};
use super::paths::{find_program, managed_auth_file};
use super::{ClaudeAdapter, ensure_oauth_token_profile};
use crate::core::state::{AccountRecord, STATE_VERSION, State};
use crate::core::storage;
use crate::core::ui as core_ui;

const DEFAULT_BUNDLE_DIR: &str = ".sclaude-account-pool";
const BUNDLE_FILENAME: &str = "bundle.enc.json";
const BUNDLE_KEY_ENV: &str = "SCLAUDE_POOL_KEY";
const BUNDLE_DIR_ENV: &str = "SCLAUDE_POOL_PATH";
const BUNDLE_ALGORITHM: &str = "xchacha20poly1305-sha256";

impl ClaudeAdapter {
    pub fn push_account_pool(
        &self,
        state_dir: &Path,
        state: &State,
        repo: &str,
        bundle_dir: Option<&str>,
        identity_file: Option<&Path>,
    ) -> Result<PushOutcome> {
        let ui = core_ui::messages();
        if state.accounts.is_empty() {
            bail!("{}", ui.repo_push_no_accounts());
        }

        let git_bin = resolve_git_bin()?;
        let repo = repo.trim();
        if repo.is_empty() {
            bail!("{}", ui.repo_sync_invalid_repo());
        }
        validate_identity_file(identity_file)?;
        let bundle_dir = resolve_bundle_dir(bundle_dir)?;
        let bundle_key = resolve_bundle_key()?;
        let checkout = clone_repo(&git_bin, state_dir, repo, identity_file)?;
        let bundle_root = checkout.checkout_dir.join(&bundle_dir);
        let bundle_path = bundle_root.join(BUNDLE_FILENAME);
        let bundle = build_repo_bundle(state)?;
        let bundle_bytes = serde_json::to_vec(&bundle)?;

        println!("{}", ui.repo_push_start(repo));
        if bundle_path.exists() {
            if let Ok(existing) = decrypt_bundle_file(&bundle_path, &bundle_key) {
                if existing == bundle_bytes {
                    return Ok(PushOutcome {
                        changed: false,
                        exported_accounts: state.accounts.len(),
                    });
                }
            }
            // 解密失败（文件损坏、格式不兼容、key 不匹配）→ 忽略，直接覆盖
        }

        prepare_bundle_dir(&bundle_root)?;
        write_bundle_file(&bundle_path, &bundle_bytes, &bundle_key)?;

        git_add(&git_bin, &checkout.checkout_dir, &bundle_dir)?;
        if !git_has_changes(&git_bin, &checkout.checkout_dir, &bundle_dir)? {
            return Ok(PushOutcome {
                changed: false,
                exported_accounts: state.accounts.len(),
            });
        }

        git_commit(&git_bin, &checkout.checkout_dir)?;
        git_push(&git_bin, &checkout.checkout_dir, repo, identity_file)?;

        Ok(PushOutcome {
            changed: true,
            exported_accounts: state.accounts.len(),
        })
    }

    pub fn pull_account_pool(
        &self,
        state_dir: &Path,
        state: &mut State,
        repo: &str,
        bundle_dir: Option<&str>,
        identity_file: Option<&Path>,
    ) -> Result<PullOutcome> {
        let ui = core_ui::messages();
        let git_bin = resolve_git_bin()?;
        let repo = repo.trim();
        if repo.is_empty() {
            bail!("{}", ui.repo_sync_invalid_repo());
        }
        validate_identity_file(identity_file)?;
        let bundle_dir = resolve_bundle_dir(bundle_dir)?;
        let bundle_key = resolve_bundle_key()?;
        let checkout = clone_repo(&git_bin, state_dir, repo, identity_file)?;
        let bundle_root = checkout.checkout_dir.join(&bundle_dir);
        let bundle_path = bundle_root.join(BUNDLE_FILENAME);

        println!("{}", ui.repo_pull_start(repo));
        if !bundle_path.exists() {
            bail!(
                "{}",
                ui.repo_pull_missing_bundle(&bundle_dir.display().to_string())
            );
        }

        let bundle: RepoBundle =
            serde_json::from_slice(&decrypt_bundle_file(&bundle_path, &bundle_key)?)
                .context("failed to parse decrypted account-pool bundle")?;
        if bundle.accounts.is_empty() {
            bail!(
                "{}",
                ui.repo_pull_no_accounts(&bundle_dir.display().to_string())
            );
        }
        *state = overwrite_local_account_pool(state_dir, &bundle)?;

        Ok(PullOutcome {
            imported_accounts: state.accounts.len(),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PushOutcome {
    pub changed: bool,
    pub exported_accounts: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct PullOutcome {
    pub imported_accounts: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct RepoBundle {
    version: u32,
    exported_at: i64,
    accounts: Vec<RepoBundleAccount>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RepoBundleAccount {
    id: String,
    email: String,
    #[serde(default)]
    account_kind: Option<String>,
    #[serde(default)]
    provider_id: Option<String>,
    account_id: Option<String>,
    #[serde(default)]
    identity_fingerprint: Option<String>,
    plan: Option<String>,
    #[serde(default)]
    credential_bundle_key: Option<String>,
    #[serde(default)]
    credential_bundle_b64: Option<String>,
    #[serde(default)]
    oauth_token: Option<String>,
    #[serde(default)]
    oauth_token_created_at: Option<i64>,
    added_at: i64,
    updated_at: i64,
    #[serde(default)]
    files: Vec<RepoBundleFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RepoBundleFile {
    relative_path: String,
    contents_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedBundleFile {
    version: u32,
    algorithm: String,
    nonce_b64: String,
    ciphertext_b64: String,
}

fn build_repo_bundle(state: &State) -> Result<RepoBundle> {
    let mut accounts = state.accounts.iter().collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.id.cmp(&right.id).then(left.email.cmp(&right.email)));

    let mut bundle_accounts = Vec::with_capacity(accounts.len());
    for account in accounts {
        bundle_accounts.push(export_account_bundle(account)?);
    }

    Ok(RepoBundle {
        version: 1,
        exported_at: super::now_ts(),
        accounts: bundle_accounts,
    })
}

fn export_account_bundle(account: &AccountRecord) -> Result<RepoBundleAccount> {
    Ok(RepoBundleAccount {
        id: account.id.clone(),
        email: account.email.clone(),
        account_kind: account.account_kind.clone(),
        provider_id: account.provider_id.clone(),
        account_id: account.account_id.clone(),
        identity_fingerprint: account.identity_fingerprint.clone(),
        plan: account.plan.clone(),
        credential_bundle_key: account.credential_bundle_key.clone(),
        credential_bundle_b64: None,
        oauth_token: account.oauth_token.clone(),
        oauth_token_created_at: account.oauth_token_created_at,
        added_at: account.added_at,
        updated_at: account.updated_at,
        files: Vec::new(),
    })
}

fn capture_profile_bundle_for_export(root: &Path) -> Result<ClaudeCredentialBundle> {
    let auth_path = managed_auth_file(root);
    capture_credential_bundle(Some(root), &auth_path)
}

fn collect_profile_files(root: &Path) -> Result<Vec<RepoBundleFile>> {
    let mut collected = Vec::new();
    collect_profile_files_recursive(root, root, &mut collected)?;
    collected.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(collected)
}

fn collect_profile_files_recursive(
    root: &Path,
    current: &Path,
    collected: &mut Vec<RepoBundleFile>,
) -> Result<()> {
    let entries =
        fs::read_dir(current).with_context(|| format!("failed to read {}", current.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // 跳过 Claude Code 运行时数据目录（不需要在 bundle 中同步）
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                dir_name,
                "file-history"
                    | "projects"
                    | "sessions"
                    | "tasks"
                    | "plugins"
                    | "remote"
                    | "usage-data"
                    | "backups"
                    | "cache"
                    | "paste-cache"
                    | "session-env"
                    | "shell-snapshots"
                    | "telemetry"
                    | "todos"
                    | "plans"
                    | "debug"
                    | "statsig"
            ) {
                continue;
            }
            collect_profile_files_recursive(root, &path, collected)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                matches!(
                    name,
                    ".credential-bundle.json"
                        | ".credentials.json"
                        | "history.jsonl"
                        | "stats-cache.json"
                        | "mcp-needs-auth-cache.json"
                        | "settings.cp"
                        | "settings.json.cp"
                        | "settings.json.poe"
                )
            })
        {
            continue;
        }
        let relative_path = path
            .strip_prefix(root)
            .with_context(|| format!("failed to strip prefix for {}", path.display()))?
            .to_string_lossy()
            .into_owned();
        let contents =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        collected.push(RepoBundleFile {
            relative_path,
            contents_b64: BASE64_STANDARD.encode(contents),
        });
    }
    Ok(())
}

fn prepare_bundle_dir(bundle_root: &Path) -> Result<()> {
    if bundle_root.exists() {
        fs::remove_dir_all(bundle_root)
            .with_context(|| format!("failed to remove {}", bundle_root.display()))?;
    }
    fs::create_dir_all(bundle_root)
        .with_context(|| format!("failed to create {}", bundle_root.display()))?;
    Ok(())
}

fn write_bundle_file(path: &Path, plaintext: &[u8], bundle_key: &[u8; 32]) -> Result<()> {
    let encrypted = encrypt_bundle_bytes(plaintext, bundle_key)?;
    let mut bytes = serde_json::to_vec_pretty(&encrypted)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn decrypt_bundle_file(path: &Path, bundle_key: &[u8; 32]) -> Result<Vec<u8>> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let encrypted: EncryptedBundleFile = serde_json::from_str(&contents)
        .with_context(|| format!("invalid encrypted bundle file: {}", path.display()))?;
    decrypt_bundle_bytes(&encrypted, bundle_key)
}

fn encrypt_bundle_bytes(plaintext: &[u8], bundle_key: &[u8; 32]) -> Result<EncryptedBundleFile> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(bundle_key));
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| anyhow!("failed to encrypt account-pool bundle"))?;

    Ok(EncryptedBundleFile {
        version: 1,
        algorithm: BUNDLE_ALGORITHM.into(),
        nonce_b64: BASE64_STANDARD.encode(nonce),
        ciphertext_b64: BASE64_STANDARD.encode(ciphertext),
    })
}

fn decrypt_bundle_bytes(encrypted: &EncryptedBundleFile, bundle_key: &[u8; 32]) -> Result<Vec<u8>> {
    if encrypted.version != 1 || encrypted.algorithm != BUNDLE_ALGORITHM {
        bail!("unsupported encrypted bundle metadata");
    }

    let nonce_bytes = BASE64_STANDARD
        .decode(&encrypted.nonce_b64)
        .context("failed to decode bundle nonce")?;
    let ciphertext = BASE64_STANDARD
        .decode(&encrypted.ciphertext_b64)
        .context("failed to decode bundle ciphertext")?;
    let nonce: [u8; 24] = nonce_bytes
        .try_into()
        .map_err(|_| anyhow!("invalid encrypted bundle nonce length"))?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(bundle_key));
    cipher
        .decrypt(XNonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| anyhow!("failed to decrypt account-pool bundle"))
}

fn overwrite_local_account_pool(state_dir: &Path, bundle: &RepoBundle) -> Result<State> {
    let accounts_root = state_dir.join("accounts");
    let staging_root = state_dir.join(format!(".sclaude-pull-{}", Uuid::new_v4()));
    if staging_root.exists() {
        fs::remove_dir_all(&staging_root)
            .with_context(|| format!("failed to remove {}", staging_root.display()))?;
    }
    fs::create_dir_all(staging_root.join("accounts"))
        .with_context(|| format!("failed to create {}", staging_root.display()))?;

    let mut state = State {
        version: STATE_VERSION,
        accounts: Vec::with_capacity(bundle.accounts.len()),
        usage_cache: Default::default(),
        current_account_id: None,
    };

    for account in &bundle.accounts {
        let staged_home = staging_root.join("accounts").join(&account.id);
        fs::create_dir_all(&staged_home)
            .with_context(|| format!("failed to create {}", staged_home.display()))?;

        for file in &account.files {
            let target = staged_home.join(&file.relative_path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            let bytes = BASE64_STANDARD
                .decode(&file.contents_b64)
                .with_context(|| format!("failed to decode {}", file.relative_path))?;
            fs::write(&target, bytes)
                .with_context(|| format!("failed to write {}", target.display()))?;
        }

        let credential_bundle_key = account
            .credential_bundle_key
            .clone()
            .unwrap_or_else(|| credential_bundle_key_for_id(&account.id));
        if let Some(bundle_b64) = account.credential_bundle_b64.as_ref() {
            let bytes = BASE64_STANDARD
                .decode(bundle_b64)
                .context("failed to decode Claude credential bundle")?;
            let bundle: ClaudeCredentialBundle = serde_json::from_slice(&bytes)
                .context("failed to parse Claude credential bundle")?;
            save_credential_bundle(&staged_home, &credential_bundle_key, &bundle)?;
            restore_credential_bundle(&staged_home, &bundle)?;
        } else if account
            .oauth_token
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        {
            ensure_oauth_token_profile(&staged_home)?;
        }
        // 使用最终位置（rename 之后）而非临时位置来设置 auth_path 和 config_path
        let final_account_home = accounts_root.join(&account.id);
        let auth_path = managed_auth_file(&final_account_home);
        state.accounts.push(AccountRecord {
            id: account.id.clone(),
            email: account.email.clone(),
            account_kind: account.account_kind.clone(),
            provider_id: account.provider_id.clone(),
            account_id: account.account_id.clone(),
            identity_fingerprint: account.identity_fingerprint.clone(),
            plan: account.plan.clone(),
            auth_path: auth_path.to_string_lossy().into_owned(),
            config_path: Some(final_account_home.to_string_lossy().into_owned()),
            credential_bundle_key: Some(credential_bundle_key),
            oauth_token: account.oauth_token.clone(),
            oauth_token_created_at: account.oauth_token_created_at,
            added_at: account.added_at,
            updated_at: account.updated_at,
        });
    }

    if accounts_root.exists() {
        fs::remove_dir_all(&accounts_root)
            .with_context(|| format!("failed to remove {}", accounts_root.display()))?;
    }
    fs::rename(staging_root.join("accounts"), &accounts_root)
        .with_context(|| format!("failed to move {} into place", accounts_root.display()))?;
    let _ = fs::remove_dir_all(&staging_root);

    state.current_account_id = state.accounts.first().map(|account| account.id.clone());
    Ok(state)
}

fn resolve_bundle_key() -> Result<[u8; 32]> {
    let secret = env::var(BUNDLE_KEY_ENV)
        .with_context(|| format!("{BUNDLE_KEY_ENV} is required for account-pool sync"))?;
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    Ok(hasher.finalize().into())
}

fn resolve_bundle_dir(bundle_dir: Option<&str>) -> Result<PathBuf> {
    let configured =
        resolve_bundle_dir_source(bundle_dir, configured_bundle_dir_from_env().as_deref())
            .to_string();
    resolve_bundle_dir_value(&configured)
}

fn resolve_bundle_dir_source<'a>(bundle_dir: Option<&'a str>, env_dir: Option<&'a str>) -> &'a str {
    bundle_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| env_dir.map(str::trim).filter(|value| !value.is_empty()))
        .unwrap_or(DEFAULT_BUNDLE_DIR)
}

fn resolve_bundle_dir_value(raw: &str) -> Result<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("bundle path cannot be empty");
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        bail!("bundle path must be relative to the repository root");
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            bail!("bundle path must stay within the repository checkout");
        }
    }
    Ok(path)
}

fn configured_bundle_dir_from_env() -> Option<String> {
    env::var(BUNDLE_DIR_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_identity_file(identity_file: Option<&Path>) -> Result<()> {
    if let Some(path) = identity_file {
        storage::ensure_exists(path, "SSH identity file")?;
    }
    Ok(())
}

fn resolve_git_bin() -> Result<PathBuf> {
    let Some(git_bin) = find_program(git_binary_names()) else {
        let install_command = if cfg!(target_os = "macos") {
            "xcode-select --install"
        } else if cfg!(target_os = "windows") {
            "winget install --id Git.Git -e"
        } else {
            "sudo apt install git"
        };
        bail!(
            "{}",
            core_ui::messages().repo_sync_missing_git(install_command)
        );
    };
    Ok(git_bin)
}

fn git_binary_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["git.exe", "git.cmd", "git"]
    } else {
        &["git"]
    }
}

#[derive(Debug)]
struct RepoCheckout {
    checkout_dir: PathBuf,
}

impl RepoCheckout {
    fn new(temp_root: &Path, prefix: &str) -> Result<Self> {
        fs::create_dir_all(temp_root)
            .with_context(|| format!("failed to create {}", temp_root.display()))?;
        let checkout_dir = temp_root.join(format!("{prefix}-{}", Uuid::new_v4()));
        fs::create_dir_all(&checkout_dir)
            .with_context(|| format!("failed to create {}", checkout_dir.display()))?;
        Ok(Self { checkout_dir })
    }
}

impl Drop for RepoCheckout {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.checkout_dir);
    }
}

fn clone_repo(
    git_bin: &Path,
    state_dir: &Path,
    repo: &str,
    identity_file: Option<&Path>,
) -> Result<RepoCheckout> {
    let temp_root = storage::tmp_dir(state_dir);
    let checkout = RepoCheckout::new(&temp_root, "git")?;
    run_git(
        git_bin,
        checkout.checkout_dir.parent(),
        Some(repo),
        identity_file,
        &[
            "clone".into(),
            "--depth".into(),
            "1".into(),
            repo.into(),
            checkout.checkout_dir.to_string_lossy().into_owned(),
        ],
    )?;
    Ok(checkout)
}

fn git_add(git_bin: &Path, checkout_dir: &Path, bundle_dir: &Path) -> Result<()> {
    run_git(
        git_bin,
        Some(checkout_dir),
        None,
        None,
        &["add".into(), bundle_dir.to_string_lossy().into_owned()],
    )?;
    Ok(())
}

fn git_has_changes(git_bin: &Path, checkout_dir: &Path, bundle_dir: &Path) -> Result<bool> {
    let output = run_git_output(
        git_bin,
        Some(checkout_dir),
        None,
        None,
        &[
            "status".into(),
            "--porcelain".into(),
            "--".into(),
            bundle_dir.to_string_lossy().into_owned(),
        ],
    )?;
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

fn git_commit(git_bin: &Path, checkout_dir: &Path) -> Result<()> {
    let message = format!("sclaude encrypted account pool sync {}", super::now_ts());
    run_git(
        git_bin,
        Some(checkout_dir),
        None,
        None,
        &[
            "-c".into(),
            "user.name=sclaude".into(),
            "-c".into(),
            "user.email=sclaude@local".into(),
            "commit".into(),
            "-m".into(),
            message,
        ],
    )?;
    Ok(())
}

fn git_push(
    git_bin: &Path,
    checkout_dir: &Path,
    repo: &str,
    identity_file: Option<&Path>,
) -> Result<()> {
    run_git(
        git_bin,
        Some(checkout_dir),
        Some(repo),
        identity_file,
        &["push".into(), "origin".into(), "HEAD".into()],
    )?;
    Ok(())
}

fn run_git(
    git_bin: &Path,
    cwd: Option<&Path>,
    repo: Option<&str>,
    identity_file: Option<&Path>,
    args: &[String],
) -> Result<()> {
    let output = run_git_output(git_bin, cwd, repo, identity_file, args)?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Some(repo) = repo
        && stderr.contains("Permission denied")
    {
        bail!("{}", core_ui::messages().repo_sync_push_auth_failed(repo));
    }
    bail!("git command failed: {}", stderr.trim());
}

fn run_git_output(
    git_bin: &Path,
    cwd: Option<&Path>,
    _repo: Option<&str>,
    identity_file: Option<&Path>,
    args: &[String],
) -> Result<Output> {
    let mut command = Command::new(git_bin);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    if let Some(identity_file) = identity_file {
        command.env(
            "GIT_SSH_COMMAND",
            format!(
                "ssh -i {} -o IdentitiesOnly=yes",
                shell_escape_path(identity_file)
            ),
        );
    }
    command.args(args);
    command
        .output()
        .with_context(|| format!("failed to execute {}", git_bin.display()))
}

fn shell_escape_path(path: &Path) -> String {
    path.to_string_lossy().replace('\'', r#"'\''"#)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use anyhow::Result;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

    use super::{
        RepoBundle, RepoBundleAccount, RepoBundleFile, build_repo_bundle,
        overwrite_local_account_pool, resolve_bundle_dir, resolve_bundle_dir_source,
    };
    use crate::core::state::{AccountRecord, State};

    #[test]
    fn bundle_dir_defaults_to_sclaude_location() -> Result<()> {
        assert_eq!(
            resolve_bundle_dir(None)?,
            PathBuf::from(".sclaude-account-pool")
        );
        Ok(())
    }

    #[test]
    fn bundle_dir_prefers_cli_argument_over_environment() {
        assert_eq!(
            resolve_bundle_dir_source(Some("custom/pool"), Some("env/pool")),
            "custom/pool"
        );
    }

    #[test]
    fn bundle_dir_uses_environment_when_cli_argument_is_missing() {
        assert_eq!(
            resolve_bundle_dir_source(None, Some("env/pool")),
            "env/pool"
        );
    }

    #[test]
    fn overwrite_local_account_pool_restores_profile_tree() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("sclaude-overwrite-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp)?;

        let bundle = RepoBundle {
            version: 1,
            exported_at: 0,
            accounts: vec![RepoBundleAccount {
                id: "acct-1".into(),
                email: "a@example.com".into(),
                account_kind: Some("oauth".into()),
                provider_id: None,
                account_id: Some("org-1".into()),
                identity_fingerprint: Some("oauth:org-1".into()),
                plan: Some("pro".into()),
                credential_bundle_key: None,
                credential_bundle_b64: None,
                oauth_token: None,
                oauth_token_created_at: None,
                added_at: 1,
                updated_at: 2,
                files: vec![
                    RepoBundleFile {
                        relative_path: ".claude.json".into(),
                        contents_b64: BASE64_STANDARD.encode(br#"{"userID":"acct-1"}"#),
                    },
                    RepoBundleFile {
                        relative_path: "sessions/1.json".into(),
                        contents_b64: BASE64_STANDARD.encode(b"{}"),
                    },
                ],
            }],
        };

        let state = overwrite_local_account_pool(&tmp, &bundle)?;

        assert_eq!(state.accounts.len(), 1);
        assert!(
            tmp.join("accounts")
                .join("acct-1")
                .join("sessions")
                .join("1.json")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn build_repo_bundle_exports_token_without_profile_files() -> Result<()> {
        let state = State {
            accounts: vec![AccountRecord {
                id: "acct-1".into(),
                email: "a@example.com".into(),
                account_kind: Some("oauth".into()),
                account_id: Some("org-1".into()),
                identity_fingerprint: Some("oauth:org-1".into()),
                oauth_token: Some("sk-ant-oat-exampleabcdef".into()),
                oauth_token_created_at: Some(1),
                added_at: 1,
                updated_at: 2,
                ..Default::default()
            }],
            ..Default::default()
        };

        let bundle = build_repo_bundle(&state)?;

        assert_eq!(bundle.accounts.len(), 1);
        let account = &bundle.accounts[0];
        assert_eq!(
            account.oauth_token.as_deref(),
            Some("sk-ant-oat-exampleabcdef")
        );
        assert_eq!(account.oauth_token_created_at, Some(1));
        assert!(account.files.is_empty());
        assert!(account.credential_bundle_b64.is_none());
        Ok(())
    }

    #[test]
    fn overwrite_local_account_pool_restores_token_only_profile() -> Result<()> {
        let tmp =
            std::env::temp_dir().join(format!("sclaude-token-overwrite-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp)?;

        let bundle = RepoBundle {
            version: 1,
            exported_at: 0,
            accounts: vec![RepoBundleAccount {
                id: "acct-token".into(),
                email: "token@example.com".into(),
                account_kind: Some("oauth".into()),
                provider_id: None,
                account_id: Some("org-token".into()),
                identity_fingerprint: Some("oauth:org-token".into()),
                plan: Some("pro".into()),
                credential_bundle_key: None,
                credential_bundle_b64: None,
                oauth_token: Some("sk-ant-oat-tokenabcdef".into()),
                oauth_token_created_at: Some(1),
                added_at: 1,
                updated_at: 2,
                files: Vec::new(),
            }],
        };

        let state = overwrite_local_account_pool(&tmp, &bundle)?;
        let account = &state.accounts[0];
        assert_eq!(
            account.oauth_token.as_deref(),
            Some("sk-ant-oat-tokenabcdef")
        );
        assert_eq!(account.oauth_token_created_at, Some(1));
        let auth_path = tmp.join("accounts").join("acct-token").join(".claude.json");
        let auth: serde_json::Value = serde_json::from_str(&fs::read_to_string(auth_path)?)?;
        assert_eq!(
            auth.get("hasCompletedOnboarding")
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        Ok(())
    }
}
