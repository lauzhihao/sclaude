use std::path::Path;

use serde_json::Value;

use super::ClaudeAdapter;
use super::paths::profile_root_for_account;
use crate::core::state::{AccountRecord, State, UsageSnapshot};

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

impl ClaudeAdapter {
    pub fn refresh_all_accounts(&self, state: &mut State) {
        for account in state.accounts.clone() {
            let usage = self.fetch_usage_for_account(&account);
            state.usage_cache.insert(account.id.clone(), usage);
        }
    }

    pub fn refresh_account_usage(
        &self,
        state: &mut State,
        account: &AccountRecord,
    ) -> UsageSnapshot {
        let usage = self.fetch_usage_for_account(account);
        state.usage_cache.insert(account.id.clone(), usage.clone());
        usage
    }

    fn fetch_usage_for_account(&self, account: &AccountRecord) -> UsageSnapshot {
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

        let profile_root = profile_root_for_account(account);
        match self.read_auth_status(&profile_root) {
            Ok(status) => UsageSnapshot {
                plan: status.subscription_type.or_else(|| account.plan.clone()),
                last_synced_at: synced_at,
                ..UsageSnapshot::default()
            },
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
