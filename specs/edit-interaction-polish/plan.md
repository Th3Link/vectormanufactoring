# Plan for Edit interaction polish

The feature ships in four PRs (`adrs.md` decision 8). Each PR adds its own
tasks below; a later PR does not touch an earlier PR's tasks except to check
them off. Criteria numbers are those of `specification.md`.

No new crate, no new dependency, no new trait, no `format_version` change (it
stays 5). If a task finds it needs one of these, stop and ask the lead.

## PR 1: `story/edit-polish-orientation-escape-keys`

Part A (polygon and star orientation), Part D (Escape cascade, Split
selection), the keyboard gate, and the R and S entries of Part F. Criteria 1 to
8, 42 to 52, 54 and 55 (without M and K), 57, 60 to 62. Criteria of PRs 2 to 4
are out of scope here. Worktree
`/home/marc/workbench/vecmanf-claude/edit-polish-1`, from `origin/main` at
`450f5f3`. PR 2 (dashed box, Part E) runs in `edit-polish-2` and touches
`vecmanf-render-core` selection-box code and `set_device_pixel_ratio`, which
PR 1 does not.

### Affected crates and modules

- `vecmanf-document-core`: `primitive_model.rs` (`ObjectSnapshot::orientation`).
- `vecmanf-ui-core`: new `modifiers.rs`; `poly_star_tool.rs`,
  `transform_math.rs` (`rotate_delta_for`), `transform_drag.rs`,
  `transform_entry.rs` (three readers), `node_tool.rs` (`cancel_drag`,
  `clear_selection`, Split selects one node, hit-test tie),
  `hit_test.rs` (tie goes to a selected node), `select_tool/entry.rs`
  (`open_entry_for_key`), the three shape tools (`drag_in_flight`).
- `vecmanf-render-core`: `decorations.rs` (selected node glyphs drawn last).
- `vecmanf-editor-wasm`: new `session/keys.rs` (`escape`, `delete_selected`,
  `decide`, `key_down`), new `wasm_keys.rs`; `session/mod.rs` (`button_down`,
  `pointer_cancelled`, `escape() -> EscapeStep`), `session/shapes.rs`,
  `session/select_view.rs`, `session/transform_entry.rs`, `lib.rs`.
- `frontend/`: `useEditorSession.ts` (`onKeyDown` becomes a forwarder; the letter
  `switch` is deleted; `pointercancel` calls `pointer_cancelled`),
  `ToolRail.tsx` (tooltips), a transient hint message next to `EditHintChip`.
- Docs: `docs/design-system.md` key table is the `ux-engineer`'s; this PR notes
  it in the PR description.

### Tasks

- [x] 1. `Modifiers { shift, ctrl }` in `ui-core::modifiers` (it is not on `main`;
  `shape-creation-from-center` has not landed). (supports 4, 5)
- [x] 2. `ObjectSnapshot::orientation()` in document-core: `StarFrame.angle +
  rotation` normalised to (-180, 180], other kinds the `rotation` register;
  tests including the wrap at 180 and rotation by a delta. (1, 2)
- [x] 3. The readers move to it: typed angle prefill, start value and typed
  target in `TransformEntry`, the rotate drag readout in
  `session/select_view.rs`; the rotate-entry tests that read the register of a
  polygon or star are rewritten to the shown angle. (1, 7)
- [x] 4. `rotate_delta_for`: Ctrl rotate of a polygon or star snaps the shown
  angle (absolute); other kinds keep the relative rule; preview and release both
  go through `TransformDrag::resolve`. (7)
- [x] 5. Polygon/Star create-drag takes `Modifiers`; one `created_frame`
  computation for preview and release; Ctrl snaps; the readout carries the
  angle ("r 12.0 mm, -15°"). Session tests: A = (100, 50), B = (110, 48), Ctrl
  gives first vertex (109.85, 47.36), radius 10.20 mm; edge-on-axis table;
  an old project opens unchanged. (3, 4, 5, 6, 8, 2)
- [x] 6. Pure-move prelude: `escape` and `delete_selected` from `session/mod.rs`
  into `session/keys.rs`. (no behaviour change)
- [x] 7. `SelectTool::open_entry_for_key` for `EntryKey::{Angle, Size}` with
  `shift = false`: the same entry as the double-click route, on the top-right
  corner rotate handle (R) and the bottom-right corner resize handle (S). Both
  corner handles exist at any box size (`transform_handles` always lists them),
  so the stored `entry_anchor` the ADR schedules for PR 3 is not needed for R
  and S; PR 3 adds it for M and K. (57)
- [x] 8. `decide(input, KeyState) -> KeyAction` and the gate (criterion 55) in
  `session/keys.rs`, table-driven tests first; `Session::key_down` for B, N, E,
  `*`, R, S, Delete, Backspace, Enter, Escape. M and K are ignored (no hint)
  until PR 3 gives them a chip. (54, 55, 57, 60, 61)
- [x] 9. `wasm_keys.rs` (`key_down`, `pointer_cancelled`, `selection_count`); the
  frontend `onKeyDown` forwards key, modifiers, repeat and one `dom_blocked`, the
  letter `switch` is deleted. (55)
- [x] 10. `NodeTool::cancel_drag` and `clear_selection` replace `NodeTool::escape`;
  `Session::escape() -> EscapeStep`, `button_down`, `pointer_cancelled`; the
  cascade of criterion 42 for every tool; the Node-tool Escape tests are
  rewritten to the two-step behaviour. (42 to 49, 60)
- [x] 11. Split selects one node (the new second one); hit-test tie goes to the
  selected node; selected node glyphs drawn on top; the `split_selected_on_*`
  tests and the session test are rewritten with stronger assertions. (50, 51, 52)
- [x] 12. Rail tooltips and `selection_count`; transient hint message for
  "Select one object to type a value"; the hint lines of the rotate and resize
  handles gain "or R" and "or S". (62, and the R and S part of 24)
- [ ] 13. Gate: fmt, clippy (host and wasm32 per core crate and editor-wasm),
  nextest, rustdoc, deny, banned-dependency check, `npm run build`, `tsc -b
  --noEmit`, `npm run lint`, license check, `npm audit`; the CI result of the
  head commit; the Browser-pane check of the UI.

### Validation

Everything with a rule is in Rust and tested there: `orientation` and
`rotate_delta_for` as pure functions, the create-drag golden numbers, the key
table as one parameterised test over (tool, selection, key, shift) plus every
gate condition on its own and in pairs, the Escape cascade once per tool, Split
on an open and a closed path. The frontend has no test runner
(`docs/technical-debt.md`); the forwarder, the tooltips and the hint chip are
checked in the Browser pane against a worktree build and by the `ux-engineer`
review.

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
- [x] 2. `fit_dashes(edge_px) -> Option<DashFit>` with its table tests first:
  every length 10 to 400 px in 0.25 steps (dash at both ends, symmetric, gap 2
  to 4 except in the two flex bands, dash at most 4 and at least 2.5), edges
  under 10 px solid, the two bands 12 to 16 and 20 to 22 (AC 64).
- [x] 3. The dashed selection box: each edge laid out from its first corner in
  the box's own frame, corners closed, solid hover box, marquee untouched
  (AC 63, 64, 66, 67). Tests: rigid under translation and zoom, rotated boxes
  follow their edges, multi-selection has one box each, equal inputs give equal
  draw lists.
- [x] 4. The pixel snap and `SelectDecorationInput::device_pixel_ratio` for
  axis-aligned selection and hover boxes (AC 65, 66). Tests: at most 0.5 device
  pixels of displacement; one whole device row or column of coverage at ratios
  1, 1.25, 1.5, 2 and 3; a rotated box is not snapped.
- [x] 5. Skew guide constants 2 on / 2 off; `the_skew_guide_draws_as_dashes`
  rewritten to the exact count of eighteen dashes at 70 px (AC 68).
- [x] 6. `Session::set_device_pixel_ratio`, called from `attach_canvas` and
  `resize` (AC 65). Session test: the input carries the ratio.
- [x] 7. Browser pixel readback (not a CI test): axis-aligned box rows at DPR 1,
  1.5 and 2, zoom 100 and 200 percent, a rotated box, the skew guide (AC 63 to 68).
  Done in the Browser pane with `devicePixelRatio` overridden; the skew guide and
  the rotated box were only looked at, not measured.
- [x] 8. `docs/design-system.md` rows (skew guide, selection box, hover box).
- [x] 9. UX review fix (criterion 68): the skew guide is pixel-snapped like the box
  (`snap_guide_line`; a rotated guide stays anti-aliased at 1 px) and the box leaves
  its own dashes off the edge the guide covers (`SelectDecorationInput::skew_guide`,
  cut to the guide's extent; a Shift centre line cuts nothing).

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
- Review decision (coordinator, after the tester's finding): a sub-pixel pan of a
  pixel-snapped box re-fits the dashes when its snapped pixel length changes by
  one (criteria 63 and 65 pull apart; accepted). Whole-pixel translation and
  zoom of the same snapped size stay rigid. The tester's formerly ignored test
  asserts exactly that. The four refinements are noted in `adrs.md` decision 7.

### Validation

Unit tests in `vecmanf-render-core` (pure functions and draw lists), one session
test in `vecmanf-editor-wasm`, the pixel readback in the browser pane, and the
whole CI gate (`.github/workflows/ci.yml`) on the PR head.

## PR 3: `story/edit-polish-typed-skew-move`

Part B (typed skew and typed move) and the keys M, K and Shift+K of Part F.
Criteria 9 to 25 except the Copy check of 23 (PR 4), and 54 (M, K), 56, 58, 59.
Plus, as the first commit, the customer change request on PR 1 (2026-10-07):
the key S scales the typed size about the box centre (criteria 57, 57a).
Worktree `/home/marc/workbench/vecmanf-claude/edit-polish-3`, from
`origin/main` at `6d2eaed`. No new crate, no new dependency, no document
change, no `format_version` change.

### Affected crates and modules

- `vecmanf-ui-core`: new `skew_entry.rs`, `move_entry.rs`; `object_bounds.rs`
  (`object_outline_bounds`), `select_tool/handles.rs` (`entry_anchor`),
  `select_tool/entry.rs` (`OpenEntry::{Skew, Move}`, the centre-handle and
  skew double-click, `open_entry_for_key` for M, K, Shift+K),
  `select_tool.rs` (the centre press is recorded, `Ignored` deleted),
  `transform_commit.rs` (`commit_move`), `transform_entry.rs` (two
  `InvalidReason`s, `EntryKind::Skew`).
- `vecmanf-editor-wasm`: `session/keys.rs` (M, K, Shift+K, two `KeyHint`s),
  `session/transform_entry.rs` (skew view, anchor from the box),
  new `session/move_entry.rs`, new `wasm_move_entry.rs` (the outcome codes
  moved there so `wasm_api.rs` shrinks), `session/select_view.rs` (`skew-y`
  hint).
- `frontend/`: new `MoveEntryChip.tsx`, `TransformEntryChip.tsx` (skew kind and
  two messages), `HandleHintChip.tsx` (hint lines), `useEditorSession.ts`,
  `Canvas.tsx`, `readoutPlacement.ts` (`placeMoveChip`).
- Docs: `docs/design-system.md` rows (S about the centre, M, K, hint lines).

### Tasks

- [x] 0. Change request: the key S scales about the box centre (the typed
  size is the one a Shift drag of the bottom-right handle resolves, the pivot
  marker shows at the centre, the fields stay independent); the double-click on
  a resize handle keeps the dragged handle's fixed point. Tests: unit test of
  the rectangle example of criterion 57 (40 x 20 at (10, 10) typed 60 x 30
  ends at (0, 5)), the pivot of both routes, a session test for the marker;
  no earlier test pinned the old S fixed point. Design-system rows. (57, 57a,
  59)
- [x] 1. `object_outline_bounds`: the tight bounds of the drawn outline (path:
  curve extrema; primitive: its rotated outline). (21)
- [x] 2. `entry_anchor(box, handle, tolerances)`: the chip anchor from the box,
  not from the drawn handle set; the three existing transform entries read it
  in `Session::transform_entry`, so R, S and K open on a box too small to draw
  the handle. (17, 56, 58, 59)
- [x] 3. `SkewEntry`: one field "Skew angle x" or "y", prefill "0", calls the
  drag's `skew_by_angle`; `SkewRange` and `TooLarge`; a flat path's field is
  read-only; the pivot marker. Tests: the worked example of 10, entry equals a
  drag for every side at rotation 0 and 30 with and without Shift, skew then
  negated skew restores the path (13), refusals (11), no-ops (12). (9 to 14)
- [x] 4. `MoveEntry` and `commit_move`: relative offset, absolute against the
  tight top-left, untouched field means no change on that axis, the mode and
  the (later) Copy check arrive with the commit. The drag's release uses the
  same `commit_move`. (18 to 22, 25)
- [x] 5. Double-click routing: the drawn centre handle opens the typed move
  (the press on it is recorded; where it is not drawn the old rule holds), a
  skew handle opens the skew entry, `SelectDoubleClickOutcome::Ignored` is
  deleted. (9, 15, 16, 17)
- [x] 6. `open_entry_for_key` for M, K and Shift+K; `Session::key_down` binds
  them; `Hint(SelectFirst)` (nothing selected, or M and K outside the Select
  tool) and `Hint(PathOnly)` (K on a non-path) join `Hint(SelectOne)`. The
  Shift-revealed side rotate handles stay hidden while a skew, move or
  parameter chip is open. (54, 56, 58, 59)
- [x] 7. Session and wasm surface: the skew chip reuses the transform entry view
  with kind "skew"; `move_entry()` and `commit_move_entry()` in a second
  `impl WasmSession` block; the outcome codes gain `skew-range` and
  `too-large`. `handle_hint()` reports `skew-y` for the left and right skew
  handles. (9, 11, 18, 24)
- [x] 8. Frontend: `MoveEntryChip` (X, Tab, Y, Tab, the Relative | Absolute
  switch with `role="switch"`, Space and arrows flip it, Enter applies,
  Escape cancels, untouched fields follow the mode), the skew chip, the hint
  lines "Double-click or M / K / Shift+K", the key hint texts. No Copy check is
  drawn (PR 4). (18, 24, 59)
- [x] 9. Tests that pinned superseded behaviour are rewritten with an equal or
  stronger assertion: skew-handle double-click (was ignored, now the skew
  entry), centre-handle double-click (was handoff or hint, now the typed
  move), the `skew` hint (now `skew` for top and bottom, `skew-y` for left
  and right; negative assertions use `starts_with("skew")` so they do not get
  weaker), the "M and K are unbound" assertions.
- [x] 10. Gate: fmt, clippy (host, wasm32 per core crate and editor-wasm),
  nextest, rustdoc, deny, banned-dependency check, `npm run build`,
  `tsc -b --noEmit`, `npm run lint` (5 warnings, the same as `main`), license
  check, `npm audit`; the Browser-pane check (M relative and absolute, K, the
  double-click on the centre and skew handles, S about the centre, the hint
  chips, the Tab loop of the move chip); the CI result of the head commit is
  recorded in the PR.

### Decisions taken here (inside the ADR)

- The chip anchor is computed by `entry_anchor` when the view is built from the
  entry's start box, not stored in the entry: the box is fixed for the entry's
  life, so the two are the same, and no field is added to four types.
- `commit_move` has no `copy` parameter in this PR: the Copy check and
  `duplicate_objects` are PR 4's, and a parameter that is always `false` would
  be dead code. PR 4 adds it.
- The hint code of a left or right skew handle is `skew-y` (top and bottom keep
  `skew`), so the hint chip can say "Shift+K" for those two.
- The move chip's hint names only "Double-click or M: type an offset" until PR 4
  adds "Shift: keep one axis" and "Ctrl: copy", which do not exist yet.
- `parse_entry_number` accepts U+2212 (the real minus) from review on: one
  function for the angle, size, skew and move chips, so a value copied from the
  PR 4 move readout parses. Exponent notation ("1e3") stays rejected.
- Review decisions (tester notes): in Absolute mode, text typed to exactly the
  one-decimal prefill counts as untouched (no change on that axis); accepted,
  because the prefill is what the field shows. A tiny skew that changes nothing
  by 1e-9 mm is "no change" (no commit, no message), "Too large" only past the
  1e7 mm limit (`skewed_unchecked` tells the two apart). K on a path with no
  height opens a chip whose field is read-only and never writes; a dedicated
  "cannot skew" hint would need a text and a criterion 59 line that the spec
  does not have, so it is left for the customer's review of the demo.

- Review round 2 (UX): the S key's chip is placed by the box centre
  (`TransformEntry::centre_chip`, `EntryView::at_centre`), no handle is
  highlighted and the centre glyph yields to the pivot marker, drawn at full
  `--accent` (`pivot_marker_full`). The corner resize hints gain a second line;
  the skew chip shows "Skew x" / "Skew y" inside a 120 px field; the Absolute
  switch has a title and a description; the pill ring is a 1.5 px shadow
  because a 1.5 px border computes to 1 px at DPR 1.

### Validation

Every rule is in Rust and tested there: `SkewEntry` against a drag, `MoveEntry`
in both modes, the key table, the double-click routing at small and large box
sizes, the session round trip. The frontend has no test runner
(`docs/technical-debt.md`); the chips are checked in the Browser pane against a
worktree build.

## PR 4: `story/edit-polish-move-copy-lock`

Part C (move modifiers: Shift axis lock, Ctrl copy, their indicators) and the
Copy check of criterion 23. Criteria 23 (Copy check), 26 to 41. Worktree
`/home/marc/workbench/vecmanf-claude/edit-polish-4`, branch
`story/edit-polish-move-copy-lock`, **stacked on PR 3**
(`story/edit-polish-typed-skew-move`, #45): both edit `select_tool/entry.rs`.
PR 3 review fixes are merged into this branch (merge, no rebase); once #45 is
on `main`, `origin/main` is merged too. No new crate, no new dependency, no
`format_version` change (the duplicate op copies the meta map by key).

### Affected crates and modules

- `vecmanf-document-core`: `objects.rs` (`duplicate_objects`, `CopySource`,
  `ObjectEditError::AnchorIds`).
- `vecmanf-ui-core`: new `select_tool/press.rs` (`classify_press`), new
  `select_tool/move_drag.rs` (`MoveDrag::resolve`); `select_tool.rs` (the move
  arms and the press classification leave it), `select_tool/preview.rs`
  (`LiveEdit.copy`, `live_offset` folded into `resolve`), `select_tool/entry.rs`
  (Ctrl at the second press), `transform_commit.rs` (`commit_move(copy,
  minter)`), `move_entry.rs` (Copy), `transform_entry.rs` (U+2212).
- `vecmanf-render-core`: new `move_axes.rs` (`MoveAxes`, drawn before the blue
  outline), `theme.rs` (two tokens).
- `vecmanf-editor-wasm`: `session/select.rs` (modifiers into the drag,
  minter), `session/select_view.rs` (move readout, `live_objects_in` copy
  rule, handle hint), `session/draw.rs` (axes), new `session/move_indicators.rs`,
  new `wasm_move.rs` (`move_indicators`), `session/move_entry.rs` and
  `wasm_move_entry.rs` (`copy`, `copy_preset`).
- `frontend/`: new `MoveBadges.tsx`, `MoveEntryChip.tsx` (Copy check),
  `HandleHintChip.tsx` (the two new lines), `useEditorSession.ts`, `Canvas.tsx`.

### Tasks

- [x] 1. `Document::duplicate_objects(&[CopySource], Vec2)`: one commit, every
  key of the source meta map copied, caller-minted `AnchorId`s, the copy
  directly above its original, `AnchorIds` and `NoSuchObject` refusals that
  write nothing. Tests first: copy equals original except ids and offset for
  every kind, preview equals commit, z-order A, A', B, B', fresh unique ids and
  a join with the copy, refusals, one commit, save and reopen, a merge with a
  concurrent move. (34, 35)
- [x] 2. `classify_press`: pure extraction from `pointer_down`
  (`PressTarget`), behaviour unchanged, the existing tests stay green. (37)
- [x] 3. `MoveDrag` and `resolve`: pure move of the move arms out of
  `select_tool.rs` and `preview.rs` first (`live_offset` is folded into
  `resolve`), then the modifiers: the Shift lock re-chosen on every event
  without a latch, Ctrl copy read live, `pointer_moved(point, modifiers,
  selection)`. The worked example of criterion 30 is a test. (26, 28 to 32)
- [x] 4. Shift at the press: the press does not toggle; the object joins the
  selection when the drag leaves the dead zone; a release inside the dead zone
  toggles; the centre handle never toggles. The slice 4 tests that asserted
  the toggle at the press are rewritten to the release. (29, 38)
- [x] 5. `commit_move(document, ids, offset, copy, minter)`: a drag, a Ctrl
  release and the typed move share it; the selection after a copy is the
  copies; a zero offset (also with Ctrl) writes nothing. (33, 34, 35, 36)
- [x] 6. `LiveEdit.copy`: in copy mode the blue outline travels alone and the
  box and handles stay on the originals (`live_objects_in`); the centre handle
  keeps its dragging look at the original. (33)
- [x] 7. render-core `MoveAxes` (two full-viewport lines through the start
  centre, `--axis-guide` / `--axis-guide-idle`), the move readout ("\u{394}
  12.5, \u{2212}3.0 mm", " Copy"), `Session::move_indicators`
  (`MoveIndicators { copy_badge, lock }`, a pure read of the cached modifiers,
  the hover position, `classify_press` and the drag) and `wasm_move.rs`. A
  property test: `badge_shows == (pointer_down with Ctrl begins a move)` over a
  grid of points. (26, 27, 33, 37)
- [x] 8. The typed move's Copy check: `MoveEntry::commit(.., copy)` through
  `commit_move`, `copy_preset` from Ctrl at the second press, the selection is
  the copy after a typed copy (flag 5), `parse_entry_number` accepts U+2212
  (flag 6), the centre handle hint gains "Shift: keep one axis" and "Ctrl:
  copy". (23, 24)
- [x] 9. Frontend: `MoveBadges.tsx` (plus and lock badge from window key events
  and every pointer event, gone in the frame the key is released), the Copy
  check as the last Tab stop of the move chip, hint lines. (23, 26, 27, 33)
- [x] 10. The 200-object preview benchmark (`#[ignore]`, release) and the
  measured commit time, noted for the PR and the demo. (41)
- [ ] 11. Gate: fmt, clippy (host and wasm32 per core crate, editor-wasm in CI
  form), nextest, rustdoc, deny, banned-dependency check, `npm run build`,
  `tsc -b --noEmit`, `npm run lint`, license check, `npm audit`; CI of the head
  sha. The Browser-pane checks are listed for a reviewer (the Browser pane was
  not used by this PR's implementer).

### Measured (criterion 41, flag 3)

Release build, 200 selected objects (100 paths of 50 nodes, 100 rectangles), on a
machine that was busy with other builds (the same run of the existing move
benchmark gives "at rest" 18 ms per frame):

| | per `draw_list()` | 8 ms budget |
|---|---|---|
| copy preview (`a_200_object_copy_previews_within_the_frame_budget...`) | 10.9 ms | missed |
| plain move preview (`unified_object_editing`, same machine, same minute) | 10.8 ms | missed |

The copy preview costs the same as the move preview, so criterion 41's first
clause holds as far as it is "within the budget of `unified-object-editing`
criterion 15" (that budget is missed by both on this machine; the 20 ms
(50 fps) gate the benchmarks assert is met). The cost is the existing full
rebuild per frame plus 200 dashed boxes, not the copy. The commit of the 200
copies (about 30,000 keys, 200 tree nodes) took **79 ms** in a native release
build; in WebAssembly it will be slower. It writes once, on release.

### Review notes (PR 4)

- The radius knobs disappear during any move drag, copy included: the existing
  rule "parameter handles are not drawn during other drags".
- Preview 11 ms against the 8 ms budget equals a plain move; the commit of 200
  copies took 78 ms native. The browser (WASM) commit time is unmeasured.
- Fixed after review: the release applies `joins` itself (a release with no
  preceding `pointer_moved` joined nothing); `pointer_down` shrank by moving the
  Object arm and the handle begin into `press.rs`.

### Decisions taken here (inside the ADR)

- The origin axes are their own render-core call (`build_move_axes`), not a
  field of `SelectDecorationInput`: that list is drawn after the blue outline,
  the axes must lie under it. Noted in `adrs.md` decision 5 with four other
  refinements (the axes' start centre, the `commit_move` and `pointer_up`
  signatures, `classify_press` public, `live_offset` deleted).
- A press on the drawn centre handle starts a move whatever the Shift state
  (criterion 38). Before this PR a Shift press there fell through to the
  outline hit and toggled.
- `parse_entry_number` already accepted U+2212 (PR 3 review fix), so flag 6
  needed no change here beyond the test over the move readout's minus.
- The Shift-revealed side rotate handles stay frozen as they were at the press
  during a move (`side_rotate_revealed`, unchanged): a Shift press on an
  object's outline can now start a move, so those four glyphs can show on the
  travelling box. Not one of the spec's criteria; noted for the UX review.
- `acceptance_unified_editing`'s `ac13_preview_equals_release_*` sweeps pressed
  the move handles with Ctrl at random and asserted the original moved; a move
  released with Ctrl is now a copy, so the sweep compares the blue preview with
  the copy (index 1, directly above its original). Same assertion on the
  committed outline, one more case covered.

### Validation

Every rule is in Rust and tested there: `duplicate_objects` against the
original, `MoveDrag::resolve` as a table (criterion 30 step by step),
`classify_press` against `pointer_down`, the badge against the press, the
Escape-then-Ctrl case, the copy commit. The frontend has no test runner
(`docs/technical-debt.md`); the badges, the Copy check and the draw order of the
axes need a reviewer in the Browser pane.
