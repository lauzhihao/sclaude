# sclaude

[English](./README.md) | [简体中文](./README.zh-CN.md)

`sclaude` 是一个基于 Rust 的 Claude Code CLI wrapper，用来做多账号管理、账号导入、加密账号池同步，以及模型固定入口。

这个仓库只包含代码，不包含账号池数据、额度缓存、本地凭据或机器相关配置。

如果你更想用 GUI 管理账号，可以参考 <https://github.com/murongg/ai-accounts-hub>。

## 安装

Unix:

```bash
curl -fsSL https://raw.githubusercontent.com/lauzhihao/sclaude/main/install.sh | bash
```

Windows PowerShell：

```powershell
irm https://raw.githubusercontent.com/lauzhihao/sclaude/main/install.ps1 | iex
```

当前已发布的预编译目标：

- Linux：`x86_64-unknown-linux-musl`
- macOS：`x86_64-apple-darwin`、`aarch64-apple-darwin`
- Windows：`x86_64-pc-windows-msvc`

安装脚本会：

- 下载最新发布的 `sclaude` 二进制
- 安装 `sclaude` 作为主命令
- 安装 `opus`、`sonnet`、`haiku` 作为模型入口
- 安装 `sclaude-original` 作为到底层 `claude` 的透传辅助命令
- 当检测到 `~/.claude.json`、`~/.config.json`、`~/.claude/.claude.json` 或 `~/.claude/.config.json` 时，自动导入当前 Claude 配置

## 依赖

- Unix 安装器：`bash`、`curl`、`tar`
- Windows 安装器：PowerShell 5+ 或 PowerShell 7+
- `claude` 仍然是 `launch`、`login` 和透传命令的运行时依赖
- 如果本机缺少 `claude`，`sclaude` 会提供通过 `npm` 安装 `@anthropic-ai/claude-code` 的选项
- `push` 和 `pull` 额外依赖 `git` 与 `SCLAUDE_POOL_KEY`

源码构建：

```bash
cargo build --release
```

## 入口命令

- `sclaude`：主命令
- `opus`：固定追加 `--model opus`
- `sonnet`：固定追加 `--model sonnet`
- `haiku`：固定追加 `--model haiku`
- `sclaude-original`：到底层 `claude` 的透传辅助命令

所有运行时入口都会以这些方式启动 Claude：

- 把 `CLAUDE_CONFIG_DIR` 指向选中的受管账号目录
- 设置 `IS_SANDBOX=1`
- 如果你没自己传，则自动补 `--dangerously-skip-permissions`

## 命令总览

| 命令 | 作用 |
| --- | --- |
| `sclaude` | 默认行为，等价于 `sclaude launch` |
| `sclaude launch` | 选择最佳账号，切换后启动或恢复 Claude |
| `sclaude auto` | 只选择最佳账号，不启动 Claude |
| `sclaude login` | 通过官方 OAuth 或 API 凭据添加一个账号，并立即切换过去 |
| `sclaude add` | 用和 `login` 相同的流程添加账号；只有传了 `--switch` 才会切换 |
| `sclaude push <repo>` | 把完整本地账号池加密后推送到 Git 仓库 |
| `sclaude pull <repo>` | 从 Git 仓库拉取并解密账号池，然后覆盖本地状态 |
| `sclaude use <label>` | 按 `list` 中显示的账号标识直接切换 |
| `sclaude rm <label>` | 按 `list` 中显示的账号标识删除一个账号 |
| `sclaude list` | 刷新当前账号状态后渲染账号表格 |
| `sclaude refresh` | 刷新所有已知账号并打印最新表格 |
| `sclaude import-auth <path>` | 导入 Claude 认证文件或 Claude 配置目录 |
| `sclaude import-known` | 导入默认已知的本地 Claude 配置 |
| `sclaude update` | 从 GitHub Releases 自更新 `sclaude`；`upgrade` 是别名 |

## 登录模式

### OAuth

```bash
sclaude login
sclaude login --oauth
sclaude login --oauth --username you@example.com
```

实际行为：

- 在临时受管目录里执行 `claude auth login --claudeai`
- `--username` 只作为传给 Claude 的邮箱提示
- `--password` 仅为兼容保留，当前不会被使用
- 登录成功后，`sclaude login` 总是会切换到新导入的账号

### API

```bash
sclaude login --api \
  --provider poe.com \
  --ANTHROPIC_BASE_URL https://example.com/api/claude \
  --ANTHROPIC_API_KEY sk-ant-xxxx
```

实际行为：

- 会生成一个最小化的受管 Claude 配置，里面包含 `ANTHROPIC_BASE_URL`、`ANTHROPIC_API_KEY` 和 `providerId`
- 账号展示名会显示成 `key-<前缀>@<provider>`
- 会按 `(ANTHROPIC_BASE_URL, ANTHROPIC_API_KEY)` 的实际指纹去重，所以同一组 API 账号重复导入时会更新原记录，而不是新增重复项
- 不同 provider，或者不同的 base URL / key 组合，可以并存

### `add`

```bash
sclaude add [--switch]
sclaude add --api --provider poe.com --ANTHROPIC_BASE_URL ... --ANTHROPIC_API_KEY ...
```

实际行为：

- 使用和 `sclaude login` 完全相同的登录参数与流程
- 与 `login` 的区别只在于：`add` 只有在传入 `--switch` 时才会切换到新账号

## 命令细节

### `launch`

```bash
sclaude launch [--no-import-known] [--no-login] [--dry-run] [--no-resume] [--no-launch] [<claude 参数...>]
```

- 除非传了 `--no-import-known`，否则会先尝试导入本机已知 Claude 配置
- 会刷新状态，并尽量继续使用当前仍然可用的账号
- 如果没有可用账号，且没传 `--no-login`，会回退到 OAuth 登录流程
- `--dry-run` 只显示会选中的账号，不执行切换和启动
- `--no-launch` 只切换账号，不启动 Claude
- 其余参数会继续透传给 Claude

### `auto`

```bash
sclaude auto [--no-import-known] [--no-login] [--dry-run]
```

- 和 `launch` 使用同一套选号逻辑
- 不会启动 Claude

### `use`

```bash
sclaude use <label>
```

- 按 `sclaude list` 中显示的账号标识匹配
- 匹配大小写不敏感

### `rm`

```bash
sclaude rm [-y|--yes] <label>
```

- 从本地状态里删除账号，并清理其受管配置目录
- 默认会要求交互式确认；传 `-y` 则跳过确认

### `list`

```bash
sclaude list
```

- 会先刷新所有已知账号
- 然后输出包含账号标识、plan、额度、重置时间和状态的表格

### `refresh`

```bash
sclaude refresh
```

- 刷新所有已知账号
- 打印刷新数量和最新账号表格

### `import-auth`

```bash
sclaude import-auth <path>
```

- `<path>` 可以是 Claude 认证文件，也可以是包含认证文件的目录
- 导入后会复制到 `sclaude` 自己的受管状态目录里

### `import-known`

```bash
sclaude import-known
```

- 如果设置了 `CLAUDE_CONFIG_DIR`，会直接导入该 live profile
- 否则会从这些默认位置导入本机 Claude 配置：
  - `~/.claude.json`
  - `~/.config.json`
  - `~/.claude/`
- 如果 `claude auth status` 可用，会优先用它识别身份；失败时再回退到本地认证文件解析

### `push`

```bash
export SCLAUDE_POOL_KEY='替换成足够长的随机 secret'
sclaude push [-i <identity_file>] [--path <repo_path>] <repo>
```

- 使用你现有的 Git 凭据克隆仓库
- 把完整本地账号池导出成加密 bundle
- 默认写到 `.sclaude-account-pool/bundle.enc.json`
- 只有加密后的 bundle 发生变化时才会提交并推送
- `--path <repo_path>` 必须是仓库内相对路径
- `-i <identity_file>` 会通过 `GIT_SSH_COMMAND` 把 SSH 私钥传给 Git

### `pull`

```bash
export SCLAUDE_POOL_KEY='替换成和 push 相同的 secret'
sclaude pull [-i <identity_file>] [--path <repo_path>] <repo>
```

- 使用你现有的 Git 凭据克隆仓库
- 解密远端账号池 bundle
- 直接覆盖本地受管账号池，不做 merge
- 导入后会立刻刷新账号状态，并打印最新表格

### `update`

```bash
sclaude update [-f|--force]
sclaude upgrade [-f|--force]
```

- 从 `lauzhihao/sclaude` 的 GitHub Releases 下载当前平台对应资产
- 替换当前 `sclaude` 主二进制
- 同时更新 `opus`、`sonnet`、`haiku` 这些 sidecar binary
- `-f`、`--force` 会在当前版本已经是最新时仍然强制重装

## 透传行为

如果第一个非全局参数不是 `sclaude` 自己声明的子命令，`sclaude` 会在完成账号选择后，把它当成 Claude CLI 子命令继续执行。

例如：

```bash
sclaude auth status
sclaude mcp list
opus auth status
```

这也是为什么 `opus auth status` 可以直接工作，尽管 `auth` 不是 `sclaude` 自己声明的子命令。

## 账号存储说明

- 受管账号保存在 `sclaude` 的本地状态目录下
- 每个账号都会被保存成隔离的 Claude 配置目录
- macOS 上会优先把 credential bundle 存进 Keychain
- 其他平台上会回退到受管账号目录里的本地 bundle 文件
