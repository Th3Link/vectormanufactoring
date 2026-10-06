# Object transform: move, scale and rotate via on-canvas handles

Status: Ready
Priority: Must
Origin: Customer

## User value

As a maker I want to resize and rotate any selected object — a path or a
primitive — directly on the canvas with drag handles, the same way Inkscape,
Illustrator and Figma already let me, so that adjusting a shape's size or
orientation after I've drawn it is a direct drag, not a redraw, and works
identically whether the object started as a pen path, a rectangle, an
ellipse or a polygon/star.

The customer's own words: "Shapes and paths should also be movable, scalable
and rotatable." Move already exists:
`specs/0004-canvas-navigation-and-selection/specification.md` criterion 20
covers dragging a selected object's body, and this slice does not redefine
that — criterion 24 below only confirms it keeps working once handles exist
alongside it. **Scale and rotate are new**, and slice 4 explicitly
anticipated them: its Out-of-scope list named "Resize/rotate handles on the
Select tool's own bounding box" as "a real feature but not what either
customer complaint asked for. Revisit as its own slice if asked." This is
that slice. It extends, rather than conflicts with, slice 4 criterion 20's
"the Select tool shows no shape handles... only the bounding box" — that
sentence described a selection with no transform feature yet; this slice is
the point at which the bounding box itself grows handles. The `ux-engineer`
should treat slice 4's UX notes on the plain bounding-box indicator as the
*unselected/no-transform-feature* baseline this slice's criteria extend, not
as a contradiction to resolve silently.

**Handle convention adopted, and why:** every actively maintained reference
tool places 8 resize handles at a selection box's corners and edge
midpoints (corner = free resize, edge = single-axis resize) — Inkscape,
Illustrator and Figma all agree here, so this is not a design choice, it's
the baseline. Where they diverge is rotation: Inkscape requires a *second
click* on an already-selected object to swap its corner arrows into
rotate/skew arrows; Illustrator and Figma instead show a small, separate
rotate handle alongside the resize handles at all times, no mode switch.
This product adopts the **Illustrator/Figma always-visible convention**: a
dedicated rotate handle, offset a fixed screen distance above the top-edge
handle, shown together with the 8 resize handles the moment a single object
is selected. Reasons: (1) this product's own precedent
(`primitive-shapes`' shape handles) is already "every handle visible at
once while selected," not a mode the maker toggles into — a second click to
reach rotation would be the one inconsistent interaction in the product;
(2) Inkscape's click-to-toggle is a frequent point of confusion for makers
arriving from Illustrator/Figma/Affinity, and this product already breaks
from Inkscape parity when matching it would read as worse, not better
(see slice 4's own Blender-navigation precedent); (3) the customer's own
loosely-specified idea elsewhere in this conversation referenced Affinity
Designer by name, so Affinity/Illustrator/Figma's mental model is already
the one the customer reaches for.

Scale and rotate **keyboard modifiers are Inkscape's own bindings**,
adopted as-is because this product already uses Ctrl this way
(`primitive-shapes` criteria 2 and 8, "the constrain modifier (Ctrl)"):
**Ctrl constrains** (proportional resize on a corner handle; 15° rotation
steps on the rotate handle), **Shift swaps the pivot** (the object's own
center instead of the opposite corner/edge, when scaling; the opposite
corner instead of the center, when rotating). One rule, two modifiers,
identical meaning on every handle.

**Architecture flag — settled for paths in `adrs.md`:** rotation is new
document-model data, not just a UI feature. `specs/0003-primitive-shapes/
adrs.md` and `specs/0004-canvas-navigation-and-selection/adrs.md` both note
that ADR 0002 §5's per-node affine transform "is still not implemented" and
becomes due "with the first story that rotates or scales an object as a
whole" — this is that story. A **path** bakes its rotation directly into
its anchor points and handle vectors the moment a rotate drag completes
(criterion 20), the same way slice 4 bakes a move into anchor `point`s —
but, unlike a move, it additionally stores its own cumulative rotation
angle in a `rotation` register, used only to keep its selection box
oriented across a later deselect/reselect (criterion 18); the baked anchors
remain the sole source of truth for the path's actual geometry, never
derived from this register. A **primitive** cannot bake rotation into
anything at all: `rect_bounds`, `ellipse_frame` and `star_frame` are
axis-aligned by construction, so representing "this rectangle, rotated 30°"
needs either a new stored angle alongside each frame, or ADR 0002 §5's
general per-node transform finally being built. Converting a rotated
primitive to a path implicitly is not an option — `primitive-shapes`
criterion 21 already rules out any implicit primitive→path conversion, and
nothing here asks for an exception. Which exact mechanism a primitive uses
(a per-shape angle field vs. the general per-node transform) is still the
architect's call; criterion 19 below states the observable requirement
(rotation persists, the object stays the same kind) without dictating
which.

A second, related consequence this spec resolves rather than leaving open:
once an object has a non-zero rotation, its selection box and resize
handles **follow the object's own orientation** (an oriented bounding box,
Figma's convention), not a box re-squared to the screen axes (one reading
of Illustrator's default). This is deliberate, not cosmetic: resizing along
the object's own local axes can always be expressed as "new width/height/rx/
ry plus the same angle" — it never needs shear. Resizing along the *screen's*
axes after a rotation, by contrast, turns a rotated rectangle into a
parallelogram, which this product's primitives cannot represent and which
skew/shear (explicitly out of scope, see below) is not being added to
support. Adopting the oriented-box convention is what keeps "scale" and
"rotate" two independent, always-combinable operations with no third,
unrequested capability smuggled in to make them compose.

## Acceptance criteria

### Reachability — extends slice 4's selection indicator

1. Given the Select tool is active with exactly one object (a path or a
   primitive, in any state) selected and that object's rotation is zero,
   then slice 4 criterion 14's plain bounding box grows 8 resize handles —
   one at each corner, one at the midpoint of each edge — plus one rotate
   handle offset a fixed screen-space distance above the top-edge handle's
   midpoint, using the same hit size as `primitive-shapes`' own shape
   handles, whatever that is at build time (`docs/design-system.md` is the
   live source — it was 8px when this slice was written but has since
   doubled to 16px alongside the node/handle sizing round, 2026-10-05).
   This is the only change to slice 4 criterion 20's
   "the Select tool shows no shape handles... only the bounding box"
   wording; nothing else about that criterion changes.
2. Given the Select tool is active with two or more objects selected (slice
   4 criterion 17), then no transform handles appear on any of them — each
   object keeps slice 4's plain per-object bounding box only. Move (slice 4
   criterion 18) and delete (criterion 19) continue to work exactly as
   specified; scaling or rotating a multi-object selection is out of scope
   for this slice (see "Out of scope").
3. Given any handle described in criteria 1–20, when the maker presses it
   and releases without moving the pointer, then nothing is written to the
   document — the same "a press and release with no movement writes
   nothing" rule every earlier slice already follows.

### Scale — corner and edge handles

4. Given a selected rectangle, ellipse or path with zero rotation, when the
   maker drags a corner handle with no modifier held, then the object
   resizes freely (its two dimensions change independently to follow the
   drag), anchored at the opposite corner — the dragged corner follows the
   pointer 1:1, the opposite corner stays fixed.
5. Given the same context, when the maker holds Ctrl while dragging a
   corner handle, then the resize is constrained proportionally (both
   dimensions scale by the same factor, matching the drag's dominant axis)
   — Inkscape's own binding, and the same modifier `primitive-shapes`
   criteria 2 and 8 already use for "constrain to square/circle."
6. Given a selected rectangle, ellipse or path, when the maker drags an
   edge handle, then only the dimension perpendicular to that edge changes
   (width for a left/right edge handle, height for a top/bottom one),
   anchored at the opposite edge; Ctrl has no additional effect on an edge
   handle, since there is only one axis to constrain.
7. Given any corner or edge handle drag (criteria 4–6), when the maker
   holds Shift, then the resize is anchored at the object's own bounding-
   box center instead of the opposite corner/edge — the object grows or
   shrinks symmetrically around its center — matching Inkscape's own
   "Shift scales from the center" binding. Shift and Ctrl combine: Shift
   held with Ctrl on a corner handle scales proportionally from the center.
8. Given any object with a non-zero stroke width and the "Scale stroke
   width" switch off (its default, criterion 27), when it is resized by any
   handle in criteria 4–7 — proportional, free or single-axis, anchored at
   a corner, an edge or the center — then its stroke width afterwards is
   exactly the value it had before the drag: the stored stroke width is not
   rewritten, and the stroke renders at the same absolute thickness (a
   rectangle with a 1 mm stroke resized to 300% of its size still has a
   1 mm stroke). This is a deliberate departure from Inkscape's default
   (its "scale stroke width" toggle defaults to on), made at the customer's
   request on 2026-10-06 after testing: a stroke that thickens with every
   resize is not wanted. The switch's on state is criterion 26.
9. Given a rectangle with a non-zero corner radius, when it is resized by
   any handle in criteria 4–7 with equal horizontal and vertical scale
   factors, then the corner radius scales by that same factor (clamped per
   `primitive-shapes` criterion 5's existing half-shorter-side rule,
   evaluated after scaling). Given a resize with different horizontal and
   vertical scale factors sx and sy, then the corner radius scales by
   √(sx·sy) instead — the same geometric-mean rule criterion 26 uses for
   stroke width when its switch is on, and for the same reason: a single
   circular-radius corner (`primitive-shapes`
   criterion 4's "one radius, not an independent horizontal/vertical pair")
   has no other way to represent an anisotropic resize, so its one radius
   reflects the resize's overall magnitude rather than either axis alone;
   the half-shorter-side clamp above still applies afterward, and a radius
   driven to exactly 0 this way is a valid sharp corner, not refused —
   unlike criterion 26's stroke width, 0 is a meaningful value here
   (`primitive-shapes` criterion 6). This matches Inkscape's "scale rounded
   corners" toolbar toggle, which defaults to on. **The corner radius keeps
   scaling in every case, whatever the "Scale stroke width" switch says**
   (criterion 31) — the customer's 2026-10-06 change concerned stroke width
   only, and radius scaling has no switch. This is a deliberate
   difference from `primitive-shapes`
   criterion 3, where the Rectangle tool's *own* corner-drag resize keeps
   the radius's absolute length — that criterion is unchanged and still
   governs the Rectangle tool's own handles; this criterion only governs
   the Select tool's transform handles, exactly mirroring Inkscape's own
   split between its Selector tool (scales the radius) and its Rectangle
   tool (does not).
10. Given a selected ellipse, when resized by any handle in criteria 4–7,
    then rx and/or ry change exactly as `primitive-shapes` criterion 9
    already defines for the Ellipse tool's own handles — this slice adds no
    new ellipse-resize rule, only a second way (the Select tool) to reach
    the one that already exists.
11. Given a selected polygon or star, then it shows **corner handles only
    — no edge handles** — and dragging any corner handle always performs
    the uniform scale `primitive-shapes` criterion 13 already defines
    (outer radius changes; for a star, the inner radius scales with it so
    the ratio is unchanged; point count and orientation unchanged),
    regardless of whether Ctrl is held. A polygon/star's stored shape is a
    single radius; it cannot represent a non-uniform resize without
    becoming an irregular shape no longer expressible by that schema (see
    "Out of scope" — this is not an oversight, it is the schema's actual
    limit).
12. Given a selected path, when resized by any handle in criteria 4–7, then
    every anchor's point moves to the position that scaling its offset
    from the drag's anchor point (the opposite corner/edge, or the
    object's center under Shift) by the drag's per-axis factor would
    produce, measured along the object's own local axes (identical to the
    document axes when its rotation is zero); each anchor's `handle_in`
    and `handle_out` vectors (already stored relative to their anchor,
    `path-node-editing`'s decision) scale by the same per-axis factors,
    unchanged otherwise — this produces the exact scaled Bézier curve, not
    an approximation, because scaling a cubic Bézier's control points by a
    fixed affine map scales the curve it describes by the same map.
13. Given a resize drag that would take an object's width, height, or (for
    an ellipse) rx/ry below 0, then the dimension clamps to 0 and the drag
    has no further effect in that direction until the pointer moves back
    past the zero crossing — the object never reports or renders a
    negative size.
14. Given a scale drag in progress (criteria 4–13), then a live numeric
    readout of the object's current size is shown on canvas, following the
    same convention `primitive-shapes`' drag-to-create readout already
    established for this product; exact placement, units display and
    digit precision are the `ux-engineer`'s call, same as that earlier
    readout.

### Rotate

15. Given a selected object (criterion 1), when the maker drags the rotate
    handle, then the object rotates live to follow the drag, pivoting
    around the object's own bounding-box center by default.
16. Given a rotate drag, when the maker holds Shift, then the pivot moves
    to the **bottom-edge midpoint** — the point directly opposite the
    rotate handle across the bounding box's center — instead of the box's
    center. The rotate handle sits at the top-edge midpoint (criterion 1),
    not a corner, so it has no corner to be "opposite"; this is Inkscape's
    own "Shift rotates around the opposite corner" binding, applied here as
    "the opposite edge-midpoint" since that is the one point this handle's
    position actually has an opposite of.
17. Given a rotate drag, when the maker holds Ctrl, then the rotation
    snaps to 15° increments from the object's angle at drag-start —
    Inkscape's own default snap-angle step.
18. Given an object with a non-zero rotation (set by criteria 15–17), when
    the maker selects it again later (including after a resize, a pan/
    zoom, or reselecting it from scratch), then its selection box and
    resize handles are oriented to match the object's own rotation, not
    re-aligned to the screen's axes — this is what keeps a subsequent
    non-uniform resize (criteria 4, 6, 12) representable as a new width/
    height/rx/ry at the same angle, never as a sheared shape.
19. Given a rotated primitive (rectangle, ellipse, polygon or star), when
    the maker saves, closes and reopens the project, then the primitive's
    rotation is exactly what it was before closing, and the object is
    still the same kind of primitive (not converted to a path) — this is
    the document-model requirement flagged above; how the angle is stored
    is the architect's decision, not pinned here.
20. Given a rotated path, then its rotation is baked into its anchor
    points and handle vectors at the moment the rotate drag completes
    (every anchor's point rotates around the pivot; every handle vector
    rotates by the same angle, needing no pivot since it is already
    relative to its anchor) — consistent with how slice 4 bakes a move
    into anchor positions, extended here to rotation. The path additionally
    stores its own cumulative rotation angle in a `rotation` register, used
    only to keep its selection box oriented on a later reselect
    (criterion 18) — it is never consulted to render, hit-test, export or
    otherwise reconstruct the path's geometry, which is already correct in
    the baked anchors.
21. Given any primitive, when it is rotated by criteria 15–17, then it
    remains a primitive of the same kind afterward — rotating is not an
    implicit trigger for "object to path," consistent with
    `primitive-shapes` criterion 21's existing rule that only the explicit
    "object to path" action converts a primitive.
22. Given a rotate drag in progress, then a live numeric readout of the
    object's current rotation angle is shown on canvas, following the
    same convention as criterion 14; exact placement and precision are the
    `ux-engineer`'s call.

### Move — reference, not redefined

23. Given the Select tool with exactly one object selected, dragging the
    object's body (not a handle) continues to move it exactly as slice 4
    criterion 20 already specifies, live and 1:1, regardless of whether
    that object has a non-zero rotation or has been scaled by this slice —
    a move is always a pure translation and never changes an object's
    rotation or size. This slice defines no new move behaviour.

### Persistence

24. Given an object scaled and/or rotated by this slice, when the maker
    saves, closes and reopens the project, then a path's resulting anchor
    geometry and its stored `rotation` register (criterion 20), and a
    primitive's frame plus corner radius/inner ratio/point count plus
    rotation angle, are exactly what they were before closing — no silent
    loss of a scale or a rotation on reopen.

### Primitives' own tool handles follow rotation

25. Given a rotated primitive (rectangle, ellipse, polygon or star), when
    the maker double-clicks it with the Select tool active and slice 4
    criterion 23's handoff switches to that primitive's own creation tool,
    then that tool's own handles — the rectangle's corner-radius handle and
    its three display-only corner echoes (`primitive-shapes` criteria 4/6);
    the polygon/star's compass-point and inner-ratio handles (criterion 14)
    — render and hit-test at the positions implied by the primitive's
    current rotation, not as if it were unrotated. A corner-radius handle
    on a rectangle rotated 30° sits inset along that rectangle's own
    rotated corner, 30° off where it would sit on an unrotated rectangle of
    the same size — the same "handles follow the object's local frame"
    rule criterion 18 already states for the Select tool's transform
    handles, extended here to each primitive's own shape-tool handles so
    the two handle sets never disagree about where the object's corners
    actually are.

### Scale stroke width switch (added 2026-10-06, customer feedback)

26. Given the "Scale stroke width" switch is on and an object with a
    non-zero stroke width, when it is resized by any handle in criteria
    4–7 with equal horizontal and vertical scale factors (a proportional
    corner resize, criterion 5, or a free resize where the drag happens to
    keep sx = sy), then the stroke width scales by that same factor (a
    resize to 150% of the original size gives a stroke 150% as wide). Given
    different factors sx and sy (a free corner resize, criterion 4, where
    they differ, or any edge-handle single-axis resize, criterion 6), then
    the stroke width scales by √(sx·sy) instead — Inkscape's rule for this
    case, so a stroke's weight reflects the resize's overall magnitude
    rather than either axis alone. The resulting stroke width is never
    below 0.01 mm: where either rule would give less (including zero), it
    is 0.01 mm, and a zero or negative width is never written
    (`stroke-and-fill-styling` refuses to open a file that contains one).
    This is the behaviour this slice shipped before the switch existed.
27. Given a new editor session (every app launch, and every project opened
    or created), then the "Scale stroke width" switch is off. It is never
    read from a project file.
28. Given the switch is toggled, then it governs the next resize drag
    only: a resize drag uses the switch's state at the moment the maker
    pressed the handle for its whole duration, so a change made while a
    drag is in progress does not alter that drag's stroke width, live
    preview or commit, and applies from the next drag on.
29. Given the switch is toggled on or off, then nothing is written to the
    document (no geometry or style change, no document-model field), and
    saving a project with the switch on produces a file byte-identical to
    saving it with the switch off. After closing and reopening the project,
    or restarting the app, the switch is off (criterion 27).
30. Given the Properties panel is visible, then it contains a section
    titled "Transform" with a switch labelled "Scale stroke width". The
    switch is operable whatever is selected, including nothing (it is tool
    state, not an object property), shows its current state at all times,
    and works from the keyboard (Tab to focus, Space to toggle).
31. Given a rectangle with a non-zero corner radius, when it is resized
    with the switch in either state, then the corner radius scales exactly
    as criterion 9 says. The switch has no effect on the radius, and no
    other property (dash lengths, stroke color, opacity) is touched by it.

## Out of scope

- **The circle-to-star morph handle** the customer separately raised
  (an Affinity-Designer-style handle that turns a circle into an inward/
  outward-pointing star). The customer immediately flagged this as unsure
  whether it belongs on the circle or the rectangle. Treated here as a
  flagged idea for a future `Proposal`-origin spec, not a criterion — it
  needs the customer's own follow-up before it is specified at all, and it
  is a different kind of feature (a shape-identity morph, not a transform)
  from everything else in this slice.
- **Skew/shear.** Not requested here, and this slice's oriented-bounding-box
  convention (criterion 18) is specifically designed so scale and rotate
  never need it. The customer has since asked for something close to it
  (2026-10-06); that is open in `specs/object-transform-refinements/`.
- **Numeric transform entry** (typing an exact rotation angle, width or
  height into a field instead of dragging). Same deferral
  `primitive-shapes` and `canvas-navigation-and-selection` already made for
  their own numeric entry; criteria 14 and 22's live readouts are
  display-only. (The customer asked for typed entry on 2026-10-06; it is
  specified separately in `specs/object-transform-refinements/`, not in
  this PR.)
- **Multi-object transform** — scaling or rotating two or more selected
  objects together, whether as one rigid group sharing a single bounding
  box or as each object transforming independently around its own center.
  Criterion 2 keeps multi-select exactly at slice 4's move/delete-only
  capability. Revisit as its own slice once the customer asks, since "one
  shared box vs. independent" is itself a real design question this slice
  does not need to answer yet.
- **Transform history or presets** (Illustrator's "Transform Again,"
  saved/reusable transform values). Not requested.
- **Non-uniform (non-proportional) resizing of a polygon or star.**
  Criterion 11 is the full extent of polygon/star resizing in this slice.
  Making a polygon or star stretch non-uniformly would need either a
  schema change (independent x/y radii, which stops being "a regular
  polygon/star") or an implicit conversion to a path, and neither is in
  scope. A maker who wants that result can invoke "object to path" first,
  then resize the resulting path per criterion 12.
- **A toggle for corner-radius scaling.** Criterion 9 adopts Inkscape's
  own default-on behaviour outright; no switch is added to turn it off. If
  the customer wants one, it becomes a second switch in the Transform
  section next to "Scale stroke width" (criterion 30). (The stroke-width
  toggle is no longer out of scope: criteria 8 and 26–31, 2026-10-06.)
  Customer decision 2026-10-06: the corner-radius switch is deferred to the
  rework of the primitives; until then the radius keeps scaling as in
  criterion 9.
- **Saving the "Scale stroke width" switch in the project file, or making
  it per object.** It is session state (criterion 29, ADR 0009 §2:
  ephemeral); persisting it would add a document-model field nobody asked
  for.
- **Snapping of any kind during a scale or rotate drag** — to grid, to
  other objects, to guides, or to any rotation angle other than Ctrl's 15°
  steps (criterion 17). Same deferral as every earlier slice.
- **Keyboard-driven transform** (arrow-key resize or rotate nudges). Not
  requested by the customer; left for the `ux-engineer` to propose later
  if judged trivial and low-risk, same stance `canvas-navigation-and-
  selection` took on keyboard zoom shortcuts.
- **Tilting an ellipse's own rx/ry axes independently of whole-object
  rotation** (an ellipse whose major axis isn't aligned with its own
  rotation, as if rx/ry and the object's angle were two separate rotations).
  Object rotation (criteria 15–20) is the only orientation concept this
  slice adds.

## UX notes

This slice is where `canvas-navigation-and-selection`'s plain bounding-box
indicator grows handles. That spec's UX notes fixed the *unselected/
no-transform* baseline (plain box, `--accent`/`--accent-hover`, no handles);
nothing below changes that baseline for multi-select (criterion 2) — it only
applies once a single object is selected and the transform handles in
criterion 1 are added on top of the same box.

**Coordinating with the handle-sizing fix, now settled:** `fix/canvas-
interaction-bugs` doubled the node tool's Bézier handle (6px→12px
diameter, hit radius 8px→16px) and, in a later round of the same fix,
the node glyph itself (7px→14px, hit radius 8px→16px) — and that second
round's size change propagated into `primitive-shapes`' shape handle too
(its hit radius is computed from the same shared tolerance function), so
it is now 16px hit radius as well, not the 8px this spec was written
against. Criterion 1's "same hit size as `primitive-shapes`' own shape
handles, whatever that is at build time" (amended above, 2026-10-05)
already accounts for this — build against `docs/design-system.md`'s
current value, not the number anywhere in this paragraph. The remaining
coordination is keeping the new glyph's *silhouette* distinct from both
existing vocabularies, which may now be the same size as each other even
though they started different — the decisions below still achieve that
through shape, not size.

### Handle glyph set — a third vocabulary, deliberately distinct from the other two

Three handle vocabularies now coexist, and only one is ever on screen at a
time (only one tool is active at once: Node tool shows Bézier handles,
a shape tool shows shape handles, Select tool shows transform handles) —
the same non-collision argument `primitive-shapes` already made for why its
own shape handle didn't need to visually diverge from the node tool's
glyphs. That argument still holds here, but this slice makes a visually
distinct choice anyway, both because three is enough vocabularies that a
maker's muscle memory benefits from each one reading as "its own thing" on
sight, and because a future slice (multi-object transform, or a new object
type) is more likely to need two of these visible together than any
previous slice was.

- **8 resize handles (corner + edge-midpoint):** 8×8px screen-space
  **rounded-corner square ("squircle," 2px corner radius)** — hollow,
  `--accent` stroke, white fill idle; solid `--accent` fill while being
  dragged. Same fill-state rule as every other handle in the product (idle
  hollow/white, active solid `--accent`); same 8×8px footprint as the
  `primitive-shapes` shape handle (criterion 1 pins the hit size, and
  reusing the footprint keeps "square silhouette = a resize control" one
  consistent visual family across both vocabularies) — but the rounded
  corner is the one deliberate difference, so the two are never
  pixel-identical even though (per above) they can never be mistaken for
  each other in context. Corner and edge-midpoint handles render
  identically; position alone conveys which does a free vs. single-axis
  resize, the same way the shape handle's own corner/edge positions already
  do their own job without a glyph-level distinction.
- **1 rotate handle:** a **12×12px circular-arrow icon glyph**, not a dot —
  `--accent` stroke / transparent fill idle, `--accent-hover` fill on hover,
  solid `--accent` fill with the glyph rendered white while dragging. Two
  deliberate choices here: (1) an icon rather than a plain circle, because a
  plain circle at any size risks reading as "a bigger version of the
  Bézier-handle endpoint" the moment the sizing fix above lands (that
  endpoint becomes a 12px circle too); an icon is unambiguous regardless of
  size. (2) **No connecting stalk line to the bounding box** — a
  line-plus-circle composition is exactly what a Bézier handle already is
  (handle line + handle endpoint), and reusing that composition for an
  unrelated control would be the one collision this slice should avoid on
  purpose, even though the two still can't appear on the same screen at
  once. The rotate handle instead floats on its own, positioned by offset
  alone (next section).
- Hit-test radii: 8px for each resize handle (criterion 1, pinned by the
  spec itself); 12px for the rotate handle, scaled to match its own larger
  12px glyph, the same margin-rule instinct `docs/design-system.md` already
  applies to every other handle in the product.

### Rotate handle placement

**Illustrator/Figma arc-icon convention, floating independently, no stalk**
(the specification's own §"Handle convention adopted" already settled
*that* a dedicated, always-visible handle is used; this is the exact
placement). Center-to-center offset: **20px screen-space** from the
top-edge resize handle's center, measured along the bounding box's own
local "up" axis (the box's own top direction, which equals the screen's "up"
only at zero rotation — see "Oriented bounding box" below). 20px is chosen
to clear the 8px resize-handle glyph and its 8px hit radius with a few
pixels of dead space on either side, so the two hit areas (8px and 12px
radius, 20px apart center-to-center) never overlap at any rotation.

### Cursor feedback

- **Each of the 8 resize handles** shows a **custom, rotated double-headed-
  arrow cursor** — not one of the browser's four fixed resize cursors
  (`nwse-resize`/`nesw-resize`/`ns-resize`/`ew-resize`), because the oriented
  bounding box (criterion 18) means a handle's actual screen-space direction
  is `object rotation + handle's own base angle (0°/45°/90°/.../315°)`,
  which is almost never one of those four fixed angles once the object is
  rotated. The cursor is a custom cursor image (the same double-headed-arrow
  glyph used at 0° rotation) rotated live to match that computed angle, set
  via CSS `cursor: url(...) <hotspot-x> <hotspot-y>, <fallback>` with
  `nwse-resize`/`ew-resize` as the nearest-angle fallback for platforms that
  ignore custom cursor images. Recomputed on every frame the object's
  rotation or the specific hovered handle changes — cheap, since it's just
  picking/rotating a glyph, not rebuilding geometry.
- **The rotate handle** shows a distinct, **non-rotating** circular-arrow
  rotate cursor (the same glyph as the handle icon itself, enlarged
  slightly for legibility as a cursor). It does not rotate with the object —
  unlike a resize direction, "rotate" has no single axis to align a cursor
  to, so every reference tool that has a dedicated rotate cursor keeps it
  static regardless of handle angle.
- Both cursor types apply only while the pointer is within the handle's own
  hit radius (8px or 12px per above); outside every handle but inside the
  bounding box, the cursor is the Select tool's normal move/arrow cursor for
  dragging the object's body (unchanged, criterion 23).

### Live numeric feedback during drag

**On-canvas only, next to the pointer — not the Properties panel, in this
slice.** This follows the `primitive-shapes` drag-to-create readout
precedent exactly (on-canvas, near the actively-manipulated point, "direct
manipulation keeps the number where the maker's eyes already are") with one
adjustment: that readout sat near the fixed point B of a creation drag; a
transform drag's own "point B" (the dragged handle) itself moves and, for
rotation, swings in a circle, so the label is anchored to the **pointer**
with a constant 12px screen-space offset (up-and-right, same offset
`primitive-shapes` used) rather than to the handle's own position — easier
to read while it's travelling in an arc.

- **Scale drag (criterion 14):** shows absolute size, not a percentage —
  "**123.4 mm × 67.8 mm**" (or the document's current display unit) for a
  rectangle, ellipse or path, matching `primitive-shapes`' own "W × H"
  format and digit precision exactly, because a laser maker cutting
  physical material cares about the resulting millimetre size, not a
  relative percentage of whatever the object happened to start at. (This is
  a deliberate departure from the "150% × 120%" phrasing floated when this
  task was framed — percentage is the wrong unit for a tool whose whole
  point is physical dimensions.) For a polygon/star (criterion 11, uniform
  scale only) the readout shows the single outer-radius value, exactly as
  `primitive-shapes`' own creation-drag readout already does for that tool.
- **Rotate drag (criterion 22):** shows the current angle, "**37.4°**" to
  one decimal place when unconstrained, falling out to a whole multiple of
  15 with no decimal ("45°") under Ctrl's snap (criterion 17) since the
  value is then exact.
- **Why not also in the Properties panel:** criteria 14 and 22 both say "on
  canvas" already, settling placement; duplicating a live readout into the
  panel would add chrome with no interaction benefit until numeric *entry*
  exists (explicitly out of scope here), and the panel's own role per
  `docs/design-system.md`'s 2026-10-05 chrome section is persistent,
  selection-independent configuration, not a second copy of a transient,
  drag-duration number that's already visible at the maker's cursor.
  Revisit placement once a future slice adds typed numeric transform entry
  — that one plausibly does belong in the Properties panel as a "Transform"
  section, since typed entry is exactly the kind of persistent, addressable
  control the panel exists for. (The "Transform" section now exists for the
  stroke switch below; typed entry would join it.)

### "Scale stroke width" switch placement (2026-10-06, for `ux-engineer` review)

Proposed by the product owner, not yet reviewed.

- **Where:** the right-hand Properties panel (`docs/design-system.md`:
  one scrolling column of named, stacked sections, 280px, docked), in a new
  section titled **"Transform"**, below the "Style" section. The switch is
  persistent tool configuration that must survive selection changes, which
  is the panel's stated role; a floating per-selection mini-toolbar is the
  wrong home because the switch has to be settable before anything is
  selected and must not appear and vanish with the selection.
- **Control:** one labelled switch, "Scale stroke width", off by default,
  with the existing shadcn-style switch/label pairing the panel uses for
  other booleans. No icon, no extra explanatory text beyond an optional
  one-line hint ("Off: a resize keeps stroke thickness").
- **Section state:** per the panel's no-collapse rule the section is always
  present and the switch always enabled (criterion 30). It is not a
  "disabled when nothing is selected" control, because it configures the
  next drag, not an object.
- **Dependency to resolve:** the Properties panel is first built by
  `stroke-and-fill-styling` and does not exist in the app yet. If this
  slice ships before it, the `ux-engineer` picks the interim host for the
  same switch (the criteria only require that it exist, is reachable and
  labelled as in criterion 30); the Transform section moves into the panel
  when the panel lands.
- **Feedback:** the switch's own state is the feedback; no on-canvas
  indicator. The live size readout (criterion 14) is unchanged.

### Oriented bounding box: handles and cursors rotate with the object

Confirmed, not optional: once an object has a non-zero rotation (criteria
15–20), every handle position in this spec is computed in the object's own
local, rotated coordinate frame and then mapped to screen space by the
current rotation (plus pan/zoom) — never computed as if the box were
screen-axis-aligned and then rotated as a finishing step, and never
re-squared to the screen axes on reselection (criterion 18). Concretely:

- The handle labelled "top-center" is always at the visual top of the
  rotated shape, not the screen's top; the same holds for every corner and
  edge handle and for the rotate handle's 20px offset (measured along the
  box's own "up," per above).
- Each resize handle's custom cursor rotates by the object's current
  rotation angle plus that handle's own base angle (above) — a "top" handle
  on a 45°-rotated object shows a cursor pointing along that 45° diagonal,
  not a plain `ns-resize`. This is what keeps the cursor direction matching
  the direction the handle will actually move the shape in, which is the
  entire point of a direction-specific cursor; a screen-axis-aligned cursor
  on a rotated object would point the wrong way and read as a bug.
- This falls out of treating rotation as a property of the object's local
  frame throughout (the same frame criterion 12's scaled-path-anchor math
  and criterion 20's baked-rotation math already use) — there is no separate
  "handle layout" code path that could drift out of sync with it.

### Modifier-key visual feedback: one pivot marker, not per-modifier chrome

**A single small pivot-point marker, shown for the duration of any scale or
rotate drag, regardless of which modifiers are held** — not a dedicated
Ctrl indicator and a dedicated Shift indicator as two separate pieces of UI.
Reasoning:

- **Shift (pivot swap, criteria 7 and 16)** genuinely needs a visual
  answer: "where is this scaling/rotating from now" is not reliably
  guessable by eye once the box itself is moving, especially for rotate's
  "opposite corner" pivot. The marker — a 6px-diameter dot, `--accent` at
  60% opacity, screen-space constant size — is drawn at whichever point is
  currently the active pivot (object center by default; the opposite
  corner/edge the instant Shift is held; back to center the instant it's
  released, live, same frame). The marker's own position is the feedback
  for Shift's state — no separate badge, label or cursor change is added on
  top, because the marker already answers the only question Shift's
  feedback needs to answer.
- **Ctrl (constrain proportions / 15° snap, criteria 5 and 17)** needs no
  separate overlay: the resulting effect — the resize visibly tracking the
  drag's dominant axis proportionally, or the rotation visibly jumping in
  15° steps instead of following the pointer continuously — is itself
  immediate, continuous feedback, the same reasoning
  `canvas-navigation-and-selection`'s UX notes already used for scroll-pan
  and Ctrl+scroll-zoom ("the view moving... is feedback enough; don't add a
  transient overlay"). Adding a second on-canvas indicator for Ctrl
  specifically would be chrome the behavior itself already provides.
- The marker is pure on-canvas drawing (WebGL draw list, screen-space
  constant, per `docs/design-system.md`'s existing rule for all canvas
  editing UI), shown only while a scale or rotate drag is in progress —
  absent the rest of the time, including when an object is merely selected
  with no drag active.

### Resolving two points flagged in `adrs.md`

- **Flag 4 (AC 16's "opposite corner from the rotate handle" is ambiguous
  since the handle sits at the top-edge midpoint, which has no opposite
  corner):** **bottom-edge midpoint**, the architect's own default — it's
  the point directly across the box's center from the rotate handle, the
  same relationship "opposite corner" describes for a corner-anchored
  handle, just substituting the one edge-midpoint pair this handle actually
  has. The pivot marker (above) makes this concrete on screen the instant
  Shift is held, so the ambiguity in the wording doesn't become an
  ambiguity the maker actually experiences.
- **Flag 5 (AC 11's corner-only handles on a polygon/star are the first
  per-kind difference in the Select tool, against
  `canvas-navigation-and-selection`'s "no type-specific affordance, full
  stop" note):** not a conflict with that note, because that note was about
  the *selected-but-not-editing* bounding box (criterion 14 of that spec) —
  every object kind still gets the exact same plain box there, unchanged.
  This slice's handles are a different state (a single object's transform
  controls), and within that state every kind already gets the same
  vocabulary (the squircle/arc-icon glyphs above); the only thing that
  varies is *how many* resize handles appear, which is the shape-handle
  precedent's own existing pattern (`primitive-shapes`' shape handle count
  and placement already differ by kind — a rectangle gets one corner-radius
  handle, an ellipse and polygon/star get none) rather than a new kind of
  inconsistency this slice introduces.

### Token additions

New rows added to `docs/design-system.md`'s Spacing and sizing table:
Transform resize handle (8×8px rounded square), its hit-test radius (8px),
the Transform rotate handle (12×12px icon), its screen offset (20px) and
hit-test radius (12px), and the Transform pivot marker (6px dot, `--accent`
60%). No new color tokens — every state reuses `--accent`/`--accent-hover`
per the existing one-accent-two-states rule. See that file for the exact
entries and the new Interaction-conventions bullet on rotated cursors.

## Links
Requirements: R-EDIT-012 (`docs/requirements.md`, added by this spec);
cross-references R-EDIT-010/011 (`specs/0004-canvas-navigation-and-selection/
specification.md`) and R-EDIT-002/004 (`specs/0003-primitive-shapes/
specification.md`)
PR:
