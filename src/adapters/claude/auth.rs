use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::ClaudeAdapter;
use super::paths::{find_claude_auth_file, find_claude_bin};
use crate::core::state::LiveIdentity;
use crate::core::storage;

impl ClaudeAdapter {
    pub(super) fn read_auth_json(&self, path: &Path) -> Result<Value> {
        storage::ensure_exists(path, "Claude profile file")?;
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let auth: Value = serde_json::from_str(&contents)
            .with_context(|| format!("invalid JSON in {}", path.display()))?;
        Ok(auth)
    }

    pub(super) fn read_identity_from_profile(&self, root: &Path) -> Result<LiveIdentityWithPlan> {
        if let Ok(status) = self.read_auth_status(root) {
            return Ok(status.into_identity());
        }

        let auth_path = find_claude_auth_file(root).ok_or_else(|| {
            anyhow::anyhow!("Claude profile file not found under {}", root.display())
        })?;
        self.read_identity_from_auth_file(&auth_path)
    }

    pub(super) fn read_identity_from_auth_file(
        &self,
        auth_path: &Path,
    ) -> Result<LiveIdentityWithPlan> {
        let auth = self.read_auth_json(auth_path)?;
        decode_identity(&auth)
    }

    pub(super) fn read_auth_status(&self, root: &Path) -> Result<AuthStatus> {
        self.read_auth_status_inner(Some(root), None)
    }

    pub(super) fn read_auth_status_with_state(
        &self,
        root: &Path,
        state_dir: &Path,
    ) -> Result<AuthStatus> {
        self.read_auth_status_inner(Some(root), Some(state_dir))
    }

    pub(super) fn read_default_auth_status(&self) -> Result<AuthStatus> {
        self.read_auth_status_inner(None, None)
    }

    fn read_auth_status_inner(
        &self,
        root: Option<&Path>,
        state_dir: Option<&Path>,
    ) -> Result<AuthStatus> {
        let claude_bin =
            find_claude_bin(state_dir).ok_or_else(|| anyhow::anyhow!("claude binary not found"))?;
        let mut command = Command::new(claude_bin);
        command.args(["auth", "status"]);
        if let Some(root) = root {
            command.env("CLAUDE_CONFIG_DIR", root);
        }
        let output = command
            .output()
            .context("failed to execute `claude auth status`")?;
        if !output.status.success() {
            anyhow::bail!(
                "`claude auth status` failed with status {}",
                output.status.code().unwrap_or(1)
            );
        }

        let payload: AuthStatusResponse = serde_json::from_slice(&output.stdout)
            .context("failed to decode `claude auth status` output")?;
        parse_auth_status(payload)
    }
}

// 新版 claude auth status 在 OAuth token 登录下可能只返回 loggedIn/authMethod，
// 不带 email/orgId。过去硬编码回填 "unknown@claude" 会在 import_known_sources 阶段
// 生成一条永远匹配不上的伪账号；这里要求至少有 email 或 orgId，否则上层会 bail
// 然后回退到读 .claude.json 本身识别身份。
fn parse_auth_status(payload: AuthStatusResponse) -> Result<AuthStatus> {
    if !payload.logged_in {
        anyhow::bail!("account is not logged in");
    }

    let email = payload
        .email
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let org_id = payload
        .org_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if email.is_none() && org_id.is_none() {
        anyhow::bail!(
            "`claude auth status` did not return an account identity (no email or orgId)"
        );
    }

    Ok(AuthStatus {
        email,
        org_id,
        subscription_type: payload.subscription_type,
    })
}

pub(super) fn decode_identity(auth: &Value) -> Result<LiveIdentityWithPlan> {
    if !has_auth_identity(auth) {
        anyhow::bail!("Claude profile does not contain account credentials");
    }

    let account_id = auth
        .get("userID")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let display = identity_display_name(auth);
    let account_kind = infer_account_kind(auth);
    let provider_id = provider_id(auth);
    let identity_fingerprint = identity_fingerprint(auth, account_id.as_deref());

    Ok(LiveIdentityWithPlan {
        email: display,
        account_kind,
        provider_id,
        account_id,
        identity_fingerprint,
        plan: None,
    })
}

fn has_auth_identity(auth: &Value) -> bool {
    auth.get("ANTHROPIC_API_KEY")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
        || auth
            .get("userID")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
}

fn identity_display_name(auth: &Value) -> String {
    let provider = provider_id(auth).unwrap_or_else(|| {
        auth.get("ANTHROPIC_BASE_URL")
            .and_then(Value::as_str)
            .and_then(parse_host)
            .unwrap_or_else(|| "claude".into())
    });

    if let Some(user_id) = auth
        .get("userID")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("user-{}@{provider}", short_token(user_id));
    }

    if let Some(api_key) = auth
        .get("ANTHROPIC_API_KEY")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return api_display_name(api_key, &provider);
    }

    format!("account@{provider}")
}

fn infer_account_kind(auth: &Value) -> Option<String> {
    if auth
        .get("ANTHROPIC_API_KEY")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        Some("api".into())
    } else if auth
        .get("userID")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        Some("oauth".into())
    } else {
        None
    }
}

fn provider_id(auth: &Value) -> Option<String> {
    auth.get("providerId")
        .and_then(Value::as_str)
        .or_else(|| auth.get("ANTHROPIC_PROVIDER_ID").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            auth.get("ANTHROPIC_BASE_URL")
                .and_then(Value::as_str)
                .and_then(parse_host)
        })
}

fn identity_fingerprint(auth: &Value, account_id: Option<&str>) -> Option<String> {
    if let Some(api_key) = auth
        .get("ANTHROPIC_API_KEY")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let base_url = auth
            .get("ANTHROPIC_BASE_URL")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        return Some(api_identity_fingerprint(base_url, api_key));
    }

    Some(oauth_identity_fingerprint(
        account_id,
        auth.get("email").and_then(Value::as_str),
    ))
}

fn parse_host(raw: &str) -> Option<String> {
    Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .filter(|value| !value.is_empty())
}

fn short_token(raw: &str) -> String {
    raw.chars().take(12).collect::<String>()
}

#[derive(Debug)]
pub(super) struct LiveIdentityWithPlan {
    pub(super) email: String,
    pub(super) account_kind: Option<String>,
    pub(super) provider_id: Option<String>,
    pub(super) account_id: Option<String>,
    pub(super) identity_fingerprint: Option<String>,
    pub(super) plan: Option<String>,
}

#[derive(Debug)]
pub(super) struct AuthStatus {
    pub(super) email: Option<String>,
    pub(super) org_id: Option<String>,
    pub(super) subscription_type: Option<String>,
}

impl AuthStatus {
    pub(super) fn into_identity(self) -> LiveIdentityWithPlan {
        let AuthStatus {
            email,
            org_id,
            subscription_type,
        } = self;
        // parse_auth_status 已保证 email / org_id 至少一个非空。
        let display = email.clone().unwrap_or_else(|| {
            let short = org_id
                .as_deref()
                .map(short_token)
                .unwrap_or_else(|| "unknown".into());
            format!("org-{short}@claude")
        });
        let identity_fingerprint = oauth_identity_fingerprint(org_id.as_deref(), email.as_deref());

        LiveIdentityWithPlan {
            email: display,
            account_kind: Some("oauth".into()),
            provider_id: None,
            account_id: org_id,
            identity_fingerprint: Some(identity_fingerprint),
            plan: subscription_type,
        }
    }
}

#[derive(Debug, Deserialize)]
struct AuthStatusResponse {
    #[serde(rename = "loggedIn")]
    logged_in: bool,
    email: Option<String>,
    #[serde(rename = "orgId")]
    org_id: Option<String>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

impl From<LiveIdentityWithPlan> for LiveIdentity {
    fn from(value: LiveIdentityWithPlan) -> Self {
        Self {
            email: value.email,
            account_id: value.account_id,
        }
    }
}

pub(super) fn api_display_name(api_key: &str, provider_id: &str) -> String {
    format!(
        "key-{}@{}",
        short_token(api_key.trim()),
        provider_id.trim().to_ascii_lowercase()
    )
}

pub(super) fn normalize_api_provider_id(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        anyhow::bail!("provider id cannot be empty");
    }
    Ok(normalized)
}

pub(super) fn normalize_api_base_url(value: &str) -> Result<String> {
    let trimmed = value.trim();
    let parsed = Url::parse(trimmed).context("invalid ANTHROPIC_BASE_URL")?;
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

pub(super) fn api_identity_fingerprint(base_url: &str, api_key: &str) -> String {
    let normalized_base = base_url.trim().trim_end_matches('/');
    let normalized_key = api_key.trim();
    let mut hasher = Sha256::new();
    hasher.update(normalized_base.as_bytes());
    hasher.update(b"\n");
    hasher.update(normalized_key.as_bytes());
    format!("api:{:x}", hasher.finalize())
}

pub(super) fn oauth_identity_fingerprint(account_id: Option<&str>, email: Option<&str>) -> String {
    if let Some(account_id) = account_id.map(str::trim).filter(|value| !value.is_empty()) {
        return format!("oauth:{account_id}");
    }

    let normalized_email = email
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown@claude")
        .to_ascii_lowercase();
    format!("oauth:{normalized_email}")
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{AuthStatusResponse, decode_identity, parse_auth_status};

    #[test]
    fn decode_identity_prefers_user_id() -> Result<()> {
        let auth = serde_json::json!({
            "userID": "1234567890abcdef",
            "ANTHROPIC_BASE_URL": "https://www.code-cli.cn/api/claudecode"
        });

        let identity = decode_identity(&auth)?;

        assert_eq!(identity.email, "user-1234567890ab@www.code-cli.cn");
        assert_eq!(identity.account_id.as_deref(), Some("1234567890abcdef"));
        Ok(())
    }

    #[test]
    fn decode_identity_accepts_api_key_profiles() -> Result<()> {
        let auth = serde_json::json!({
            "ANTHROPIC_API_KEY": "sk-ant-api03-example",
            "ANTHROPIC_BASE_URL": "https://api.example.com"
        });

        let identity = decode_identity(&auth)?;

        assert_eq!(identity.email, "key-sk-ant-api03@api.example.com");
        assert_eq!(identity.account_kind.as_deref(), Some("api"));
        Ok(())
    }

    #[test]
    fn decode_identity_rejects_plain_settings_without_credentials() {
        let auth = serde_json::json!({
            "hasCompletedOnboarding": true,
            "autoUpdates": true
        });

        assert!(decode_identity(&auth).is_err());
    }

    fn auth_status_payload(
        logged_in: bool,
        email: Option<&str>,
        org_id: Option<&str>,
    ) -> AuthStatusResponse {
        AuthStatusResponse {
            logged_in,
            email: email.map(ToOwned::to_owned),
            org_id: org_id.map(ToOwned::to_owned),
            subscription_type: None,
        }
    }

    #[test]
    fn parse_auth_status_bails_on_minimal_oauth_token_payload() {
        // 新版 claude auth status 仅返回 loggedIn/authMethod/apiProvider 的场景
        let result = parse_auth_status(auth_status_payload(true, None, None));
        assert!(result.is_err());
    }

    #[test]
    fn parse_auth_status_bails_when_logged_out() {
        let result = parse_auth_status(auth_status_payload(false, Some("a@b.com"), None));
        assert!(result.is_err());
    }

    #[test]
    fn parse_auth_status_accepts_email_only_payload() -> Result<()> {
        let status = parse_auth_status(auth_status_payload(true, Some("User@Example.com"), None))?;
        assert_eq!(status.email.as_deref(), Some("user@example.com"));
        assert!(status.org_id.is_none());
        let identity = status.into_identity();
        assert_eq!(identity.email, "user@example.com");
        Ok(())
    }

    #[test]
    fn parse_auth_status_accepts_org_id_only_payload() -> Result<()> {
        let status = parse_auth_status(auth_status_payload(true, None, Some("org-123456789abc")))?;
        assert!(status.email.is_none());
        assert_eq!(status.org_id.as_deref(), Some("org-123456789abc"));
        let identity = status.into_identity();
        // 无 email 时用 org_id 短哈希派生 display，避免写死 unknown@claude
        assert!(
            identity.email.starts_with("org-"),
            "display should derive from org_id, got {}",
            identity.email
        );
        assert!(identity.email.ends_with("@claude"));
        assert_eq!(identity.account_id.as_deref(), Some("org-123456789abc"));
        Ok(())
    }

    #[test]
    fn parse_auth_status_treats_empty_strings_as_missing() {
        let result = parse_auth_status(auth_status_payload(true, Some("   "), Some("")));
        assert!(result.is_err());
    }
}
