# 2026-02-08 Knowledge Search UX Final Pass

## Stage Goal
- Continue hardening Knowledge search interactions for practical daily UX.
- Reduce discoverability friction for command palette branches and search workflows.

## Implemented
1. Recent query memory (lightweight)
- Added recent query ring history (capacity 8) for:
  - Palette Quick Open
  - Palette Search
  - Search panel
  - Explorer filter
- Query history is deduplicated (case-insensitive) and moved-to-front.

2. Keyboard+hint affordance
- Added palette branch hint row under input:
  - Commands: `Esc close · ↑/↓ navigate · Enter run`
  - Quick Open/Search: `Esc close · ↑/↓ navigate · Enter open`
  - Empty-query state shows `Recent: ...` when available.
- Added search panel hint row under RESULTS title:
  - Empty query: `Type to search… · Esc clear/close · Enter open` + recent tip
  - Active query: `Esc clear/close · ↑/↓ navigate · Enter open`

3. Empty-state improvements
- Search panel empty placeholder now optionally shows second line for recent query.
- Palette search empty-state explicitly distinguishes:
  - query empty => guidance text
  - query non-empty => `No search matches`

4. State lifecycle consistency
- Recent query histories are cleared on vault reopen/reset path together with search caches.

## Validation
- `cargo check -p xnote-ui` ✅
- `cargo test -p xnote-core` ✅
- `cargo test -p xnote-ui --no-run` ✅

## Current Status
- Knowledge search UX now has stronger discoverability and lower interaction friction while maintaining stable modal/input behavior.
