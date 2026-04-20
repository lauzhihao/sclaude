use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::state::AccountRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InstallCommand {
    pub(super) program: String,
    pub(super) args: Vec<String>,
}

impl InstallCommand {
    pub(super) fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub(super) fn claude_config_root() -> PathBuf {
    if let Some(root) = env::var_os("CLAUDE_CONFIG_DIR") {
        PathBuf::from(root)
    } else if let Some(home) = env::var_os("HOME") {
        PathBuf::from(home).join(".claude")
    } else {
        PathBuf::from(".claude")
    }
}

pub(super) fn default_claude_auth_file() -> Option<PathBuf> {
    if env::var_os("CLAUDE_CONFIG_DIR").is_some() {
        return find_claude_auth_file(&claude_config_root());
    }

    let home = env::var_os("HOME").map(PathBuf::from)?;
    [home.join(".claude.json"), home.join(".config.json")]
        .into_iter()
        .find(|path| path.exists())
}

pub(super) fn claude_install_command() -> InstallCommand {
    InstallCommand {
        program: npm_command_name().to_string(),
        args: vec![
            "install".into(),
            "-g".into(),
            "@anthropic-ai/claude-code".into(),
        ],
    }
}

fn npm_command_name() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

pub(super) fn find_claude_bin() -> Option<PathBuf> {
    if let Some(env) = env::var_os("CLAUDE_BIN") {
        let path = PathBuf::from(env);
        if path.exists() {
            return Some(path);
        }
    }

    for candidate in claude_binary_names() {
        if let Some(path) = find_in_path(candidate) {
            return Some(path);
        }
    }

    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        for candidate in claude_home_binary_candidates(&home) {
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    npm_global_claude_bin()
}

fn claude_binary_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["claude.cmd", "claude.exe", "claude.bat", "claude"]
    } else {
        &["claude"]
    }
}

fn claude_home_binary_candidates(home: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![
            home.join("AppData")
                .join("Roaming")
                .join("npm")
                .join("claude.cmd"),
            home.join("AppData")
                .join("Roaming")
                .join("npm")
                .join("claude.exe"),
        ]
    } else {
        vec![home.join(".local").join("bin").join("claude")]
    }
}

fn npm_global_claude_bin() -> Option<PathBuf> {
    let npm = if cfg!(windows) {
        find_in_path("npm.cmd")
            .or_else(|| find_in_path("npm.exe"))
            .or_else(|| find_in_path("npm"))
    } else {
        find_in_path("npm")
    }?;

    let output = Command::new(npm).args(["prefix", "-g"]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if prefix.is_empty() {
        return None;
    }

    let prefix = PathBuf::from(prefix);
    let candidates = if cfg!(windows) {
        vec![prefix.join("claude.cmd"), prefix.join("claude.exe")]
    } else {
        vec![prefix.join("bin").join("claude")]
    };

    candidates.into_iter().find(|path| path.exists())
}

pub(super) fn find_claude_auth_file(root: &Path) -> Option<PathBuf> {
    let direct_candidates = [root.join(".config.json"), root.join(".claude.json")];
    for candidate in direct_candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let entries = std::fs::read_dir(root).ok()?;
    let mut matching = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(".claude") && name.ends_with(".json") && path.is_file()
                })
        })
        .collect::<Vec<_>>();
    matching.sort();
    matching.into_iter().next()
}

pub(super) fn managed_auth_file(root: &Path) -> PathBuf {
    root.join(".claude.json")
}

pub(super) fn profile_root_for_account(account: &AccountRecord) -> PathBuf {
    if let Some(path) = account.config_path.as_ref() {
        return PathBuf::from(path);
    }

    Path::new(&account.auth_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(super) fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

pub(super) fn find_program(candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .find_map(|candidate| find_in_path(candidate))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{claude_install_command, find_claude_auth_file};

    #[test]
    fn install_command_uses_official_npm_package() {
        let command = claude_install_command();
        assert!(command.program == "npm" || command.program == "npm.cmd");
        assert_eq!(
            command.args,
            vec!["install", "-g", "@anthropic-ai/claude-code"]
        );
    }

    #[test]
    fn auth_file_discovery_prefers_managed_paths() {
        let root = std::env::temp_dir().join(format!("sclaude-auth-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join(".claude.json"), "{}").expect("auth");

        let found = find_claude_auth_file(&root).expect("auth file");
        assert_eq!(found, root.join(".claude.json"));
    }
}
