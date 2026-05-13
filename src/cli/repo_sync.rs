use std::env;
use std::path::Path;

use anyhow::{Result, bail};

use crate::core::storage;
use crate::core::ui;

pub(super) const REPO_SYNC_REPO_ENV: &str = "SCLAUDE_POOL_REPO";

pub(super) fn resolve_repo_sync_repo(state_dir: &Path, cli_repo: Option<&str>) -> Result<String> {
    if let Some(repo) = cli_repo.map(str::trim).filter(|value| !value.is_empty()) {
        persist_repo_sync_repo(state_dir, repo)?;
        return Ok(repo.to_string());
    }

    let env_repo = env::var(REPO_SYNC_REPO_ENV).ok();
    let config = storage::load_repo_sync_config(state_dir)?;
    if let Some(repo) =
        resolve_repo_sync_repo_source(None, env_repo.as_deref(), config.last_repo.as_deref())
            .map(ToOwned::to_owned)
    {
        return Ok(repo);
    }

    bail!(
        "{}",
        ui::messages().repo_sync_repo_required(REPO_SYNC_REPO_ENV)
    );
}

fn resolve_repo_sync_repo_source<'a>(
    cli_repo: Option<&'a str>,
    env_repo: Option<&'a str>,
    saved_repo: Option<&'a str>,
) -> Option<&'a str> {
    cli_repo
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| env_repo.map(str::trim).filter(|value| !value.is_empty()))
        .or_else(|| saved_repo.map(str::trim).filter(|value| !value.is_empty()))
}

fn persist_repo_sync_repo(state_dir: &Path, repo: &str) -> Result<()> {
    let mut config = storage::load_repo_sync_config(state_dir)?;
    if config.last_repo.as_deref() == Some(repo) {
        return Ok(());
    }
    config.last_repo = Some(repo.to_string());
    storage::save_repo_sync_config(state_dir, &config)
}

pub(super) fn repo_sync_repo_for_pull(state_dir: &Path) -> Result<Option<String>> {
    let env_repo = env::var(REPO_SYNC_REPO_ENV).ok();
    let config = storage::load_repo_sync_config(state_dir)?;
    let repo =
        resolve_repo_sync_repo_source(None, env_repo.as_deref(), config.last_repo.as_deref())
            .map(ToOwned::to_owned);
    let has_key = env::var("SCLAUDE_POOL_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .is_some();
    Ok(repo.filter(|_| has_key))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{persist_repo_sync_repo, repo_sync_repo_for_pull, resolve_repo_sync_repo_source};
    use crate::core::storage;

    #[test]
    fn repo_sync_repo_source_prefers_cli_then_env_then_saved() {
        assert_eq!(
            resolve_repo_sync_repo_source(Some("git@cli"), Some("git@env"), Some("git@saved")),
            Some("git@cli")
        );
        assert_eq!(
            resolve_repo_sync_repo_source(None, Some("git@env"), Some("git@saved")),
            Some("git@env")
        );
        assert_eq!(
            resolve_repo_sync_repo_source(None, None, Some("git@saved")),
            Some("git@saved")
        );
    }

    #[test]
    fn persist_repo_sync_repo_updates_config_file() {
        let state_dir = std::env::temp_dir().join(format!("sclaude-repo-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&state_dir).expect("create temp dir");

        persist_repo_sync_repo(&state_dir, "git@github.com:org/repo.git").expect("persist repo");
        let config = storage::load_repo_sync_config(&state_dir).expect("load config");

        assert_eq!(
            config.last_repo.as_deref(),
            Some("git@github.com:org/repo.git")
        );
        let _ = fs::remove_dir_all(&state_dir);
    }

    #[test]
    fn repo_sync_repo_for_pull_requires_repo_and_key() {
        let state_dir = std::env::temp_dir().join(format!("sclaude-pull-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&state_dir).expect("create temp dir");

        // 这些测试会临时改进程环境变量，作用域被限定在单个测试里。
        unsafe {
            std::env::remove_var("SCLAUDE_POOL_REPO");
            std::env::remove_var("SCLAUDE_POOL_KEY");
        }
        assert!(repo_sync_repo_for_pull(&state_dir).expect("repo").is_none());

        unsafe {
            std::env::set_var("SCLAUDE_POOL_REPO", "git@github.com:org/repo.git");
        }
        assert!(repo_sync_repo_for_pull(&state_dir).expect("repo").is_none());

        unsafe {
            std::env::set_var("SCLAUDE_POOL_KEY", "secret");
        }
        assert_eq!(
            repo_sync_repo_for_pull(&state_dir).expect("repo").as_deref(),
            Some("git@github.com:org/repo.git")
        );

        unsafe {
            std::env::remove_var("SCLAUDE_POOL_REPO");
            std::env::remove_var("SCLAUDE_POOL_KEY");
        }
        let _ = fs::remove_dir_all(&state_dir);
    }
}
