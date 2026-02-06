# Plugin Runtime Stage 7 Log

## Stage 7 (2026-02-06)
- Goal: Build IDE-grade plugin process protocol baseline (host/runtime handshake + IPC contract).
- Steps:
  1) Define host/runtime protocol envelope and message schema
  2) Add runtime handshake + capability negotiation path
  3) Add request-level timeout and cancellation contract
  4) Wire process runtime to protocol layer incrementally
  5) Add focused tests and update roadmap/log
- Status: partial
- Next: implement stdio transport abstraction and runtime-side worker bootstrap entry (split host/runtime test harness)

## Progress (2026-02-06 13:36)
- Updated log naming rule to `YYYY-MM-DD_内容.md` in `AGENTS.md`.
- Migrated previous infra log from `infra-bootstrap.md` to `2026-02-06_infra-bootstrap.md`.
- Stage 7 execution started.

## Progress (2026-02-06 13:42)
- Start implementing protocol envelope + handshake in `xnote-core::plugin`.
- Plan to wire process runtime activation through line-delimited JSON IPC with timeout/cancel handling.

## Progress (2026-02-06 14:02)
- Implemented line-delimited JSON IPC envelope in `xnote-core::plugin` (`PluginWireMessage`).
- Implemented process runtime handshake/ack path with protocol version verification.
- Implemented request-level activation message (`Activate`) and host cancellation message (`Cancel`).
- Added watchdog-assisted receive loop + cancel/timeout conversion to `RuntimeStatus::Cancelled`.
- Added protocol-focused process runtime tests (handshake success / runtime error / timeout / spawn failure).

## Verification (2026-02-06 14:03)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (29 passed)
- `cargo check -p xnote-ui` ✅

## Progress (2026-02-06 14:14)
- Start Stage 7 phase-2: replace script-based runtime test harness with standalone `plugin-worker` process.
- Target: real host/runtime process split with handshake + request-level activation/cancel compatibility.

## Progress (2026-02-06 14:33)
- Added standalone runtime crate `crates/xnote-plugin-worker` and included it in workspace.
- Updated process runtime default command path to `xnote-plugin-worker` (or `XNOTE_PLUGIN_WORKER_BIN` override).
- Migrated protocol tests from inline shell scripts to real worker process boot (`cargo run -p xnote-plugin-worker`).
- Added protocol-version mismatch test against worker handshake (`XNOTE_PLUGIN_WORKER_PROTOCOL_VERSION`).
- Kept host-side timeout/cancel state mapping stable while switching to external worker.

## Verification (2026-02-06 14:34)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (30 passed)
- `cargo check -p xnote-ui` ✅
- `cargo check -p xnote-plugin-worker` ✅

## Progress (2026-02-06 14:52)
- Extracted shared plugin IPC schema into `xnote-core::plugin_protocol`.
- Updated host (`xnote-core::plugin`) and worker (`xnote-plugin-worker`) to consume same wire message definitions.
- Added host-side capability scope gate during trigger activation (policy-aware runtime precheck).
- Added regression test for policy gating at trigger time (`trigger_event_blocks_disallowed_capability_by_policy`).

## Verification (2026-02-06 14:53)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (31 passed)
- `cargo check -p xnote-plugin-worker` ✅
- `cargo check -p xnote-ui` ✅

## Progress (2026-02-06 15:02)
- Start test hardening pass: add protocol module unit tests + cross-crate process runtime integration tests.
- Goal: ensure host/runtime boundary is verifiable outside `plugin.rs` internal unit tests.

## Progress (2026-02-06 15:18)
- Added worker-reported capability support in handshake ack (`reported_capabilities`).
- Implemented host handshake-time dual validation:
  1) worker-reported capabilities must be subset of manifest-declared capabilities;
  2) worker-reported capabilities must pass host policy scope gate.
- Added protocol unit tests in `plugin_protocol` for compatibility/roundtrip.
- Added integration tests in `crates/xnote-core/tests/plugin_runtime_integration.rs` using standalone worker process.
- Added capability mismatch tests in `plugin.rs` (undeclared capability + policy-blocked reported capability).

## Verification (2026-02-06 15:19)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (35 unit + 2 integration passed)
- `cargo check -p xnote-plugin-worker` ✅
- `cargo check -p xnote-ui` ✅

## Quality Review Snapshot (2026-02-06 15:20)
- Module boundary: improved (protocol schema extracted to `xnote-core::plugin_protocol`, host/worker shared).
- Decoupling: improved (worker standalone crate; integration tests cross process boundary).
- Test layers: improved (unit + integration both present for runtime contract).
- Remaining gaps to IDE-grade:
  1) Need typed transport abstraction (not only stdio line JSON in one file);
  2) Need structured error taxonomy instead of string-based errors;
  3) Need long-running runtime session + multiplexed requests (current model is activation-scoped);
  4) Need contract version negotiation strategy for rolling upgrades.

## Progress (2026-02-06 15:38)
- Completed transport abstraction milestone: `PluginTransport` trait + `StdioProcessTransport` implementation are now the default host/runtime IO boundary.
- Process runtime in `xnote-core::plugin` now delegates IPC send/receive/terminate to transport instead of directly owning stdio details.
- Cleaned compiler warnings in runtime foundation files:
  - `crates/xnote-core/src/plugin_transport.rs`: moved `VecDeque` import into test module and removed unused `Stdio` import.
  - `crates/xnote-core/src/plugin.rs`: removed unnecessary `mut` on spawned child binding.

## Verification (2026-02-06 15:39)
- `cargo fmt` ✅
- `cargo test -p xnote-core` ✅ (36 unit + 2 integration passed)
- `cargo check -p xnote-plugin-worker` ✅
- `cargo check -p xnote-ui` ✅

## Stage Status (2026-02-06 15:39)
- Stage 7 status: done (for current scope).
- Next suggested Stage 8 focus:
  1) typed runtime error taxonomy;
  2) persistent runtime session + request multiplexing;
  3) protocol version negotiation strategy.
