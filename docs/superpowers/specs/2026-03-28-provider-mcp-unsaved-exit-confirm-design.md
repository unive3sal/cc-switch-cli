# Provider And MCP Unsaved Exit Confirm Design

## Goal

When the user edits a Provider or MCP form in the ratatui TUI and presses `Esc` or `q`, the app should warn before closing the form if there are unsaved changes. The warning should cover both add and edit flows.

## Current Problem

`Provider` and `MCP` forms close immediately on `Esc` and `q`.

- This is easy to trigger by accident.
- The user can lose changes with no warning.
- The editor already has a save-before-close confirm, but the form layer does not.

## Scope

In scope:

- Provider add form
- Provider edit form
- MCP add form
- MCP edit form
- Top-level form close from `Esc` or `q`

Out of scope:

- Changing editor close behavior
- Changing picker or overlay close behavior
- Adding save-before-close for forms
- Extending this behavior to other routes

## Design

### 1. Form-level dirty detection

Both `ProviderAddFormState` and `McpAddFormState` will keep a normalized snapshot of their initial serialized content.

The dirty check will compare:

- the snapshot taken when the form is opened
- the current serialized content at the moment the user tries to close the form

This is preferred over a mutable `dirty` flag because form state changes come from several paths:

- direct text input
- picker selections
- toggles
- nested editors that write back into the form
- MCP env row add, edit, and delete

A snapshot comparison covers all of them without having to remember to set `dirty = true` in every branch.

### 2. Confirm flow

When a dirty Provider or MCP form receives top-level `Esc` or `q`, the app opens a confirm overlay instead of closing the form.

Behavior:

- `Enter` or `y`: discard form changes and close the form
- `Esc` or `n`: cancel the discard and return to the form
- clean form: close immediately, same as today

This confirm should use its own form-specific action instead of reusing the editor action. The editor confirm means "save before close". The form confirm means "discard changes and close". They are different flows.

### 3. Overlay text

The confirm should tell the user that the form has been modified and that leaving now will discard unsaved changes.

The wording should work for both Provider and MCP forms, so the same overlay can be reused across both.

### 4. Boundaries

The dirty check belongs to the form state and form close path, not to the route layer.

- Form state owns snapshot creation and comparison.
- Form handlers decide whether to close directly or open the confirm.
- Confirm handling performs the discard close action.

This keeps the logic local to the TUI form surface and avoids spreading form-specific conditions into unrelated route handlers.

## File Plan

Expected updates:

- `src-tauri/src/cli/tui/form.rs`
  - add initial snapshot fields to Provider and MCP form state
- `src-tauri/src/cli/tui/form/provider_state.rs`
  - initialize and compare Provider snapshots
- `src-tauri/src/cli/tui/form/mcp.rs`
  - initialize and compare MCP snapshots
- `src-tauri/src/cli/tui/app/types.rs`
  - add a form discard confirm action
- `src-tauri/src/cli/tui/app/form_handlers/mod.rs`
  - intercept top-level form close and open confirm when dirty
- `src-tauri/src/cli/tui/app/overlay_handlers/dialogs.rs`
  - handle confirm accept and cancel for form discard
- `src-tauri/src/cli/i18n/texts/...`
  - add confirm title and message strings if no suitable shared strings already exist
- `src-tauri/src/cli/tui/app/tests.rs`
  - add focused form-close confirmation tests

## Testing

Add focused tests for:

- dirty Provider add form + `Esc` opens confirm
- dirty Provider edit form + `Esc` opens confirm
- clean Provider form + `Esc` closes immediately
- dirty MCP add form + `Esc` opens confirm
- dirty MCP edit form + `Esc` opens confirm
- clean MCP form + `Esc` closes immediately
- confirm accept closes the form
- confirm cancel keeps the form open

The tests should target the maintained TUI behavior and stay narrow. No broad refactor is needed.

## Risks And Mitigations

- Snapshot drift because of non-deterministic serialization
  - Mitigation: compare the same internal serialization path used by the form state, not ad hoc display text.
- Missing a mutation path
  - Mitigation: use snapshot comparison instead of manual dirty flags.
- Behavior conflict with existing editor confirm
  - Mitigation: keep a separate confirm action for form discard.

## Success Criteria

- A modified Provider form does not close immediately on `Esc`.
- A modified MCP form does not close immediately on `Esc`.
- Clean forms still close immediately.
- Confirm accept discards the form and exits.
- Confirm cancel returns to the form with current edits intact.
