# Primitive shapes: rectangle, ellipse and polygon/star tools, and "object to path"

Status: Done
Priority: Must
Origin: Customer

## User value

As a maker I want dedicated rectangle, circle/ellipse and polygon/star tools
that create their own adjustable shape — not a path I have to build node by
node — and an explicit "object to path" action to drop into full node editing
only when I actually need it, so that the common case (a rectangle with
rounded corners for a box joint, a circle for a mounting hole, a hex/star
outline) stays a few drag-and-adjust steps instead of a pen-tool exercise,
matching how Inkscape itself separates "shape tool" from "node tool" and
makes converting between them an explicit choice rather than something that
happens automatically underneath the maker.

Per `specs/index.md`'s ordering note, this slice is a thin layer over
`path-node-editing` (slice 2), not a parallel implementation: it reuses that
slice's tool rail, selection/hover conventions, hit-testing tolerances and
command/commit model, and "object to path" converts into exactly the
anchor/path model slice 2 already built — no new document representation for
edited paths. What is new here is the primitive shapes themselves: each is
its own object with its own parameters (width/height/corner radius; rx/ry;
point count/ratio), not a path, until the maker explicitly converts it.

**What we do differently from the tool this replaces:** at the interaction
level, nothing by design — R-EDIT-002 and R-EDIT-004 ask for the primitives
Inkscape already has, and a maker switching tools gradually (R-SYS-006)
should find the rectangle/ellipse/star tools where they expect them, behaving
the way they expect. Where this slice simplifies relative to Inkscape's exact
feature set (independent horizontal/vertical corner radii, extra Ctrl ratio
presets, ellipse arc/pie modes, star rounding/randomization), that is stated
per criterion below and in "Out of scope", not silently dropped.

## Acceptance criteria

### Rectangle tool

1. Given the rectangle tool is active and no shape is being drawn, when the
   maker presses the mouse button down at point A and drags to point B before
   releasing, then a rectangle primitive is created with A and B as two
   opposite corners of its axis-aligned bounding box (width = |Bx − Ax|,
   height = |By − Ay|), zero corner radius, and it becomes the selected
   object. A drag where A equals B (including a plain click with no movement)
   creates nothing.
2. Given the rectangle tool is active, when the maker holds the constrain
   modifier (Ctrl) while dragging, then the created rectangle is a square,
   sized to the larger of the drag's horizontal and vertical extents — this
   slice supports only the 1:1 constraint; Inkscape's further ratio presets
   (golden ratio, 1:2, 2:1, ...) are not reproduced (see "Out of scope").
3. Given a selected rectangle and the rectangle tool active, when the maker
   drags one of its resize handles, then the rectangle's width and/or height
   changes to match the drag, any existing corner radius keeps its absolute
   length unless the new, smaller dimension forces it down (criterion 5's
   clamp), and the object remains a rectangle primitive, not a path.
4. Given a selected rectangle and the rectangle tool active, when the maker
   drags its corner-radius handle away from the rectangle's own corner, then
   all four corners round by the same radius value — this slice's rectangles
   have one radius, not Inkscape's independent horizontal/vertical pair (see
   "Out of scope") — each corner replaced by a quarter-circle-equivalent
   curve of that radius.
5. Given a rectangle whose corner radius would otherwise exceed half of its
   shorter side, then the radius is clamped to exactly half the shorter side
   instead of letting the rounded corners overlap or self-intersect.
6. Given a rounded rectangle, when the maker drags the corner-radius handle
   back onto the rectangle's own corner, or invokes "remove rounding", then
   the radius returns to exactly zero and all four corners are sharp again.

### Circle/ellipse tool

7. Given the ellipse tool is active and no shape is being drawn, when the
   maker presses the mouse button down at point A and drags to point B before
   releasing, then an ellipse primitive is created whose axis-aligned
   bounding box has A and B as opposite corners (rx = |Bx − Ax| / 2,
   ry = |By − Ay| / 2), and it becomes the selected object. A drag where A
   equals B creates nothing.
8. Given the ellipse tool is active, when the maker holds the constrain
   modifier (Ctrl) while dragging, then the created ellipse is a circle
   (rx = ry), sized to the larger of the drag's horizontal and vertical
   extents — the same single 1:1 constraint as criterion 2; Inkscape's
   further ratio presets are not reproduced.
9. Given a selected ellipse and the ellipse tool active, when the maker drags
   one of its resize handles, then rx and/or ry change to match the drag and
   the object remains an ellipse primitive, not a path. (A circle created
   under criterion 8 can be reshaped into a non-circular ellipse this way —
   nothing keeps rx = ry after creation.)

### Polygon/star tool

10. Given the polygon/star tool is active, when the maker sets the
    point-count control to an integer N, then the control only accepts
    3 ≤ N ≤ 1024 (Inkscape's own limit, adopted here as a crash-safety bound,
    not a style choice — an unbounded N risks the same kind of
    resource-exhaustion crash `project-file-foundation` AC 7 already rules
    out for a damaged file) and every subsequently created polygon/star has N
    points; the control is not reset between shapes. Given a `.vmf` file
    whose stored `point_count` for a polygon/star is outside 3–1024, when the
    maker opens it, then the file is refused the same way a damaged file is
    (a named error, not a crash or a silently clamped value) — see
    `project-file-foundation` AC 7.
11. Given the polygon/star tool is active in "polygon" mode, when the maker
    presses the mouse button down at center point A and drags to point B
    before releasing, then a regular N-sided polygon primitive is created
    centered at A, with one vertex at B and the remaining N − 1 vertices
    spaced evenly around A at the same radius |AB|, and it becomes the
    selected object. A drag where A equals B (including a plain click with no
    movement) creates nothing, same as criteria 1 and 7.
12. Given the polygon/star tool is active in "star" mode with an inner/outer
    radius ratio R set (0 < R < 1), when the maker presses the mouse button
    down at center point A and drags to point B before releasing, then a star
    primitive is created with N outer vertices and N inner vertices
    alternating around center A — outer radius = |AB| with one outer vertex
    at B, inner radius = R × outer radius — and it becomes the selected
    object. A drag where A equals B creates nothing, same as criteria 1, 7
    and 11.
13. Given a selected polygon or star and the tool active, when the maker
    drags one of its resize handles, then the whole shape scales uniformly
    (outer radius changes; for a star, inner radius scales with it so ratio R
    is unchanged), keeping point count and orientation the same.
14. Given a selected star (not a plain polygon) and the tool active, when the
    maker drags its inner-radius handle, or edits the ratio control, then the
    inner radius changes, the outer radius stays fixed, and the shape updates
    live; a plain polygon has no inner-radius handle, since it has no inner
    radius distinct from its outer one.
15. Given a selected polygon or star and the tool active, when the maker
    changes the point-count control, then the shape updates live to the new
    point count, keeping its current size, ratio (for a star) and
    orientation.

### Placeholder rendering

16. Given any primitive created by criteria 1, 2, 7, 8, 11 or 12, when it is
    rendered on canvas, then it shows the same 0.25 mm solid black stroke and
    no fill as a pen-tool path (`path-node-editing` criterion 6) — this slice
    adds no styling of its own; stroke and fill the maker can change remain
    `stroke-and-fill-styling`'s job (slice 4).

### Object to path

17. Given one selected primitive shape (rectangle, ellipse/circle, polygon or
    star, in any state criteria 1–15 can produce), when the maker invokes
    "object to path", then the primitive is replaced by one path object built
    from the `path-node-editing` anchor/path model, visually identical to the
    primitive at the moment of conversion within that slice's existing
    hit-test tolerance, selected and immediately editable with the node
    tool — and the shape's own parameters (width, height, corner radius, rx,
    ry, point count, ratio) no longer exist anywhere; only path anchors
    remain.
18. Given a rectangle with zero corner radius, when converted to a path
    (criterion 17), then the result has exactly 4 nodes, all of `path-node-
    editing`'s **Corner** type, joined by straight segments. Given a
    rectangle with a non-zero corner radius, when converted, then the result
    has exactly 8 nodes, all likewise of **Corner** type (not **Smooth** —
    a smooth node's mirrored, equal-length handles cannot represent a
    straight edge meeting a curved corner), 4 of them joined to their
    neighbor by a straight segment and 4 joined by one curve approximating
    that corner's quarter-circle.
19. Given a circle or ellipse, when converted to a path (criterion 17), then
    the result has exactly 4 smooth nodes joined by 4 cubic Bézier segments
    (one per quadrant), using the standard circle-to-Bézier control-point
    ratio (kappa ≈ 0.5523), deviating from the true ellipse by no more than
    0.1% of its larger radius.
20. Given a polygon with N points, when converted to a path (criterion 17),
    then the result has exactly N corner nodes joined by straight segments.
    Given a star with N points, when converted, then the result has exactly
    2N corner nodes (alternating outer/inner) joined by straight segments —
    a star or polygon's edges are never curved by this slice's conversion.
21. Given a primitive shape, when the maker does anything other than
    explicitly invoking "object to path" — switching tools, saving the
    project, exporting, or selecting a different object — then the shape
    stays a primitive with its own parameters; no implicit trigger performs
    the conversion.
22. Given two or more primitives selected together, when the maker invokes
    "object to path", then each selected primitive converts independently
    and correctly per criteria 17–20 (correct geometry, kept node identity,
    no duplication or dropped shapes), the whole batch committed as one
    atomic operation — which, if any, of the resulting path objects end up
    selected afterward is not specified by this slice (see "Out of scope").

## Out of scope

- Independent horizontal/vertical corner radii on a rectangle (Inkscape's
  separate Rx/Ry handles, which can round corners into quarter-ellipses
  rather than quarter-circles). This slice has one radius per rectangle
  (criterion 4). Revisit if a maker workflow needs elliptical corners.
- Ctrl-constrain ratio presets beyond 1:1 for the rectangle and ellipse tools
  (Inkscape also snaps to the golden ratio, 1:2, 2:1, and others while
  dragging). Only square/circle is supported (criteria 2, 8).
- The ellipse tool's arc/segment/pie sub-modes (dragging out a start/end
  angle to produce an open arc or a pie-slice wedge instead of a full
  ellipse). Full circles/ellipses only.
- The star tool's "rounded" and "randomized" parameters (Inkscape's -10..10
  corner-smoothing and point-jitter controls). Only point count and
  inner/outer ratio are adjustable here (criteria 10, 14, 15).
- Switching an already-created shape's mode between "polygon" and "star".
  Pick the mode before drawing (criterion 10); changing an existing shape's
  mode means redrawing it. Inkscape allows toggling this in place on a
  selected star.
- Numeric entry fields for exact width/height/radius/rx/ry/point-count/ratio
  values. This slice's acceptance criteria require only handle-drag
  adjustment; whether a numeric control also exists is for the ux-engineer to
  decide, not an acceptance criterion here.
- Snapping (to grid, to other objects, to guides) for any of these tools —
  same deferral as `path-node-editing`.
- Keyboard nudging or numeric coordinate entry for a primitive or its
  handles — same deferral as `path-node-editing`.
- Undo/redo of any operation in this slice — `undo-redo` (slice 5).
- Stroke/fill styling beyond the placeholder default (criterion 16) —
  `stroke-and-fill-styling` (slice 4).
- Boolean operations, grouping, layers — slices 6 and 7.
- "Object to path" for text. R-EDIT-004 names "any shape or text"; this
  product has no text tool yet, so text is out of scope until one exists.
- Reverting "object to path" back to a primitive. The conversion is one-way,
  per criterion 17 and R-EDIT-004's own wording; there is no inverse
  "path to primitive" action.
- Multi-path node selection — selecting and editing nodes across more than
  one path object at once, needed to make a multi-object "object to path"
  (criterion 22) leave every result selected together. `NodeSelection`
  (`vecmanf-ui-core`) holds one path at a time; this is deferred to the
  future general selection-tool story slices 2 and 3 already point at.

## UX notes

This slice extends the tool rail, selection language and tokens
`path-node-editing` (slice 2) established, rather than starting a parallel
set — see `docs/design-system.md`, extended alongside this file with the new
tokens referenced below.

### Tool rail additions and shortcuts

- Three new buttons appended below **Pen**, **Node** in the same 48px rail,
  in the order the maker reaches for them most (box/hole shapes before
  polygons): **Pen, Node, Rectangle, Ellipse, Polygon/Star.** Per slice 2's
  own rule, existing icons don't move.
- **Shortcuts: `R` (Rectangle), `E` (Ellipse), `*` (Polygon/Star)** — exact
  Inkscape bindings, same reasoning slice 2 used for `B`/`N`: R-EDIT-002
  points at Inkscape parity the same way R-EDIT-001 did, and a maker
  switching tools gradually should find these where Inkscape trained them,
  including the unusual `*` binding (not a letter, but it's what Inkscape
  uses and "almost matching" would be worse than matching). All three are
  canvas-focus single-letter/symbol shortcuts, same mechanism as `B`/`N`, not
  native-menu accelerators.
- `aria-label`s: "Rectangle tool (R)", "Ellipse tool (E)", "Polygon/star tool
  (*)". Tooltip, active-state fill, and focusability all follow slice 2's
  button convention unchanged.
- None of these three change the default tool on an empty canvas (still
  Pen, per slice 2).

### Live creation feedback (drag-to-create)

Same interaction category as slice 2's node/handle drag, so it gets the same
treatment: **live final-shape outline, not just a bounding box**, rendered
continuously during the drag in slice 2's "in-progress" style (hollow,
`--accent` outline, screen-space-constant stroke weight) — a maker dragging
out a rectangle sees a rectangle updating live, not a placeholder box that
snaps to shape on release. Specifically:

- Rectangle/ellipse: the live outline *is* the final shape at the current
  drag extent (criteria 1, 7) — no separate "preview" rendering path needed
  beyond what slice 2 already draws in-progress geometry with.
- Polygon/star: the live outline recomputes all N (and, for a star, 2N)
  vertices on every pointer-move, at the tool-options bar's current point
  count/mode/ratio (criteria 11, 12) — point count and ratio are fixed
  before the drag starts (AC10, AC12), so only the radius changes live.
- **Numeric readout during the drag**, positioned in a small label near
  point B (on-canvas, not status-bar — direct manipulation keeps the number
  where the maker's eyes already are): "W × H" in the document's display
  unit for rectangle, "rx × ry" for ellipse, outer radius (and, for a star,
  the fixed ratio already set) for polygon/star. This is new relative to
  slice 2 (which has no numeric readout for curve-dragging, because the
  curve shape itself is the feedback) because these shapes are frequently
  cut to a dimension that matters (a box-joint side, a mounting-hole
  diameter) and Inkscape itself shows the equivalent in its own status bar
  during the same drags — on-canvas placement is the one change from
  Inkscape's convention, for consistency with this product's "chrome stays
  out of the way" rule.
- A drag that would create nothing (A = B, criteria 1, 7) shows no preview
  at all, not a zero-size one.

### Corner-radius adjustment (rectangle)

**2026-10-05 note:** the "larger than the node tool's 7px/6px glyphs"
comparison below was true when this slice shipped; the node tool's
handle endpoint has since doubled to 12px (`canvas-interaction-bugs`
follow-up), so the shape handle (unchanged at 8px) is no longer the
larger of the two. `docs/design-system.md`'s own token table is the live
source of truth for current sizes — this file is not updated in place.

**On-canvas handle, not a tool-options numeric field, for this first pass.**
Rationale: this is pure drag-to-create-then-adjust territory, the same
category as slice 2's handle drags, and Inkscape's own rectangle tool
answers this with a handle, not a field — matching precedent beats adding
new chrome the spec doesn't require (the spec explicitly leaves numeric
entry as our call, and "Out of scope" already defers exact numeric entry
generally). Revisit with a numeric field only if the customer asks for
exact radius values.

- **New glyph, deliberately not the node tool's square/diamond/circle
  vocabulary:** an 8px hollow square, `--accent` outline, filled solid
  `--accent` while being dragged — call it the **shape handle**, reused
  below for resize and for the polygon/star ratio handle too. It's larger
  than the node tool's 7px/6px glyphs, always square regardless of what it
  is adjusting (no diamond, no circle), and — critically — it can never be
  on screen at the same time as a node-tool glyph on the same object: a
  primitive is not a path, the node tool has nothing to show on it (see
  below), so the two vocabularies never visually collide, only run
  sequentially as the maker's mental model shifts from "shape" to "path."
- **Placement:** one shape handle inset along the diagonal from the
  rectangle's top-right corner, visible whenever the rectangle tool is
  active and the rectangle is selected — present even at zero radius (AC1),
  so the maker discovers rounding by finding and dragging it, not by
  reading a manual. Once radius > 0, matching (non-draggable, display-only)
  shape-handle glyphs appear at the other three corners too, to confirm
  "all four corners round together" (AC4) without implying four
  independent controls.
- Dragging it out from the corner increases the radius live on all four
  corners simultaneously (AC4); dragging it back onto the corner, or
  "remove rounding," zeroes it (AC6); the half-shorter-side clamp (AC5)
  simply stops the handle's travel at that distance — it doesn't overshoot
  and snap back.
- The connecting guide from corner to handle is a **dashed** `--accent-hover`
  line (1px screen-space) — visually distinct from the node tool's *solid*
  handle line — another deliberate small difference so a maker who's just
  switched from node-editing a different object doesn't misread this as a
  Bézier handle.

### Polygon/star point-count and ratio

**Both a tool-options bar and on-canvas handles — point count lives in the
bar (it must: AC10 requires a persistent control "not reset between
shapes," which only a persistent bit of chrome can be), ratio is adjustable
both ways.**

- A contextual tool-options bar appears under the main menu (same row and
  mechanism as slice 2's node-tool actions bar) whenever the Polygon/Star
  tool is active:
  - **Mode toggle:** two-state segmented control, "Polygon" / "Star" (AC11
    vs. AC12). Switching it only affects shapes drawn *after* the switch —
    an existing shape's mode doesn't change (per "Out of scope").
  - **Point count:** numeric stepper, integer, minimum 3, maximum 1024
    (AC10's crash-safety bound — the stepper itself refuses to go further,
    rather than accepting an out-of-range value and failing some other way
    later), direct type-in or up/down arrows. Persists across shapes and
    across mode changes (AC10).
  - **Ratio (spoke ratio):** numeric field + slider, range 0.01–0.99 (AC12's
    0 < R < 1 taken literally — the field never allows exactly 0 or 1, which
    would collapse the star to a point or a plain polygon). Visible only in
    star mode; disabled/hidden in polygon mode, since a polygon has no
    ratio (AC14).
  - Changing point count or ratio with a shape selected updates it live
    (AC15), same "live" rule as everything else in this slice.
- **On-canvas, with the tool active and a shape selected:** four shape-handle
  glyphs (same 8px square as the rectangle's) at the outer bounding circle's
  N/E/S/W-most points, any one draggable to scale the whole shape uniformly
  (AC13 — outer radius changes, inner radius follows to keep the ratio, N
  and orientation unchanged). For a star only, one additional shape handle
  sits at the first inner vertex (clockwise from the topmost outer vertex);
  dragging it changes the ratio live and writes the new value back into the
  tool-options bar's ratio field (AC14) — the two controls stay in sync, not
  independent sources of truth. A plain polygon never shows this fifth
  handle, per AC14.

### Primitive vs. path: handles don't coexist

Once "object to path" runs (AC17), the primitive's entire adjustment
vocabulary — shape handles (resize, corner-radius, inner-radius), and the
tool-options bar's point-count/ratio fields if the polygon/star tool is
still active — **disappears outright**, with no equivalent reappearing.
What takes over is exactly slice 2's ordinary node/handle glyphs (corner and
smooth node squares/diamonds, circular Bézier handle endpoints), on the new
path object, shown under the node tool exactly as any other path. Implementer
note: don't keep both affordances alive even transiently during the
conversion — the shape handles for a primitive and the node/handle glyphs
for a path are two different vocabularies for two different object types
that are mutually exclusive by construction (a thing is a primitive or a
path, never briefly both), not two skins over the same adjustable object.

The inverse also holds and is worth stating explicitly since it's easy to
miss: a primitive is not a path, so **the node tool shows nothing on a
primitive** — no nodes, no handles, nothing to click. If the maker selects
a primitive and switches to the node tool, the shape renders plain (its
criterion-16 stroke only) with no editing glyphs at all. This isn't a new
restriction so much as a direct consequence of "object to path" being an
explicit, one-way action (AC17, AC21): there is nothing for the node tool to
show until that action runs.

### Selection and hover convention for primitives

**Bounding-box-with-handles, not slice 2's node-by-node selection** — a
deliberate, precedent-setting fork, because a primitive's identity really is
"a shape with width/height" (plus, for rectangle/polygon/star, a couple of
scalar parameters), not a set of independently-selectable points the way a
path's anchors are. Concretely:

- Selected primitive, matching tool active: the shape's own criterion-16
  stroke renders as usual, plus its bounding box outlined with a 1px
  `--accent` line (screen-space) and shape handles at the positions
  described above (8 for rectangle/ellipse, 4 or 5 for polygon/star). There
  is no concept of selecting "one corner" independently of the others —
  the whole object is the selection unit, consistent with there being no
  per-node identity on a primitive at all.
- Hover (matching tool active, not yet selected): bounding box outlined at
  `--accent-hover` (the existing 20%-opacity rule), no handles yet — handles
  are a selected-state affordance, same as slice 2's "handles show only when
  selected" rule for path nodes.
- **Tool mismatch:** per slice 2's own deferral (no general selection tool
  exists yet), a primitive's selection visuals — bounding box and handles —
  only render while its *own* matching tool (Rectangle/Ellipse/Polygon-Star)
  is active. Switching to Pen, Node, or a different shape tool hides them;
  this mirrors the same gap slice 2 already accepted for paths and the node
  tool, not a new one invented here. Revisit both together once a general
  selection tool exists.
- This is the convention every later "it's a shape with width/height" object
  inherits (per this file's own framing); slice 2's node-by-node convention
  remains what every path-shaped object inherits. Two conventions, each
  scoped to the object kind it actually fits — not a single convention
  stretched to cover both.

### New/extended tokens (see `docs/design-system.md`)

Added there alongside this file rather than inline here, per slice 2's own
rule: the **shape handle** glyph (8px hollow square / filled-on-drag,
`--accent`), the dashed corner-radius guide line, the bounding-box selection
outline, and the `R`/`E`/`*` shortcuts and rail order above.

## Links
Requirements: R-EDIT-002, R-EDIT-004 (`docs/requirements.md`)
PR: https://github.com/Th3Link/vectormanufactoring/pull/10
