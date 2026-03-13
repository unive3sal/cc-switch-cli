# Install And Update Alignment Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Align the CLI self-update and one-line installer behavior with the upstream updater model while making overwrite behavior safe for existing installations.

**Architecture:** Treat the release manifest as the single source of truth for update metadata, platform asset URLs, and signatures. Keep the CLI/TUI backend update behavior centralized in `src-tauri/src/cli/commands/update.rs`, and keep installer policy inside `install.sh` with a small amount of supporting release-workflow logic so shell and Rust flows stay consistent.

**Tech Stack:** Bash, Rust, GitHub Actions, `reqwest`, `serde`, `semver`, `minisign-verify`, Rust unit/integration tests

---

### Task 1: Lock install overwrite policy with failing tests first

**Files:**
- Create: `src-tauri/tests/install_script.rs`
- Modify: `install.sh`

**Step 1: Write the failing non-interactive overwrite test**

Create a Rust integration test that:

```rust
let output = harness.run(&[]);
assert!(!output.status.success());
assert!(String::from_utf8_lossy(&output.stderr).contains("CC_SWITCH_FORCE=1"));
```

The harness should:

- create a fake existing `cc-switch` in the target install dir
- stub `curl` and `uname`
- run `bash install.sh` without a TTY-backed stdin

**Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --test install_script install_script_requires_force_for_non_tty_overwrite
```

Expected: FAIL because current `install.sh` silently overwrites existing binaries.

**Step 3: Write the failing PATH-shadow warning test**

Add a second test that sets up:

- an existing `cc-switch` earlier in `PATH`
- a target install path with an older binary
- `CC_SWITCH_FORCE=1`

Assert the script succeeds but emits a warning that the new install may be shadowed.

**Step 4: Write the failing Linux libc override test**

Add a test that sets `CC_SWITCH_LINUX_LIBC=glibc` and asserts the stub downloader was asked for `cc-switch-cli-linux-x64.tar.gz` instead of the musl archive.

**Step 5: Do not commit**

Per repo instructions, do not create a git commit unless the user explicitly asks.

### Task 2: Implement safe installer overwrite behavior and asset selection

**Files:**
- Modify: `install.sh:1-177`
- Verify: `README.md:112-183`
- Verify: `README_ZH.md:114-185`

**Step 1: Detect existing installs before downloading**

Add helpers that inspect:

- `TARGET`
- `command -v cc-switch`
- the resolved installed version when executable

Do this before any download so the script fails fast.

**Step 2: Add overwrite gating with TTY-aware prompting**

Implement this policy:

- if no existing install is found, continue without prompting
- if `CC_SWITCH_FORCE=1`, continue and log that overwrite is forced
- if an existing install is found and a real terminal is available via `/dev/tty`, ask `Update` or `Cancel`
- if an existing install is found and no TTY is available, exit non-zero with a friendly message saying nothing was overwritten and `CC_SWITCH_FORCE=1` is required

Do not treat a piped stdin alone as proof that the session is non-interactive; `curl ... | bash` must still be able to prompt through `/dev/tty`.

**Step 3: Keep Linux default on musl, but add explicit override support**

Support:

- `CC_SWITCH_LINUX_LIBC=auto` or empty => musl first behavior
- `CC_SWITCH_LINUX_LIBC=musl`
- `CC_SWITCH_LINUX_LIBC=glibc`

Reject invalid values early with a clear error.

**Step 4: Improve replacement safety**

Install via a staged file in the target directory, then rename into place so replacement is less fragile than moving directly from `/tmp`.

**Step 5: Warn on PATH shadowing**

After install, compare `command -v cc-switch` with `TARGET`. If they differ, print a warning and a concrete next step.

### Task 3: Lock update backend behavior with failing tests first

**Files:**
- Modify: `src-tauri/src/cli/commands/update.rs`
- Modify: `src-tauri/Cargo.toml`

**Step 1: Write a failing manifest-selection test**

Add a unit test that serves a `latest.json`-style manifest and asserts the updater resolves the expected platform entry for the current platform.

For Linux-specific coverage, also add tests around a pure helper so you can assert musl-first and glibc-override behavior without depending on the host platform.

**Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test cli::commands::update::tests::fetch_update_manifest_resolves_platform_asset
```

Expected: FAIL because the current implementation does not fetch or parse a signed release manifest.

**Step 3: Write a failing signature-verification test**

Add a unit test that verifies a minisign-signed manifest or artifact signature passes with the trusted public key and fails after tampering.

**Step 4: Write a failing fallback test**

Add a test that makes the GitHub API path fail while keeping `/releases/latest/download/latest.json` available, and assert the updater still resolves the correct asset.

**Step 5: Keep the current downgrade tests**

Preserve the existing downgrade-policy tests and expand only the minimum needed to prove the new manifest path does not change the downgrade rules.

### Task 4: Rework update.rs to use an upstream-style signed manifest

**Files:**
- Modify: `src-tauri/src/cli/commands/update.rs:1-1088`
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/updater/minisign.pub`

**Step 1: Model the manifest format close to upstream**

Add serde structs for a `latest.json`-style payload with:

```rust
struct UpdateManifest {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
    platforms: BTreeMap<String, UpdatePlatformEntry>,
}
```

where each platform entry contains at least:

```rust
struct UpdatePlatformEntry {
    url: String,
    signature: String,
}
```

**Step 2: Centralize platform key resolution**

Add a helper that maps the current target to an updater platform key compatible with the release manifest, for example:

- `darwin-x86_64`
- `darwin-aarch64`
- `linux-x86_64`
- `linux-aarch64`
- `windows-x86_64`

When Linux override support is enabled, keep the public manifest platform key stable and use a secondary selector for musl/glibc asset preference if the manifest publishes both choices.

**Step 3: Fetch latest version from the signed manifest first**

Replace the current GitHub-API-first latest check with:

- fetch `releases/latest/download/latest.json`
- parse and validate the target version
- use that manifest for version comparison and asset resolution

Keep a narrow fallback path for older releases or transitional assets only if needed.

**Step 4: Verify the downloaded asset with the trusted public key**

Add a trusted public key file under `src-tauri/updater/minisign.pub` and verify the downloaded archive against the signature from `latest.json` using `minisign-verify`.

This should replace the current “checksum from release metadata/checksums.txt” trust model as the primary security check.

**Step 5: Keep existing safety behavior intact**

Preserve:

- explicit-version override behavior
- implicit downgrade skip behavior
- staged self-replacement and permission-denied messaging
- tagged/versionless asset compatibility only if required during migration

### Task 5: Teach the release workflow to publish the upstream-style manifest and signatures

**Files:**
- Modify: `.github/workflows/release.yml:163-286`
- Create or modify supporting script if needed under `scripts/`

**Step 1: Sign each release asset**

Use a release signing key from GitHub Actions secrets to produce minisign signatures for the published tarballs and zip.

**Step 2: Generate `latest.json`**

Create a manifest that includes:

- `version`
- optional `notes`
- `pub_date`
- `platforms` entries pointing at release asset download URLs
- embedded signature strings for the corresponding artifacts

Ensure the file is uploaded as `release-assets/latest.json` so `releases/latest/download/latest.json` works.

**Step 3: Publish the public key if needed for operational clarity**

If the repo benefits from it, upload the public key as a release asset too, but keep the updater trusting the copy embedded in source.

**Step 4: Keep existing archives unchanged**

Do not rename the primary archives unless the manifest generation absolutely requires it.

### Task 6: Update docs and run end-to-end verification

**Files:**
- Modify: `README.md`
- Modify: `README_ZH.md`

**Step 1: Document new installer controls**

Add brief install notes for:

- `CC_SWITCH_FORCE=1`
- `CC_SWITCH_LINUX_LIBC=glibc`
- non-TTY overwrite refusal

**Step 2: Format Rust code**

Run:

```bash
cargo fmt
```

**Step 3: Run focused install-script tests**

Run:

```bash
cargo test --test install_script
```

**Step 4: Run focused update tests**

Run:

```bash
cargo test cli::commands::update::tests::
```

**Step 5: Run a broader confidence suite**

Run:

```bash
cargo test update
bash -n install.sh
```

Expected: targeted install tests pass, update tests pass, and the shell syntax check passes.
