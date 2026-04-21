# Task 003

## 标题

设计 wrapper profile 抽象，解除 `src/cli.rs` 对 `CodexAdapter` 的直接绑定

## 状态

Backlog

## 优先级

High

## 来源任务

- `task-001`
- `task-002`

## 任务目标

在不破坏现有 `scodex` 兼容行为的前提下，为仓库引入一层明确的 wrapper profile 抽象，使当前实现从“单一 `scodex` + 单一 `CodexAdapter`”演进到：

- 共享 wrapper runtime
- profile 驱动的产品元数据
- adapter 驱动的底层 CLI 能力

本任务的目标不是实现 `ClaudeCodeAdapter`，而是清理当前阻碍多入口扩展的结构性硬编码，为后续：

- `scodex`
- `opus`
- `sonnet`
- `haiku`

这类命令入口共存创造基础条件。

## 背景

根据 `task-001` 与 `task-002` 的结论，当前仓库虽然已有 `core + adapter` 的分层方向，但在真正的运行入口上仍然是明显的 `codex-first`：

- `src/cli.rs` 直接实例化 `CodexAdapter`
- `src/core/storage.rs` 直接写死 `auto-codex`
- `src/core/update.rs` 直接写死 `scodex`
- 安装器只安装 `scodex`、`auto-codex`、`scodex-original`

如果不先抽象 wrapper profile，后续即使实现了 `ClaudeCodeAdapter`，也会遇到以下问题：

- CLI 命令名和帮助文案无法复用
- 状态目录与环境变量命名无法隔离
- 自更新与发布资产命名无法扩展
- 安装器无法同时安装多个产品入口
- `opus` / `sonnet` / `haiku` 最终只能走复制代码路线

## 当前代码中的硬编码问题

### 1. `src/cli.rs`

当前问题：

- `Cli` 的命令名固定为 `scodex`
- `run()` 中直接 `let adapter = CodexAdapter::default()`
- 默认行为、帮助文案、透传语义都围绕 `Codex`
- `build_autofill_request()` 与 `login --oauth` 也是 `Codex` 专用路径

影响：

- 无法让同一套命令路由挂接不同 profile
- 无法为不同入口切换产品文案
- 无法在不复制 `cli.rs` 的情况下接入新的 adapter

### 2. `src/core/storage.rs`

当前问题：

- 默认状态目录名固定为 `auto-codex`
- legacy 状态目录固定为 `codex-autoswitch`
- 环境变量固定为：
  - `AUTO_CODEX_HOME`
  - `CODEX_AUTOSWITCH_HOME`

影响：

- 无法按 profile 定义不同默认 state namespace
- 无法清晰区分 `scodex` 与 Claude 工具族的本地状态

### 3. `src/core/update.rs`

当前问题：

- 默认 repo 固定为 `lauzhihao/scodex`
- User-Agent 固定为 `scodex`
- release 资产固定命名为 `scodex-*`
- 压缩包中固定查找 `scodex` / `scodex.exe`
- sidecar binary 仅识别 `auto-codex`

影响：

- 自更新无法为多入口产品工作
- 后续即使共用同一二进制，也无法按 profile 正确更新 wrapper siblings

### 4. `install.sh` / `install.ps1`

当前问题：

- 只安装：
  - `scodex`
  - `auto-codex`
  - `scodex-original`
- 安装后自动导入的是 `~/.codex/auth.json`

影响：

- 安装器完全绑定当前产品
- 无法演进为“同一二进制，多 wrapper profile”安装策略

## 推荐抽象方向

### 1. 引入 `WrapperProfile`

推荐增加一个统一的 profile 概念，例如：

- `ScodexProfile`
- `OpusProfile`
- `SonnetProfile`
- `HaikuProfile`

这个 profile 不直接代表 adapter，而是代表“一个用户可见产品入口”的元数据集合。

### 2. 引入 profile 元数据结构

推荐至少拆出以下几个维度：

#### `ProfileBranding`

负责：

- CLI 显示名称
- 帮助文案中的产品名
- 默认命令名
- 底层 CLI 显示名称
- 安装提示文案

#### `ProfilePaths`

负责：

- 默认状态目录名
- legacy 状态目录名
- profile 级环境变量名
- 兼容命令名集合

#### `ProfileUpdateSpec`

负责：

- 默认 release repo
- 资产命名规则
- 自更新时需要同步更新的 sibling binary 列表
- User-Agent / 渠道标识

#### `ProfileRuntime`

负责：

- 默认 adapter id
- 是否支持某些顶层子命令
- 默认 passthrough 目标 CLI
- profile 级默认模型或启动参数

## 推荐设计原则

### 1. profile 与 adapter 必须分层

不要把 profile 和 adapter 混为一谈。

例子：

- `scodex` 是一个 profile
- `codex` 是一个底层 CLI / adapter
- `opus` / `sonnet` / `haiku` 将来也是 profile
- 它们共享 `ClaudeCodeAdapter`

### 2. `src/cli.rs` 应先改成“解析命令 + 读取当前 profile + 委派执行”

`src/cli.rs` 的最终职责应是：

- 解析公共命令结构
- 根据当前可执行名或显式配置确定 profile
- 读取 profile branding / paths / runtime
- 再把执行权交给对应 adapter runtime

不应继续直接创建 `CodexAdapter`。

### 3. storage 与 update 都应由 profile 驱动

任何涉及以下内容的逻辑都不应再写死：

- 状态目录名
- 环境变量名
- 发布 repo
- 资产名
- sidecar / compatibility binary 名称

### 4. 第一阶段不追求“所有命令都抽象完”

本任务的目标是先把结构性阻塞点拆掉，而不是一次性完成完整多产品框架。

优先级应是：

1. profile 判定
2. `cli.rs` 去掉对 `CodexAdapter` 的直接依赖
3. storage / update 元数据 profile 化
4. 安装器为多入口预留结构

## 明确不在本任务内的内容

- 不实现 `ClaudeCodeAdapter`
- 不实现 Claude 多账号切换
- 不实现 Claude usage 刷新
- 不实现 `opus` / `sonnet` / `haiku` 的最终产品命令
- 不在本任务中修改选号策略本身

## 对实现的明确要求

1. 不允许复制一份 `src/cli.rs` 作为新入口。
2. 不允许通过条件分支把 `cli.rs` 改成新的硬编码拼盘。
3. 必须把“产品入口元数据”和“底层 adapter 能力”分开建模。
4. 现有 `scodex` 默认行为必须保持兼容。
5. 现有 `auto-codex` 兼容入口不能被静默破坏。
6. `update` 与安装器的后续扩展点必须在设计中预留。

## 推荐实施顺序

### Phase 1: profile 基础模型

- 定义 `WrapperProfile`
- 定义 branding / paths / update / runtime 元数据结构
- 提供 `scodex` 的默认 profile 实现

### Phase 2: CLI 解耦

- 让 `src/cli.rs` 从 profile 读取命令名与产品文案
- 去掉 `run()` 对 `CodexAdapter::default()` 的直接依赖
- 改为通过 profile 决定 adapter runtime

### Phase 3: storage / update 解耦

- 把状态目录名与 env var 改为从 profile 读取
- 把 release repo、资产名、sidecar binary 集合改为从 profile 读取

### Phase 4: 安装器预留多入口结构

- 先为安装器抽象命令名和 sibling wrapper 列表
- 当前仍只输出 `scodex` 系列也可以，但结构上不能再写死

## 验收标准

完成本任务后，至少应满足：

1. `src/cli.rs` 不再直接依赖 `CodexAdapter`。
2. 当前运行入口能够解析出一个 `scodex` profile。
3. `src/core/storage.rs` 不再写死 `auto-codex` / `AUTO_CODEX_HOME` 这类值，而是从 profile 读取。
4. `src/core/update.rs` 不再写死 `scodex` 资产命名和 sidecar 名称，而是从 profile 读取。
5. `scodex` 现有行为与兼容入口保持不变。
6. 后续 `opus` / `sonnet` / `haiku` 可以在不复制 `cli.rs` 的前提下挂入新 profile。

## 与后续任务的关系

- `task-003` 完成后，才能进入 `task-005`
- `task-005` 的目标应是：
  - 基于新 profile 结构实现 `ClaudeCodeAdapter` 只读 PoC
- 在 `task-005` 完成前，不要开始三入口产品化

## 完成定义

当后续 agent 能够在不复制顶层命令树、不重写自更新与状态目录逻辑的前提下，为仓库接入新的 profile 和 adapter，并以此为基础继续实现 Claude 工具族时，本任务视为完成。
