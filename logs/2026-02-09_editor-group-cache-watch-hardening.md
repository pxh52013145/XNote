# 2026-02-09 Editor Group + Cache + Watch Hardening

## Stage Goal
- Deliver a one-pass completion and optimization round for Knowledge editor responsiveness and split-group operability.
- Keep GPUI interaction contracts stable while reducing perceived latency on note open/switch.

## Implementation Scope

### 1) Cache-first note opening path
- Introduced cache-assisted open path in `open_note`:
  - Try in-memory note cache first for immediate render.
  - Keep async disk read as source-of-truth refresh.
  - If disk read fails but cache is already shown, preserve visible content and surface non-destructive status.
- Added helper `apply_loaded_note_content` to unify:
  - editor buffer hydration,
  - word-count recompute,
  - markdown invalidation + parse scheduling,
  - pending line-jump cursor restoration.

### 2) Note cache consistency hardening
- On successful save (`schedule_save_note`), write-back into note cache.
- On watch move/rename replay, move cache key via `move_note_content_cache_path`.
- On watch remove and content-upsert replay, evict stale cache entries via `evict_note_content_cache_path`.
- For active note external updates:
  - auto-refresh open editor when current note is not dirty,
  - avoid destructive overwrite while local unsaved edits exist.

### 3) Multi-group focus operability
- Added group-focus keyboard navigation in editor key handling:
  - `Alt+Left` -> focus previous editor group.
  - `Alt+Right` -> focus next editor group.
- Reused existing deterministic group focus helpers and active-note binding flow.

### 4) Safety + maintainability
- Reduced duplicated content-apply logic by centralizing it in one method.
- Kept nonce-guarded async update contract unchanged.
- Preserved modal/overlay and render safety behavior from prior hardening passes.

## Tests / Validation
- Added unit test:
  - `touch_cache_order_moves_recent_and_evicts_oldest`
- Full checks executed:
  - `cargo fmt --all`
  - `cargo check -p xnote-ui`
  - `cargo test -p xnote-core`
  - `cargo test -p xnote-ui`
- All passed in this milestone.

## Current Status
- Knowledge editor open/switch path is now faster and more resilient under file-system churn.
- Split editor groups have deterministic keyboard focus traversal.
- Cache/watch/save flows are aligned for lower stale-state risk.

## Next Suggested Optimizations
- Add bounded background warm-cache for recently active tabs.
- Add group-aware tab move/copy semantics (VSCode-style editor group actions).
- Add targeted active-note external-diff indicator before auto-refresh.

