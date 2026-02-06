# Plugin Runtime Stage 8 Log (Error Taxonomy)

## Stage 8 (2026-02-06)
- Goal: move process runtime failure model from string-only errors to typed, IDE-grade error taxonomy.
- Scope:
  1) introduce structured runtime error code;
  2) keep host/runtime boundary behavior unchanged;
  3) add targeted unit/integration assertions for error code stability.
- Status: done

## Changes
- Added typed runtime error model in `xnote-core::plugin`:
  - `RuntimeErrorCode` (`InvalidConfig`, `SpawnFailed`, `TransportIo`, `HandshakeRejected`, `ProtocolMismatch`, `CapabilityViolation`, `ProtocolViolation`, `ActivationRejected`)
  - `RuntimeError { code, detail }`
  - `RuntimeStatus::Failed(String)` -> `RuntimeStatus::Failed(RuntimeError)`
- Added helper conversion path:
  - `runtime_failed(code, detail)` for consistent failure construction.
- Updated process runtime failure mapping:
  - empty command -> `InvalidConfig`
  - spawn failure -> `SpawnFailed`
  - send/receive/transport errors -> `TransportIo`
  - handshake rejected -> `HandshakeRejected`
  - protocol version mismatch -> `ProtocolMismatch`
  - undeclared/restricted capability reported by worker -> `CapabilityViolation`
  - unexpected wire message -> `ProtocolViolation`
  - activate result `ok=false` -> `ActivationRejected`
- Kept registry/user-facing message format compatible by storing `err.to_string()` into runtime state.

## Tests
- Updated plugin runtime unit tests to assert error code where meaningful:
  - activation rejected path -> `ActivationRejected`
  - protocol mismatch path -> `ProtocolMismatch`
  - spawn failure path -> `SpawnFailed`
  - undeclared capability path -> `CapabilityViolation`
- Added integration test:
  - `process_runtime_worker_integration_activation_rejected_code`

## Verification (2026-02-06 15:53)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (36 unit + 3 integration)
- `cargo check -p xnote-plugin-worker` ✅
- `cargo check -p xnote-ui` ✅

## Next
- Stage 9 proposal:
  1) persistent runtime session (avoid process-per-activation);
  2) request multiplexing and correlation IDs across command streams;
  3) protocol negotiation policy (min/max supported version window).
