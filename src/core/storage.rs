use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::core::state::State;

const DEFAULT_STATE_BASENAME: &str = "sclaude";
const STATE_DIR_ENV: &str = "SCLAUDE_HOME";
const REPO_SYNC_CONFIG_FILENAME: &str = "repo-sync.json";
const BIN_DIR_NAME: &str = "bin";
const RUNTIME_DIR_NAME: &str = "runtime";
const TMP_DIR_NAME: &str = "tmp";

pub fn resolve_state_dir(override_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = override_dir {
        return Ok(expand_user_path(path));
    }

    if let Some(path) = configured_state_dir_from_env() {
        return Ok(path);
    }

    default_state_dir()
}

pub fn bin_dir(state_dir: &Path) -> PathBuf {
    state_dir.join(BIN_DIR_NAME)
}

pub fn runtime_dir(state_dir: &Path) -> PathBuf {
    state_dir.join(RUNTIME_DIR_NAME)
}

pub fn tmp_dir(state_dir: &Path) -> PathBuf {
    state_dir.join(TMP_DIR_NAME)
}

fn configured_state_dir_from_env() -> Option<PathBuf> {
    env::var_os(STATE_DIR_ENV).map(|value| expand_user_path(Path::new(&value)))
}

fn default_state_dir() -> Result<PathBuf> {
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        return Ok(home.join(format!(".{DEFAULT_STATE_BASENAME}")));
    }

    let base_dirs =
        BaseDirs::new().context("unable to resolve base directories for current user")?;
    Ok(default_state_dir_for_home(None, base_dirs.data_local_dir()))
}

fn default_state_dir_for_home(home: Option<&Path>, data_local_dir: &Path) -> PathBuf {
    home.map(|home| home.join(format!(".{DEFAULT_STATE_BASENAME}")))
        .unwrap_or_else(|| data_local_dir.join(DEFAULT_STATE_BASENAME))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RepoSyncConfig {
    #[serde(default)]
    pub last_repo: Option<String>,
}

pub fn load_state(state_dir: &Path) -> Result<State> {
    let state_file = state_dir.join("state.json");
    if !state_file.exists() {
        return Ok(State::default());
    }

    let contents = fs::read_to_string(&state_file)
        .with_context(|| format!("failed to read {}", state_file.display()))?;
    let mut state: State = serde_json::from_str(&contents)
        .with_context(|| format!("invalid state file: {}", state_file.display()))?;
    normalize_state_account_paths(state_dir, &mut state);
    drop_legacy_unknown_accounts(&mut state);
    Ok(state)
}

// 清理历史上 read_auth_status 硬编码 "unknown@claude" 时遗留的伪账号。
// 只匹配 email + account_id 两个关键字段都对得上伪造特征的记录，避免误删。
fn drop_legacy_unknown_accounts(state: &mut State) {
    let mut dropped_ids = Vec::new();
    state.accounts.retain(|account| {
        let is_legacy = account.email.eq_ignore_ascii_case("unknown@claude")
            && account.account_id.is_none();
        if is_legacy {
            dropped_ids.push(account.id.clone());
        }
        !is_legacy
    });
    for id in &dropped_ids {
        eprintln!("[sclaude] dropped legacy placeholder account {id}");
        state.usage_cache.remove(id);
        if state.current_account_id.as_deref() == Some(id.as_str()) {
            state.current_account_id = None;
        }
    }
}

pub fn save_state(state_dir: &Path, state: &State) -> Result<()> {
    fs::create_dir_all(state_dir)
        .with_context(|| format!("failed to create {}", state_dir.display()))?;
    let tmp_path = state_dir.join(".state.json.tmp");
    let final_path = state_dir.join("state.json");
    let mut bytes = serde_json::to_vec_pretty(state)?;
    bytes.push(b'\n');
    fs::write(&tmp_path, bytes)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    fs::rename(&tmp_path, &final_path)
        .with_context(|| format!("failed to move {} into place", final_path.display()))?;
    Ok(())
}

pub fn load_repo_sync_config(state_dir: &Path) -> Result<RepoSyncConfig> {
    let config_path = state_dir.join(REPO_SYNC_CONFIG_FILENAME);
    if !config_path.exists() {
        return Ok(RepoSyncConfig::default());
    }

    let contents = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("invalid repo sync config: {}", config_path.display()))
}

pub fn save_repo_sync_config(state_dir: &Path, config: &RepoSyncConfig) -> Result<()> {
    fs::create_dir_all(state_dir)
        .with_context(|| format!("failed to create {}", state_dir.display()))?;
    let tmp_path = state_dir.join(".repo-sync.json.tmp");
    let final_path = state_dir.join(REPO_SYNC_CONFIG_FILENAME);
    let mut bytes = serde_json::to_vec_pretty(config)?;
    bytes.push(b'\n');
    fs::write(&tmp_path, bytes)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    fs::rename(&tmp_path, &final_path)
        .with_context(|| format!("failed to move {} into place", final_path.display()))?;
    Ok(())
}

fn normalize_state_account_paths(state_dir: &Path, state: &mut State) -> bool {
    let mut changed = false;
    let accounts_dir = state_dir.join("accounts");

    for account in &mut state.accounts {
        let canonical_home = accounts_dir.join(&account.id);
        let canonical_auth = canonical_home.join(".claude.json");
        let canonical_config = canonical_home.clone();
        if account.credential_bundle_key.is_none() && !account.id.is_empty() {
            account.credential_bundle_key = Some(format!("claude-bundle-{}", account.id));
            changed = true;
        }

        if canonical_auth.exists() {
            let canonical_auth_str = canonical_auth.to_string_lossy().into_owned();
            if account.auth_path != canonical_auth_str {
                account.auth_path = canonical_auth_str;
                changed = true;
            }
        }

        if canonical_config.exists() {
            let canonical_config_str = canonical_config.to_string_lossy().into_owned();
            if account.config_path.as_deref() != Some(canonical_config_str.as_str()) {
                account.config_path = Some(canonical_config_str);
                changed = true;
            }
        } else if let Some(existing_config) = account.config_path.as_ref() {
            if !Path::new(existing_config).exists() {
                account.config_path = None;
                changed = true;
            }
        }
    }

    changed
}

fn expand_user_path(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(suffix) = raw.strip_prefix("~/") {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(suffix);
        }
    }

    if path.is_absolute() {
        return path.to_path_buf();
    }

    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}

pub fn ensure_exists(path: &Path, label: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    bail!("{label} not found: {}", path.display())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{
        RepoSyncConfig, default_state_dir_for_home, load_repo_sync_config, load_state,
        save_repo_sync_config,
    };

    #[test]
    fn default_state_dir_prefers_home_hidden_directory() {
        let path = default_state_dir_for_home(Some(Path::new("/tmp/home")), Path::new("/tmp/data"));
        assert_eq!(path, Path::new("/tmp/home/.sclaude"));
    }

    #[test]
    fn default_state_dir_falls_back_to_data_directory_without_home() {
        let path = default_state_dir_for_home(None, Path::new("/tmp/data"));
        assert_eq!(path, Path::new("/tmp/data/sclaude"));
    }

    #[test]
    fn managed_subdirectories_stay_under_state_dir() {
        let state_dir = Path::new("/tmp/home/.sclaude");

        assert_eq!(super::bin_dir(state_dir), state_dir.join("bin"));
        assert_eq!(super::runtime_dir(state_dir), state_dir.join("runtime"));
        assert_eq!(super::tmp_dir(state_dir), state_dir.join("tmp"));
    }

    #[test]
    fn load_state_drops_legacy_unknown_placeholder_accounts() {
        let dir = std::env::temp_dir().join(format!("sclaude-legacy-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let state_json = r#"{
            "version": 1,
            "accounts": [
                {
                    "id": "real",
                    "email": "real@example.com",
                    "account_id": "org-real",
                    "auth_path": "",
                    "added_at": 1,
                    "updated_at": 1
                },
                {
                    "id": "legacy",
                    "email": "unknown@claude",
                    "account_id": null,
                    "auth_path": "",
                    "added_at": 2,
                    "updated_at": 2
                }
            ],
            "usage_cache": {
                "legacy": {}
            },
            "current_account_id": "legacy"
        }"#;
        fs::write(dir.join("state.json"), state_json).expect("write state");

        let state = load_state(&dir).expect("load state");

        assert_eq!(state.accounts.len(), 1);
        assert_eq!(state.accounts[0].id, "real");
        assert!(state.current_account_id.is_none());
        assert!(!state.usage_cache.contains_key("legacy"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_state_keeps_unknown_email_when_account_id_present() {
        // 有 account_id 说明身份其实是真的（只是 email 不正常），不应误删
        let dir = std::env::temp_dir().join(format!("sclaude-keep-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let state_json = r#"{
            "version": 1,
            "accounts": [
                {
                    "id": "kept",
                    "email": "unknown@claude",
                    "account_id": "org-present",
                    "auth_path": "",
                    "added_at": 1,
                    "updated_at": 1
                }
            ],
            "usage_cache": {},
            "current_account_id": "kept"
        }"#;
        fs::write(dir.join("state.json"), state_json).expect("write state");

        let state = load_state(&dir).expect("load state");
        assert_eq!(state.accounts.len(), 1);
        assert_eq!(state.current_account_id.as_deref(), Some("kept"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repo_sync_config_round_trip_persists_last_repo() {
        let dir = std::env::temp_dir().join(format!("sclaude-config-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let config = RepoSyncConfig {
            last_repo: Some("git@github.com:org/repo.git".into()),
        };

        save_repo_sync_config(&dir, &config).expect("save config");
        let loaded = load_repo_sync_config(&dir).expect("load config");

        assert_eq!(loaded, config);
        let _ = fs::remove_dir_all(&dir);
    }
}
