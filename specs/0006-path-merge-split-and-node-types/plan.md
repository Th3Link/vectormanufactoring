# Plan for "Path merge/split and a third node type"

## Scope note (read before the task list)

This slice was built in parallel with `canvas-navigation-and-selection`
(slice 4), directly off `main`, **before slice 4's `ObjectSelection`
landed**. Per the lead's brief, this rules out building any selection
mechanism for "pick two path objects, then switch to the Node tool"
(acceptance criteria 6, 7, and acceptance criterion 9's "two different
pre-existing open path objects" case). Those three are **deferred**,
explicitly, not silently dropped — see "Deferred" below.

Everything else ships: the three node kinds (1-5, 16), Split (12-15),
and same-path Join (8, 10, 11, and the "two ends of one open path" half
of 9). `Document::join_endpoints` itself is built fully generic over
same-path vs. cross-path — the cross-object *document-model* operation
works and is unit-tested — only the UI path to *select* two different
pre-existing objects' endpoints together is missing.

One side effect worth recording: Split's own result (acceptance
criterion 13) can legitimately produce two coincident nodes on two
*different* path objects, and criterion 15 asks for both to end up
selected, immediately re-Joinable. That one case *is* reachable end to
end without `ObjectSelection` — it is exercised by `vecmanf-ui-core`'s
and `vecmanf-editor-wasm`'s own tests (`split_then_rejoin_restores_one_object`,
`split_selected_on_an_interior_node_through_the_session`).

## Affected crates/modules

- `vecmanf-document-core`: `path_model.rs` (`AnchorKind::Asymmetric`,
  two new `PathEditError` variants), `path_codec.rs` (kind tag
  rename/alias, `write_closed`, `write_path_style`), `paths.rs`
  (`resolve_handle_pair`'s Asymmetric rule, `convert_anchor_kind`'s full
  table + no-op rule, `join_endpoints`, `split_at_anchor`,
  `create_path_uncommitted`), `document.rs` (`CURRENT_FORMAT_VERSION`
  4 — see note below).
- `vecmanf-ui-core`: `selection.rs` (`NodeSelection`'s new `SplitPair`
  state, `join_pairs()`), `node_tool.rs` (`can_join`/`can_split`,
  `join_selected`/`split_selected`, `NodeToolbarState`'s new/renamed
  fields).
- `vecmanf-render-core`: `glyphs.rs` (`triangle`), `decorations.rs` /
  `pen_preview.rs` (third glyph arm).
- `vecmanf-editor-wasm`: `session/mod.rs` / `wasm_api.rs`
  (`join_selected`/`split_selected` passthrough, `kind_from_str`,
  `NodeToolbarState` mirror).
- `frontend/`: `useEditorSession.ts`, `NodeToolbar.tsx` (3-segment
  kind `ToggleGroup`, Join/Split buttons + context-menu entries),
  `App.tsx`/`Canvas.tsx` wiring.

## `format_version` note

`adrs.md` says "`format_version` goes to 5" on the assumption that
`object-transform` (slice 5) had already merged and taken 4. On `main`
as branched, `CURRENT_FORMAT_VERSION` was still 3 and no `rotation`
register exists yet. This build takes **4**, the next free version, and
records the reconciliation need as a dated note in `document.rs` and in
`adrs.md` itself. Whichever of this slice and `object-transform` merges
second needs its version renumbered — expected integration work, not a
defect in either slice.

## Tasks

- [x] 1. Rename `AnchorKind::Smooth` → `Symmetric` everywhere (label-only
      at the UI surface); add `Asymmetric`. Codec: write
      `"symmetric"`/`"asymmetric"`, read `"smooth"` as an alias
      (never rewritten). `format_version` → 4. (AC 1)
- [x] 2. `resolve_handle_pair`'s Asymmetric rule (rotate without
      rescaling the opposite); `set_handle` reads real current handles
      and writes the mirror for Asymmetric too (bug found while
      testing: the old code only mirrored for `Symmetric`). (AC 3)
- [x] 3. `convert_anchor_kind`'s full 3×3 table: Corner→Asymmetric
      (per-side keep-or-default length), the shape-preserving
      conversions (kind-only), Corner/Symmetric→Symmetric reset, and
      the same-kind no-op (no write, no commit). (AC 2, 4, 5, 16)
- [x] 4. `Document::join_endpoints`: same-path close (AC 10) and
      cross-object merge (AC 9) in one generic command, covering all
      four endpoint-combination reversals; refuses per AC 8's
      conditions including the 2-anchor-path gap. (AC 8, 9, 10, 11)
- [x] 5. `Document::split_at_anchor`: open-path split into two objects
      (AC 13, via a new `create_path_uncommitted` shared with
      `create_path` so Split stays one commit) and closed-path split
      via movable-list `mov` rotation (AC 14). (AC 12, 13, 14)
- [x] 6. `vecmanf-ui-core`: `NodeSelection::SplitPair` + `join_pairs()`
      (narrowly scoped — see `selection.rs`'s own doc comment for why
      this is not a general cross-path selection mechanism);
      `NodeTool::can_join`/`join_selected`/`can_split`/`split_selected`.
      (AC 8, 11, 12, 15)
- [x] 7. `vecmanf-render-core`: triangle glyph, third `match` arm in
      decorations/pen-preview.
- [x] 8. `vecmanf-editor-wasm` + `frontend/`: wasm passthroughs, binding
      strings, 3-segment `ToggleGroup` + Join/Split buttons (toolbar and
      context menu), disable-don't-hide throughout.
- [x] 9. Golden-file migration test: reused `paths_v2.vmf` (already
      stores the legacy `"smooth"` tag) rather than adding a new
      fixture, per `adrs.md`'s own note that this is the intended test.
- [ ] 10. **Deferred**: AC 6, AC 7, and AC 9's "two different
      pre-existing open path objects" case — no multi-object Node-tool
      session, no cross-object selection UI. Blocked on
      `canvas-navigation-and-selection`'s `ObjectSelection` landing.

## Validation

- Unit tests throughout `vecmanf-document-core` (new `join_endpoints`/
  `split_at_anchor`/conversion-table/`resolve_handle_pair` tests),
  `vecmanf-ui-core` (`NodeTool`/`NodeSelection`), `vecmanf-editor-wasm`
  (`Session`-level end-to-end Join/Split).
- Golden-file: `paths_v2.vmf` (existing fixture) now doubles as the
  `format_version` migration test.
- Full gate (`CLAUDE.md` §7): green except `vecmanf-app`'s pre-existing
  `tauri-macros` proc-macro panic, confirmed to reproduce identically
  on an unmodified `main` checkout (unrelated to this story).
