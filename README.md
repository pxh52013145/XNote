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

