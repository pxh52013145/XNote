# AGENTS.md (xnote-ui / GPUI UI Guardrails)

This file defines mandatory UI engineering rules for all files under `crates/xnote-ui/`.

## Auto-Apply Skill Rule

For any task that touches `xnote-ui` or mentions UI behavior (including settings, modal, overlay, palette, sidebar, status bar, hotkeys, focus, scroll, theme, responsive behavior), the agent must automatically apply:

- Skill: `gpui-ide-ui-standards`
- Path: `C:/Users/31625/.codex/skills/gpui-ide-ui-standards/SKILL.md`

The user does not need to explicitly mention the skill name.

## Progressive Disclosure Rule

Do not load everything at once. Load only what is needed:

1. Read `SKILL.md` first.
2. Then load targeted references as needed.
3. At minimum, run through review checklist before finalizing:
   - `C:/Users/31625/.codex/skills/gpui-ide-ui-standards/references/review-checklist.md`

## Mandatory GPUI Contracts

When implementing or changing UI, preserve these contracts:

- Overlay/modal must block click-through to base UI.
- Opening click must not immediately trigger backdrop close.
- Modal shell size remains stable across sections/tabs.
- Long pages use inner scrolling, not shell resizing.
- `Esc` and hotkeys route to top-most active surface.
- Async UI updates must be nonce/validity guarded.
- Avoid panic-prone indexing in render/list code.

## Verification

Before final response for UI changes, run at least:

- `cargo check -p xnote-ui`

If behavior changed materially, also run relevant broader checks when feasible.
