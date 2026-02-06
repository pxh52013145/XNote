# Plugin Runtime Stage 12 Log (Telemetry + Sequenced Request IDs)

## Stage 12 (2026-02-06)
- Goal: add runtime telemetry baseline and deterministic request ID sequencing as groundwork for multiplexed request handling.
- Scope:
  1) host-side telemetry counters;
  2) monotonic request ID generation in runtime session scope;
  3) preserve existing host/runtime behavior and pass all regressions.
- Status: done

## Changes
- Telemetry model introduced:
  - `RuntimeTelemetrySnapshot` added with counters:
    - `spawn_count`
    - `handshake_count`
    - `activation_request_count`
    - `reused_session_count`
    - `session_ping_failure_count`
    - `evicted_by_limit_count`
    - `evicted_by_idle_ttl_count`
  - file: `crates/xnote-core/src/plugin.rs`.
- Runtime state additions:
  - `ProcessPluginRuntime` now tracks:
    - `request_sequence`
    - `telemetry`
  - `telemetry_snapshot()` public accessor added.
  - file: `crates/xnote-core/src/plugin.rs`.
- Request ID sequencing:
  - added `next_request_id(prefix, plugin_id)`;
  - ping and activation now both use monotonic request IDs.
  - file: `crates/xnote-core/src/plugin.rs`.
- Telemetry instrumentation points:
  - spawn increments `spawn_count`;
  - successful handshake increments `handshake_count`;
  - activation send increments `activation_request_count`;
  - reuse success increments `reused_session_count`;
  - ping failure increments `session_ping_failure_count`;
  - limit/TTL evictions increment dedicated counters.
  - file: `crates/xnote-core/src/plugin.rs`.

## Tests
- integration tests added:
  - `process_runtime_worker_integration_telemetry_counts_spawn_reuse_and_activation`
  - `process_runtime_worker_integration_telemetry_counts_limit_eviction`
  - file: `crates/xnote-core/tests/plugin_runtime_integration.rs`.
- existing tests retained and passing for protocol/session behavior.

## Verification (2026-02-06 16:52)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (42 unit + 10 integration)
- `cargo check -p xnote-plugin-worker` ✅
- `cargo check -p xnote-ui` ✅

## Next
- Stage 13 proposal:
  1) introduce runtime request queue abstraction to prepare true multiplexing;
  2) expose telemetry to UI diagnostics surface;
  3) add counter for protocol mismatch and handshake rejection classes.
