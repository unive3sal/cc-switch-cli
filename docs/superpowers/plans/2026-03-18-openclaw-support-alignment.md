# OpenClaw Support Alignment Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the existing OpenClaw support on `pr-59-openclaw-dev` into parity with upstream commit `3dad255a2a926d05bd39c1015874a3406de1b303` for provider/config/TUI main flow plus `Env / Tools / Agents Defaults / health warning`, without rewriting the branch from scratch.

**Architecture:** Reuse the current `AppType::OpenClaw` path and treat `src-tauri/src/openclaw_config.rs` plus `src-tauri/src/services/provider/mod.rs` as the contract surface. Lock parity in targeted tests first, then align backend write/import/warning behavior, then map the stable contract into ratatui config and provider surfaces. Keep high-conflict files single-owner per round and finish with an independent parity review.

**Tech Stack:** Rust, serde/serde_json, json5 + `json_five` round-trip parser, ratatui, cargo test, git worktrees, subagents

---

**Spec:** `docs/superpowers/specs/2026-03-18-openclaw-support-design.md`

**Parity Baseline:** `/Users/saladday/dev/cc-switch-cli/.upstream/cc-switch` at `3dad255a2a926d05bd39c1015874a3406de1b303`

## File map

- `src-tauri/src/provider.rs`: OpenClaw provider/model serde shape; must match upstream camelCase fields and preserve unknown keys with `flatten`.
- `src-tauri/src/openclaw_config.rs`: OpenClaw JSON5 read/write, round-trip preservation, provider map updates, `env`, `tools`, `agents.defaults`, default model, health scan.
- `src-tauri/src/services/provider/mod.rs`: add/update/switch/delete/import behavior; additive-mode semantics for OpenClaw.
- `src-tauri/src/services/provider/live.rs`: live snapshot capture/restore and live import bridge for OpenClaw.
- `src-tauri/src/settings.rs`: OpenClaw override dir plus `current_provider_openclaw` storage and effective-current helpers.
- `src-tauri/src/cli/tui/data.rs`: snapshot layer for provider rows, OpenClaw config slices, warnings, and default-model state.
- `src-tauri/src/cli/tui/ui.rs`: central route-to-render wiring; new OpenClaw config routes must be added here.
- `src-tauri/src/cli/tui/route.rs`: route additions for OpenClaw config surfaces if needed.
- `src-tauri/src/cli/tui/app/app_state.rs`: new actions, config items, editor submit variants, and app state indexes.
- `src-tauri/src/cli/tui/app/content_config.rs`: key handling and route transitions for OpenClaw config items.
- `src-tauri/src/cli/tui/runtime_actions/editor.rs`: editor submit handlers for OpenClaw config JSON/forms.
- `src-tauri/src/cli/tui/runtime_actions/providers.rs`: add/remove/default-model actions and provider-specific toasts.
- `src-tauri/src/cli/tui/form.rs`: OpenClaw constants, fields, and form state.
- `src-tauri/src/cli/tui/form/provider_state.rs`: visible fields, defaults, field summaries, model-editor entry points.
- `src-tauri/src/cli/tui/form/provider_state_loading.rs`: provider -> form hydration for OpenClaw arrays/headers/protocol.
- `src-tauri/src/cli/tui/form/provider_json.rs`: form -> provider JSON mapping; must preserve unknown keys and full `models[]` arrays.
- `src-tauri/src/cli/tui/form/provider_templates.rs`: dedicated OpenClaw template/preset mapping; stop aliasing to OpenCode templates.
- `src-tauri/src/cli/tui/ui/config.rs`: OpenClaw config menu and detail rendering.
- `src-tauri/src/cli/tui/ui/forms/provider.rs`: OpenClaw provider form labels, hints, model editor affordances.
- `src-tauri/src/cli/tui/ui/providers.rs`: provider list/detail status, default-model copy, warning placement.
- `src-tauri/src/cli/i18n.rs`: OpenClaw labels, toasts, warning copy, config item names.
- `src-tauri/tests/support.rs`: shared isolated HOME fixtures.
- `src-tauri/tests/openclaw_config.rs`: new integration tests for round-trip/warnings/section writers.
- `src-tauri/tests/provider_service.rs`: additive-mode import/write/delete/default-model service contract tests.
- `src-tauri/tests/opencode_provider.rs`: cross-check additive sync behavior for OpenClaw/OpenCode.
- `src-tauri/src/cli/tui/form/tests.rs`: form round-trip/preset/model-array tests.
- `src-tauri/src/cli/tui/ui/tests.rs`: provider/config TUI copy and key-bar rendering tests.
- `src-tauri/src/cli/tui/app/tests.rs`: route transitions, editor opens, and action dispatch tests.

## High-conflict ownership and order

- **Round 1 owner:** backend parity subagent owns `src-tauri/src/openclaw_config.rs`, `src-tauri/src/provider.rs`, `src-tauri/src/services/provider/mod.rs`, `src-tauri/src/services/provider/live.rs`, `src-tauri/src/settings.rs`.
- **Round 2 owner:** TUI-config subagent owns `src-tauri/src/cli/tui/route.rs`, `src-tauri/src/cli/tui/ui.rs`, `src-tauri/src/cli/tui/app/app_state.rs`, `src-tauri/src/cli/tui/app/content_config.rs`, `src-tauri/src/cli/tui/runtime_actions/editor.rs`, `src-tauri/src/cli/tui/ui/config.rs`, `src-tauri/src/cli/tui/data.rs`, `src-tauri/src/cli/i18n.rs`.
- **Round 3 owner:** TUI-provider subagent owns `src-tauri/src/cli/tui/form.rs`, `src-tauri/src/cli/tui/form/provider_state.rs`, `src-tauri/src/cli/tui/form/provider_state_loading.rs`, `src-tauri/src/cli/tui/form/provider_json.rs`, `src-tauri/src/cli/tui/form/provider_templates.rs`, `src-tauri/src/cli/tui/runtime_actions/providers.rs`, `src-tauri/src/cli/tui/ui/forms/provider.rs`, `src-tauri/src/cli/tui/ui/providers.rs`.
- **Round 4 owner:** tests/docs subagent owns tests, parity summary docs, and verification notes.
- Do not let two implementation subagents edit `src-tauri/src/openclaw_config.rs`, `src-tauri/src/services/provider/mod.rs`, or `src-tauri/src/cli/i18n.rs` in the same round.

## Round-trip JSON path checklist

Allowed write targets for this feature:

- `models.providers.<provider-id>`
- `agents.defaults`
- `tools`
- `env`

Must preserve unless upstream baseline explicitly rewrites them:

- comments and JSON5 syntax outside the target subtree
- sibling root sections not owned by the current write
- unknown provider fields via `flatten extra`
- unrelated provider entries inside `models.providers`
- readable JSON5 output (do not downgrade to strict JSON or rewrite the whole file just to update one provider)

Recommended fixture shapes for tests:

- JSON5 file with comments before and after `models`
- config containing `tools.profile`, legacy `agents.defaults.timeout`, `env.vars`, `env.shellEnv`
- config with one saved provider plus one unrelated sibling section
- config where live provider order differs from stale TUI snapshot order

## Warning mapping that must stay explicit

- `config_parse_failed` -> source: `scan_openclaw_config_health()` parse path -> display: OpenClaw config view/banner -> test: `src-tauri/tests/openclaw_config.rs`
- unsupported `tools.profile` -> source: `scan_openclaw_health_from_value()` -> display: OpenClaw config view/banner -> test: `src-tauri/tests/openclaw_config.rs`
- `legacy_agents_timeout` -> source: `scan_openclaw_health_from_value()` and `set_agents_defaults()` stripping legacy field -> display: OpenClaw config view/banner -> test: `src-tauri/tests/openclaw_config.rs`
- malformed `env.vars` / `env.shellEnv` -> source: `scan_openclaw_health_from_value()` -> display: OpenClaw config view/banner -> test: `src-tauri/tests/openclaw_config.rs`

## Chunk 1: Backend parity and fixtures

### Task 1: Freeze the parity matrix and add backend regression fixtures

**Owner:** backend parity subagent

**Files:**
- Create: `docs/superpowers/plans/2026-03-18-openclaw-parity-gap-list.md`
- Create: `src-tauri/tests/openclaw_config.rs`
- Modify: `src-tauri/tests/support.rs`
- Test: `src-tauri/tests/openclaw_config.rs`

- [ ] **Step 1: Write the parity matrix before touching backend code**

Create `docs/superpowers/plans/2026-03-18-openclaw-parity-gap-list.md` with four sections: `provider/config`, `Env/Tools/Agents Defaults`, `TUI mapping`, `tests/docs`. Each row must record `baseline commit`, `upstream file + line`, `local file`, `status`, `handling recommendation`, and `owner`. Seed it from the spec, then link each row back to the baseline implementation under `/.upstream/cc-switch` instead of relying on memory.

- [ ] **Step 2: Add failing OpenClaw round-trip and warning tests**

Create `src-tauri/tests/openclaw_config.rs` with fixture-driven tests similar to this:

```rust
#[test]
fn set_agents_defaults_preserves_comments_and_strips_legacy_timeout() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();
    let openclaw_dir = home.join(".openclaw");
    std::fs::create_dir_all(&openclaw_dir).unwrap();
    let path = openclaw_dir.join("openclaw.json");
    std::fs::write(&path, r#"{
      // keep this comment
      agents: {
        defaults: {
          timeout: 30,
          timeoutSeconds: 45,
        },
      },
      tools: { profile: 'minimal' },
    }"#).unwrap();

    let defaults = OpenClawAgentsDefaults {
        model: Some(OpenClawDefaultModel {
            primary: "openai/gpt-4.1".to_string(),
            fallbacks: vec!["openai/gpt-4.1-mini".to_string()],
            extra: HashMap::new(),
        }),
        models: None,
        extra: HashMap::from([(String::from("timeoutSeconds"), json!(45))]),
    };

    set_agents_defaults(&defaults).unwrap();
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("keep this comment"));
    assert!(!raw.contains("timeout:"));
    assert!(raw.contains("timeoutSeconds"));
}
```

Also add tests for parse warnings, unsupported `tools.profile`, malformed `env.vars` / `env.shellEnv`, and preserving non-target root sections while updating `env` / `tools`. Add one shared JSON5 fixture that exercises all of these boundaries at once:

- provider add/remove under `models.providers.<provider-id>` without rewriting `models.mode`
- preserving unrelated providers and unknown provider keys
- updating `agents.defaults` without clobbering sibling `agents` keys
- updating default model without rewriting unrelated `models.providers` entries

- [ ] **Step 3: Run the new backend tests and capture the failing contract**

Run from `src-tauri/`:

```bash
cargo test --test openclaw_config -- --nocapture
```

Expected: FAIL on at least one round-trip or warning assertion that still differs from the upstream baseline.

- [ ] **Step 4: Add any missing shared test helpers, but keep them tiny**

If the new tests need helper functions, add only focused helpers to `src-tauri/tests/support.rs`, for example:

```rust
pub fn write_test_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent directory");
    }
    std::fs::write(path, content).expect("write fixture file");
}
```

Do not move business logic into test helpers.

- [ ] **Step 5: Re-run the backend fixture file after helper cleanup**

Run from `src-tauri/`:

```bash
cargo test --test openclaw_config -- --nocapture
```

Expected: still FAIL, but now only on product behavior, not on broken fixtures.

- [ ] **Step 6: Checkpoint commit only if the user explicitly asks for commits**

If and only if the user has requested checkpoint commits by execution time:

```bash
git add docs/superpowers/plans/2026-03-18-openclaw-parity-gap-list.md src-tauri/tests/openclaw_config.rs src-tauri/tests/support.rs
git commit -m "test(openclaw): add parity fixtures for config contract"
```

### Task 2: Align `openclaw_config.rs` and provider schema with upstream JSON5 contract

**Owner:** backend parity subagent

**Files:**
- Modify: `src-tauri/src/openclaw_config.rs`
- Modify: `src-tauri/src/provider.rs`
- Test: `src-tauri/tests/openclaw_config.rs`

- [ ] **Step 1: Align the OpenClaw serde structs with the upstream typed baseline, and preserve everything else through `flatten`**

Update `OpenClawProviderConfig`, `OpenClawModelEntry`, and `OpenClawModelCost` so the typed layer matches the upstream baseline instead of inventing extra local fields. Only the baseline-typed keys should be explicit; everything else must survive through `flatten extra`. The target shape should look like this:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawModelEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<OpenClawModelCost>,
    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}
```

`OpenClawModelCost` should explicitly keep upstream `input` / `output`, and preserve any additional keys such as `cacheRead` or `cacheWrite` through `extra` instead of a new local-only typed expansion.

- [ ] **Step 2: Make each section writer update only its owned subtree**

Keep using the round-trip document flow in `src-tauri/src/openclaw_config.rs`, but make sure writes are scoped to the section being changed. The write path should keep the existing pattern:

```rust
pub fn set_tools_config(tools: &OpenClawToolsConfig) -> Result<OpenClawWriteOutcome, AppError> {
    let value = serde_json::to_value(tools)
        .map_err(|source| AppError::JsonSerialize { source })?;
    write_root_section("tools", &value)
}
```

Apply the same minimal-write rule to provider updates, `env`, `agents.defaults`, and default model writes.

- [ ] **Step 3: Keep upstream warning codes and legacy-field cleanup exact**

In `scan_openclaw_health_from_value()` and `set_agents_defaults()`, make the warnings and cleanup rules match the baseline commit. Keep the warning codes stable and keep legacy timeout stripping on write:

```rust
if defaults_obj.remove("timeout").is_some() {
    warnings.push(warning(
        "legacy_agents_timeout",
        "agents.defaults.timeout is deprecated; use agents.defaults.timeoutSeconds.",
        Some("agents.defaults.timeout"),
    ));
}
```

Do not invent new warning categories in this round.

- [ ] **Step 4: Run the targeted backend contract tests until they pass**

Run from `src-tauri/`:

```bash
cargo test --test openclaw_config -- --nocapture
cargo test openclaw_config::tests -- --nocapture
```

Expected: PASS for the new integration tests and the internal unit tests already in `src-tauri/src/openclaw_config.rs`.

- [ ] **Step 5: Checkpoint commit only if the user explicitly asks for commits**

If and only if the user has requested checkpoint commits by execution time:

```bash
git add src-tauri/src/openclaw_config.rs src-tauri/src/provider.rs src-tauri/src/settings.rs src-tauri/tests/openclaw_config.rs
git commit -m "fix(openclaw): align config round-trip contract with upstream"
```

### Task 3: Align ProviderService additive-mode behavior with upstream OpenClaw rules

**Owner:** backend parity subagent

**Files:**
- Modify: `src-tauri/src/services/provider/mod.rs`
- Modify: `src-tauri/src/services/provider/live.rs`
- Modify: `src-tauri/src/app_config.rs`
- Modify: `src-tauri/src/services/config.rs`
- Modify: `src-tauri/src/settings.rs`
- Test: `src-tauri/tests/provider_service.rs`
- Create: `src-tauri/tests/settings_current_provider.rs`
- Test: `src-tauri/tests/opencode_provider.rs`

- [ ] **Step 1: Add only the truly missing failing service-layer and settings-layer tests**

Keep the existing `provider_service.rs` coverage for incremental import and first-model-name rules. Add tests only for the gaps that are still uncovered, especially blank IDs and the settings-layer parity around `current_provider_openclaw`:

```rust
#[test]
fn provider_service_import_openclaw_live_skips_blank_ids_and_existing_entries() {
    // seed openclaw.json with "", "openai", and an already-saved provider
    // import should skip blank IDs and existing snapshot entries
}

#[test]
fn settings_current_provider_openclaw_matches_upstream_placeholder_behavior() {
    // manager.current should remain empty for additive mode
    // settings.current_provider_openclaw should follow the upstream getter/setter contract
}
```

Put the `current_provider_openclaw` assertions in `src-tauri/tests/settings_current_provider.rs` if that keeps `provider_service.rs` readable.

- [ ] **Step 2: Keep import/write/delete logic aligned with upstream rules, not local convenience**

In `src-tauri/src/services/provider/live.rs`, `src-tauri/src/services/provider/mod.rs`, and `src-tauri/src/settings.rs`, keep these rules exact:

```rust
for (id, provider_config) in providers {
    if id.trim().is_empty() || provider_config.models.is_empty() {
        continue;
    }
    if manager.providers.contains_key(&id) {
        continue;
    }

    let name = provider_config
        .models
        .first()
        .and_then(|model| model.name.clone())
        .unwrap_or_else(|| id.clone());
    // insert provider without setting manager.current
}
```

Also align the settings helpers with upstream:

```rust
pub fn get_current_provider(app_type: &AppType) -> Option<String> { /* include OpenClaw */ }

pub fn set_current_provider(app_type: &AppType, id: Option<&str>) -> Result<(), AppError> {
    /* include current_provider_openclaw */
}
```

Switching OpenClaw must still sync only the target live entry, keep additive mode behavior, and avoid turning OpenClaw into a single-current-provider app, while the settings layer stays consistent with the upstream placeholder contract.

- [ ] **Step 3: Preserve OpenClaw and OpenCode additive semantics separately**

Use `src-tauri/tests/opencode_provider.rs` as the guardrail. After changing OpenClaw service code, OpenCode additive sync still needs to behave exactly as before. Do not refactor the shared additive path unless a failing test forces it.

- [ ] **Step 4: Run the targeted service tests**

Run from `src-tauri/`:

```bash
cargo test --test provider_service openclaw -- --nocapture
cargo test --test settings_current_provider openclaw -- --nocapture
cargo test --test opencode_provider openclaw_add_syncs_all_providers_to_live_config -- --exact
```

Expected: PASS for all OpenClaw provider-service tests and no OpenCode regression.

- [ ] **Step 5: Checkpoint commit only if the user explicitly asks for commits**

If and only if the user has requested checkpoint commits by execution time:

```bash
git add src-tauri/src/services/provider/mod.rs src-tauri/src/services/provider/live.rs src-tauri/src/app_config.rs src-tauri/src/services/config.rs src-tauri/src/settings.rs src-tauri/tests/provider_service.rs src-tauri/tests/settings_current_provider.rs src-tauri/tests/opencode_provider.rs
git commit -m "fix(openclaw): align additive provider service behavior"
```

## Chunk 2: TUI config flow and provider mapping

### Task 4: Add OpenClaw config routes, snapshot data, and config-menu entry points

**Owner:** TUI-config subagent

**Files:**
- Modify: `src-tauri/src/cli/tui/route.rs`
- Modify: `src-tauri/src/cli/tui/ui.rs`
- Modify: `src-tauri/src/cli/tui/app/app_state.rs`
- Modify: `src-tauri/src/cli/tui/app/helpers.rs`
- Modify: `src-tauri/src/cli/tui/app/menu.rs`
- Modify: `src-tauri/src/cli/tui/app/content_config.rs`
- Modify: `src-tauri/src/cli/tui/data.rs`
- Modify: `src-tauri/src/cli/tui/ui/config.rs`
- Test: `src-tauri/src/cli/tui/app/tests.rs`
- Test: `src-tauri/src/cli/tui/ui/tests.rs`

- [ ] **Step 1: Add OpenClaw-specific config routes and menu items**

Extend the route and config enums so OpenClaw gets dedicated config surfaces instead of forcing everything through provider detail. Follow the existing `ConfigWebDav` pattern:

```rust
pub enum Route {
    Main,
    Providers,
    ProviderDetail { id: String },
    Config,
    ConfigWebDav,
    ConfigOpenClawEnv,
    ConfigOpenClawTools,
    ConfigOpenClawAgents,
    Settings,
    SettingsProxy,
}
```

Add matching `ConfigItem` variants such as `OpenClawEnv`, `OpenClawTools`, and `OpenClawAgents`, but only show them when `app_type == AppType::OpenClaw`. Make `visible_config_items` / `config_items_filtered` app-aware instead of relying on a filter-only signature. Keep the flow single-path: Enter on the top-level `Config` page pushes the dedicated OpenClaw sub-route, and the sub-route is where `Enter` / `e` opens the editor.

- [ ] **Step 2: Extend `ConfigSnapshot` with the OpenClaw slices needed by the UI**

Load the current OpenClaw config state once in `src-tauri/src/cli/tui/data.rs` and carry it into UI rendering:

```rust
pub struct ConfigSnapshot {
    pub config_path: PathBuf,
    pub config_dir: PathBuf,
    pub backups: Vec<BackupInfo>,
    pub common_snippet: String,
    pub common_snippets: CommonConfigSnippets,
    pub webdav_sync: Option<WebDavSyncSettings>,
    pub openclaw_env: Option<OpenClawEnvConfig>,
    pub openclaw_tools: Option<OpenClawToolsConfig>,
    pub openclaw_agents_defaults: Option<OpenClawAgentsDefaults>,
    pub openclaw_warnings: Vec<OpenClawHealthWarning>,
}
```

Only populate the OpenClaw-specific fields when `app_type` is OpenClaw.

- [ ] **Step 3: Add failing route and config-menu tests first**

Extend `src-tauri/src/cli/tui/app/tests.rs` with tests like:

```rust
#[test]
fn openclaw_config_menu_exposes_env_tools_and_agents_items() {
    let mut app = App::new(Some(AppType::OpenClaw));
    app.route = Route::Config;
    let items = visible_config_items(&app.app_type, &app.filter);
    assert!(items.contains(&ConfigItem::OpenClawEnv));
    assert!(items.contains(&ConfigItem::OpenClawTools));
    assert!(items.contains(&ConfigItem::OpenClawAgents));
}
```

Add a second test proving non-OpenClaw apps do not see these items. Do not assert on final localized labels in Task 4; Task 4 owns route gating and wiring, while Task 5 owns the OpenClaw config copy and warning strings.

- [ ] **Step 4: Implement the route wiring and renderers, then run focused tests**

Wire all three places together in the same task: route enum, key dispatch/menu handling, and the central render match in `src-tauri/src/cli/tui/ui.rs`.

Run from `src-tauri/`:

```bash
cargo test openclaw_config_menu_exposes_env_tools_and_agents_items -- --exact
cargo test openclaw_config -- --nocapture
```

Expected: PASS for the new route/config tests and any related UI snapshot checks you added.

- [ ] **Step 5: Checkpoint commit only if the user explicitly asks for commits**

If and only if the user has requested checkpoint commits by execution time:

```bash
git add src-tauri/src/cli/tui/route.rs src-tauri/src/cli/tui/ui.rs src-tauri/src/cli/tui/app/app_state.rs src-tauri/src/cli/tui/app/helpers.rs src-tauri/src/cli/tui/app/menu.rs src-tauri/src/cli/tui/app/content_config.rs src-tauri/src/cli/tui/data.rs src-tauri/src/cli/tui/ui/config.rs src-tauri/src/cli/tui/app/tests.rs src-tauri/src/cli/tui/ui/tests.rs
git commit -m "feat(tui): add openclaw config routes and snapshots"
```

### Task 5: Wire OpenClaw `Env / Tools / Agents Defaults` editors and health-warning display

**Owner:** TUI-config subagent

**Files:**
- Modify: `src-tauri/src/cli/tui/app/editor_state.rs`
- Modify: `src-tauri/src/cli/tui/app/content_config.rs`
- Modify: `src-tauri/src/cli/tui/runtime_actions/mod.rs`
- Modify: `src-tauri/src/cli/tui/runtime_actions/editor.rs`
- Modify: `src-tauri/src/cli/tui/ui/config.rs`
- Modify: `src-tauri/src/cli/i18n.rs`
- Test: `src-tauri/src/cli/tui/app/tests.rs`
- Test: `src-tauri/src/cli/tui/ui/tests.rs`

- [ ] **Step 1: Add editor submit variants for each OpenClaw config surface**

Extend `EditorSubmit` so each OpenClaw config slice has an explicit submit target:

```rust
pub enum EditorSubmit {
    ProviderFormApplyJson,
    ProviderFormApplyOpenClawModels,
    ProviderAdd,
    ProviderEdit { id: String },
    ConfigOpenClawEnv,
    ConfigOpenClawTools,
    ConfigOpenClawAgents,
    ConfigWebDavSettings,
}
```

Use `EditorKind::Json` for `Env` and `Tools`. For `Agents`, choose JSON unless a small focused form can be implemented without exploding file size.

- [ ] **Step 2: Add failing editor-submit tests before the handlers**

Write tests in `src-tauri/src/cli/tui/app/tests.rs` for the full closed loop, and require one explicit closure per entry point: `Env`, `Tools`, and `Agents Defaults`. Either write three named tests or one parameterized matrix test, but it must cover `Config -> sub-route -> editor submit target -> save -> UiData reload -> warning visible/cleared` for all three entries. Add UI tests in `src-tauri/src/cli/tui/ui/tests.rs` that assert warning copy is visible when `ConfigSnapshot.openclaw_warnings` is non-empty.

In the same task, make `src-tauri/src/cli/i18n.rs` the single owner of the final OpenClaw config copy: add dedicated keys for the `Env`, `Tools`, and `Agents Defaults` config items, then add one `src-tauri/src/cli/tui/ui/tests.rs` assertion that the OpenClaw config screen renders those keys instead of hard-coded strings.

- [ ] **Step 3: Implement editor parsing with backend source-of-truth helpers**

In `src-tauri/src/cli/tui/app/content_config.rs`, keep the sub-route behavior narrow: only the OpenClaw config routes should open the editor. In `src-tauri/src/cli/tui/runtime_actions/editor.rs`, parse the edited JSON into the backend types and route through the existing config helpers instead of duplicating validation in the TUI:

```rust
let env: OpenClawEnvConfig = serde_json::from_str(&content)
    .map_err(|err| AppError::Message(format!("invalid OpenClaw env JSON: {err}")))?;
crate::openclaw_config::set_env_config(&env)?;
*ctx.data = UiData::load(&ctx.app.app_type)?;
```

Do the same for `OpenClawToolsConfig` and `OpenClawAgentsDefaults`, and surface backend warning messages/toasts after save when applicable.

- [ ] **Step 4: Render health warnings where the user actually sees them**

In `src-tauri/src/cli/tui/ui/config.rs`, show warning lines above the OpenClaw config list/detail when `openclaw_warnings` is non-empty. Keep the banner scoped to the OpenClaw config surfaces. Do not add generic stream-check or runtime request failures into this banner.

- [ ] **Step 5: Run the focused config TUI tests**

Run from `src-tauri/`:

```bash
cargo test openclaw_config_menu -- --nocapture
cargo test openclaw_config_route_ -- --nocapture
cargo test openclaw_config_warning -- --nocapture
```

Expected: PASS for the new route/editor/warning tests.

- [ ] **Step 6: Checkpoint commit only if the user explicitly asks for commits**

If and only if the user has requested checkpoint commits by execution time:

```bash
git add src-tauri/src/cli/tui/app/editor_state.rs src-tauri/src/cli/tui/app/content_config.rs src-tauri/src/cli/tui/runtime_actions/mod.rs src-tauri/src/cli/tui/runtime_actions/editor.rs src-tauri/src/cli/tui/ui/config.rs src-tauri/src/cli/i18n.rs src-tauri/src/cli/tui/app/tests.rs src-tauri/src/cli/tui/ui/tests.rs
git commit -m "feat(tui): add openclaw config editors and warnings"
```

### Task 6: Align OpenClaw provider form, presets, and provider list/detail semantics

**Owner:** TUI-provider subagent

**Files:**
- Modify: `src-tauri/src/cli/tui/form.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_state.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_state_loading.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_json.rs`
- Modify: `src-tauri/src/cli/tui/form/provider_templates.rs`
- Modify: `src-tauri/src/cli/tui/app/form_handlers/provider.rs`
- Modify: `src-tauri/src/cli/tui/runtime_actions/providers.rs`
- Modify: `src-tauri/src/cli/tui/ui/forms/provider.rs`
- Modify: `src-tauri/src/cli/tui/ui/providers.rs`
- Test: `src-tauri/src/cli/tui/form/tests.rs`
- Test: `src-tauri/src/cli/tui/ui/tests.rs`

- [ ] **Step 1: Add failing form tests for upstream-only fields that are still missing**

Extend `src-tauri/src/cli/tui/form/tests.rs` to cover the missing OpenClaw model fields and presets, for example:

```rust
#[test]
fn provider_add_form_openclaw_roundtrip_preserves_reasoning_input_max_tokens_and_cache_costs() {
    let provider = Provider::with_id(
        "oclaw1".to_string(),
        "OpenClaw Provider".to_string(),
        json!({
            "api": "openai-responses",
            "models": [{
                "id": "gpt-4.1",
                "name": "GPT-4.1",
                "reasoning": true,
                "input": ["text", "image"],
                "contextWindow": 128000,
                "maxTokens": 8192,
                "cost": {
                    "input": 2.0,
                    "output": 8.0,
                    "cacheRead": 1.0,
                    "cacheWrite": 4.0
                }
            }]
        }),
        None,
    );

    let form = ProviderAddFormState::from_provider(AppType::OpenClaw, &provider);
    let roundtrip = form.to_provider_json_value();
    assert_eq!(roundtrip["settingsConfig"]["models"][0]["maxTokens"], 8192);
    assert_eq!(roundtrip["settingsConfig"]["models"][0]["cost"]["cacheRead"], 1.0);
}
```

Also add a test proving `provider_builtin_template_defs(AppType::OpenClaw)` no longer aliases OpenCode.

- [ ] **Step 2: Give OpenClaw its own template/preset mapping**

Update `src-tauri/src/cli/tui/form/provider_templates.rs` so OpenClaw stops borrowing `PROVIDER_TEMPLATE_DEFS_OPENCODE`. Mirror the upstream preset intent with a dedicated OpenClaw template list, even if the first pass only exposes `Custom` plus a small set of upstream-backed presets.

Reuse the OpenClaw copy keys introduced in Task 5. Do not reopen `src-tauri/src/cli/i18n.rs` in this task unless the Task 5 owner explicitly hands off a follow-up micro-change.

- [ ] **Step 3: Preserve full `models[]` arrays instead of shrinking to one primary model**

In `provider_state_loading.rs` and `provider_json.rs`, make the OpenClaw path preserve the full array and unknown per-model fields. Keep the existing editor-based `openclaw_models` flow instead of projecting everything back to the OpenCode single-model fields. The TUI summary can still show the first model as the default/primary model.

- [ ] **Step 4: Keep provider list/detail semantics additive and OpenClaw-specific**

In `runtime_actions/providers.rs` and `ui/providers.rs`, verify these behaviors remain true after the form work:

```rust
assert!(all.contains("s add/remove"));
assert!(all.contains("x set default"));
assert!(!all.contains("s switch"));
assert!(!all.contains("stream check"));
```

Also preserve the OpenClaw status copy (`default`, `in config + saved`, `live only`, `saved only`, `untracked`) and show the effective default model id in provider detail.

- [ ] **Step 5: Run the focused provider-form and provider-UI tests**

Run from `src-tauri/`:

```bash
cargo test provider_add_form_openclaw -- --nocapture
cargo test openclaw_provider_ -- --nocapture
```

Expected: PASS for the OpenClaw form round-trip tests and provider list/detail rendering tests.

- [ ] **Step 6: Checkpoint commit only if the user explicitly asks for commits**

If and only if the user has requested checkpoint commits by execution time:

```bash
git add src-tauri/src/cli/tui/form.rs src-tauri/src/cli/tui/form/provider_state.rs src-tauri/src/cli/tui/form/provider_state_loading.rs src-tauri/src/cli/tui/form/provider_json.rs src-tauri/src/cli/tui/form/provider_templates.rs src-tauri/src/cli/tui/app/form_handlers/provider.rs src-tauri/src/cli/tui/runtime_actions/providers.rs src-tauri/src/cli/tui/ui/forms/provider.rs src-tauri/src/cli/tui/ui/providers.rs src-tauri/src/cli/tui/form/tests.rs src-tauri/src/cli/tui/ui/tests.rs
git commit -m "feat(tui): align openclaw provider flow with upstream fields"
```

## Chunk 3: Final regression lock and verification

### Task 7: Lock remaining regressions, document parity state, and verify the whole slice

**Owner:** tests/docs subagent, then independent reviewer

**Files:**
- Modify: `docs/superpowers/plans/2026-03-18-openclaw-parity-gap-list.md`
- Modify: `src-tauri/tests/provider_service.rs`
- Modify: `src-tauri/tests/opencode_provider.rs`
- Modify: `src-tauri/src/cli/tui/app/tests.rs`
- Modify: `src-tauri/src/cli/tui/form/tests.rs`
- Modify: `src-tauri/src/cli/tui/ui/tests.rs`
- Optional Modify: `README.md`
- Optional Modify: `README_ZH.md`

- [ ] **Step 1: Mark each parity-gap row with final status and evidence**

Update `docs/superpowers/plans/2026-03-18-openclaw-parity-gap-list.md` so each row ends with one of: `closed`, `deferred`, `explicitly out of scope`. Add the evidence column with the exact test name or file path that now covers the row.

- [ ] **Step 2: Add any last missing regression tests instead of hand-waving them**

If the implementation exposed gaps that are still only manually checked, add tests now. Likely candidates are:

```rust
#[test]
fn openclaw_config_warning_banner_is_hidden_when_health_scan_is_clean() {}

#[test]
fn provider_service_delete_openclaw_preserves_other_live_providers_and_root_sections() {}
```

Prefer adding a test over adding a note that says "manually verified".

- [ ] **Step 3: Run the focused verification set first**

Run from `src-tauri/`:

```bash
cargo test --test openclaw_config -- --nocapture
cargo test --test provider_service openclaw -- --nocapture
cargo test --test opencode_provider openclaw -- --nocapture
cargo test openclaw_config_menu -- --nocapture
cargo test openclaw_config_route_ -- --nocapture
cargo test openclaw_config_warning -- --nocapture
cargo test provider_add_form_openclaw -- --nocapture
cargo test openclaw_provider_ -- --nocapture
```

Expected: PASS for the full OpenClaw-targeted slice.

- [ ] **Step 4: Run repository-level verification before claiming completion**

Run from `src-tauri/`:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: all commands pass. If any one of these commands fails, the work is not complete. If the failure is confirmed to be unrelated and pre-existing, record it in the parity gap list and hand off the branch as `blocked`, not `complete`.

- [ ] **Step 5: Request an independent parity review before any completion claim**

Dispatch an independent reviewer with this checklist:

```text
1. Compare backend behavior against upstream commit 3dad255a2a926d05bd39c1015874a3406de1b303.
2. Check that OpenClaw TUI keeps upstream field meaning even when the layout differs.
3. Confirm that OpenClaw round-trip writes do not rewrite unrelated JSON5 content.
4. Confirm that health warnings come from backend source-of-truth code, not duplicated TUI logic.
```

Do not skip this review even if the tests are green.

- [ ] **Step 6: Resolve or classify review findings before any completion claim**

If the independent reviewer reports `critical` or `important` findings, fix them or classify them as `deferred` / `explicitly out of scope` in `docs/superpowers/plans/2026-03-18-openclaw-parity-gap-list.md`, then re-run the affected focused tests and the repository-level verification set. Only after that may the branch be treated as done.

- [ ] **Step 7: Final checkpoint commit only if the user explicitly asks for commits**

If and only if the user has requested checkpoint commits by execution time:

```bash
git add docs/superpowers/plans/2026-03-18-openclaw-parity-gap-list.md src-tauri/tests/provider_service.rs src-tauri/tests/opencode_provider.rs src-tauri/src/cli/tui/app/tests.rs src-tauri/src/cli/tui/form/tests.rs src-tauri/src/cli/tui/ui/tests.rs
# Add README.md or README_ZH.md only if this task actually modified them.
git commit -m "feat(openclaw): finish upstream parity alignment"
```

## Execution handoff

When execution starts, do not batch everything into one subagent. Use fresh subagents per task group in this order:

1. Backend parity (`Task 1` -> `Task 3`)
2. TUI config (`Task 4` -> `Task 5`)
3. TUI provider mapping (`Task 6`)
4. Tests/docs (`Task 7`)
5. Independent review

For each task, require the implementation subagent to report:

- closed parity gaps
- changed files
- commands run
- remaining risks
- whether another subagent now owns the next step

Plan complete and saved to `docs/superpowers/plans/2026-03-18-openclaw-support-alignment.md`. Ready to execute?
