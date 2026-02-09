# 2026-02-09 Split Notes UX Parity Pack

## Stage Goal
- Align Knowledge split/editor interactions with IDE-grade expectations while preserving note-taking ergonomics.
- Deliver in one pass: non-focused split visibility + tab/group workflows + safer multi-group operations.

## Implemented in this pass

### 1) Split panes now all show content
- Changed split rendering contract:
  - non-active groups no longer show placeholder-only text,
  - every group renders visible note content,
  - only active group remains interactive/editable.
- Added bounded preview shaping for inactive groups:
  - `compact_preview_from_content`
  - line/char caps to avoid heavy rendering and jitter.

### 2) Group operations improved (IDE parity)
- Added editor-group utilities:
  - `close_other_editor_groups`
  - `close_groups_to_right`
  - `split_active_group_to_new_note`
  - `move_current_editor_to_next_group`
- Added focus behavior:
  - `focus_last_editor_group`
  - group MRU tracking (`editor_group_mru`) for deterministic switching.

### 3) Tab workflows upgraded
- Added pinning system for open tabs:
  - `pinned_editors` state,
  - `toggle_pin_editor` action,
  - pinned-first ordering (`reorder_open_editors_with_pins`).
- Added tab drag payload support for editor tabs:
  - `DraggedEditorTab`
  - drag tab to group drop targets in tabs row.
- Added move-by-shortcut flows:
  - `Ctrl+Alt+Right`: move current editor to next group.
  - `Ctrl+Tab`: MRU-like group focus + per-group note history swap.
  - `Ctrl+Shift+P`: pin/unpin current tab.

### 4) Per-group note history
- Added `editor_group_note_history: HashMap<u64, VecDeque<String>>`.
- Records note visits per group and supports quick swap in current group.

### 5) Consistency under watch/rename/remove
- Extended rename/remove handling to keep new tab states coherent:
  - pinned tabs updated on move/rename,
  - per-group note history updated on move/rename and purged on remove,
  - cache + pending external reload logic remains coherent.

## Validation
- `cargo fmt --all`
- `cargo check -p xnote-ui`
- `cargo test -p xnote-ui`
- `cargo test -p xnote-core`
- all passed.

## Added tests
- `compact_preview_from_content_limits_lines_and_chars`
- `pin_reorder_places_pinned_before_unpinned`

## Outcome
- Split now behaves as expected for side-by-side reading/comparison.
- Group/tab operations are much closer to VSCode/Zed style while retaining note-first interaction priorities.

