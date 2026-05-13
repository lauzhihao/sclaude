// 注意：新增子命令时，必须在 `command_help_topic` 中补一条映射，
// 同时在 `render_help_en` / `render_help_zh` 中加一段对应的帮助文案，
// 否则 `sclaude <new-cmd> --help` 会回退到根帮助。

use std::ffi::OsString;
use std::fmt::Write as _;

use crate::core::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HelpTopic {
    Root,
    Launch,
    Auto,
    Add,
    Login,
    SetToken,
    Push,
    Pull,
    Use,
    Rm,
    List,
    Refresh,
    Update,
    ImportAuth,
    ImportKnown,
}

pub(super) fn requested_help_topic(args: &[OsString]) -> Option<HelpTopic> {
    let tokens = args
        .iter()
        .skip(1)
        .map(|item| item.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let first = tokens.first()?.as_str();

    if matches!(first, "-h" | "--help") {
        return Some(HelpTopic::Root);
    }

    if first == "help" {
        return tokens
            .get(1)
            .and_then(|item| command_help_topic(item))
            .or(Some(HelpTopic::Root));
    }

    let topic = command_help_topic(first)?;
    if tokens
        .iter()
        .skip(1)
        .any(|item| item == "-h" || item == "--help")
    {
        Some(topic)
    } else {
        None
    }
}

fn command_help_topic(name: &str) -> Option<HelpTopic> {
    match name {
        "launch" => Some(HelpTopic::Launch),
        "auto" => Some(HelpTopic::Auto),
        "add" => Some(HelpTopic::Add),
        "login" => Some(HelpTopic::Login),
        "set-token" => Some(HelpTopic::SetToken),
        "push" => Some(HelpTopic::Push),
        "pull" => Some(HelpTopic::Pull),
        "use" => Some(HelpTopic::Use),
        "rm" => Some(HelpTopic::Rm),
        "list" => Some(HelpTopic::List),
        "refresh" => Some(HelpTopic::Refresh),
        "update" | "upgrade" => Some(HelpTopic::Update),
        "import-auth" => Some(HelpTopic::ImportAuth),
        "import-known" => Some(HelpTopic::ImportKnown),
        _ => None,
    }
}

pub(super) fn render_help(topic: HelpTopic) -> String {
    let ui = ui::messages();
    if ui.is_zh() {
        render_help_zh(topic)
    } else {
        render_help_en(topic)
    }
}

fn render_help_en(topic: HelpTopic) -> String {
    let mut out = String::new();
    match topic {
        HelpTopic::Root => {
            writeln!(&mut out, "{}", ui::messages().cli_about()).unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude [OPTIONS] [COMMAND]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Commands:").unwrap();
            writeln!(
                &mut out,
                "  launch       Switch to the best account and launch or resume Claude"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  auto         Switch to the best account without launching Claude"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  add          Add one account through the same login flow as `login`"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  login        Add one account through OAuth or API credentials"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  set-token    Run `claude setup-token` for the selected account"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  push         Push the local account pool into a Git repository"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  pull         Pull an account pool from a Git repository"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  use          Switch directly to a known account by displayed label"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  rm           Remove a stored account by displayed label"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  list         Show stored accounts and latest status"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  refresh      Refresh latest status for all known accounts"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  update       Self-update sclaude [alias: upgrade]"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  import-auth  Import a Claude auth file or profile directory"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  import-known Import the default known auth sources"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  help         Print this message or the help of the given subcommand(s)"
            )
            .unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(
                &mut out,
                "      --state-dir <STATE_DIR>  Override the local state directory"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help                   Print help").unwrap();
        }
        HelpTopic::Launch => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude launch [OPTIONS] [<claude args...>]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(
                &mut out,
                "      --no-import-known  Skip auto-import of known auth sources"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --no-login         Do not start Claude login when no usable account exists"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --dry-run          Show the selected account without switching or launching"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --no-resume        Always start a fresh Claude session"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --no-launch        Switch the account but do not start Claude"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help             Print help").unwrap();
        }
        HelpTopic::Auto => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude auto [OPTIONS]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(
                &mut out,
                "      --no-import-known  Skip auto-import of known auth sources"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --no-login         Do not start Claude login when no usable account exists"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --dry-run          Show the selected account without switching"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help             Print help").unwrap();
        }
        HelpTopic::Add => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude add [OPTIONS]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(
                &mut out,
                "      --oauth                Use Claude official OAuth login"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --api                  Add one API-backed account instead of OAuth"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --provider <PROVIDER_ID>  Required with --api; used for display labels such as key-xxxx@poe.com"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --ANTHROPIC_BASE_URL <URL>  Required with --api"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --ANTHROPIC_API_KEY <KEY>   Required with --api"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --username <EMAIL>     Optional email hint passed to OAuth login"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --password <PASS>      Reserved for compatibility; currently ignored"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --switch               Switch to the newly added account after login"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help                 Print help").unwrap();
        }
        HelpTopic::Login => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude login [OPTIONS]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(
                &mut out,
                "      --oauth                Use Claude official OAuth login"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --api                  Add one API-backed account instead of OAuth"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --provider <PROVIDER_ID>  Required with --api; used for display labels such as key-xxxx@poe.com"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --ANTHROPIC_BASE_URL <URL>  Required with --api"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --ANTHROPIC_API_KEY <KEY>   Required with --api"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --username <EMAIL>     Optional email hint passed to OAuth login"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --password <PASS>      Reserved for compatibility; currently ignored"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help                 Print help").unwrap();
        }
        HelpTopic::SetToken => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude set-token").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(
                &mut out,
                "Runs `claude setup-token` for the selected account and then saves the pasted OAuth token."
            )
            .unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(&mut out, "  -h, --help  Print help").unwrap();
        }
        HelpTopic::Push => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude push [OPTIONS] [REPO]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Arguments:").unwrap();
            writeln!(
                &mut out,
                "  [REPO]  Git remote URL or local repository path; remembered after explicit use"
            )
            .unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(
                &mut out,
                "      --path <REPO_PATH>  Repository subdirectory used for the account pool"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --all               Export all local accounts instead of only reusable OAuth token accounts"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  -i <IDENTITY_FILE>      SSH private key passed to git via GIT_SSH_COMMAND"
            )
            .unwrap();
            writeln!(&mut out, "Environment:").unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_KEY  Symmetric key source for encrypting the account pool"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_PATH Repository subdirectory used for the account pool when --path is omitted"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_REPO Repository used when [REPO] is omitted"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help            Print help").unwrap();
        }
        HelpTopic::Pull => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude pull [OPTIONS] [REPO]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Arguments:").unwrap();
            writeln!(
                &mut out,
                "  [REPO]  Git remote URL or local repository path; remembered after explicit use"
            )
            .unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(
                &mut out,
                "      --path <REPO_PATH>  Repository subdirectory used for the account pool"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  -i <IDENTITY_FILE>      SSH private key passed to git via GIT_SSH_COMMAND"
            )
            .unwrap();
            writeln!(&mut out, "Environment:").unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_KEY  Symmetric key source for decrypting the account pool"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_PATH Repository subdirectory used for the account pool when --path is omitted"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_REPO Repository used when [REPO] is omitted"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help            Print help").unwrap();
        }
        HelpTopic::Use => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude use <EMAIL>").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Arguments:").unwrap();
            writeln!(&mut out, "  <EMAIL>  Account label shown by `list`").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(&mut out, "  -h, --help  Print help").unwrap();
        }
        HelpTopic::Rm => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude rm [OPTIONS] <EMAIL>").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Arguments:").unwrap();
            writeln!(&mut out, "  <EMAIL>  Account label shown by `list`").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(
                &mut out,
                "  -y, --yes   Skip the interactive confirmation prompt"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help  Print help").unwrap();
        }
        HelpTopic::List => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude list").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(&mut out, "  -h, --help  Print help").unwrap();
        }
        HelpTopic::Refresh => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude refresh").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(&mut out, "  -h, --help  Print help").unwrap();
        }
        HelpTopic::Update => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude update [OPTIONS]").unwrap();
            writeln!(&mut out, "  sclaude upgrade [OPTIONS]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(
                &mut out,
                "  -f, --force  Reinstall even when the current version is already latest"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help   Print help").unwrap();
        }
        HelpTopic::ImportAuth => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude import-auth <PATH>").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Arguments:").unwrap();
            writeln!(
                &mut out,
                "  <PATH>  Path to a Claude auth file or a profile directory containing it"
            )
            .unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(&mut out, "  -h, --help  Print help").unwrap();
        }
        HelpTopic::ImportKnown => {
            writeln!(&mut out, "Usage:").unwrap();
            writeln!(&mut out, "  sclaude import-known").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "Options:").unwrap();
            writeln!(&mut out, "  -h, --help  Print help").unwrap();
        }
    }
    out
}

fn render_help_zh(topic: HelpTopic) -> String {
    let mut out = String::new();
    match topic {
        HelpTopic::Root => {
            writeln!(&mut out, "{}", ui::messages().cli_about()).unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude [选项] [命令]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "命令：").unwrap();
            writeln!(
                &mut out,
                "  launch       切换到最佳账号，并启动或恢复 Claude"
            )
            .unwrap();
            writeln!(&mut out, "  auto         切换到最佳账号，但不启动 Claude").unwrap();
            writeln!(
                &mut out,
                "  add          通过与 `login` 相同的流程新增一个账号"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  login        通过 OAuth 或 API 凭据新增一个账号"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  set-token    为当前选中账号执行 `claude setup-token`"
            )
            .unwrap();
            writeln!(&mut out, "  push         把本地账号池推送到 Git 仓库").unwrap();
            writeln!(&mut out, "  pull         从 Git 仓库拉取账号池").unwrap();
            writeln!(&mut out, "  use          按 `list` 中显示的标识切换账号").unwrap();
            writeln!(&mut out, "  rm           按 `list` 中显示的标识删除账号").unwrap();
            writeln!(&mut out, "  list         显示已保存账号及其最新状态").unwrap();
            writeln!(&mut out, "  refresh      刷新所有已知账号的最新状态").unwrap();
            writeln!(&mut out, "  update       自更新 sclaude [别名：upgrade]").unwrap();
            writeln!(&mut out, "  import-auth  导入 Claude 认证文件或配置目录").unwrap();
            writeln!(&mut out, "  import-known 导入默认已知认证来源").unwrap();
            writeln!(&mut out, "  help         显示帮助").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(&mut out, "      --state-dir <STATE_DIR>  覆盖本地状态目录").unwrap();
            writeln!(&mut out, "  -h, --help                   显示帮助").unwrap();
        }
        HelpTopic::Launch => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude launch [选项] [<claude 参数...>]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(
                &mut out,
                "      --no-import-known  跳过自动导入已知认证来源"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --no-login         当没有可用账号时，不自动发起 Claude 登录"
            )
            .unwrap();
            writeln!(&mut out, "      --dry-run          只显示会选中的账号").unwrap();
            writeln!(&mut out, "      --no-resume        总是新开 Claude 会话").unwrap();
            writeln!(
                &mut out,
                "      --no-launch        只切换账号，不启动 Claude"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help             显示帮助").unwrap();
        }
        HelpTopic::Auto => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude auto [选项]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(
                &mut out,
                "      --no-import-known  跳过自动导入已知认证来源"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --no-login         当没有可用账号时，不自动发起 Claude 登录"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --dry-run          只显示会选中的账号，不执行切换"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help             显示帮助").unwrap();
        }
        HelpTopic::Add => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude add [选项]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(
                &mut out,
                "      --oauth                使用 Claude 官方 OAuth 登录"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --api                  添加一个 API 模式账号"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --provider <PROVIDER_ID>  配合 --api 使用；用于显示成 key-xxxx@poe.com 这类标识"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --ANTHROPIC_BASE_URL <URL>  配合 --api 使用"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --ANTHROPIC_API_KEY <KEY>   配合 --api 使用"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --username <EMAIL>     可选，作为 OAuth 登录邮箱提示"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --password <PASS>      兼容保留参数，当前会被忽略"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --switch               登录完成后切换到新账号"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help                 显示帮助").unwrap();
        }
        HelpTopic::Login => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude login [选项]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(
                &mut out,
                "      --oauth                使用 Claude 官方 OAuth 登录"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --api                  添加一个 API 模式账号"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --provider <PROVIDER_ID>  配合 --api 使用；用于显示成 key-xxxx@poe.com 这类标识"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --ANTHROPIC_BASE_URL <URL>  配合 --api 使用"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --ANTHROPIC_API_KEY <KEY>   配合 --api 使用"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --username <EMAIL>     可选，作为 OAuth 登录邮箱提示"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --password <PASS>      兼容保留参数，当前会被忽略"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help                 显示帮助").unwrap();
        }
        HelpTopic::SetToken => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude set-token").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(
                &mut out,
                "为当前选中的账号执行 `claude setup-token`，然后保存你手动粘贴的 OAuth token。"
            )
            .unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(&mut out, "  -h, --help  显示帮助").unwrap();
        }
        HelpTopic::Push => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude push [选项] [REPO]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "参数：").unwrap();
            writeln!(
                &mut out,
                "  [REPO]  Git 远端 URL 或本地仓库路径；显式传入后会记住"
            )
            .unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(
                &mut out,
                "      --path <REPO_PATH>  仓库内用于保存账号池的子目录"
            )
            .unwrap();
            writeln!(
                &mut out,
                "      --all               导出完整本地账号池，而不是只导出可远端复用的 OAuth token 账号"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  -i <IDENTITY_FILE>      通过 GIT_SSH_COMMAND 传给 git 的 SSH 私钥"
            )
            .unwrap();
            writeln!(&mut out, "环境变量：").unwrap();
            writeln!(&mut out, "  SCLAUDE_POOL_KEY  用于加密账号池的对称密钥来源").unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_PATH 未传 --path 时，仓库内账号池子目录来源"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_REPO 未传 [REPO] 时，账号池仓库地址来源"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help            显示帮助").unwrap();
        }
        HelpTopic::Pull => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude pull [选项] [REPO]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "参数：").unwrap();
            writeln!(
                &mut out,
                "  [REPO]  Git 远端 URL 或本地仓库路径；显式传入后会记住"
            )
            .unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(
                &mut out,
                "      --path <REPO_PATH>  仓库内用于保存账号池的子目录"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  -i <IDENTITY_FILE>      通过 GIT_SSH_COMMAND 传给 git 的 SSH 私钥"
            )
            .unwrap();
            writeln!(&mut out, "环境变量：").unwrap();
            writeln!(&mut out, "  SCLAUDE_POOL_KEY  用于解密账号池的对称密钥来源").unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_PATH 未传 --path 时，仓库内账号池子目录来源"
            )
            .unwrap();
            writeln!(
                &mut out,
                "  SCLAUDE_POOL_REPO 未传 [REPO] 时，账号池仓库地址来源"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help            显示帮助").unwrap();
        }
        HelpTopic::Use => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude use <EMAIL>").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "参数：").unwrap();
            writeln!(&mut out, "  <EMAIL>  `list` 中显示的账号标识").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(&mut out, "  -h, --help  显示帮助").unwrap();
        }
        HelpTopic::Rm => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude rm [选项] <EMAIL>").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "参数：").unwrap();
            writeln!(&mut out, "  <EMAIL>  `list` 中显示的账号标识").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(&mut out, "  -y, --yes   跳过交互式二次确认").unwrap();
            writeln!(&mut out, "  -h, --help  显示帮助").unwrap();
        }
        HelpTopic::List => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude list").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(&mut out, "  -h, --help  显示帮助").unwrap();
        }
        HelpTopic::Refresh => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude refresh").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(&mut out, "  -h, --help  显示帮助").unwrap();
        }
        HelpTopic::Update => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude update [选项]").unwrap();
            writeln!(&mut out, "  sclaude upgrade [选项]").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(
                &mut out,
                "  -f, --force  即使当前版本已经最新，也强制重新安装"
            )
            .unwrap();
            writeln!(&mut out, "  -h, --help   显示帮助").unwrap();
        }
        HelpTopic::ImportAuth => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude import-auth <PATH>").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "参数：").unwrap();
            writeln!(
                &mut out,
                "  <PATH>  Claude 认证文件路径，或包含该文件的配置目录"
            )
            .unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(&mut out, "  -h, --help  显示帮助").unwrap();
        }
        HelpTopic::ImportKnown => {
            writeln!(&mut out, "用法：").unwrap();
            writeln!(&mut out, "  sclaude import-known").unwrap();
            writeln!(&mut out).unwrap();
            writeln!(&mut out, "选项：").unwrap();
            writeln!(&mut out, "  -h, --help  显示帮助").unwrap();
        }
    }
    out
}
