# 2026-02-09 Split Tab DnD + MRU Polish

## Stage Goal
- Complete the remaining split-view interaction gaps in one pass, focusing on IDE-grade tab drag/drop semantics, predictable MRU hotkeys, and state consistency under folder move/remove watch events.

## Implementation Steps
1. Audited existing split/group/tab flow and identified unfinished links between drag-over metadata and drop execution.
2. Hardened tab drag-over state updates to avoid unnecessary notify/render churn.
3. Unified tab drop path through `handle_tab_drop` so same-group drop reorders tabs and cross-group drop moves note ownership.
4. Added tab-level drag-move tracking (`on_drag_move::<DraggedEditorTab>`) and visual insertion indicator (left/right accent strip).
5. Added tab drag-over clear points on tab row mouse-up and tab mouse-up for predictable teardown.
6. Normalized key behavior:
   - `Ctrl+Tab` => swap note MRU inside active group.
   - `Ctrl+Shift+Tab` => focus last active editor group (group MRU).
7. Finalized folder-level watcher consistency:
   - Clear `pending_external_note_reload` when a removed folder deletes current pending path.
   - Rewrite `pending_external_note_reload` when folder-move rewrites note paths.
8. Re-validated with format/check/tests.

## Current Status
- Completed and validated.

## Verification
- `cargo fmt --all`
- `cargo check -p xnote-ui`
- `cargo test -p xnote-ui`
- `cargo test -p xnote-core`

All commands passed.

## UX/Contract Outcomes
- Split groups remain simultaneously readable (existing behavior retained).
- Tab DnD now has stable “reorder vs move-group” semantics with insertion-side feedback.
- MRU shortcuts no longer mix two actions in one chord, reducing mental/behavioral ambiguity.
- Watch-folder path rewrites/removals no longer leave stale deferred external-reload markers.

## Next Step
- Continue with advanced split ergonomics requested by user (drag tab onto explicit target groups, move editor between adjacent groups via command palette entries, pin-aware group-local tab ordering policy, and richer group-level MRU visualization) while preserving no-jitter rendering and overlay interaction contracts.
