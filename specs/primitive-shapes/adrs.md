# ADRs for "Primitive shapes: rectangle, ellipse and polygon/star tools, and 'object to path'"

This slice extends `path-node-editing` (slice 2). It does not reopen any
decision from that slice. It puts the second kind of geometry into the
document model: a shape kept as parameters, not as anchors. That makes it
protected ground again (a stored format and its merge behaviour), so the schema
is written out in full below. It reads as a continuation of slice 2's anchor
schema.

All 22 acceptance criteria can be built against ADR 0002 §5/§6 and ADR 0009 §3
as they read today. **No new crate, no new external dependency and no new
`vecmanf-geometry-core` function** is needed (see the boundary decision below).
One acceptance criterion conflicts with crash safety (AC 10, under "Flagged to
the lead"). No acceptance criterion conflicts with an ADR.

## Depends on

- [ADR 0002 §6](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  "Rectangles, circles and ellipses are kept as their own primitives (they
  carry editing intent and corner radii); 'object to path' is an explicit,
  destructive conversion." That sentence is this slice. The slice extends it
  to polygon/star, on the same reasoning: point count and inner ratio are
  editing intent that a path cannot hold. AC 21's "no implicit conversion" is
  §6's "explicit".
- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  a primitive is **one tree node** in the same tree as paths, with a
  CRDT-minted `NodeId`, its own fully resolved style, and z-order taken from
  sibling order. Primitives and paths share one z-order, so they share one
  tree. They are not two lists.
- [ADR 0002 §2, §3, §4](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  every stored parameter is a millimetre `f64` behind a newtype, in Y-down
  document space. Sizes and radii are `Length`, positions are `Point`.
  Polygon/star rotation is the first use of §3's `Angle` (radians), and this
  slice adds that type to `units.rs`. Point count and inner ratio are
  validated newtypes (≥ 3, and 0 < R < 1), not a bare `u32`/`f64` on a public
  signature.
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one interaction is one commit, made when the interaction completes. This
  covers creating a shape, each handle drag, each committed control change,
  and one "object to path" call (including AC 22's multi-object case). Slice
  2's command funnel (one `Document` method per command, each ending in
  exactly one labelled Loro commit, ids resolved before the first write) is
  extended, not replaced.
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): these
  are ephemeral and never written: the in-progress drag-out rectangle,
  ellipse or star, live handle-drag geometry, object selection, and the
  polygon/star tool's point-count, ratio and mode settings (AC 10's "not
  reset between shapes"). Object selection holds `NodeId`s and resolves them
  lazily, the same way slice 2's anchor selection does.
- [ADR 0009 §3](../../docs/adr/0009-concurrent-editing-semantics.md): every
  primitive parameter is a per-field LWW register. The decisions below set
  where the field boundaries fall. §3 names "radii" among its scalars, and
  this slice is where that word first gets a stored shape.
- [ADR 0011 §2, §3](../../docs/adr/0011-workspace-and-crate-layout.md):
  everything fits the four crates slice 2 created and the existing edges.
  `render-core → document-core` only is the constraint that decides where
  outline construction lives (below).
- [ADR 0003 §1, §2, §7](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  `vecmanf-geometry-core` is used, not extended. Hit-testing a primitive's
  outline uses slice 2's `nearest_point_on_segment`. Display tessellation
  stays in `render-core` at its own tolerance.
- [ADR 0001 §1, §3, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  the three tools are state machines in `vecmanf-ui-core`. The frontend adds
  tool-rail entries and the polygon/star controls and holds no editing logic.
  Live drags cross the wasm boundary as typed arrays.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  forces the `format_version` bump below.
- [`specs/path-node-editing/adrs.md`](../path-node-editing/adrs.md): the
  anchor schema (decisions 1–3), "commands carry resolved geometry", the
  path/node crate boundary, and "a press and release with no pointer movement
  writes nothing". All four apply here unchanged.

## Deliberately not in scope for this slice

- **SVG export of primitives** (`svg-import-export`, slice 8). AC 21 lists
  "exporting", but nothing exports yet, so in this slice AC 21 is tested
  against save, tool switching and selection. When slice 8 arrives it must
  emit a rectangle as `<rect>` (and an ellipse as `<ellipse>`) and must not
  convert them to paths. ADR 0002 §10 lists primitives in the owned subset.
- **Bit-identical polygon/star vertices across platforms.** Vertex positions
  come from `f64::sin`/`cos`. Their last bit can differ between the desktop
  and the `wasm32` libm. Nothing in this slice reaches ADR 0009 §4's
  "identical export bytes" promise: rendering does not care, and a
  conversion stores the anchors it computed as data. This becomes slice 8's
  question if the `d` of a star is exported from parameters.
- **Per-node affine transform** (ADR 0002 §5) is still not implemented, as
  in slice 2. A primitive's position is part of its own parameters, the same
  as SVG's `x`/`y` and `cx`/`cy`. When a transform field arrives, those
  parameters become the shape's local geometry and stay as they are.

## Feature-local decisions

- **2026-10-04: the primitive schema.** It sits beside slice 2's path schema
  in the same Loro tree. Exactly what the 22 criteria need:

  ```text
  object = tree node in the existing "paths" tree (ADR 0002 §5), fields:
     shape         : absent = path | "rect" | "ellipse" | "polygon" | "star"
                     written at creation; deleted by object-to-path; never
                     written otherwise
     stroke_width, stroke, (fill)   slice 2's style registers, same keys,
                                    same 0.25 mm black default    (AC 16)
  path (shape absent):  closed, anchors        slice 2, unchanged
  rect:     rect_bounds   : [x, y, w, h] mm   ONE LWW register   (AC 1–3)
            corner_radius : mm, stored raw    LWW register        (AC 4–6)
  ellipse:  ellipse_frame : [cx, cy, rx, ry]  ONE LWW register   (AC 7–9)
  polygon:  star_frame    : [cx, cy, r, θ]    ONE LWW register   (AC 11, 13)
            point_count   : integer ≥ 3       LWW register        (AC 10, 15)
  star:     star_frame, point_count as polygon, plus
            inner_ratio   : 0 < R < 1         LWW register        (AC 12, 14)
  ```

  Details:

  - **The tree key stays `"paths"`.** It is an opaque container name in a
    file format we already write. Renaming it would mean a container
    migration and buy nothing. Rename the Rust constant to `OBJECTS_TREE`
    and add a comment.
  - **"Absent means path"** lets every version-2 file open as it is, so the
    migration stays empty. `shape` has exactly one creating write and at
    most one delete. No interaction ever writes it concurrently with itself,
    so it never needs a merge rule. Polygon and star are two `shape` values,
    not a flag, because the specification makes the mode fixed after
    creation ("Out of scope"). A circle is an ellipse with `rx = ry`. It has
    no kind of its own and no flag (AC 9: nothing keeps `rx = ry`).
  - **Rect bounds are normalized on write.** `x, y` is the top-left (minimum)
    corner and `w, h ≥ 0`. A drag from A to B in any direction stores the
    same value (AC 1).
  - **θ is the angle of the first outer vertex**, `atan2(dy, dx)` from the
    centre in Y-down document space. That vertex sits at B (AC 11, 12). The
    star's inner vertices are at angular midpoints and are derived, not
    stored. Inkscape's independent `arg2` (a twisted star) is not
    representable, and no criterion asks for it.
  - **`Document::path(id)` returns `None` for a primitive node.** It must not
    return an empty `PathSnapshot`, so slice 2's node tool and hit-testing
    never mistake a primitive for a path with no anchors. Reading goes
    through an object-level snapshot (`enum` of path / primitive). The
    primitive variant is an `enum` over the four shapes. This is not a
    trait: the set is closed, and `CLAUDE.md` §5 applies.

- **2026-10-04: merge granularity. A handle's whole geometry is one
  register; each independent shape parameter is its own register.** The
  rule: values that one handle drag writes together are one register;
  values that separate controls write are separate registers. Slice 2
  decision 1 applied to shapes:

  1. **`rect_bounds`, `ellipse_frame` and `star_frame` are each one
     register.** Dragging a corner handle of a rectangle moves the corner,
     which changes `x` and `w` together. As two registers, two peers
     dragging opposite handles could merge into A's `x` with B's `w`. That
     is a box neither peer drew: ADR 0009 option D reached by field
     splitting. The ellipse is treated the same way so that the
     `ux-engineer` can choose bounding-box handles or Inkscape's separate
     rx/ry handles without a format change. The cost is ADR 0009 §3's
     stated floor: two peers dragging the same ellipse's rx and ry at the
     same instant keep one of the two drags.
  2. **`corner_radius`, `point_count` and `inner_ratio` are separate
     registers, separate from the frame and from each other.** The
     specification itself makes them orthogonal. AC 3: resizing keeps the
     radius. AC 13: scaling keeps the ratio and the point count. AC 14:
     ratio editing keeps the outer radius. AC 15: a point-count change keeps
     size, ratio and orientation. A resize therefore writes only the frame,
     and a concurrent radius, ratio or count edit survives with it. Every
     combination is a shape someone could have asked for, so this is not a
     blend.
  3. **The star stores `inner_ratio`, not an inner radius.** That is what
     makes AC 13 (outer changes, ratio fixed) and AC 14 (ratio changes,
     outer fixed) write disjoint registers. Storing an inner length would
     make every resize write two registers and race a concurrent ratio
     drag.

- **2026-10-04: the corner radius is stored raw and clamped where it is
  evaluated.** Effective radius = `min(corner_radius, min(w, h) / 2)`. Every
  consumer uses the effective value: outline, rendering, hit-testing, handle
  placement, conversion, and later export. A resize never writes
  `corner_radius` (AC 3). The radius-handle drag writes a value already
  clamped against the bounds at that moment. Dragging onto the rectangle's
  own corner (within the point hit tolerance) writes exactly `0.0`, and so
  does "remove rounding" (AC 6). Clamping when writing was rejected because
  it cannot hold AC 5 under merge. Peer A shrinks the box while peer B
  concurrently sets a radius measured against the old box. The merged state
  then has B's radius on A's box, and nobody re-clamps it. Clamping on read
  makes AC 5 true for every reachable state, merged ones included, and it
  is SVG's own `rx` rule. One consequence is visible and not pinned by the
  specification: **shrinking a rounded rectangle and then growing it back
  restores the original radius.** Inkscape does the same, because it keeps
  `rx` and clamps when drawing. "Rounded" in AC 18 means effective radius
  > 0.

- **2026-10-04: "object to path" keeps the `NodeId`.** The command rewrites
  the same tree node in one commit. It deletes `shape` and every primitive
  parameter key, and writes `closed = true` and the `anchors` list. Style
  registers are not touched. Deleting the primitive node and creating a new
  path node was rejected for four reasons:

  1. Two peers converting the same rectangle concurrently would get **two
     overlapping paths**. Rewriting in place converges to one: both create
     the `anchors` container under the same map key, and LWW keeps one
     complete outline.
  2. z-order is preserved with no tree move.
  3. AC 17's "selected" and AC 22's "remain selected together" need no
     selection remapping, because the ids do not change.
  4. A concurrent style edit to the shape (slice 4) by another peer
     survives the conversion. With delete and create it would land on a
     tombstone.

  A concurrent *parameter* edit (peer B resizes while A converts) is lost.
  The object is a path because `shape` is absent. If B's write is ordered
  last, its key can reappear as a stray `rect_bounds` on a path node. That
  key is never read. **Open-file validation must therefore tolerate unknown
  keys on a node and dispatch on `shape` alone.** This is the delete-versus-edit
  outcome ADR 0009 §3 accepts, and it is visible, not silent: the shape
  became a path.

  The command carries resolved geometry (slice 2's rule). `ui-core` computes
  each outline, attaches `AnchorId`s minted by its existing
  `AnchorIdMinter`, and dispatches one `convert_to_paths(&[(NodeId,
  Vec<NewAnchor>)])` for the whole selection. The method resolves every id
  before writing. It refuses the call if any id is missing or is not a
  primitive, and makes one commit. `ui-core` filters the selection to
  primitives first, so a mixed selection converts its primitives and leaves
  its paths alone.

- **2026-10-04: outline construction lives in `vecmanf-document-core`.**
  This sharpens slice 2's dividing question. The operative test, as slice 2
  applied it, is *does this code evaluate a curve* (flatten, project,
  subdivide, intersect)? Placing control points with a closed-form formula
  does not, and slice 2 already put "make curve" (handles at chord / 3) in
  `document-core` on that basis. A primitive's outline is closed-form in the
  same sense: trigonometry for polygon/star vertices, and the constant
  `KAPPA = 4·(√2 − 1)/3 ≈ 0.552285` for ellipse quadrants and rounded
  corners. It is one pure function, a primitive's parameters → its outline
  anchors without ids, in its own module (e.g. `primitive_outline.rs`).
  Three consumers reach it over edges that already exist:

  - `render-core` draws a primitive by stroking that outline through slice
    2's existing lyon path builder. AC 16's identical stroke holds by
    construction, and AC 17's "visually identical" holds exactly, not just
    within a tolerance.
  - `ui-core` hit-tests the outline with `geometry-core`'s
    `nearest_point_on_segment`, and converts it for AC 17–20.
  - slice 8's SVG exporter, which ADR 0011 §2 places in `document-core`.

  Rejected: **(a) `geometry-core` owns it.** `render-core` and the SVG
  exporter cannot reach `geometry-core` (ADR 0011 §3). This would cost an
  edge amendment or a second implementation that AC 17 then requires to
  agree with the first. Nothing in `geometry-core` or `kurbo` helps.
  `kurbo`'s `Ellipse`/`RoundedRect` path elements choose their own segment
  count from a tolerance, so they cannot guarantee AC 18/19's exact counts,
  and the kappa construction would be hand-written either way. **(b)
  `render-core` draws primitives with lyon's own `add_ellipse` /
  `add_rounded_rectangle`.** That is a second outline implementation, and
  AC 17 would become a cross-implementation tolerance claim instead of
  identity.

  The exact outline. All outlines are closed. Start points and direction
  are fixed so that golden tests are deterministic: direction is increasing
  angle in Y-down space, which is clockwise on screen, as Inkscape's rect →
  path is. AC 18–20:

  | Shape | Anchors | Kind | Handles |
  |---|---|---|---|
  | rect, effective r = 0 | 4, from top-left | Corner | all zero |
  | rect, effective r > 0 | 8 tangent points, from the top edge's left one | **Corner** | κ·r toward the corner on the arc side, zero on the line side |
  | ellipse | 4, from `(cx+rx, cy)` | Smooth | vertical κ·ry at the rx extremes, horizontal κ·rx at the ry extremes; mirrored, equal length |
  | polygon | N, from vertex at θ | Corner | all zero |
  | star | 2N, outer at θ + 2πk/N, inner at θ + π/N + 2πk/N, radius R·r | Corner | all zero |

  The rounded-rectangle tangent points are **Corner, not Smooth**. Slice 2's
  `Smooth` means mirrored, equal-length handles (`handle_in =
  -handle_out`). A line-to-arc join has one zero handle. As `Smooth`, the
  first handle drag would pull a curve out of the straight side. At
  effective r = ½ the shorter side, the two anchors on that side coincide.
  AC 18 still says 8, and Inkscape behaves the same. For a circle, kappa's
  maximum radial deviation is ≈ 0.027 %. An ellipse is an affine image of a
  circle, so it stays within 0.027 % of its larger radius, under AC 19's
  0.1 %.

- **2026-10-04: the crate boundary for the tools.** No new crate.
  - `vecmanf-document-core`: the schema above, `Angle`, the two validated
    newtypes, the object-level snapshot, the outline module, and the command
    methods. The methods are create primitive, set frame, set corner radius,
    set point count, set inner ratio and convert to paths. Each makes one
    commit, and each refuses with a typed error when the id is stale or the
    shape is wrong (AC 14: a polygon has no inner ratio to set).
  - `vecmanf-ui-core`: the rectangle, ellipse and polygon/star tool state
    machines; object selection; **handle layout** (which handles a selected
    primitive shows and where, from its snapshot and effective radius);
    handle-drag → parameter arithmetic; and hit-testing for objects and
    handles. Handle layout lives here, not in `document-core`, because which
    handles exist is interaction design (the `ux-engineer`'s). `render-core`
    receives handle positions and states through `DecorationInput`, which
    `editor-wasm` passes across as binding, exactly as slice 2 passes the
    anchor selection. That gives one source for handle positions, used by
    both hit-testing and drawing. No `Tool` trait is required: rectangle and
    ellipse may share their drag-out state machine through an `enum`.
  - `vecmanf-render-core`: object snapshots → outline → the existing stroke
    path; handle glyphs from `DecorationInput`. **Rename the existing
    `primitives.rs` (decoration glyphs: squares, diamonds, rings) to
    `glyphs.rs`** in this slice. After this slice, "primitive" means a
    document shape, and a module of that name holding UI glyphs would
    mislead (`CLAUDE.md` §5, one responsibility, stated plainly).
  - `vecmanf-geometry-core`: no change.
  - `vecmanf-editor-wasm` / `frontend/`: tool-rail entries, the polygon/star
    point-count, ratio and mode controls, and "object to path" / "remove
    rounding" actions. Binding only.

- **2026-10-04: interaction commits.** Each of these is one commit:
  - creating a shape, on pointer-up;
  - one handle drag, on release;
  - one committed change to the point-count or ratio control while a shape
    is selected (a slider drag commits on release, with an ephemeral
    preview);
  - one "object to path" call.

  Creation where A = B writes nothing (AC 1, 7). **The same rule applies to
  polygon/star** (see "Flagged to the lead" 2). Slice 2's "a press and
  release with no pointer movement writes nothing" extends to every handle
  drag here. Changing a control with no shape selected changes only
  ephemeral tool state and writes nothing.

- **2026-10-04: `format_version` goes to 3, and `document.json` lists
  `objects`.** A slice-2 reader would refuse a primitive node as `Damaged`
  (it has no `anchors`). That is the wrong message: ADR 0004 §9 wants
  "newer version". Migration from version 2 is empty by construction,
  because absent `shape` means path. `document.json`'s `paths` array
  becomes `objects` in z-order, each tagged with its `shape`. Open-file
  validation dispatches on `shape` and refuses with `OpenError::Damaged` in
  these cases:
  - an unknown `shape` value;
  - a missing or mistyped parameter for the given `shape`;
  - a non-finite number;
  - a negative size or radius;
  - `point_count < 3` (and above the cap; see "Flagged to the lead" 1);
  - `inner_ratio ∉ (0, 1)`.

  Unknown extra keys are tolerated (see "object to path").

## Flagged to the lead

1. **Conflict: AC 10 ("any N ≥ 3 with no upper limit") against crash
   safety.** `project-file-foundation` AC 7 says a damaged file is a named
   error, not a crash. A typed or crafted `point_count` of 10⁹ makes
   rendering, hit-testing and conversion allocate billions of anchors, which
   is an out-of-memory abort. That is the case in the editor, and on open
   for a crafted file. Options: (a) the PO amends AC 10 to `3 ≤ N ≤ 1024`,
   Inkscape's own limit, which the AC already cites; (b) keep AC 10 as
   written and accept the crash. **Recommendation (a). Default if no
   answer: implement the cap of 1024 in the control and in open-file
   validation, and the PO aligns AC 10's wording.**
2. **Gap: AC 11/12 do not say what A = B does for polygon/star.** AC 1/7 do
   say it for rectangle and ellipse. A zero-radius polygon is invisible and
   cannot be selected. Decided above: it creates nothing. The PO should
   fold that into AC 11/12. Related: AC 1 literally allows a zero-height
   rectangle (A ≠ B on one horizontal line). It is kept as specified. Its
   conversion has coincident anchor pairs.
3. **AC 18's "4 corner nodes" is ambiguous about node kind.** Decided above:
   all 8 nodes are `Corner`. The tester should assert that kind. The PO may
   want the wording to say so.
4. **Decided here, not pinned by the specification:** a rounded rectangle
   that is shrunk and then grown back gets its original radius back
   (clamping on read). This matches Inkscape, and it is the only rule under
   which AC 5 survives a merge.
5. **No new crate, no new dependency, no new `geometry-core` function.** No
   ADR amendment is needed. ADR 0003 §1's "only place geometric algorithms
   live" is read the way slice 2 read it, sharpened to "evaluates a curve".
