# Knowledge Editor Engine RFC (Native Rust + GPUI)

Date: 2026-02-07  
Status: Draft (Implementation scaffold started)

## 1. Background

Current Knowledge editor is text-editable and stable enough for MVP workflows, but still lacks a dedicated native editor engine boundary. The current implementation keeps most behavior in UI code paths, which makes long-term maintainability and IDE-grade evolution harder.

This RFC defines a native editor engine direction aligned with XNote goals:

- Rust-first core.
- GPUI-native rendering.
- IDE-grade extensibility and reliability.
- No WebView dependency.

## 2. Goals

- Define a reusable editor engine contract in `xnote-core`.
- Move edit transaction semantics (apply/undo/redo/version/stats) into core.
- Keep UI as renderer + input adapter, not the owner of edit correctness.
- Prepare for incremental markdown parsing + preview/split architecture.

## 3. Non-Goals (This Phase)

- Full markdown semantic rendering in editor body.
- Syntax highlighting and code-block language services.
- Multi-cursor editing.
- CRDT/collaborative editing.

## 4. Architecture Proposal

## 4.1 Layering

- `xnote-core::editor` (new)
  - canonical text buffer + edit transactions + undo/redo + stats.
- `xnote-ui`
  - input handling, layout, painting, IME bridge.
  - delegates textual mutations to core editor buffer.

## 4.2 Core Types

- `EditorBuffer`
  - rope-backed buffer.
  - versioned edits.
  - undo/redo stacks.
- `EditTransaction`
  - insert/delete/replace over byte ranges (UTF-8 boundary validated).
- `EditRecord`
  - before/after inverse history for undo/redo.
- `EditorStats`
  - chars, lines, words.

## 4.3 Data Flow

1. Open note -> load string from vault.
2. Initialize `EditorBuffer` from content.
3. UI edit action -> build `EditTransaction` -> `EditorBuffer::apply`.
4. UI refreshes from `EditorBuffer::to_string()`.
5. Autosave writes current content to vault.

## 5. Why Native Instead of CM6/Monaco

- Avoid WebView runtime and cross-language bridge complexity.
- Preserve predictable keyboard/focus behavior under GPUI.
- Keep performance profiling and crash analysis fully Rust-native.
- Align with Zed-like lightweight/low-overhead architecture target.

## 6. Risks and Mitigations

- Risk: byte/char boundary bugs in UTF-8 edits.
  - Mitigation: strict boundary checks + dedicated tests.
- Risk: regression from mixed legacy/new edit paths.
  - Mitigation: progressively route all mutation paths through core buffer.
- Risk: large render functions still pressure debug stack.
  - Mitigation: split render builders and keep stack reserve guard.

## 7. Acceptance Criteria

- All editor mutation paths can be routed through `xnote-core::editor`.
- Undo/redo works for keyboard edits in primary editing flow.
- `cargo test -p xnote-core` passes with editor tests.
- `cargo check -p xnote-ui` passes with core buffer integration.

## 8. Milestones

## M1: Engine Foundation (done/started)

- Add `xnote-core::editor` rope-based buffer.
- Add transaction model + undo/redo + stats.
- Add core unit tests for apply/undo/redo/utf8 boundary.

## M2: UI Integration Baseline (in progress)

- Initialize `EditorBuffer` on note open.
- Route primary replace paths through `EditorBuffer`.
- Add `Ctrl+Z/Ctrl+Y` integration in editor key handling.

## M3: Mutation Path Unification

- Remove remaining direct string-splice write paths.
- Ensure IME replace-and-mark path also uses transaction model.

## M4: Markdown Engine Bridge

- Add parser abstraction in core (`parse(document)` interface).
- Start with `pulldown-cmark` for preview AST/event stream.
- Keep editor mode text-first; add preview pipeline separately.

## M5: Preview/Split View

- Add `Edit / Preview / Split` modes in UI shell.
- Preview rendered from parser output, not raw heuristics.
- Keep scroll/focus contracts stable.

## M6: Performance and Hardening

- Incremental parse invalidation windows.
- profiling hooks for p50/p95 edit latency.
- targeted tests for large-note editing and rapid undo/redo.

## 9. Implementation Notes (Current)

Initial scaffold has been added:

- `crates/xnote-core/src/editor.rs`
- `crates/xnote-core/src/lib.rs` (exports `editor`)
- `crates/xnote-core/Cargo.toml` (adds `ropey`)

Initial UI wiring has started:

- note open initializes `EditorBuffer`
- major replace paths attempt buffer transaction first
- keyboard undo/redo added (`Ctrl+Z`, `Ctrl+Y`)

Further cleanup is tracked in M2/M3.
