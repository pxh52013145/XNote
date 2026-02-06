# Plugin Runtime Stage 11 Log (Idle TTL + Diagnostics)

## Stage 11 (2026-02-06)
- Goal: improve persistent runtime session policy with idle-time cleanup and runtime diagnostics visibility.
- Scope:
  1) idle TTL-based session eviction;
  2) lightweight session diagnostics snapshot API;
  3) maintain passing unit/integration matrix.
- Status: done

## Changes
- Session policy configuration extension:
  - `session_idle_ttl_ms` added into `ProcessRuntimeConfig`.
  - normalization helper `normalized_session_idle_ttl_ms()` added.
  - file: `crates/xnote-core/src/plugin.rs`.
- Session model upgraded:
  - replaced raw transport map with `RuntimeSession { transport, last_used_at }`.
  - keep `session_order` for eviction policy alignment.
  - file: `crates/xnote-core/src/plugin.rs`.
- Host runtime policy behavior:
  - `evict_idle_sessions()` runs before activation when keep-alive is enabled.
  - `last_used_at` updates after each activation request.
  - stale sessions are terminated and removed deterministically.
  - files: `crates/xnote-core/src/plugin.rs`.
- Diagnostics API:
  - `RuntimeSessionSnapshot { session_key, idle_ms }`
  - `active_sessions_snapshot()` returns sorted session diagnostics.
  - file: `crates/xnote-core/src/plugin.rs`.

## Tests
- unit test:
  - `process_runtime_config_normalizes_session_policy_values`
  - validates lower bounds for ping timeout/session cap/idle ttl.
  - file: `crates/xnote-core/src/plugin.rs`.
- integration tests:
  - `process_runtime_worker_integration_idle_ttl_eviction`
  - `process_runtime_worker_integration_session_snapshot_exposed`
  - file: `crates/xnote-core/tests/plugin_runtime_integration.rs`.

## Verification (2026-02-06 16:39)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (42 unit + 8 integration)
- `cargo check -p xnote-plugin-worker` ✅
- `cargo check -p xnote-ui` ✅

## Next
- Stage 12 proposal:
  1) single-session multi-request correlation contract groundwork;
  2) runtime metrics counters (spawn/reuse/evict/ttl-expire);
  3) diagnostics export path to UI inspector panel.
