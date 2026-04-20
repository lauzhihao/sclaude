use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_json::json;
use uuid::Uuid;

use self::auth::{
    LiveIdentityWithPlan, api_display_name, api_identity_fingerprint, normalize_api_base_url,
    normalize_api_provider_id,
};
use self::paths::{
    claude_config_root, claude_install_command, default_claude_auth_file, find_claude_bin,
    find_in_path, profile_root_for_account,
};
use crate::adapters::{AdapterCapabilities, CliAdapter};
use crate::core::policy::choose_best_account;
use crate::core::state::{AccountRecord, LiveIdentity, State, UsageSnapshot};
use crate::core::ui as core_ui;

mod account;
mod auth;
mod credentials;
mod paths;
mod repo_sync;
mod ui;
mod usage;

#[derive(Debug, Default)]
pub struct ClaudeAdapter;

#[derive(Debug, Clone, Copy)]
pub enum LoginMode<'a> {
    Oauth {
        email_hint: Option<&'a str>,
    },
    Api {
        provider_id: &'a str,
        base_url: &'a str,
        api_key: &'a str,
    },
}

impl CliAdapter for ClaudeAdapter {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            import_known: true,
            read_current_identity: true,
            switch_account: true,
            login: true,
            launch: true,
            resume: true,
            live_usage: true,
        }
    }
}

impl ClaudeAdapter {
    pub fn add_account_via_browser(
        &self,
        state_dir: &Path,
        state: &mut State,
        mode: LoginMode<'_>,
    ) -> Result<AccountRecord> {
        self.run_login_mode(state_dir, state, mode)
    }

    pub fn run_login_mode(
        &self,
        state_dir: &Path,
        state: &mut State,
        mode: LoginMode<'_>,
    ) -> Result<AccountRecord> {
        match mode {
            LoginMode::Oauth { email_hint } => {
                self.run_interactive_login(state_dir, state, email_hint)
            }
            LoginMode::Api {
                provider_id,
                base_url,
                api_key,
            } => self.run_api_key_login(state_dir, state, provider_id, base_url, api_key),
        }
    }

    pub fn read_live_identity(&self) -> Option<LiveIdentity> {
        if env::var_os("CLAUDE_CONFIG_DIR").is_some() {
            return self
                .read_identity_from_profile(&claude_config_root())
                .ok()
                .map(Into::into);
        }

        self.read_default_auth_status()
            .map(|status| status.into_identity())
            .or_else(|_| {
                default_claude_auth_file()
                    .ok_or_else(|| anyhow::anyhow!("default Claude auth file not found"))
                    .and_then(|path| self.read_identity_from_auth_file(&path))
            })
            .ok()
            .map(Into::into)
    }

    pub fn active_identity_from_state(&self, state: &State) -> Option<LiveIdentity> {
        let current_id = state.current_account_id.as_ref()?;
        let account = state
            .accounts
            .iter()
            .find(|account| &account.id == current_id)?;
        Some(LiveIdentity {
            email: account.email.clone(),
            account_id: account.account_id.clone(),
        })
    }

    pub fn ensure_best_account(
        &self,
        state_dir: &Path,
        state: &mut State,
        no_import_known: bool,
        no_login: bool,
        perform_switch: bool,
    ) -> Result<Option<(AccountRecord, UsageSnapshot)>> {
        if !no_import_known {
            self.import_known_sources(state_dir, state);
        }

        if state.accounts.is_empty() {
            if no_login {
                return Ok(None);
            }
            let record = self.run_interactive_login(state_dir, state, None)?;
            let usage = self.refresh_account_usage(state, &record);
            if perform_switch {
                self.switch_account(&record)?;
                state.current_account_id = Some(record.id.clone());
            }
            return Ok(Some((record, usage)));
        }

        self.refresh_all_accounts(state);

        if let Some(current) = state
            .current_account_id
            .as_ref()
            .and_then(|id| state.accounts.iter().find(|account| &account.id == id))
            .cloned()
        {
            let usage = state
                .usage_cache
                .get(&current.id)
                .cloned()
                .unwrap_or_default();
            if !usage.needs_relogin && usage.last_sync_error.is_none() {
                if perform_switch {
                    self.switch_account(&current)?;
                    state.current_account_id = Some(current.id.clone());
                }
                return Ok(Some((current, usage)));
            }
        }

        if let Some(best) = choose_best_account(state).cloned() {
            let usage = state.usage_cache.get(&best.id).cloned().unwrap_or_default();
            if perform_switch {
                self.switch_account(&best)?;
                state.current_account_id = Some(best.id.clone());
            }
            return Ok(Some((best, usage)));
        }

        if let Some(best) = state
            .accounts
            .iter()
            .max_by_key(|account| account.updated_at)
            .cloned()
        {
            let usage = state.usage_cache.get(&best.id).cloned().unwrap_or_default();
            if perform_switch {
                self.switch_account(&best)?;
                state.current_account_id = Some(best.id.clone());
            }
            return Ok(Some((best, usage)));
        }

        if no_login {
            return Ok(None);
        }

        let record = self.run_interactive_login(state_dir, state, None)?;
        let usage = self.refresh_account_usage(state, &record);
        if perform_switch {
            self.switch_account(&record)?;
            state.current_account_id = Some(record.id.clone());
        }
        Ok(Some((record, usage)))
    }

    pub fn run_interactive_login(
        &self,
        state_dir: &Path,
        state: &mut State,
        email_hint: Option<&str>,
    ) -> Result<AccountRecord> {
        let ui = core_ui::messages();
        let claude_bin = self.resolve_claude_bin()?;
        let temp_root = state_dir.join(".tmp");
        fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create {}", temp_root.display()))?;
        let tmp_home = temp_root.join(format!("sclaude-login-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_home)
            .with_context(|| format!("failed to create {}", tmp_home.display()))?;

        println!("{}", ui.login_start());

        let mut command = Command::new(&claude_bin);
        command
            .args(["auth", "login", "--claudeai"])
            .env("CLAUDE_CONFIG_DIR", &tmp_home);
        if let Some(email) = email_hint.map(str::trim).filter(|value| !value.is_empty()) {
            command.arg("--email").arg(email);
        }

        let status = command
            .status()
            .with_context(|| format!("failed to execute {}", claude_bin.display()))?;
        if !status.success() {
            let _ = fs::remove_dir_all(&tmp_home);
            bail!("{}", ui.claude_login_failed(status.code().unwrap_or(1)));
        }

        let record = self.import_auth_path(state_dir, state, &tmp_home)?;
        let _ = fs::remove_dir_all(&tmp_home);
        Ok(record)
    }

    pub fn run_api_key_login(
        &self,
        state_dir: &Path,
        state: &mut State,
        provider_id: &str,
        base_url: &str,
        api_key: &str,
    ) -> Result<AccountRecord> {
        let normalized_provider = normalize_api_provider_id(provider_id)?;
        let normalized_base_url = normalize_api_base_url(base_url)?;
        let normalized_api_key = api_key.trim();
        if normalized_api_key.is_empty() {
            bail!("ANTHROPIC_API_KEY cannot be empty");
        }

        let temp_root = state_dir.join(".tmp");
        fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create {}", temp_root.display()))?;
        let tmp_home = temp_root.join(format!("sclaude-api-login-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_home)
            .with_context(|| format!("failed to create {}", tmp_home.display()))?;

        let auth_path = paths::managed_auth_file(&tmp_home);
        let auth_json = json!({
            "ANTHROPIC_BASE_URL": normalized_base_url,
            "ANTHROPIC_API_KEY": normalized_api_key,
            "providerId": normalized_provider,
        });
        fs::write(&auth_path, serde_json::to_vec_pretty(&auth_json)?)
            .with_context(|| format!("failed to write {}", auth_path.display()))?;

        let identity = LiveIdentityWithPlan {
            email: api_display_name(normalized_api_key, &normalized_provider),
            account_kind: Some("api".into()),
            provider_id: Some(normalized_provider.clone()),
            account_id: None,
            identity_fingerprint: Some(api_identity_fingerprint(
                &normalized_base_url,
                normalized_api_key,
            )),
            plan: None,
        };
        let record = self.import_auth_path_with_identity(
            state_dir,
            state,
            &auth_path,
            Some(&tmp_home),
            identity,
        )?;
        let _ = fs::remove_dir_all(&tmp_home);
        Ok(record)
    }

    pub fn launch_claude(
        &self,
        account: &AccountRecord,
        extra_args: &[OsString],
        resume: bool,
    ) -> Result<i32> {
        let ui = core_ui::messages();
        self.switch_account(account)?;
        let claude_bin = self.resolve_claude_bin()?;
        let fresh_cmd = build_claude_launch_command(&claude_bin, extra_args, false);
        let profile_root = profile_root_for_account(account);

        if resume && !contains_resume_flag(extra_args) {
            let resume_cmd = build_claude_launch_command(&claude_bin, extra_args, true);
            println!("{}", ui.resume_session());
            let status = Command::new(&resume_cmd[0])
                .args(&resume_cmd[1..])
                .env("CLAUDE_CONFIG_DIR", &profile_root)
                .env("IS_SANDBOX", "1")
                .status()
                .context("failed to execute claude continue")?;
            if status.success() {
                return Ok(status.code().unwrap_or(0));
            }
            eprintln!("{}", ui.resume_fallback());
        } else {
            println!("{}", ui.fresh_session());
        }

        let status = Command::new(&fresh_cmd[0])
            .args(&fresh_cmd[1..])
            .env("CLAUDE_CONFIG_DIR", &profile_root)
            .env("IS_SANDBOX", "1")
            .status()
            .context("failed to execute claude")?;
        Ok(status.code().unwrap_or(1))
    }

    pub fn run_passthrough(&self, account: &AccountRecord, extra_args: &[OsString]) -> Result<i32> {
        self.switch_account(account)?;
        let claude_bin = self.resolve_claude_bin()?;
        let profile_root = profile_root_for_account(account);
        let command = build_passthrough_command(&claude_bin, extra_args);
        let status = Command::new(&command[0])
            .args(&command[1..])
            .env("CLAUDE_CONFIG_DIR", &profile_root)
            .env("IS_SANDBOX", "1")
            .status()
            .with_context(|| format!("failed to execute {}", claude_bin.display()))?;
        Ok(status.code().unwrap_or(1))
    }

    pub fn resolve_claude_bin(&self) -> Result<PathBuf> {
        if let Some(path) = find_claude_bin() {
            return Ok(path);
        }

        self.offer_to_install_claude()?;
        find_claude_bin()
            .ok_or_else(|| anyhow::anyhow!(core_ui::messages().claude_install_still_missing()))
    }

    fn offer_to_install_claude(&self) -> Result<()> {
        let install = claude_install_command();
        let install_line = install.display();
        let ui = core_ui::messages();

        eprintln!("{}", ui.missing_claude());
        eprintln!("{}", ui.install_hint());
        eprintln!();
        eprintln!("{install_line}");
        eprintln!();

        let Some(installer_bin) = find_in_path(&install.program) else {
            eprintln!("{}", ui.claude_install_tool_missing(&install.program));
            eprintln!();
            eprintln!("{}", ui.manual_install());
            eprintln!();
            eprintln!("{install_line}");
            std::process::exit(1);
        };

        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            eprintln!("{}", ui.manual_install());
            std::process::exit(1);
        }

        loop {
            print!("{}", ui.confirm_install());
            io::stdout().flush().context("failed to flush stdout")?;

            let mut answer = String::new();
            io::stdin()
                .read_line(&mut answer)
                .context("failed to read confirmation input")?;

            match parse_yes_no(&answer) {
                Some(true) => {
                    let status = Command::new(&installer_bin)
                        .args(&install.args)
                        .status()
                        .with_context(|| format!("failed to execute `{install_line}`"))?;
                    if !status.success() {
                        bail!("{}", ui.claude_install_failed(status.code().unwrap_or(1)));
                    }
                    return Ok(());
                }
                Some(false) => {
                    eprintln!("{}", ui.manual_install());
                    eprintln!();
                    eprintln!("{install_line}");
                    std::process::exit(1);
                }
                None => eprintln!("{}", ui.invalid_yes_no()),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserOpenOutcome {
    Opened,
    NoGui,
    Failed,
}

fn try_open_signup_page(url: &str) -> Result<BrowserOpenOutcome> {
    if requires_gui_hint() && !has_gui_environment() {
        return Ok(BrowserOpenOutcome::NoGui);
    }

    let Some((program, args)) = browser_open_command(url) else {
        return Ok(BrowserOpenOutcome::NoGui);
    };

    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to open browser for {url}"))?;
    if status.success() {
        Ok(BrowserOpenOutcome::Opened)
    } else {
        Ok(BrowserOpenOutcome::Failed)
    }
}

fn requires_gui_hint() -> bool {
    !(cfg!(target_os = "windows") || cfg!(target_os = "macos"))
}

fn has_gui_environment() -> bool {
    if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
        return true;
    }

    env::var_os("DISPLAY").is_some()
        || env::var_os("WAYLAND_DISPLAY").is_some()
        || env::var_os("MIR_SOCKET").is_some()
}

fn browser_open_command(url: &str) -> Option<(&'static str, Vec<String>)> {
    if cfg!(target_os = "macos") {
        return Some(("open", vec![url.to_string()]));
    }
    if cfg!(target_os = "windows") {
        return Some((
            "cmd",
            vec!["/C".into(), "start".into(), "".into(), url.to_string()],
        ));
    }

    if find_in_path("xdg-open").is_some() {
        Some(("xdg-open", vec![url.to_string()]))
    } else if find_in_path("gio").is_some() {
        Some(("gio", vec!["open".into(), url.to_string()]))
    } else {
        None
    }
}

pub(crate) fn parse_yes_no(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => Some(true),
        "n" | "no" => Some(false),
        _ => None,
    }
}

fn build_claude_launch_command(
    claude_bin: &Path,
    extra_args: &[OsString],
    resume: bool,
) -> Vec<OsString> {
    let mut command = vec![claude_bin.as_os_str().to_os_string()];
    if resume {
        command.push("-c".into());
    }
    append_runtime_flags(&mut command, extra_args);
    command.extend(extra_args.iter().cloned());
    command
}

fn build_passthrough_command(claude_bin: &Path, extra_args: &[OsString]) -> Vec<OsString> {
    let mut command = vec![claude_bin.as_os_str().to_os_string()];
    append_runtime_flags(&mut command, extra_args);
    command.extend(extra_args.iter().cloned());
    command
}

fn append_runtime_flags(command: &mut Vec<OsString>, extra_args: &[OsString]) {
    if !has_flag(extra_args, "--dangerously-skip-permissions") {
        command.push("--dangerously-skip-permissions".into());
    }

    if !has_flag(extra_args, "--model")
        && let Some(model) = invoked_model_alias()
    {
        command.push("--model".into());
        command.push(model.into());
    }
}

fn has_flag(args: &[OsString], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn contains_resume_flag(args: &[OsString]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.to_str(), Some("-c" | "--continue" | "-r" | "--resume")))
}

fn invoked_model_alias() -> Option<&'static str> {
    let invoked = env::args_os().next()?;
    let stem = Path::new(&invoked)
        .file_stem()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();

    match stem.as_str() {
        "opus" | "sclaude-opus" => Some("opus"),
        "sonnet" | "sclaude-sonnet" => Some("sonnet"),
        "haiku" | "sclaude-haiku" => Some("haiku"),
        _ => None,
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[allow(dead_code)]
fn detect_local_ip() -> String {
    "127.0.0.1".into()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::{build_claude_launch_command, contains_resume_flag, parse_yes_no};

    #[test]
    fn build_launch_command_adds_model_flags_when_missing() {
        let command = build_claude_launch_command(
            Path::new("/usr/bin/claude"),
            &[OsString::from("agents")],
            true,
        );

        assert_eq!(command[1], OsString::from("-c"));
        assert!(
            command
                .iter()
                .any(|arg| arg == "--dangerously-skip-permissions")
        );
    }

    #[test]
    fn parse_yes_no_accepts_common_answers() {
        assert_eq!(parse_yes_no("y"), Some(true));
        assert_eq!(parse_yes_no("NO"), Some(false));
        assert_eq!(parse_yes_no("maybe"), None);
    }

    #[test]
    fn resume_flag_detection_handles_claude_syntax() {
        assert!(contains_resume_flag(&[OsString::from("-c")]));
        assert!(contains_resume_flag(&[OsString::from("--resume")]));
        assert!(!contains_resume_flag(&[OsString::from("agents")]));
    }
}
