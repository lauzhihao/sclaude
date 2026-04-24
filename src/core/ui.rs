use std::env;
use std::path::Path;

use anyhow::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLanguage {
    En,
    ZhHans,
}

#[derive(Debug, Clone, Copy)]
pub struct Messages {
    language: UiLanguage,
}

pub fn messages() -> Messages {
    Messages {
        language: detect_ui_language(),
    }
}

pub fn detect_ui_language() -> UiLanguage {
    let locale = env::var("LC_ALL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("LC_MESSAGES")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            env::var("LANG")
                .ok()
                .filter(|value| !value.trim().is_empty())
        });

    locale
        .as_deref()
        .and_then(parse_ui_language_from_locale)
        .unwrap_or(UiLanguage::En)
}

pub fn parse_ui_language_from_locale(locale: &str) -> Option<UiLanguage> {
    let normalized = locale.trim().to_ascii_lowercase();
    if !normalized.starts_with("zh") {
        return None;
    }
    if normalized.contains("utf-8") || normalized.contains("utf8") {
        Some(UiLanguage::ZhHans)
    } else {
        None
    }
}

pub fn format_top_level_error(error: &Error) -> String {
    let ui = messages();
    let prefix = if ui.is_zh() { "错误" } else { "Error" };
    let chain = error.chain().map(ToString::to_string).collect::<Vec<_>>();
    if chain.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}: {}", chain.join(": "))
    }
}

impl Messages {
    pub fn is_zh(&self) -> bool {
        matches!(self.language, UiLanguage::ZhHans)
    }

    pub fn cli_about(&self) -> &'static str {
        if self.is_zh() {
            "面向代理 CLI 的跨平台账号感知启动器。"
        } else {
            "Cross-platform account-aware launcher for agent CLIs."
        }
    }

    pub fn no_usable_account(&self) -> &'static str {
        if self.is_zh() {
            "没有找到可用账号。"
        } else {
            "No usable account found."
        }
    }

    pub fn no_usable_account_hint(&self) -> &'static str {
        if self.is_zh() {
            "没有可用账号，请先执行 `sclaude add` 添加一个账号。"
        } else {
            "No usable accounts found. Run `sclaude add` to add one first."
        }
    }

    pub fn no_importable_accounts(&self) -> &'static str {
        if self.is_zh() {
            "没有找到可导入的账号。"
        } else {
            "No importable accounts found."
        }
    }

    pub fn added_account(&self, email: &str) -> String {
        if self.is_zh() {
            format!("已添加 {email}")
        } else {
            format!("Added {email}")
        }
    }

    pub fn unknown_account(&self, email: &str) -> String {
        if self.is_zh() {
            format!("未知账号：{email}")
        } else {
            format!("Unknown account: {email}")
        }
    }

    pub fn confirm_rm(&self, email: &str) -> String {
        if self.is_zh() {
            format!("确认删除账号 {email}？此操作不可恢复 (Y/N)：")
        } else {
            format!("Remove account {email}? This cannot be undone (Y/N): ")
        }
    }

    pub fn rm_cancelled(&self) -> &'static str {
        if self.is_zh() {
            "已取消。"
        } else {
            "Cancelled."
        }
    }

    pub fn removed_account(&self, email: &str) -> String {
        if self.is_zh() {
            format!("已移除 {email}")
        } else {
            format!("Removed {email}")
        }
    }

    pub fn rm_requires_tty(&self) -> &'static str {
        if self.is_zh() {
            "当前输入不是终端；请加 -y 跳过确认。"
        } else {
            "Input is not a terminal; pass -y to skip confirmation."
        }
    }

    pub fn refreshed_accounts(&self, count: usize) -> String {
        if self.is_zh() {
            format!("已刷新 {count} 个账号。")
        } else {
            format!("Refreshed {count} account(s).")
        }
    }

    pub fn usable_account_summary(&self, count: usize) -> String {
        if self.is_zh() {
            format!("共有 {count} 个可用账号")
        } else {
            format!("{count} usable account(s)")
        }
    }

    pub fn update_already_current(&self, version: &str, path: &Path) -> String {
        if self.is_zh() {
            format!(
                "当前已是最新已安装版本（{version}），位置：{}",
                path.display()
            )
        } else {
            format!(
                "Already on the latest installed version ({version}) at {}",
                path.display()
            )
        }
    }

    pub fn update_completed(&self, previous: &str, installed: &str, path: &Path) -> String {
        if self.is_zh() {
            format!(
                "已将 sclaude 从 {previous} 更新到 {installed}，位置：{}",
                path.display()
            )
        } else {
            format!(
                "Updated sclaude from {previous} to {installed} at {}",
                path.display()
            )
        }
    }

    pub fn restart_terminal_hint(&self) -> &'static str {
        if self.is_zh() {
            "如果当前终端仍然解析到旧二进制，请重启终端。"
        } else {
            "Restart the current terminal if it still resolves the old binary."
        }
    }

    pub fn imported_account(&self, email: &str, id: &str) -> String {
        if self.is_zh() {
            format!("已导入 {email} -> {id}")
        } else {
            format!("Imported {email} -> {id}")
        }
    }

    pub fn selection_switched(&self) -> &'static str {
        if self.is_zh() {
            "已切换到"
        } else {
            "Switched to"
        }
    }

    pub fn selection_would_select(&self) -> &'static str {
        if self.is_zh() {
            "将会选择"
        } else {
            "Would select"
        }
    }

    pub fn na(&self) -> &'static str {
        "N/A"
    }

    pub fn table_headers(&self) -> [&'static str; 8] {
        if self.is_zh() {
            [
                "当前",
                "邮箱",
                "类型",
                "Token",
                "5h",
                "7d",
                "重置时间",
                "状态",
            ]
        } else {
            [
                "Active", "Email", "Type", "Token", "5h", "7d", "ResetOn", "Status",
            ]
        }
    }

    pub fn official_subscription_label(&self) -> &'static str {
        if self.is_zh() {
            "官方订阅"
        } else {
            "Official"
        }
    }

    pub fn third_party_api_label(&self) -> &'static str {
        if self.is_zh() {
            "第三方API"
        } else {
            "3P API"
        }
    }

    pub fn status_ok(&self) -> &'static str {
        if self.is_zh() { "正常" } else { "OK" }
    }

    pub fn status_error(&self) -> &'static str {
        if self.is_zh() { "错误" } else { "ERROR" }
    }

    pub fn status_relogin(&self) -> &'static str {
        if self.is_zh() { "需重登" } else { "RELOGIN" }
    }

    pub fn login_start(&self) -> &'static str {
        if self.is_zh() {
            "正在启动 `claude auth login --claudeai`。"
        } else {
            "Starting `claude auth login --claudeai`."
        }
    }

    pub fn resume_session(&self) -> &'static str {
        if self.is_zh() {
            "正在恢复当前目录的最新 Claude 会话。"
        } else {
            "Resuming latest Claude session for this directory."
        }
    }

    pub fn resume_fallback(&self) -> &'static str {
        if self.is_zh() {
            "恢复会话未能正常完成，正在回退到新会话。"
        } else {
            "Resume did not complete cleanly; falling back to a fresh Claude session."
        }
    }

    pub fn fresh_session(&self) -> &'static str {
        if self.is_zh() {
            "正在启动新的 Claude 会话。"
        } else {
            "Starting a fresh Claude session."
        }
    }

    pub fn missing_claude(&self) -> &'static str {
        if self.is_zh() {
            "未找到 claude。这会导致 sclaude 无法正常工作。"
        } else {
            "claude not found. This will cause sclaude to behave incorrectly."
        }
    }

    pub fn install_hint(&self) -> &'static str {
        if self.is_zh() {
            "你可以先运行下面的命令安装 Claude Code CLI："
        } else {
            "You can install Claude Code CLI by running:"
        }
    }

    pub fn manual_install(&self) -> &'static str {
        if self.is_zh() {
            "请先手动安装 Claude Code CLI，然后重新运行 sclaude。"
        } else {
            "Please install Claude Code CLI manually and run sclaude again."
        }
    }

    pub fn confirm_install(&self) -> &'static str {
        if self.is_zh() {
            "如果你希望我现在帮你安装，请确认（Y/N）："
        } else {
            "I can try to install it for you now. Continue? (Y/N): "
        }
    }

    pub fn invalid_yes_no(&self) -> &'static str {
        if self.is_zh() {
            "请输入 Y/YES/N/NO。"
        } else {
            "Please answer Y/YES/N/NO."
        }
    }

    pub fn claude_install_still_missing(&self) -> &'static str {
        if self.is_zh() {
            "Claude Code CLI 安装似乎已完成，但当前仍然找不到 `claude`。请重启 shell，或显式设置 CLAUDE_BIN。"
        } else {
            "Claude Code CLI installation completed, but `claude` is still not available. Restart the shell or set CLAUDE_BIN explicitly."
        }
    }

    pub fn claude_install_failed(&self, status: i32) -> String {
        if self.is_zh() {
            format!("Claude Code CLI 安装失败，退出码：{status}")
        } else {
            format!("Claude Code CLI installation failed with status {status}")
        }
    }

    pub fn claude_install_tool_missing(&self, tool: &str) -> String {
        if self.is_zh() {
            format!("未找到 {tool}。要自动安装 Claude Code CLI，当前机器需要先安装 Node.js/npm。")
        } else {
            format!(
                "{tool} not found. Install Node.js/npm first before trying to install Claude Code CLI automatically."
            )
        }
    }

    pub fn claude_login_failed(&self, status: i32) -> String {
        if self.is_zh() {
            format!("claude 登录失败，退出码：{status}")
        } else {
            format!("claude auth login failed with status {status}")
        }
    }

    pub fn setup_token_start(&self) -> &'static str {
        if self.is_zh() {
            "正在启动 `claude setup-token`。完成网页授权后，请复制终端中输出的 OAuth token。"
        } else {
            "Starting `claude setup-token`. After browser authorization, copy the OAuth token printed by Claude."
        }
    }

    pub fn setup_token_prompt(&self) -> &'static str {
        if self.is_zh() {
            "请粘贴 OAuth token："
        } else {
            "Paste OAuth token: "
        }
    }

    pub fn setup_token_required(&self) -> &'static str {
        if self.is_zh() {
            "OAuth token 不能为空，且必须以 sk-ant-oat 开头。"
        } else {
            "OAuth token is required and must start with sk-ant-oat."
        }
    }

    pub fn setup_token_saved(&self) -> &'static str {
        if self.is_zh() {
            "已保存 OAuth token。"
        } else {
            "Saved OAuth token."
        }
    }

    pub fn setup_token_failed(&self, status: i32) -> String {
        if self.is_zh() {
            format!("claude setup-token 失败，退出码：{status}")
        } else {
            format!("claude setup-token failed with status {status}")
        }
    }

    pub fn repo_sync_missing_git(&self, install_command: &str) -> String {
        if self.is_zh() {
            format!(
                "未找到 git。执行 `sclaude push` 或 `sclaude pull` 需要它。请先安装 git，例如：{install_command}"
            )
        } else {
            format!(
                "git not found; `sclaude push` and `sclaude pull` require it. Install git first, for example: {install_command}"
            )
        }
    }

    pub fn repo_sync_invalid_repo(&self) -> &'static str {
        if self.is_zh() {
            "仓库参数不能为空。"
        } else {
            "Repository argument must not be empty."
        }
    }

    pub fn repo_sync_repo_required(&self, env_name: &str) -> String {
        if self.is_zh() {
            format!(
                "未找到账号池仓库地址。请先显式执行一次 `sclaude push <REPO>` 或 `sclaude pull <REPO>`，或设置环境变量 {env_name}。"
            )
        } else {
            format!(
                "No account-pool repository configured. Run `sclaude push <REPO>` or `sclaude pull <REPO>` once, or set {env_name}."
            )
        }
    }

    pub fn repo_sync_push_auth_failed(&self, repo: &str) -> String {
        if self.is_zh() {
            format!(
                "无法写入仓库：{repo}。请检查当前 Git 凭据、SSH key 或 PAT 是否有这个私有仓库的写入权限。"
            )
        } else {
            format!(
                "Cannot write to repository: {repo}. Check whether your current Git credentials, SSH key, or PAT has write access to this private repository."
            )
        }
    }

    pub fn repo_push_no_accounts(&self) -> &'static str {
        if self.is_zh() {
            "当前状态目录里没有账号可推送。"
        } else {
            "No accounts found in the current state directory."
        }
    }

    pub fn repo_push_start(&self, repo: &str) -> String {
        if self.is_zh() {
            format!("正在把本地账号池全量推送到 {repo}")
        } else {
            format!("Pushing the full local account pool to {repo}")
        }
    }

    pub fn repo_push_completed(&self, repo: &str, count: usize) -> String {
        if self.is_zh() {
            format!("已用本地账号池覆盖 {repo}，共 {count} 个账号")
        } else {
            format!("Overwrote {repo} with the local account pool ({count} account(s))")
        }
    }

    pub fn repo_push_no_changes(&self, repo: &str) -> String {
        if self.is_zh() {
            format!("{repo} 里的账号池没有差异，无需推送")
        } else {
            format!("No account-pool changes to push to {repo}")
        }
    }

    pub fn repo_pull_start(&self, repo: &str) -> String {
        if self.is_zh() {
            format!("正在从 {repo} 拉取账号池，并准备覆盖本地")
        } else {
            format!("Pulling the account pool from {repo} and preparing to overwrite local state")
        }
    }

    pub fn repo_pull_missing_bundle(&self, path: &str) -> String {
        if self.is_zh() {
            format!("仓库里没有找到账号池目录：{path}")
        } else {
            format!("Account-pool directory not found in repository: {path}")
        }
    }

    pub fn repo_pull_no_accounts(&self, path: &str) -> String {
        if self.is_zh() {
            format!("账号池目录里没有可导入的账号：{path}")
        } else {
            format!("No importable accounts found in account-pool directory: {path}")
        }
    }

    pub fn repo_pull_completed(&self, repo: &str, count: usize) -> String {
        if self.is_zh() {
            format!("已用 {repo} 的账号池覆盖本地，共 {count} 个账号")
        } else {
            format!("Overwrote the local account pool with {count} account(s) from {repo}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{UiLanguage, parse_ui_language_from_locale};

    #[test]
    fn chinese_utf8_locale_selects_chinese_messages() {
        assert_eq!(
            parse_ui_language_from_locale("zh_CN.UTF-8"),
            Some(UiLanguage::ZhHans)
        );
        assert_eq!(
            parse_ui_language_from_locale("zh_CN.utf8"),
            Some(UiLanguage::ZhHans)
        );
    }

    #[test]
    fn locale_without_utf8_or_without_zh_falls_back_to_english() {
        assert_eq!(parse_ui_language_from_locale("zh_CN.GBK"), None);
        assert_eq!(parse_ui_language_from_locale("en_US.UTF-8"), None);
        assert_eq!(parse_ui_language_from_locale("C"), None);
    }
}
