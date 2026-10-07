# ADRs for Polygon and star: the selection box turns with the shape

**Verdict (architect, 2026-10-07): buildable as one small PR. No stored field,
no new key, no `format_version` change (stays 5), no change in
`curvyo-document-core`, no new crate, trait, generic or dependency, no new ADR.
Nothing here is `needs-customer`** (`CLAUDE.md` §3: no platform, UI framework,
document-model, persistence, plugin, license or account decision; the box size
stays, only its direction changes, and `edit-interaction-polish` criterion 1
already says the box turns with the shape). The spec can be `Ready`.

Reference state: `main` at `6f7bf99`, everything below read in that tree.

## Depends on

- [ADR 0002 §3, §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  angles are `Angle`; the `rotation` register and `StarFrame` stay as they are.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md): no
  new key and no new meaning of a key. Decision 1 is the proof.
- [ADR 0001 §3](../../docs/adr/0001-ui-framework-and-canvas-rendering.md): the
  box and every rule on it stay plain Rust in `ui-core`; the frontend is not
  touched.
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md): every change
  sits in an existing crate over an existing edge.
- [`specs/edit-interaction-polish/adrs.md`](../edit-interaction-polish/adrs.md)
  decision 1: defines the shown angle as `StarFrame.angle + rotation`
  (`ObjectSnapshot::orientation()`) and records the "Known limit" this feature
  removes. Its option (b) stays rejected, see below.
- [`specs/0005-object-transform/adrs.md`](../0005-object-transform/adrs.md): the
  oriented box lives in `curvyo-ui-core`, a primitive's local frame is its
  stored frame. This feature changes that last sentence for polygon and star
  only.

## Feature-local decisions

### 1. The box direction is `orientation()`, derived at the one seam `oriented_bounds` (spec 1 to 6)

- **2026-10-07: `oriented_bounds` (ui-core, `oriented_box.rs`) sets
  `OrientedBox.angle` to `ObjectSnapshot::orientation()` for every primitive.**
  `orientation()` already returns `StarFrame.angle + rotation` (normalized) for
  a polygon or star and the `rotation` register for every other kind, so the
  change is one expression, `angle: primitive.rotation` becomes
  `angle: object.orientation()`, and needs no branch on the kind. `min`, `max`
  (the circumscribed square `C ± (R, R)`) and `pivot` (= C) are as before. A
  path keeps `path.rotation`.
- **The box-local frame of a polygon or star is now the stored frame turned by
  `StarFrame.angle`**: its +x axis points at the first outer vertex. Every
  reader that works in box-local coordinates and assumed "box-local frame =
  stored frame" must stop adding `StarFrame.angle` itself. The audit in
  decision 2 found exactly two.
- **Options considered.**
  - **(b) Fold the created angle into `rotation`**, so the existing readers keep
    working. Two variants, both rejected. *Fold at creation* (new shapes store
    `rotation = θ`, frame angle 0; no version change because both fields exist):
    old files still hold `frame angle ≠ 0`, so the sum, i.e. this option (a),
    is needed anyway, and two representations of one shape live on for good
    (the reason `edit-interaction-polish` rejected its option (c)). *Fold at the
    first rotation* (a rotate writes `star_frame` and `rotation` together): a
    rotate then writes two LWW registers, and a peer's concurrent resize writes
    `star_frame` with the old angle, so one of the two merged values makes the
    shape jump by the created angle (the merge hazard of `edit-interaction-polish`
    decision 1, option (b)); and the box of an old shape stays tilted until its
    first rotation and then jumps.
  - **(c) Keep the stored fields and compute only a display box in a separate
    `object_oriented_box`**, leaving resize, pivot, hit rule and parameter
    handles on the `rotation`-only frame. Rejected: corners are drawn from one
    frame and tested and dragged in another, which is the "second box" the spec
    forbids (criterion 2) and breaks "preview equals commit" at the handles.
    The seam `oriented_bounds` already is the one function every reader goes
    through (`select_view`, `select_tool/handles.rs`, `entry.rs`,
    `param_handles`, `transform_entry`, `param_entry`), so doing (a) there *is*
    the single-function solution the question asked for.
  - **(a) chosen**: no write, no migration, one derived value, the same sum the
    outline and the readout already use.
- **Shrink-wrapped box rejected for this story.** It would put the box centre
  off C for odd N, so the resize pivot (box centre), the centre handle and the
  "corner handle follows the pointer" factor (`polygon_star_resize_factor`
  assumes a corner at `R·√2` on the diagonal) all change. The circumscribed
  square is accepted behaviour.

### 2. Every reader of `rotation` and `StarFrame.angle` (audited on `main`)

**Must change (five sites, all in `curvyo-ui-core` or `curvyo-editor-wasm`):**

| Site | Today | Change |
|---|---|---|
| `ui-core/src/oriented_box.rs`, `oriented_bounds`, primitive arm | `angle: primitive.rotation` | `angle: object.orientation()`. Update the module doc ("a primitive's local frame coincides with its stored frame": true for rectangle and ellipse; for polygon and star the frame turned by its angle). |
| `ui-core/src/param_handles.rs`, `star_inner_vertex` (used only by `param_handles`) | local angle `frame.angle + π/N` | local angle `π/N`: in the turned box-local frame the first outer vertex is at 0. Document position unchanged. |
| `ui-core/src/param_edit.rs`, `value_from_pointer`, `InnerRadius` arm | projects `local_delta` onto `frame.angle + π/N` | project onto `π/N` for the same reason. |
| `editor-wasm/src/session/select_view.rs`, `cursor_hint` | cursor base angle plus `ObjectSnapshot::rotation()` | plus `oriented_bounds(object).angle` (equal to `rotation()` for every kind but polygon and star, so rectangles, ellipses and paths do not change). |
| `ui-core/src/transform_primitive.rs`, `resize_primitive` | `pin_resize_anchor(.., primitive.rotation, ..)` | pass `start_box.angle`. No behaviour change today (a polygon or star scales about its fixed centre, so the pin translates by zero, and a rectangle or ellipse has `box.angle == rotation`); it keeps "one box frame" true for the next reader. |

**Read the box through `oriented_bounds` and need no edit** (their local-frame
arithmetic is frame-agnostic or already uses `box_.angle`, `to_local`,
`to_document`): `select_tool/handles.rs` (`handle_spec_for` keeps four corner
handles and no skew for a polygon or star), `select_tool/entry.rs`,
`select_tool/preview.rs`, `transform_handle_layout.rs` (handle positions,
`is_drawn_handle`, `skew_side_visible`), `transform_drag.rs` (`local_delta_of`,
`pivot_for`, `resolve`: preview and commit both use the `start_box` captured at
the press, so preview equals commit by construction), `transform_math.rs`
(`rotate_pivot`, `polygon_star_resize_factor`, `local_delta_for_radius`: only
the corner's diagonal in the box frame, which is the shape's own symmetry
whatever the angle), `transform_entry.rs` and `param_entry.rs`,
`editor-wasm/session/select_view.rs` (selected and hovered corners, handle
overlay, glyph direction `glyph_kind`), `session/transform_entry.rs` (box
centre = C). `ui-core/src/skew_math.rs`: paths only.

**Read `rotation` or `frame.angle` for the outline or for equality, not for the
box; unchanged on purpose:** `document-core` `outline_of_rotated`, `ObjectSnapshot::rotated`
and `rotate_shape` (the frame centre turns about the pivot, `rotation` advances
by Δ, so `orientation()` advances by Δ and the box follows), `orientation()`
itself; `render-core` `shape_preview.rs` and `live_preview.rs`;
`ui-core` `hit_test_object.rs` (outline hit), `conversion.rs` (outline to path),
`transform_commit.rs::numbers_of` and `same_within_tolerance` (compare stored
numbers, including `frame.angle` and `rotation` separately),
`poly_star_tool.rs::created_frame` and `editor-wasm/session/shapes.rs` lines
160 and 168 (the create-drag writes `frame.angle` with `rotation` 0 and reads it
back for the live readout; this is the creation path and stays),
`select_view.rs` rotate readout (`orientation()`), `transform_entry.rs` angle
prefill and typed target (`orientation()`).

**`object_bounds` (axis-aligned frame bounds, `object_bounds.rs`) is not the
drawn box** and is not changed: for a primitive it is `shape_frame_bounds`, the
axis-aligned frame square, today and after. Only `advanced-selection`'s marquee
will use it, and that feature's decision 4 already says to use the oriented
corners instead; see sequencing.

A hit on a missed reader shows up in the invariant test (decision 4, item 2):
the parameter handle of a star must land on `outline_of_rotated`'s first inner
vertex, and every outline vertex must lie inside the box.

### 3. Format, old files, mixed builds (spec 13, 14)

- **No format change.** Nothing is written or read differently: `rotation`,
  `star_frame` and their codecs are untouched, `format_version` stays 5, no key
  is added. An old file opens with the same outlines and the same shown angle
  (`orientation()` already exists) and its box now turns by that angle. A new
  file is byte-for-byte what `main` writes, so a build from before this change
  opens it with the same shapes (it draws the older, tilted-wrong box; the box
  is not stored).
- **Mixed builds on one session** only differ in the ephemeral box drawing,
  never in stored state (ADR 0009 §3: view state is ephemeral).
- **Writes.** The refit adds no write. A rotate about C still writes `rotation`
  only, a rotate about another pivot writes `rotation` and the frame centre, a
  resize writes the frame radius; none writes `StarFrame.angle`. A test pins
  this (decision 4, item 8).
- **If a later feature wants the angle stored in one place** (the affine of
  `docs/technical-debt.md`, "Rotation is a stored angle"), that migration is
  its own `needs-customer` ADR; this feature does not make it harder, since all
  readers go through `orientation()` and `oriented_bounds`.

### 4. Test plan (tests first; new files, so no test module grows past 500 lines)

New `curvyo-ui-core/tests/polygon_star_box_refit.rs` and
`curvyo-editor-wasm/tests/polygon_star_box_refit.rs`. The proptest dev
dependency already exists in `ui-core`.

1. **Box table** (spec 1, 3, 4). `oriented_bounds` of polygon and star over
   `(frame angle, rotation)` = (0, 0), (30°, 0), (78.7°, 0), (10°, 30°),
   (-90°, 0), (180°, 0), (0, -78.7°): `angle == orientation()` to 1e-12, `min`,
   `max`, `pivot` as `C ∓ R`, `C`; worked corners C = (0, 0), R = 10, 30°:
   (-3.66, -13.66), (13.66, -3.66), (3.66, 13.66), (-13.66, 3.66) to 0.01 mm.
   Rectangle, ellipse, path: the same box as `main` (values taken from `main`
   before the change), spec 15.
2. **Invariants, proptest** over centre, R, N in 3..=24, star ratio, frame
   angle, rotation (spec 1, 2): every vertex of `outline_of_rotated` lies inside
   the box (`to_local` within `[min, max]` plus 1e-9); the first outer vertex
   equals `to_document((C.x + R, C.y))` to 1e-9; a star's `param_handles` point
   equals the first inner vertex of the outline to 1e-9; rotating by Δ about C
   advances the box direction by Δ (mod 2π) and moves each corner exactly by that
   rotation.
3. **Typed rotation** (session level, spec 4): create at 78.7° by drag, press R,
   type 0, Enter: corners `C ± (R, R)` to 1e-9, readout and next prefill 0°;
   a table of typed angles A ∈ {-135, -90, 0, 15, 45, 90, 180}: box direction
   equals A.
4. **Drag rotate, preview equals commit** (spec 5): per frame of a corner-rotate
   drag the live box direction equals the live readout angle; the committed box
   equals the last live box within 1e-9 mm and 1e-12 rad; with and without
   Shift (pivot = opposite corner of the shown box, spec 9).
5. **Ctrl snap** (spec 6): 78.7° plus raw 1° lands on 75°, box direction 75°
   exactly; 45° plus raw 10° lands on 60°.
6. **Resize** (spec 7): hexagon C = (0, 0), R = 10, 30°; drag from (13.66, -3.66)
   by (4.83, -1.29): R = 13.54 (0.01), angle 30.0°, handle under the pointer
   within 0.01 mm; typed radius 13.54 gives the same shape within 1e-9; stroke
   width follows the switch as in `main`.
7. **Star parameter handle** (spec 8): for a star at frame angle 78.7°, rotation
   0 and for 10°/30°, the handle's document position equals the one the old code
   computed (golden numbers taken from `main` before the change) to 1e-9, and
   dragging it by 1 mm along the vertex direction changes the ratio by
   `1 / R` (clamped as before).
8. **No write, no format change** (spec 13, 14): over the old-style
   documents `acceptance_edit_polish_pr1_orientation.rs` already builds (a
   generated one with a star at 10°/30° and a polygon at 78.7°/0, plus
   `curvyo-document-core/tests/fixtures/primitives_v3.curvyo` and `rotation_v5.curvyo`):
   open, select, hover, close without an
   edit leaves the document's version vector and the registers' bytes equal; a
   rotate about C changes `rotation` only; a resize changes the frame radius
   only; `star_frame.angle` is bit-identical after every gesture; box direction
   78.7° and 40° on the fixture's two shapes; `CURRENT_FORMAT_VERSION` is 5.
9. **Cursor** (spec 11): hexagon at 30°: NE `resize:165.0`, SE `resize:75.0`;
   rectangle at 30° rotation: unchanged from `main`.
10. **Hit rule and handles** (spec 10, 12): hexagon at 30°, view scale 4 px/mm so
    handle areas are under 3 mm: a press at (0, -11) starts a move; a press at
    (0, -9) starts a move; no skew handle exists in `transform_handles` or in
    the hit rule at any angle in the table of item 1.

**Existing tests expected to change.** Any test that makes a polygon or star
with a non-zero `StarFrame.angle` and then names a box direction or reads a
handle position of the box: a star at frame angle -π/2 now has its `Ne` handle
where the box's turned corner is. Grep `angle: Angle::from_radians(` with a
non-zero value in `curvyo-editor-wasm/tests/` (`acceptance_0005.rs` 177, 190;
`acceptance_0005_switch.rs` 157, 870; `acceptance_otr_tester.rs` 99;
`acceptance_otr_dblclick.rs` 56; `acceptance_unified_editing.rs` 61;
`acceptance_edit_polish_pr1_escape_keys.rs` 580, 997) and `curvyo-ui-core/tests/acceptance_0005.rs`
260. A square box turned by a multiple of 90° is the same set of points, so
such a test usually only needs its handle name or corner order updated; the
implementer fixes the expectation to the new rule and lists each in the PR. A
test that fails for a reason other than the box direction is a regression, not
an update.

### 5. Size, ownership, PR

- **One PR, `story/polygon-star-box-refit`.** Production code: about 25 changed
  lines in four files (`oriented_box.rs`, `param_handles.rs`, `param_edit.rs`,
  `transform_primitive.rs`) plus `session/select_view.rs`; the rest is tests and
  docs. Crates touched: `curvyo-ui-core`, `curvyo-editor-wasm`. Not touched:
  `curvyo-document-core`, `render-core`, `app`, the frontend, any `Cargo.toml`.
- **Modules.** `oriented_box.rs` is 332 lines with tests (about 180 without) and
  keeps its one sentence ("the oriented selection box of one object"). No
  module nears 500 lines because of this change; no function grows past a few
  lines. New tests live in two new files.
- **Gate.** The full gate of `CLAUDE.md` §7; `ui-core` stays wasm32-clean (no new
  import). `architect` review is not needed beyond this note unless the PR
  adds a field, a key or a crate. `ux-engineer` reviews the corner handle
  placement and the cursor directions (spec UX notes). `tester` works from the
  spec only.

### 6. Sequencing against the other transform-touching features

All four edit the same crates (`ui-core`, `editor-wasm`), so by `CLAUDE.md` §4 no
two of them run in parallel, and this one is small enough to go alone and first.

**Recommended order: `polygon-star-box-refit`, then `rectangle-corner-radii`,
then `0007-stroke-and-fill-styling`, then `advanced-selection`, then
`ellipse-arcs-and-shaping`.** The three in the middle are free among
themselves; the hard constraints are these two:

- **Refit before `0007`.** `0007`'s gradient frame is `oriented_bounds` (its
  `adrs.md`, 2026-10-06 note). If a gradient on a polygon ships first, the refit
  then turns the gradient of every existing polygon created at an angle, a look
  change of saved files. Landing the refit first means a polygon's gradient is
  built on the final box.
- **Refit before `ellipse-arcs-and-shaping`.** Its part B adds a curve handle on
  polygon and star positioned at "vertex 0". A builder who copies the
  `star_inner_vertex` pattern before the refit adds `frame.angle` in box-local
  coordinates and has to undo it after. The rule from now on is the one in
  decision 1: **in box-local coordinates the first outer vertex of a polygon or
  star is at angle 0; handle placement never adds `StarFrame.angle`.** That
  feature's `adrs.md`, when written, must say so.
- **`rectangle-corner-radii`** touches `param_handles.rs`, `param_edit.rs` and
  `transform_primitive.rs` (rectangle arms), the same files but other match
  arms; the conflicts are textual and small, and it carries the next
  `format_version` bump, which this feature does not need. Refit first keeps its
  rebase trivial.
- **`advanced-selection`** touches `select_tool.rs` and the session's selection
  code, not the refit's files. Its marquee must read the box through
  `oriented_bounds` (its decision 4), so with the refit first "the box the user
  sees" is the same turned square for a polygon.

## Questions for the customer

None. (The box size is unchanged and criterion 1 of `edit-interaction-polish`,
which the customer accepted, already states that the box turns with the shape.
If the customer would rather have a shrink-wrapped box, that is the separate
story named in the spec's Out of scope, not a blocker for this one.)
