use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use uuid::Uuid;

use super::ClaudeAdapter;
use super::auth::LiveIdentityWithPlan;
use super::credentials::{
    capture_credential_bundle, credential_bundle_key, credential_bundle_key_for_id,
    delete_credential_bundle, materialize_account_credentials, restore_credential_bundle,
    save_credential_bundle,
};
use super::now_ts;
use super::paths::{
    claude_config_root, default_claude_auth_file, find_claude_auth_file, managed_auth_file,
    profile_root_for_account,
};
use crate::core::state::{AccountRecord, State};
use crate::core::storage;

impl ClaudeAdapter {
    pub fn import_auth_path(
        &self,
        state_dir: &Path,
        state: &mut State,
        raw_path: &Path,
    ) -> Result<AccountRecord> {
        self.import_auth_path_with_id(state_dir, state, raw_path, None)
    }

    pub(super) fn import_auth_path_with_id(
        &self,
        state_dir: &Path,
        state: &mut State,
        raw_path: &Path,
        preferred_id: Option<&str>,
    ) -> Result<AccountRecord> {
        let source_root = resolve_profile_root(raw_path);
        let source_auth = resolve_profile_auth_path(raw_path, &source_root)?;
        let identity = self
            .read_identity_from_profile(&source_root)
            .or_else(|_| self.read_identity_from_auth_file(&source_auth))?;

        self.import_profile_source(
            state_dir,
            state,
            &source_auth,
            Some(&source_root),
            identity,
            preferred_id,
        )
    }

    pub(super) fn import_auth_path_with_identity(
        &self,
        state_dir: &Path,
        state: &mut State,
        source_auth: &Path,
        source_root: Option<&Path>,
        identity: LiveIdentityWithPlan,
    ) -> Result<AccountRecord> {
        self.import_profile_source(state_dir, state, source_auth, source_root, identity, None)
    }

    fn import_profile_source(
        &self,
        state_dir: &Path,
        state: &mut State,
        source_auth: &Path,
        source_root: Option<&Path>,
        identity: LiveIdentityWithPlan,
        preferred_id: Option<&str>,
    ) -> Result<AccountRecord> {
        let source_root = source_root.filter(|path| path.exists());
        let bundle = capture_credential_bundle(source_root, source_auth)?;

        let existing = find_matching_account(
            state,
            &identity.email,
            identity.account_id.as_deref(),
            identity.identity_fingerprint.as_deref(),
        )
        .cloned();
        let account_id = existing
            .as_ref()
            .map(|item| item.id.clone())
            .or_else(|| {
                preferred_id
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let bundle_key = existing
            .as_ref()
            .and_then(|item| item.credential_bundle_key.clone())
            .unwrap_or_else(|| credential_bundle_key_for_id(&account_id));
        let account_home = state_dir.join("accounts").join(&account_id);
        stage_profile_copy(source_root, source_auth, &account_home)?;
        save_credential_bundle(&account_home, &bundle_key, &bundle)?;
        restore_credential_bundle(&account_home, &bundle)?;

        let timestamp = now_ts();
        let record = AccountRecord {
            id: account_id,
            email: identity.email,
            account_kind: identity.account_kind,
            provider_id: identity.provider_id,
            account_id: identity.account_id,
            identity_fingerprint: identity.identity_fingerprint,
            plan: identity.plan,
            auth_path: managed_auth_file(&account_home)
                .to_string_lossy()
                .into_owned(),
            config_path: Some(account_home.to_string_lossy().into_owned()),
            credential_bundle_key: Some(bundle_key),
            added_at: existing.map(|item| item.added_at).unwrap_or(timestamp),
            updated_at: timestamp,
        };

        replace_account(state, record.clone());
        Ok(record)
    }

    pub fn import_known_sources(&self, state_dir: &Path, state: &mut State) -> Vec<AccountRecord> {
        let mut imported = Vec::new();
        let root = claude_config_root();

        if std::env::var_os("CLAUDE_CONFIG_DIR").is_some() {
            if root.exists()
                && let Ok(record) = self.import_auth_path(state_dir, state, &root)
            {
                imported.push(record);
            }
            return imported;
        }

        let Some(source_auth) = default_claude_auth_file() else {
            return imported;
        };
        let identity = self
            .read_default_auth_status()
            .map(|status| status.into_identity())
            .or_else(|_| self.read_identity_from_auth_file(&source_auth));

        if let Ok(identity) = identity
            && let Ok(record) = self.import_profile_source(
                state_dir,
                state,
                &source_auth,
                root.exists().then_some(root.as_path()),
                identity,
                None,
            )
        {
            imported.push(record);
        }
        imported
    }

    pub fn find_account_by_email<'a>(
        &self,
        state: &'a State,
        email: &str,
    ) -> Option<&'a AccountRecord> {
        let target = email.trim().to_ascii_lowercase();
        state
            .accounts
            .iter()
            .find(|account| account.email.eq_ignore_ascii_case(&target))
    }

    pub fn switch_account(&self, account: &AccountRecord) -> Result<()> {
        materialize_account_credentials(account)?;
        storage::ensure_exists(Path::new(&account.auth_path), "managed Claude profile")?;
        Ok(())
    }

    pub fn remove_account(&self, state_dir: &Path, state: &mut State, id: &str) -> Result<()> {
        if let Some(account) = state.accounts.iter().find(|account| account.id == id) {
            let profile_root = profile_root_for_account(account);
            let _ = delete_credential_bundle(&profile_root, &credential_bundle_key(account));
        }
        state.accounts.retain(|account| account.id != id);
        state.usage_cache.remove(id);
        if state.current_account_id.as_deref() == Some(id) {
            state.current_account_id = None;
        }
        let account_home = state_dir.join("accounts").join(id);
        if account_home.exists() {
            fs::remove_dir_all(&account_home)
                .with_context(|| format!("failed to remove {}", account_home.display()))?;
        }
        Ok(())
    }
}

fn resolve_profile_root(raw_path: &Path) -> PathBuf {
    if raw_path.is_dir() {
        raw_path.to_path_buf()
    } else {
        raw_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

fn resolve_profile_auth_path(raw_path: &Path, root: &Path) -> Result<PathBuf> {
    if raw_path.is_file() {
        return Ok(raw_path.to_path_buf());
    }

    find_claude_auth_file(root)
        .ok_or_else(|| anyhow::anyhow!("Claude profile file not found under {}", root.display()))
}

fn stage_profile_copy(
    source_root: Option<&Path>,
    source_auth: &Path,
    account_home: &Path,
) -> Result<()> {
    let staging = account_home
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".{}.tmp",
            account_home
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        ));
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .with_context(|| format!("failed to remove {}", staging.display()))?;
    }
    fs::create_dir_all(&staging)
        .with_context(|| format!("failed to create {}", staging.display()))?;

    copy_profile_contents(source_root, source_auth, &staging)?;

    if account_home.exists() {
        fs::remove_dir_all(account_home)
            .with_context(|| format!("failed to remove {}", account_home.display()))?;
    }
    fs::rename(&staging, account_home)
        .with_context(|| format!("failed to move {} into place", account_home.display()))?;
    Ok(())
}

fn copy_profile_contents(
    source_root: Option<&Path>,
    source_auth: &Path,
    destination_root: &Path,
) -> Result<()> {
    let auth_target = managed_auth_file(destination_root);
    fs::copy(source_auth, &auth_target).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source_auth.display(),
            auth_target.display()
        )
    })?;

    let Some(source_root) = source_root else {
        return Ok(());
    };

    let entries = fs::read_dir(source_root)
        .with_context(|| format!("failed to read {}", source_root.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if should_skip_top_level_entry(name) {
            continue;
        }
        if path == source_auth {
            continue;
        }

        let target = destination_root.join(name);
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else if path.is_file() {
            fs::copy(&path, &target).with_context(|| {
                format!("failed to copy {} to {}", path.display(), target.display())
            })?;
        }
    }

    Ok(())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    let entries =
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else if path.is_file() {
            fs::copy(&path, &target).with_context(|| {
                format!("failed to copy {} to {}", path.display(), target.display())
            })?;
        }
    }
    Ok(())
}

fn should_skip_top_level_entry(name: &str) -> bool {
    matches!(
        name,
        ".claude.json"
            | ".credential-bundle.json"
            | ".credentials.json"
            | ".config.json"
            | "backups"
            | "cache"
            | "debug"
            | "history.jsonl"
            | "paste-cache"
            | "shell-snapshots"
            | "statsig"
            | "telemetry"
            | "todos"
    )
}

fn find_matching_account<'a>(
    state: &'a State,
    email: &str,
    account_id: Option<&str>,
    identity_fingerprint: Option<&str>,
) -> Option<&'a AccountRecord> {
    state.accounts.iter().find(|account| {
        identity_fingerprint
            .is_some_and(|candidate| account.identity_fingerprint.as_deref() == Some(candidate))
            || account.email.eq_ignore_ascii_case(email)
            || account_id.is_some_and(|candidate| account.account_id.as_deref() == Some(candidate))
    })
}

fn replace_account(state: &mut State, updated: AccountRecord) {
    if let Some(slot) = state
        .accounts
        .iter_mut()
        .find(|account| account.id == updated.id)
    {
        *slot = updated;
    } else {
        state.accounts.push(updated);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use anyhow::Result;
    use uuid::Uuid;

    use crate::adapters::claude::ClaudeAdapter;
    use crate::core::state::State;

    #[test]
    fn import_auth_path_copies_profile_into_state_storage() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("sclaude-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp)?;
        let raw_home = tmp.join("raw");
        fs::create_dir_all(&raw_home)?;
        fs::write(
            raw_home.join(".claude.json"),
            serde_json::json!({
                "userID": "acct-1",
                "ANTHROPIC_API_KEY": "sk-ant-123"
            })
            .to_string(),
        )?;
        fs::create_dir_all(raw_home.join("sessions"))?;
        fs::write(raw_home.join("sessions").join("1.json"), "{}")?;

        let state_dir = tmp.join("state");
        let mut state = State::default();
        let adapter = ClaudeAdapter;

        let record = adapter.import_auth_path(&state_dir, &mut state, &raw_home)?;

        assert!(Path::new(&record.auth_path).exists());
        assert!(
            Path::new(record.config_path.as_deref().unwrap_or(""))
                .join("sessions")
                .exists()
        );
        Ok(())
    }
}
