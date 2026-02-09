# XNote (Rust + GPUI)

Local-first note workstation (Knowledge-first) targeting **Zed-like performance** with **Rust Core + GPUI UI**.

## Repo layout

- `design/` — blueprint docs (source of truth for product/architecture)
- `docs/workstation_feature_map.csv` — execution checklist (P0/P1, status tracked)
- `Xnote.pen` — UI mock canvas
- `crates/xnote-core/` — vault/index/order core (Rust)
- `crates/xnote-ui/` — GPUI app (Rust)
- `archive/electron/app/` — archived Electron + React + TypeScript implementation (reference)

## Dev

```powershell
cargo test -p xnote-core
cargo run -p xnote-ui
```

Archived Electron app:

```powershell
cd archive/electron/app
npm install
npm run dev
```

## Foundation baseline gate

Run once for a full IDE-baseline health check:

```powershell
cargo run -p xtask -- foundation-gate --path .\Knowledge.vault --query note --iterations 10
```

or:

```powershell
pwsh scripts/foundation_gate.ps1
```

This gate covers:
- `xnote-core` tests
- `xnote-ui` compile checks (including test target build)
- `xtask` compile check
- perf baseline checks (`default` + `windows_ci`) and delta reports (median of 3 runs for stability)

`xtask perf` also includes a filesystem watch transaction replay stress section for **1k+ directory batch changes** (`watch_txn_*` metrics), and now uses prefix-incremental folder replay in benchmark apply path to reduce full-rebuild pressure.
