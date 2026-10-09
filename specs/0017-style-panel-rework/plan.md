# Plan for Style panel rework (and, on the same branch, stroke markers)

One branch (`story/style-panel-rework`), one PR. The customer widened the PR
(2026-10-09): it carries `0017-style-panel-rework` (milestones 1 to 4), then
`0018-stroke-markers` (milestones 5 to 7, criteria numbered as in
`specs/0018-stroke-markers/specification.md`), then `0030-document-size-presets`
once its spec is Ready (milestone 8, added then). Both features share ONE
`format_version`: `CURRENT_FORMAT_VERSION` is 9, taken by the odd dash lists of
0017 and by the marker keys of 0018, so the constant moves once. Each milestone
is a set of commits that leaves the tree green (`CLAUDE.md` §7 on the touched
crates); the full gate runs before the PR is opened. Criteria are numbered as in
`specification.md`; decisions as in `adrs.md`.

## Affected crates/modules

- `curvyo-document-core`: `style_model` (Fill loses kind and stops, `DashPattern`
  accepts odd lists), `legacy_fill` (new, the read-past rule for old gradient
  keys), `style_codec`, `style_validation`, `styles` (`StyleEdit::StrokeRgba`,
  `FillEnabled`, `FillRgba`), format version bump, fixtures
  `legacy_gradient_v7.curvyo` and `dash_v<N>.curvyo`.
- `curvyo-render-core`: gradient code deleted, `build_artwork(objects, view)`,
  `dash.rs` (odd lists, zero-length "on" draws a dot).
- `curvyo-ui-core`: `style_stops` deleted, `style_panel` / `style_entry` /
  `style_edit` reshaped, new `colour_hsv`, `value_scale`, `colour_pick`,
  `parse_dash_text`, eyedropper hit test (`hit_test_object` internals).
- `curvyo-editor-wasm`: GPU gradient parts and stops session deleted, style view
  and panel calls reshaped, `preview_value_field` / `step_value_field`, the
  session's pick mode (`begin_colour_pick` / `end_colour_pick`).
- `frontend`: gradient components deleted; new `components/panel/` folder (rows,
  inline picker, value field, dash group and line, eyedropper), `useValueDrag`,
  oxlint override for `components/panel/`; `react-colorful`, `ui/popover`,
  `ui/select`, `ColourPopover`, `ColorAlphaPicker`, `DashSelect` removed.
- Docs: `docs/technical-debt.md`, ADR 0002 section 10 note (already amended by
  the architect), this folder.

## Milestones and tasks

### Milestone 1: gradients removed (criteria 49 to 54)

- [x] 1.1 `document-core`: Fill is `enabled` / colour / opacity; `legacy_fill`
  reads an old gradient fill as off and drops `fill_kind` / `fill_stops` on the
  next fill write; fixture renamed with its bytes unchanged, tests for open,
  no-write-on-open, Paint Solid, colour edit, save without edit (53, 54).
- [x] 1.2 `render-core`: gradient ramp, frames, draw-list ranges deleted (51).
- [x] 1.3 `ui-core`: stop editor, `StopField`, fill modes deleted; the fill row
  reads paint on/off/mixed (49).
- [x] 1.4 `editor-wasm`: GPU ramp texture and vertex attribute, stops session,
  `gradient_frames` deleted; `set_fill_mode` becomes `set_fill_paint(on)` (49,
  51).
- [x] 1.5 `frontend`: `GradientEditor`, `GradientBar`, `RampTrack`, `StopRows`,
  `StopThumb`, `useThumbDrag`, gradient icons and options deleted (49).
- [x] 1.6 Docs: `technical-debt.md` loses its gradient entries and gets the
  `legacy_fill` item (52).

### Milestone 2: panel content (criteria 1 to 21, 28 to 33, 49, 55 to 60)

- [x] 2.1 Empty and hidden rules in `style_panel` / `style_view` and the panel
  components: no `disabled` anywhere, Paint None hides the rows, heading and
  switch are one row, mixed Paint shows the rows (1 to 10).
- [x] 2.2 8-digit RGBA hex: `StyleEdit::StrokeRgba` / `FillRgba`, `parse_hex`
  for 3/4/6/8 digits, alpha stored as AA/255 (11 to 16).
- [x] 2.3 `colour_hsv.rs` and the inline picker (area, hue slider) in TSX; remove
  `react-colorful`, `ColourPopover`, `ColorAlphaPicker`, `ui/popover` (17 to 21,
  55, 56).
- [x] 2.4 Dash: preset group, text line, `parse_dash_text`, odd lists in
  `DashPattern`, renderer expansion and zero-on dot, format version bump and
  fixture, `DashSelect` and `ui/select` removed (28 to 33).
- [x] 2.5 `.oxlintrc.json` override for `components/panel/`, Escape order, focus
  rules (1, 57, 59, 60).

### Milestone 3: value fields (criteria 34 to 48, 61)

- [x] 3.1 `value_scale.rs` with the check values of criteria 46 and 47.
- [x] 3.2 wasm `preview_value_field` / `step_value_field`; `ValueField.tsx`,
  `useValueDrag.ts`; `NumberField` becomes `EntryField` without `disabled`.
- [x] 3.3 Reset slot and Ctrl+Backspace; spinbutton semantics; user-select rules
  (34 to 45, 61).

### Milestone 4: eyedropper (criteria 22 to 27)

- [x] 4.1 `colour_pick.rs` and the session pick mode (`begin_colour_pick` /
  `end_colour_pick`, view fields `pick_target` / `pick_hover_hex`).
- [x] 4.2 Frontend eyedropper button, cursor, hover chip, cancel rules (22 to 27).

### Milestone 5: marker model and format (0018 criteria 3, 4, 6, 10, 22, 23, 25 to 29)

- [x] 5.1 `document-core`: `MarkerShape`, `MarkerPlace`, `MarkerCount`, `Markers`
  in `Stroke`; five registers in the codec; strict open validation; `StyleEdit`
  marker variants with `StyleEditError::NotAPath`; no new format version (9 is
  shared); fixture `markers_v9.curvyo`; older fixtures open with every slot None.

### Milestone 6: marker placement and drawing (0018 criteria 4 to 5, 7 to 17, 19)

- [x] 6.1 `render-core`: `marker_place.rs` (anchors, tangents, spaced fractions),
  `markers.rs` (arrow, dot, tessellation), the stroke layer rule, `MarkerBudget`,
  the benchmark extension, debt note.

### Milestone 7: marker panel (0018 criteria 1 to 3, 18 to 20, 22, 24, 30 to 32)

- [x] 7.1 `ui-core`: `ValueScale::MarkerCount`, panel state (Markers group, Place,
  Count, mixed over paths, the closed-path line flag); `editor-wasm` calls.
- [x] 7.2 Frontend: Markers rows under Cap, Place group, Count value field,
  muted closed-path line.

## Validation

- Rust: unit and integration tests per task, written first for the core logic;
  the check values of criteria 46 and 47; golden fixtures for the legacy gradient
  file and the odd dash list.
- Frontend: `tsc`, `oxlint`, `npm test`; the DOM rules (selection, focus, no
  popup roles) are checked by hand in the Browser pane against a production
  wasm build, because no component test runner exists (`technical-debt.md`).
- Full gate of `CLAUDE.md` §7 before the PR is opened.

## As-built notes (for the product owner)

- **A version-8 file that holds an odd dash list** opens as stored: open-file
  validation does not look at the declared `format_version` for this key. The
  spec does not define the case; an older (version-8) writer never wrote one, so
  only a hand-edited file can hit it. Accepted.
- **Criterion 37's worked example** ("300 px out, 100 px back") gives 19.5 mm for
  Width, not exactly 20, because the scale is logarithmic and the value at
  `dx = 200 px` from the press is not the value at the end of the scale. The rule
  (`clamp(p0 + dx / W)`, no re-basing at the ends) is built as written; only the
  example's number is off.
- **A system `pointercancel` during a value drag** drops the preview and writes
  nothing (criterion 40), the same path as Escape.
