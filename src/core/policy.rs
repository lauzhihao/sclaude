#![allow(dead_code)]

use crate::core::state::{AccountRecord, LiveIdentity, State, UsageSnapshot};

pub fn choose_best_account(state: &State) -> Option<&AccountRecord> {
    let mut candidates = state
        .accounts
        .iter()
        .filter(|account| {
            state
                .usage_cache
                .get(&account.id)
                .map(account_is_usable)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then(left.email.cmp(&right.email))
    });
    candidates.into_iter().next()
}

pub fn choose_current_account<'a>(
    state: &'a State,
    live: Option<&LiveIdentity>,
) -> Option<&'a AccountRecord> {
    if let Some(current_id) = state.current_account_id.as_ref() {
        let account = state
            .accounts
            .iter()
            .find(|account| &account.id == current_id)?;
        let usage = state.usage_cache.get(&account.id)?;
        if account_is_usable(usage) {
            return Some(account);
        }
    }

    let live = live?;
    let account = state
        .accounts
        .iter()
        .find(|account| identity_matches(account, live))?;
    let usage = state.usage_cache.get(&account.id)?;
    account_is_usable(usage).then_some(account)
}

pub fn identity_matches(account: &AccountRecord, live: &LiveIdentity) -> bool {
    if account.email.eq_ignore_ascii_case(&live.email) {
        return true;
    }

    match (&account.account_id, &live.account_id) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

pub fn is_current_account_usable(usage: &UsageSnapshot) -> bool {
    account_is_usable(usage)
}

fn account_is_usable(usage: &UsageSnapshot) -> bool {
    !usage.needs_relogin && usage.last_sync_error.is_none()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{choose_best_account, choose_current_account, identity_matches};
    use crate::core::state::{AccountRecord, LiveIdentity, State, UsageSnapshot};

    #[test]
    fn current_account_prefers_explicit_selection() {
        let state = State {
            version: 1,
            current_account_id: Some("a".into()),
            accounts: vec![
                AccountRecord {
                    id: "a".into(),
                    email: "a@example.com".into(),
                    ..AccountRecord::default()
                },
                AccountRecord {
                    id: "b".into(),
                    email: "b@example.com".into(),
                    ..AccountRecord::default()
                },
            ],
            usage_cache: BTreeMap::from([
                ("a".into(), UsageSnapshot::default()),
                ("b".into(), UsageSnapshot::default()),
            ]),
        };

        let current = choose_current_account(&state, None).expect("current");
        assert_eq!(current.id, "a");
    }

    #[test]
    fn best_account_prefers_recent_update() {
        let state = State {
            version: 1,
            current_account_id: None,
            accounts: vec![
                AccountRecord {
                    id: "older".into(),
                    email: "a@example.com".into(),
                    updated_at: 1,
                    ..AccountRecord::default()
                },
                AccountRecord {
                    id: "newer".into(),
                    email: "b@example.com".into(),
                    updated_at: 2,
                    ..AccountRecord::default()
                },
            ],
            usage_cache: BTreeMap::default(),
        };

        let best = choose_best_account(&state).expect("best");
        assert_eq!(best.id, "newer");
    }

    #[test]
    fn identity_match_supports_account_id() {
        assert!(identity_matches(
            &AccountRecord {
                email: "local".into(),
                account_id: Some("org-1".into()),
                ..AccountRecord::default()
            },
            &LiveIdentity {
                email: "remote".into(),
                account_id: Some("org-1".into())
            }
        ));
    }
}
