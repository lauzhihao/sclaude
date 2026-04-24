use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(target_os = "macos")]
use sha2::Digest;
#[cfg(target_os = "macos")]
use sha2::Sha256;

use super::ClaudeAdapter;
use super::paths::profile_root_for_account;
use crate::core::state::{AccountRecord, State, UsageSnapshot};

// API 响应结构体
#[derive(Debug, Clone, Deserialize, Serialize)]
struct OauthUsageResponse {
    five_hour: Option<OauthUsageSlot>,
    seven_day: Option<OauthUsageSlot>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OauthUsageSlot {
    utilization: f64,
    #[serde(rename = "resets_at")]
    resets_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccountFlavor {
    OfficialSubscription,
    ThirdPartyApi,
}

pub(super) fn account_flavor(account: &AccountRecord) -> AccountFlavor {
    match account.account_kind.as_deref() {
        Some("api") => AccountFlavor::ThirdPartyApi,
        Some("oauth") => AccountFlavor::OfficialSubscription,
        _ if account.provider_id.is_some() && account.account_id.is_none() => {
            AccountFlavor::ThirdPartyApi
        }
        _ => AccountFlavor::OfficialSubscription,
    }
}

// OAuth token 读取：从 {profile_root}/.credentials.json 读取 .claudeAiOauth.accessToken
// 在 macOS 上，如果文件不存在，尝试从 Keychain 读取（迁移兼容性）
fn read_oauth_token(profile_root: &Path) -> Option<String> {
    let cred_path = profile_root.join(".credentials.json");

    // 优先读文件
    if let Ok(content) = fs::read_to_string(&cred_path) {
        if let Ok(json) = serde_json::from_str::<Value>(&content) {
            if let Some(token) = json
                .get("claudeAiOauth")
                .and_then(|oauth| oauth.get("accessToken"))
                .and_then(Value::as_str)
            {
                return Some(token.to_string());
            }
        }
    }

    // macOS 后备：如果文件不存在，尝试从 Keychain 读取（为了兼容之前的存储方式）
    #[cfg(target_os = "macos")]
    {
        let service = if is_default_system_claude_dir(profile_root) {
            "Claude Code-credentials".into()
        } else {
            let hash = Sha256::digest(profile_root.to_string_lossy().as_bytes());
            let mut suffix = String::new();
            for byte in &hash[..4] {
                suffix.push_str(&format!("{byte:02x}"));
            }
            format!("Claude Code-credentials-{suffix}")
        };

        let account = std::env::var("USER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "claude-code-user".into());

        if let Ok(payload) = security_framework::passwords::get_generic_password(&service, &account)
        {
            if let Ok(json) = serde_json::from_slice::<Value>(&payload) {
                if let Some(token) = json
                    .get("claudeAiOauth")
                    .and_then(|oauth| oauth.get("accessToken"))
                    .and_then(Value::as_str)
                {
                    return Some(token.to_string());
                }
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn is_default_system_claude_dir(config_dir: &Path) -> bool {
    std::env::var_os("HOME")
        .map(|home| std::path::Path::new(&home).join(".claude") == config_dir)
        .unwrap_or(false)
}

// OAuth usage API 查询
fn fetch_oauth_usage(token: &str) -> Result<OauthUsageResponse> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token))?,
    );
    headers.insert(
        "anthropic-beta",
        HeaderValue::from_static("oauth-2025-04-20"),
    );

    let client = Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(10))
        .build()?;

    let response = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .send()?
        .error_for_status()?
        .json::<OauthUsageResponse>()?;

    Ok(response)
}

impl ClaudeAdapter {
    pub fn refresh_all_accounts(&self, state_dir: &Path, state: &mut State) {
        for account in state.accounts.clone() {
            let usage = self.fetch_usage_for_account(state_dir, &account);
            state.usage_cache.insert(account.id.clone(), usage);
        }
    }

    pub fn refresh_account_usage(
        &self,
        state_dir: &Path,
        state: &mut State,
        account: &AccountRecord,
    ) -> UsageSnapshot {
        let usage = self.fetch_usage_for_account(state_dir, account);
        state.usage_cache.insert(account.id.clone(), usage.clone());
        usage
    }

    fn fetch_usage_for_account(&self, state_dir: &Path, account: &AccountRecord) -> UsageSnapshot {
        let synced_at = Some(super::now_ts());
        if let Err(error) = self.switch_account(account) {
            return UsageSnapshot {
                plan: account.plan.clone(),
                last_synced_at: synced_at,
                last_sync_error: Some(error.to_string()),
                needs_relogin: true,
                ..UsageSnapshot::default()
            };
        }

        // 官方订阅账号才有 Claude subscription 状态可查询，API 账号没有 5h/7d 配额语义。
        if matches!(account_flavor(account), AccountFlavor::ThirdPartyApi) {
            return UsageSnapshot {
                plan: account.plan.clone(),
                last_synced_at: synced_at,
                ..UsageSnapshot::default()
            };
        }

        if let Some(token) = account_oauth_token(account) {
            let mut result = UsageSnapshot {
                plan: account.plan.clone(),
                last_synced_at: synced_at,
                ..UsageSnapshot::default()
            };
            match fetch_oauth_usage(token) {
                Ok(usage) => apply_oauth_usage(&mut result, usage),
                Err(error) => {
                    result.last_sync_error = Some(error.to_string());
                    result.needs_relogin = true;
                }
            }
            return result;
        }

        let profile_root = profile_root_for_account(account);
        match self.read_auth_status_with_state(&profile_root, state_dir) {
            Ok(status) => {
                let mut result = UsageSnapshot {
                    plan: status.subscription_type.or_else(|| account.plan.clone()),
                    last_synced_at: synced_at,
                    ..UsageSnapshot::default()
                };

                // 尝试获取实时 OAuth usage 配额信息
                if let Some(token) = read_oauth_token(&profile_root) {
                    match fetch_oauth_usage(&token) {
                        Ok(usage) => apply_oauth_usage(&mut result, usage),
                        Err(_) => {
                            // 配额接口失败（网络错误、429 频率限制等）属于暂时性问题，
                            // 不污染 last_sync_error，避免账号被错误标记为不可用。
                            // 配额字段保持 None，显示为 N/A。
                        }
                    }
                }

                result
            }
            Err(_) if self.profile_uses_api_key(account) => UsageSnapshot {
                plan: account.plan.clone(),
                last_synced_at: synced_at,
                ..UsageSnapshot::default()
            },
            Err(error) => UsageSnapshot {
                plan: account.plan.clone(),
                last_synced_at: synced_at,
                last_sync_error: Some(error.to_string()),
                needs_relogin: true,
                ..UsageSnapshot::default()
            },
        }
    }

    fn profile_uses_api_key(&self, account: &AccountRecord) -> bool {
        self.read_auth_json(Path::new(&account.auth_path))
            .ok()
            .is_some_and(|auth| {
                auth.get("ANTHROPIC_API_KEY")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty())
            })
    }
}

fn account_oauth_token(account: &AccountRecord) -> Option<&str> {
    account
        .oauth_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn apply_oauth_usage(result: &mut UsageSnapshot, usage: OauthUsageResponse) {
    if let Some(slot) = usage.five_hour {
        result.five_hour_remaining_percent = Some((100.0 - slot.utilization).round() as i64);
        result.five_hour_refresh_at = slot.resets_at;
    }
    if let Some(slot) = usage.seven_day {
        result.weekly_remaining_percent = Some((100.0 - slot.utilization).round() as i64);
        result.weekly_refresh_at = slot.resets_at;
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountFlavor, account_flavor};
    use crate::core::state::AccountRecord;

    #[test]
    fn account_flavor_uses_explicit_account_kind() {
        let oauth = AccountRecord {
            account_kind: Some("oauth".into()),
            ..AccountRecord::default()
        };
        let api = AccountRecord {
            account_kind: Some("api".into()),
            ..AccountRecord::default()
        };

        assert_eq!(account_flavor(&oauth), AccountFlavor::OfficialSubscription);
        assert_eq!(account_flavor(&api), AccountFlavor::ThirdPartyApi);
    }

    #[test]
    fn account_flavor_falls_back_to_provider_shape_for_legacy_api_records() {
        let legacy_api = AccountRecord {
            provider_id: Some("poe.com".into()),
            account_id: None,
            ..AccountRecord::default()
        };

        assert_eq!(account_flavor(&legacy_api), AccountFlavor::ThirdPartyApi);
    }
}
