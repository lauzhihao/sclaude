use std::path::Path;

use anyhow::Result;

use crate::adapters::claude::ClaudeAdapter;
use crate::core::state::{AccountRecord, State, UsageSnapshot};
use crate::core::ui;

use super::repo_sync::repo_sync_repo_for_pull;

pub(super) fn format_percent(value: Option<i64>) -> String {
    let ui = ui::messages();
    value
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| ui.na().into())
}

pub(super) fn print_selection(prefix: &str, account: &AccountRecord, usage: &UsageSnapshot) {
    println!(
        "{} {} [weekly={}, 5h={}]",
        prefix,
        account.email,
        format_percent(usage.weekly_remaining_percent),
        format_percent(usage.five_hour_remaining_percent),
    );
}

pub(super) fn ensure_launch_account(
    adapter: &ClaudeAdapter,
    state_dir: &Path,
    state: &mut State,
    no_import_known: bool,
    no_login: bool,
    perform_switch: bool,
) -> Result<Option<(AccountRecord, UsageSnapshot, bool)>> {
    let mut pulled = false;
    if let Some((account, usage)) =
        adapter.ensure_best_account(state_dir, state, no_import_known, no_login, perform_switch)?
    {
        return Ok(Some((account, usage, pulled)));
    }

    if !no_import_known {
        adapter.import_known_sources(state_dir, state);
    }
    if let Some(repo) = repo_sync_repo_for_pull(state_dir)? {
        adapter.pull_account_pool(state_dir, state, &repo, None, None)?;
        pulled = true;
        if let Some((account, usage)) =
            adapter.ensure_best_account(state_dir, state, true, no_login, perform_switch)?
        {
            return Ok(Some((account, usage, pulled)));
        }
    }

    Ok(None)
}
