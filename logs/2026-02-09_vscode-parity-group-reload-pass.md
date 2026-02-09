# 2026-02-09 VSCode Parity Group + Reload Pass

## Stage Goal
- Continue one-pass IDE-parity improvements for Knowledge UI with focus on:
  - richer multi-editor-group operations,
  - safe external-change handling while local edits are dirty,
  - status-bar clarity for pending reload conditions.

## Implemented Enhancements

### 1) Editor-group operations (VSCode-like ergonomic layer)
- Added `split_active_group_to_new_note`:
  - split current editor group, then create/open a new note in that new group.
  - keyboard mapping: `Ctrl+Shift+\`.
- Added `close_other_editor_groups`:
  - keep active group only, close all others.
- Added `close_groups_to_right`:
  - close only groups to the right of active group.
- Added toolbar actions in editor tabs bar:
  - Close Other Groups.
  - Close Groups to Right.
  - both include tooltips and keep visual style consistent with existing controls.

### 2) External-change safe reload flow
- Added state flag:
  - `pending_external_note_reload: Option<String>`.
- Watch replay behavior for active note changed:
  - if current note is dirty: mark pending external reload (non-destructive), do not force overwrite.
  - if current note is clean: auto-reload immediately.
- Save path integration:
  - after successful save, if pending external reload exists for current note, auto-reopen from disk (`reopen_external_current_note`).

### 3) Cache + watch consistency extensions
- Pending reload path now follows rename/move events.
- Pending reload path is cleared when note is removed.
- Existing cache coherence retained:
  - move/rename updates cache key,
  - remove/upsert evicts stale cache.

### 4) Status visibility
- Status dot/text now includes a dedicated state:
  - `External Pending` when external file change is detected but deferred due to dirty editor.

## Validation
- `cargo fmt --all`
- `cargo check -p xnote-ui`
- `cargo test -p xnote-core`
- `cargo test -p xnote-ui`
- All passed.

## Added Tests
- `close_groups_to_right_keeps_left_prefix_and_active`
- `move_note_content_cache_path_transfers_entry`

## Current Outcome
- Multi-group workflow is closer to VSCode editing ergonomics.
- External file changes no longer risk silent destructive overwrite for dirty buffers.
- User can observe and resolve pending external-sync state deterministically.

