# ADRs for "Object transform refinements: handles, pivots, numeric entry, 22.5° snap"

This feature adds no stored field. Every write goes to a register slice 5
already writes (`rotation`, the primitive frame, `corner_radius`,
`stroke_width`, path anchors and handle vectors), through the commands
slice 5 already has. Handles, Shift reveal, the entry chip and its state are
ephemeral UI state. **No new crate, no new external dependency, no new
`vecmanf-geometry-core` function, no ADR amendment, no `format_version`
change** (it stays at `main`'s 5). Skew (Part B) is baked into path anchors
the way rotation is, so ADR 0002 §5's per-node affine is still not needed.

Four criteria are unbuildable as written or contradict slice 5 as merged.
They are under "Flagged to the lead", each with the default this file builds
against. The UX notes (complete, 2026-10-06) are taken as given; where they
already list a criterion change ("Changes the criteria need"), it is not
repeated here.

## Depends on

- [ADR 0001 §1, §4](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  handle layout, hit test, snapping, the entry's parsing, validation, field
  linking and resolution live in `vecmanf-ui-core`; the entry chip is a DOM
  text overlay (allowed for text input); the frontend renders and forwards.
- [ADR 0001 §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  the entry crosses the wasm boundary as small declared messages (strings and
  scalars), never as a serialized document.
- [ADR 0002 §3](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  angles are `Angle` (radians); degrees exist only in readouts, the entry
  text and the snap table's definition. Every comparison takes an explicit
  tolerance (see the snap and skew notes).
- [ADR 0002 §5, §6](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  skew is baked into path anchors; primitives are never converted implicitly
  (criteria 50-51, `primitive-shapes` criterion 21).
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one drag or one confirmed entry is one commit; no movement, no change, or
  untouched entry text writes nothing.
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md):
  entry state, Shift reveal and hover are ephemeral; merge granularity per
  register is slice 5's table, unchanged.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  no new key and no new meaning of an existing key, so no version bump.
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md): every
  piece fits an existing crate over an existing edge.
- [`specs/0005-object-transform/adrs.md`](../0005-object-transform/adrs.md):
  `rotation` semantics, `OrientedBox`, "preview and commit share one
  implementation", the merge-granularity table, `StrokeScaling` captured at
  press, the four resize commands with `stroke_width: Option<Length>`.
- [`specs/0004-canvas-navigation-and-selection/adrs.md`](../0004-canvas-navigation-and-selection/adrs.md):
  the host detects a double-click and calls `double_click`; `Session`
  dispatches it; `tool_for` is the one handoff mapping.

## Feature-local decisions

- **2026-10-06: the crate boundary per criterion group.**

  | Criteria | `document-core` | `ui-core` | `render-core` | `editor-wasm` / `frontend/` |
  |---|---|---|---|---|
  | 1-4 center handle | none | layout position, hover-only hit (UX: not a press target), 48 px tier as a mm threshold passed in | move glyph | cursor `move`, hint chip |
  | 5-11 rotate handles | none | 8 rotate positions, Shift-dependent set, one nearest-center hit test | glyph kind | modifier forwarding (below) |
  | 12-17 pivot rule | none | `rotate_pivot(box, grabbed, shift)` | pivot marker (exists) | none |
  | 18-32 numeric entry | none | entry state, parser, linking, target-to-delta, resolution through the drag functions | handle in dragging look, pivot marker | binding; DOM chip |
  | 33-36 snap | none | `snap_angle` | none | readout (exists) |
  | 37-53 skew | `PathSnapshot::sheared` | skew handles, skew arithmetic, `compute_skew` | skew glyph, fixed-line guide | binding, cursor, readout |

  `document-core` gains one public method and nothing else; the commit goes
  through the existing `Document::resize_path`.

- **2026-10-06: one resolving function per gesture, shared by preview,
  release and typed entry.** Slice 5's rule is extended to entry. Options:
  (A) the entry converts the typed target into the same inputs a drag
  produces and calls the same function; chosen. (B) A separate
  `resize_to(width, height)` / `rotate_to(angle)` that computes the frame
  directly. Rejected: it is a second implementation of the fixed-point rule
  (`resize_anchor_local_position`, `pin_resize_anchor`, the polygon/star
  center rule, the corner-radius and stroke factors), and criteria 16 and 27
  ("never differ") would then hold only as long as two code paths agree.
  Concretely:
  - `compute_resize` is split at the pointer: `local_delta_of(start_box,
    down_at, current)` and `resize_by_local_delta(start, start_box,
    direction, local_delta, options)`. The drag calls both; the entry calls
    `local_delta_for_size(start_box, direction, target, shift)` and then the
    same `resize_by_local_delta`. The inverse of `resized_extent` per
    touched axis: without Shift `sign · (target − start_extent)`, with Shift
    half that. Under Ctrl (linked, criterion 29) only the last edited field's
    axis gets a delta and the other is 0, so the existing dominant-axis rule
    picks the edited field's factor exactly as a drag along that axis would.
    Polygon/star: the delta is the corner's diagonal unit times
    `√2 · (r_target − r_start)`, which `polygon_star_resize_factor` maps
    back to exactly `r_target`.
  - `compute_rotate` is split the same way: `rotate_by(start, pivot,
    delta)` is `start.rotated(pivot, delta)` behind `sane_or`. The drag's
    delta comes from the pointer (snapped under Ctrl); the entry's is
    `normalized(target − start.rotation())`.
  - Commit: `commit_resize` and `Document::rotate_object`, unchanged, for
    drag and entry alike.
  - Test: for each kind and each handle and modifier combination, a drag to
    the pointer that yields size S (or angle A) and an entry of S (A) leave
    snapshots equal within 1e-9 mm and 1e-12 rad.
  - In `select_tool`, the per-variant arms of `live_resize`, `live_rotate`
    and `pointer_up` collapse into one `resolve(&drag, current, modifiers)`
    in `transform_drag`, called by the preview and by the release. That makes
    the rule structural rather than a convention.

- **2026-10-06: numeric entry, where the state lives.** Options:
  (A) `SelectTool` holds `entry: Option<TransformEntry>` (object id,
  start snapshot and box, grabbed handle, the pivot or fixed-point rule
  fixed at open, `ctrl` link, `StrokeScaling` read at open, and the prefill
  texts); the frontend holds only the DOM input's text, caret and focus.
  Chosen. (B) The frontend holds everything and calls a stateless
  `rotate_selected_to(deg, pivot)`. Rejected for slice 5's reason (two
  owners): the pivot marker is drawn from `TransformDecorationInput`, which
  only `Session` builds; closing on selection change or tool switch is
  `Session`'s knowledge; and the criteria's tests (21, 27, 30) would not
  reach a TypeScript-only rule.
  - **Module:** `vecmanf-ui-core/src/transform_entry.rs` (the entry type,
    the parser, linking, target-to-delta). `select_tool` only opens, closes
    and forwards. `Session` glue goes in a new
    `vecmanf-editor-wasm/src/session/transform_entry.rs`, not in
    `session/select.rs`.
  - **Parser (criteria 19, 21, 30):** `parse_entry_number(text, allow_degree)
    -> Option<f64>`. Trim; optional leading sign; digits with at most one
    separator, `.` or `,`; on the angle field, one optional trailing `°`.
    Anything else (empty, letters, `1,2,3`, exponents, unit suffixes) is
    `None`. The result must be finite. Size fields also require `> 0` and
    at most `MAX_COORDINATE_MM` (the 10 km sanity bound in
    `transform_drag`); a value beyond it is reported invalid rather than
    silently dropped by `sane_or`. Angles accept any finite value and are
    normalized by `rotated`.
  - **Outcome:** `commit` returns `Committed`, `Unchanged`, or
    `Invalid { field, reason: NotANumber | NotPositive }`; the frontend maps
    the reason to the UX notes' two messages and keeps the chip open.
  - **Unchanged (criteria 19, 31; UX change 4):** a field whose text equals
    its prefill text writes nothing, and so does a parsed target within
    1e-9 mm (size) or 1e-12 rad (angle) of the current value. Both rules are
    needed because the prefill is the readout's rounded value.
  - **Linked fields (criterion 29):** the other field's text comes from
    `ui-core` (`linked_text(field, text)`), so the ratio rule exists once.
  - **Close without writing:** `Session` closes the entry in `set_tool`,
    any `pointer_down`, `escape`, `delete_selected`, `convert_selected_to_paths`,
    and on a new or opened session; the frontend calls `cancel` on Escape,
    blur and window blur. Closing is idempotent. The press that closes it
    is processed normally (UX notes).
  - **Unit:** millimetres, the unit the readout already uses. No display
    unit exists yet; when `document-size-and-rulers` adds one, the entry
    and the readout switch together.
  - **Zero-extent axis of a path:** a path whose box has zero extent on an
    axis (a horizontal line has no height) cannot be scaled on that axis by
    a drag either (`safe_factor` returns 1). That field is reported
    read-only, so it is never an invalid-looking dead input.
  - **wasm surface:** `double_click(x, y, shift, ctrl)`; `transform_entry()
    -> Option<TransformEntryView>` (kind angle/size/radius, up to two fields
    with label, prefill text and editable flag, linked flag, handle position
    in screen px); `transform_entry_linked(field, text) -> String`;
    `commit_transform_entry(first, second, last_edited) -> outcome code`;
    `cancel_transform_entry()`. Strings and scalars only.

- **2026-10-06: double-click on a handle.** `Session::select_double_click`
  asks `SelectTool::handle_at` first, at the **second press's** position and
  with its modifiers. The frontend stores both at the suppressed press and
  passes them to `double_click` (today it passes the release position and no
  modifiers).

  | Hit at the second press | Effect | Criterion |
  |---|---|---|
  | rotate handle (corner, or side with Shift) | angle entry; pivot = opposite point with Shift, else center | 18, 22, 23 |
  | resize handle | size entry; fixed point as the drag (Shift = center); Ctrl links W/H on a corner (not polygon/star) | 25-29, 32 |
  | skew handle | nothing; no handoff | 49 |
  | inside the sole selected object's box, incl. the center handle | handoff to the selected object's own tool (`tool_for`) | 3 (see flag 2) |
  | anything else | slice 4's `hit_test_object` handoff, unchanged | — |

  The first click of the double-click is an ordinary press and release on
  the handle. Under the dead zone below it writes nothing.

- **2026-10-06: a 3 px dead zone for every Select-tool drag on the selected
  object.** A move (body or center handle), resize, rotate or skew writes and
  previews nothing until the pointer has left a 3 screen-px radius around the
  press. Past it, the delta is computed from the original press point, so the
  handle still follows 1:1. Reason: slice 5 tests `down_at == point`
  exactly, so a double-click with 1 px of jitter would commit a sub-pixel
  resize and then open the entry on the changed value (criterion 18: "the
  first press and release writes nothing"). The constant is
  `PEN_DRAG_THRESHOLD_PX` renamed to `DRAG_THRESHOLD_PX`, the rename
  `specs/advanced-selection/adrs.md` already plans for its marquee; whichever
  slice comes first renames it. The frontend's 5 px double-click distance
  stays as it is; a first click moved 3 to 5 px is a real drag. See flag 4.

- **2026-10-06: handle set, hit test and the Shift reveal.**
  - `TransformHandle` becomes `Resize(ResizeDirection)`,
    `Rotate(ResizeDirection)` (corners = corner handles, edges = side
    handles; all eight values valid), `Skew(Side)` with a new 4-variant
    `Side` (no invalid `Skew(Ne)`), and `Move`. Positions are computed in
    the box's local frame from screen-px offsets converted to mm by the
    caller (slice 5's convention); `TransformHandleTolerances` grows the new
    radii, offsets and the 48 px / 24 px tiers as mm values.
  - One hit-test function serves press, hover, cursor and the hint chip
    (slice 5 rule). Every visible handle that contains the pointer within
    its own radius (resize keeps slice 5's shrinking radius and inner band)
    is a candidate. The nearest center wins; distances equal within 1e-9 mm
    tie-break resize, skew, rotate. `Move` is never a press candidate (UX:
    the center handle is the body); it is returned only by the hover query.
    This replaces slice 5's "rotate first, then resize".
  - **Shift state.** The handle set is a function of (selection, live
    Shift while idle). During a drag the set is frozen as it was at the
    press, which gives criterion 6's "Shift during a drag does not reveal"
    and "a dragged side handle stays" with one rule. Today the frontend
    re-sends hover on Shift/Ctrl only if the pointer has moved over the
    canvas and only while the canvas container has focus. Decided:
    `Session::modifiers_changed(shift, ctrl)` updates the cached modifier
    state and re-runs hover only if a pointer position exists; the frontend
    calls it from window-level `keydown`/`keyup` and with `(false, false)`
    on window blur and canvas blur (UX "Shift reveal"). `advanced-selection`
    later adds `alt` to the same call.
  - Pivot preview while Shift is held over a handle (UX) is
    `rotate_pivot`/`resize_anchor_local_position` evaluated for the hovered
    handle: the same functions the drag uses, no new rule.

- **2026-10-06: pivot rule (criteria 12-17).** `rotate_pivot(box, grabbed:
  ResizeDirection, shift) -> Point`: with Shift, the local position of the
  handle opposite `grabbed` (corner to opposite corner, side to opposite
  side's midpoint) mapped to document space; otherwise the box center. Live
  Shift re-evaluates it every frame from the drag-start box (slice 5
  pattern), so switching pivots mid-drag accumulates no error. Writes per
  kind stay those of slice 5's merge table (a non-center pivot also writes
  the frame or anchors).

- **2026-10-06: the snap stops (criteria 33-36, 47).** New module
  `vecmanf-ui-core/src/angle_snap.rs`, one pure function `snap_angle(raw:
  Angle) -> Angle`. The stops are `{k·15°} ∪ {k·22.5°}`; within each 45°
  period they are 0, 15, 22.5, 30 (+ 45k). Computed on `|raw|` with the
  sign restored, so negative angles mirror positive ones. The nearest stop
  wins; when two distances are equal within 1e-9 rad, the stop with the
  smaller magnitude (nearer the start) wins. The rotate drag applies it to
  the delta, as slice 5 does for its 15° step, then normalizes.
  - Tests: criterion 34's ten values, both signs; every midpoint (7.5,
    18.75, 26.25, 37.5 and the same in each later period) both signs and
    ±1e-6° either side of each; ±180°; 0.
  - **Shared with skew:** yes, one table. The skew applies `snap_angle` to
    α and then caps the result at |α| ≤ 75° (flag 3).
  - Criterion 35 needs no code: `format_degrees` in `session/select.rs`
    already prints one decimal unless the value is whole. Add a test.

- **2026-10-06: skew (Part B) writes anchors only.** Options: (A) bake into
  anchors and handle vectors, `rotation` untouched; chosen by the customer
  (P1) and the only option without a document-model change. (B) A stored
  per-object matrix (P3); deferred by the customer to the primitives rework,
  still recorded in `docs/technical-debt.md` ("Rotation is a stored angle").
  - **Frame.** The oriented box's local frame: document-space unit vectors
    `u = (cos θ, sin θ)`, `v = (−sin θ, cos θ)` (y down), θ = the path's
    `rotation`. Top/bottom handles shear along `u`, left/right along `v`.
  - **Map.** In local coordinates, about the fixed line:
    x skew `L = [[1, k], [0, 1]]` (u' = u + k·(v − v₀)), y skew
    `L = [[1, 0], [k, 1]]` (v' = v + k·(u − u₀)). In document space the
    linear part is `M = R(θ) · L · R(−θ)`. With `q` any document point on
    the fixed line (the fixed edge's midpoint, or the center under Shift):
    anchor `p' = q + M(p − q)`, handle vector `h' = M·h` (relative to its
    anchor, so not translated). An affine map sends a cubic's control points
    to the control points of its image, so every Bézier stays exact; it
    keeps collinearity and ratios along a line, so Symmetric and Asymmetric
    anchors keep their kind.
  - **Factor.** `k = d / (c_g − c_0)`, where `d` is the pointer's local
    displacement along the side (Δu for top/bottom, Δv for left/right),
    `c_g` the grabbed side's local coordinate and `c_0` the fixed line's
    (opposite side, or center under Shift), all from the drag-start box.
    Readout and snap use `α = atan(d / |c_g − c_0|)`; under Ctrl,
    `d = |c_g − c_0| · tan(α_snapped)`. `|c_g − c_0|` below 1e-6 mm
    resolves to "no change" (the UX hides such handles at 24 px anyway). A
    `k` of exactly 0 returns the start snapshot unchanged, because
    `R(θ)·R(−θ)` is not bit-exact and must not produce a commit for a
    drag that returned to its start.
  - **Where.** `PathSnapshot::sheared(pivot, ku, kv) -> Self` in
    `document-core` next to `scaled` (one of `ku`, `kv` is zero; it uses the
    path's own `rotation` for the frame, as `scaled` does). `compute_skew`
    in `ui-core::transform_drag`, called by preview and release.
  - **Commit.** `Document::resize_path(id, anchors, None)`: anchors and
    handle vectors in one commit; no `rotation`, no `stroke_width`, no new
    key. No new `Document` method.
  - **Exact inverse (criterion 43).** `L(k)·L(−k) = I`. An x skew leaves
    every point's `v` unchanged (Bézier control points included, so the
    curve's tight `v` range too); the box's `v₀`, `c_g − c_0` and θ are
    therefore the same for the second drag, and the same handle with `−d`
    gives `−k`. Test: skew then inverse-skew round-trips anchors and handles
    within 1e-9 mm for coordinates up to 1 m (f64 rounding is about 1e-13
    mm per step), for θ = 0 and a non-axis θ, with and without Shift, for
    open and closed paths with curves.
  - **Primitives (criteria 50-51).** No `Skew` handle exists for a
    primitive, so no press can start a skew on one (UX: hidden).
    `compute_skew` additionally returns a primitive unchanged, so the rule
    does not depend on the layout alone.

- **2026-10-06: split the oversized modules first, as a pure move.** Plan
  task 1, its own commit, before any new code: no behaviour change, no public
  API change (`wasm_api.rs` untouched), all existing tests unchanged and
  green. Whichever of this feature, `0007` and `advanced-selection` starts
  first does it (`docs/technical-debt.md`, "`Session` is one module past the
  size limit").
  - `ui-core/src/transform_handle_layout.rs` (546 non-test lines) keeps
    layout, hit test and cursor angle. The arithmetic (`ResizedBox`,
    `resize_local_box` and helpers, `resize_anchor_local_position`,
    `polygon_star_resize_factor`, `stroke_or_radius_factor`,
    `scaled_and_floored`, `rotate_pivot`, `rotate_delta_angle`) moves to a
    new `transform_math.rs`: "pure resize and rotate arithmetic in an
    oriented box's local frame".
  - `editor-wasm/src/session/mod.rs` (837 non-test lines): Node tool glue
    (`live_node_drag_paths`, `apply_live_node_drag`, the convert, line,
    curve, join, split and insert actions, `node_toolbar_state`,
    `decoration_input`) to `session/node.rs`; Pen glue (`finish_pen`,
    `pen_in_progress`, `is_hovering_pen_close_target`, `drag_threshold`)
    to `session/pen.rs`; `select_live_offset`/`select_live_transform` to
    `session/select.rs`; `draw_list` assembly to `session/draw.rs`. Target:
    `mod.rs` under 500 non-test lines.
  - Then, with the new code: `angle_snap.rs` and `transform_entry.rs` are
    new modules; skew arithmetic goes to `transform_math.rs`. `select_tool.rs`
    (536 non-test lines today) must not grow: the `resolve` collapse above
    removes more than the new `Skewing` variant and entry forwarding add. If
    `transform_drag.rs` (413) passes 500, `commit_resize` and the new commit
    dispatch move to `transform_commit.rs`.

- **2026-10-06: `format_version` stays 5.** Every write uses an existing key
  with its existing meaning: rotate (`rotation`, frame or anchors), resize
  (frame, `corner_radius`, `stroke_width` only with the switch on, anchors),
  skew (anchors). A version-5 reader reads every document this feature
  writes correctly. No migration, no fixture change.

- **2026-10-06: sequencing.** Serial, in this order: **this feature, then
  `0007-stroke-and-fill-styling`, then `advanced-selection`.**
  - This feature and `advanced-selection` both rework `select_tool`'s press
    dispatch and the modifier path. The handle hit test is the layer that
    runs first on every press; building it first lets `advanced-selection`
    call `handle_at` as a finished function before its candidate cycle,
    marquee and lasso, instead of threading eight rotate handles, skew and
    the entry through its new `Pending*`/`Marquee`/`Lasso` state machine.
    Its `Modifiers { shift, ctrl, alt }` then absorbs this feature's
    `modifiers_changed` and `double_click` booleans.
  - `0007` before `advanced-selection`: its flag 2 (candidate order once
    filled interiors hit) is then decided with fills present, so
    `hit_test_objects`' ordering is written once.
  - `0007`'s overlap with this feature is small: `commit_resize` and
    `scale_stroke` (the stroke field), and nothing in the handle code.
  - Fallback: if this spec is not Ready when an implementer slot frees,
    `0007` goes first. Nothing here changes, except that the stroke-width
    reads below become `style.stroke_width`.

- **2026-10-06: what `0007`'s resize notes need once this lands.** For the
  `0007` implementer; to be copied into `0007/adrs.md` as a dated note when
  this merges.
  - The typed size entry commits through the same `commit_resize`, so
    re-pointing the four resize commands to `style.stroke_width` covers it.
    There is no second call site. Its `StrokeScaling` is read when the chip
    opens.
  - Skew commits through `resize_path(.., None)` and must keep writing no
    style key. Dash lengths are multiples of the width, so they are
    unaffected.
  - The gradient box is the oriented box (0007, 2026-10-06). After a skew it
    is recomputed tight in the same θ frame (criterion 45), so a gradient
    re-fits to the new rectangle and does not shear with the shape. The
    `0007` tester adds one case: a linear-gradient path, skewed 30°.
  - `0007`'s "resize commands refuse a width ≤ 0" is never reached by the
    entry: sizes are refused at ≤ 0 before resolution and the stroke factor
    `√(sx·sy)` is then > 0.

## Flagged to the lead

1. **Criterion 28 contradicts slice 5 for polygon and star.** Slice 5 as
   merged scales a polygon/star about its center for every corner handle,
   with or without Shift (`resize_primitive` keeps the frame center; the
   pivot marker shows the center). Criterion 28's "the opposite corner"
   holds for rectangles, ellipses and paths only. Criterion 27 ("same as a
   hand-drag") is the governing rule. **Default: polygon/star size entry is
   about the center; the PO adds "polygon and star: the center" to
   criterion 28.**
2. **Criterion 3's double-click on the center handle.** "What a
   double-click on the body does today" is nothing for an unfilled object:
   slice 4's `double_click` hits only the outline, while slice 5's
   2026-10-06 note made an interior *press* a move but left the double-click
   alone. The UX notes assume a handoff. **Default: a double-click inside
   the sole selected object's box (center handle included) hands off to its
   tool, matching the press rule. Small change to slice 4/5 behaviour; the
   PO rewords criterion 3.**
3. **Criterion 47 can reach a degenerate shear.** The stop set includes
   ±90°, where `tan α` is infinite; criterion 38 requires |α| < 90°.
   **Default: a Ctrl skew snaps to the nearest stop and is capped at ±75°,
   the last stop below 90°. The PO adds that sentence.**
4. **Criterion 18's "no movement beyond the click tolerance" needs a dead
   zone** that slice 5 does not have (above). **Default: 3 px for every
   Select-tool drag on the selected object. A drag under 3 px no longer
   writes, which is a small change to slice 4/5. The PO adds one sentence.**
5. **Criterion 25's "document's display unit"** does not exist yet.
   Default: mm, as the readout; not a conflict.
6. **For the tester.** Slice 5 tests that press the rotate handle above the
   top edge (`vecmanf-ui-core/tests/acceptance_0005.rs`,
   `vecmanf-editor-wasm/tests/acceptance_0005*.rs`, `acceptance_0004.rs`,
   `select_tool.rs` unit tests) change with open question 2's default: that
   position now holds the Shift-only side handle, so they either hold Shift
   or use a corner handle. Rewrite them as part of this feature; do not
   delete them.
