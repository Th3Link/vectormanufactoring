# Plan for Edit interaction polish

The feature ships in four PRs (`adrs.md` decision 8). Each PR's implementer
appends its own section; a merge keeps every section.

## PR 2: dashed selection box (`story/edit-polish-dashed-box`)

Part E, criteria 63 to 68. Worktree `/home/marc/workbench/vecmanf-claude/edit-polish-2`.
No new crate, no new dependency, no document change, no `format_version`
change. Runs beside PR 1; the only shared file is `session/draw.rs`'s input
assembly, which this PR does not need to change (the ratio is set on `Session`
and read where the select decoration input is built, `session/select_view.rs`).

### Affected crates/modules

- `vecmanf-render-core`: new `select_box.rs` (moved box code, `fit_dashes`,
  the pixel snap, the dashed box); `select_decoration.rs` loses the box code;
  `theme.rs` (dash and skew guide constants); `lib.rs`.
- `vecmanf-editor-wasm`: `session/mod.rs` (one field and `set_device_pixel_ratio`),
  `session/select_view.rs` (passes the ratio), `wasm_api.rs` (the ratio is set
  from the two calls that already receive it; the file does not grow).
- `docs/design-system.md`: the skew guide row, the box rows.

### Tasks

- [x] 1. Pure move: the box drawing (`SelectionBox`, `SelectDecorationInput`,
  `build`) from `select_decoration.rs` to `select_box.rs`. No behaviour change
  (fulfils nothing; prepares AC 63 to 66 and keeps both files under 500 lines).
- [ ] 2. `fit_dashes(edge_px) -> Option<DashFit>` with its table tests first:
  every length 10 to 400 px in 0.25 steps (dash at both ends, symmetric, gap 2
  to 4 except in the two flex bands, dash at most 4 and at least 2.5), edges
  under 10 px solid, the two bands 12 to 16 and 20 to 22 (AC 64).
- [ ] 3. The dashed selection box: each edge laid out from its first corner in
  the box's own frame, corners closed, solid hover box, marquee untouched
  (AC 63, 64, 66, 67). Tests: rigid under translation and zoom, rotated boxes
  follow their edges, multi-selection has one box each, equal inputs give equal
  draw lists.
- [ ] 4. The pixel snap and `SelectDecorationInput::device_pixel_ratio` for
  axis-aligned selection and hover boxes (AC 65, 66). Tests: at most 0.5 device
  pixels of displacement; one whole device row or column of coverage at ratios
  1, 1.25, 1.5, 2 and 3; a rotated box is not snapped.
- [ ] 5. Skew guide constants 2 on / 2 off; `the_skew_guide_draws_as_dashes`
  rewritten to the exact count of eighteen dashes at 70 px (AC 68).
- [ ] 6. `Session::set_device_pixel_ratio`, called from `attach_canvas` and
  `resize` (AC 65). Session test: the input carries the ratio.
- [ ] 7. Browser pixel readback (not a CI test): axis-aligned box rows at DPR 1,
  1.5 and 2, zoom 100 and 200 percent, a rotated box, the skew guide (AC 63 to 68).
- [ ] 8. `docs/design-system.md` rows (skew guide, selection box, hover box).

### Decisions taken here (inside the ADR)

- `fit_dashes` picks, among the counts that fit exactly (dash 4, gap 2 to 4),
  the one whose gap is nearest 3, not the smallest count the ADR text says.
  The smallest count is the largest gap, so nearly every edge would draw as
  4 on / 4 off; the customer's V1 is 4 on / 3 off. Edges in the two bands
  without an exact fit use gap 2 and a flexed dash, as the ADR says.
- Dashes at a corner extend half a line width past the corner, so the two edges
  cover the corner pixel completely. The hover box (20 percent alpha) is not
  extended, so no corner pixel is blended twice.
- The pixel snap works in CSS pixels times the ratio, and the axis-aligned line
  width becomes `max(1, round(ratio))` device pixels (at ratio 1.5 that is 2
  device pixels, 1.33 CSS pixels), because 1.5 device pixels cannot be crisp.
- An edge longer than 50,000 screen pixels is drawn solid (it bounds the draw
  list at absurd zoom; no clipping to the viewport exists in render-core).

### Validation

Unit tests in `vecmanf-render-core` (pure functions and draw lists), one session
test in `vecmanf-editor-wasm`, the pixel readback in the browser pane, and the
whole CI gate (`.github/workflows/ci.yml`) on the PR head.
