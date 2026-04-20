use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;

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
        self.read_auth_status_inner(Some(root))
    }

    pub(super) fn read_default_auth_status(&self) -> Result<AuthStatus> {
        self.read_auth_status_inner(None)
    }

    fn read_auth_status_inner(&self, root: Option<&Path>) -> Result<AuthStatus> {
        let claude_bin =
            find_claude_bin().ok_or_else(|| anyhow::anyhow!("claude binary not found"))?;
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
        if !payload.logged_in {
            anyhow::bail!("account is not logged in");
        }

        let email = payload
            .email
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown@claude".into());

        Ok(AuthStatus {
            email,
            org_id: payload.org_id,
            subscription_type: payload.subscription_type,
        })
    }
}

pub(super) fn decode_identity(auth: &Value) -> Result<LiveIdentityWithPlan> {
    let account_id = auth
        .get("userID")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let display = identity_display_name(auth);

    Ok(LiveIdentityWithPlan {
        email: display,
        account_id,
        plan: None,
    })
}

fn identity_display_name(auth: &Value) -> String {
    let host = auth
        .get("ANTHROPIC_BASE_URL")
        .and_then(Value::as_str)
        .and_then(parse_host)
        .unwrap_or_else(|| "claude".into());

    if let Some(user_id) = auth
        .get("userID")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("user-{}@{host}", short_token(user_id));
    }

    if let Some(api_key) = auth
        .get("ANTHROPIC_API_KEY")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("key-{}@{host}", short_token(api_key));
    }

    format!("account@{host}")
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
    pub(super) account_id: Option<String>,
    pub(super) plan: Option<String>,
}

#[derive(Debug)]
pub(super) struct AuthStatus {
    pub(super) email: String,
    pub(super) org_id: Option<String>,
    pub(super) subscription_type: Option<String>,
}

impl AuthStatus {
    pub(super) fn into_identity(self) -> LiveIdentityWithPlan {
        LiveIdentityWithPlan {
            email: self.email,
            account_id: self.org_id,
            plan: self.subscription_type,
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

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::decode_identity;

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
}
