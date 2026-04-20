use std::path::Path;

use serde_json::Value;

use super::ClaudeAdapter;
use super::paths::profile_root_for_account;
use crate::core::state::{AccountRecord, State, UsageSnapshot};

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
        if let Err(error) = self.switch_account(account) {
            return UsageSnapshot {
                plan: account.plan.clone(),
                last_synced_at: Some(super::now_ts()),
                last_sync_error: Some(error.to_string()),
                needs_relogin: true,
                ..UsageSnapshot::default()
            };
        }

        let profile_root = profile_root_for_account(account);
        match self.read_auth_status(&profile_root) {
            Ok(status) => UsageSnapshot {
                plan: status.subscription_type.or_else(|| account.plan.clone()),
                last_synced_at: Some(super::now_ts()),
                ..UsageSnapshot::default()
            },
            Err(_) if self.profile_uses_api_key(account) => UsageSnapshot {
                plan: account.plan.clone(),
                last_synced_at: Some(super::now_ts()),
                ..UsageSnapshot::default()
            },
            Err(error) => UsageSnapshot {
                plan: account.plan.clone(),
                last_synced_at: Some(super::now_ts()),
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
