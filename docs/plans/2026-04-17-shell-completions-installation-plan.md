# Shell Completions Installation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close issue #113 by turning the existing `cc-switch completions <shell>` generator into a practical shell-completion workflow that works well for ordinary users while staying predictable and low-intrusion for advanced users.

**Architecture:** Keep shell completion logic entirely inside the CLI layer as a stateless tool path. Reuse the existing `clap_complete` generator for script output, add a CLI-only installer/status/uninstall flow for bash and zsh, and treat `install.sh`, `cc-switch update`, and the README as thin wrappers around the same Rust command surface instead of separate sources of truth.

**Tech Stack:** Rust, `clap`, `clap_complete`, existing atomic file-write helpers in `src-tauri/src/config.rs`, Bash installer script, Rust unit/integration tests, README docs.

---

### Task 1: Lock the command surface and no-startup-state boundary with tests first

**Files:**
- Modify: `src-tauri/src/cli/mod.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src-tauri/src/cli/commands/completions.rs`

**Step 1: Write parsing tests for the new command surface**

Add CLI parsing tests that cover:

- `cc-switch completions bash` keeps working as the compatibility generator path
- `cc-switch completions zsh` keeps working as the compatibility generator path
- `cc-switch completions install`
- `cc-switch completions install --shell auto`
- `cc-switch completions install --shell zsh --activate`
- `cc-switch completions status`
- `cc-switch completions uninstall --shell bash`
- invalid mixed forms such as `cc-switch completions bash --activate` are rejected cleanly

The parsing contract should preserve the current generator syntax while adding explicit management subcommands for installation lifecycle actions, and it must make the precedence between positional shell generation and `install/status/uninstall` subcommands unambiguous.

**Step 2: Lock the startup-state bypass behavior**

Extend `command_requires_startup_state` tests in `src-tauri/src/main.rs` so these commands all skip `AppState::try_new_with_startup_recovery()`:

- generator path via `cc-switch completions bash`
- `cc-switch completions install ...`
- `cc-switch completions status ...`
- `cc-switch completions uninstall ...`

This is required to preserve the current “pure CLI utility” boundary even when the local database is missing, old, or too new.

**Step 3: Run focused tests and confirm they fail first**

Run:

```bash
cargo test cli::tests --lib
cargo test update_and_completions_skip_startup_state --bin cc-switch
```

Expected: FAIL until the new parser shape and startup-state exemptions exist.

### Task 2: Add a CLI-only completion installer/status/uninstall module

**Files:**
- Create: `src-tauri/src/cli/commands/completions.rs`
- Modify: `src-tauri/src/cli/commands/mod.rs`
- Modify: `src-tauri/src/cli/mod.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Define the lifecycle command surface**

Add a dedicated CLI module that supports:

- compatibility generator path: `cc-switch completions <shell>`
- installer path: `cc-switch completions install [--shell auto|bash|zsh] [--activate]`
- status path: `cc-switch completions status [--shell auto|bash|zsh]`
- uninstall path: `cc-switch completions uninstall [--shell auto|bash|zsh]`

Keep generator support for all `clap_complete` shells. Limit install/status/uninstall automation to bash and zsh in this issue, because that matches the issue scope and avoids premature shell-specific branching for fish/PowerShell.

**Step 2: Keep all logic stateless and local**

Implement the module without:

- touching `services/`
- reading provider state
- initializing `AppState`
- writing anything under the cc-switch database/config root unless explicitly needed for temporary CLI behavior

Use only local shell/home-directory inspection and file operations.

**Step 3: Reuse the existing generator**

Keep `clap_complete` as the single script generator. The new module should call through a shared helper instead of duplicating completion text generation logic in multiple places.

**Step 4: Return structured user-facing outcomes**

Make each action print concrete, user-readable outcomes:

- detected shell
- installed file path
- whether activation is already present
- what the next command or shell reload action should be

This output becomes the shared UX surface for direct CLI use, `install.sh`, and future automation.

The install command should support two clear user modes:

- conservative mode: `cc-switch completions install`
- one-shot ordinary-user mode: `cc-switch completions install --activate`

### Task 3: Implement bash/zsh install planning, activation, and cleanup

**Files:**
- Create: `src-tauri/src/cli/commands/completions.rs`
- Modify: `src-tauri/src/config.rs`

**Step 1: Add deterministic install-path planning helpers**

Add pure helpers that resolve:

- bash completion file path: `~/.local/share/bash-completion/completions/cc-switch`
- zsh completion file path: `~/.local/share/zsh/site-functions/_cc-switch`
- bash activation file: `~/.bashrc`
- zsh activation file: `~/.zshrc`

These helpers must be easy to unit test under temp-home overrides.

**Step 2: Install completion files with atomic writes**

Write the generated completion script to the planned target path using the existing atomic text-write helper patterns from `src-tauri/src/config.rs`.

Create parent directories as needed, but keep all writes limited to the user’s home-directory shell locations.

**Step 3: Make shell-rc activation explicit and idempotent**

Implement `--activate` as the only flag that is allowed to modify rc files automatically.

Use managed markers such as:

```text
# >>> cc-switch completions >>>
# <<< cc-switch completions <<<
```

Within that managed block:

- bash: source the installed completion file if it exists
- zsh: prepend the completion directory to `fpath`; if `.zshrc` already contains `compinit`, insert the block before the first `compinit` call, otherwise append a guarded `compinit` bootstrap with the smallest reasonable footprint

Repeated installs must update or preserve the managed block without duplication.

**Step 4: Keep ordinary-user flow simple without silent rc edits**

Default `install` behavior should:

- detect shell automatically when possible
- install the completion file
- not silently modify rc files
- print the exact next step needed to activate completions

For the ordinary-user one-shot path, `cc-switch completions install --activate` should be the documented happy path. This keeps activation explicit while avoiding any hidden rc-file edits.

**Step 5: Support status and uninstall for managed assets**

`status` should report:

- detected or selected shell
- completion file path and existence
- rc file path
- whether a managed activation block exists
- whether the current shell likely needs reload/restart

`uninstall` should:

- remove the managed completion file if present
- remove the managed activation block if present
- leave unrelated user shell config intact

### Task 4: Lock bash/zsh behavior with focused tests

**Files:**
- Modify: `src-tauri/src/cli/commands/completions.rs`
- Modify: `src-tauri/src/test_support.rs`
- Create if needed: `src-tauri/tests/completions_command.rs`

**Step 1: Add path-resolution and shell-detection tests**

Cover:

- auto shell detection from `SHELL`
- fallback behavior when `SHELL` is missing or unsupported
- bash and zsh path calculation under a temp home directory
- unsupported install shell errors for non-bash/zsh install attempts

**Step 2: Add install/activate idempotency tests**

Cover:

- installing a completion file creates the right parent directories
- `--activate` adds exactly one managed block
- rerunning `--activate` does not duplicate the block
- uninstall removes only the managed block and leaves surrounding user content unchanged

**Step 3: Add zsh placement tests**

Cover both cases:

- `.zshrc` already contains `compinit`, so the managed block is inserted before it
- `.zshrc` has no `compinit`, so the managed block appends the minimal guarded bootstrap

**Step 4: Run focused verification**

Run:

```bash
cargo test cli::commands::completions --lib
cargo test --test completions_command
```

Expected: PASS

### Task 5: Wire install/update/docs to the new CLI surface without duplicating policy

**Files:**
- Modify: `install.sh`
- Modify: `src-tauri/src/cli/commands/update.rs`
- Modify: `README.md`
- Modify: `README_ZH.md`

**Step 1: Keep `install.sh` as a thin wrapper**

After binary installation succeeds:

- detect the current shell for messaging only
- print a short recommended next step such as `cc-switch completions install --activate`
- also mention the conservative non-rc-edit path `cc-switch completions install`

Do not reimplement completion-path or rc-editing rules in Bash.

**Step 2: Keep `update` conservative**

After a successful self-update, print a reminder such as:

```text
If you want to install or refresh shell completions, run: cc-switch completions install
```

Do not automatically rewrite shell files during update.

**Step 3: Update docs for both ordinary and advanced users**

README and README_ZH should each document:

- the recommended ordinary-user path: `cc-switch completions install --activate`
- the conservative non-activation path: `cc-switch completions install`
- the compatibility generator path: `cc-switch completions bash`
- the advanced lifecycle commands: `status` and `uninstall`
- the scope note that automated install/activation currently targets bash and zsh, while raw generation remains available for other shells

### Task 6: Explicitly keep dynamic provider-id completion out of scope

**Files:**
- Verify only: this plan and the final docs

**Step 1: Do not couple completion generation to provider/runtime state**

Do not implement dynamic completion for:

- provider IDs
- MCP IDs
- prompt names
- other database-backed entities

That work would require a separate design because it would pressure the current no-startup-state completion boundary and add shell-specific runtime callback complexity.

**Step 2: Record the non-goal in docs or follow-up notes if needed**

If contributors are likely to ask for dynamic entity completion immediately after this lands, add a brief “not in this issue” note to the implementation comments or follow-up backlog.

### Task 7: Run end-to-end verification before claiming completion

**Files:**
- Modify if needed after failures: same files as above

**Step 1: Run formatting**

```bash
cargo fmt
```

**Step 2: Run focused command and module tests**

```bash
cargo test cli::tests --lib
cargo test cli::commands::completions --lib
cargo test --test completions_command
cargo test update_and_completions_skip_startup_state --bin cc-switch
```

**Step 3: Run a broader Rust verification pass**

```bash
cargo test
```

If unrelated failures exist, separate them explicitly from completion changes before claiming success.

**Step 4: Run a manual CLI sanity pass**

At minimum, manually verify these flows in a temp home directory:

- `cc-switch completions bash`
- `cc-switch completions install --shell bash`
- `cc-switch completions install --shell zsh --activate`
- `cc-switch completions status --shell zsh`
- `cc-switch completions uninstall --shell zsh`

---

## Acceptance Criteria

- `cc-switch completions <shell>` still works exactly as a raw generator path.
- `cc-switch completions install --activate` gives ordinary users a short, actionable path to enable bash/zsh completion.
- `cc-switch completions install` remains available as the conservative no-rc-edit path.
- rc-file changes are opt-in and idempotent.
- install/status/uninstall commands do not require startup-state/database initialization.
- `install.sh`, `update`, and README all point to the same Rust command surface instead of carrying separate shell-completion policies.
- The implementation explicitly excludes dynamic provider/entity completion from this issue.

## Risks And Mitigations

- **Risk:** zsh `compinit` ordering is easy to get wrong.
  **Mitigation:** test both pre-existing-`compinit` and no-`compinit` cases, and keep the managed block minimal.
- **Risk:** ordinary users may confuse “installed file” with “current shell already active”.
  **Mitigation:** always print the selected rc file and whether shell reload/restart is still required.
- **Risk:** shell-rc editing may feel too invasive.
  **Mitigation:** keep rc edits behind the explicit `--activate` flag.
- **Risk:** scope creep toward dynamic entity completion.
  **Mitigation:** codify it as a non-goal in the plan and docs for this issue.

## Verification Summary

This plan is ready to implement when:

- the new command surface is locked by tests first
- the CLI-only module owns all completion lifecycle logic
- bash/zsh install and uninstall behavior are idempotent
- startup-state bypass behavior is preserved
- docs and install/update entry points converge on the same CLI command
