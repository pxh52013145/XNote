# 2026-02-08 Search & Quick Open UX Hardening

## Stage Goal
- Complete the requested IDE-grade UX uplift for Knowledge search surfaces in one pass:
  - Quick Open weighted ranking hardening (fuzzy + path/stem weighting).
  - Search results grouped by file with collapse/expand behavior.
  - Match highlighting in both panel search and command palette search.
  - Keep rendering stable (non-jitter) and interaction deterministic.

## Scope Delivered

### 1) Data model upgrades (UI state and rows)
- Extended `SearchRow` to carry highlight metadata:
  - File row: `path_highlights`.
  - Match row: `preview_highlights`.
- Extended `OpenPathMatch` to carry `path_highlights`.
- Introduced grouped search structures:
  - `SearchResultGroup` (file-level group)
  - `SearchMatchEntry` (line-level hit under each file)
- Added grouped state containers in window state:
  - `search_groups`
  - `palette_search_groups`
- Added collapse state sets:
  - `search_collapsed_paths`
  - `palette_search_collapsed_paths`

### 2) Deterministic grouping + collapse helpers
- Added helper methods:
  - `refresh_search_rows_from_groups`
  - `refresh_palette_search_rows_from_groups`
  - `toggle_search_group_collapsed`
  - `toggle_palette_search_group_collapsed`
- Added shared flatten utility:
  - `flatten_search_groups(groups, collapsed_paths) -> Vec<SearchRow>`
- Behavior:
  - Always render file header first.
  - Child matches hidden when group collapsed.
  - Selection kept valid after toggle/rebuild.

### 3) Search compute pipeline upgraded
- `schedule_apply_search` now builds `Vec<SearchResultGroup>` instead of flat rows.
- Cache changed to grouped payload (`search_query_cache`).
- Flat rows are derived only from grouped state + collapse state.
- All empty/failure/index-missing paths now clear group state and rebuild rows through unified helpers.

### 4) Palette search pipeline upgraded
- `schedule_apply_palette_results` now:
  - Quick Open branch: computes and carries path highlights.
  - Search branch: computes grouped search results + preview highlights.
- Palette search collapse state retained only for existing paths after each refresh.
- Empty/cached/error branches cleaned through group-aware reset flows.

### 5) Quick Open weighted ranking hardening
- Added UI-side ranking pass:
  - `apply_quick_open_weighted_ranking(query, paths, max_results)`
- Ranking factors include:
  - file stem exact/prefix/contains,
  - file name prefix,
  - full path prefix/contains,
  - subsequence fuzzy score (stem-weighted + path-weighted),
  - token boosts,
  - deterministic tie-breakers (shorter path, then lexical).
- Added lightweight subsequence helper:
  - `subsequence_score_simple`

### 6) Stable highlight rendering
- Added token extraction and highlight range builder:
  - `unique_case_insensitive_tokens`
  - `collect_highlight_ranges_lowercase`
- Highlight ranges are merged to avoid fragmented repaint noise.
- Added rendering helper:
  - `render_highlighted_segments`
- Important stability details:
  - byte-range conversion is made UTF-8 safe for display slicing,
  - highlight/non-highlight segments rendered as fixed inline spans,
  - no layout-height jitter introduced.

### 7) Interaction behavior
- Search panel + palette search now support:
  - click file row to toggle group collapse,
  - `Left/Right` keyboard to collapse/expand selected group,
  - `Enter` on file row toggles collapse,
  - `Enter` on match row opens note at line.
- Added row tooltips for truncated paths and match lines.

### 8) GPUI contract alignment
- Preserved existing overlay/modal interaction boundaries.
- No click-through/modal policy regressions introduced in this pass.
- Kept list rows fixed-height and deterministic for smooth virtual-list behavior.

## Tests Added/Updated
- Added tests in `crates/xnote-ui/src/main.rs`:
  - `quick_open_weighted_ranking_prefers_stem_then_short_path`
  - `flatten_search_groups_hides_matches_for_collapsed_group`
  - `collect_highlight_ranges_merges_overlaps`
- Existing tests kept green after refactor.

## Validation Run
- `cargo fmt --all`
- `cargo check -p xnote-ui`
- `cargo test -p xnote-core`
- `cargo test -p xnote-ui`

All passed in this milestone.

## Current Status
- Quick Open / Search UX requested in this wave is now landed with grouped+highlighted+collapsible behavior.
- State model is ready for next-level extensions (e.g. file-group pinning, advanced fuzzy explainability, per-group lazy expansion).

## Next Suggested Follow-up
- Add optional persisted collapse memory per query-session (debounced, bounded map).
- Add keyboard jump-to-next-file-group / previous-file-group.
- Add richer highlight strategy (token class colors, exact vs fuzzy visual differentiation).
