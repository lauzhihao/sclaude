# Task 002

## 标题

梳理 `Claude Code` 的 auth / identity / usage / model 参数能力矩阵

## 状态

Backlog

## 优先级

High

## 来源任务

- `task-001`

## 任务目标

基于官方文档与本机只读验证，输出 `ClaudeCodeAdapter` 的能力边界与实现建议，为后续 `opus` / `sonnet` / `haiku` 工具族扩展提供直接可执行的输入。

本任务的交付目标不是实现代码，而是回答以下问题：

- `claude` 是否原生支持 `opus` / `sonnet` / `haiku` 三个模型入口
- `Claude Code` 的 auth 与 identity 能力是否足以支撑多账号 wrapper
- `Claude Code` 的 usage / quota 语义是否足以映射 `scodex` 的选号逻辑
- 哪些能力已经可以进入实现
- 哪些能力仍需 PoC 或脚本级验证

## 调研范围

### 本地仓库实现

- `ARCHITECTURE.md`
- `src/cli.rs`
- `src/adapters/mod.rs`
- `src/adapters/codex/mod.rs`
- `src/adapters/codex/account.rs`
- `src/adapters/codex/usage.rs`
- `src/core/state.rs`
- `src/core/policy.rs`
- `src/core/storage.rs`
- `src/core/update.rs`
- `install.sh`
- `install.ps1`

### 外部官方资料

本任务于 `2026-04-20` 调研以下官方资料：

- Claude Code Docs
- Claude Help Center

重点覆盖：

- Quickstart
- CLI reference
- Authentication / Credential management
- Model configuration
- Error reference
- Status line
- Cost / usage / extra usage

### 本机只读验证

仅做了低风险只读验证：

- `claude --help`
- `claude auth --help`
- `command -v claude`
- `~/.claude` 目录结构
- `~/.claude/settings.json` 的顶层键

未读取任何 secret 值，未打印 token 内容，未修改本机 Claude 配置。

## 已确认结论

### 1. `claude` 原生支持模型选择，且 `opus` / `sonnet` / `haiku` 是官方概念

已确认：

- `claude` CLI 原生支持 `--model <model>`
- 官方文档明确存在 `opus`、`sonnet`、`haiku` 模型 alias
- 官方文档明确存在：
  - `ANTHROPIC_DEFAULT_OPUS_MODEL`
  - `ANTHROPIC_DEFAULT_SONNET_MODEL`
  - `ANTHROPIC_DEFAULT_HAIKU_MODEL`

结论：

- `opus` / `sonnet` / `haiku` 不需要伪造为“业务命名”，它们本身就是 Claude 官方模型入口语义
- 后续三入口工具最合理的实现方式是：
  - 同一个 `ClaudeCodeAdapter`
  - 三个 wrapper profile
  - 启动时向底层 `claude` 注入不同默认模型 alias 或全名

### 2. `claude` CLI 的会话与 passthrough 能力足够强，命令映射空间充足

本机 `claude --help` 已确认：

- 支持 `--model`
- 支持 `--resume`
- 支持 `--continue`
- 支持 `--print`
- 支持 `auth` 子命令
- 支持 `setup-token`
- 支持 `update`

官方文档也确认：

- `claude -c` / `--continue` 可继续最近会话
- `claude -r` / `--resume` 可恢复指定会话

结论：

- `scodex launch` / `auto` / passthrough 的多数入口语义，在 Claude 侧都有对应能力
- `scodex resume --last` 这类“未知子命令透传到底层 CLI”的设计在 Claude 工具族上也具备可行性

### 3. Claude 的 auth 体系比 Codex 更复杂，不能照搬 `auth.json` 覆盖方案

官方文档已确认：

- macOS：凭据存储在 Keychain
- Linux / Windows：凭据默认存储于 `~/.claude/.credentials.json`
- 可用 auth 类型不止一种：
  - Claude.ai subscription OAuth
  - Claude Console API credentials
  - Azure Auth
  - Bedrock Auth
  - Vertex Auth
- 还支持：
  - `ANTHROPIC_API_KEY`
  - `ANTHROPIC_AUTH_TOKEN`
  - `CLAUDE_CODE_OAUTH_TOKEN`
  - `apiKeyHelper`

官方文档还明确给出了 authentication precedence。

结论：

- 不能把 `scodex` 的“复制 `~/.codex/auth.json` 即完成切换”方案平移到 Claude
- `ClaudeCodeAdapter` 必须把“凭据来源”当成能力矩阵来设计，而不是只假设单一本地文件
- 尤其在 macOS 上，直接文件覆盖不具备跨平台稳定性，因为凭据默认不在普通文件里

### 4. Claude 确实存在与订阅相关的 usage / quota 语义

官方文档已确认：

- Claude subscription 用户存在 rolling usage allowance
- 错误文案中明确出现：
  - session limit
  - weekly limit
  - Opus limit
- 官方建议用户运行 `/usage` 查看 plan limits 与 reset 时间
- 状态行文档明确提供：
  - `rate_limits.five_hour.used_percentage`
  - `rate_limits.seven_day.used_percentage`
  - 对应 `resets_at`
- `rate_limits` 仅对 Claude.ai subscriber 出现，且是首次 API 响应后出现

结论：

- Claude 侧确实存在足以类比 `scodex` 的 `5h` 与 weekly 窗口语义
- `UsageSnapshot` 至少在字段级别上具备可映射性：
  - `five_hour_remaining_percent`
  - `weekly_remaining_percent`
  - `five_hour_refresh_at`
  - `weekly_refresh_at`
- 这意味着 Claude 工具族不是只能做固定模型 wrapper，理论上有机会接入自动选号

### 5. 但 Claude 的 usage 读取路径尚未达到“直接实现”的程度

虽然官方确认了 `/usage` 与状态行中的 `rate_limits` 字段存在，但当前还没有确认以下实现级问题：

- `/usage` 是否存在稳定、可脚本解析的输出格式
- `claude` 是否提供 machine-readable 的 usage CLI 接口
- 状态行 `rate_limits` 是仅供 UI 渲染，还是存在稳定的独立查询方式
- API key / Bedrock / Vertex / Foundry 模式下，usage 语义是否与 subscriber 模式兼容
- `Opus limit` 是否需要单独建模进 `UsageSnapshot` 或额外策略字段

结论：

- Claude usage 不是“不存在”，而是“存在官方语义，但实现路径仍需 PoC”
- 后续不能直接承诺 `scodex` 风格的完整自动选号，必须先做脚本级可读性验证

### 6. 订阅与模型可用性存在强耦合，会直接影响 profile 设计

官方资料已确认：

- Pro 计划对 Opus 的可用性有限制
- 某些 Opus / 1M context / fast mode 需要 extra usage
- extra usage 适用于 Claude Code
- subscriber 与 API 用户的 cost / usage 观察方式不同：
  - API 用户更偏 `/cost`
  - subscriber 更偏 `/stats` 与 `/usage`

结论：

- `opus` / `sonnet` / `haiku` 不能简单理解为“换个默认模型参数”而已
- profile 设计必须考虑：
  - 某个账号是否有权使用该 profile 对应模型
  - 如果账号被订阅限制阻断，是否视为不可用
  - 是否允许 fallback，例如 Opus 不可用时退回 Sonnet

### 7. 合规边界必须写死进实现要求

官方 legal / compliance 文档明确区分：

- OAuth authentication 是面向 Claude 原生订阅用户的
- 第三方开发者不允许代表用户路由 Free / Pro / Max 凭据

结论：

- 后续 wrapper 必须坚持“本地工具、本地凭据、本地执行”
- 不允许把用户的 Claude subscription 凭据抽到某个中间服务代发请求
- 账号池与切换设计必须是本地优先，不得跨过官方认证边界

## 本机只读验证结果

### 环境观察

本机已安装 `claude`：

- 路径：`/Users/liuzhihao/.local/bin/claude`

本机 `claude --help` 观察到：

- `--model`
- `--resume`
- `--continue`
- `--print`
- `auth`
- `setup-token`
- `update`

本机 `claude auth --help` 观察到：

- `auth login`
- `auth logout`
- `auth status`

### 本机 `~/.claude` 观察

本机存在：

- `~/.claude/settings.json`
- `~/.claude/history.jsonl`
- `~/.claude/stats-cache.json`
- `~/.claude/sessions/`
- `~/.claude/plugins/`

本机未发现 `~/.claude/.credentials.json`。

这个现象与官方文档一致：

- macOS 使用 Keychain 存储凭据
- Linux / Windows 才默认写 `~/.claude/.credentials.json`

结论：

- 本机实际情况进一步证明：不能把 Claude 账号切换方案建立在“总有一个可复制的凭据文件”之上

## 能力矩阵

### A. Auth / Credential

已确认：

- 存在稳定的官方认证入口
- 支持 subscription / API key / cloud provider 多种认证方式
- 支持 `auth login` / `auth logout` / `auth status`
- 支持 `setup-token` 生成长生命周期 token 用于脚本和 CI

未确认：

- `auth status` 是否足够 machine-readable
- `CLAUDE_CODE_OAUTH_TOKEN` 是否适合本地多账号切换主路径
- 多 subscription 账号在 macOS Keychain 中是否可稳定导出、切换、恢复

实现判断：

- 可以进入只读 PoC
- 不可以直接进入跨平台多账号切换实现

### B. Identity

已确认：

- Claude Code 有“当前活跃凭据”的概念
- `/status` 会显示当前账号信息

未确认：

- `auth status` / `/status` 的输出是否稳定到足以解析 email / account id / plan
- 不同 auth 类型下的 identity 字段是否统一

实现判断：

- 需要 PoC 验证
- `AccountRecord.account_id` 是否保留以及如何填充，暂不能定稿

### C. Model Selection

已确认：

- `--model`
- `/model`
- 环境变量默认模型映射
- 官方 alias：`opus` / `sonnet` / `haiku`

实现判断：

- 这是当前最成熟、最适合先接入的能力
- 三入口工具完全可以先建模为同 adapter 下的三个 profile

### D. Resume / Continue

已确认：

- CLI 原生支持 `--resume`
- CLI 原生支持 `--continue`
- `~/.claude` 中存在会话和 transcript 存储

实现判断：

- `launch` / passthrough / resume 语义具备较高实现可行性

### E. Usage / Quota

已确认：

- subscriber 存在 5h 与 7d usage window
- 有 reset time
- 存在 `/usage`
- 状态行提供 `rate_limits` 字段

未确认：

- 最佳读取路径
- 可脚本解析稳定性
- API 用户与 subscriber 用户的统一建模方式
- `Opus limit` 是否要单独纳入策略层

实现判断：

- 可进入 PoC
- 暂不适合直接承诺完整选号逻辑

## 推荐实现结论

### 1. 三入口应建模为三个 profile，不应建模为三套独立 adapter

推荐：

- `ClaudeCodeAdapter`
- `OpusProfile`
- `SonnetProfile`
- `HaikuProfile`

原因：

- 底层 CLI 是同一个 `claude`
- 核心差异主要在：
  - 默认模型
  - 可能的 fallback 策略
  - 文案 / 品牌 / 状态目录 namespace

### 2. 后续实现顺序必须调整为“先 profile 抽象，再 Claude PoC”

推荐顺序：

1. `task-003`
   - 解耦 `src/cli.rs` 对 `CodexAdapter` 的直接绑定
   - 引入 wrapper profile 概念
2. `task-005`
   - 做 `ClaudeCodeAdapter` 只读 PoC
   - 只验证 auth / identity / usage 读取
3. 验证通过后
   - 才进入 launch / auto / use / list / refresh / passthrough 适配

### 3. 多账号切换不应默认基于凭据文件复制

推荐优先探索以下路线：

- 基于环境变量或 token 注入的切换
- 基于 `setup-token` / `CLAUDE_CODE_OAUTH_TOKEN` 的本地多账号管理
- 必要时在 Linux / Windows 与 macOS 采用不同 credential backend

不推荐直接假设：

- 所有平台都能通过覆盖 `~/.claude/.credentials.json` 完成切换

## 对后续 agent 的明确要求

1. 不要把 Claude 当作另一个 `codex`。
2. 不要复制 `src/adapters/codex` 一份再批量替换字符串。
3. 不要在未验证 `auth status` / `/usage` 可机读前实现自动选号。
4. 不要在 macOS 上假设存在可复制的 `.credentials.json`。
5. 不要把 `opus` / `sonnet` / `haiku` 建成三套独立命令树。
6. 优先实现只读 PoC，再决定是否做切换与选号。

## 建议后续验证清单

后续 agent 在真正编码前，应完成以下脚本级验证：

1. 验证 `claude auth status` 是否支持稳定解析当前账号信息。
2. 验证 `/status` 与 `/usage` 是否存在非交互可读路径。
3. 验证 `claude -p` 或其他非交互模式下，是否能拿到 usage / account 信息。
4. 验证 `CLAUDE_CODE_OAUTH_TOKEN`、`ANTHROPIC_API_KEY`、`ANTHROPIC_AUTH_TOKEN` 三条路径的切换体验。
5. 验证不同计划下：
   - Opus 是否可用
   - Sonnet 是否可用
   - Haiku 是否可用
   - rate_limits 字段是否完整
6. 验证 macOS 与 Linux / Windows 的 credential backend 差异是否需要平台分支实现。

## 交付结论

`Claude Code` 已经满足了“值得继续扩展”的最低条件：

- 有官方模型 alias
- 有原生 CLI
- 有会话恢复
- 有 subscription usage 语义
- 有 auth / token / script 多种认证路径

但它还没有满足“可以直接照抄 `scodex` 的实现模式”的条件：

- 凭据存储跨平台不一致
- usage 读取路径还未完成脚本级验证
- 订阅权限与模型可用性强耦合

因此，后续扩展的正确路线是：

- 先做 profile 抽象
- 再做 `ClaudeCodeAdapter` 只读 PoC
- 最后再决定是否接入完整自动选号和多账号切换

## 参考依据

### 本地只读验证

- `command -v claude`
- `claude --help`
- `claude auth --help`
- `~/.claude` 目录结构
- `~/.claude/settings.json` 顶层键

### 官方资料

- Claude Code Quickstart
- Claude Code CLI reference
- Claude Code Authentication / Credential management
- Claude Code Model configuration
- Claude Code Error reference
- Claude Code Status line
- Claude Code Costs
- Claude Help Center: Claude Code model configuration
- Claude Help Center: extra usage for paid Claude plans

## 完成定义

当后续 agent 可以直接基于本任务开始：

- wrapper profile 抽象设计
- `ClaudeCodeAdapter` 只读 PoC
- Claude 工具族入口设计

且不需要重新做同级别外部调研时，本任务视为完成。
