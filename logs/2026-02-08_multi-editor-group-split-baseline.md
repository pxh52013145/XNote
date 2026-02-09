# 2026-02-08 Multi Editor Group Split Baseline

## Stage Goal
- Raise split interaction from single split-preview demo to VSCode-like multi editor group baseline.
- Support repeated split expansion and explicit editor-group focus switching.

## Implemented
1. Multi editor-group model
- Added `EditorGroup` state with stable IDs.
- Added active group tracking and safe group bootstrap.
- Group state syncs current note path per group.

2. Unlimited split growth (group append model)
- Split action now creates a new group next to active group.
- Repeated split creates additional groups (`N` groups, not fixed 2 panes).
- Added close-group action for active group (keeps at least one group).

3. Group focus switching behavior
- Clicking a group activates focus.
- Activation restores/loads the group’s note context.
- Active group updates selection and tab-state restore behavior.

4. Watcher/rename/remove consistency
- Folder/note move/remove flows now also update/clear group note bindings.
- Prevent stale group pointers after file-system events.

5. Rendering baseline
- Editor body now renders as group row with separators.
- Active group renders full interactive editor/preview according to mode.
- Inactive groups render lightweight placeholder with current note label and focus hint.

## Validation
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo test -p xnote-ui --no-run` ✅

## Notes
- This is baseline parity for multi-group split and focus semantics.
- Next parity work: per-group tabs/states and true bidirectional split tree (right/down mixed nesting) can be layered incrementally.
