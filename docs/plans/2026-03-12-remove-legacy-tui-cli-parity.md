# Remove Legacy TUI And Expand CLI Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the dead legacy interactive TUI path and expose the highest-value TUI capabilities through non-interactive CLI commands.

**Architecture:** Keep the existing service layer as the single source of truth. Shrink `interactive::run()` down to a TTY gate plus ratatui entrypoint, then add thin clap command adapters that call existing provider, WebDAV, skills, and local-environment services.

**Tech Stack:** Rust, clap, existing `services::*`, existing integration tests under `src-tauri/tests/`

---

### Task 1: Remove legacy interactive TUI

**Files:**
- Delete: `src-tauri/src/cli/interactive/legacy.rs`
- Delete: `src-tauri/src/cli/interactive/provider.rs`
- Delete: `src-tauri/src/cli/interactive/mcp.rs`
- Delete: `src-tauri/src/cli/interactive/prompts.rs`
- Delete: `src-tauri/src/cli/interactive/config.rs`
- Delete: `src-tauri/src/cli/interactive/skills.rs`
- Delete: `src-tauri/src/cli/interactive/settings.rs`
- Delete: `src-tauri/src/cli/interactive/utils.rs`
- Modify: `src-tauri/src/cli/interactive/mod.rs`
- Modify: `src-tauri/src/cli/i18n.rs`

**Step 1: Write the failing tests**

- Add unit tests in `src-tauri/src/cli/interactive/mod.rs` for:
  - non-TTY path returns `texts::interactive_requires_tty()` directly
  - `CC_SWITCH_LEGACY_TUI=1` no longer routes to a legacy module and instead returns a removal error or warning-backed error

**Step 2: Run the focused tests to watch them fail**

Run: `cargo test cli::interactive::`

**Step 3: Replace the legacy branch with a thin TTY gate**

- Remove `pub mod legacy;`
- Keep only `TTY -> crate::cli::tui::run(app)`
- Return a direct `interactive_requires_tty()` error for non-TTY
- Remove legacy-only i18n helpers that are no longer referenced

**Step 4: Re-run the focused tests**

Run: `cargo test cli::interactive::`

**Step 5: Delete the dead legacy files and run a compile check**

Run: `cargo test cli::interactive:: cli::tui::ui::tests::nav_does_not_show_manage_prefix_or_view_config`

### Task 2: Add provider health and model discovery CLI commands

**Files:**
- Modify: `src-tauri/src/cli/mod.rs`
- Modify: `src-tauri/src/cli/commands/provider.rs`
- Modify: `src-tauri/tests/provider_commands.rs`

**Step 1: Write the failing tests**

- Add clap parsing tests in `src-tauri/src/cli/mod.rs` for:
  - `cc-switch provider stream-check demo`
  - `cc-switch provider fetch-models demo`
- Add focused command/helper tests in `src-tauri/tests/provider_commands.rs` for:
  - extracting a provider's base URL and auth from stored config
  - formatting a stream-check result line set or model list output helper

**Step 2: Run the focused tests to watch them fail**

Run: `cargo test parses_provider_ && cargo test provider_commands`

**Step 3: Implement thin provider command adapters**

- Add `ProviderCommand::StreamCheck { id }`
- Add `ProviderCommand::FetchModels { id }`
- Reuse `StreamCheckService` and `ProviderService::fetch_provider_models`
- Resolve the provider by current app, extract base URL/auth from existing stored config, print stable CLI output

**Step 4: Re-run the focused tests**

Run: `cargo test parses_provider_ && cargo test provider_commands`

### Task 3: Add `config webdav` CLI subtree

**Files:**
- Modify: `src-tauri/src/cli/commands/mod.rs`
- Modify: `src-tauri/src/cli/commands/config.rs`
- Create: `src-tauri/src/cli/commands/config_webdav.rs`
- Modify: `src-tauri/tests/webdav_settings.rs`
- Modify: `src-tauri/src/cli/mod.rs`

**Step 1: Write the failing tests**

- Add clap parsing tests in `src-tauri/src/cli/mod.rs` for:
  - `cc-switch config webdav show`
  - `cc-switch config webdav set --base-url ... --username ... --password ...`
  - `cc-switch config webdav check-connection`
- Add command-facing tests in `src-tauri/tests/webdav_settings.rs` for:
  - set/clear/show helpers preserving normalized settings
  - Jianguoyun preset helper

**Step 2: Run the focused tests to watch them fail**

Run: `cargo test parses_config_webdav_ && cargo test webdav_settings`

**Step 3: Implement the WebDAV command module**

- Keep `config.rs` as the parent clap tree
- Add `config webdav` subcommands:
  - `show`
  - `set`
  - `clear`
  - `jianguoyun`
  - `check-connection`
  - `upload`
  - `download`
  - `migrate-v1-to-v2`
- Reuse `set_webdav_sync_settings`, `get_webdav_sync_settings`, `webdav_jianguoyun_preset`, and `WebDavSyncService`

**Step 4: Re-run the focused tests**

Run: `cargo test parses_config_webdav_ && cargo test webdav_settings`

### Task 4: Add `env tools` and repo toggle CLI coverage

**Files:**
- Modify: `src-tauri/src/cli/commands/env.rs`
- Modify: `src-tauri/src/cli/commands/skills.rs`
- Modify: `src-tauri/src/cli/mod.rs`

**Step 1: Write the failing tests**

- Add clap parsing tests in `src-tauri/src/cli/mod.rs` for:
  - `cc-switch env tools`
  - `cc-switch skills repos enable owner/name`
  - `cc-switch skills repos disable owner/name`
- Add small unit tests near `env.rs` / `skills.rs` for status formatting and repo-spec parsing with enable-state preservation

**Step 2: Run the focused tests to watch them fail**

Run: `cargo test parses_env_tools && cargo test parses_skills_repo_ && cargo test cli::commands::skills::`

**Step 3: Implement command support**

- Add `EnvCommand::Tools` using `services::local_env_check::check_local_environment`
- Extend `SkillReposCommand` with `Enable` and `Disable`
- Reuse `SkillService::list_repos` plus `SkillService::upsert_repo` to flip `enabled`

**Step 4: Re-run the focused tests**

Run: `cargo test parses_env_tools && cargo test parses_skills_repo_ && cargo test cli::commands::skills::`

### Task 5: Final verification and cleanup

**Files:**
- Modify as needed based on formatter/test output

**Step 1: Format the crate**

Run: `cargo fmt`

**Step 2: Run targeted verification for touched areas**

Run: `cargo test cli::interactive:: cli::tests:: provider_commands webdav_settings`

**Step 3: Run the full suite**

Run: `cargo test`

**Step 4: Check git diff for accidental churn**

Run: `git status --short && git diff -- src-tauri/src/cli src-tauri/tests docs/plans/2026-03-12-remove-legacy-tui-cli-parity.md`
