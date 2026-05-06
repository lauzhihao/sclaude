# scodex API Optimizations Porting Notes

本文档总结了本次在 `scodex` 中已经完成、且后续需要迁移到 `sclaude` 的一组优化。目标不是复述会话过程，而是把已经验证过的改进整理成一份可执行的迁移蓝图，方便下一轮在 `sclaude` 中按项落地。

## 1. 背景

本次 `scodex` 优化集中在 API 类型账号的完整生命周期：

- CLI 参数接入
- API 登录行为
- 账号状态持久化
- 历史脏数据兼容迁移
- `list` 表格渲染
- API 账号显示标识
- 发布收尾

这些改动本质上不是 Codex 专属，而是“带有 API 类型账号的多账号 CLI launcher”通用问题，因此对 `sclaude` 具有较高复用价值。

## 2. 已完成优化总览

### 2.1 补齐 `add/login --api` 命令能力

在 `scodex` 中，之前存在两个明显缺口：

1. `add` 子命令不支持 `--api`
2. `login --api` 虽然有参数，但行为不完整

最终做法：

- 在 `src/cli.rs` 中把 API 相关参数抽成共享结构
- `add` 和 `login` 共同复用 API 参数解析与校验逻辑
- `add --api` 和 `login --api` 最终走同一条 API account 创建路径

迁移到 `sclaude` 时，对应检查：

- `src/cli.rs`
- `src/main.rs`
- 如存在多个 model entrypoint，还要确认入口是否共享同一套 CLI 解析

### 2.2 API 登录从交互式 subprocess 改为本地直写认证文件

在 `scodex` 中，原先 API 登录依赖外部 CLI 的交互式登录行为，不符合“命令行已经给出 key，应该直接可用”的预期。

最终做法：

- 不再启动交互式 `subprocess`
- 直接根据命令行参数在临时 home 中生成 API 认证文件
- 再导入到 `state_dir/accounts/<id>/...`
- 最后切换为当前活跃账号

这类改法的核心价值：

- 消除手动输入 API key
- 消除对外部 CLI 交互行为的依赖
- 更容易测试
- 更容易做批量迁移与自动化

迁移到 `sclaude` 时，对应检查：

- `src/adapters/claude/mod.rs`
- `src/adapters/claude/credentials.rs`
- `src/adapters/claude/account.rs`

重点不是照抄 `auth.json` 结构，而是先确认 Claude 的本地认证文件格式，再决定直写内容。

### 2.3 增加历史 API 脏状态兼容迁移

`scodex` 在本次会话中暴露了一个历史问题：某些 API 账号已经存在于本地状态中，但被错误地写成了 subscription 账号。

这会导致：

- `list` 中类型显示错误
- usage 刷新走错逻辑
- API 账号被错误地当成订阅账号参与策略判断

最终做法：

- 在 adapter 层增加 `normalize_account_records()` 之类的修正逻辑
- 程序启动后、命令执行前先跑一次兼容迁移
- 识别条件基于：
  - 受管配置文件标记
  - 本地认证文件内容
  - 可推断的 provider/base_url/token label
- 修正字段包括：
  - `account_type`
  - `email`
  - `api_provider`
  - `api_base_url`
  - `api_token_label`
  - 清理错误的 usage cache

这个点对 `sclaude` 非常关键，因为一旦上线过旧逻辑，就很容易出现历史状态兼容问题。

迁移到 `sclaude` 时，对应检查：

- `src/adapters/claude/account.rs`
- `src/core/storage.rs`
- `src/core/state.rs`
- `src/cli.rs`

建议保持原则：

- 兼容迁移逻辑放 adapter 层
- `core/storage` 仍只负责通用序列化/反序列化
- 不把 Claude-specific 识别规则塞进 core

### 2.4 修复 `list` 中 API 行 merged 渲染与边框问题

这块是本次会话里最细的一组 UI 修复。

目标表现：

- API 行类型显示为 `API`
- 后续 quota/status 几列不再逐列显示，而是合并成一个大单元格
- 该单元格固定显示 `N/A`
- 上下边框在普通订阅行、API merged row、summary/footer 三者之间都要自然连接

`scodex` 最终做了这些修复：

1. API 行正文 merged
2. API 行在中英文模式下都固定显示 `N/A`，不显示“无”
3. `subscription -> API` 过渡边框修正
4. `API -> summary` 过渡边框修正
5. 边框连接符从简单二态扩展成四态：
   - `┼`
   - `┴`
   - `┬`
   - `─`

迁移到 `sclaude` 时，对应检查：

- `src/adapters/claude/ui.rs`
- `src/core/ui.rs`

如果 `sclaude` 的列表列数不同，不要直接照搬列索引；应先确认：

- API 行要合并哪几列
- footer 是否存在
- summary 是否占整行

### 2.5 API 账号显示标识格式调整，并迁移已有账号

`scodex` 最终把 API 类型账号的邮箱/显示标识，从：

- `sk-abcd-wxyz@provider`

改成：

- `key 后 6 位@provider`

例如：

- `9aaeb2@codeproxy.dev`

这个改动同时配套了历史账号迁移，因此已有账号在下一次启动时会被自动改写到新格式。

迁移到 `sclaude` 时，需要先决定 Claude 项目里 API 账号的显示标识策略：

- 是否沿用“后 6 位@provider”
- 是否需要更短或更长
- 是否仍保留一个独立的 `api_token_label` 字段作为内部显示/调试用途

对应检查：

- `src/adapters/claude/account.rs`
- `src/core/state.rs`
- `src/adapters/claude/ui.rs`

## 3. 在 sclaude 中的建议落点

结合当前 `sclaude` 目录结构，建议按下面映射迁移：

### 3.1 CLI 层

- `src/cli.rs`

目标：

- 给 `add`/`login` 补齐 API 参数
- 抽出共享 API 参数结构与校验逻辑
- 保持 CLI surface 明确，不做隐式行为

### 3.2 Claude adapter 主流程

- `src/adapters/claude/mod.rs`

目标：

- 改造 API 登录主流程
- 从交互式/间接行为切到本地直写认证文件
- 保持“添加后切换”和“状态导入”的行为一致

### 3.3 账号导入、修正与切换

- `src/adapters/claude/account.rs`

目标：

- API account 写入
- 历史脏状态修正
- API 显示标识生成
- 活跃账号切换时的配置联动

### 3.4 Claude 凭据文件格式

- `src/adapters/claude/credentials.rs`
- `src/adapters/claude/auth.rs`

目标：

- 明确 Claude 本地认证文件格式
- 区分 subscription 与 API 凭据的最小可用结构
- 为本地直写 API 登录提供准确落点

### 3.5 列表渲染

- `src/adapters/claude/ui.rs`

目标：

- API 行 merged 渲染
- `N/A` 固定显示
- 行间 border 正确连接
- summary/footer 过渡自然

### 3.6 使用量刷新与选择策略

- `src/adapters/claude/usage.rs`
- `src/core/policy.rs`

目标：

- API 账号不参与 subscription quota 刷新
- API 账号不被错误纳入 subscription 选择逻辑
- 历史脏 usage cache 在迁移时被清理

## 4. 迁移时的风险点

### 4.1 Claude 的认证文件格式未必与 Codex 相同

这是首要风险。`scodex` 中的“本地直写 API auth”思路可以复用，但文件内容不能直接照抄。

必须先确认：

- Claude Code API 认证文件结构
- provider/base_url 的配置位置
- 当前 CLI 如何识别 API 登录态

如果实现中没有明确体现，必须直接检查本地代码和实际认证文件。

### 4.2 表格列结构可能不同

`scodex` 的 merged 逻辑依赖具体列布局。`sclaude` 如果列数、顺序或 footer 结构不同，不能直接复用列索引和边框规则。

### 4.3 历史状态兼容必须先考虑

一旦 `sclaude` 里已经存在旧状态文件，任何 API 标识、类型字段、凭据格式的变更都要配套 migration。

否则会出现：

- 类型显示错
- usage 刷新错
- 切换逻辑错
- 用户误以为是渲染问题，实际是写入问题

### 4.4 不要把 adapter-specific 规则下沉到 core

这次 `scodex` 的经验很明确：

- `core` 保持通用
- adapter-specific 的兼容修正、文件识别、配置推断都放在 adapter

`sclaude` 里也应保持这个边界。

## 5. 建议实施顺序

建议按下面顺序推进，不要一开始就碰 UI：

1. 先确认 `sclaude` 的本地 API 认证文件格式
2. 再补 CLI 参数与 API 登录主路径
3. 再补账号持久化与切换逻辑
4. 再补历史状态兼容迁移
5. 最后再修 `list` 中 API merged row 的显示
6. 收尾时统一改测试与 release 版本

这样可以避免“UI 看起来不对，实际是底层状态写错”的来回返工。

## 6. 可执行检查清单

后续在 `sclaude` 开工时，可以按这个 checklist 逐项执行：

1. 读 `.project_map`，定位 `cli.rs`、`adapters/claude/*`、`core/*`
2. 确认 Claude API 认证文件和配置文件格式
3. 给 `add/login` 增加 API 参数
4. 实现本地直写 API 登录
5. 实现 API account 导入与切换
6. 实现历史 API 脏状态自动修正
7. 确保 API 账号不走 subscription usage 刷新
8. 确保 API 账号不参与 subscription 选择策略
9. 修 `list` 里 API 行 merged 渲染
10. 固定 API 行显示 `N/A`
11. 修普通行、API 行、summary 之间的边框连接
12. 确定 API 账号显示标识格式
13. 给已有账号做兼容迁移
14. 补单元测试和命令级验证
15. 再做版本 bump、提交、tag、release

## 7. 结论

本次 `scodex` 会话证明，这类 launcher 项目里，API 账号支持不能只补一个 CLI 参数；它会连带影响：

- 认证文件生成
- 状态结构
- 历史兼容
- usage 刷新
- 选择策略
- 列表渲染
- 用户可读的账号标识

因此，后续在 `sclaude` 中实施相同优化时，应把它作为一组成体系的改动来推进，而不是拆成零散的小补丁。
