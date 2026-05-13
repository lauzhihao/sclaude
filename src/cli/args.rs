use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;

use crate::adapters::claude::LoginMode;

#[derive(Debug, Args)]
pub struct LaunchArgs {
    #[arg(long)]
    pub no_import_known: bool,
    #[arg(long)]
    pub no_login: bool,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub no_resume: bool,
    #[arg(long)]
    pub no_launch: bool,
    #[arg(trailing_var_arg = true)]
    pub extra_args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub struct AutoArgs {
    #[arg(long)]
    pub no_import_known: bool,
    #[arg(long)]
    pub no_login: bool,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    #[arg(long)]
    pub oauth: bool,
    #[arg(long)]
    pub api: bool,
    #[arg(long)]
    pub username: Option<String>,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long = "provider", value_name = "PROVIDER_ID")]
    pub provider_id: Option<String>,
    #[arg(
        long = "ANTHROPIC_BASE_URL",
        alias = "anthropic-base-url",
        value_name = "URL"
    )]
    pub anthropic_base_url: Option<String>,
    #[arg(
        long = "ANTHROPIC_API_KEY",
        alias = "anthropic-api-key",
        value_name = "KEY"
    )]
    pub anthropic_api_key: Option<String>,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[arg(long)]
    pub switch: bool,
    #[command(flatten)]
    pub login: LoginArgs,
}

#[derive(Debug, Args)]
pub struct RepoSyncArgs {
    #[arg(long, value_name = "REPO_PATH")]
    pub path: Option<String>,

    #[arg(short = 'i', value_name = "IDENTITY_FILE")]
    pub identity_file: Option<PathBuf>,

    #[arg(long)]
    pub all: bool,

    pub repo: Option<String>,
}

#[derive(Debug, Args)]
pub struct UseArgs {
    pub email: String,
}

#[derive(Debug, Args)]
pub struct RmArgs {
    #[arg(short = 'y', long = "yes")]
    pub assume_yes: bool,
    pub email: String,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    #[arg(short = 'f', long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ImportAuthArgs {
    pub path: PathBuf,
}

pub(super) fn resolve_login_mode(args: &LoginArgs) -> Result<LoginMode<'_>> {
    if args.oauth && args.api {
        bail!("--oauth and --api cannot be used together");
    }

    if args.api {
        let provider_id = args
            .provider_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("--api requires --provider"))?;
        let base_url = args
            .anthropic_base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("--api requires --ANTHROPIC_BASE_URL"))?;
        let api_key = args
            .anthropic_api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("--api requires --ANTHROPIC_API_KEY"))?;
        return Ok(LoginMode::Api {
            provider_id,
            base_url,
            api_key,
        });
    }

    Ok(LoginMode::Oauth {
        email_hint: args
            .username
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    })
}
