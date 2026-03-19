# OpenClaw Parity Gap List

基线 commit 固定为 `.upstream/cc-switch` 的 `3dad255a2a926d05bd39c1015874a3406de1b303`。

状态口径：`closed` / `deferred` / `explicitly out of scope` / `blocked`。

## overall verification snapshot

| check | status | evidence | note |
| --- | --- | --- | --- |
| `cargo fmt --check` | closed | 通过 | 当前没有 formatting blocker。 |
| `cargo test -q` | closed | 通过，`585 passed; 0 failed`，其余 test groups 也全绿 | 之前记录的 repo-level 测试失败已经清零，不再是交付 blocker。 |
| `cargo clippy --all-targets -- -D warnings` | blocked | 仍失败；首个明确 blocker 是 `src/claude_mcp.rs:12` dead_code | 当前分支还不能按 repo-level fully green 口径收尾。 |

## provider/config

| gap | baseline commit | upstream file + line | local file | status | evidence | handling recommendation | owner |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 最后一个 provider 删除后的 `models` 重写语义 | `3dad255a2a926d05bd39c1015874a3406de1b303` | `.upstream/cc-switch/src-tauri/src/openclaw_config.rs:686` | `src-tauri/src/openclaw_config.rs:706` | deferred | `src-tauri/tests/openclaw_config.rs:595` (`remove_last_provider_still_rewrites_models_section_differently_from_upstream_baseline`) | 保留 parity sentinel 明确记录当前仍与上游不同；后续若要改成完全对齐，先删掉该 sentinel 再改实现。 | backend parity implementer |
| `models.providers.<id>` 定点 add/remove 与 unknown provider key 保留 contract | `3dad255a2a926d05bd39c1015874a3406de1b303` | `.upstream/cc-switch/src-tauri/src/openclaw_config.rs:662` | `src-tauri/tests/openclaw_config.rs` | closed | `src-tauri/tests/openclaw_config.rs:448` (`provider_point_updates_preserve_models_mode_and_other_provider_keys`); `src-tauri/tests/openclaw_config.rs:534` (`shared_round_trip_fixture_preserves_provider_and_agents_contracts`) | 继续以 fixture-driven contract test 锁定；后续若 backend 改动 `models.providers` 写回边界，先更新测试再动实现。 | tests/docs implementer + backend parity implementer |

## Env/Tools/Agents Defaults

| gap | baseline commit | upstream file + line | local file | status | evidence | handling recommendation | owner |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `tools.profile` / malformed `env` / legacy `agents.defaults.timeout` 的 warning source-of-truth 只在 backend config 层定义 | `3dad255a2a926d05bd39c1015874a3406de1b303` | `.upstream/cc-switch/src-tauri/src/openclaw_config.rs:578` | `src-tauri/src/openclaw_config.rs:609` | closed | `src-tauri/tests/openclaw_config.rs:317` (`openclaw_health_scan_reports_profile_and_env_shape_warnings`); `src-tauri/src/cli/tui/data.rs:487`; `src-tauri/src/cli/tui/ui/tests.rs:2429` (`openclaw_config_warning_banner_shows_backend_warning_copy`) | 保持 `scan_openclaw_config_health` 为唯一 warning source-of-truth；TUI 只消费 snapshot 里的 backend warning。 | backend parity implementer |
| `env` / `tools` / `agents.defaults` 的跨 section round-trip 之前只有源文件内 unit tests | `3dad255a2a926d05bd39c1015874a3406de1b303` | `.upstream/cc-switch/src-tauri/src/openclaw_config.rs:845` | `src-tauri/tests/openclaw_config.rs` | closed | `src-tauri/tests/openclaw_config.rs:349` (`set_env_config_preserves_other_root_sections`); `src-tauri/tests/openclaw_config.rs:399` (`set_tools_config_preserves_other_root_sections`); `src-tauri/tests/openclaw_config.rs:476` (`set_agents_defaults_preserves_sibling_agents_keys`) | 继续用集成测试做外层 contract guard；后续若 backend 改动这些 section 的写回边界，必须先更新 fixture 再改实现。 | tests/docs implementer |

## TUI mapping

| gap | baseline commit | upstream file + line | local file | status | evidence | handling recommendation | owner |
| --- | --- | --- | --- | --- | --- | --- | --- |
| OpenClaw 表单当前只映射 provider 基础字段与 models，不含 `env` / `tools` / `agents.defaults` 编辑入口 | `3dad255a2a926d05bd39c1015874a3406de1b303` | `.upstream/cc-switch/src-tauri/src/openclaw_config.rs:845` | `src-tauri/src/cli/tui/app/app_state.rs:250` | closed | `src-tauri/src/cli/tui/app/tests.rs:1866` (`openclaw_config_menu_exposes_env_tools_and_agents_items`); `src-tauri/src/cli/tui/app/tests.rs:1933` (`openclaw_config_route_env_enter_opens_dedicated_subroute`); `src-tauri/src/cli/tui/runtime_actions/editor.rs:1041` (`openclaw_config_route_env_entry_from_config_menu_saves_and_clears_warning`) | 维持独立 OpenClaw config 路由与编辑流，不再把这些 section 塞回 provider 基础表单。 | TUI mapping implementer |
| TUI provider detail 只展示 live/saved/default-model 状态，未消费 backend health warnings | `3dad255a2a926d05bd39c1015874a3406de1b303` | `.upstream/cc-switch/src-tauri/src/openclaw_config.rs:144` | `src-tauri/src/cli/tui/ui/providers.rs:222` | explicitly out of scope | `docs/superpowers/plans/2026-03-18-openclaw-support-alignment.md:524`; `src-tauri/src/cli/tui/ui/tests.rs:2467` (`openclaw_config_warning_global_banner_is_visible_on_all_subroutes`) | 本轮只把 warning 接到 OpenClaw config surfaces，不把 provider list/detail 变成第二套 warning 入口。 | TUI mapping implementer |

## tests/docs

| gap | baseline commit | upstream file + line | local file | status | evidence | handling recommendation | owner |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 缺少 fixture-driven 外层契约测试来锁定 OpenClaw backend parity | `3dad255a2a926d05bd39c1015874a3406de1b303` | `.upstream/cc-switch/src-tauri/src/openclaw_config.rs:870` | `src-tauri/tests/openclaw_config.rs` | closed | `src-tauri/tests/openclaw_config.rs:305` (`openclaw_health_scan_reports_parse_failures_from_backend_source_of_truth`); `src-tauri/tests/openclaw_config.rs:534` (`shared_round_trip_fixture_preserves_provider_and_agents_contracts`); `src-tauri/tests/openclaw_config.rs:595` (`remove_last_provider_still_rewrites_models_section_differently_from_upstream_baseline`) | 保留当前外层 suite 作为 parity lock；已知偏差继续由 sentinel test 明确标记，而不是靠人工记忆。 | tests/docs implementer + backend parity implementer |
| `openclaw_config` 仍是 crate 私有模块，集成测试只能通过 harness 直接编译源文件锁定 contract | `3dad255a2a926d05bd39c1015874a3406de1b303` | `.upstream/cc-switch/src-tauri/src/openclaw_config.rs:144` | `src-tauri/src/lib.rs:15` | explicitly out of scope | `src-tauri/tests/openclaw_config.rs:183`; `src-tauri/src/lib.rs:15` | 本轮继续保留 harness 方案，不为了测试便利扩大生产 API 暴露面。 | tests/docs implementer + backend parity implementer |
| repo-level verification 仅剩 clippy blocker，当前分支不能按“完成”口径交付 | `n/a` | `n/a` | `src-tauri/src/claude_mcp.rs` | blocked | `cargo fmt --check` 通过；`cargo test -q` 通过，`585 passed; 0 failed`，其余 test groups 也全绿；`cargo clippy --all-targets -- -D warnings` 仍失败，首个明确 blocker 为 `src/claude_mcp.rs:12` dead_code | 旧的 repo-level 测试 blocker 已处理完；当前只把 clippy 失败继续作为交付阻塞项，不再把已关闭的测试问题留在跟踪表里。 | tests/docs implementer + branch integrator |
