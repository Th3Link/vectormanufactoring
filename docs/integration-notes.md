# Integration notes: `integration/preview`

Temporary. For the lead and the later per-PR rebases; do not delete. The branch
`integration/preview` merges the three open slice PRs onto `origin/main` (`dc3ecef`), in this
order, one merge commit each: #79 `story/path-tools`, #78 `story/style-panel-rework`, #80
`story/multi-object-transform`. It is a try-out tree, never merged into `main`.

## Textual conflicts and how they were resolved

Merge 2 (#78 into #79):

| File | Resolution |
|---|---|
| `curvyo-document-core/src/lib.rs` | Both lists kept: `junction`, `path_extend`, `DocumentVersion`, `merged_junction`, `reversed_anchors`, `smooth_corner_handles` (#79) and `document_presets`, `legacy_fill`, the marker types (#78). `gradient_ramp` and its exports stay removed (#78). |
| `curvyo-editor-wasm/src/lib.rs` | One export list with `BreakApartOutcome`, `ClosePathOutcome`, `ClosePathState`, `CombineOutcome` and `DocumentPresetsRecord`. |
| `curvyo-ui-core/src/lib.rs` | Modules and exports of both: `break_apart`, `close_path`, `closing_join`, `combine` and `colour_hsv`, `colour_pick`. |
| `curvyo-editor-wasm/src/session/mod.rs` | Module list: `close_path`, `colour_pick`, `combine`. `colour_pick_press/move/release` stay the first thing in `pointer_down/hover/up`, above the Pen and Node code. |
| `curvyo-editor-wasm/src/session/draw.rs` | `build_artwork(&artwork_objects, view)` (#78, no gradient frames), then #79's bend-preview block after it. |
| `frontend/src/components/Canvas.tsx` | Imports of `ColourPickChip` (#78) and `PenHintChip`, `usePenCue` (#79). The inline eyedropper cursor still overrides the Pen cursor classes. |
| `docs/technical-debt.md` | Both appended sections kept. |

Merge 3 (#80 into the result):

| File | Resolution |
|---|---|
| `curvyo-editor-wasm/src/session/draw.rs` | Import list without `GradientFrame` (#78) and with `build_group_draw_list` (#80). |
| `curvyo-render-core/src/lib.rs` | `mod group_box` and its export (#80); the gradient module and exports stay removed (#78). |
| `curvyo-editor-wasm/src/session/select_view.rs` | #80 moved `cursor_hint` to `select_cursor.rs`; the hunk was dropped here and #78's eyedropper check is the first check of `select_cursor.rs::cursor_hint`. |
| `frontend/src/lib/cursors.ts` | Both hints kept: `"eyedropper"` (#78) and `"wait"` (#80). |
| `specs/README.md` | #78's "In progress (#78)" rows for 0017 and 0018 and #80's "In progress" row for 0019. |

No conflict in `path_model.rs`, `paths.rs`, `path_codec.rs`, `docs/design-system.md`, `Canvas.tsx` (merge 3),
`useEditorSession.ts`, `App.tsx`; they merged cleanly and compile. `format_version` stays 9 (only #78
bumps it).

## Semantic fixes (`fix(integration): ...` commits)

1. **`curvyo-editor-wasm/tests/acceptance_0019_tester.rs` used the gradient API that #78 removed.**
   `fill_object` called `set_fill_mode(FillMode::Solid)` and `ac51_style_fields_are_untouched` called
   `set_fill_mode(FillMode::Linear)`; the tree did not compile. Fix: `set_fill_paint(true)`, and for
   criterion 51 a translucent fill (`set_style_text(StyleField::FillColor, "#0000FF80")`). The assertions
   are unchanged. Belongs to the rebase of #80 onto #78 (the architect review of #80 missed this file).
2. **The eyedropper (#78) against the path tools (#79).** `session/boolean.rs` (`apply_boolean`),
   `session/combine.rs` (`apply_combine`, `apply_break_apart`) and `session/close_path.rs`
   (`close_paths`) now call `end_colour_pick()` after their gate, so the Pick button clears and a later
   press does not paint the result. `session/pen.rs::pen_target` returns `None` while picking, so the Pen
   cue, the hint chip, the join/close data and the rubber band end follow. `session/select_cursor.rs`:
   `handle_hint` is empty while picking (a stale group-handle hint chip). Already right without a change:
   the segment hover band (`pointer_hover` returns after `colour_pick_move`, and clears `hovered` first),
   Escape (picking ends first and returns), the cursor (`cursorHint === "eyedropper"` is the inline style
   and wins over the Pen classes), the panel (`pen_path_unfinished` is true during a continuation).
   Pinned by `curvyo-editor-wasm/tests/merged_slices_interplay.rs`, which also pins "Break apart shows
   the group box in the same frame", "Combine back to one drops it" and "a compound path in a selection
   moves as one path". Belongs to whichever of #78 or #79 merges second.
3. **`curvyo-document-core/src/path_model.rs` was 528 non-test lines (limit about 500, `CLAUDE.md` §5).**
   #80 left it at 499 and #79 adds `reversed_anchors` and more. Fix: a pure move. `PathSnapshot::rotated`,
   `scaled`, `scaled_along`, `sheared`, `sheared_along`, the two `scale_*_in_local_frame` helpers and their
   unit tests now live in the new `path_transform.rs` (319 lines with tests); `path_model.rs` is 394 lines.
   No signature, visibility or behaviour changed; `lib.rs` gets `mod path_transform;`. Whoever merges
   second (#79 after #80, or #80 after #79) does the same move.

## Checked, no change needed

- Markers, dashes and the straight-segment `line_to` of #80 (`render-core/src/stroke.rs`): the marker and
  dash tests of #78 and the stroke tests of #80 pass together.
- The default view inset of 128 px (#79) against #78's Document section: the 0015 and 0030 tests pass.
- Format version 9: only #78 bumps it; the `unpack(9, ..)` tests of #79 and #80 pass.
