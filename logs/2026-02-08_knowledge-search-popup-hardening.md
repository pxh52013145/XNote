# 2026-02-08 Knowledge Search/Popup Hardening

## Stage Goal
- Harden Knowledge UI search/palette/settings interaction to IDE-grade behavior.
- Fix popup click-through/instant-close issues and async search/filter stability.
- Improve UX parity for command palette branches (Commands/Quick Open/Search) and Explorer filter feedback.

## Implemented
1. Palette Search mode hardening
- Added full `PaletteMode::Search` i18n-based title/placeholder/group labels.
- Ensured search result list uses guarded async pipeline with nonce + index generation checks.
- Preserved Quick Open cache path and fixed Rust 2021-incompatible let-chain usage.

2. Async filter/search stability
- Fixed explorer filter async tuple destructuring (`query + tokens + paths_lower`).
- Enabled multi-token AND matching reliably for explorer filter.
- Added visible match count in filter hint (`Filter: <query> (<count>)`).

3. Overlay/modal event routing contract
- Applied modal occlusion at surface box level (`palette_box`, `prompt_box`, `settings modal`).
- Removed unnecessary occlusion from container wrappers to avoid routing ambiguity.
- Backdrop close with arm-delay remains intact; inside-surface clicks no longer route to backdrop.

4. Command routing consistency
- `FocusSearch` now opens Search palette when already in palette context; otherwise keeps panel-focus behavior.
- Keeps global panel routing while improving top search branch discoverability.

5. UI text cleanup
- Repaired malformed separator and prompt glyphs in key interaction strings.

## Validation
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo test -p xnote-ui --no-run` ✅

## Current Status
- Popup/search/filter interaction path is stable and compile-clean.
- Knowledge UI behavior aligns better with VSCode/Zed-like modal and command surface expectations.

## Next
- If needed, add lightweight telemetry for popup close reason (Esc/backdrop/explicit) and search mode open source (hotkey/titlebar/command), for UX tuning.
