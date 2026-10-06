# Arc and curve handles: ellipse arcs, and a Curve handle on ellipse, polygon and star

Status: Draft
Priority: Should
Origin: Customer (Part A); Customer (Part B: reading confirmed 2026-10-06, extended to polygon and star)

## User value

As a maker I want to pull a circle or ellipse into an arc, a pie sector or a
segment with two handles, and pull the sides of a circle, ellipse, polygon or
star in or out with one more handle, so that a half circle cutout, a quarter
disc, a curved slot edge, a diamond, a four-pointed star (a sparkle), a
hexagon with bulging sides or a five-pointed star with concave sides is made
in a few drags from one shape instead of being built node by node.

The customer's words (translated, 2026-10-06): "The circle tool needs handles
to create a circular arc. It would also be practical to squash/stretch the
circle/ellipse at its imagined inner cross using nodes, analogous to Affinity,
into a [star]." The word before "squash" was unreadable; the customer then sent
two screenshots of Affinity Designer's Polygon tool (Sides: 4, option "Curve"),
which Part B is built from. He confirmed the reading and asked for Curve on
polygon and star right away: "polygon und stern auch gleich" (2026-10-06).

**Depends on `specs/unified-object-editing/`.** The arc handles and the curve
handle are parameter handles of the one edit mode, the Select tool, not a
feature of the Ellipse tool or the Polygon/Star tool. Part A can be built
without Part B. The curve on polygon and star (criteria 35 to 52) extends the
curve on the ellipse (criteria 17 to 34) and uses the same shared evaluation.

**Field reference.**

- Inkscape's Ellipse tool: Start and End angle fields and two round handles on
  the outline; buttons "Slice" (pie), "Arc" (open) and "Chord" (closed by a
  straight line) and "Make whole". Dragging a handle with the pointer inside
  the circle makes an arc, outside makes a slice. All of it needs the Ellipse
  tool. Inkscape has no curve parameter; a star with curved sides needs a path
  effect or manual nodes.
- Affinity Designer: the Ellipse tool can switch to a pie shape with Start and
  End angle in the context toolbar and a "Close pie" option. Its Polygon tool
  has "Sides" and "Curve" (help: "the convex or concave nature of the sides";
  0% is straight, a negative value is concave; the usable percentage depends on
  the number of sides) and "Smooth points" (only for curve above 0%). With
  Sides 4 and Curve 100% the shape is a circle with the usual eight box
  handles; with Curve about -100% it is a four-pointed star with concave
  sides. A small handle inside the shape, at the middle of one side, drags the
  value. Other side counts work the same way (Curve 100% is a circle for any
  number of sides); our definition for general N is under "Curve on polygon
  and star".
- Ink/Stitch, LightBurn: no equivalent; a LightBurn circle has no handles.

Where we do better: the arc handles and the curve handle are there whenever a
circle is selected, in the same tool as resizing and rotating; the kind (pie,
arc, chord) is an explicit setting, not a side effect of where the pointer is;
angles and the curve can be typed by double-click; the curve lives on the
circle or ellipse itself (stretch it to any aspect afterwards), where Affinity
needs a 4-sided polygon; the same handle works on our star, which Affinity's
Polygon tool cannot make.

## Part A: Arc handles

Terms. The **arc** of an ellipse is given by a start angle, an end angle and a
**kind**. Angles are the ellipse's own parametric angles, measured from its
own right-hand axis (3 o'clock in its own frame) and increasing clockwise on
screen, the product's outline direction. For a circle that is the ordinary
angle. The arc runs from the start angle to the end angle in the direction of
increasing angle. Kinds:

- **Pie** (sector): the arc plus a straight line from each end to the centre.
  A closed shape.
- **Arc** (open): the arc only. An open path with two ends.
- **Chord** (segment): the arc plus a straight line from its end to its start.
  A closed shape.

A start and an end angle that are equal (within 0.1°) mean the whole ellipse.

Part A concerns ellipses whose curve is 100% (a true ellipse, Part B). An
ellipse with a curve other than 100% has no arc (Part B criterion 31).

### Acceptance criteria

1. Given one selected circle or ellipse and handles drawn
   (`specs/unified-object-editing/` criteria 6 to 8), then two arc handles are
   shown, a start handle and an end handle, on the outline at the arc's start
   and end angles, together with the transform handles. They are shown on a
   whole ellipse as well, so the maker finds them without a manual; where
   exactly (both would sit at the same point as a resize handle) is the
   `ux-engineer`'s decision, and the two are always distinguishable from each
   other and reachable.
2. Given the maker drags an arc handle, then that handle's angle follows the
   pointer: with the pointer at (x, y) in the ellipse's own frame relative to
   its centre, the angle is atan2(y / ry, x / rx), whether the pointer is
   inside or outside the ellipse (for a circle, the plain direction from the
   centre to the pointer). The
   other angle and the kind do not change. The first drag of a handle on a
   whole ellipse starts an arc of the kind in criterion 3. Escape cancels and
   writes nothing; a press and release without 3 screen pixels of movement
   writes nothing.
3. Given the Select tool's bar shows, for a selected ellipse, a three-way
   choice "Pie / Arc / Chord" and a button "Whole ellipse", then choosing a
   kind changes the selected arc's kind in one commit and keeps the angles;
   "Whole ellipse" makes the ellipse whole again in one commit (start and end
   angle no longer stored). A new arc, made by dragging a handle on a whole
   ellipse, has the kind "Arc" (default of question 1). The kind choice and
   the button are shown only while a single ellipse is selected, and the kind
   choice is disabled on a whole ellipse.
4. Given an arc, then what is drawn and stroked is: Pie, the closed outline
   arc, line to the centre, line to the arc's start; Arc, only the arc, not
   closed; Chord, the arc and the straight chord, closed. Nothing of the rest of
   the ellipse is drawn. Given a fill (`specs/0007-stroke-and-fill-styling/`),
   then a Pie and a Chord are filled inside their outline, and an open Arc is
   filled as if closed by a straight line from its end to its start while only
   the arc itself is stroked (the SVG rule).
5. Given an arc whose sweep from start to end is S degrees, then S is between
   0 and 360 and is never negative: dragging a handle past the other handle
   gives the sweep the pointer's position implies (an end handle that crosses
   the start handle turns an almost whole ellipse into a sliver). A drag that
   ends within 0.1° of the other handle makes the ellipse whole.
6. Given a drag of an arc handle, then the old shape stays in black and the
   new one is drawn in blue (`specs/unified-object-editing/` criteria 10 to 13),
   and a readout at the pointer shows "start 30.0° · sweep 120.0°" for the
   dragged handle's angle and the resulting sweep.
7. Given Ctrl held during an arc-handle drag, then the dragged angle snaps to
   the nearest stop of the multiples of 15° and 22.5°
   (`specs/object-transform-refinements/` criteria 33 and 34), measured from
   the ellipse's own right-hand axis (0°), positive and negative. Without
   Ctrl no snapping. Ctrl has no effect on a numeric entry.
8. Given a double-click on an arc handle, then a field with a "°" suffix opens
   next to it, pre-filled with that handle's angle (the readout's value, one
   decimal), with the cancel, invalid and untouched-Enter rules of
   `specs/object-transform-refinements/` criteria 19 to 21. Enter writes the
   angle in one commit; no double-click switches a tool.
9. Given an arc, then its selection box is the tight oriented box around what
   is drawn (a half circle of radius 20 mm has a box of 40 × 20 mm; a pie
   includes its centre), in the ellipse's own rotated frame. Resize handles,
   the centre handle, the live size readouts, typed W and H, the marquee and
   the pivot all use that box (question 3).
10. Given a resize of an arc by any transform handle and any modifier, then the
    ellipse's size and position change by the affine map the drag defines in
    the local frame and the angles do not change (parametric angles survive a
    non-uniform scale), so a half circle stays a half circle at any aspect.
    Stroke width follows the "Scale stroke width" switch and nothing else
    changes (`0005` criteria 8 and 26).
11. Given a rotation or a move, then the ellipse's rotation and position change
    as for any primitive and the arc angles are unchanged (they are relative to
    the ellipse's own axis). Skew handles are not shown (primitives,
    `specs/object-transform-refinements/` criterion 50).
12. Given an arc, then hit-testing (click, hover, lasso, Alt-click cycle) uses
    the drawn outline only, including the straight lines of a pie or chord; the
    missing part of the ellipse is not clickable.
13. Given "Object to path" on an arc, then the result is a path of exactly the
    drawn outline: an Arc becomes an open path, a Pie and a Chord closed
    paths. The arc is split into `ceil(S / 90°)` cubic Bézier segments of equal
    sweep (the standard arc-to-cubic construction, deviating from the true
    ellipse by at most 0.1 % of its larger radius); nodes between two arc
    segments are Symmetric, the ends of an Arc and the corners of a Pie or
    Chord are Corner nodes. A Pie adds the centre as one Corner node. A whole
    ellipse converts as today (4 Symmetric nodes, `0003` criterion 19).
14. Given an arc, when the maker saves, closes and reopens the project, then
    start angle, end angle and kind are exactly as saved and the object is
    still an ellipse. A whole ellipse is stored exactly as before (no arc
    fields), so existing files are unchanged.
15. Given a project containing an arc, when an older build opens it, then it is
    refused with the "saved by a newer version" message and never shown as a
    whole ellipse; the `format_version` increases.
16. Given an ellipse created by the Ellipse tool (Shift and Ctrl rules of
    `specs/shape-creation-from-center/` included), then it is a whole ellipse;
    arcs are made only with the arc handles.

### Document-model impact (Part A)

`Shape::Ellipse { frame }` gains an optional arc: `start`, `end` (angles) and
`kind` (three values). Absent means whole. This is a stored-field change and
a `format_version` increase, even though whole ellipses are written as before,
because an older build would otherwise show an arc as a whole ellipse
(criterion 15). The architect decides: field layout (three LWW registers or
one), the writer's rule for "whole" (absent fields), validation on open
(finite angles; a kind outside the three values is damaged), the next free
`format_version` at merge time (coordinate with `specs/rectangle-corner-
radii/` and `specs/0007-stroke-and-fill-styling/`), the tight-box computation
in `oriented_bounds`, the shared evaluation function every consumer calls,
and golden fixtures for all three kinds (`CLAUDE.md` §5).

## Part B: Curve handle, from circle to star

**Status of this part: the reading of the customer's request and the two
Affinity screenshots is confirmed by the customer (question 2, resolved
2026-10-06), including Curve on polygon and star from the start. Requirement,
not proposal. Criteria 17 to 34 define the ellipse; criteria 35 to 52 add
polygon and star.**

What we understood. A circle or ellipse has four **cross nodes** where its
imagined inner cross (the horizontal and vertical axis through the centre)
meets the outline: east, south, west, north. One handle lets the maker change
the curvature of the four sides between these nodes continuously: a circle at
100%, a diamond with straight sides at 0%, a four-pointed star with concave
sides and sharp tips at -100%. This is Affinity's "Curve" option of the Polygon
tool with 4 sides (screenshots: Curve about -100% gives the star with a small
handle inside near one side; Curve 100% gives a circle with a bounding box of
eight handles).

Considered and dropped (customer: none of these): a standing or lying oval
(already possible with the resize handles), an egg (one node pulled along its
axis), a rounder or squarer shape by node tension. The screenshots show one
curvature parameter, not node pulling.

Terms. **Curve** `k` is a value from -100% to 100%; below, `k` is the same
value as a fraction (-1 to 1). The ellipse has centre `C`, half-axes `rx` and
`ry` in its own frame, and cross nodes E (`rx`, 0), S, W and N. A **side** joins
two neighbouring cross nodes (E to S, S to W, W to N, N to E). The **unit
shape** is the shape for `rx = ry = 1`; every ellipse is its image under the
scale `(x, y) → (rx·x, ry·y)`, so every statement below about the unit shape
holds on an ellipse with per-axis radii.

### Acceptance criteria

17. Given an ellipse (or circle), then it has one curve value, from -100% to
    100%, 100% by default. A circle or ellipse made by the Ellipse tool has
    100% (criterion 34). Curve 100% is the ellipse as it exists today.
18. Given curve `k`, then every side of the unit shape is a circular arc
    through its two cross nodes whose tangent at each end makes an angle of
    `45° · |k|` with the chord between the two nodes, bulging away from the
    centre for `k > 0`, towards the centre for `k < 0`, and straight (no arc)
    for `k = 0`. (The arc then subtends `90° · |k|`.) The three fixed points:
    - `k = 100%`: each side is a quarter of the circle; the shape is the
      circle (the arc at 45° tangent angle is the circle's own arc).
    - `k = 0%`: each side is a straight line; the shape is a square turned by
      45° with its vertices at the cross nodes (for `rx ≠ ry` a rhombus). It is
      a diamond, not a rounded square.
    - `k = -100%`: each side is a quarter of a circle of radius 1 centred at
      the nearest corner of the bounding square (for example, the side E to N
      is the arc of the circle centred at (1, 1) through (1, 0) and (0, 1),
      concave); the shape is a four-pointed star with sharp tips at the cross
      nodes. It is not the mathematical astroid: the curve's middle is 0.293
      of a radius from the centre along the diagonal, the astroid's is 0.354.
19. Given curve `k`, then the middle of the side E to N (and of the other three
    by symmetry) lies on the diagonal of the box at `(rx·d, ry·d)` from the
    centre (towards E and N), with `d(k) = 0.5 · (1 + tan(22.5° · k))`. So
    `d(100%) = 0.7071`, `d(0%) = 0.5`, `d(-100%) = 0.2929`. Examples, circle of
    radius 20 mm: at 100% the side's middle is at 14.14 mm on both axes and the
    area is 1256.6 mm²; at 0% the shape is a diamond with 40 mm diagonals,
    middle at 10 mm, area 800.0 mm²; at -100% the middle is at 5.86 mm and the
    area is 343.4 mm² (a 40 × 40 mm square minus four quarter discs of
    radius 20 mm). Example, ellipse `rx = 30`, `ry = 10` mm at -100%: the
    middle of the side E to N is at (8.79, 2.93) mm.
20. Given any curve value, then the shape is one closed outline that does not
    cross itself, passes through the four cross nodes, and stays inside the
    ellipse's bounding box. Its selection box is therefore the box of the
    ellipse, `2·rx × 2·ry`, for every curve value; typed W and H mean
    `2·rx` and `2·ry`.
21. Given a circle, then the interior angle at each cross node is
    `90° · (1 + k/100%)`: 180° at 100% (smooth), 90° at 0% (corner), 0° at
    -100% (cusp, a needle tip). For an ellipse the angle at -100% is 0° and at
    100% 180° as well; between, it depends on the aspect ratio. Given a stroke
    (`specs/0007-stroke-and-fill-styling/`) and a fill, then the stroke joins
    at the cross nodes follow the object's join style like at any other
    corner, a mitre join falls back to a bevel by that style's mitre limit at
    sharp tips, and the fill is the closed outline. Nothing in this spec adds
    a stroke or offset rule; an offset of the shape is the offset of the path
    "Object to path" would give (criterion 29), and tips below 180° are what
    an offset has to handle for any path.
22. Given one selected ellipse and handles drawn (`specs/unified-object-editing/`
    criteria 6 to 8), then one **curve handle** is shown inside or on the
    outline at the middle of the side E to N, the upper right side of the
    unrotated ellipse (position per criterion 19; at 100% it sits on the
    outline at 45°), together with the transform handles (and, on a whole
    ellipse, the arc handles of Part A). It is drawn with the same parameter-
    handle silhouette as the other parameter handles, and the hit order is that
    of `specs/unified-object-editing/` criterion 5. The curve handle and an arc
    handle can sit at the same point (an arc at -45°); then the arc handle,
    which is on the outline, wins the tie and the curve handle is reached after
    moving the arc handle. The `ux-engineer` may choose a different resolution.
23. Given the maker drags the curve handle with the pointer at (x, y) in the
    ellipse's own frame relative to its centre, then with `u = x / rx` and
    `v = -y / ry` (screen y down, so `u`, `v` are positive towards the handle's
    corner) the curve becomes `k = atan((u + v) − 1) / 22.5°`, limited to
    -100% to 100% (`atan` in degrees; it is the exact inverse of criterion 19:
    the handle stays under the pointer until a limit is reached). Dragging
    towards the centre gives negative values, away from it towards the
    outline's diagonal point gives 100%. Within 1 percentage point of 100%, 0%
    or -100% the value snaps to exactly that value, with no modifier. With Ctrl
    held the value snaps to the nearest multiple of 25 percentage points
    (Ctrl has no effect on a numeric entry). Escape cancels and writes
    nothing; a press and release under 3 screen pixels of movement writes
    nothing; release writes one commit.
24. Given a double-click on the curve handle, then a field with a "%" suffix,
    accessible name "Curve", opens next to it, pre-filled with the current
    value (one decimal), and Enter writes the value in one commit. A value
    above 100 or below -100 is limited and the limited value is written; an
    empty or non-numeric value keeps the field open and marked invalid; Escape,
    an unedited Enter, a click elsewhere, a tool switch and a selection change
    close it and write nothing (`specs/object-transform-refinements/`
    criteria 19 to 21 and 30 to 31). No double-click switches a tool.
25. Given a curve-handle drag, then the old shape stays in black and the shape
    the release would commit is drawn in blue, hollow
    (`specs/unified-object-editing/` criteria 10 to 13), and a readout at the
    pointer shows "curve -35.0 %".
26. Given a selection that consists only of ellipses (one or more; polygons and
    stars join the selection in criterion 44), then the Select tool's top bar shows a "Curve" number field (-100 to 100, "%"),
    with the rules of `specs/unified-object-editing/` criterion 21: it updates
    every selected ellipse live, commits once per interaction, shows "Mixed"
    when the values differ, and a typed value applies to all. It is disabled,
    with a hint, for an ellipse that has an arc (criterion 31). Typing 100 makes
    the shape an ellipse again; there is no separate reset button.
27. Given a resize by any transform handle and any modifier, or a typed W and
    H, then `rx`, `ry` and position change by the affine map the drag defines
    in the local frame and the curve value does not change (the shape scales
    like any ellipse; a star made at -100% stays a star at any aspect). Stroke
    width follows the "Scale stroke width" switch and nothing else. Given a
    rotation or a move, then rotation and position change and the curve value
    is unchanged. Skew handles are not shown (`specs/object-transform-
    refinements/` criterion 50).
28. Given any curve value, then hit-testing (click, hover, lasso, Alt-click
    cycle) and the marquee use the drawn outline and the selection box of
    criterion 20, by the same rules as for any closed shape.
29. Given "Object to path", then the result is a closed path of 4 nodes in the
    order and direction a whole ellipse's conversion has today
    (`0003` criterion 19), one cubic Bézier per side. In the unit shape the
    side E (1, 0) to N (0, 1) has the control points
    `P1 = E + L·(cos α, sin α)` and `P2 = N + L·(sin α, cos α)` (the mirror
    image of `P1` across the diagonal), with `α = 135° − 45°·k` and `L = (√2 / 3) / cos²(22.5° · |k|)`, and the
    other three sides are the same rotated or mirrored; control points are then
    scaled by `rx`, `ry` and placed in the ellipse's frame. Check values: at
    100% `P1 = (1, 0.5523)`, at -100% `P1 = (0.4477, 0)`, at 0% the control
    points are on the chord and the segment is written as a straight line
    without handles. Each Bézier deviates from its true circular arc by at
    most 0.1% of the larger radius. Node kinds: Symmetric at 100%, Corner at
    every other value (including -100%, where the two handles of a node point
    the same way). A shape at 100% converts exactly as a whole ellipse does today.
30. Given "Object to path" on a selection of several objects, then each is
    converted independently and atomically, as `specs/unified-object-editing/`
    criterion 22 defines; the curve value is not kept (a path has none).
31. Given an ellipse with a curve other than 100%, then no arc handles, no
    "Pie / Arc / Chord" choice and no "Whole ellipse" button are shown for it,
    and no arc can be made; given an ellipse with an arc, then no curve handle
    is shown and the bar's "Curve" field is disabled. To combine the two the
    maker makes one of them whole first (arc: "Whole ellipse"; curve: type
    100). Arc and curve never coexist on one object. (Excluded to keep the
    angle definition of Part A, which assumes a true ellipse; combining them is
    a possible later story.)
32. Given an ellipse with a curve other than 100%, when the maker saves, closes
    and reopens the project, then the curve value is exactly as saved and the
    object is still an ellipse with the same size, rotation and other
    parameters. An ellipse with curve 100% is stored exactly as before (no new
    field), so existing projects are unchanged and not rewritten on open.
33. Given a project containing a curve other than 100%, when an older build
    opens it, then it is refused with the "saved by a newer version" message
    and never shown as a plain ellipse; the `format_version` increases (once
    for Part A and Part B if they ship in one PR). Given a file with a curve
    that is not finite or outside -100% to 100%, or with a curve and an arc on
    one ellipse, then it is refused as damaged with a named error, not clamped.
34. Given an ellipse made by the Ellipse tool (Shift and Ctrl rules of
    `specs/shape-creation-from-center/` included), then its curve is 100%;
    the curve is changed only with the handle or the bar field.

### Curve on polygon and star

The customer asked for polygon and star in the same slice (2026-10-06). The
ellipse is the special case: an ellipse is a 4-sided shape whose first vertex
is at its right-hand cross node E, scaled to `rx` and `ry`; the rule below
reduces to criteria 18, 19, 23 and 29 for it (criterion 51 makes that a test).

Terms. A polygon with N sides has N **vertices** on its circumscribed circle of
radius `R` (its outer radius). A star with N points has 2N vertices, N outer
(radius `R`) and N inner (radius `ρ·R`, `ρ` the ratio), alternating. The
vertices form a closed **ring** of `M` vertices (`M = N` for a polygon, `M = 2N`
for a star, `M = 4` for the ellipse); a **side** joins two neighbouring
vertices; its chord has length `c` (polygon `c = 2·R·sin(180°/N)`; star
`c = R·√(1 + ρ² − 2·ρ·cos(180°/N))`). The **full angle** is `φ = 180°/M`
(polygon `180°/N`, star `90°/N`, ellipse 45°). **Curve** `k` is a fraction from
-1 to 1 as in Part B above. **Vertex 0** is the shape's first vertex: for a
polygon the vertex the creation drag put at B, for a star the first outer
vertex (the one the inner-radius handle's first inner vertex follows, clockwise
on screen), for the ellipse E.

35. Given a polygon or a star, then it has one curve value from -100% to 100%,
    **0% by default** (straight sides: every polygon and star made so far, and
    every one the Polygon/Star tool makes). The default differs from the
    ellipse's 100% on purpose, so no existing shape changes its look. A shape
    made by the Polygon/Star tool has 0%; the curve is changed only with the
    handle or the bar field.
36. Given curve `k` and a side of the ring, then the side is a circular arc
    through its two vertices whose tangent at each end makes the angle
    `θ = |k|·φ` with the chord, bulging to the outside of the straight shape
    (the side of the chord on which the shape's interior is not) for `k > 0`,
    to the inside for `k < 0`, and straight for `k = 0`. The arc subtends
    `2θ`. All sides of one shape use the same `k`. Fixed points:
    - `k = 0%`: the shape as it is today (straight sides).
    - polygon, `k = 100%`: each side is an arc of the circumscribed circle; the
      shape is that circle, for every N from 3 to 1024. (A 4-sided polygon at
      100% is the circle of criterion 18.) A star at 100% is no circle: its
      outer and inner vertices are on two circles; its 2N arcs bulge outward and
      the shape is a pillow-edged star.
    - `k = -100%` (where the limit of criterion 38 allows it): each side is the
      mirror image of the 100% arc across its chord, concave. A 4-sided polygon
      at -100% is the four-pointed star of criterion 18; a hexagon is a
      six-pointed star with concave sides; a star made by the Star tool gets
      concave sides and keeps its points.
37. Given a polygon with outer radius `R` and N sides, then the middle of a side
    lies on the perpendicular bisector of its chord, at the distance
    `m(k) = R·(cos(180°/N) + sin(180°/N)·tan(k·90°/N))` from the centre. The
    bulge of the middle from its chord is the sagitta
    `s = (c/2)·tan(k·φ/2)`, signed (positive outward), and the same formula
    gives the middle of a star's sides. Examples, `R = 20` mm: hexagon at 100%:
    20.00 mm (on the circle, area 1256.6 mm²), at 0%: 17.32 mm (area
    1039.2 mm²), at -100%: 14.64 mm; triangle at 0%: 10.00 mm, at -50%
    (its limit): 5.36 mm; square (N = 4) at 0%: 14.14 mm, at 100%: 20.00 mm.
38. Given a shape, then the lowest allowed curve is `k_min = max(-1, −A/(2·φ))`
    with `A` the smallest interior angle of the straight shape (polygon:
    `180° − 360°/N`; star: the smaller of its outer and inner interior angle).
    At `k_min` the sharpest tips become cusps (interior angle 0°); below it the
    outline would cross itself. So: every polygon with N ≥ 4 has `k_min = -100%`;
    a triangle has -50%; a star with N = 3, ρ = 0.5 (hexagram) has -100%; a
    star with N = 5, ρ = 0.382 (pentagram) has -100%; a star with N = 5,
    ρ = 0.2 has about -44.4% (tip angle 15.97°, `φ = 18°`). The value written by
    a drag, a typed entry or the bar field is limited to `[k_min, 100%]` for that
    shape and the limited value is written. A stored value below `k_min`
    (the maker later lowers Points or the ratio) is valid and kept as saved;
    the **effective** curve is `max(stored, k_min)` and everything drawn, shown
    and converted uses the effective curve. Restoring the Points or ratio
    brings the stored value back. For every N from 3 to 1024, every ratio from
    0.01 to 0.99 and every effective curve value the outline is one closed
    curve without self-crossings (touching only at cusps at `k_min`).
39. Given a polygon or star at curve `k`, then the interior angle at every
    vertex is the straight shape's interior angle plus `2·k·φ`, so polygon
    corners are smooth (180°) at 100% and a star's outer tips widen as `k`
    grows. Strokes and fills follow the rules of criterion 21 (join style at
    every vertex, mitre limit at sharp tips, the fill is the closed outline).
40. Given one selected polygon or star and handles drawn
    (`specs/unified-object-editing/` criteria 6 to 8), then one **curve handle**
    is shown at the middle of one side together with the transform handles (and,
    on a star, its inner-radius handle). The side is the one on the
    counter-clockwise (on screen) side of vertex 0 (for the unrotated ellipse
    that is E to N, criterion 22; for a star it is the side on the other side of
    vertex 0 from the inner-radius handle's vertex). The handle sits at the
    middle of that side at the effective curve (criterion 37: on the straight
    side's middle at 0%, outside it for `k > 0`, inside for `k < 0`); it follows
    rotation. It has the parameter-handle silhouette of criterion 22; hit order
    as in `specs/unified-object-editing/` criterion 5; no two glyphs closer
    than 4 px. A polygon had no parameter handle before this; the curve handle
    is its only one.
41. Given the maker drags the curve handle, then the pointer is projected onto
    the perpendicular bisector of that side's chord (in the shape's own frame,
    unaffected by rotation), `t` is its signed distance from the chord's middle
    (positive outward) and the curve becomes `k = 2·atan(2·t/c) / φ`, limited
    to `[k_min, 100%]` (it is the exact inverse of criterion 37: the handle
    stays under the pointer's projection until a limit is reached; for the
    ellipse it equals criterion 23). Snapping and cancelling are as in
    criterion 23: within 1 percentage point of 100%, 0% or `k_min` the value
    snaps to it with no modifier, Ctrl snaps to multiples of 25 percentage
    points (the limit `k_min` is also a stop), Escape cancels and writes
    nothing, under 3 screen pixels of movement writes nothing, release writes
    one commit.
42. Given a double-click on the curve handle of a polygon or star, then the
    field of criterion 24 opens ("%" suffix, accessible name "Curve",
    pre-filled with the effective value to one decimal); a value outside
    `[k_min, 100%]` is limited to the nearest bound and the limited value is
    written; every other rule is criterion 24's.
43. Given a curve-handle drag on a polygon or star, then the preview and the
    readout are criterion 25's: the old shape stays in black, the shape the
    release would commit is drawn in blue, hollow, and a readout at the pointer
    shows "curve -35.0 %".
44. Given a selection that consists only of ellipses, polygons and stars in any
    mix, then the Select tool's top bar shows the "Curve" field of criterion 26
    (next to "Points" and "Ratio" where those apply). A typed value or a drag
    of the field is limited per shape to that shape's own range (a triangle in
    the selection stops at -50% while a hexagon goes to -100%); the field shows
    the effective value and "Mixed" when the effective values differ; typing
    100 gives an ellipse its ellipse shape back and a polygon its circle. For a
    selection that contains an ellipse with an arc the field is disabled with
    the hint of criterion 26. There is no reset button; 0 resets a polygon or
    star to straight sides.
45. Given a resize of a polygon or star (always uniform and about the centre,
    `specs/unified-object-editing/` criterion 16) or a typed r, then the outer
    radius `R` changes (r is `R`, as today) and the curve value, ratio, Points
    and orientation do not change; the curved sides scale with the shape
    (`k` is scale-free). Rotation and move change rotation and position only.
    Stroke width follows the "Scale stroke width" switch. No skew handles.
46. Given a change of Points N or of the ratio while a curve is set, then the
    stored curve value is kept and the shape is re-evaluated with the new
    `φ`, `c` and limit (criterion 38). The curve is a fraction of the
    full angle, so 100% on a polygon is a circle for every N. The curve handle
    and the "Curve" field show the effective value.
47. Given a polygon or star at any curve value, then its selection box is the
    tight oriented box around the drawn outline, by the rule the box has for
    these shapes today applied to the curved outline (it grows when sides bulge
    outward, up to the circle's box for a polygon at 100%, and is never
    smaller than the straight shape's box when `k > 0`), and the resize
    handles, centre handle, readouts, marquee and pivot use that box. The
    pivot of a rotation and of a resize stays the shape's centre
    (`specs/unified-object-editing/` criterion 16). Hit-testing (click, hover,
    lasso, Alt-click cycle) uses the drawn outline, by the rules for any closed
    shape.
48. Given "Object to path" on a polygon or star, then the result is a closed
    path with `M` nodes (polygon N, star 2N) in the order and direction of
    today's conversion (`0003` criterion 20), one segment per side: at 0% N or
    2N Corner nodes joined by straight segments exactly as today, otherwise one
    cubic Bézier per side from vertex `V` to the next vertex `W`, with control
    points `V + L·d` and `W + L·d'`, where `L = (c/3)/cos²(θ/2)`, `d` is the
    unit vector of the chord `V→W` turned towards the outside by `θ` for
    `k > 0` (towards the inside for `k < 0`), and `d'` its mirror image across
    the chord's perpendicular bisector. (For the ellipse's unit shape this is
    criterion 29's `L` and angles.) A side whose arc subtends more than 90°
    (only a triangle with an effective curve above 75%) is split into two equal
    arcs by one extra Symmetric node at its middle, so a triangle at 100%
    gives 6 nodes. Each Bézier deviates from its true circular arc by at most
    0.1% of `R`. The effective curve is the one converted, and the curve value
    is not kept (criterion 30 applies).
49. Given "Object to path" on a polygon or star with a curve other than 0%,
    then a node is **Symmetric** where the two tangents at it are collinear
    (interior angle 180° within 0.01°; for a polygon exactly at 100%) and
    **Corner** at every other vertex (including every vertex at `k_min`, where
    the two handles of a node point the same way). Nodes at 0% are Corner, as
    today.
50. Given a polygon or star with a curve other than 0%, when the maker saves,
    closes and reopens the project, then the curve value is exactly as saved
    and Points, ratio, outer radius, rotation and every other parameter are
    unchanged. A polygon or star at 0% is stored exactly as before (no new
    field), so existing projects are unchanged and not rewritten on open.
    Given a project that contains a curve on a polygon or star, when an older
    build opens it, then it is refused with the "saved by a newer version"
    message and never shown with straight sides; the `format_version` is the
    one increase shared with the ellipse register of criterion 33 (once for
    Part A and Part B if they ship in one PR). Given a file with a curve that is
    not finite or outside -100% to 100%, then it is refused as damaged with a
    named error, not clamped; a value below that shape's `k_min` is not damaged
    (criterion 38).
51. Given a 4-sided polygon with vertex 0 at the ellipse's E, or a circle, then
    at every curve value from -100% to 100% (steps of 12.5 percentage points
    included) the polygon's outline of criteria 36 and 37 and the circle's of
    criteria 18 and 19 are the same curve within 0.001 mm for `R = 20` mm, and
    their converted paths of criteria 29 and 48 have the same control points.
52. Given a selection that contains a path, a rectangle or any other object that
    is not an ellipse, polygon or star, then the "Curve" field is not shown and
    no curve handle exists for that object (a rectangle's rounded corners are
    `specs/rectangle-corner-radii/`, not a curve).

### Document-model impact (Part B)

`Shape::Ellipse { frame }` gains an optional `curve` register (a unit-carrying
newtype, never a bare `f64`; absent means 100%), next to the optional arc of
Part A. The polygon and the star gain the same kind of optional `curve`
register (the same newtype; absent means 0% there, criterion 35), so one
stored field and one `format_version` increase cover all three shapes, which
the ellipse register and the polygon/star register share (criterion 50). The
stored value is raw and clamped on evaluation to `[k_min, 100%]` for polygon
and star (criterion 38), like the rectangle's radius; validation on open only
rejects non-finite values and values outside -100% to 100%. A shared ring
evaluation (vertices, `φ`, `c`, side arc, middle, cubic control points,
`k_min`) is called by the ellipse and by polygon and star, so criterion 51
holds by construction; the architect decides where it lives and the property
test of criterion 38 (no self-crossing for N from 3 to 1024, ratio from 0.01 to
0.99, any allowed curve value). It is a new stored field and a `format_version` increase, even though
untouched ellipses are written as before (criterion 33). The half-axes `rx`,
`ry` are unchanged, so nothing about the frame or `oriented_bounds` changes for
Part B (criterion 20). The architect decides: field layout; the writer's rule
for 100% (absent); validation on open (finite, within the range, not together
with an arc); what evaluation does when two peers set an arc and a curve
concurrently and a merge yields both (suggestion: a file with both is damaged
on open, but a live merge is not refused; there the arc wins and the curve is
ignored until the arc is made whole);
the one evaluation function every consumer (outline, render, hit test, handle
layout, "Object to path") calls, so the drawn outline and the converted path
agree; the true-arc-to-cubic construction of criterion 29; golden fixtures for
curve 100%, 50%, 0%, -50% and -100% on a circle and one ellipse, and for the
polygon and star on a triangle (limit -50%), a hexagon and a pentagram star at
100%, 50%, 0%, -50% and the limit, plus the triangle at 100% (6 nodes)
(`CLAUDE.md` §5).

A four-pointed star with straight sides still comes from the Star tool
(4 points and a ratio); the curve is the way to concave or convex sides on
any of the three shapes.

## Out of scope

- "Smooth points" (Affinity: smoothing of the corners above 0%), a separate
  curve per side or per node, a different curve for a star's outer and inner
  sides, a true astroid or any curve family other than circular sides, curve
  and arc on the same object (criterion 31), a Curve on a rectangle, path or
  text, a curve field in the Polygon/Star tool's own bar (it only creates;
  the Select tool edits).
- A donut or ring (inner radius, Affinity's "Donut"), Inkscape's "pointer
  inside or outside decides the kind", moving both arc handles together with
  one drag, arcs on rectangles, stars or polygons (a polygon's curve is not an arc), elliptical arcs tilted
  against the shape's own rotation.
- Rotating the arc inside the ellipse by a modifier (the start and end handles
  do it by two drags or by typing).
- Skewing primitives (`specs/unified-object-editing/` question 5), undo and
  redo, keyboard nudging.
- Stroke offsetting and boolean operations on the shape (their own slices; the
  shape is an ordinary closed path through "Object to path").

## Questions for the customer

1. **Which kinds of arc, and which one first?** (a) Three kinds with a switch,
   a new arc starts as an open arc (default, recommended; you asked for a
   "circular arc"; a wedge is one click on "Pie"); (b) three kinds, a new arc
   starts as a pie (what Inkscape does when you drag outside); (c) only the open
   arc for now.
2. **Is this what you meant by the Affinity screenshots?** Resolved by the
   customer (2026-10-06): yes, option (b). Curve (circle 100%, diamond 0%,
   four-pointed star -100%) is accepted and goes onto polygon and star in the
   same slice ("polygon und stern auch gleich"; criteria 35 to 52). Arc and
   curve stay mutually exclusive on an ellipse (criterion 31).
3. **Selection box of a half circle.** The box around only the drawn half (a
   half circle of radius 20 mm is 40 × 20 mm, default, recommended; it is the
   size of the part you cut) or around the whole circle behind it (40 × 40 mm).
   The first also moves the pivot of a rotation to the middle of the drawn part.

## UX notes

(filled in by ux-engineer before Ready)

Open points: (Part B, polygon and star) the curve handle's side
(counter-clockwise of vertex 0) and how it stays clear of a star's
inner-radius handle and of a polygon's resize handles, the position of the
effective value when the stored one is below the limit (a hint in the bar
field, for example "limited to -44.4 % by Points/Ratio"), the "Curve" field
in the bar next to "Points" and "Ratio"; and, for Part A: where two arc handles sit on a whole ellipse and how they differ
from the resize handle at the same point; the look of "Pie / Arc / Chord" and
"Whole ellipse" in the Select bar; the readout text; how the shape tool's old
preview and the blue preview look on a partly drawn outline; (Part B) the
curve handle's glyph and position (inside the shape on the diagonal, up to
0.7071 of the half-axes from the centre, so it can sit close to the centre
handle at -100% and close to an arc handle at 45°: no two glyphs closer than
4 px, `specs/unified-object-editing/` criterion 8; the parameter-handle size
threshold), the "Curve" field's place in a Select bar that also carries two
switches and, for ellipses, the arc controls, and how a shape that looks like a
star but is an "ellipse" is labelled in the interface.

## Links
Requirements: R-EDIT-002 (`docs/requirements.md`)
Builds on: `specs/unified-object-editing/` (must come first),
`specs/0003-primitive-shapes/` (criteria 7 to 9, 10 to 15, 19, 20),
`specs/object-transform-refinements/` (criteria 19 to 21, 33 and 34)
PR:
