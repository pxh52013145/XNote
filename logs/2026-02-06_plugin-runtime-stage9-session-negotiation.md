# Plugin Runtime Stage 9 Log (Session + Negotiation)

## Stage 9 (2026-02-06)
- Goal: evolve plugin runtime boundary from per-activation process to IDE-grade session baseline, while adding protocol negotiation compatibility.
- Scope:
  1) protocol negotiation contract (`supported_protocol_versions`);
  2) keep-alive runtime session option;
  3) keep tests and compatibility stable.
- Status: done

## Changes
- Protocol contract upgraded (shared schema):
  - `PluginWireMessage::Handshake` now carries `supported_protocol_versions` (with `serde(default)` for backward compatibility).
  - File: `crates/xnote-core/src/plugin_protocol.rs`.
- Runtime config capability upgraded:
  - `ProcessRuntimeConfig` now includes:
    - `supported_protocol_versions: Vec<u32>`
    - `keep_alive_session: bool`
  - Added normalized and negotiation helpers:
    - `normalized_supported_protocol_versions()`
    - `negotiate_protocol(runtime_version)`
  - File: `crates/xnote-core/src/plugin.rs`.
- Worker updated for negotiation-aware handshake reply:
  - reads host-supported versions from handshake;
  - chooses compatible version (or falls back to host proposed version);
  - still supports env override for deterministic tests.
  - File: `crates/xnote-plugin-worker/src/main.rs`.
- Process runtime refactor:
  - extracted activation flow into reusable units:
    - `spawn_transport`
    - `perform_handshake`
    - `request_activation`
  - introduced keep-alive session map:
    - `sessions: HashMap<String, Box<dyn PluginTransport>>`
    - keyed by plugin id/version/capability signature.
  - retains session on `Ready` and `ActivationRejected`; resets on timeout/cancel/protocol/transport failures.
  - `Drop` now terminates all remaining transports for deterministic cleanup.
  - File: `crates/xnote-core/src/plugin.rs`.

## Tests
- Added protocol module tests:
  - handshake roundtrip with supported versions;
  - legacy handshake compatibility without supported versions.
  - File: `crates/xnote-core/src/plugin_protocol.rs`.
- Added core unit tests:
  - `process_runtime_config_negotiates_protocol_from_supported_set`
  - `process_runtime_accepts_legacy_version_with_supported_set`
  - File: `crates/xnote-core/src/plugin.rs`.
- Added integration tests:
  - `process_runtime_worker_integration_keep_alive_session_reuse`
  - `process_runtime_worker_integration_protocol_negotiation_accepts_legacy_runtime`
  - File: `crates/xnote-core/tests/plugin_runtime_integration.rs`.

## Verification (2026-02-06 16:12)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (40 unit + 5 integration)
- `cargo check -p xnote-plugin-worker` ✅
- `cargo check -p xnote-ui` ✅

## Next
- Stage 10 proposal:
  1) request multiplexing over persistent session;
  2) host-side runtime pool and eviction policy;
  3) heartbeat/health probe and automatic recovery strategy.
