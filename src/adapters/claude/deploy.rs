use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use uuid::Uuid;

use super::ClaudeAdapter;
use super::paths::{find_program, profile_root_for_account};
use crate::core::state::AccountRecord;
use crate::core::storage;
use crate::core::ui as core_ui;

impl ClaudeAdapter {
    pub fn deploy_live_auth(
        &self,
        account: &AccountRecord,
        target: &str,
        identity_file: Option<&Path>,
    ) -> Result<()> {
        let ui = core_ui::messages();
        let source = profile_root_for_account(account);
        if !source.exists() {
            bail!("{}", ui.deploy_missing_auth(&source));
        }

        let Some(ssh_bin) = find_program(ssh_binary_names()) else {
            bail!("{}", ui.deploy_missing_ssh());
        };
        let Some(scp_bin) = find_program(scp_binary_names()) else {
            bail!("{}", ui.deploy_missing_scp());
        };

        let remote = parse_remote_deploy_target(target)?;
        if let Some(identity_file) = identity_file {
            storage::ensure_exists(identity_file, "SSH identity file")
                .map_err(|_| anyhow::anyhow!(ui.deploy_identity_not_found(identity_file)))?;
        }

        println!("{}", ui.deploy_start(&remote.display_target()));
        with_ssh_master_connection(&ssh_bin, identity_file, &remote.host, |master| {
            let ssh_status = Command::new(&ssh_bin)
                .args(master.base_args())
                .args(identity_arg(identity_file))
                .arg(&remote.host)
                .arg(format!(
                    "mkdir -p {}",
                    shell_single_quote(&remote.remote_dir)
                ))
                .status()
                .with_context(|| format!("failed to execute {}", ssh_bin.display()))?;
            if !ssh_status.success() {
                bail!(
                    "{}",
                    ui.deploy_prepare_remote_dir_failed(ssh_status.code().unwrap_or(1))
                );
            }

            let scp_status = Command::new(&scp_bin)
                .args(master.base_args())
                .args(identity_arg(identity_file))
                .arg("-r")
                .arg(&source)
                .arg(remote.scp_destination())
                .status()
                .with_context(|| format!("failed to execute {}", scp_bin.display()))?;
            if !scp_status.success() {
                bail!("{}", ui.deploy_copy_failed(scp_status.code().unwrap_or(1)));
            }

            Ok(())
        })?;

        println!("{}", ui.deploy_completed(&remote.display_target()));
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct RemoteDeployTarget {
    host: String,
    remote_dir: String,
    remote_path: String,
}

impl RemoteDeployTarget {
    fn display_target(&self) -> String {
        format!("{}:{}", self.host, self.remote_path)
    }

    fn scp_destination(&self) -> String {
        format!("{}:{}", self.host, shell_single_quote(&self.remote_path))
    }
}

#[derive(Debug, Clone)]
struct SshMasterConnection {
    ssh_bin: PathBuf,
    host: String,
    control_path: PathBuf,
}

impl SshMasterConnection {
    fn without_control(&self) -> Self {
        Self {
            ssh_bin: self.ssh_bin.clone(),
            host: self.host.clone(),
            control_path: PathBuf::new(),
        }
    }

    fn base_args(&self) -> Vec<std::ffi::OsString> {
        if self.control_path.as_os_str().is_empty() {
            return Vec::new();
        }

        vec![
            "-o".into(),
            "ControlMaster=auto".into(),
            "-o".into(),
            format!("ControlPath={}", self.control_path.display()).into(),
            "-o".into(),
            "ControlPersist=60".into(),
        ]
    }

    fn close(&self, identity_file: Option<&Path>) -> Result<()> {
        if self.control_path.as_os_str().is_empty() || !self.control_path.exists() {
            return Ok(());
        }

        let _ = Command::new(&self.ssh_bin)
            .args(self.base_args())
            .args(identity_arg(identity_file))
            .arg("-O")
            .arg("exit")
            .arg(&self.host)
            .status();
        Ok(())
    }
}

fn ssh_binary_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["ssh.exe", "ssh"]
    } else {
        &["ssh"]
    }
}

fn scp_binary_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["scp.exe", "scp"]
    } else {
        &["scp"]
    }
}

fn identity_arg(identity_file: Option<&Path>) -> Vec<&std::ffi::OsStr> {
    identity_file
        .map(|path| vec![std::ffi::OsStr::new("-i"), path.as_os_str()])
        .unwrap_or_default()
}

fn parse_remote_deploy_target(target: &str) -> Result<RemoteDeployTarget> {
    let ui = core_ui::messages();
    let Some((host, raw_path)) = target.split_once(':') else {
        bail!("{}", ui.deploy_invalid_target(target));
    };
    let host = host.trim();
    let raw_path = raw_path.trim().trim_end_matches('/');
    if host.is_empty() || raw_path.is_empty() {
        bail!("{}", ui.deploy_invalid_target(target));
    }

    let remote_dir = remote_parent_dir(raw_path);
    Ok(RemoteDeployTarget {
        host: host.to_string(),
        remote_dir,
        remote_path: raw_path.to_string(),
    })
}

fn remote_parent_dir(path: &str) -> String {
    if let Some((parent, _)) = path.rsplit_once('/') {
        if parent.is_empty() {
            "/".into()
        } else {
            parent.into()
        }
    } else {
        ".".into()
    }
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'"'"'"#))
}

fn with_ssh_master_connection<F>(
    ssh_bin: &Path,
    identity_file: Option<&Path>,
    host: &str,
    f: F,
) -> Result<()>
where
    F: FnOnce(&SshMasterConnection) -> Result<()>,
{
    let temp_root = env::temp_dir().join(format!("sclaude-ssh-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_root)
        .with_context(|| format!("failed to create {}", temp_root.display()))?;
    let control_path = temp_root.join("control");
    let connection = SshMasterConnection {
        ssh_bin: ssh_bin.to_path_buf(),
        host: host.to_string(),
        control_path,
    };

    let establish = Command::new(ssh_bin)
        .args(connection.base_args())
        .args(identity_arg(identity_file))
        .arg("-MNf")
        .arg(host)
        .status()
        .with_context(|| format!("failed to execute {}", ssh_bin.display()));

    let connection = if matches!(establish.as_ref().map(|status| status.success()), Ok(true)) {
        connection
    } else {
        connection.without_control()
    };

    let result = f(&connection);
    let _ = connection.close(identity_file);
    let _ = fs::remove_dir_all(&temp_root);
    result
}
