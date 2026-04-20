use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use sha2::{Digest, Sha256};

use super::paths::managed_auth_file;
use crate::core::state::AccountRecord;

const CREDENTIAL_BUNDLE_FILE: &str = ".credential-bundle.json";
const CLAUDE_SECURE_CREDENTIALS_FILE: &str = ".credentials.json";
#[cfg(target_os = "macos")]
const SCLAUDE_BUNDLE_SERVICE: &str = "sclaude.claude-account-bundle";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct ClaudeCredentialBundle {
    pub(super) auth_json: Vec<u8>,
    pub(super) credentials_json: Option<Vec<u8>>,
}

pub(super) fn credential_bundle_key(account: &AccountRecord) -> String {
    account
        .credential_bundle_key
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| credential_bundle_key_for_id(&account.id))
}

pub(super) fn credential_bundle_key_for_id(account_id: &str) -> String {
    format!("claude-bundle-{account_id}")
}

pub(super) fn capture_credential_bundle(
    source_root: Option<&Path>,
    source_auth: &Path,
) -> Result<ClaudeCredentialBundle> {
    let auth_json = fs::read(source_auth)
        .with_context(|| format!("failed to read {}", source_auth.display()))?;
    let credentials_json = source_root
        .filter(|root| root.exists())
        .map(read_secure_credentials)
        .transpose()?
        .flatten();

    Ok(ClaudeCredentialBundle {
        auth_json,
        credentials_json,
    })
}

pub(super) fn save_credential_bundle(
    profile_root: &Path,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] key: &str,
    bundle: &ClaudeCredentialBundle,
) -> Result<()> {
    let payload =
        serde_json::to_vec(bundle).context("failed to serialize Claude credential bundle")?;

    #[cfg(target_os = "macos")]
    {
        security_framework::passwords::set_generic_password(SCLAUDE_BUNDLE_SERVICE, key, &payload)
            .map_err(|error| {
                anyhow::anyhow!("failed to save Claude credential bundle in Keychain: {error}")
            })?;
        let _ = fs::remove_file(profile_root.join(CREDENTIAL_BUNDLE_FILE));
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        fs::create_dir_all(profile_root)
            .with_context(|| format!("failed to create {}", profile_root.display()))?;
        fs::write(profile_root.join(CREDENTIAL_BUNDLE_FILE), payload).with_context(|| {
            format!(
                "failed to write Claude credential bundle under {}",
                profile_root.display()
            )
        })?;
        Ok(())
    }
}

pub(super) fn load_credential_bundle(
    profile_root: &Path,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] key: &str,
) -> Result<Option<ClaudeCredentialBundle>> {
    #[cfg(target_os = "macos")]
    {
        match security_framework::passwords::get_generic_password(SCLAUDE_BUNDLE_SERVICE, key) {
            Ok(payload) => {
                return serde_json::from_slice(&payload)
                    .map(Some)
                    .context("failed to parse Claude credential bundle from Keychain");
            }
            Err(error) if error.code() != -25300 => {
                return Err(anyhow::anyhow!(
                    "failed to load Claude credential bundle from Keychain: {error}"
                ));
            }
            Err(_) => {}
        }
    }

    let path = profile_root.join(CREDENTIAL_BUNDLE_FILE);
    let payload = match fs::read(&path) {
        Ok(payload) => payload,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(anyhow::anyhow!(
                "failed to read Claude credential bundle from {}: {error}",
                path.display()
            ));
        }
    };

    serde_json::from_slice(&payload).map(Some).with_context(|| {
        format!(
            "failed to parse Claude credential bundle from {}",
            path.display()
        )
    })
}

pub(super) fn delete_credential_bundle(
    profile_root: &Path,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] key: &str,
) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        match security_framework::passwords::delete_generic_password(SCLAUDE_BUNDLE_SERVICE, key) {
            Ok(()) => {}
            Err(error) if error.code() == -25300 => {}
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "failed to delete Claude credential bundle from Keychain: {error}"
                ));
            }
        }
    }

    let path = profile_root.join(CREDENTIAL_BUNDLE_FILE);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(anyhow::anyhow!(
            "failed to delete Claude credential bundle {}: {error}",
            path.display()
        )),
    }
}

pub(super) fn restore_credential_bundle(
    profile_root: &Path,
    bundle: &ClaudeCredentialBundle,
) -> Result<()> {
    fs::create_dir_all(profile_root)
        .with_context(|| format!("failed to create {}", profile_root.display()))?;

    let auth_path = managed_auth_file(profile_root);
    fs::write(&auth_path, &bundle.auth_json)
        .with_context(|| format!("failed to write {}", auth_path.display()))?;

    match bundle.credentials_json.as_deref() {
        Some(credentials) => write_secure_credentials(profile_root, credentials),
        None => clear_secure_credentials(profile_root),
    }
}

pub(super) fn materialize_account_credentials(account: &AccountRecord) -> Result<()> {
    let profile_root = super::paths::profile_root_for_account(account);
    let key = credential_bundle_key(account);
    if let Some(bundle) = load_credential_bundle(&profile_root, &key)? {
        restore_credential_bundle(&profile_root, &bundle)?;
    }
    Ok(())
}

fn read_secure_credentials(config_dir: &Path) -> Result<Option<Vec<u8>>> {
    let fallback_path = config_dir.join(CLAUDE_SECURE_CREDENTIALS_FILE);
    match fs::read(&fallback_path) {
        Ok(bytes) => return Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(anyhow::anyhow!(
                "failed to read Claude secure credentials from {}: {error}",
                fallback_path.display()
            ));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let service = claude_keychain_service_name(config_dir);
        let account = current_username();
        match security_framework::passwords::get_generic_password(&service, &account) {
            Ok(payload) => Ok(Some(payload)),
            Err(error) if error.code() == -25300 => Ok(None),
            Err(error) => Err(anyhow::anyhow!(
                "failed to read Claude secure credentials from Keychain service {service}: {error}"
            )),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(None)
    }
}

fn write_secure_credentials(config_dir: &Path, payload: &[u8]) -> Result<()> {
    let fallback_path = config_dir.join(CLAUDE_SECURE_CREDENTIALS_FILE);

    #[cfg(target_os = "macos")]
    {
        let service = claude_keychain_service_name(config_dir);
        let account = current_username();
        match security_framework::passwords::set_generic_password(&service, &account, payload) {
            Ok(()) => {
                let _ = fs::remove_file(&fallback_path);
                return Ok(());
            }
            Err(_) => {
                let _ = security_framework::passwords::delete_generic_password(&service, &account);
            }
        }
    }

    fs::write(&fallback_path, payload)
        .with_context(|| format!("failed to write {}", fallback_path.display()))
}

fn clear_secure_credentials(config_dir: &Path) -> Result<()> {
    let fallback_path = config_dir.join(CLAUDE_SECURE_CREDENTIALS_FILE);
    match fs::remove_file(&fallback_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(anyhow::anyhow!(
                "failed to remove {}: {error}",
                fallback_path.display()
            ));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let service = claude_keychain_service_name(config_dir);
        let account = current_username();
        match security_framework::passwords::delete_generic_password(&service, &account) {
            Ok(()) => {}
            Err(error) if error.code() == -25300 => {}
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "failed to remove Claude secure credentials from Keychain service {service}: {error}"
                ));
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn claude_keychain_service_name(config_dir: &Path) -> String {
    if is_default_system_claude_dir(config_dir) {
        return "Claude Code-credentials".into();
    }

    let hash = Sha256::digest(config_dir.to_string_lossy().as_bytes());
    let mut suffix = String::new();
    for byte in &hash[..4] {
        suffix.push_str(&format!("{byte:02x}"));
    }
    format!("Claude Code-credentials-{suffix}")
}

#[cfg(target_os = "macos")]
fn is_default_system_claude_dir(config_dir: &Path) -> bool {
    std::env::var_os("HOME")
        .map(|home| Path::new(&home).join(".claude") == config_dir)
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn current_username() -> String {
    std::env::var("USER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "claude-code-user".into())
}
