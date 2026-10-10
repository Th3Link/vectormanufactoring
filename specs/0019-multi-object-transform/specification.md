# Multi-object transform: one group box with the same handles as a single object

Status: Ready
Priority: Should
Origin: Customer
Depends on: `advanced-selection` merged (PR #61). Its press rules for Shift, Ctrl and Alt and its 8 px outline tolerance are part of the press order in criterion 43.
Sequencing: `style-panel-rework` runs before this story (both change `ui-core` and `editor-wasm`, so they do not run in parallel); this story starts after it and after #61.

## User value

As a maker I want a selection of several objects to get its own bounding box
with the same handles a single object has, so that I can move, scale, rotate
(and skew) a whole arrangement of parts as one piece, keeping the relative
positions I set up, before it goes to the machine.

The customer's words (translated, 2026-10-08): "The selection should get an
additional bounding box with the same handles as a single object has: several
objects or paths can be rotated, moved, (skewed) and scaled together. Let's do
this separately, not as a bug/fix." This is a new feature, not a fix to
`specs/0005-object-transform/`. That spec left a multi-selection at move and
delete on purpose (its criterion 2 and its out-of-scope entry "Multi-object
transform", which said "one shared box vs. independent is itself a real design
question"). This spec answers the question: one shared box, in addition to the
per-object boxes that are drawn today. The "skew" is in brackets in the
customer's sentence; the customer decided on 2026-10-08 to keep it for selections of paths only (question 4).

**Field reference.** LightBurn treats a multi-object selection as one unit with
the selection handles (scale, corner rotate handles about the selection centre,
a numeric rotate field). Inkscape draws one axis-parallel box around a
multi-selection with scale, rotate and skew handles. Illustrator keeps the
rotated box after a rotate and has a "Reset Bounding Box" command; its box
silently resets in some mixed-angle cases, which makers report as a bug. Where
we do better: the handles, pivots, modifiers, typed values and the blue-new/
black-old preview are the ones the maker already knows from a single object, so
nothing new has to be learned; a typed value and a dragged value use the same
reference point; and when a handle cannot do what it promises for some object
in the selection (a polygon cannot be stretched), it is not drawn and the hint
says why, rather than the gesture changing some of the objects and not others.

## What exists today

Facts from the specs listed under Links; the state is `main` plus
`advanced-selection`.

- A multi-selection shows each object's own box and nothing else. No transform
  handle, no centre handle, no parameter handle (`0005` criterion 2,
  `unified-object-editing` criterion 37).
- A drag that starts on a selected object's outline (or, per `0007` criterion
  29, its filled interior) moves the whole selection. A press on empty canvas
  inside the area the selected objects span clears the selection (click) or
  starts a marquee (drag); only a **sole** selected object moves from an
  interior press. This stays as it is for a multi-selection (criterion 45,
  question 3 decided).
- Ctrl-copy and Shift axis lock of a move already work on several objects
  (`edit-interaction-polish` criteria 28 to 38). The typed move, angle, size
  and skew exist for one object only; the keys M, R, S, K with several objects
  selected show "Select one object to type a value".
- A single object's box is its **oriented** box, which turns with the object and
  persists in its `rotation` register (`0005` criterion 18). A multi-selection
  has no register to hold an orientation, and there is no group object yet
  (`layers-and-grouping` is slice 10, not started).

## Terms

- **Multi-selection**: two or more objects selected in the Select tool. Exactly
  one object keeps every single-object rule, unchanged.
- **Group box**: the box of criterion 1. Its **centre**, **corners** and **side
  midpoints** are the points the handles and pivots below refer to. `s` is its
  shorter side in screen pixels.
- **Aligned primitive**: a rectangle or ellipse whose `rotation` is a multiple
  of 90° within 1e-9 rad, or an ellipse whose `rx` and `ry` differ by less than
  the geometric tolerance (a circle, whatever its rotation). A polygon or star is
  never aligned.
- **Uniform-only selection**: a multi-selection that contains a polygon, a star,
  or a rectangle or ellipse that is not an aligned primitive. Criterion 21.
- **Δ**: the gesture's own angle, clockwise on screen as everywhere in the
  product, measured from the press.
- Lengths are compared with the document's geometric tolerance, angles with
  1e-9 rad, as in the earlier specs. Document-space millimetres, X to the right,
  Y down.

## Changes to accepted behaviour

Mentioned by the lead at the demo. The owning specs are not edited by this one; the
lead updates them when this is accepted.

1. `0005` criterion 2 and its out-of-scope entry "Multi-object transform", and
   `unified-object-editing` criterion 37 (first clause), no longer apply: a
   multi-selection has transform handles. Parameter handles stay absent.
2. `object-transform-refinements` criterion 1 (last sentence, centre handle),
   criterion 53 and the out-of-scope lines "Typed entry or handles for multi-object
   selections" and "Skew of multi-object selections" are realised or superseded.
3. `edit-interaction-polish` criterion 25 (typed move for one object only),
   criteria 54 to 59 (M, R, S, K "with one object selected") and the hint "Select one
   object to type a value" are superseded by Part D; its out-of-scope line "Typed
   move for several selected objects" is superseded.
4. (Removed 2026-10-08, question 3 decided (b).) A press on empty canvas inside a
   multi-selection's group box does not move the selection and a click there still
   clears it, exactly as today (criterion 45). No accepted press behaviour
   changes; the group box adds only handles.
5. (Removed 2026-10-08, question 3 decided (b).) The advanced-selection note
   "Marquee over a selected object's box" is not extended to the group box: a plain
   marquee inside it starts without Escape first, as today.
6. (Removed 2026-10-08.) `0007` criterion 29 (option B) is not extended to a
   selection; it applies as today (criterion 44).

## Acceptance criteria

### Part A: the group box

1. Given the Select tool and a multi-selection, then a group box is drawn: the
   axis-aligned rectangle (sides parallel to the document axes) that is the
   tight bounds in document space of the drawn outlines of all selected objects.
   For a path the bounds are curve-accurate; for a primitive they are the
   outline as drawn, after its rotation and corner radius (the bounds
   `edit-interaction-polish` criterion 21 uses for the typed absolute move).
   Example: a rectangle 20 × 10 mm with centre (10, 5) and rotation 30°, and a
   circle (rx = ry = 5) with centre (40, 5): the group box runs from x = -1.16 to
   45.00 and y = -4.33 to 14.33 (W 46.16, H 18.66, to 0.01 mm).
2. Given any selected object with a stroke, then the stroke width is not part of
   the group box, as it is not part of a single object's box. Changing a stroke
   width, dash or colour does not change the box.
3. Given the objects' own oriented boxes (`0005` criterion 18), then they are not
   united into the group box: a rotated rectangle contributes its outline, so
   the group box is as tight as an axis-aligned box can be. (Default D1.)
4. Given a multi-selection, then each selected object keeps its own box as today,
   in addition to the group box (the customer asked for an "additional" box). The
   group box is the one that carries handles and reads as "the selection". A
   per-object ("member") box is 1 px, dashed 4 / 3, in `--member-box` (`--accent`
   at 60%, `docs/design-system.md`), in the object's own oriented frame, drawn
   below the group box. The part of a member box edge that lies within 1.5 device
   pixels of a group box edge is not drawn. A member box is not drawn when it is
   under 6 px on both sides or entirely outside the viewport. Only the group box
   has handles and a hit area. (Default D2.)
5. Given a multi-selection of more than 500 selected objects (the count of the
   selection, not of what is visible, so panning and zooming never change it),
   then no member box is drawn and nothing is drawn in its place (no merged
   box); the group box and its handles are drawn as for any other count.
   Crossing 500 in either direction adds or removes all member boxes in the same
   frame. (Default D2; no measurement needed.)
6. Given the selection changes (an object added, removed, deleted, or the count
   going from one to two or from two to one), then the group box appears,
   updates or disappears in the same frame, and a single selected object shows
   exactly the box and handles it showed before this feature.
7. Given a gesture is committed, then the group box is recomputed from the
   committed geometry and is again axis-aligned. A rotate leaves no orientation
   behind: there is no stored or session group angle (question 1, decided (a)).
   Example: two 10 × 10 squares at (0, 0) and (20, 0), group box 30 × 10;
   rotated by 90° about the centre, the box is 10 × 30 (from (10, -10) to
   (20, 20)); rotated again by 45° about the new centre (15, 5), the box is the
   axis-aligned bounds of the result, 28.28 × 28.28 (from (0.86, -9.14) to
   (29.14, 19.14)), not a turned 10 × 30 box.
8. Given an axis (width or height) of the group box that is smaller than the
   geometric tolerance (all objects are on one vertical or horizontal line, or
   there is a single point), then the box is **degenerate** in that axis. Criteria
   12 to 14 define the handles; the box is still drawn, as one dashed line (drawn
   once, not as two coincident edges) or, in both axes, as criterion 13 says.

### Part B: the handles

9. Given a multi-selection, then the group box shows the same handle kinds as a
   single object, with the layout, hit radii, size tiers and clearances of
   `specs/0008-object-transform-refinements/` and `specs/0009-unified-object-editing/`
   (sizes live in `docs/design-system.md`; this spec does not restate numbers):
   four corner resize handles, four edge resize handles, four corner rotate
   handles, four side rotate handles while Shift is held and no drag runs, the
   centre move handle, and (Part E) four skew handles. No parameter handle is
   drawn for a multi-selection (`unified-object-editing` criterion 37, kept).
10. Given `s` of the group box, then the tiers of `unified-object-editing`
    criterion 7 apply to it: under 24 px only corner resize and corner rotate;
    24 to 47 plus the edge resize handles; 48 and up plus the centre handle. The
    edge resize handles of a box under 24 px are not drawn but stay hit-testable,
    as for a single object. Corner resize and corner rotate handles are never
    hidden by size.
11. Given a press, a hover or a cursor query, then one nearest-centre test over all
    drawn group handles decides, with the radii and the tie order (resize, skew,
    rotate) of `specs/0008-object-transform-refinements/` criterion 9. The centre
    handle is tested last and only inside its hover region
    (`edit-interaction-polish` criterion 16). Group handles never compete with
    any object.
12. Given a group box degenerate in one axis, then a resize handle that would
    change that axis is not drawn and has no hit area: the four corner resize
    handles and the two edge handles on that axis's sides. The two edge handles on
    the other axis stay (they scale the extent that exists). Rotate handles sit at
    the box's corners and side midpoints with the outward directions of a notional
    square (the box's own axes with the signs (±1, ±1)); skew handles follow
    `specs/0008-object-transform-refinements/` criterion 37 (zero height removes the top
    and bottom handles, zero width the left and right). For the tiers of criterion
    10 such a box uses its other, non-zero extent as `s`. Example: a selection of
    two horizontal lines at the same height, 80 px wide on screen, has a box of
    height 0; among the resize handles only the left and right edge handles are
    drawn, and the centre handle is drawn.
13. Given a group box degenerate in both axes (all selected objects are the same
    point), then it has `s` = 0 and shows no resize, rotate, skew or centre handle
    (a rotation about the only point is the identity, so there is no rotate
    gesture). Instead of the dashed box, a 6 px square outline (solid, 1 px,
    `--accent`, with casing) marks the point, so the selection is never invisible.
    The selection still moves by a press on an object's outline.
14. Given any handle that is not drawn (criteria 10, 12, 13, 21, 40), then it has
    no hit area, hover or cursor state, and a press where it would be is an
    ordinary press (criterion 43), not a move of the selection; the one exception
    is the edge resize handles under 24 px (criterion 10), which are hit-testable
    although not drawn.
15. Given handles that fall outside the viewport, then they are not pulled into
    it (the known gap of `specs/0008-object-transform-refinements/`, unchanged); the
    centre handle is the one that stays reachable.

### Part C: move, scale and rotate

#### Move

16. Given a multi-selection, when the maker presses the centre move handle, or a
    selected object's outline (within the 8 px tolerance) or filled interior
    (`0007` criterion 29), and drags, then the whole selection moves 1:1 with the
    pointer, after the 3 px dead zone, by the same operation as a drag on a
    selected object's outline today (a pure translation: sizes, rotations and
    parameters are unchanged), in one commit on release. Under 48 px, or whenever
    the centre handle is not drawn, the objects themselves are the grip. The empty
    interior of the group box does not move the selection (criterion 45). A press
    and release under the dead zone writes nothing. Press order is criterion 43.
17. Given a move drag of a multi-selection, then Ctrl (copy of every selected
    object, plus badge, selection becomes the copies) and Shift (axis lock, lock
    badge) behave exactly as `edit-interaction-polish` criteria 26 to 38 define,
    with the **origin axes** drawn through the group box centre at the press (the
    position of the centre handle). Escape cancels, nothing is written.

#### Scale

18. Given a multi-selection that is not uniform-only (criterion 21), when the
    maker drags a corner resize handle with no modifier, then every selected
    object is scaled by the same factors (sx, sy) about the opposite corner of the
    group box, in the document axes, and the dragged corner follows the pointer
    1:1 (the box is refit from the result, criterion 29). Example: two
    unrotated 10 × 10 rectangles at (0, 0) and (20, 0), group box (0, 0) to
    (30, 10); dragging the bottom-right corner from (30, 10) to (60, 30) gives
    sx = 2, sy = 3: the rectangles become (0, 0) to (20, 30) and (40, 0) to
    (60, 30).
19. Given the same selection, then Ctrl on a corner handle scales both axes by one
    factor (the drag's dominant axis, `0005` criterion 5); Shift scales about the
    group box centre instead of the opposite corner or side, and combines with
    Ctrl; an edge resize handle changes only the axis perpendicular to its side
    (anchored at the opposite side, or the centre with Shift) and Ctrl has no
    effect on it. Shift and Ctrl may be pressed or released mid-drag and the
    result is always computed from the state at the press
    (`specs/0008-object-transform-refinements/` criterion 14).
20. Given a scale gesture of a selection, then the result for each object is:

    | Kind | Result |
    |---|---|
    | path | every anchor point is mapped, every handle vector is scaled by (sx, sy) in the document axes; the curve is the exact image; `rotation` is unchanged |
    | rectangle, ellipse, uniform factor s | frame centre mapped, both dimensions times s, `rotation` unchanged |
    | aligned rectangle or ellipse, sx ≠ sy | frame centre mapped; the dimension along the document x axis is multiplied by sx and the one along y by sy (rotation 0° or 180°: width by sx, height by sy; ±90°: width by sy, height by sx); `rotation` unchanged. A circle (rx = ry within tolerance) with any rotation becomes an ellipse with rx times sx, ry times sy and `rotation` 0 |
    | polygon, star | frame centre mapped, outer radius times s (a star's inner radius scales with it); point count, ratio and `rotation` unchanged |

    Every object stays the same kind (no conversion to a path,
    `primitive-shapes` criterion 21). The outline drawn after the scale is the
    exact image of the outline drawn before, within the geometric tolerance, for
    every object whose rule is in this table, except a rounded rectangle whose
    corner radius does not follow the outline: while "Scale corner radius" is off
    (criterion 24; the radius stays absolute, aligned or rotated), and whenever
    sx ≠ sy (a circular radius cannot follow two factors).
21. Given a uniform-only selection, then it can be scaled only proportionally:
    the four edge resize handles are not drawn and have no hit area, and a corner
    drag always scales both axes by one factor whatever Ctrl does, as a single
    polygon or star does (`0005` criterion 11). Shift (about the centre) still
    applies. The corner handle's hover hint chip (criterion 38) names the cause:
    the title "Resize selection, proportional only", a cause line "Holds a ..."
    that lists only the kinds present, in the order polygon, star, rotated
    rectangle, rotated ellipse, each with "a", joined by commas and "and" (for
    example "Holds a star and a rotated rectangle"; a circle never counts as
    rotated), and the line "To stretch: Object to path first". A maker who wants
    to stretch such an object uses "Object to path" first (it is in the Select
    bar and acts on the primitives only, `unified-object-editing` criterion 22);
    its tooltip reads "Convert the selected shapes to paths. Paths can be
    stretched and skewed freely." (Question 2, decided (a) by the customer / lead
    2026-10-08.)
22. Given a scale drag that would take a factor below 0, then that factor clamps
    to 0 and stays there until the pointer returns past the zero crossing, as for
    a single object (`0005` criterion 13); no object reports or renders a negative
    size and no gesture flips the selection. Given a factor that would put any
    coordinate of any selected object beyond ±1e7 mm (or make any object's result
    otherwise invalid), then the whole resolution is no change, all or nothing
    (the single-object rule): no object of the selection is transformed. There is
    no undo yet, so a release at factor 0 collapses the
    selection in that axis for good, exactly as it does for one object; Escape
    during the drag cancels it (D5).
23. Given a scale gesture, then a live readout shows the group box size at the
    pointer, "123.4 mm × 67.8 mm", and the pivot marker shows the fixed point
    (opposite corner or side midpoint, the centre with Shift), as
    `0005` criteria 14 and the pivot marker rules define.
24. Given "Scale stroke width" and "Scale corner radius" (the two switches of the
    Select bar, off at program start, read at the press that starts the drag),
    then with a switch off the stored stroke width, respectively every stored
    corner radius, of every selected object is not rewritten. With a switch on,
    each object's stroke width, respectively each of its rectangle corner radii
    (`rectangle-corner-radii`, shipped in #53), is multiplied by √(sx·sy), the
    group's one factor; the stroke width is floored at 0.01 mm
    (`0005` criteria 8, 9, 26, 31 and `unified-object-editing` criterion 23). A
    path or ellipse has no corner radius and is unaffected by that switch.
    Dash lengths, colours, opacity and fills are never written.

#### Rotate

25. Given a multi-selection, when the maker drags a corner rotate handle with no
    modifier, then every selected object turns by Δ about the group box centre.
    With Shift held, the pivot is the corner of the group box diagonally opposite
    the grabbed one; with Shift held while a side rotate handle is grabbed, the
    midpoint of the opposite side (`specs/0008-object-transform-refinements/` criteria
    12 to 14, applied to the group box). Shift may change mid-drag and the pivot
    switches at once, computed from the state at the press.
26. Given a rotate gesture, then the result for each object is:

    | Kind | Result |
    |---|---|
    | path | every anchor point turned about the pivot, every handle vector turned by Δ, `rotation` increased by Δ (normalized to (-π, π]) |
    | rectangle, ellipse, polygon, star | frame centre turned about the pivot, frame size and parameters unchanged, `rotation` increased by Δ (normalized); a polygon's or star's shown angle changes by Δ and its box turns with it (`polygon-star-box-refit`) |

    The selection turns rigidly: the distance between any two points of any two
    selected objects is unchanged within the geometric tolerance, and rotating by
    Δ and then by -Δ restores every anchor and frame within the geometric
    tolerance and every `rotation` within 1e-9 rad. Example: the two squares of
    criterion 7 rotated by 90° about the group box centre (15, 5): the left square
    ends at (10, -10) to (20, 0), the right one at (10, 10) to (20, 20), each with
    `rotation` 90°.
27. Given a rotate drag with Ctrl held, then Δ snaps to the nearest stop of the
    table of `specs/0008-object-transform-refinements/` criteria 33 and 34 (multiples
    of 15° and of 22.5°, all quadrants, positive and negative), measured from 0,
    and the readout shows it. There is no absolute angle of a multi-selection, so
    the absolute snapping of a single polygon or star (`edit-interaction-polish`
    criterion 7) does not apply.
28. Given a rotate drag, then the live readout shows "Δ 37.4°" (one decimal at
    most, the same formatter as the rotate readout, clockwise positive) and the
    pivot marker shows the pivot. The group box in the preview is the box at the
    press turned by the live Δ about the live pivot, together with its handles and
    the marker; every object is previewed by criterion 32. On release the box is
    refit to the committed geometry and is axis-aligned again (criterion 7), in
    one frame and without animation. From the release until the commit is
    presented, the cursor is `wait` and the preview stays unchanged (no frame of
    old geometry). A simplified preview is not part of this feature: the architect
    set no threshold (criterion 49); if one is introduced later it draws no
    per-object outline but the turned group box as a solid 1.5 px blue outline,
    with the readout line "Preview simplified" (UX notes section 12).

#### All gestures

29. Given any drag of Part C or E, then the group box, its handles, pivot marker
    and readout follow the geometry the release would commit (the box of
    criterion 1 computed from the preview), except in a rotate (criterion 28). The
    dragged edge follows the pointer 1:1 for every selection whose outlines scale
    exactly (criterion 20); for a rotated rounded rectangle whose radius does not
    follow the outline ("Scale corner radius" off, or sx ≠ sy) it may differ from
    the pointer by up to the radius. For an aligned rounded rectangle the box
    still follows the pointer, since its corners stay inside its frame.
30. Given a press and release on a group handle with less than 3 screen pixels of
    movement, a drag back to the start, or Escape, then nothing is written and no
    blue outline remains (`unified-object-editing` criterion 12). Given a
    double-click on a group handle, the entries of Part D open (and the
    object-to-own-tool handoff never happens for a handle).
31. Given a gesture is released, then exactly one commit is made for all selected
    objects together. It is atomic: no reader of the document, no saved file and
    no peer sees some objects transformed and others not. Each object receives the
    writes that the single-object gesture of the same kind writes for it
    (`specs/0005-object-transform/adrs.md`, merge granularity: a move or scale
    writes frame or anchors, a rotate adds `rotation` and, off-centre, the frame).
    One write goes beyond the single-object gesture: a circle (rx = ry within
    tolerance) at a rotation that is not a multiple of 90° that is stretched
    (sx ≠ sy) becomes an ellipse with `rotation` 0 (criterion 20, D3). Nothing
    else is written: no style field, no new register. Undo is not built
    (slice 8): the commit is final, which the architect and the lead mention at
    the demo.
32. Given a gesture on a multi-selection, then for the whole drag every selected
    object is drawn twice, as `unified-object-editing` criteria 10 to 14 define:
    its committed geometry unchanged in its own style, and the geometry the
    release would commit as the 1.5 px blue outline (at every selection size; the
    architect set no simplified preview, criterion 49); a filled or translucent
    object keeps its fill and shows no new fill. The preview and the commit come
    from the same function with the same inputs (the single-object rule, for every
    kind in the selection). The hover highlight is suppressed during the drag
    (`unified-object-editing` criterion 15a).

### Part D: typed values and keys (separable, default included)

A multi-selection has no stored orientation, so a typed angle can only be
relative; every other entry has the same fixed point as the drag.

33. Given a multi-selection, when the maker double-clicks a rotate handle or
    presses R (the Select tool, the gate of `edit-interaction-polish` criterion
    55), then an angle chip opens at the handle (R: at the top-right corner rotate
    handle, drawn or not), with one field, the suffix "°", the accessible name
    "Rotate selection by", a visible label "Δ" at the left edge (as the "W" and
    "H" labels of the size chip; a selection has no absolute angle, so the number
    must not look absolute), prefilled "0" and selected. Enter turns the selection by
    the typed Δ in one commit, about the pivot fixed when the chip opened (box
    centre; opposite corner or side with Shift at the second press of the
    double-click; R always the box centre), and the document ends as a drag of
    that Δ would leave it. Typing 0, an unedited Enter and a value that turns by a
    multiple of 360° write nothing. A decimal comma, a point, U+2212 and a
    trailing "°" are accepted; a non-finite value keeps the chip open and marked
    invalid ("Enter a number"). The rules for Escape, blur, a tool switch and the
    unswallowed closing press are those of `specs/0008-object-transform-refinements/`
    criteria 19 to 23.
34. Given a multi-selection, when the maker double-clicks a resize handle or
    presses S, then a size chip opens with "W" and "H" (accessible names "Width"
    and "Height", in mm), prefilled with the group box size. The fixed point is
    what a drag of that handle would hold (opposite corner or side; the centre with
    Shift at the second press) and for S always the box centre, shown by the pivot
    marker (`edit-interaction-polish` criterion 57a). Enter scales the selection
    to the typed size in one commit, with both switches read when the chip opens.
    For a uniform-only selection the two fields are linked (typing one sets the
    other at the box's aspect ratio, shown with the chain glyph); its edge handles
    are not drawn, so there is no single-field edge entry. A size of
    zero or less keeps the chip open and invalid ("Must be above 0"); a result
    beyond ±1e7 mm is invalid ("Too large"); an unedited Enter or a size equal to
    the current one writes nothing.
35. Given a multi-selection, when the maker double-clicks the centre move handle
    (inside its hover region, criterion 11) or presses M, then the move chip of
    `edit-interaction-polish` criteria 15 to 25 opens, with the same Relative and
    Absolute modes, the same key order and the Copy check. Relative translates
    every selected object by (X, Y). Absolute puts the top-left corner of the
    **group box** at (X, Y) in document coordinates; it is the same box the group
    handles use and the same bounds the single object's absolute move uses
    (criterion 1). The "Absolute" segment states its reference: its `title` and
    accessible description read "Top-left corner of the selection". A copy (Ctrl at the second press, or the check) copies every
    selected object with the displacement and selects the copies.
36. (Part E, kept: question 4 decided (a) 2026-10-08.) Given a selection of paths only, when the maker double-clicks a skew
    handle, or presses K (top handle) or Shift+K (right handle), then the skew
    chip of `edit-interaction-polish` criteria 9 to 13 opens and applies the
    typed relative angle to every path, about the line a drag of that handle holds
    fixed.
37. Given the keys M, R, S and K with nothing selected, then the hint "Select an
    object first" shows as today; given K or Shift+K with a multi-selection that
    contains a rectangle, ellipse, polygon or star, then the hint "Skew works on
    paths only" shows and nothing changes. The hint "Select one object to type a
    value" no longer exists.
38. Given the pointer rests on a group handle for 600 ms, then a hint chip appears
    (same chip, `pointer-events: none`, wrapped at 240 px) with these lines:

    | Handle | Lines |
    |---|---|
    | Corner resize | "Resize selection" / "Shift: from the centre" / "Ctrl: keep proportions" / "Double-click or S: type a size" |
    | Edge resize | the same without the Ctrl line |
    | Corner resize, uniform-only | "Resize selection, proportional only" / the cause line of criterion 21 / "To stretch: Object to path first" / "Shift: from the centre" / "Double-click or S: type a size" |
    | Corner rotate | "Rotate selection" / "Shift: pivot at opposite corner" / "Ctrl: snap" / "Double-click or R: type an angle" |
    | Side rotate | "Rotate selection" / "Pivot: opposite side" / "Ctrl: snap" / "Double-click or R: type an angle" |
    | Skew | "Skew selection" / "Shift: from the centre line" / "Ctrl: snap" / "Double-click or K: type an angle" (left and right handles: "Shift+K") |
    | Centre | "Move selection" / "Shift: keep one axis" / "Ctrl: copy" / "Double-click or M: type an offset" |
39. Given the Select bar with a multi-selection, then its content follows
    `unified-object-editing` criteria 21 to 23 unchanged (the two switches first,
    then the kind groups the selection contains, then "Object to path"); the only
    text change is the tooltip of "Object to path" (criterion 21). This feature
    adds no control to the bar or to the Properties panel.

### Part E: skew (separable; kept, question 4 decided (a) 2026-10-08)

40. Given a multi-selection of paths only (open or closed), whose group box has
    non-zero extent in the axis the handle skews along, then four skew handles are
    shown, one per side of the group box, with the layout, glyph, 24 px lever rule
    and hit rules of `specs/0008-object-transform-refinements/` criteria 37 and 48. A
    selection that contains any rectangle, ellipse, polygon or star shows no skew
    handle at all (hidden, not inert), and nothing skews: this is the rule
    `specs/0008-object-transform-refinements/` criterion 53 recorded in advance, and it
    follows the single-object decision P1 (a primitive cannot be skewed). The same
    box that holds a path and a primitive therefore has fewer handles than the box
    of two paths. (Default D4.)
41. Given a drag of a skew handle, then every path is sheared by the same linear
    map in the document axes: the line of the group box's opposite side stays
    fixed (with Shift, the line through the box centre), every anchor moves along
    the side's direction in proportion to its distance from that line, and the
    skew angle is `atan(d / h)` with `h` the distance from the fixed line to the
    grabbed side at the press (`specs/0008-object-transform-refinements/` criteria 38
    to 40). Ctrl snaps the angle to the stops of criterion 27, capped at ±75°. A
    path's `rotation`, node kinds, count, order, closed state and stroke width are
    unchanged; a skew by α then by -α with the same handle restores every anchor
    and handle vector within the geometric tolerance. Example: two closed
    10 × 10 paths at (0, 0) and (20, 0), the top handle, α = 45° with the bottom
    side fixed: every point at y = 0 moves 10 mm in x, every point at y = 10 does
    not move.
42. Given a skew gesture, then the readout "Skew x +12.5°", the pivot marker and
    the fixed-line guide of `specs/0008-object-transform-refinements/` and
    `edit-interaction-polish` criterion 68 appear as for a single path, the box in
    the preview is the axis-aligned bounds of the sheared preview, and the writes
    are anchors and handle vectors only.

### Part F: press order, hover and double-click

43. Given a multi-selection and a press, then the order is:
    1. Alt held: the lasso starts, also on a handle (`advanced-selection`
       criteria 16 and 17);
    2. a hit on a drawn group handle (criterion 11): that handle's drag; a Shift
       press on a visible side rotate handle starts a rotate, it does not toggle;
       Shift or Ctrl on the centre handle is a move with modifiers
       (`edit-interaction-polish` criterion 38);
    3. Shift held: an outline or filled-interior hit within the 8 px tolerance
       toggles that object (a click) or starts an axis-locked move (a drag past
       the dead zone, `edit-interaction-polish` criterion 29); on a point that hits
       no object, inside the group box or not, Shift arms the add marquee, as
       today;
    4. Ctrl held: on a hit of a selected object's outline or filled interior, a copy
       move (`edit-interaction-polish` criterion 37); on a point that hits no
       object, inside the group box or not, the remove marquee, with the minus
       badge (`advanced-selection` UX notes), as today;
    5. a plain press with a hit (outline within 8 px, or filled interior) on a
       **selected** object: a move of the selection (criterion 16);
    6. a plain press with a hit on an **unselected** object: that object is
       selected, replacing the selection (`advanced-selection` for outlines, `0007`
       criterion 29 option B for filled interiors), inside the group box or not;
    7. a plain press on nothing, inside the group box or not: the marquee starts
       on a drag, and a click clears the selection at release, as today
       (criterion 45).
    Where a selected and an unselected object are both hit at the press, the
    existing hit order of `advanced-selection` and `0007` criterion 29 decides,
    unchanged. The Alt-click cycle is unchanged.
44. Removed 2026-10-08 (question 3 decided (b)): the exception that gave a plain
    press inside the group box to an unselected filled object above the selection
    is no longer needed; criterion 43 step 6 covers every unselected object. The
    number is kept so that `adrs.md` references stay valid.
45. Given a plain press on empty canvas (no object hit, no handle hit) inside the
    group box, then it behaves exactly as it does today for a multi-selection:
    a drag starts the marquee (Alt the lasso, Shift adds, Ctrl removes, none of
    them needing Escape first) and a click clears the selection at release. The
    group box has no hit area of its own; the selection is moved only by the
    centre handle or by a press on a selected object (criterion 16). Question 3,
    decided (b).
46. Given the pointer inside the group box, then the hover highlight is the one of
    today for the object under the pointer, and none on empty canvas. The cursor is
    the Select tool's normal one inside the box, `move` over the centre handle, and
    the handle cursors of a single object over the other handles. From the release
    of a gesture until its commit is presented, the cursor is `wait`.
47. Given a double-click on a group handle, then criteria 33 to 36 apply; given a
    double-click anywhere else, then it behaves as it did before this feature.

### Part G: size of the selection, robustness, persistence

48. Given a multi-selection of 200 objects (100 paths with 50 nodes each, 100
    rectangles), in a native release build (`#[ignore]` benchmarks on the build
    host, as in `unified-object-editing`), then a preview frame of a scale, rotate
    or skew gesture takes at most 1.1 times a plain move preview frame of the same
    selection, measured in the same run on the same machine; and computing the
    group box of the 200 objects takes under 1 ms. The absolute 8 ms of
    `unified-object-editing` criterion 15 is not a gate here: the plain move
    preview already takes 13.8 to 15.6 ms at 200 objects
    (`docs/technical-debt.md`), so it cannot pass; it stays the target of the
    draw-list cache item. (Architect decision 3, `adrs.md`.)
49. Given a multi-selection of 10,000 objects (5,000 paths with 20 nodes, 5,000
    rectangles), in a native release build, then: computing the group box takes
    under 20 ms; one preview step (resolving the gesture for all objects plus the
    preview box) takes under 50 ms; the commit of a move, scale, rotate or skew
    each finishes within 5 s; the preview box shown is the box that would be
    committed (for a rotate, the turned box of criterion 28); Escape cancels the
    gesture without writing anything. From the release until the commit is
    presented the cursor is `wait` and the preview stays unchanged. There is no
    simplified preview in this feature. The end-to-end pointer-to-frame time, the
    frame time at rest, the commit time of a copy, the browser (WASM) commit times
    and the bytes a 10,000-object scale adds to the saved file are measured and
    reported in the PR description, not gated: a full-fidelity frame of 10,000
    objects needs the draw-list cache (out of scope). (Architect decision 3,
    `adrs.md`; D7.)
50. Given a project in which a multi-selection was transformed, when the maker
    saves, closes and reopens it, then every path's anchors and `rotation`, every
    primitive's frame, parameters and `rotation` are exactly as before closing, and
    every primitive is still the same kind. A saved file never contains preview
    data or a group box. **Expected: no file format change, no new field, no
    `format_version` change** (every write is a field the single-object gestures
    already write; the architect confirms in `adrs.md`).
51. Given a multi-selection that contains a filled or translucent object, then the
    transform changes only geometry fields; the style fields are untouched. The
    group box interior has
    no hit area; presses follow criterion 43, in which a selected object's filled
    interior is a hit.
52. Given a selection of one object, or a selection that has just dropped from two
    to one, then every criterion of the earlier specs for a single object holds
    unchanged, and every single-object test of those specs still passes.

## Out of scope

- A **group object**, layers, locking and hiding (`layers-and-grouping`, slice 10).
  Design note so this feature does not block it: the per-object results of
  criteria 20, 26 and 41 are functions of one object and a map in the document axes,
  so a future group can call them with its own frame; this feature stores no group
  data, so a later group can choose its own storage (ADR 0002 section 5's affine);
  selecting a future group uses the single-object rules with its stored
  orientation, while an ad-hoc multi-selection keeps this spec; locked and hidden
  objects will need rules there (this spec has no such objects).
- A **draw-list and snapshot cache** (the "Canvas performance" item of
  `docs/technical-debt.md`). Ten frames per second at 10,000 selected objects
  needs it; it is a separate `chore/`, not this story (criterion 49).
- A **persistent or session-long group orientation** (a box that keeps its turn
  after a rotate): question 1, option (b); the customer decided (a) on
  2026-10-08.
- Moving the selection by a press on the **empty interior** of the group box
  (question 3, option (a), declined by the customer 2026-10-08).
- Skew, shear or non-uniform scale of polygons, stars and misaligned rectangles
  and ellipses, and any conversion to a path (the decisions P1 and criterion 21 of
  `0003`). The remedy is "Object to path" first.
- A movable rotation pivot, flip and mirror, Transform Again, saved transforms,
  numeric transforms in the Properties panel, keyboard nudging, snapping to grid,
  objects or guides.
- Select all (Ctrl+A) and other ways to make large selections; the 10,000-object
  case comes from marquee and lasso.
- Per-object independent transforms (each object about its own centre), "distribute"
  and "align".
- Parameter handles (radius, ratio) on a multi-selection
  (`unified-object-editing` criterion 37); the bar's "Radius", "Points" and
  "Ratio" fields already act on several objects.
- Undo and redo (`undo-redo`, slice 8).

## Open questions

Questions 1 to 4 are **decided by the customer / lead on 2026-10-08**: 1 (a),
2 (a), 3 (b, a change from the product owner's default), 4 (a). The text of each
is kept for the reasons.

1. **Does the group box turn with the rotation and keep its turn?** Decided: (a).
   (a) Default, recommended: no. During the drag the box turns with the gesture,
   on release it is refit to an axis-aligned box around the result (criteria 7,
   28). A rotated arrangement has a bigger box, and rotating it back by dragging
   gives back the old box only when the geometry is back. This is what Inkscape and
   LightBurn show and it needs no group object. A typed angle is then relative
   (Δ). (b) The box keeps the turn while the same selection stays selected (a
   session-only angle, not stored; lost on any selection change). It gives the
   oriented box you liked on a single object, but the angle disappears when you
   select anything else, so the same arrangement looks different a minute later,
   which is the Illustrator "box silently resets" complaint. (c) Wait for groups
   (slice 10), where a group can store its orientation like an object.2. **Stretching a selection that holds a polygon, a star or a rotated rectangle.**
   Decided: (a), proportional only, with the hint. These cannot be stretched without becoming something else. (a) Default,
   recommended: the whole selection can only be scaled proportionally, the edge
   handles are not drawn and the hint says why (criterion 21); "Object to path"
   first removes the restriction. (b) Every such object is converted to a path on
   the first stretch (rejected earlier: no implicit conversion). (c) Such an
   object is scaled by an approximate rule that discards the shear, so it changes
   less than the paths next to it.
3. **A press on empty canvas inside the group box.** Decided: (b). Today it
   clears a multi-selection (click) or starts a marquee (drag). (a) was: it moves
   the selection, as inside a single object's box, and a click there does not
   clear it. (b), chosen: only the centre handle and the selected objects' own
   outlines and filled interiors move the selection; the empty interior stays
   "clear or marquee", exactly as today (criteria 16, 43, 45). Reason (UX notes
   section 4): a group box is mostly empty space, and under (a) a press on an
   unselected neighbour would move the whole selection instead of selecting it.
4. **Skew for a multi-selection (you wrote it in brackets).** Decided: (a)
   include, for selections of paths only (Part E and criterion 36); a selection
   with a primitive shows no skew handle.

Design defaults the `product-owner` set; change any you disagree with, none needs an
answer:

- **D1. Which bounds.** The group box is the tight bounds of the drawn outlines,
  no stroke, not a union of the oriented boxes (criterion 3): a path turned
  earlier keeps a stale oriented box much larger than its curve, and a union would
  show a box with air around it. The same bounds already define the typed absolute
  move, so the box and "Absolute" agree.
- **D2. One box or two.** Both: the group box with handles, and the per-object
  boxes, lighter (criterion 4), as the customer asked for an additional box; the
  per-object boxes stop above 500 selected objects (criterion 5; a count, no
  measurement needed).
- **D3. Circles.** A circle counts as aligned whatever its rotation, since its
  outline does not depend on it, so a rotated group of circles and paths can still
  be stretched (criterion 20).
- **D4. Mixed skew.** A selection with any primitive does not skew at all, the
  rule recorded in `specs/0008-object-transform-refinements/` criterion 53. Refusing
  the whole gesture, rather than skewing the paths only, keeps the arrangement
  consistent: no object is left behind.
- **D5. Scale to zero.** A factor clamps at 0 as for a single object, because a
  second rule would be a second thing to learn; Escape is the way back while
  dragging. Typed sizes refuse 0. Revisit with `undo-redo`.
- **D6. A filled object above the selection.** Removed with criterion 44
  (question 3 decided (b)). A plain press on any unselected object, filled or
  not, selects it, as today; there is no special press rule inside the group box.
- **D7. Budgets.** Set by the architect (`adrs.md` decision 3), replacing the
  product owner's first proposal: the feature's own costs are gated (criteria 48
  and 49), end-to-end frame times are reported. The existing plain move already
  takes 13.8 to 15.6 ms at 200 objects, so the 8 ms rule cannot be a gate, and a
  full-fidelity frame of 10,000 objects needs the draw-list cache (out of scope).
- **D8. Stroke width and radius.** They follow the two Select-bar switches, off by
  default, with the single-object rule √(sx·sy) when on (criterion 24).

Defaults from the `ux-engineer` (UX notes section 15), kept unless the customer
objects; none needs an answer:

- **U1. A member box sticking out of the group box.** A turned path keeps a stale
  oriented box larger than its curve, so its light dashed member box can lie
  outside the heavy group box. Default: kept as it is (true to what a single
  selection shows). Alternative: member boxes use the tight outline bounds, which
  loses the visible orientation. Decided at the screenshot review.
- **U2. The jump of the box on a rotate release.** Default: instant (criterion 28).
  Alternative: a 120 ms interpolation of the four corners, none under
  `prefers-reduced-motion`. Cheap to add later, no model change.
- **U3. The 500 limit** (criterion 5) is a count. If the review of a 600-object
  selection shows the missing member boxes matter, raise the limit before adding
  anything else.
- **U4. Screen-reader live region (Should).** A visually hidden polite live region
  on the canvas container speaks the selection when it changes ("4 objects
  selected, 46.2 by 18.7 mm", at most once per 500 ms). Default: built with this
  feature; dropped if the key handler refactor makes it expensive. It is not an
  acceptance criterion.

## For the architect (`adrs.md`)

Answered in `adrs.md` of 2026-10-08: budgets (criteria 48 and 49, decision 3), the
resolving function and group map (decisions 1 and 2), writes and format
(criterion 50, decision 5), `object_outline_bounds` at 10,000 objects, and the
forward note for groups. Two points of `adrs.md` follow the old press rules and
need updating for question 3 (b): decision 4 ("criterion 44",
`filled_interior_above` over a selection, `InsideSelectedBox` for a point inside
the group box) and "Dropped if the customer answers 4(b)" (decided 4(a)).

## UX notes

Status: complete (`ux-engineer`, 2026-10-08). Sizes, tokens and component rows
are in `docs/design-system.md` (rows "Group selection box", "Member box",
"Transform handle layout", "Transform handle hint chip", "Transform entry chip"
and the rows they extend); this section gives the decisions and the reasons.
Distances are screen pixels. `s` is the shorter side of the group box on screen.
Where a decision departs from the criteria it says "Change wanted", and section
14 lists those by number.

**Update 2026-10-08 (product owner, UX):** every change wanted in section 14 is
applied to the criteria. Question 3 is decided as option (b); section 4 now states
the decided press model. The architect's `adrs.md` decision 3 means there is no
simplified preview in this story (section 12 keeps the look as a dormant rule) and
the 500 limit needs no measurement.

### 1. Vocabulary: "selection" in the UI, "group" nowhere

The UI strings say "selection" ("Resize selection", "Rotate selection by"). The
word "group" is kept for the future group object (`layers-and-grouping`): a
maker who sees "group" today will look for Ungroup. "Group box" is a term of this
spec and of the design system, not of any visible text.

### 2. The group box and the per-object boxes

| Part | Look |
|---|---|
| Group box | The look of a single object's box, unchanged: 1 px `--accent`, dashed 4 / 3 (V1), laid out from the first corner and fitted per edge so every corner is closed, pixel-snapped (it is axis-aligned, so always), white casing under the dashes. Nothing new to learn, which is the point of the customer's request |
| Per-object ("member") box | The same pattern, lighter: 1 px, dashed 4 / 3, `--member-box` (`--accent` at 60%, casing at 60% under the dashes; the colour of `--shape-handle-guide`, the hover box's treatment). 2.1:1 on `--canvas-bg`; not yet measured on fills, the review measures it on the fills listed under "Casing over artwork" and accepts 1.9:1 or better. In its own oriented frame, as today. Drawn below the group box |
| Hover box | Unchanged (`--hover-box`, solid). A hovered member adds no line, as for a single object |

**Why lighter by opacity and not by weight or a new dash.** 1.5 px accent
outlines mean "what the release commits" (blue preview) and nothing else may use
them, so the group box cannot be heavier. A third dash rhythm next to V1 (4 / 3),
the skew guide (2 / 2) and the lasso (4 / 3, 1.5 px) would be one more thing to
tell apart. The group box is the dominant line for three reasons that do not rest
on lightness: it is the outermost line, it is the only one with handles, and the
members are dashed at 60%. 2.1:1 is below the 3:1 non-text minimum on purpose:
membership is also carried by the group box and the count in the Properties
panel's subject line, the same reasoning that exempts the hover box.

**Coinciding edges.** Every member box shares at least one edge position with the
group box, and two 4 / 3 patterns on one edge fill each other's gaps into a heavy
near-solid line (the problem the skew guide had on the fixed edge). Rule: the part
of a member box edge that lies within 1.5 device pixels of a group box edge is not
drawn. The group box is the only line there.

**When member boxes are not drawn.** (a) More than 500 selected objects: none
(criterion 5; counted on the selection, not on what is visible, so panning and
zooming never make them flicker). (b) A member whose box is under 6 px on both
sides, or entirely outside the viewport: not drawn. (c) During a rotate, resize
or skew drag they follow their preview geometry like every other part. The group
box and its handles are never subject to these rules. Crossing 500 (a Shift-click
adds the 501st) removes all member boxes in the same frame, no fade; the subject
line "501 objects" is the only signal, which is enough because the group box is
still the selection.

**A member's box can stick out of the group box.** The group box is the tight
bounds of the outlines (criterion 3), a member's box is its own oriented box, and
a path turned earlier has a stale oriented box much larger than its curve. A
light dashed line outside the heavy one is odd but true to what a single selection
shows; kept (question A).

**Degenerate box (criteria 8, 12, 13).** Zero extent in one axis: the box is
drawn as one dashed line, once (the two coincident edges are not both drawn). The
remaining handles follow criterion 12. Corner rotate handles stand 45 px apart at
each end (above and below the line on the notional square's diagonals). Zero in
both axes: no box dashes and no handle; a 6 px square outline in `--accent`, solid,
1 px, with casing, marks the point so a selection is never invisible.

### 3. Handles and tiers of a group box

The same glyphs, offsets, hit caps and tie order as one object, in the group
box's frame (always axis-aligned, so every glyph and cursor is at 0°). Tiers on
`s`: under 24 corner resize and corner rotate; 24 to 47 plus edge resize; 48 and
up plus the centre handle. There is no 72 px tier: a multi-selection has no
parameter handles. Skew: per axis, 24 px lever rule, paths only (section 8). Edge
resize handles of a uniform-only selection are not drawn and have no hit area at
any size (criterion 21); under 24 px the edge handles of other selections are
hidden but hit-testable (criterion 10, unchanged).

Handles are drawn above all artwork, the member boxes and the blue preview. A
group box is made of the objects' own extremes, so edge-midpoint handles and the
centre handle often sit on an object; their white ground and the nearest-centre
test (criterion 11) win over the object, as they do for one object. The way to
reach the object under a handle is to zoom in or to press outside the handle's
cap. Rotate and skew handles stand 16 to 32 px outside the box and take a press
before any object near them. Known cost, same as for one object; in a crowded
drawing it shows more often. The review checks a 40-part laser layout.

The pivot marker (6 px, `--accent` at 60%) and the rule that a handle exactly at
the pivot is not drawn are unchanged. For a group the default pivot is the group
box centre, so the centre handle is hidden while a rotate (default pivot) or a
Shift scale runs, and the marker sits where it was.

### 4. Press model and cursors

**Decided by the customer, 2026-10-08 (question 3, option b): the empty interior
of the group box does not move the selection.** The group box has no hit area of
its own. Reasons, kept because they are the test for any later change:

1. A single object's box is tight around one thing; a group box is the bounds of
   many and is mostly other space. Two parts at the corners of a job enclose the
   whole sheet, so "empty" there is not "this selection".
2. The unselected objects inside it are the maker's next targets, and laser and
   plotter work is mostly outlines. A press on one selects it, as today; it never
   moves the whole selection.
3. A click on empty canvas deselects, as today, wherever the box is large.

What moves the selection: the centre handle (its hover region, drawn from `s` of
48), a drag from the outline (8 px tolerance) or the filled interior (option B of
`0007`) of a **selected** object, and the typed move. Under 48 px, or where the
centre handle is hidden, the selected objects themselves are the grip. A press
where a handle is not drawn is an ordinary press. Order of a press: criterion 43.

**Hover and cursor.** Inside the group box nothing is highlighted and the cursor
is not changed by the box: on empty canvas the Select arrow (and, as today, the
marquee on a drag); over any object the hover highlight and cursor that object
gets today. The only things the box adds are the handles. A pointer inside the box
on empty canvas must look exactly like a pointer outside it.

**Cursors** (same mechanism, no new assets). Resize: the four stock angles
(`nwse`, `nesw`, `ns`, `ew`), since the box is axis-aligned. Rotate: the existing
non-rotating arc cursor, all eight. Skew: the existing two-arrow cursor at 0°
(top, bottom) and 90° (left, right). Centre handle: built-in `move`. Elsewhere:
the Select arrow. Modifiers never change a cursor. From release until the commit
is presented, `wait` (section 12).

### 5. Move, copy and axis lock

The modifier indicators of `edit-interaction-polish` apply unchanged
(`docs/design-system.md`, "Modifier indicators in a move").

- **Copy (Ctrl):** the plus badge shows wherever a Ctrl press would start a copy
  move, which is what criterion 43 step 4 says: over the outline or filled interior
  of a **selected** object, and over the centre handle. Not over an unselected
  object and not on empty canvas, where Ctrl is the remove marquee with its minus
  badge. The badge uses the press test, so it never promises a copy the press would
  not start. "Copy" at the end of the readout. In a copy drag **the group box, its
  handles and the member boxes stay on the originals**; the blue outlines travel
  alone. On release the selection becomes the copies and the box moves to them in
  that frame.
- **Axis lock (Shift):** the two origin axes pass through the group box centre at
  the press, which is where the centre handle is drawn, whether or not the handle
  is drawn. Full-viewport lines, `--axis-guide` on the locked axis and
  `--axis-guide-idle` on the other, 1 px, solid. If the centre is far outside the
  viewport the lines may not show; the lock badge and the readout ("Δ 30.0, 0.0
  mm") carry the same information and are the primary signals.
- Readout: "Δ 12.5, −3.0 mm" as for one object.

### 6. Scale

- **Readout:** "123.4 mm × 67.8 mm", the size of the group box in the preview,
  also for a uniform-only selection (not "r R mm": there is no single radius). The
  pivot marker shows the fixed point (opposite corner or side midpoint, centre
  with Shift), as for one object.
- **Uniform-only selection** (criterion 21): the four edge handles are absent. The
  absence alone explains nothing, so the corner handles' hint chip (section 10)
  names the cause and the way out, and the tooltip of "Object to path" says what it
  buys (section 11). No toast, no message on selection: the maker who never stretches
  is not interrupted. The Ctrl line is missing from the corner chip because Ctrl
  cannot change anything.
- **Factor clamps** (criterion 22): at 0 the readout shows the real size
  ("0.0 mm × 40.0 mm") and the preview shows what a release would write, a collapsed
  selection. No extra message: Escape is the way back, and the readout is the
  warning. The 1e7 limit stops the handle; no message.

### 7. Rotate: the preview and the refit

- During the drag the preview group box is the box at the press turned by the live
  Δ about the live pivot, with its handles; glyphs are not turned (the arcs never
  are; resize glyphs are squares), the dashes are laid out in the turned frame
  and anti-aliased, as for a rotated single object. The member boxes turn with their
  objects. The pivot marker sits at the pivot.
- **Readout:** "Δ 37.4°", clockwise positive, real minus sign (U+2212), one
  decimal at most ("Δ 45°" under Ctrl), shown in (−180°, 180°] like the rotate
  readout of one object. "Δ" tells the maker it is a turn since the press, not an
  absolute angle (a single object's readout is "37.4°").
- **On release** the box is replaced in one frame by the axis-aligned box around
  the result, usually larger; handles move with it. No animation (the canvas has none
  and a commit is a discrete event). This is the one place a maker may be
  surprised, since the box they just saw turned is gone. It is what Inkscape and
  LightBurn do and what question 1 (a) asks for. If the customer finds the jump
  jarring, the fix is a 120 ms ease-out interpolation of the four corners from the
  turned box to the refit box (none under `prefers-reduced-motion`); not built now
  (question B).

### 8. Skew (Part E)

Glyph, offsets, 24 px lever rule, hover, cursor and hit radius as for a path.
Readout "Skew x +12.5°" / "Skew y −8.0°" ("22.5°" under Ctrl). The fixed-line guide
runs along the group box side (or the line through the centre with Shift), 1 px,
`--transform-guide`, dashed **2 / 2** (the single-object value, so it never looks
like the 4 / 3 box); while a skew drag runs the group box does not draw its own
dashes along the fixed edge (the guide replaces them: that side 2 / 2, the other
three 4 / 3), and member box edges that coincide with it are already skipped. The
group box in the preview is the axis-aligned bounds of the sheared result.

**Hidden, not inert** (criterion 40, D4): a selection holding a rectangle, ellipse,
polygon or star shows no skew handle. Nothing on screen explains it, as for one
object. The key does: `K` / `Shift+K` show "Skew works on paths only" (criterion
37). After "Object to path" the four arrows appear in place with no animation. The
review checks a mixed selection screenshot with the skew handles absent.

### 9. Typed entry chips

All chips are the "Transform entry chip" and "Move entry chip" surfaces, opened
by double-click on the handle or by `M`, `R`, `S`, `K`, placed as for one object
(a key places the chip where the handle would be, drawn or not; the box centre for
M and S). A group box can be larger than the viewport; then the handle is off
screen and the chip clamps at the canvas edge, which is the existing rule. The
pivot marker shows the fixed point for as long as the chip is open.

| Chip | Content | Difference from one object |
|---|---|---|
| Angle | One 80 px field, visible "Δ" at the left edge (12 px, as the "W" and "H" labels), fixed "°" suffix, prefilled "0", selected. Accessible name "Rotate selection by" | The prefill is 0 and the label is Δ: a selection has no absolute angle, so a number that looks absolute would mislead |
| Size | "W" and "H" fields, mm, prefilled with the group box size. Accessible group name "Resize selection", field names "Width", "Height" | Uniform-only: the two fields are linked (typing one sets the other at the box's aspect ratio) and the 12 px chain glyph sits between them, as for a Ctrl-linked chip; there is no one-field edge entry |
| Move | The move chip. Absolute puts the group box's top-left corner at (X, Y) | The "Absolute" segment gets `title` and accessible description "Top-left corner of the selection" so the reference is stated (one object needs no sentence) |
| Skew | The skew chip, about the fixed line of its handle | Paths only |

Errors, Escape, blur and Enter are the rules of the "Numeric entry chip": "Enter a
number", "Must be above 0", "Too large"; the error line is a polite live region
and the border plus message carry the state, not colour alone.

### 10. Hover hint chips

Same chip, 600 ms, `pointer-events: none`, now with a title that names the
selection. Up to five lines; the chip wraps at 240 px.

| Handle | Lines |
|---|---|
| Corner resize | "Resize selection" / "Shift: from the centre" / "Ctrl: keep proportions" / "Double-click or S: type a size" |
| Edge resize | the same without the Ctrl line |
| Corner resize, uniform-only | "Resize selection, proportional only" / "Holds a star and a rotated rectangle" / "To stretch: Object to path first" / "Shift: from the centre" / "Double-click or S: type a size" |
| Corner rotate | "Rotate selection" / "Shift: pivot at opposite corner" / "Ctrl: snap" / "Double-click or R: type an angle" |
| Side rotate | "Rotate selection" / "Pivot: opposite side" / "Ctrl: snap" / "Double-click or R: type an angle" |
| Skew | "Skew selection" / "Shift: from the centre line" / "Ctrl: snap" / "Double-click or K: type an angle" (left and right: "Shift+K") |
| Centre | "Move selection" / "Shift: keep one axis" / "Ctrl: copy" / "Double-click or M: type an offset" |

The cause line of the uniform-only chip lists only the kinds that are present, in
the order polygon, star, rotated rectangle, rotated ellipse, each with "a", joined
by commas and "and" ("Holds a polygon, a star and a rotated rectangle"). It names
what to fix; it does not count. "Rotated" means a rectangle or ellipse whose
rotation is not a multiple of 90° (a circle never counts).

### 11. Select bar and Properties panel

**Select bar: unchanged** (criterion 39). The two switches first, then the kind
groups the selection contains, then "Object to path". No "N objects" text, no group
size, no transform fields: the count would shift the bar, and the numbers live in the
readouts and chips. The count is already in the panel. One text change: the title of
"Object to path" becomes "Convert the selected shapes to paths. Paths can be
stretched and skewed freely." It is the only discovery path for the way out of a
uniform-only selection.

**Properties panel, Style section: unchanged.** Subject line as `0007` has it
("3 rectangles", "4 objects", "2 paths"); mixed values by the house rule (empty
field with "Mixed", hatched swatch, no segmented item pressed). Nothing the
transform writes changes a style field, so the panel does not refresh during a
gesture except for the subject line when the count changes.

### 12. Feedback in the preview, and a selection of thousands

- **Blue new, black old** for every selected object, as `unified-object-editing`
  defines it: the committed geometry in its own style, the geometry a release would
  commit as the 1.5 px blue outline, no fill preview. The group box, handles, pivot
  marker and readout follow the preview and draw above the blue. The hover highlight
  is suppressed. Nothing blue remains after release, Escape or a drag back to the
  start.
- **Order, bottom to top:** artwork, axis lines, member boxes, blue outlines, group
  box, handles, DOM (readout, badges).
- **No simplified preview in this story** (`adrs.md` decision 3): every selected
  object gets its blue outline at any count. Dormant rule, kept so it need not be
  designed again if a threshold is ever added: per-object outlines for part of the
  selection would read as "the others stay", so a simplified preview draws no
  per-object outline, draws the preview group box as a solid 1.5 px `--preview-new`
  rectangle with its handles (the one other use of that outline: it is still the
  geometry a release would commit) and adds the readout line "Preview simplified".
  Nothing in this feature draws it.
- **After release:** from the release until the commit is presented, the cursor
  is `wait` and the blue preview stays (no frame of old geometry, no flash). The
  commit of 10,000 objects may take seconds (criterion 49); there is no dialog and
  no change of the preview. Escape
  during the drag cancels within one frame. A copy of thousands is reported by
  time in the PR, not by a message.
- **First frame:** the group box and its handles are drawn before the member
  boxes and before any per-object work, so a large selection shows its box within
  the 200 ms of criterion 49 and the rest follows.

### 13. Marquee and lasso, accessibility

**Marquee and lasso.** Unchanged under (b): a drag from empty canvas, also inside
the group box, is the marquee, Alt the lasso; Shift adds, Ctrl removes (minus
badge). They draw above the group box (1.5 px, green or red, against the group
box's 1 px dashed blue). The group box and the member boxes update when the
marquee is released, not live. A press within a handle's cap takes the handle,
Shift or not (criterion 43 step 2): a Shift-drag that starts 20 px from a corner,
meant as an add marquee, becomes a rotate. This is the single-object rule; with a
multi-selection Shift is a more frequent modifier, and Shift also reveals the four
side rotate handles. The review checks it with a scene where the first selected
object is near the box edge.

**Accessibility and non-colour cues.**

- Group box against member box: position (outermost), dash opacity, and the handles,
  not colour alone. Handles are told apart by silhouette (square, arc arrow, two
  arrows, four-way square), as for one object. Copy and lock are a plus and an arrow
  in the badge plus a word and numbers in the readout; the axis lines are
  informational. Uniform-only is stated in words in the chip, not only by absent
  handles.
- Keyboard route to every group transform: `M`, `R`, `S`, `K` / `Shift+K` open the
  same chips, on the canvas, with the Select tool active and a selection (the gate of
  the Keyboard concept). Canvas handles are not in the accessibility tree (known
  gap, `object-transform-refinements`); the chips are.
- Chips: visible labels (Δ, W, H, X, Y), accessible names of criteria 33 to 35,
  `aria-invalid` and a message line, a polite live region; Escape closes, Tab wraps.
- Should (small): a visually hidden polite live region on the canvas container that
  speaks the selection when it changes: "4 objects selected, 46.2 by 18.7 mm"
  (count and group box size, one sentence, no more than once per 500 ms). It adds
  no visible control and gives a screen-reader user the one fact the canvas draws.
- Contrast: group box 3.7:1 on `--canvas-bg` and cased over fills; member box 2.1:1
  (exempt, see section 2); handles unchanged. Hit targets: unchanged (16 px caps,
  skew 12 px, tier rules). No new animation, so `prefers-reduced-motion` has nothing
  to switch off; the optional morph of question B would.
- Large documents: member boxes stop at 500, are skipped under 6 px, and are
  clipped to the viewport; the box does not depend on the count.

### 14. Changes wanted from the product owner

By criterion number. 1 to 3, 6 to 12, 15, 17 to 20, 22 to 27, 29 to 32, 34, 36, 37,
39 to 42, 47, 48, 50 to 52 need no change (39: tooltip text only, section 11).

- **Question 3, criteria 14, 16, 43 to 46, "Changes to accepted behaviour" 4 and
  5, D6:** applied; the wording is the criteria's (43 steps 3 to 7, 44 removed,
  45 and 46 as written). The Ctrl copy badge follows criterion 43 step 4: selected
  objects and the centre handle only (section 5).
- **4:** replace "a lighter style that the `ux-engineer` defines" with: 1 px
  dashed 4 / 3 in `--member-box`, edges on a group box edge not drawn, not drawn
  under 6 px or outside the viewport. **5:** "More than 500 selected objects"
  (count of the selection); the measurement is no longer needed.
- **13:** the box is a 6 px marker square, not "no handle" only.
- **21:** the example hint text is replaced by section 10; add that the title of
  "Object to path" names what it buys (section 11).
- **28 and 49:** applied. No simplified preview is built (`adrs.md` decision 3);
  criterion 28 keeps the rule as dormant. 49 carries the `wait` cursor and the
  unchanged preview from release to presented commit.
- **33:** the angle chip has the visible label "Δ". **35:** the Absolute segment
  states its reference ("Top-left corner of the selection").
- **38:** replace "the same lines ... added" by the table of section 10.

### 15. Open design questions, with defaults

- **A. Member boxes outside the group box** (a turned path with a stale oriented
  box). Default: kept as they are. Alternative: member boxes use the tight outline
  bounds (loses the visible orientation of a turned object). Decide at the screenshot
  review.
- **B. The jump of the box on a rotate release.** Default: instant. Alternative:
  120 ms corner interpolation. Cheap to add later; no model change.
- **C. The 500 limit** is a count of the selection. If the review of a 600-object selection shows the missing member
  boxes matter, raise the limit before adding anything else.
- **D. The live region** of section 13 is "Should"; default: built with this
  feature, dropped if the key handler refactor makes it expensive.

## Links

Requirements: R-EDIT-012 (`docs/requirements.md`), extended; grouping itself is
R-EDIT-009 (slice 10)
Builds on: `specs/0005-object-transform/` (criterion 2 and the out-of-scope entry
"Multi-object transform" are superseded), `specs/0008-object-transform-refinements/`
(criteria 1, 9, 12 to 14, 19 to 23, 33 to 34, 37 to 53), `specs/0009-unified-object-editing/`
(criteria 7, 10 to 15a, 21 to 23, 35, 37), `specs/0010-edit-interaction-polish/`
(criteria 9 to 25, 26 to 38, 54 to 59), `specs/0014-advanced-selection/` (PR #61),
`specs/0007-stroke-and-fill-styling/` (criteria 27 to 29), `specs/0012-polygon-star-box-refit/`
PR:
