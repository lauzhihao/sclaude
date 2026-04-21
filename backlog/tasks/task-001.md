# Task 001

## 标题

设计并验证基于 `scodex` 扩展 `opus` / `sonnet` / `haiku` 工具族的可行方案

## 状态

Backlog

## 优先级

High

## 背景

当前仓库已经具备 `core + adapter` 的总体分层方向，账号状态、选号策略、共享存储、账号池同步与部分 UI 具备复用基础，但实际实现仍明显是 `codex-first`：

- 顶层命令入口直接绑定 `CodexAdapter`
- 命令名、安装器、自更新、兼容命令名、默认状态目录仍写死为 `scodex` / `codex`
- `ClaudeCodeAdapter` 目前还未落地，架构文档也明确要求先验证 identity switching 与 usage semantics，再决定是否推进

本任务的目标不是直接实现一套 Claude 工具族，而是先定义一条可落地、可验证、不会把仓库拖进三份近似代码分叉的扩展路线。

## 目标

基于当前 `scodex` 实现逻辑，产出一份可执行的扩展方案，使仓库后续能够支持：

- `opus`
- `sonnet`
- `haiku`

这些命令需要满足：

- 子命令集合与 `scodex` 保持一致
- 运行时基于 `claude code` 与 `Claude` 模型族
- 账号与订阅语义基于 `Claude` 体系，而不是复用 `Codex` 假设
- 尽量复用已有 `core` 与共享基础设施

## 非目标

- 本任务不直接实现 `ClaudeCodeAdapter`
- 本任务不直接修改现有 `scodex` 行为
- 本任务不默认承诺 Claude 侧一定存在与 `Codex` 对等的 live usage 能力
- 本任务不默认承诺必须做成三份独立二进制

## 当前实现依据

以下文件是本任务的直接分析依据：

- `ARCHITECTURE.md`
- `src/cli.rs`
- `src/adapters/mod.rs`
- `src/adapters/codex/mod.rs`
- `src/adapters/codex/account.rs`
- `src/adapters/codex/usage.rs`
- `src/adapters/codex/deploy.rs`
- `src/adapters/codex/repo_sync.rs`
- `src/core/state.rs`
- `src/core/policy.rs`
- `src/core/storage.rs`
- `src/core/update.rs`
- `install.sh`
- `install.ps1`

## 基于当前代码的分析结论

### 1. 可以直接复用的部分

- `src/core/state.rs`
  - `AccountRecord`、`UsageSnapshot`、`State` 已经是通用数据结构
- `src/core/policy.rs`
  - 选号打分与“当前账号是否继续使用”已经独立于 `Codex` 路径
- `src/adapters/codex/repo_sync.rs`
  - 账号池导出、加密、Git push/pull 的主体逻辑可以抽成共享层
- `src/adapters/codex/deploy.rs`
  - 远端传输逻辑可以复用，但源凭据路径与目标文件名需要 adapter/profile 化
- `src/core/ui.rs`
  - 通用错误包装、表格基础文案、部分共享提示可保留，但产品名与底层 CLI 名称必须解耦

### 2. 不能直接复用、必须先抽象的部分

- `src/cli.rs`
  - 当前 `run()` 直接实例化 `CodexAdapter`
  - CLI 名称固定为 `scodex`
  - `help`、`Passthrough`、打印文案都绑定 `Codex`
- `src/core/storage.rs`
  - 默认状态目录与兼容环境变量仍为 `auto-codex` / `AUTO_CODEX_HOME`
- `src/core/update.rs`
  - 自更新只支持 `scodex` 主二进制和 `auto-codex` 兼容 sidecar
- `install.sh` / `install.ps1`
  - 安装器只安装 `scodex`、`auto-codex`、`scodex-original`
- `src/adapters/codex/mod.rs`
  - `launch` / `resume` / `login` / 安装底层 CLI / 浏览器注册页 都是 `Codex` 专用流程
- `src/adapters/codex/paths.rs`
  - `CODEX_HOME`、`codex` binary 解析、`npm install -g @openai/codex` 都写死
- `src/adapters/codex/auth.rs`
  - identity 解析基于 OpenAI token 结构，不能假设 Claude 凭据具备同样字段
- `src/adapters/codex/usage.rs`
  - usage 刷新完全依赖 `Codex` 当前可用的 API 语义

### 3. 当前最合理的扩展方向

推荐先把仓库从“单一 `scodex` 命令 + 单一 `CodexAdapter`”调整为“两层可配置结构”：

- 第一层：共享 wrapper runtime
  - 统一处理命令路由、状态持久化、账号池同步、表格展示、共享 policy
- 第二层：adapter/profile
  - adapter 负责底层 CLI 能力
  - profile 负责产品名、默认状态目录、兼容命令名、安装/更新品牌、默认模型参数

如果不先做这层抽象，`opus` / `sonnet` / `haiku` 很可能演变成三份复制版 `scodex`，后续会同时产生：

- 三份 CLI 帮助文案维护
- 三份安装器分支
- 三份 update 逻辑
- 三份与 `CodexAdapter` 高度相似但无法稳定合并的命令分发代码

## 需要验证的关键问题

### A. Claude 侧能力验证

必须先确认 `claude code` 是否真的支持以下能力；在验证完成前，禁止承诺全量对齐 `scodex`：

- 是否存在稳定的本地 auth 文件或 home 目录，可用于导入和切换
- 是否可以从本地凭据中稳定解析出 identity
- 是否可以可靠判断“当前 live identity”
- 是否存在安全、可维护的账号切换方式
- 是否存在稳定的 usage/订阅查询来源
- 如果存在 usage，是否能映射到当前 `UsageSnapshot`
- 如果不存在 usage，`list` / `refresh` / 自动选号该如何降级
- 是否支持与 `resume` 类似的会话恢复语义
- 是否允许通过 CLI 参数稳定指定默认模型

### B. `opus` / `sonnet` / `haiku` 的产品建模

必须先明确以下设计，不允许边实现边猜：

- 三个命令是同一个 adapter 下的三个 profile，还是三个独立 adapter
- 三个命令是否共享同一账号池
- 三个命令是否共享同一状态目录根，再按 profile 分 namespace
- `use` / `rm` / `push` / `pull` 的对象是 Claude 全局账号池，还是 profile 独立池
- `launch` 与 `Passthrough` 是否自动注入模型选择参数
- 模型选择是硬编码映射，还是允许用户覆盖
- Claude subscription 是否影响某些模型不可用，从而反向影响选号结果

### C. 发布与安装策略

必须先明确以下工程策略：

- 单一发布资产，多入口运行
- 单一二进制，按 `argv[0]` 决定 profile
- 多个 wrapper 名称指向同一可执行文件
- 是否继续保留 `scodex` 原行为完全不变
- 自更新是否更新所有 sibling binary / wrapper
- 安装器是否同时安装 `opus` / `sonnet` / `haiku`

## 推荐实施阶段

### Phase 1: 设计与验证

- 研究 `claude code` 的本地凭据、identity、usage、resume、模型选择能力
- 输出 `ClaudeCodeAdapter` 能力矩阵
- 确认 `opus` / `sonnet` / `haiku` 应建模为 profile，而不是复制命令树

### Phase 2: 基础抽象

- 让 `src/cli.rs` 不再直接绑定 `CodexAdapter`
- 引入 wrapper profile 概念
- 解耦产品名、状态目录名、默认命令名、帮助文案
- 解耦安装器和 `self-update` 对 `scodex` 的硬编码

### Phase 3: Claude PoC

- 先实现只读能力：
  - 发现本地凭据
  - 解析 identity
  - 显示账号列表
- 如果 usage 语义可靠，再接入：
  - `refresh`
  - 自动选号
  - `launch` / `auto`

### Phase 4: 三入口产品化

- 基于同一个 `ClaudeCodeAdapter` 挂接：
  - `opus`
  - `sonnet`
  - `haiku`
- 为每个入口定义默认模型参数与显示品牌
- 明确安装、更新、兼容命令与状态隔离策略

## 实施要求

后续真正开始实现时，必须满足以下要求：

1. 不允许复制一整份 `src/cli.rs` 形成三套平行入口逻辑。
2. 不允许把 Claude 的 auth 结构硬塞进现有 `Codex` 假设。
3. 不允许在未验证 usage 语义前，伪造与 `scodex` 完全等价的自动选号承诺。
4. 不允许破坏现有 `scodex` 兼容命令与默认行为。
5. 共享逻辑优先下沉到 `core` 或通用 adapter helper，避免在 `codex` / `claude` 两边复制。

## 建议拆分出的后续子任务

- `task-002`：梳理 `claude code` 的 auth / identity / usage / model 参数能力矩阵
- `task-003`：设计 wrapper profile 抽象，解除 `src/cli.rs` 对 `CodexAdapter` 的直接绑定
- `task-004`：重构安装器与 `self-update`，支持多入口同二进制或多 wrapper 策略
- `task-005`：实现 `ClaudeCodeAdapter` 的只读 PoC
- `task-006`：在能力验证完成后，再决定是否实现 `opus` / `sonnet` / `haiku` 三入口

## 验收标准

- 明确列出当前代码中可以复用和不能复用的模块
- 明确 `ClaudeCodeAdapter` 上线前必须验证的外部能力
- 明确 `opus` / `sonnet` / `haiku` 的推荐建模方式
- 明确安装、更新、状态目录和账号池同步的工程影响
- 明确推荐的分阶段实施顺序
- 任务文档应足够具体，后续可直接据此拆分实现任务

## 风险

- Claude 的凭据与订阅语义可能无法稳定映射到当前 `UsageSnapshot`
- Claude 可能不提供可靠的 live usage，导致自动选号必须降级
- 如果把三入口做成三份分叉 wrapper，维护成本会迅速失控
- 如果 profile 与账号池边界定义不清，后续 `push` / `pull` / `use` / `rm` 会出现行为歧义

## 完成定义

当后续负责人能够基于本任务直接展开 PoC 和抽象改造，而不需要重新做一轮同等级别的架构调研时，本任务视为完成。
