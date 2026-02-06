# Infrastructure Bootstrap Log

## Stage 1 (2026-02-06)
- Goal: Establish baseline infrastructure for i18n, keybind, command bus, plugin module.
- Steps:
  1) Add `logs/` workflow rule to `AGENTS.md`
  2) Create core modules in `xnote-core` (`command`, `keybind`, `plugin`)
  3) Wire UI command and keybind to core definitions
  4) Add i18n dictionary and language switching support
  5) Run regression checks and record status
- Status: done

## Stage 2 (2026-02-06)
- Goal: Align base infrastructure closer to mainstream IDE patterns (VSCode/Zed/Obsidian)
- Steps:
  1) Add persisted app settings model (`settings.json`, schema_version)
  2) Add validated keymap override pipeline
  3) Add plugin policy persistence and boot-time policy loading
  4) Wire UI bootstrap to load locale/keymap/policy from settings
  5) Re-run regression and consolidate gap analysis
- Status: done

## Stage 3 (2026-02-06)
- Goal: Continue building IDE-grade foundation with contextual keybind and layered settings
- Steps:
  1) Implement keybind context engine (`when` expressions)
  2) Add contextual keymap rules in settings model
  3) Introduce user + project settings layered merge
  4) Apply effective settings at UI boot
  5) Keep persistence path for both user and project settings
- Status: done

## Stage 4 (2026-02-06)
- Goal: Build plugin lifecycle baseline (activation events + runtime state + failure handling)
- Steps:
  1) Add plugin activation events (`on_startup_finished`, `on_vault_opened`, `on_command`)
  2) Add plugin lifecycle state machine (`Registered/Activating/Active/Failed/Disabled`)
  3) Add trigger-based activation API with per-plugin runtime state tracking
  4) Add failure threshold policy (`max_failed_activations`) and disable-on-threshold
  5) Wire UI to trigger activation on startup/vault open/command execution
- Status: done

## Stage 5 (2026-02-06)
- Goal: Introduce host/runtime isolation contract with activation timeout/cancel semantics
- Steps:
  1) Add explicit runtime boundary traits (`PluginRuntime`, `RuntimeActivationSpec/Result`)
  2) Add host-side activation wrapper with timeout enforcement
  3) Extend lifecycle states with `Cancelled` and metrics (`total_activation_ms`)
  4) Add policy field `activation_timeout_ms` and map from settings
  5) Keep UI trigger flow compatible with new contract
- Status: done

## Stage 6 (2026-02-06)
- Goal: Upgrade from API-level isolation to execution-level isolation
- Steps:
  1) Introduce process-based runtime implementation for plugin activation
  2) Add host-side cancellation token flow and watchdog loop
  3) Add runtime selection and wiring at UI bootstrap
  4) Add tests for timeout/cancel/process outcomes
- Status: done
- Next: proceed to Stage 7 (plugin process protocol + IPC contract + handshake)

### Stage 6 Progress Note (2026-02-06 13:10)
- Started implementation pass for execution-level isolation.
- Focus this pass: process runtime + cancellation token contract + UI runtime selection path.
- Status: done

### Stage 6 Result Note (2026-02-06 13:28)
- Implemented process runtime in `xnote-core` (`ProcessPluginRuntime`) with watchdog loop and timeout kill path.
- Added cancellation token contract (`ActivationCancellation`) and host-boundary cancellation/timeout unification.
- Added runtime mode selection (`in_process` / `process`) and wired UI bootstrap + activation path to use selected mode.
- Expanded tests for process success/spawn failure/timeout + cancellation short-circuit.

## Verification (2026-02-06)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (28 passed)
- `cargo check -p xnote-ui` ✅

## Notes
- Current host/runtime boundary now supports both in-process and process runtime modes.
- Timeout and explicit cancellation both map to `Cancelled` lifecycle state and contribute to plugin runtime metrics.
