# Plan for Document background

One branch `story/document-background`, one PR, opened when the whole slice can be tried
end to end. The four milestones of `adrs.md` decision 12 are commits on that branch, each
leaving the local gate green; none of them is a PR. Spec, UX notes and ADR notes:
`specification.md` and `adrs.md` in this folder.

Based on `origin/main` plus the local integration commit of #80 and #84 (`chore: local
integration`, to be dropped with `git rebase --onto origin/main <commit>` once both land).

Defaults taken (the spec's open questions, customer may change): default colour `#E8E8EB`;
the pasteboard picks nothing; None is drawn as a checkerboard; the picker is always visible;
the block sits below Fit to content; no shortcut; no edge line on a None document; chip
word order "Background #RRGGBBAA".

## Affected crates/modules
- `curvyo-document-core`: new `document_background.rs` (value, registers, strict
  validation, `Document::background` and `set_background`); `document.rs` (format version 10,
  `document.json` view, validation call); `units.rs` (`DocumentSize::contains`); `lib.rs`;
  fixtures in `tests/fixtures/` (`background_v10.curvyo` and four damaged files).
- `curvyo-render-core`: `document_area.rs` (background-aware area, `checker_tone`,
  `checker_grid`, `background_at`), `glyphs.rs` (`DrawList` checkerboard prefix), `theme.rs`
  (`CHECKER_A`, `CHECKER_B`, `CANVAS_BG` from the document default), `artwork.rs` (`paint()`
  made `pub(crate)`), `pen_preview.rs` and `pen_cue.rs` (new `background_at` arguments).
- `curvyo-editor-wasm`: `gpu_pipeline.rs` (second fragment entry point, third pipeline, uniform
  fields), `gpu.rs` (draw calls), `session/background.rs` (new: edits, preview, view),
  `session/frame.rs`, `session/colour_pick.rs`, `session/mod.rs` (one field), new
  `wasm_background.rs` bindings, `lib.rs`.
- `curvyo-ui-core`: `colour_pick.rs` (background source, `PaintTarget::Background`).
- `curvyo-storage-io`: a test that older fixtures open unchanged on disk (criterion 4).
- `frontend`: `BackgroundBlock.tsx`, `useBackgroundPanel.ts`, `ColourBlock.tsx`,
  `ColourPicker.tsx`, `ValueField.tsx` (callbacks instead of `StylePanelApi`), `Swatch.tsx`
  unchanged, `DocumentSection.tsx` (the block), `ColourPickChip.tsx`, `Canvas.tsx` (chip and
  announcement), `PropertiesPanel.tsx` (scroll rules).
- Docs: `specs/README.md` (format version table), `specification.md` (status line), the
  comment on `CURRENT_FORMAT_VERSION`.

## Milestone 1: `document-core` (AC 1 to 8, 22, 40 to 45 model side)
- [x] 1. `DocumentSize::contains` (closed, tolerance zero) with tests (AC 34).
- [x] 2. `document_background.rs`: types, `DEFAULT`, read (lenient), `set_background` per
  register with one commit `set_document_background`, equal value writes nothing (AC 1, 2, 3,
  17, 22).
- [x] 3. Strict `validate` on open; four damaged fixtures with a test each; absent keys valid
  (AC 4, 7).
- [x] 4. Format version 10, `document.json` gains `background`, comment paragraph on the
  constant, tests compare against `CURRENT_FORMAT_VERSION` (AC 5); `acceptance_0030` literal 9
  follows the constant.
- [x] 5. Golden `background_v10.curvyo`; round trip keeps every component and a None paint
  keeps its colour (AC 6); older fixtures open with the default and byte-identical on disk
  in `storage-io` (AC 4).
- [x] 6. Resize, Fit, presets, display unit and copy write no background register (AC 42 to
  44); two-peer merge keeps one complete colour; paint and colour from two peers both survive
  (AC 45).

## Milestone 2: `render-core` and the GPU (AC 9 to 14, 46, 47, 49)
- [x] 7. `DrawList` checkerboard prefix (`checker_end`, `extend` keeps self's prefix); theme
  tones (AC 12, 47).
- [x] 8. `checker_tone`, `checker_grid` (device cell `round(8 dpr)`, at least 1, corner snapped
  as the area) and `build_document_area(size, background, view, dpr)`: opaque is one quad as
  before, None one checker quad, translucent checker quad plus colour quad (AC 9 to 13, 47).
- [x] 9. `background_at(size, background, point)` and the Pen call sites (AC 46).
- [x] 10. Shader `fs_checker`, third pipeline, uniform with corner, cell and tones, draw calls
  in `Gpu::render`; wasm32 build and a one-off naga 30.0.1 validation of the WGSL (AC 12). The
  shader runs only in a browser; it has not been seen on a screen here.

## Milestone 3: `ui-core` and `editor-wasm` session (AC 14 to 39 session side, 40)
- [x] 11. `pick_colour` with the background source, `PaintTarget::Background` (AC 28 to 34, 48).
- [x] 12. `session/background.rs`: paint, hex, picker preview, Opacity (drag, key step, reset,
  type), commit and cancel, view record, preview read by `frame_draw_list` and `background_at`
  (AC 14 to 25 core part, 22, 23).
- [x] 13. Eyedropper for the background: button target, press writes one commit, equal value
  writes nothing and ends picking, chip hover text, status text (AC 29, 32 to 39).
- [x] 14. Not an object: click, Ctrl+A, nudge, Delete, Fit and the object count never see it (AC 40, 44);
  New and Open start without preview (AC 8). There is no clipboard for objects yet, so AC 44 holds
  structurally (the background is not in the objects tree).
- [x] 15. `wasm_background.rs` bindings.

## Milestone 4: frontend (AC 15 to 27, 35, 36, 39)
- [x] 16. Colour components take callbacks; Style section keeps working.
- [x] 17. `BackgroundBlock.tsx` and `useBackgroundPanel.ts`; block in `DocumentSection`
  (AC 15 to 27).
- [x] 18. Panel scroll rules (AC 15a) and focus rules (AC 25). The scroll rules were already the panel's (one scroll, `scroll-padding-block: 12px`, body keyed on the tab); focus uses `useRowsFocus`. The 800 x 600 positions of 15a are not measured here (no browser): the UX review does that.
- [x] 19. Chip with the background source, announcement, cursor over the pasteboard
  (AC 35, 36, 39).
- [x] 20. Docs: README table, spec status; frontend lint, typecheck, tests.

## Validation
Core logic test first. Golden files for the file format (valid v10, four damaged, older
fixtures byte-identical after Open). Pixel rules of the shader are mirrored by the pure
`checker_tone` and tested natively at ratios 1 and 2; the WGSL is validated with naga. The
frontend is not exercised in a browser here (a UX review follows); it is type-checked and
linted. Full gate of `CLAUDE.md` section 7 plus `wasm32` builds of every touched core crate.
