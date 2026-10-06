# ADRs for "Object transform: move, scale and rotate via on-canvas handles"

This slice adds one stored field: an object's **rotation**. That makes it
protected ground (a stored format and its merge behaviour), so the field and
every reader that must honour it are written out below. Everything else
(scale, the stroke and corner-radius scaling, the oriented box, the handles)
rewrites registers that already exist or is ephemeral UI state.
**No new crate, no new external dependency, no new `vecmanf-geometry-core`
function, no ADR amendment. `format_version` goes to 5** (was 4; see the
2026-10-05 architect note below).

One acceptance criterion conflicts with another (AC 18 against AC 20's last
sentence), and three are underdetermined. They are under "Flagged to the
lead", each with the default this file builds against.

## Depends on

- [ADR 0002 §3](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  rotation is an `Angle` (radians, the type slice 3 added for the star's θ).
  Degrees exist only in the readout (AC 22) and in the 15° snap step (AC 17),
  which is computed in radians from the drag-start angle. Every comparison
  takes an explicit `Tolerance`.
- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  the per-node affine transform is **still not built**. See "rotation is a
  stored angle, not the affine" below for why, and for how a later affine
  composes with this field.
- [ADR 0002 §6](../../docs/adr/0002-document-model-units-and-svg-round-trip.md)
  and slice 3 AC 21: rotating never converts a primitive (AC 19, 21).
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one handle drag is one commit, written on release; a press and release with
  no movement writes nothing (AC 3). Live preview renders transformed
  snapshots and writes nothing (slice 4's `translated` pattern).
- [ADR 0009 §2, §3](../../docs/adr/0009-concurrent-editing-semantics.md):
  handle hover, drag state, modifiers and the live readouts are ephemeral.
  §3 already lists "transform components" among the per-field LWW scalars;
  `rotation` is one.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  forces the `format_version` bump below.
- [ADR 0003 §1, §7](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  a path's tight box needs curve extrema, which stay in `geometry-core`
  (slice 4's `segment_bounds`). Nothing new is added there.
- [ADR 0001 §1, §3, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md)
  and [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md):
  handle layout, drag arithmetic and hit-testing in `ui-core`; drawing from
  `DecorationInput` in `render-core`; binding only in `editor-wasm` and
  `frontend/`. Every piece fits an existing crate over an existing edge.
- [`specs/0003-primitive-shapes/adrs.md`](../0003-primitive-shapes/adrs.md):
  the primitive schema, merge granularity, clamp-on-read corner radius, the
  outline module in `document-core`, handle layout in `ui-core`, and "object
  to path keeps the `NodeId`".
- [`specs/0004-canvas-navigation-and-selection/adrs.md`](../0004-canvas-navigation-and-selection/adrs.md):
  `ObjectSelection`, `hit_test_object`, `object_bounds`, the primitive frame
  box in `document-core`, and "a move rewrites geometry".

## Deliberately not in scope for this slice

- **The general per-node affine (ADR 0002 §5).** Due with the first story
  that needs shear or a group transform (`layers-and-grouping`, or SVG import
  of a skewed `<rect>` in `svg-import-export`). See the decision below and
  `docs/technical-debt.md`.
- **SVG export of a rotated object.** When slice 10 arrives, a rotated
  primitive exports as its element plus `transform="rotate(deg cx cy)"`; a
  path exports its baked anchors and nothing else.

## Feature-local decisions

- **2026-10-05: rotation is a stored angle on every object, not the
  affine.** Options:

  - **(A) One `rotation` register per object, the angle of the object's local
    x-axis in document space.** Chosen.
  - **(B) Build ADR 0002 §5's per-node affine now** (six numbers or
    translate/rotate/scale). Rejected. Every criterion here is rotation plus
    a resize *along the object's own axes* (AC 18 exists precisely so that no
    shear is ever needed), and that is fully expressed by "new frame, same
    angle". An affine would make every reader compose it: render, all six
    tools' hit tests and handle layouts, the node tool's drag inversion,
    `outline_of` / object to path, `translated`, later export and job
    generation. That is slice 4's rejection, unchanged, and nothing in this
    slice uses the extra degrees of freedom. It would also merge worse: a
    matrix is one register, so a concurrent resize and rotate keep one of the
    two, where (A) keeps both (below). The readout (AC 22) would become a
    matrix decomposition.
  - **(C) Bake rotation into primitives as a path.** Ruled out by slice 3
    AC 21 and this slice's AC 21.

  (A) is not a shortcut that (B) later has to undo. A primitive's rotation is
  a shape parameter in the same sense as its position and the star's θ
  already are (slice 3: "when a transform field arrives, those parameters
  become the shape's local geometry and stay as they are"). A later node or
  group affine composes on top of it, and a missing affine key reads as
  identity. The one open question it leaves (should a later affine absorb
  `rotation`?) is recorded in `docs/technical-debt.md`.

- **2026-10-05: one key, one meaning, two consequences by kind.**

  ```text
  object node (every kind), new field:
     rotation : f64 radians, LWW register, absent = 0
                written normalized to (-π, π]
  primitive:  the frame registers (rect_bounds, ellipse_frame, star_frame)
              are the shape in its LOCAL frame; the outline is rotated by
              `rotation` about the frame's centre.
  path:       anchors stay in document space (rotation is baked, AC 20,
              slice 4's "a move rewrites geometry"); `rotation` only says
              which axes the selection box and the resize handles use.
  ```

  The meaning is the same for both kinds: *the object's local axes*. For a
  primitive the stored geometry lives in those axes; for a path it does not.
  Consequences:

  - **Polygon/star keep θ.** θ stays the first vertex's angle in the local
    frame (the polygon tool's creation intent); `rotation` is the object's
    rotation like every other kind. The Select tool's rotate writes
    `rotation`, never θ. Two angles that add up is the price of one rule for
    all four primitives: a freshly drawn star has rotation 0 and an
    axis-aligned box (slice 4's baseline, AC 1), and AC 22's readout means
    the same thing on every kind. Rejected: deriving a star's "rotation"
    from θ, which would tilt the box of any star drawn at an angle and leave
    AC 1's "rotation is zero" undefined for stars.
  - **Object to path keeps `rotation`.** It is the converted path's
    orientation now, with the same meaning, so slice 3's "delete every
    primitive parameter key" excludes it. The converted path's box keeps
    the primitive's orientation.
  - **A path's `rotation` can never disagree with its geometry in a harmful
    way.** The box is computed tight *in* that frame (below), so any angle
    gives a correct enclosing box. A merge of a rotate with a concurrent node
    drag (slice 2's floor) still gives a valid box.

- **2026-10-06 (verification): `document.json` always writes `"rotation": 0.0`.** The decision above says `document.json` omits `rotation` when 0; the implementation writes it for every object, zero or not. Accepted: `document.json` is the non-authoritative view (ADR 0004 §1) and nothing reads it back, so an always-present number is simpler for a non-Rust reader than a key that appears and disappears. The authoritative `document.loro` register stays absent-until-set (absent = 0). The other reading rule from the same decision — any finite `rotation` is normalized to (−π, π] *on read* — is implemented (`path_codec::read_rotation`, which also reads a stored integer as that many radians).

- **2026-10-06 (verification): a press inside the selected object's box moves it.** Slice 4 hits an unfilled object only on its outline, so an interior press deselected. With transform handles that leaves a small selected object unmovable: its 16px handle radii tile the whole outline (every outline pixel is a handle) and its interior hits nothing — dragging the top edge of a 40 × 40 square resized it to zero height. Decision: with a **single** selection, a press inside that object's oriented box (edges included) that is not on a handle (resize or rotate) starts a move (AC 23's "body"). This slightly changes slice 4 for a *selected* unfilled object (its interior press used to deselect); an *unselected* object still hits only on its outline, a multi-selection is unchanged, and a press on empty canvas outside the selected box still deselects. Another object's outline inside the selected box still wins the press (it is hit first). Handles keep priority where they are (`SelectTool::handle_at`).

- **2026-10-05: merge granularity. `rotation` is its own register.** Slice 3
  rule 2 applies: every combination of a frame and an angle is a shape
  someone could have asked for, so this is not a blend. The common rotate
  (about the centre, AC 15) writes only `rotation`, so a concurrent resize
  and rotate both survive. A Shift-pivot rotate (AC 16) also moves the
  frame's centre and writes the frame too; a concurrent resize then keeps one
  frame and the angle, still a valid rotated shape. Folding the angle into
  the frame register (as `star_frame` does with θ) was rejected: it would
  change the arity of two registers we already write and lose the concurrent
  resize-plus-rotate case. Writes per interaction, one commit each:

  | Interaction | Primitive writes | Path writes |
  |---|---|---|
  | rotate about centre | `rotation` | anchors, `rotation` |
  | rotate, Shift pivot | `rotation`, frame | anchors, `rotation` |
  | resize (any handle) | frame, `corner_radius` (rect, only if changed), `stroke_width` (only with the switch on, AC 26) | anchors, `stroke_width` (only with the switch on) |
  | move (slice 4) | frame | anchors |

  A Select-tool resize now writes `corner_radius` (AC 9), where slice 3's
  own resize never does. A concurrent radius edit then keeps one of the two
  radii (ADR 0009 §3's floor).

- **2026-10-05: rotation is applied in exactly one place per kind.**
  - **Primitive:** `document-core`'s outline function returns the rotated
    outline (local outline, then rotate every anchor point about the frame
    centre and every handle vector by the angle). Rendering, `hit_test_object`,
    object to path and later export all read that outline, so none of them
    changes. `render-core` gets rotation for free and stays "commands carry
    resolved geometry"; it does not learn about angles.
  - **Path:** the rotate drag rewrites anchors with
    `ObjectSnapshot::rotated(pivot, Angle)` in `document-core` (points about
    the pivot, handle vectors by the angle, AC 20). A path resize rewrites
    them with `scaled(pivot, sx, sy, rotation)`: into the local frame,
    per-axis scale, back out (AC 12). Both are elementary arithmetic on
    `document-core`'s own types, as `translated` is, and the live preview and
    the commit share them. No public affine type is added (`CLAUDE.md` §5).
  - **The shape tools must become rotation-aware.** Slice 4 AC 23 hands a
    double-clicked primitive to its own tool. For a rotated one, that tool's
    handles (corners, radius, inner radius) must sit on the rotated shape and
    drag in its local axes. Decided: `ui-core` maps the pointer into the
    local frame (rotate by −angle about the frame centre) before the existing
    handle arithmetic, and maps handle positions out after layout. One pair
    of functions shared by the rectangle, ellipse and polygon/star tools; the
    tools' arithmetic is unchanged. Creating a shape writes no `rotation`.
    No criterion names this, but without it a rotated shape's own handles are
    in the wrong place (flag 3).

- **2026-10-05: the oriented box lives in `vecmanf-ui-core`.** The dividing
  question is unchanged: does it evaluate a curve? `object_bounds` returns an
  `OrientedBox` (a rectangle in the object's local frame plus the angle)
  instead of slice 4's axis-aligned rectangle, which is the angle-0 case:
  - primitive: slice 4's frame box from `document-core`, with `rotation`.
    One sine and cosine; no curve.
  - path: rotate the anchors by −`rotation` (`rotated`, above), take the
    union of `geometry-core`'s existing `segment_bounds` in that frame, and
    attach the angle. The extrema stay in `geometry-core`; the rotation is
    arithmetic. A cubic under a rotation is the cubic of the rotated control
    points, so the box is exact.

  Handle positions (8 or 4 corners, the rotate handle a fixed screen
  distance along the box's local −y from the top-edge midpoint), their hit
  test, and the inverse mapping of a drag into local axes all derive from
  `OrientedBox` in `ui-core`. `render-core` receives quads and handle
  positions through `DecorationInput`, as slice 4 passes rectangles. The
  hover box uses the same rule.

- **2026-10-05: stroke width and corner radius scaling are drag arithmetic,
  not a model concept.** `ui-core` computes the new absolute values from the
  drag-start snapshot and the drag's per-axis factors, and writes them with
  the frame in the same commit (AC 8, 9). Factors are always taken from the
  drag start, never compounded per event. For a non-uniform resize (an edge
  handle, or a free corner drag) one width cannot follow two factors, so the
  factor is **√(sx·sy)**, Inkscape's rule for "scale stroke width" (flag 2).
  The corner radius uses the same factor on its **raw** stored value; the
  clamp still happens on read (slice 3), so AC 9's "clamped after scaling"
  holds with no new rule. A factor of 0 (AC 13 clamp) never writes a stroke
  width ≤ 0: the width is floored at 0.01 mm, because
  `stroke-and-fill-styling` refuses `stroke_width ≤ 0` on open and a file
  must never become unopenable from a drag. *2026-10-06 (architect,
  review):* this sentence first said the width keeps its drag-start value;
  AC 8 was reworded to "the smallest value still above zero", and the code
  (`transform_drag::MIN_STROKE_WIDTH_MM` = 0.01 mm) follows AC 8. Floored
  at 0.01 mm, never ≤ 0. *2026-10-06 (customer feedback):* stroke scaling
  now happens only with the "Scale stroke width" switch on (AC 26); see the
  next note. The corner-radius rule is unchanged (AC 9, 31).

- **2026-10-06 (architect): the "Scale stroke width" switch (AC 8, 26-31).**
  A resize leaves `stroke_width` alone unless the switch is on.
  - **Where the state lives: `SelectTool`, not `Document`.** Options: (A) a
    field on `ui-core`'s `SelectTool`, reached through `Session`. Chosen. It
    is tool state (the customer, 2026-10-06: it belongs to the tool and sits
    in the Select tool's contextual bar, so AC 30's Properties-panel section
    is superseded). It is ephemeral under ADR 0009 §2, like the polygon/star
    tool's mode and point count. `Session::new` and `Session::open` build a
    fresh `SelectTool`, so AC 27 (off in every new session) holds by
    construction, and AC 29 holds because nothing reaches the document.
    (B) a document or project setting. Rejected: AC 27 and 29 forbid both.
    (C) the frontend holds the state and passes it on every pointer event.
    Rejected: that gives two owners, and the `ui-core` tests for AC 28 could
    not reach it.
  - **Type: `pub enum StrokeScaling { Keep, Proportional }` in `ui-core`,
    default `Keep`.** It is not a `bool`, because `compute_resize` already
    takes `shift` and `ctrl`, and a third bool trips
    `clippy::fn_params_excessive_bools` (pedantic, `-D warnings`).
  - **How it reaches the arithmetic (AC 28).** `SelectTool::pointer_down`
    copies the tool's current value into `SelectDrag::Resizing {
    stroke_scaling, .. }`. The live preview and the release commit both
    pass that copied value to the single `compute_resize(..,
    stroke_scaling)`. A toggle during a drag changes only the tool field,
    so it applies from the next press. `scale_stroke` runs only for
    `Proportional`. The √(sx·sy) factor and the 0.01 mm floor are
    unchanged. The corner-radius factor is computed as before in both
    states (AC 31).
  - **Writes (AC 8: "the stored stroke width is not rewritten").** The
    four commands `Document::resize_rect`, `resize_ellipse`,
    `resize_star_frame` and `resize_path` take `stroke_width:
    Option<Length>`, and `None` leaves the key untouched.
    `commit_resize(document, id, result, stroke_scaling)` passes `None`
    for `Keep`. Rejected: always passing the width and skipping the write
    when it equals the stored value. With the switch off, a peer's stroke
    edit that merged in during the drag differs from the drag-start
    snapshot, so the comparison would write the old width back over it.
    `None` cannot do that. With the switch on, the same command still
    skips a write equal to the stored value. `resize_rect` also skips an
    unchanged `corner_radius`: a radius of 0 scales to 0, and rewriting
    it would beat a concurrent radius edit. This is slice 2's rule and
    0007's style rule 5 (an LWW rewrite of an unchanged value is a new
    operation). The merge table above reflects this.
  - **wasm and frontend.** Add a getter/setter pair on `WasmSession`,
    `scale_stroke_width() -> bool` and `set_scale_stroke_width(bool)`, the
    same pattern as `poly_star_mode`/`set_poly_star_mode`. They forward
    to `Session` and on to `SelectTool`. A `bool` is fine at the JS
    boundary. The frontend reads the getter after New/Open and never
    stores the value (no `localStorage`). No new crate, no new
    dependency, no format change, and `format_version` is unaffected.
  - **Tests to update** (they assert the old always-on scaling):
    `vecmanf-editor-wasm/tests/acceptance_0005.rs` `ac8_*` (lines
    730-826; switch them on, and add AC 8 off-state, AC 27 and AC 28
    tests); `vecmanf-ui-core/src/select_tool.rs` unit tests
    `ac8_proportional_resize_scales_stroke_width`,
    `ac8_non_proportional_resize_scales_stroke_width_by_the_geometric_mean`
    and `ac8_stroke_width_is_floored_above_zero_when_a_resize_collapses_the_object`
    (set `Proportional`); every call site of the four resize commands
    for the `Option` signature: `vecmanf-document-core` unit tests in
    `shapes.rs`, `paths.rs` and `objects.rs:486`,
    `tests/acceptance_0005.rs:127-172` and `tests/acceptance_0005_peers.rs:60-84`. The `ui-core` property tests
    on `stroke_or_radius_factor` and the editor-wasm reverify
    finiteness/rotate tests stay valid. No fixture file assumes stroke
    scaling.

- **2026-10-05: `format_version` goes to 4.** Migration from version 3 is
  empty: absent `rotation` reads as 0. The bump is needed for the reader. A
  version-3 reader tolerates unknown keys (slice 3), so it would open a
  rotated rectangle and draw it unrotated: the silent partial read ADR 0004
  §9 forbids. `document.json` adds `rotation` (radians) to each entry in
  `objects`, omitted when 0. Open-file validation refuses with
  `OpenError::Damaged` a present `rotation` that is not a finite number. Any
  finite value is accepted and normalized on read. **`stroke-and-fill-styling`
  therefore goes to 5** (dated note in its `adrs.md`), because this slice
  ships first.

- **2026-10-05 (architect): `format_version` is 5, not 4.**
  `path-merge-split-and-node-types` was built and merges before this
  slice and takes 4 (`specs/0006-path-merge-split-and-node-types/adrs.md`,
  architect resolution). Everything in the decision above holds with
  "version 3" read as "version 4". `stroke-and-fill-styling` stays at 6.
  This slice starts after that branch merges. Two additions to its plan:
  `PathSnapshot` gains `rotation`, and `Document::split_at_anchor`'s new
  object (open path) copies the original's rotation, with a test; Join
  needs no change, because the surviving path keeps its own meta map and
  the other path is deleted. A version number in an `adrs.md` is
  provisional: the PR that merges takes `main`'s
  `CURRENT_FORMAT_VERSION + 1`.

- **2026-10-05: the crate boundary.**
  - `vecmanf-document-core`: the `rotation` field on primitive and path
    snapshots, its codec and validation, the rotated outline,
    `ObjectSnapshot::rotated` and `scaled`, `Document::set_rotation`-style
    command(s) that write the table above in one commit, object to path
    keeping `rotation`, `CURRENT_FORMAT_VERSION = 5` (provisional — see the
    architect's resolution above: the merging PR takes `main`'s
    `CURRENT_FORMAT_VERSION + 1` at merge time).
  - `vecmanf-geometry-core`: no change.
  - `vecmanf-ui-core`: `OrientedBox`, transform-handle layout and hit test,
    the scale/rotate drag state machine in `select_tool` (modifiers, pivots,
    15° snap, zero clamp), stroke/radius factor arithmetic, the local-frame
    mapping for the shape tools.
  - `vecmanf-render-core`: oriented quads, transform handles and the rotate
    handle from `DecorationInput`. No angle logic.
  - `vecmanf-editor-wasm` / `frontend/`: binding, modifier state, the two
    live readouts (positioned from values `Session` returns already in screen
    pixels, slice 4's rule).

## Flagged to the lead

1. **Conflict: AC 18 against AC 20's "No new document-model field is needed
   for paths."** AC 18 says a rotated object, *reselected from scratch*,
   shows a box oriented to its rotation; AC 12 resizes a path "along the
   object's own local axes"; AC 22 shows a path's current angle. Once
   rotation is baked into anchors, nothing on a path remembers the angle, so
   none of the three is possible without a stored angle. Options: (a) paths
   store `rotation` as orientation only (decided above; geometry still baked,
   AC 20's first sentence holds); (b) the PO limits AC 18 to primitives, and
   a path's box re-squares on release, which is Inkscape's behaviour (a
   screen-axis resize of a path is a representable shear). **Recommendation
   (a): one rule for every object, one cheap register. Default: (a); the PO
   deletes AC 20's last sentence.**
2. **AC 8/9 do not say which factor applies when sx ≠ sy.** Default above:
   √(sx·sy), and a zero factor floors the stroke width at 0.01 mm, never
   ≤ 0 (2026-10-06: was "leaves the stroke width unchanged"; AC 8 now says
   so).
3. **Gap: a rotated primitive handed to its own tool (slice 4 AC 23).** No
   criterion says its handles follow the rotation. Decided above that they
   do. The PO should add a criterion so the tester covers it.
4. **AC 16's "opposite corner from the rotate handle" is ambiguous.** The
   handle sits above the top-edge *midpoint*, which has no opposite corner.
   For the `ux-engineer`/PO: bottom-edge midpoint (through the centre), or a
   fixed bottom corner. Default: bottom-edge midpoint. No architectural
   impact.
5. **Slice 4 AC 20 is not a conflict.** "Only the bounding box" is true when
   slice 4 ships; this slice's AC 1 names its own amendment explicitly, and
   slice 4's out-of-scope list deferred exactly these handles. No edit to the
   Ready slice-4 spec. The tester's slice-4 regression test must assert "no
   shape handles and no path nodes", not "no handles at all". For the
   `ux-engineer`: AC 11's corner-only handles on polygon/star are the first
   per-kind difference in the Select tool, against slice 4's "no
   type-specific affordance, full stop" note.
6. **No new crate, no new dependency, no ADR amendment.**
