# Claude Provider Switch UX Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a first-use protection dialog before overwriting an existing Claude `settings.json`, and show a one-time post-switch tip about shared common config after the first real Claude provider switch.

**Architecture:** Keep the existing provider list key flow unchanged and intercept the behavior in TUI runtime actions. Add one new three-choice centered overlay for the pre-switch protection, and one reusable close-only notice overlay for the post-switch hint. Persist the one-time hint state in `AppSettings` so it survives restarts.

**Tech Stack:** Rust, ratatui TUI overlays, existing `ProviderService`, existing `AppSettings` persistence, Rust unit/integration tests.

---

### Task 1: Lock the new switch behaviors with tests

**Files:**
- Modify: `src-tauri/src/cli/tui/runtime_actions/providers.rs`
- Modify: `src-tauri/src/cli/tui/app/tests.rs`
- Modify: `src-tauri/src/cli/tui/ui/tests.rs`

**Step 1: Write the failing tests**

- Add a runtime-actions test for Claude first-use protection when `~/.claude/settings.json` exists and `current_id` is empty.
- Add a runtime-actions test for the one-time post-switch tip after a real `A -> B` switch.
- Add app overlay key handling tests for the new three-choice dialog.
- Add a UI render test to verify the new centered dialog shows all three actions.

**Step 2: Run tests to verify they fail**

Run: `cargo test cli::tui::runtime_actions::providers --lib`
Expected: FAIL because the new overlay/action types and behavior do not exist yet.

### Task 2: Add the new overlay/action plumbing

**Files:**
- Modify: `src-tauri/src/cli/tui/app/app_state.rs`
- Modify: `src-tauri/src/cli/tui/app/types.rs`
- Modify: `src-tauri/src/cli/tui/app/overlay_handlers/dialogs.rs`
- Modify: `src-tauri/src/cli/tui/app/overlay_handlers/views.rs`
- Modify: `src-tauri/src/cli/tui/ui/overlay/render.rs`
- Modify: `src-tauri/src/cli/tui/ui/overlay/basic.rs`
- Modify: `src-tauri/src/cli/i18n.rs`

**Step 1: Add the failing type usage first**

- Define the new provider-switch warning overlay state and the new runtime actions it can emit.
- Add dialog key handling for left/right selection, enter to execute, and esc to cancel.

**Step 2: Write the minimal rendering code**

- Render a centered dialog with balanced padding and three selectable actions: import, continue, cancel.
- Reuse the existing close-only confirm pattern for the one-time informational hint.

**Step 3: Run focused tests**

Run: `cargo test cli::tui::app::tests --lib`
Expected: PASS for the new overlay interaction tests.

### Task 3: Implement the Claude switch runtime behavior

**Files:**
- Modify: `src-tauri/src/cli/tui/runtime_actions/providers.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Implement the first-use guard**

- Detect Claude-only first-use overwrite risk from existing live settings plus empty current provider.
- Open the new warning overlay instead of switching immediately.

**Step 2: Implement follow-up actions**

- Add a runtime action to import the current Claude live config as a provider and reload UI data.
- Add a runtime action to continue the switch after the user confirms.

**Step 3: Implement the one-time post-switch hint**

- Detect the first real `A -> B` switch, excluding first-use empty-current cases and same-provider no-ops.
- Persist a `shown` flag in `AppSettings` after the hint is scheduled.
- If a proxy-required notice is also needed, queue the common-config hint to appear right after dismissal.

**Step 4: Run focused tests**

Run: `cargo test cli::tui::runtime_actions::providers --lib`
Expected: PASS for the new runtime behavior tests.

### Task 4: Verify end-to-end behavior

**Files:**
- Modify if needed after failures: same files as above

**Step 1: Run targeted library tests**

Run: `cargo test cli::tui::runtime_actions::providers --lib && cargo test cli::tui::app::tests --lib && cargo test cli::tui::ui::tests --lib`
Expected: PASS

**Step 2: Run broader verification**

Run: `cargo test`
Expected: PASS, or identify unrelated failures separately.

**Step 3: Format if required**

Run: `cargo fmt`
Expected: no diff after formatting.

---

## Execution Status

- 2026-03-18: Merge conflict resolution preserved the Task 1-4 Claude provider switch UX behavior while keeping OpenClaw support in the provider flows.
- Verified passing during merge resolution with `cargo fmt`, `cargo test cli::tui::runtime_actions::providers --lib`, `cargo test cli::tui::runtime_actions::editor --lib`, `cargo test cli::tui::app::tests --lib`, and `cargo test --test provider_service`.
- Broader verification completed on 2026-03-18 with `cargo test`; the full Rust test suite passed after fixing the Codex stale-takeover current-provider restore path.
