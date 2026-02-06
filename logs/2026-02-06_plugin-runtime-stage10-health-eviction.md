# Plugin Runtime Stage 10 Log (Health + Eviction)

## Stage 10 (2026-02-06)
- Goal: strengthen persistent runtime sessions with health probing and bounded retention to reduce stale session risk and resource leak risk.
- Scope:
  1) ping/pong heartbeat contract;
  2) keep-alive session healthcheck before reuse;
  3) bounded session cache with eviction.
- Status: done

## Changes
- Protocol extension (shared):
  - added `Ping { request_id }` and `Pong { request_id }` to `PluginWireMessage`.
  - file: `crates/xnote-core/src/plugin_protocol.rs`.
- Worker runtime updated:
  - responds to `Ping` with `Pong` after handshake.
  - file: `crates/xnote-plugin-worker/src/main.rs`.
- Host process runtime session hardening:
  - config additions:
    - `session_ping_timeout_ms`
    - `max_keep_alive_sessions`
  - session healthcheck:
    - `ping_transport(...)` probes reused session before activation.
    - unhealthy session is terminated and replaced.
  - session retention/eviction:
    - added `session_order` tracking.
    - `enforce_session_limit()` evicts oldest sessions when exceeding cap.
  - observability:
    - `active_session_count()` for integration assertions.
  - file: `crates/xnote-core/src/plugin.rs`.

## Tests
- protocol tests:
  - `ping_pong_roundtrip`
  - file: `crates/xnote-core/src/plugin_protocol.rs`.
- integration tests:
  - `process_runtime_worker_integration_session_limit_eviction`
  - existing keep-alive and negotiation tests retained and passing.
  - file: `crates/xnote-core/tests/plugin_runtime_integration.rs`.

## Verification (2026-02-06 16:26)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (41 unit + 6 integration)
- `cargo check -p xnote-plugin-worker` ✅
- `cargo check -p xnote-ui` ✅

## Next
- Stage 11 proposal:
  1) true request multiplexing over single session;
  2) runtime pool policy (idle timeout / priority eviction);
  3) health telemetry export for diagnostics panel.
