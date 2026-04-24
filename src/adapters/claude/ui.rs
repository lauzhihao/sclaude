use std::env;
use std::io::{self, IsTerminal};

use chrono::{DateTime, Local, Utc};
use unicode_width::UnicodeWidthStr;

use super::ClaudeAdapter;
use super::usage::{AccountFlavor, account_flavor};
use crate::core::state::{AccountRecord, LiveIdentity, State, UsageSnapshot};
use crate::core::ui as core_ui;

impl ClaudeAdapter {
    pub fn render_account_table(&self, state: &State, active: Option<&LiveIdentity>) -> String {
        let ui = core_ui::messages();
        if state.accounts.is_empty() {
            return ui.no_usable_account_hint().to_string();
        }

        let mut accounts = state.accounts.iter().collect::<Vec<_>>();
        accounts.sort_by(|left, right| left.email.cmp(&right.email));
        let mut usable_count = 0usize;

        let rows = accounts
            .into_iter()
            .map(|account| {
                let usage = state
                    .usage_cache
                    .get(&account.id)
                    .cloned()
                    .unwrap_or_default();
                if account_is_usable(&usage) {
                    usable_count += 1;
                }
                vec![
                    if active.is_some_and(|live| {
                        account.email.eq_ignore_ascii_case(&live.email)
                            || account.account_id.is_some() && account.account_id == live.account_id
                    }) {
                        active_account_marker()
                    } else {
                        String::new()
                    },
                    account.email.clone(),
                    format_account_type(account_flavor(account)),
                    format_token_summary(account),
                    format_quota_percent(usage.five_hour_remaining_percent),
                    format_quota_percent(usage.weekly_remaining_percent),
                    format_reset_on(usage.weekly_refresh_at.as_deref()),
                    format_account_status(&usage),
                ]
            })
            .collect::<Vec<_>>();

        if usable_count == 0 {
            ui.no_usable_account_hint().to_string()
        } else {
            render_table(
                &ui.table_headers(),
                &rows,
                &[
                    "center", "left", "center", "center", "center", "center", "center", "center",
                ],
                Some(ui.usable_account_summary(usable_count)),
            )
        }
    }
}

fn format_token_summary(account: &AccountRecord) -> String {
    let ui = core_ui::messages();
    let Some(token) = account
        .oauth_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return ui.na().into();
    };

    let suffix = token
        .chars()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    let expires = account
        .oauth_token_created_at
        .and_then(|created_at| DateTime::<Utc>::from_timestamp(created_at + 365 * 24 * 60 * 60, 0))
        .map(|expires_at| expires_at.format("%Y%m%d").to_string())
        .unwrap_or_else(|| ui.na().into());

    format!("sk...{suffix} {expires}")
}

fn format_account_type(flavor: AccountFlavor) -> String {
    let ui = core_ui::messages();
    match flavor {
        AccountFlavor::OfficialSubscription => ui.official_subscription_label().into(),
        AccountFlavor::ThirdPartyApi => ui.third_party_api_label().into(),
    }
}

fn format_percent(value: Option<i64>) -> String {
    let ui = core_ui::messages();
    value
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| ui.na().into())
}

fn format_quota_percent(value: Option<i64>) -> String {
    let text = format_percent(value);
    match value {
        Some(value) if value < 20 => style_text(&text, AnsiStyle::Red),
        Some(value) if value < 50 => style_text(&text, AnsiStyle::Yellow),
        Some(_) => style_text(&text, AnsiStyle::Green),
        None => text,
    }
}

fn format_reset_on(value: Option<&str>) -> String {
    let ui = core_ui::messages();
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return ui.na().into();
    };
    if value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("null")
        || value.eq_ignore_ascii_case("n/a")
    {
        return ui.na().into();
    }
    if let Ok(timestamp) = value.parse::<i64>() {
        if let Some(parsed) = DateTime::<Utc>::from_timestamp(timestamp, 0) {
            return parsed
                .with_timezone(&Local)
                .format("%m-%d %H:%M")
                .to_string();
        }
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return parsed
            .with_timezone(&Local)
            .format("%m-%d %H:%M")
            .to_string();
    }
    ui.na().into()
}

fn format_account_status(usage: &UsageSnapshot) -> String {
    let ui = core_ui::messages();
    if usage.needs_relogin {
        style_text(ui.status_relogin(), AnsiStyle::Red)
    } else if usage.last_sync_error.is_some() {
        style_text(ui.status_error(), AnsiStyle::Red)
    } else {
        style_text(ui.status_ok(), AnsiStyle::Green)
    }
}

fn account_is_usable(usage: &UsageSnapshot) -> bool {
    !usage.needs_relogin && usage.last_sync_error.is_none()
}

fn active_account_marker() -> String {
    "✓".into()
}

fn render_table(
    headers: &[&str],
    rows: &[Vec<String>],
    aligns: &[&str],
    summary: Option<String>,
) -> String {
    let widths = headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            rows.iter()
                .map(|row| row.get(index).map_or(0, |value| visible_width(value)))
                .fold(visible_width(header), usize::max)
        })
        .collect::<Vec<_>>();

    let render_border = |left: char, middle: char, right: char| {
        format!(
            "{}{}{}",
            left,
            widths
                .iter()
                .map(|width| "─".repeat(width + 2))
                .collect::<Vec<_>>()
                .join(&middle.to_string()),
            right
        )
    };

    let render_row = |values: Vec<String>| {
        let cells = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| align_cell(value, widths[index], aligns[index]))
            .collect::<Vec<_>>();
        format!("│ {} │", cells.join(" │ "))
    };

    let mut lines = vec![
        render_border('┌', '┬', '┐'),
        render_row(headers.iter().map(|item| (*item).to_string()).collect()),
        render_border('├', '┼', '┤'),
    ];
    for (index, row) in rows.iter().enumerate() {
        lines.push(render_row(row.clone()));
        if index + 1 != rows.len() {
            lines.push(render_border('├', '┼', '┤'));
        }
    }
    if let Some(summary) = summary {
        let total_width = widths.iter().sum::<usize>() + (widths.len() - 1) * 3;
        let total_inner = total_width + 2;
        lines.push(render_border('├', '┴', '┤'));
        let summary = align_cell(summary, total_width, "center");
        lines.push(format!("│ {} │", summary));
        lines.push(format!("└{}┘", "─".repeat(total_inner)));
    } else {
        lines.push(render_border('└', '┴', '┘'));
    }
    lines.join("\n")
}

fn align_cell(value: String, width: usize, align: &str) -> String {
    let value_width = visible_width(&value);
    let padding = width.saturating_sub(value_width);
    match align {
        "left" => format!("{value}{}", " ".repeat(padding)),
        "right" => format!("{}{}", " ".repeat(padding), value),
        "center" => {
            let left = padding / 2;
            let right = padding - left;
            format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
        }
        _ => value,
    }
}

fn visible_width(value: &str) -> usize {
    UnicodeWidthStr::width(strip_ansi_codes(value).as_str())
}

fn strip_ansi_codes(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }
        result.push(ch);
    }
    result
}

fn style_enabled() -> bool {
    io::stdout().is_terminal()
        && env::var_os("NO_COLOR").is_none()
        && !matches!(env::var("TERM").ok().as_deref(), Some("dumb"))
}

#[derive(Debug, Clone, Copy)]
enum AnsiStyle {
    Red,
    Yellow,
    Green,
}

fn style_text(value: &str, style: AnsiStyle) -> String {
    if !style_enabled() {
        return value.to_string();
    }
    let code = match style {
        AnsiStyle::Red => "31",
        AnsiStyle::Yellow => "33",
        AnsiStyle::Green => "32",
    };
    format!("\u{1b}[{code}m{value}\u{1b}[0m")
}

#[cfg(test)]
mod tests {
    use super::{render_table, strip_ansi_codes, visible_width};
    use crate::adapters::claude::ClaudeAdapter;
    use crate::core::state::{AccountRecord, State, UsageSnapshot};

    #[test]
    fn strip_ansi_codes_keeps_visible_width_correct() {
        let styled = "\u{1b}[32m80%\u{1b}[0m";
        assert_eq!(strip_ansi_codes(styled), "80%");
        assert_eq!(visible_width(styled), 3);
    }

    #[test]
    fn table_uses_unicode_borders() {
        let rendered = render_table(
            &["A", "B"],
            &[vec!["1".into(), "2".into()]],
            &["left", "left"],
            Some("1 usable account(s)".into()),
        );
        assert!(rendered.contains('┌'));
        assert!(rendered.contains('┬'));
        assert!(rendered.contains('└'));
        assert!(rendered.contains('│'));
    }

    #[test]
    fn table_can_render_summary_without_rows() {
        let rendered = render_table(&["A", "B"], &[], &["left", "left"], Some("0 usable".into()));
        assert!(rendered.contains("0 usable"));
        assert!(rendered.contains('┌'));
        assert!(rendered.contains('└'));
    }

    #[test]
    fn render_account_table_returns_empty_state_message_without_accounts() {
        let adapter = ClaudeAdapter;
        let rendered = adapter.render_account_table(&State::default(), None);
        assert_eq!(
            rendered,
            crate::core::ui::messages().no_usable_account_hint()
        );
    }

    #[test]
    fn render_account_table_returns_empty_state_message_when_no_account_is_usable() {
        let adapter = ClaudeAdapter;
        let mut state = State::default();
        state.accounts.push(AccountRecord {
            id: "acct-1".into(),
            email: "a@example.com".into(),
            account_kind: Some("oauth".into()),
            auth_path: "/tmp/auth.json".into(),
            oauth_token: Some("sk-ant-oat-exampleabcdef".into()),
            oauth_token_created_at: Some(0),
            ..Default::default()
        });
        state.usage_cache.insert(
            "acct-1".into(),
            UsageSnapshot {
                last_sync_error: Some("quota api failed".into()),
                ..Default::default()
            },
        );

        let rendered = adapter.render_account_table(&state, None);
        assert_eq!(
            rendered,
            crate::core::ui::messages().no_usable_account_hint()
        );
    }

    #[test]
    fn render_account_table_places_summary_inside_table_footer() {
        let adapter = ClaudeAdapter;
        let mut state = State::default();
        state.accounts.push(AccountRecord {
            id: "acct-1".into(),
            email: "a@example.com".into(),
            account_kind: Some("oauth".into()),
            auth_path: "/tmp/auth.json".into(),
            oauth_token: Some("sk-ant-oat-exampleabcdef".into()),
            oauth_token_created_at: Some(0),
            ..Default::default()
        });
        state.usage_cache.insert(
            "acct-1".into(),
            UsageSnapshot {
                five_hour_remaining_percent: Some(100),
                weekly_remaining_percent: Some(47),
                weekly_refresh_at: Some("2026-04-20T15:32:00Z".into()),
                ..Default::default()
            },
        );

        let rendered = adapter.render_account_table(&state, None);
        let lines = rendered.lines().collect::<Vec<_>>();
        let summary = crate::core::ui::messages().usable_account_summary(1);

        assert!(rendered.contains("sk...abcdef"));
        assert!(rendered.contains("19710101"));
        // transition border before summary
        assert_eq!(lines[lines.len() - 3].chars().next(), Some('├'));
        // summary row
        assert_eq!(lines[lines.len() - 2].chars().next(), Some('│'));
        assert!(lines[lines.len() - 2].contains(&summary));
        // clean bottom border (no column dividers)
        assert_eq!(lines.last().and_then(|line| line.chars().next()), Some('└'));
        assert!(!lines.last().unwrap().contains('┴'));
    }

    #[test]
    fn render_account_table_shows_type_column_and_na_for_api_accounts() {
        let adapter = ClaudeAdapter;
        let mut state = State::default();
        state.accounts.push(AccountRecord {
            id: "acct-api".into(),
            email: "key-1234@poe.com".into(),
            account_kind: Some("api".into()),
            auth_path: "/tmp/auth.json".into(),
            ..Default::default()
        });
        state
            .usage_cache
            .insert("acct-api".into(), UsageSnapshot::default());

        let rendered = adapter.render_account_table(&state, None);

        assert!(rendered.contains(crate::core::ui::messages().third_party_api_label()));
        assert!(rendered.contains("key-1234@poe.com"));
        assert!(rendered.contains(crate::core::ui::messages().na()));
        assert!(rendered.contains(crate::core::ui::messages().status_ok()));
    }
}
