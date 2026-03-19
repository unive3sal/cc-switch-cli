# OpenClaw Support Alignment Design

## Background

目标分支是 `pr-59-openclaw-dev`，远端跟踪 `origin/openclaw-dev`，worktree 位于 `/Users/saladday/dev/cc-switch-cli/.worktrees/pr-59-openclaw-dev`。

这条分支已经落了一批 OpenClaw 相关改动，核心新增包括 `src-tauri/src/openclaw_config.rs`、`src-tauri/src/services/provider/mod.rs` 中的 OpenClaw 分支、以及对应的 TUI 与测试代码。当前工作不从零开始重做，而是在现有实现基础上，按上游 `.upstream/cc-switch` 的 OpenClaw 行为逐项对齐，收敛成可审查、可验证、可继续派工的状态。

## Parity Baseline

本轮唯一的上游比对基线固定为本地共享上游镜像 `/Users/saladday/dev/cc-switch-cli/.upstream/cc-switch` 的当前 `HEAD`：`3dad255a2a926d05bd39c1015874a3406de1b303`（`chore: bump version to v3.12.1 and add release notes`）。

后续的 parity gap 清单、子代理实施说明、独立 review 和最终验收，都以这个 commit 为唯一口径。若中途需要切换上游基线，必须先更新本文档，再重刷 gap 清单和受影响的 review 结论。

## Goal

在不推翻现有分支骨架的前提下，把本仓库的 OpenClaw 支持收敛到以下目标：

1. 后端行为尽量与 `.upstream/cc-switch` 保持一致。
2. TUI 不复刻上游 GUI 外观，但表单字段、配置语义、默认值和动作语义尽量一致。
3. 本轮覆盖 `provider/config/TUI 主流程`，以及 `Env / Tools / Agents Defaults / health warning`。
4. 通过子代理编排推进实施，主代理以拆解、派工、协调、复核和验收为主，默认不直接写功能代码。

## Scope

### In Scope

- OpenClaw provider schema、默认值、导入/更新/删除/默认模型相关行为。
- `openclaw.json` 的 live 读取、写回、增量导入、移除、健康检查与告警来源。
- `Env`、`Tools`、`Agents Defaults` 三组 OpenClaw 专属配置能力。
- TUI 中的 OpenClaw provider 新增、编辑、列表动作、默认模型操作、独立配置入口和 warning 展示。
- OpenClaw 对齐所需的 i18n、测试和最小必要文档。

### Out of Scope

- OpenClaw session/resume 全量能力对齐。
- 上游 GUI 的视觉复刻。
- 与 OpenClaw 无关的 provider 通用重构。
- 非交互 `provider` commands 的体验对齐不是本轮目标面；只有当共享服务层 contract 变化导致现有命令层测试需要同步时，才做最小回归修补。
- 借题发挥整理整个 `src-tauri/src/services/provider/mod.rs`、`src-tauri/src/cli/i18n.rs` 或其他大文件。
- 本轮不包含 commit、PR、发布动作，除非后续明确要求。

## Alignment Principles

### 1. Backend parity first

只要上游 `.upstream/cc-switch` 已经定义了 OpenClaw 的字段、默认值、导入规则、写回规则、warning 条件或能力边界，本仓库就优先对齐这些行为，而不是发明本地版本。

### 2. TUI can differ in interaction, not in meaning

TUI 可以按照 ratatui 的习惯重排布局、拆分步骤、改用列表或子编辑器，但不能改掉字段语义。比如 `providerKey`、`api`、`baseUrl`、`apiKey`、`headers.User-Agent`、`models[]`、`env`、`tools`、`agents.defaults` 的含义和数据形状都要尽量与上游一致。

### 3. Keep changes narrow

高风险大文件同一轮只允许一个实施子代理主改，避免在 `src-tauri/src/openclaw_config.rs`、`src-tauri/src/services/provider/mod.rs`、`src-tauri/src/cli/i18n.rs` 这类文件上并发踩踏。

### 4. Test the contract, not just the code path

本轮测试目标不是证明“本地能跑”，而是锁定“是否和上游对齐”。优先把关键行为写成测试，再由实现去满足这些测试。

### 5. Preserve OpenClaw round-trip semantics

`openclaw.json` 不是普通导出文件，本轮把上游 `openclaw_config.rs` 的 round-trip 语义当作 contract 本身处理。实施必须尽量保留非目标节、未知字段和现有 JSON5 可读性，不允许把文件粗暴重建成纯 JSON，也不允许为了省事做全文件无差别重排。允许触碰的范围应收敛在 OpenClaw 管理的目标子树，例如本轮要修改的 provider 项、`env`、`tools`、`agents.defaults` 和默认模型相关节点，具体边界以基线 commit 的实现为准。

## Warning Contract

本轮 `health warning` 只指 OpenClaw 配置健康告警，不包含运行期请求失败、provider 缺失凭证时的普通表单校验提示，也不包含 stream check 的运行期提示。原因是 OpenClaw 在本仓库与上游里都不走标准 stream check 主路径。

本轮至少要覆盖以下 warning category：

- `openclaw.json` 解析或结构损坏。
- `tools.profile` 非上游允许值。
- 旧 `agents.defaults.timeout` 等 legacy 字段引发的迁移告警。
- `env` 相关结构异常，例如上游明确处理的 `env.vars` 或 `env.shellEnv` 异常情况。

这些 warning 的 source of truth 由后端 OpenClaw 配置读取层定义，TUI 负责消费和展示，不单独发明第二套判定规则。

## Current Branch Baseline

当前分支已经具备 OpenClaw 的基础接入骨架，但仍需要做一轮系统性对齐。根据前期调研，后续工作会重点核对以下几类 gap：

- `provider/config` 是否与上游 additive 模式一致，尤其是 live import、live write、删除、默认模型和 `current_provider_openclaw` 的处理。
- `openclaw.json` 的 provider 片段字段、默认值、JSON5 round-trip、warning 规则、legacy 字段处理是否与上游一致。
- TUI 是否完整覆盖 `provider + Env + Tools + Agents Defaults + health warning`，以及是否保留了上游字段语义。
- 现有 OpenClaw TUI 是否还残留过多 OpenCode 命名和模板复用，导致后续维护和理解成本偏高。
- 测试是否已经覆盖对齐 contract，而不是只覆盖局部 happy path。

## Workstream Breakdown

本次实施拆成 `3 个实施子域 + 1 个独立评审子域`。

### A. Backend parity

负责对齐 OpenClaw 后端语义，不处理 TUI 视觉和页面组织。

主要关注：

- `src-tauri/src/openclaw_config.rs`
- `src-tauri/src/provider.rs`
- `src-tauri/src/services/provider/mod.rs`
- `src-tauri/src/services/provider/live.rs`
- 必要时触及 `src-tauri/src/app_config.rs`
- 必要时触及 `src-tauri/src/settings.rs`
- 必要时触及 `src-tauri/src/services/config.rs`

交付目标：

- provider schema 与上游对齐
- live 配置导入/写回/移除行为与上游对齐
- `Env / Tools / Agents Defaults` 的后端读写契约稳定
- health warning 的判定来源明确

### B. TUI mapping

负责把上游字段语义映射到 ratatui 主流程，不改后端 contract。

主要关注：

- `src-tauri/src/cli/tui/form.rs`
- `src-tauri/src/cli/tui/form/provider_state.rs`
- `src-tauri/src/cli/tui/form/provider_state_loading.rs`
- `src-tauri/src/cli/tui/form/provider_json.rs`
- `src-tauri/src/cli/tui/form/provider_templates.rs`
- `src-tauri/src/cli/tui/app/form_handlers/provider.rs`
- `src-tauri/src/cli/tui/runtime_actions/providers.rs`
- `src-tauri/src/cli/tui/runtime_actions/editor.rs`
- `src-tauri/src/cli/tui/ui/forms/provider.rs`
- `src-tauri/src/cli/tui/ui/providers.rs`
- `src-tauri/src/cli/i18n.rs`

交付目标：

- Provider 表单与列表动作覆盖 OpenClaw 主流程
- `Env / Tools / Agents Defaults` 在 TUI 中有清晰入口与编辑路径
- warning 展示位置和文案可被用户理解
- TUI 保持本地交互风格，但不改字段语义

### C. Tests and docs

负责把已经确认的行为编码进测试，并补最小必要说明。

主要关注：

- `src-tauri/tests/provider_service.rs`
- `src-tauri/tests/opencode_provider.rs`
- 必要时回归 `src-tauri/tests/provider_commands.rs`，但只验证共享 service contract 对命令层的影响，不把非交互命令 UX 扩成主战场
- `src-tauri/src/cli/tui/form/tests.rs`
- `src-tauri/src/cli/tui/ui/tests.rs`
- 必要时新增或更新 `docs/` 下的 OpenClaw 说明

交付目标：

- 锁定 provider/config 行为
- 锁定 additive live sync 行为
- 锁定 TUI 字段映射和主要交互
- 记录已对齐、未覆盖和明确排除项

### R. Independent review

独立 reviewer 不写功能代码，只检查两件事：

1. 后端行为是否真的与 `.upstream/cc-switch` 对齐。
2. TUI 是否只做交互层本地优化，而没有改掉上游字段语义。

## Execution Model

实施不是一次性放开所有子代理，而是分阶段推进。

### Phase 0. Freeze the parity gap list

先形成一份逐条对照上游的 gap 清单。每条 gap 至少包含：

- 上游来源文件
- 本地对应文件
- 当前状态：已对齐 / 部分对齐 / 缺失 / 偏离
- 处理建议
- 归属子代理

### Phase 1. Turn gaps into work packages

把 gap 清单切成互不阻塞的工作包。高冲突文件单独归属，低冲突文件再并行推进。

### Phase 2. Backend first

先由后端子代理稳定 schema、live config 和专属配置 contract。没有稳定 contract 之前，不放行大规模 TUI 改动。

### Phase 3. TUI mapping

在后端 contract 稳定后，再派 TUI 子代理补页面流、字段映射、warning 和交互文案。

### Phase 4. Tests and docs

由测试/文档子代理把已确认行为落到测试中，并补一份面向本仓库的差异说明。

### Phase 5. Independent review and verification

最后由独立 reviewer 做对齐审查，主代理汇总验证结果并给出“已完成 / 未完成 / 明确排除”的结论。

## Subagent Contract

每个实施子代理在真正编码前，先提交一版小提案。提案至少说明：

- 准备关闭哪些 parity gap
- 预计修改哪些文件
- 哪些点依赖上游行为核对
- 计划跑哪些验证

主代理先审提案，再决定是否放行实施。

每个实施子代理的结果回报统一使用以下格式：

- 关闭了哪些 gap
- 修改了哪些文件
- 跑了哪些验证
- 剩余哪些风险或阻塞
- 哪些问题需要主代理裁决

## File Ownership Rules

- `src-tauri/src/openclaw_config.rs`、`src-tauri/src/services/provider/mod.rs`、`src-tauri/src/cli/i18n.rs` 这类高冲突文件，同一轮只允许一个实施子代理主改。
- 其他子代理如果依赖这些文件，只能提出 patch 建议或等待下一轮，不直接并发落改。
- 测试文件和文档文件可以更积极并行，但前提是不反向改写行为 contract。

## Acceptance Criteria

本轮验收分为两个等级。

### Functional completion

- 可以新增、编辑、删除、导入 OpenClaw provider。
- 可以稳定写入和读取 `openclaw.json`。
- TUI 能覆盖 `provider + Env + Tools + Agents Defaults + health warning` 主流程。

### Parity completion

- 上游关键字段、默认值、导入/写回规则、warning 条件已逐项核对。
- 关键行为有测试兜底，而不是只靠人工确认。
- 剩余差异被写清楚，并标明是未完成项还是明确排除项。

只有达到 parity completion，才算本轮真正收敛。

## Risks

- `src-tauri/src/openclaw_config.rs`、`src-tauri/src/services/provider/mod.rs`、`src-tauri/src/cli/i18n.rs` 已经很大，任何顺手重构都会扩大 review 面和回归面。
- 现有分支已经包含大量 OpenClaw 改动，后续必须先梳理 gap，再并行实施，否则容易出现“按旧 contract 改 TUI、按新 contract 改后端”的返工。
- TUI 中已有一些 OpenCode 命名复用，短期内可能继续保留，以换取更小改动面；如果要清理命名，应另开任务，不和功能对齐混在一起。
- 如果上游存在行为模糊点，默认不由实施子代理自行拍板，而是升级给主代理做裁决。

## Deliverables

本轮阶段性交付物包括：

1. 一份 OpenClaw parity gap 清单。
2. 一份子代理分工和执行顺序说明。
3. 一组验证结果与剩余风险说明。
4. 一份“已对齐 / 未覆盖 / 明确排除”摘要。

## Next Step

这份设计确认后，下一步不是直接编码，而是基于本文写出实施计划，把 parity gap 清单、工作包顺序、验证命令和 review 节点拆成可执行任务，再交由子代理执行。
