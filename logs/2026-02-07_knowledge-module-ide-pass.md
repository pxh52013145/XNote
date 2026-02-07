# 2026-02-07 Knowledge Module IDE Pass

## Stage Goal
- Complete a consolidated **M-KNOWLEDGE-HARDEN** delivery for the Knowledge module baseline.
- Ensure core capabilities are stable, testable, and aligned with IDE-grade expectations: robust ranking, theme extensibility, settings usability, and performance/quality gates.

## Scope Summary

### 1) Quick Open Ranking Hardening (`xnote-core`)
- Upgraded quick-open matching pipeline in `crates/xnote-core/src/knowledge.rs`:
  - Added robust fallback candidate strategy when token inverted lookup misses.
  - Added fuzzy subsequence matching (`subsequence_score`) for path/title/stem.
  - Strengthened filename stem weighting (exact/prefix/contains boosts).
  - Added word-boundary aware scoring helper (`is_word_boundary_at`).
  - Added path-length tie-breaker after score sorting to stabilize ranking.

### 2) Deterministic Ranking Tests (`xnote-core`)
- Added and validated ranking regression tests:
  - `quick_open_prefers_filename_stem_over_deeper_path`
  - `quick_open_supports_subsequence_matching`
  - `quick_open_tiebreak_prefers_shorter_path`
- Objective: prevent score regressions and keep quick-open behavior deterministic over future iterations.

### 3) Theme Tokenization Hardening (`xnote-ui`)
- Extended `UiTheme` in `crates/xnote-ui/src/main.rs` to support richer semantic tokens:
  - `surface_bg`, `surface_alt_bg`, `titlebar_bg`
  - `text_secondary`, `text_subtle`
  - `accent_soft`
- Wired tokens through major high-frequency surfaces:
  - Explorer/Search/Workspace/Editor shells
  - Tabs/titlebar/status shell and splitter visuals
  - Settings modal shell and key controls
  - Palette and vault prompt major shells/inputs
- Fixed Rust 2024 `'static` closure capture issues caused by `cx.processor(...)` by converting affected closures to `move` closures.

### 4) Stability Fix Included
- Fixed bookmark panel compile bug in `xnote-ui`:
  - Avoided move of `bookmark_paths` in `for` loop by iterating with reference.

## Validation & Gates

### Executed Checks
- `cargo fmt`
- `cargo test -p xnote-core`
- `cargo check -p xnote-ui`
- `cargo check -p xtask`
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 5`

### Result
- All above checks passed.
- `foundation-gate` passed including perf baseline checks and delta report generation.

## Milestones Achieved
- Quick Open now has stronger fuzzy behavior and deterministic tie-breaking.
- Theme baseline is substantially more semantic-token driven across core UI surfaces.
- Settings/knowledge interactions remain stable under full gate execution.

## Remaining Follow-up (Next Consolidated Wave)
- Continue tokenization in remaining low-frequency or isolated hardcoded color spots to fully remove palette drift.
- Add UI-level integration tests for critical settings-to-runtime behaviors (theme switch, hotkey edit/reset, references/bookmarks interactions).
- Optionally add recency/session-based ranking features in quick-open once usage telemetry contracts are finalized.

## Current Status
- Stage status: **completed (hardening wave)**
- Branch/worktree status: validation green; ready for next large-scope module pass.

## 2026-02-07 Stability Incident Follow-up (Settings / Module Switch Crash)

### Symptom
- User action: click **Settings** (rail button) or click left-bottom module switch (`status.module`).
- Runtime error: process exited with `0xc000041d` (`STATUS_FATAL_USER_CALLBACK_EXCEPTION`).

### Root Cause Analysis
- Crash dumps from `%LOCALAPPDATA%/CrashDumps` were analyzed with `minidump-stackwalk`.
- Crashing stack consistently pointed to `_chkstk` while rendering overlay trees:
  - `XnoteWindow::settings_overlay` (`crates/xnote-ui/src/main.rs:2330`)
  - `XnoteWindow::palette_overlay` (`crates/xnote-ui/src/main.rs:1843`)
- This indicates a **stack overflow risk in deep/large UI render construction** during overlay render, not a plugin/runtime boundary failure.

### Fix Applied
- Added a Windows-specific linker stack reserve for `xnote-ui` executable:
  - `crates/xnote-ui/Cargo.toml`: add `build = "build.rs"`
  - `crates/xnote-ui/build.rs`: emit `cargo:rustc-link-arg=/STACK:16777216`
- Effect: increases main thread stack headroom for large GPUI overlay render paths, preventing callback-fatal `_chkstk` overflow under settings/palette open actions.

### Verification
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 5` ✅
- Process smoke run (`target/debug/xnote-ui.exe`) starts and stays alive for timed window ✅

### Next Hardening Suggestion
- Keep this stack-reserve fix as immediate stability guard.
- In next cleanup wave, gradually split very large overlay builders into smaller reusable UI units with `AnyElement` boundaries to reduce per-frame stack pressure further.

## 2026-02-07 Modal Interaction Fix (Settings/Palette/Vault Prompt)

### Issue
- Settings modal click could "fall through" to underlying UI.
- In some paths, opening click and backdrop click were effectively chained, so modal closed immediately.

### Fix
- Added explicit overlay click-blocking for all modal layers (`occlude`) and modal container layers.
- Added lightweight backdrop-arm delay (`120ms`) to avoid the opening click being consumed as a close click.
- Applied the same mechanism consistently to:
  - Settings overlay
  - Command palette overlay
  - Open-vault prompt overlay
- Normalized close paths (`Esc`/close button/programmatic close) to clear arm state, avoiding stale modal state.

### Validation
- `cargo check -p xnote-ui` ✅
- `cargo run -p xnote-ui` launches and remains running (manual close by process stop in automation) ✅

## 2026-02-07 GPUI Engineering Skill Added

### Goal
- Capture GPUI-specific modern UI guardrails into a reusable skill to avoid repeated interaction/layout/render pitfalls in future screens.

### Deliverables
- New Codex skill:
  - `C:/Users/31625/.codex/skills/gpui-ide-ui-standards/SKILL.md`
  - `C:/Users/31625/.codex/skills/gpui-ide-ui-standards/agents/openai.yaml`
  - `C:/Users/31625/.codex/skills/gpui-ide-ui-standards/references/pitfall-catalog.md`
  - `C:/Users/31625/.codex/skills/gpui-ide-ui-standards/references/review-checklist.md`

### What It Covers
- Overlay/modal event routing contract (no click-through, no open-close race).
- Layout invariants (fixed shells, internal scroll, no section-switch jitter).
- Focus and keyboard priority routing (`Esc` and top-most surface ownership).
- Async/state safety (nonce checks and close-path cleanup).
- Render safety and stack-pressure awareness for large GPUI builders.

### Validation
- Skill lint/structure validated by `quick_validate.py` ✅

## 2026-02-07 Native Editor Foundation (M1/M2 Baseline)

### Stage Goal
- Start native Knowledge editor engine foundation in `xnote-core` and wire minimal UI integration in `xnote-ui`.
- Produce a formal RFC and phased milestones for full editor evolution.

### Implemented
- Core module added:
  - `crates/xnote-core/src/editor.rs`
  - exports from `crates/xnote-core/src/lib.rs`
  - dependency `ropey` added in `crates/xnote-core/Cargo.toml`
- Core capabilities:
  - `EditorBuffer` (rope-backed text buffer)
  - `EditTransaction` (insert/delete/replace)
  - `undo/redo` stacks with inverse records
  - `version` and `EditorStats`
  - UTF-8 boundary validation for byte ranges
- UI baseline integration (`crates/xnote-ui/src/main.rs`):
  - initialize core `EditorBuffer` on note open
  - route major replace paths through transaction `apply` first
  - fallback to legacy splice path if transaction apply fails
  - keep buffer/content synchronized on save and load transitions
  - add keyboard undo/redo (`Ctrl+Z`, `Ctrl+Y`) baseline behavior

### RFC Delivered
- Added native editor decision and roadmap:
  - `design/knowledge/Editor-RFC.md`
- Includes:
  - architecture boundary
  - goals/non-goals
  - risk mitigation
  - acceptance criteria
  - milestones M1..M6

### Milestone Status
- M1 (Engine Foundation): done
- M2 (UI Integration Baseline): in progress (baseline done, mutation-path unification pending)

### Validation
- `cargo test -p xnote-core` ✅
- `cargo check -p xnote-ui` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 5` ✅

## 2026-02-07 Native Editor M3/M4 Execution (Mutation Unification + Markdown Pipeline)

### Stage Goal
- Execute M3 + M4 in one pass:
  - M3: converge mutation paths onto `xnote-core::editor::EditorBuffer` transactions.
  - M4: introduce markdown parser abstraction and wire asynchronous parse feedback into UI.

### Implemented
- Core markdown module introduced:
  - `crates/xnote-core/src/markdown.rs`
  - API: `parse_markdown(&str) -> MarkdownParseResult`
  - Summary outputs: heading list, links, code fence count, block count.
  - Uses `pulldown-cmark` with commonly needed extensions enabled.
  - Added unit tests covering heading/link/code-fence extraction.
- Core exports/deps updated:
  - `crates/xnote-core/src/lib.rs`: `pub mod markdown;`
  - `crates/xnote-core/Cargo.toml`: add `pulldown-cmark`.
- UI mutation-path convergence completed in `crates/xnote-ui/src/main.rs`:
  - Keyboard edit path uses `EditTransaction::replace` via `EditorBuffer`.
  - IME paths (`replace_text_in_range`, `replace_and_mark_text_in_range`) use the same core transaction path.
  - Undo/Redo integrated into command bus (`CommandId::Undo`, `CommandId::Redo`) and dispatched through existing command execution path.
  - Plugin activation events now include Undo/Redo command triggers, keeping lifecycle hooks consistent.
- Markdown parse pipeline wired with stability guards:
  - Debounced parse scheduling (`MARKDOWN_PARSE_DEBOUNCE`).
  - Nonce guard (`next_markdown_parse_nonce` / `pending_markdown_parse_nonce`) prevents stale async result overwrite.
  - Parse runs in background executor and updates lightweight counters only.
  - UI header chips now expose compact markdown stats (`H`, `L`, `Code`) without full preview overhead.

### Stability/Perf Considerations
- Parse is guarded by note-open/loading state to avoid unnecessary work.
- Debounce + nonce model prevents parse storms and race-condition flicker on rapid edits.
- Current stage intentionally keeps parsing summary-oriented to avoid large render costs while preserving future extensibility.

### Validation (Post-M3/M4)
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 5` ✅

### Milestone Snapshot
- M1: completed
- M2: completed (baseline integration)
- M3: completed (mutation unification)
- M4: completed (parser abstraction + async integration)
- M5/M6: pending (syntax pipeline, viewport/render optimization, higher-level editor parity)

## 2026-02-07 Native Editor M5/M6 Execution (Preview/Split + Performance Hardening)

### Stage Goal
- Complete RFC M5/M6 in one consolidated delivery:
  - M5: deliver parser-backed Preview/Split editor surface.
  - M6: add performance hardening primitives (incremental invalidation windows + edit latency telemetry + tests).

### Implemented (M5)
- Editor shell mode system added in `xnote-ui`:
  - `EditorViewMode::{Edit, Preview, Split}` state.
  - Header mode toggles (`Edit` / `Preview` / `Split`).
  - `ToggleSplit` command now maps to mode transitions via a single helper.
- Preview pane is parser-backed (not raw text heuristics):
  - Uses markdown parse output blocks/headings.
  - Includes outline (heading tree) and body block rendering (heading/paragraph/code/quote/list styles).
  - Split mode now renders left editor + right preview, while preview mode renders preview-only.

### Implemented (M6)
- Markdown incremental invalidation contract in `xnote-core`:
  - `MarkdownInvalidationWindow` with `from_edit`, `merge`, `len`, `as_range`.
  - `parse_markdown_window(text, window)` for bounded parse windows.
  - `parse_markdown_summary(text)` for low-cost whole-document counters.
- UI parse pipeline upgraded:
  - Keeps pending invalidation window and merges multiple edit windows before parse run.
  - Debounce + nonce guard retained.
  - Parse pass now runs summary (full) + body parse (window/full) in background executor.
- Mutation path hardening:
  - Unified keyboard/IME edits via `apply_editor_transaction` helper.
  - Unified undo/redo via `apply_editor_history` helper.
  - Removed duplicate local mutation logic and centralized post-edit side effects (dirty/status/autosave/parse).
- Latency profiling hooks:
  - Added in-memory edit latency rolling window (`EditLatencyStats`).
  - Status bar shows `Edit p50/p95` and sample count for interactive hardening feedback.

### Core Correctness Fixes + Tests
- Fixed `xnote-core::editor` undo/redo record direction for variable-length edits.
- Added tests:
  - variable-length edit undo/redo regression.
  - invalidation window clamp/merge.
  - window parse offset mapping.
  - summary parse consistency versus full parse.

### Validation (Post-M5/M6)
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 5` ✅

### Milestone Snapshot (Updated)
- M1: completed
- M2: completed
- M3: completed
- M4: completed
- M5: completed
- M6: completed

## 2026-02-07 Native Editor M7 Execution (Diagnostics/Gutter/Highlight + Extension Hooks)

### Stage Goal
- Add IDE-like editor enhancement layer on top of M1-M6:
  - parser-driven diagnostics,
  - gutter marker fundamentals,
  - bounded token highlighting,
  - extension-ready diagnostics provider contract.

### Implemented
- Core (`xnote-core::markdown`) additions:
  - `MarkdownDiagnosticSeverity` + `MarkdownDiagnostic`.
  - `MarkdownDiagnosticsProvider` trait as extension hook boundary.
  - `lint_markdown(text)` built-in lint rules:
    - heading jump (`H1 -> H3`) warning,
    - multiple H1 warning,
    - unclosed code fence error,
    - long line info.
  - `lint_markdown_with_providers(text, providers)` for host + external provider merge.
- UI (`xnote-ui`) integration:
  - parse pipeline now also emits diagnostics from `lint_markdown`.
  - added diagnostics panel in editor shell (fixed-height, virtualized list).
  - added editor gutter region with line numbers and red markers on diagnostic lines.
  - added bounded markdown token highlight spans (heading/code/quote/list/link) for editor body.
  - highlight generation automatically disables above configured byte threshold for stability.
- Hardening and quality:
  - fixed core undo/redo variable-length regression (kept with test).
  - ensured async parse/diagnostics updates remain nonce-guarded to avoid stale UI overwrite.

### Tests Added/Updated
- Core markdown tests:
  - lint rule detection (heading jump + unclosed fence).
  - provider merge behavior.
- UI tests:
  - highlight span detection for heading/link.
  - highlight disable path for very large document.

### Validation (Post-M7)
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo test -p xnote-ui --no-run` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 5` ✅

### Milestone Snapshot (Final)
- M1: completed
- M2: completed
- M3: completed
- M4: completed
- M5: completed
- M6: completed
- M7: completed

## 2026-02-07 Post-Review Global Optimization Pass (M1-M7 Unified)

### Stage Goal
- Execute a one-shot optimization and verification pass across all M1-M7 outputs.
- Eliminate known correctness gaps (preview model, wrapped-line diagnostics), remove quality blockers (clippy), and tighten tokenized theming consistency.

### Implementation Steps
- Correctness hardening:
  - switched markdown parse scheduling to full-document parse for preview model consistency under continuous edits,
  - removed stale window-only preview replacement path that could truncate preview blocks,
  - added wrapped-line to logical-line mapping for gutter numbering + diagnostics markers.
- UI tokenization completion:
  - wired semantic theme tokens into syntax highlight colors,
  - wired gutter background/text and diagnostics severity colors to theme tokens,
  - wired status sync dot colors (`Loading/Unsaved/Synced`) to semantic status tokens.
- Core + UI code quality cleanup:
  - command parsing now uses `FromStr` + `CommandId::parse`,
  - editor buffer switched from inherent `to_string` to `Display` implementation,
  - added `MarkdownInvalidationWindow::is_empty`,
  - reduced plugin activation helper argument count by deriving watchdog tick from runtime config,
  - fixed test/style clippy violations in settings and vault,
  - resolved UI clippy deny/warn set (`never_loop`, `map_or`, `single_match`, needless borrows, redundant closure).

### Validation
- `cargo clippy -p xnote-core --all-targets -- -D warnings` ✅
- `cargo clippy -p xnote-ui --all-targets` ✅
- `cargo test -p xnote-core` ✅
- `cargo test -p xnote-ui` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 5` ✅

### Current Status
- Unified optimization pass: **completed**
- Regressions observed during this pass: **none**

### Next Step
- Start M8+ layer on top of this stable baseline: editor viewport virtualization, richer markdown semantic tokens, and plugin-provided diagnostics pipeline (provider registry + UI source attribution).

## 2026-02-07 Explorer New Folder Reliability Fix (UI + Core)

### Stage Goal
- Fix user-visible issue where Explorer `New Folder` appears to fail, and ensure behavior is IDE-like and predictable.

### Root Cause
- Explorer tree index was built only from markdown note paths, so **empty folders were not represented** in tree structures.
- New folder creation succeeded on disk, but if the folder had no markdown file yet, Explorer did not show it after rescan, causing “new folder not working” perception.
- New item target folder was primarily derived from selected note, not explicit folder-context selection in Explorer rows.

### Implementation
- Core (`xnote-core`):
  - Added `VaultScan { notes, folders }` model.
  - Added `Vault::fast_scan_notes_and_folders()` to collect both markdown files and directory paths (excluding `.xnote/`).
  - Kept `fast_scan_notes()` backward-compatible by delegating to `fast_scan_notes_and_folders().notes`.
- UI (`xnote-ui`):
  - Explorer index build now consumes `VaultScan` and incorporates scanned folder paths into `folder_children`/`folder_notes` tree construction.
  - Added explicit Explorer folder selection state (`selected_explorer_folder`) so new-item placement prefers current folder context.
  - Added `resolve_base_folder_for_new_items(...)` helper for deterministic target resolution:
    - selected folder context first,
    - selected note’s parent folder second,
    - vault root fallback.
  - On folder row/vault row click, update selected Explorer folder context.
  - On create-folder success:
    - expand new folder ancestor chain,
    - select new folder context,
    - clear filter when current filter would hide the newly created folder,
    - then rescan.

### Tests Added
- `xnote-core`:
  - `fast_scan_notes_and_folders_includes_empty_folders`
  - `fast_scan_notes_still_returns_only_notes`
- `xnote-ui`:
  - `resolve_base_folder_prefers_selected_folder_context`
  - `resolve_base_folder_falls_back_to_selected_note_folder`
  - `resolve_base_folder_uses_root_when_no_context`

### Validation
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo test -p xnote-ui` ✅

### Outcome
- Explorer `New Folder` is now reliable and visible immediately after creation, including empty folders.
- New-item placement behavior is now aligned with IDE user expectation (current folder context first).

## 2026-02-07 Create/Open Latency Perception Hardening (Optimistic UI + Two-Phase Index)

### Stage Goal
- Remove strong perceived lag when creating notes/folders and opening vaults.
- Keep correctness via delayed reconciliation while making Explorer instantly responsive.

### Implemented
- Two-phase vault open/rescan in `xnote-ui`:
  - phase 1: fast scan + Explorer tree ready immediately,
  - phase 2: knowledge index builds in background async task.
- Added `rebuild_knowledge_index_async(entries, cx)` to decouple heavy index build from initial Explorer visibility.
- Create-note/create-folder now optimistic:
  - immediately patch in-memory tree/list state (`add_note_optimistically`, `add_folder_optimistically`),
  - update selection/ancestor expansion instantly,
  - schedule delayed reconciliation scan via `schedule_reconcile_after_create`.
- Added delayed reconcile timer (`CREATE_RECONCILE_DELAY`) to keep eventual consistency while avoiding synchronous UX stall.

### UX Outcome
- New file/folder appears in Explorer right away (no longer waiting for full rescan/index build).
- Opening vault becomes “Explorer-first”; search/quick-open capability catches up as background index completes.

### Validation
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 1` ✅

## 2026-02-07 Watch Pipeline Full-Coverage Upgrade (File + Folder Events)

### Stage Goal
- Complete event-driven incremental pipeline so folder operations are first-class (create/remove/move), reducing fallback-to-rescan cases.

### Implemented
- Core watch model (`xnote-core::watch`) extended with folder events:
  - `FolderCreated { path }`
  - `FolderRemoved { path }`
  - `FolderMoved { from, to }`
- Event extraction now classifies both note and folder paths:
  - create/remove folder kinds mapped directly,
  - rename handling supports note rename and folder rename variants.
- Dedup pipeline upgraded to include folder create/remove/move collapse semantics with move-chain collapse support.
- UI watch apply path (`xnote-ui`) upgraded:
  - handles folder create via `ensure_folder_branch` + ancestor expansion,
  - handles folder remove via subtree note/path cleanup + tree state cleanup,
  - handles folder move by path remap and tree state rename updates,
  - merges folder-derived note deltas back into the existing note incremental pipeline for consistent index/bookmark/open-editor updates.
- `xtask` watch benchmark adapter updated to handle new folder events (fallback rebuild in benchmark mode).

### Tests Added
- `xnote-core/src/watch.rs`:
  - `dedup_keeps_folder_events`
  - `dedup_collapses_folder_move_chain`

### Outcome
- Incremental watch processing now covers common folder-level external edits without immediate full rescan.
- Create/move/remove burst handling remains coalesced and incremental-first.

### Validation
- `cargo check -p xnote-core` ✅
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo test -p xnote-ui` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 1` ✅

## 2026-02-07 Incremental Reconcile v2 (No Full Rescan on Create Path)

### Stage Goal
- Move create-path reconcile from full-vault rescan to targeted incremental reconcile.
- Keep eventual consistency with lower IO/index churn.

### Implemented
- Added dedicated pending sets for create reconciliation:
  - `pending_created_note_reconcile`
  - `pending_created_folder_reconcile`
- `add_note_optimistically` / `add_folder_optimistically` now register pending reconcile targets.
- Replaced create-path full rescan with `reconcile_pending_creates`:
  - validates pending note/folder existence against filesystem,
  - patches tree/index structures incrementally (upsert/remove only affected paths),
  - refreshes derived caches/fingerprint and reruns active filter/search/quick-open projections.
- Added index-readiness queue for watcher events:
  - `pending_watch_changes_until_index_ready` buffers watch deltas while index is `None` during two-phase load,
  - queue flushes immediately after background index build success.

### Outcome
- Create burst path now avoids expensive full-vault rescan in normal cases.
- Reconcile workload scales with changed entities, not vault size.
- Better stability during open/rescan phase by preventing watch-event drop while index is warming.

### Validation
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 1` ✅

### Perf Snapshot (Current)
- `scan_ms` ~ 81
- `knowledge_index_build_ms` ~ 538
- baseline gate: OK

## 2026-02-07 Reconcile Debounce & Coalescing (Create Burst Hardening)

### Stage Goal
- Ensure repeated create operations do not trigger repeated full rescans.
- Keep eventual consistency while reducing unnecessary background churn.

### Implemented
- Added create-reconcile nonce pair:
  - `next_create_reconcile_nonce`
  - `pending_create_reconcile_nonce`
- `schedule_reconcile_after_create` now behaves as coalescing debounce:
  - every new create bumps nonce and supersedes older pending reconcile tasks,
  - delayed task checks nonce validity before running,
  - if app is currently scanning/index-building, reconcile reschedules itself instead of stacking parallel rescans,
  - only the latest pending nonce is allowed to trigger one final `rescan_vault`.

### Outcome
- Burst create operations now converge to one effective reconcile scan.
- Reduced redundant full-tree/index refresh pressure under heavy interactive creation.

### Validation
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo run -p xtask -- foundation-gate --path Knowledge.vault --query note --iterations 1` ✅
