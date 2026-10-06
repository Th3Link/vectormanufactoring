# Object transform refinements: handles, pivots, numeric entry, 22.5° snap

Status: Ready
Priority: Must
Origin: Customer

## User value

As a maker I want the Select tool's transform handles to be easier to reach
and to behave predictably around a clear pivot, and I want to type an exact
angle or size instead of dragging, so that adjusting a part to a precise
orientation or dimension for the machine takes seconds and never a guess.

This is the customer's change list after testing `specs/0005-object-transform/`
(PR #29, otherwise accepted: "fast alles fehlerfrei"). The customer split it
off on purpose: "ich wünsche mir änderungen, die machen wir aber separat.
erstmal möchte ich korrekteres verhalten durchwinken." Nothing here changes
slice 5's accepted behaviour unless a criterion says so.

**Slice 5's final behaviour, which this spec builds on:** one selected
object shows an oriented selection box (it rotates with the object) with 8
resize handles (4 corners, 4 edge midpoints; polygon and star: corners only)
and one rotate handle a fixed screen distance above the top edge; pressing
anywhere inside a selected object's box moves it; scale and rotate modifiers
are Ctrl (proportional resize, 15° rotation steps) and Shift (resize about
the center; rotate about the bottom edge midpoint instead of the center). The
live size and angle readouts and the pivot marker already exist. Sizes,
hit radii and the rotate offset live in `docs/design-system.md`; this spec
names them by role and does not restate numbers.

**Field reference.** LightBurn puts a square move handle at the selection's
center, four curved-arrow rotate handles at the corners, and separate shear
handles; its rotation pivot is the center. Inkscape swaps handle sets on a
second click, has a movable rotation center, and an axis-parallel box. Where
we do better: all handle kinds are visible at once with no mode switch, the
pivot is always shown on screen, and typed entry uses the same reference
point as the drag, so a typed value and a dragged value never disagree.

## Parts and dependencies

Items 1–5 and 7 (criteria below) are ready to refine and build together.
**Item 8 (skew handles, "Part B") is no longer blocked** (customer decision
2026-10-06): paths only, baked into the geometry, so there is no
document-model change and no ADR. It is a separate part; the architect adds
a short dated note to `adrs.md` (which fields a skew writes), and it can be
built after or alongside Part A. Items 1–5 and 7 do not wait for it. Item 6
is out of scope; item 9 is a recorded decision.

## Changes to shipped behaviour and compatibility

Decided by the `product-owner` on the architect's flags (2026-10-06). Items
1 and 2 are **customer-visible changes to behaviour accepted in slices 4 and
5**; the lead mentions them at demo. The rest is compatibility bookkeeping.

1. **Double-click inside the selected object (customer-visible).** Today a
   double-click inside an unfilled object does nothing, because only the
   outline is hit-tested (slice 4). After this spec a double-click anywhere
   inside the box of the sole selected object, the center handle included,
   switches to that object's own tool, like a double-click on its outline.
   Reason: a press inside the box already moves the object (slice 5), so a
   double-click there doing nothing would be the odd one out, and the center
   handle needs a defined double-click. Handle double-clicks are not
   affected (criteria 23, 32, 49). Criterion 3.
2. **3 px dead zone (customer-visible).** A Select-tool drag of the selected
   object (move, resize, rotate, skew) now starts only once the pointer has
   left a 3 screen-pixel radius around the press; a press, a click or a
   wobble under 3 px writes nothing. Past 3 px the object follows the pointer
   1:1 from the original press point, so nothing jumps. Before, a 1 px
   wobble produced a tiny move or resize. Needed so a double-click with a
   slightly unsteady hand does not edit the object before the entry opens
   (criterion 18). Criteria 3, 18, 41.
3. **Typed size entry for polygon and star** scales about the shape's
   center, like their drag does in slice 5 (criterion 28).
4. **Ctrl skew** caps at ±75° (criterion 47).
5. **Units.** There is no display unit setting yet; the readouts and the
   entry fields use millimetres. When `document-size-and-rulers` adds a
   display unit, readout and entry change together. Criterion 25's "display
   unit" reads as "mm" until then.
6. **Slice 5 tests that must be rewritten (not deleted).** The old rotate
   handle above the top edge no longer exists; that spot now holds the
   Shift-only top side rotate handle. Tests that press it
   (`vecmanf-ui-core/tests/acceptance_0005.rs`,
   `vecmanf-editor-wasm/tests/acceptance_0005*.rs`, `acceptance_0004.rs`,
   `select_tool.rs` unit tests) hold Shift or use a corner rotate handle
   instead. Tests that expect a drag under 3 px to write something are
   rewritten to move at least 3 px (item 2). Tests that expect a
   double-click inside an unfilled object to do nothing (item 1) are
   rewritten to expect the handoff.

## Part A: criteria (items 1–5, 7)

### Center move handle (item 1)

1. Given one selected object (slice 5 criterion 1), when it is shown with
   transform handles, then a move handle is drawn at the center of its
   selection box, in addition to the resize and rotate handles. It is not
   shown for a multi-object selection (slice 5 criterion 2).
2. Given a press on the center handle and a drag, then the object moves 1:1
   with the pointer exactly as dragging its body does
   (`canvas-navigation-and-selection` criterion 20, slice 5 criterion 23):
   a pure translation, rotation and size unchanged, one commit on release.
3. Given a press inside the selection box that is not on any handle, when
   the maker drags, then the object still moves exactly as before. The
   center handle adds a visible target; it takes nothing away. A press and
   release on the center handle with less than 3 screen pixels of movement
   writes nothing (slice 5 criterion 3, with the dead zone described under
   "Changes to shipped behaviour"; the same dead zone applies to a body
   drag). Given a double-click on the center handle, or anywhere else inside
   the box of the sole selected object that is not on another handle, then
   the tool switches to the object's own tool
   (`canvas-navigation-and-selection` criterion 23: Rectangle, Ellipse or
   Polygon/Star for a primitive, the Node tool for a path). This is a
   change: before, a double-click inside an unfilled object did nothing.
4. Given a selection box whose shorter side is smaller than 48 screen
   pixels, then the center handle is not drawn and has no hover or cursor
   state, so it never covers the resize handles. It owns no hit area of its
   own (a press on it is a body press, criterion 3), so pressing inside the
   box still moves the object.

### Rotate handles at corners and sides (item 2)

5. Given one selected object, then four rotate handles are always shown,
   one outside each corner of the selection box, on the diagonal through the
   box center, diagonally outside the corner's resize handle at the rotate
   offset in `docs/design-system.md`. They replace the single rotate handle
   above the top edge (Proposal; see open question 2).
6. Given the Shift key is held while no handle drag is in progress, then
   four more rotate handles are shown, one outside the midpoint of each
   side, on the axis through the box center; they disappear when Shift is
   released. Pressing Shift during a drag does not reveal them. A side
   handle that is being dragged stays visible until the drag ends, even
   if Shift is released.
7. Given a polygon or star (corner resize handles only, slice 5 criterion
   11), then all eight rotate handle positions are available exactly as for
   other objects; the missing edge resize handles do not remove side rotate
   handles.
8. Given the box is rotated by any angle, then all eight rotate handles
   follow the box's own corners and sides (the oriented box, slice 5
   criterion 18), not the screen axes.
9. Given any pointer position, then press, hover state and cursor all use
   one nearest-centre test over the visible handles: the handle whose centre
   is nearest the pointer and within its own hit radius wins. Radii: resize
   min(16, s/3) with s the shorter box side in screen pixels (slice 5's
   shrink rule, at least 4), rotate 16, skew 12. On an exact tie the order is
   resize, then skew, then rotate. The center handle is not part of this
   test (criterion 4; criterion 3's body move is the fallback inside the
   box). An edge resize handle on a box whose shorter side is under 24 px is
   not drawn but stays hit-testable (slice 5's rule: hit-testing is
   unchanged; the glyphs would merge). Layout (`ux-engineer`, UX notes): a
   corner rotate handle sits 32 px
   outward on the diagonal from its corner resize handle, so resize and
   rotate hit areas never overlap at any box size; every handle glyph keeps
   a clear gap of at least 4 px to every other glyph. Skew hit areas
   deliberately overlap their two neighbours on the same side (criterion 48);
   the nearest-centre test splits the overlap.
10. Given the pointer is over a rotate handle, then the cursor is the
    existing non-rotating rotate cursor (slice 5 UX notes); a press and
    release without movement writes nothing (slice 5 criterion 3).
11. Given a Shift-press on a visible side rotate handle, then a rotate drag
    starts; it does not perform Shift-click selection
    (`advanced-selection`).

### Pivot rule (item 3)

The customer: "bei shift dreht es verändert sich der drehpunkt immer auf den
unteren. mit punkt 2. und den 4/8 drehhandels nehmen wir immer die
gegenüberliegende ecke/seite als drehpunkt." Slice 5 has Shift meaning
"pivot on the opposite point"; with only a top handle that was always the
bottom edge. **Proposal, to be confirmed (open question 1):** keep the
meaning of Shift and generalize it to the new handles.

| Grabbed handle | No modifier | Shift held |
|---|---|---|
| Corner rotate handle | box center | the opposite corner |
| Side rotate handle (only visible while Shift is held) | box center, if Shift is released during the drag | the opposite side's midpoint |

Shift therefore keeps one meaning on rotate handles (opposite point as
pivot), and it is also what reveals the side handles, so a side handle always
starts with the opposite side as its pivot. Resize keeps slice 5's rule
(Shift = about the center; Ctrl = proportional) unchanged.

12. Given a rotate drag on a corner handle with no modifier, then the
    object rotates about its box center, as today.
13. Given a rotate drag on a corner handle with Shift held, then the pivot is
    the corner diagonally opposite the grabbed one. Given a rotate drag on a
    side handle with Shift held, then the pivot is the midpoint of the
    opposite side. "Opposite" is taken in the object's own (rotated) frame.
14. Given Shift is pressed or released during a rotate drag, then the pivot
    switches immediately, the pivot marker moves to it in the same frame
    (slice 5 UX notes), and the result is always computed from the state at
    the start of the drag, so no error accumulates.
15. Given a rotate drag with Ctrl held, then the angle snaps (criteria
    33–36) about whichever pivot criteria 12–14 select.
16. Given the same final angle and pivot, then a drag and a numeric entry
    (criteria 18–24) leave the object in the same state, so the two ways of
    rotating never differ.
17. Given a polygon, star, rectangle, ellipse or path, then the pivot rule is
    identical for all of them, and the writes per kind are the ones in
    `specs/0005-object-transform/adrs.md` ("merge granularity"): a pivot
    other than the center also moves the frame (primitive) or anchors
    (path).

### Numeric angle entry (item 4)

The customer: "wenn man doppelt auf das handle klickt soll man die gradanzahl
direkt eingeben können." And: "ebenso dann auch shift+doppelklick" — the
Shift pivot rule applies to typed entry too.

18. Given one selected object, when the maker double-clicks a rotate handle
    (two presses on the same handle within the system double-click interval,
    each press-release moving the pointer less than 3 screen pixels, the
    dead zone of "Changes to shipped behaviour"), then a text field opens
    next to the handle, pre-filled with the object's current rotation in
    degrees (the value the live readout shows, slice 5 criterion 22, same
    sign convention) and with its text selected. The first press and
    release writes nothing (slice 5 criterion 3). A side handle can only be
    double-clicked while Shift is held, because it exists only then.
19. Given the field is open and the maker types a number and presses Enter,
    then the object's rotation becomes that value (the angle typed is the
    object's absolute rotation, so typing 0 restores an unrotated object),
    in one commit. The field accepts a decimal point or a decimal comma, an
    optional trailing "°", and any finite value, which is stored normalized
    as slice 5's `rotation` register already is. A value equal to the
    current rotation writes nothing. Given the field's text was never edited
    since it opened, then Enter closes the field and writes nothing (the
    prefill is the readout's rounded value, so Enter on an untouched "37.4"
    must not overwrite a rotation of 37.428).
20. Given the field is open, when the maker presses Escape, clicks anywhere
    else, switches tool, changes the selection or the window loses focus,
    then the field closes and nothing is written (cancel on losing focus
    confirmed by `ux-engineer`). The press that closes the field by blur is
    not swallowed: it also does what it would have done without the field
    (selecting another object, toggling "Scale stroke width").
21. Given the field contains text that is not a finite number (empty,
    letters, "1,2,3"), when the maker presses Enter, then the field stays
    open, is marked invalid, and nothing is written.
22. Given the second press of the double-click happens with Shift held on a
    corner or side handle, then the entry rotates about the opposite
    corner or side midpoint (criterion 13); otherwise about the box center
    (criterion 12). The pivot is fixed when the field opens and shown with
    the pivot marker while the field is open; Shift state afterwards is
    ignored.
23. Given a rotate handle is double-clicked, then the double-click does not
    trigger the object-to-own-tool handoff of
    `canvas-navigation-and-selection` criterion 23.
24. Given the rotation was set by numeric entry, when the maker saves, closes
    and reopens the project, then the rotation is what was typed (slice 5
    criterion 24), and the object is the same kind as before.

### Numeric size entry (item 5)

The customer: "ebenso beim doppelklick auf die größer zieh handles, die
referenz / fester punkt bleibt identisch zum händischen ziehen."

25. Given one selected rectangle, ellipse or path, when the maker
    double-clicks a corner resize handle, then a field group opens next to
    it with fields labelled "W" and "H" (visible labels; the accessible
    names are "Width" and "Height"), pre-filled with the object's current
    size along its own axes (the values the live readout shows, slice 5
    criterion 14) in millimetres (there is no display-unit setting yet; the
    readout and the fields change together when one exists), the Width field focused
    and selected, Tab moving to Height. Given an edge resize handle, then
    only the one dimension that handle changes is shown (Width for a
    left or right handle, Height for a top or bottom one).
26. Given one selected polygon or star, when the maker double-clicks a
    corner resize handle, then a single field opens, visibly labelled "r"
    (accessible name "Radius"), pre-filled
    with the outer radius the readout shows; entering a value scales the
    shape uniformly as slice 5 criterion 11 defines.
27. Given a size entry is committed with Enter, then the document ends in
    the same state as a hand-drag with the same handle and the same
    modifiers would have left it at that size: same fixed point, same
    per-kind rules (slice 5 criteria 9–12, polygon/star uniform), and the
    "Scale stroke width" switch (slice 5 criteria 8 and 26) honored with the
    value it had when the entry opened. One commit.
28. Given the entry opens, then the fixed reference is the one the hand-drag
    would use: for a corner handle of a rectangle, ellipse or path the
    opposite corner, for an edge handle the opposite edge; if Shift is held
    at the second press of the double-click, the box center. For a polygon
    or star it is always the shape's center, with or without Shift, because
    slice 5 scales them about their center (criterion 26). It is fixed when
    the entry opens and shown with the pivot marker while it is open.
29. Given the second press of a corner-handle double-click happens with Ctrl
    held (rectangle, ellipse, path only), then Width and Height are linked:
    changing one updates the other to keep the aspect ratio the box had when
    the entry opened, matching Ctrl's proportional drag (slice 5 criterion
    5). Without Ctrl they are independent.
30. Given a field contains a value that is not a finite number, or a size of
    zero or less, when the maker presses Enter, then the group stays open,
    marks the offending field invalid, and nothing is written. (A drag may
    clamp to 0, slice 5 criterion 13; typed entry refuses it, since a typed
    zero is a mistake, not a gesture.)
31. Given an entry equal to the current size, or fields whose text was never
    edited since the entry opened (Enter on untouched prefill, as criterion
    19), or Escape, or a click elsewhere, a tool switch or selection change,
    then nothing is written (as criterion 20, including that the press that
    closes the entry by blur is not swallowed). Clicking the "Scale stroke
    width" switch while the entry is open closes it without writing.
32. Given a resize handle is double-clicked, then the double-click does not
    trigger the object-to-own-tool handoff (criterion 23's rule, here for
    resize handles).

### 22.5° snap stop (item 7)

The customer: "aus pragmatischen gründen würde ich mit 22,5 noch als grad
stop bei strg wünschen." "noch" reads as "additionally". **Resolved 2026-10-06:** the customer
confirmed: "22,5 zusätzlich zu den 15 grad, also auch 67,5 quasi immer die
45/halbe konsequent durch die 360grad durch." The stop set is the union of
multiples of 15° and of 22.5°, repeated consistently through all four
quadrants, nearest stop wins. (They wrote "77,5", evidently a typo for
67.5.)

33. Given a rotate drag with Ctrl held, then the rotation snaps to the
    nearest stop of {k × 15°} ∪ {k × 22.5°}, measured from the object's
    angle at drag start (slice 5 criterion 17), positive and negative.
    Within one 45° period the stops are 0°, 15°, 22.5°, 30°, 45°; so in a
    half turn: 0, 15, 22.5, 30, 45, 60, 67.5, 75, 90, 105, 112.5, 120, 135,
    150, 157.5, 165, 180. The stops repeat through all four quadrants and the
    full 360°: the second half turn continues 195, 202.5, 210, 225, 240,
    247.5, 255, 270, 285, 292.5, 300, 315, 330, 337.5, 345, 360 (and the same
    set, mirrored, for negative rotation). Every quadrant has the same stops
    relative to its axis, so 67.5° and 337.5° exist exactly like 22.5°.
    Between any two adjacent stops the nearest one wins.
34. Given a raw rotation of 10°, 19°, 18.5°, 26°, 26.5°, 40°, 55°, 64°,
    71° and 100° from the start angle, then Ctrl snaps to 15°, 22.5°, 15°,
    22.5°, 30°, 45°, 60°, 67.5°, 67.5° and 105°. At an exact midpoint
    between two stops (18.75°), the stop nearer the start angle wins.
35. Given a Ctrl snap lands on a stop, then the live readout (slice 5
    criterion 22) shows up to one decimal place ("22.5°", "45°"), where it
    previously showed whole degrees under Ctrl.
36. Given Ctrl is not held, then rotation is unsnapped as before; Ctrl has
    no effect on numeric entry (criteria 19, 31).

## Part B: Skew handles (item 8, R-EDIT-015)

The customer: "ich hätte gerne noch wie bei inkscape und lightburn gemischt
extra handles für 'ins trapez ziehen mit pfeilen. in x und y richtung'", and
on the meaning: "ja genau ich meine schere wie bei inkscape" (skew, the
parallelogram; not a taper).

**Field reference.** Inkscape: a second click swaps the handles to rotate and
skew handles; the arrows on the edge midpoints shear along x or y.
LightBurn: separate shear squares next to the corners; its Warp/Deform tool
(a real quadrilateral warp) is a different feature. Neither has an on-canvas
taper handle. Where we do better: skew handles are visible together with the
resize and rotate handles with no mode switch, the fixed edge follows the
same Shift rule as resize, and the box always shows the result.

### Decision (customer, 2026-10-06)

The customer chose primitive option **P1**: "nein wir bleiben erstmal bei 1
(weniger code). es ist ok, wenn man es versucht und nichts passiert. und
verschieben 3 bis wir die primitive verfeinern."

- **Paths (open and closed):** skew is baked into the anchors and handle
  vectors, as move and rotate already are
  (`specs/0005-object-transform/adrs.md`). No document-model change, no
  `format_version` bump, no ADR. The architect only adds a dated note to
  `adrs.md` (a skew writes anchors only, never `rotation`; see criteria
  44–46).
- **Primitives (rectangle, ellipse, polygon, star):** skew is not supported.
  Trying it does nothing. No conversion to a path, no message. The maker who
  needs a skewed primitive uses "Object to path" first
  (`primitive-shapes` criterion 21: no implicit conversion stays intact).
- **Recorded future option, not planned:** P3, a stored per-object matrix so
  primitives stay primitives under skew. It is a document-model change
  (new stored field, version bump, every outline reader composing it) and
  is deferred until the customer's primitives rework ("ich möchte an die
  primitive eh nochmal ran"). P2 (auto-convert to a path on first use) is
  rejected for now.
- Taper (true trapezoid, non-affine) is dropped; it becomes its own
  `Proposal` spec only if the customer later asks for it (for example
  tapered cups on a rotary).

### Skew handles on a path (item 8)

Terms. "Local axes" are the axes of the selection box, which follows the
path's `rotation` register (slice 5 criterion 18): `u` along the box's top
edge, `v` along its left edge. A skew handle on the top or bottom side skews
in the `u` direction (x skew); one on the left or right side skews in the `v`
direction (y skew). The *fixed edge* is the side opposite the grabbed one.

37. Given exactly one selected path, open or closed, with a selection box
    whose sides all have non-zero length, then four skew handles are shown,
    one per side of the oriented box: top and bottom for x skew, left and
    right for y skew. Each has an arrow glyph pointing along its side. They
    are shown together with the resize and rotate handles, with no mode
    switch. For a rotated path they follow the oriented box (criterion 8's
    rule), not the screen axes. Given the box has zero height (top and
    bottom coincide, for example a straight line along `u`), then the top
    and bottom handles are not shown, since an x skew has no distance to
    scale by; likewise zero width removes the left and right handles. "Zero"
    means below the geometric tolerance, not a pixel size. Separately, a
    screen-size rule applies on top: the top and bottom handles are hidden
    when the box is less than 24 screen pixels high, the left and right
    handles when it is less than 24 screen pixels wide (a 2 px twitch on a
    shorter lever changes the skew angle by 5° or more).
38. Given a press on a skew handle and a drag, then the path is sheared
    along the grabbed side's direction: the fixed edge's line does not move,
    and every point of the path moves along `u` (or `v`) by an amount
    proportional to its distance from that line, with the grabbed edge's
    line moving 1:1 with the pointer's displacement projected onto the
    side's direction. The skew angle is `atan(d / h)`, where `d` is that
    projected displacement and `h` is the distance from the fixed line to
    the grabbed side at drag start. The angle therefore stays strictly
    between −90° and +90° and the shape never degenerates, however far the
    pointer goes. Straight segments stay straight; every Bézier stays a
    Bézier.
39. Given Shift is held, then the line through the box center, parallel to
    the grabbed side, stays fixed instead of the opposite side: the grabbed
    side moves 1:1 with the pointer, the opposite side moves the opposite
    way by the same distance (`h` is then half the side distance). This is
    resize's Shift rule (slice 5 criterion 8) applied to skew. Given Shift
    is pressed or released mid-drag, then the result switches immediately
    and is always computed from the state at drag start, so no error
    accumulates (as criterion 14).
40. Given a skew drag in progress, then the drawn preview and the committed
    result are produced by the same function from the same inputs, so
    releasing the pointer never changes what was shown. A live readout shows
    the skew angle in degrees, to one decimal place, with sign (positive in
    the direction of the pointer's displacement along `u` or `v`;
    placement is the `ux-engineer`'s call, as slice 5 criterion 14). The
    pivot marker shows the fixed line's reference point (Proposal: the
    midpoint of the fixed edge, or the box center with Shift).
41. Given a skew drag, when the maker releases the pointer, then exactly one
    commit is made: one undo step restores the geometry before the drag,
    one redo re-applies it. Given the maker presses Escape during the drag,
    then the path is exactly as before the press and nothing is written.
    Given a press and release on a skew handle with less than 3 screen
    pixels of movement (the dead zone), or a drag that returns to its start
    point, then nothing is written (slice 5 criterion 3).
42. Given a skewed path, then every anchor point and every handle vector of
    the committed path is the linear skew applied about the fixed line, and
    nothing else about the path changes: node kinds, node count and order,
    open or closed state, and the stroke width are unchanged (a skew has no
    single scale factor, so the "Scale stroke width" switch does not apply
    to it, on or off). Bézier handle vectors are transformed by the linear
    part only, not translated, since they are stored relative to their anchor.
43. Given a skew by angle α followed by a skew by −α with the same handle
    and the same Shift state, then the path's anchors and handle vectors
    return to their original values within the geometric tolerance. (The
    fixed line and the grabbed side's distance to it are unchanged by the
    first skew, so the inverse is exact up to rounding.)

**Interaction with `rotation` and the box.** Proposal, flagged because it
is not obvious: the skew is applied in the box's local frame (`u`, `v` as
above), and the `rotation` register is **not changed** by a skew.

44. Given a path with `rotation` θ (any angle, including 0), when it is
    skewed, then the shear acts along the local axes at θ, not the screen
    axes, and the path's `rotation` is still θ afterwards, in particular the
    rotation readout is unchanged and criterion 24's save/reopen
    round-trip still yields the same θ.
45. Given a skewed path, then its selection box is recomputed as the tight
    oriented rectangle around the new geometry in the same θ frame (the way
    slice 5 criterion 18 and its `adrs.md` define it for any path: rotate the
    anchors by −θ, take the extremes, attach θ). The box is again a
    rectangle with perpendicular sides; the skew handles, resize handles
    and rotate handles are placed on that box. A second skew, a resize or a
    rotation after a skew therefore works on the baked geometry exactly as
    for any other path, and the box edges are *not* tilted with the skewed
    shape. This matches the oriented-box decision (item 9).
46. Given a skew is committed, then the document writes only the path's
    anchors and handle vectors (slice 5's "merge granularity": a skew writes
    the same fields as a resize). It writes no `rotation`, creates no new
    stored field and does not change `format_version`; a document saved
    after a skew is an ordinary path document.
47. Given a Ctrl-held skew drag, then the skew angle snaps to the nearest stop
    of the set in criteria 33–34 (multiples of 15° and of 22.5°), measured
    from 0° (the unskewed state at drag start), positive and negative, and
    the readout shows it. The stops ±90° would be a degenerate shear
    (criterion 38 requires an angle strictly between −90° and +90°), so a
    Ctrl skew is capped at ±75°, the last stop below 90°: a raw angle of
    80° or 89° snaps to 75°, and −80° to −75°. (Proposal: reuses the
    existing snap function; drop this criterion if the customer wants even
    less code.)
48. Given the pointer is over a skew handle, then the cursor is a skew
    cursor appropriate to its axis (`ux-engineer`). Skew handles take part
    in criterion 9's single nearest-centre test (hit radius 12): the handle
    whose centre is nearest wins, on an exact tie resize beats skew and skew
    beats rotate. A skew handle sits at its side's midpoint, 16 px outward,
    between the edge resize handle and the Shift-only side rotate handle, so
    its hit area necessarily overlaps those two neighbours; the
    nearest-centre test splits the overlap. Its glyph keeps a clear gap of
    at least 4 px to every other glyph. (The LightBurn layout of a small
    arrow next to a corner was dropped: corners hold resize and rotate.)
49. Given a double-click on a skew handle, then nothing opens, nothing is
    written, and the object-to-own-tool handoff
    (`canvas-navigation-and-selection` criterion 23) is not triggered
    (criteria 23 and 32's rule, here for skew handles). Typed skew entry
    does not exist (see "Out of scope").

### Primitives and other selections under skew

50. Given exactly one selected rectangle, ellipse, polygon or star, then no
    skew handles are shown (UX proposal for the `ux-engineer`: not drawing a
    handle that does nothing is the clearer affordance; if the
    `ux-engineer` finds showing them inert cheaper or clearer, that is
    acceptable as long as criterion 51 holds). All other handles behave as
    in Part A and slice 5.
51. Given a rectangle, ellipse, polygon or star, when the maker tries to
    skew it by any means this spec provides, then nothing happens: the
    document is unchanged, no commit is made, no undo step is added, no
    object is converted to a path, and no error or message is shown. In
    particular a press that would have started a skew does not move, resize
    or rotate the primitive either.
52. Given a primitive that was converted with "Object to path", then its
    path shows skew handles and skews as in criteria 37–49, with the
    `rotation` it kept (criteria 44–45).
53. Given two or more selected objects, then no skew handles are shown
    (slice 5 criterion 2: no transform handles on a multi-object selection),
    whatever their kinds; a mixed selection of paths and primitives
    therefore never skews anything today. Forward rule, recorded here so a
    later multi-object transform story does not have to decide it again:
    if multi-object transform handles are ever added, a selection that
    contains any primitive does not skew at all (no partial skew of the
    paths in it), consistent with this decision.

**Trade-off, accepted.** On a path the edge-midpoint resize handle is 14 px
deep (8 px outward, 6 px inward) instead of slice 5's 22 px, because the skew
arrow sits next to it on the same side and nearest-centre splits the overlap
(criteria 9, 48). Corner resize handles are unaffected and keep their full
reach. Primitives (no skew handles) and polygon/star keep slice 5's depth.

**Not in this part.** Everything under "Out of scope" below.

## Optional criteria (Should, Proposal from `ux-engineer`)

The template has one priority per spec, so these are separated here. They are
kept by default and can each be cut without touching any criterion above;
the customer decides. They are `Proposal`, not customer requirements.

54. (Should, Proposal) Given the pointer rests on a resize, rotate, skew or
    center handle for 600 ms, then a text-only hint chip appears 12 px up and
    right of the pointer, does not follow the pointer, takes no pointer
    events, and disappears on press, on leaving the handle or on any key. It
    names the handle and its modifiers, one short line each: resize
    ("Shift: from center", "Ctrl: keep proportions" on corners of
    rectangle, ellipse and path only, "Double-click: type a size"); rotate
    ("Shift: pivot at opposite corner" or "opposite side", "Ctrl: snap",
    "Double-click: type an angle"); skew ("Shift: from the center line",
    "Ctrl: snap"); center ("Move"). A skew handle's hint has no double-click
    line (criterion 49).
55. (Should, Proposal) Given Shift is held, no drag runs and the pointer is
    over a resize, rotate or skew handle, then the pivot marker shows the
    point that handle would use if pressed (opposite corner or side midpoint
    for rotate; box center for resize and skew), so the pivot rule is visible
    before the press. Releasing Shift or leaving the handle removes the
    preview.
56. (Should, Proposal) Given a skew drag in progress, then a 1 px dashed
    guide is drawn along the line that stays fixed (the fixed edge, or the
    line through the box center with Shift), extended 16 px past each end of
    the box; it disappears on release or Escape.

## Accepted decisions (not criteria)

- **Item 9, the oriented selection box stays.** The customer: "ich finde das
  eigentlich cool, dass durch die mitdrehung der bounding box die
  ursprüngsposition wieder herstellen lässt, auch recht intuitiv. inkscape
  hat die bounding box immer parallel zu den achsen. ich würde das kurz
  notieren, aber so lassen." The box keeps rotating with the object
  (slice 5 criterion 18), unlike Inkscape's axis-parallel box, because
  it lets the maker see and restore the original orientation. No criterion
  changes; every handle and pivot rule above is defined on the oriented
  box.

## Open questions for the customer

Resolved 2026-10-06 (removed from this list): snap stops (item 7, union of
15° and 22.5° multiples, see criteria 33–36); item 8 shear vs taper (shear
like Inkscape) and the primitive question (P1, see "Decision" in Part B);
corner-radius switch (deferred, see "Out of scope"). Decided by the
`product-owner` 2026-10-06 on the architect's flags: see "Changes to shipped
behaviour and compatibility".

Status is Ready on the stated defaults of the two questions below and on
keeping the optional criteria 54–56; the implementer builds against those
defaults. A different answer changes only the pivot table and criteria
12–15 (question 1), criteria 5–7 (question 2), or the single optional
criterion concerned.

1. **Pivot scheme (item 3).** (a) Default: no modifier = rotate about the
   center, Shift = about the opposite corner or side (slice 5's meaning of
   Shift, generalized; side handles appear with Shift, so they always start
   with the opposite pivot) — recommended; (b) always the opposite
   corner/side, and the center needs another modifier (Alt, which collides
   with window-move on many Linux window managers).
2. **The old top rotate handle (item 2).** (a) Default: replaced by the four
   corner handles; the top side handle exists only while Shift is held;
   (b) keep it always visible as a fifth handle.

## Out of scope

- **Moving the rotation pivot (item 6).** The customer considered it and
  does not want it now: "müsste man ja auch irgendwie zurücksetzen können".
  A movable pivot needs a stored per-object pivot and a way to reset it.
  Inkscape's movable rotation center is the precedent we are not following
  here. The pivot is only ever the box center or the opposite corner/side
  (criteria 12–13).
- Taper (trapezoid) handles, and any Warp/Deform of an object into a
  general quadrilateral.
- Skew of primitives (rectangle, ellipse, polygon, star), by conversion
  (P2, rejected) or by a stored per-object matrix (P3, deferred until the
  primitives rework). Skew of multi-object selections.
- Typed skew entry (a double-click on a skew handle does nothing, criterion
  49), a skew snap to anything but the criterion 47 stops, and skew about a
  point other than the fixed edge or the box center.
- A tilted selection box that follows a skewed shape: the box stays the
  rectangle of criterion 45.
- **A corner-radius scaling switch (decision 2026-10-06).** The customer
  assumed rounded corners were not implemented (they are: `0005` slice 3
  has a rectangle corner radius, but no UI setting for how it scales) and
  deferred the switch: "lass uns das später machen, ich möchte an die
  primitive eh nochmal ran". No switch now; the radius keeps scaling
  proportionally as in `0005` criterion 9, also for typed size entry
  (criterion 27). Revisit with the primitives rework.
- Typed entry of position (X/Y), of a percentage, of stroke width, or with
  unit suffixes ("12mm"); a Properties-panel transform form.
- Typed entry or handles for multi-object selections (slice 5 criterion 2).
- Keyboard-driven rotate or resize nudges.
- Snapping to a grid, objects or guides.
- A movable or visible rotation center, and any change to the oriented box.
- Ctrl-constrained or Shift-constrained *move* from the center handle
  (LightBurn locks movement to 45°/90° with Shift; not asked for here).

## UX notes

Status: complete (`ux-engineer`, 2026-10-06). Sizes and tokens live in
`docs/design-system.md` ("Transform handle layout", "Transform center
move handle", "Transform skew handle", "Transform entry chip" and their
neighbours); this section gives the decisions and the reasons. The
criteria were updated to match these notes by the `product-owner`
(2026-10-06).

Distances are screen pixels. "Outward" is away from the box center in the
box's own (rotated) frame. `s` is the shorter side of the oriented box on
screen.

### Handle layout on one box

| Handle | Position | Glyph | Hit radius | Drawn when |
|---|---|---|---|---|
| Corner resize | the corner | 8 px squircle (slice 5) | min(16, s/3), at least 4 (slice 5) | always |
| Edge resize | edge midpoint | 8 px squircle (slice 5) | same | `s` >= 24; not on polygon/star |
| Skew | side midpoint, 16 outward, glyph along the side | 18 x 12 | 12 | path only; per axis, see below |
| Corner rotate | corner, 32 outward on the diagonal (22.6 on each local axis) | 12 px arc arrow | 16 | always |
| Side rotate | side midpoint, 32 outward | 12 px arc arrow | 16 | only while Shift is held and no drag runs |
| Center move | box center | 16 px rounded square | hover only, min(12, s/4) | `s` >= 48, not during a resize/rotate/skew drag or an open entry |

Rules that make this work:

1. **Everything new sits outside the box.** Corner rotate, side rotate and
   skew are all outside the edge; the center handle is the only new glyph
   inside. A press inside the box that is not in slice 5's edge band
   (6/16 of the resize radius) is always a move. Slice 5's lesson (handle
   radii tiling a small box so it could not be grabbed) cannot recur
   from the new handles, because none of them has a hit area inside the
   box.
2. **One radial stack per side.** Along a side's outward normal the glyphs
   sit at 0 (edge resize), 16 (skew), 32 (side rotate). Clear space
   between glyph edges: 6, then 4 px. Corners: resize at 0, rotate at 32
   on the diagonal. The 32 px is the shipped rotate offset
   (the sum of the two 16 px radii), now used for corner and side alike.
3. **Shift adds, never moves.** The side rotate handles take the outermost
   ring, so revealing them displaces nothing. Skew arrows stay visible
   while Shift is held (a Shift-skew pivots on the center line).
4. **One hit test.** A press, the hover state and the cursor all use the
   same function. All visible handles compete by distance of the pointer
   to the handle center, each within its own cap (table above). Ties:
   resize, then skew, then rotate; the center handle is never hit-tested
   against them (below). This replaces slice 5's "rotate first, then
   resize" order.
5. **Size tiers.** `s` >= 48: everything. 24 <= `s` < 48: no center
   handle. `s` < 24: no edge resize (slice 5), only corner resize. Skew is
   decided per axis, not by `s`: top and bottom arrows need the box
   height >= 24, left and right arrows need the width >= 24 (the
   opposite side is the lever: a skew angle is `atan(d / h)`, so below
   that a 2 px twitch changes the angle by 5 degrees or more). Rotate
   handles are never hidden by size: they sit outside and cannot collide
   with anything inside the box, and a tiny part still has to be
   rotatable. Resize hit radii shrink exactly as slice 5 defines.
6. **Clearance check, any size.** Corner rotate to side rotate: at least
   12 px between glyph edges. Skew to corner resize: at least 6 px. Side
   rotate to skew: 4 px. Center to any resize handle: at least 12 px at
   `s` = 48 (that is why the threshold is 48, not 40: the hover region
   `s/4` is then still 12). Resize and rotate hit regions are disjoint at
   every size (radius at most 16 each, 32 apart). Skew and resize, and
   skew and side rotate, overlap by design (skew sits between them);
   nearest-center splits the overlap: the resize handle keeps 8 px outward
   of the edge, the skew arrow 8 to 24 px (to 28 without Shift), the side rotate
   handle from 24 px outward. Trade-off, accepted: on a path the edge-midpoint resize
   handle is 14 px deep (8 out, 6 in) instead of slice 5's 22. Corner
   resize handles keep their full reach.
7. **Polygon and star:** no edge resize, no skew; the four corner rotate
   handles and (under Shift) four side rotate handles exist as for any
   other kind (criterion 7). The side rotate handle then stands alone on
   its side.
8. **Off-screen handles** are not pulled into the viewport. At high zoom
   a large box has its handles off screen; the center handle is the one
   that stays reachable. A known gap, not new, not solved here.

### Glyphs

- **Center move handle:** 16 x 16 rounded square (3 px radius), white
  fill, 1 px `--accent` outline, a four-way arrow 10 px wide inside
  (1.5 px stroke, `--accent`). It does not rotate with the box (a move is
  along the screen axes). Square matches the resize family; the larger
  size and the arrow tell the two apart. Not drawn at the box center while
  another drag is running, because the pivot marker lives there.
- **Corner and side rotate:** the shipped 12 px circular-arrow icon,
  unchanged, never rotated (the arc is near-symmetric; rotating it adds
  nothing). All eight identical; position is the only difference.
- **Skew:** 18 x 12 footprint, two opposed parallel arrows (1.5 px stroke,
  3 px heads) pointing along the side: the "shear" picture, not a single
  double arrow, so it never reads as a second resize handle. Idle: stroke
  `--accent`, transparent ground. Hover: `--accent-hover` rounded-rect
  ground. Dragging: solid `--accent` ground, white arrows. It rotates with
  the box (top and bottom along `u`, left and right along `v`).
- **States:** rotate and skew follow the shipped rotate states; the
  center handle has white fill idle, `--accent-hover` over it on hover,
  solid `--accent` with a white glyph while its own move drag runs. While
  a numeric entry is open, the handle it belongs to stays in its
  dragging look, so the maker sees what the chip refers to.
- A handle exactly at the active pivot stays hidden (slice 5 rule); the
  pivot marker replaces it.

### Cursors

- Center handle: the built-in `move` cursor. Elsewhere inside the box the
  cursor stays the Select tool's normal one (unchanged).
- Rotate (all eight): the shipped non-rotating circular arrow.
- Resize: shipped rotated double arrow.
- Skew: a custom cursor, two opposed parallel arrows, rotated live like
  the resize cursor: the angle is the box rotation for top/bottom (x skew)
  and the box rotation + 90 degrees for left/right (y skew). Hint string
  `skew:<deg>`, fallback `ew-resize` or `ns-resize` by nearest angle.
- The cursor does not change with Shift or Ctrl.

### Shift reveal

The side rotate handles appear in the frame Shift goes down and vanish in
the frame it goes up. No fade (minimal animation), no distinct styling:
the same glyph at the outermost ring is itself the signal, and "Shift adds,
never moves" means nothing jumps. Reveal state is cleared when the window
or the canvas loses focus, so a Shift released outside the app cannot
leave them stuck. Not revealed by pressing Shift during a drag
(criterion 6). While Shift is held and the pointer is over a resize,
rotate or skew handle, the pivot marker previews the point that handle
would use (opposite corner or side midpoint for rotate, center for
resize and skew), so the pivot rule is visible before the press, not only
during the drag. This reuses the existing marker; nothing new is drawn.

Discoverability: a hover hint chip (below) names Shift and the double-click
entry. Side handles being Shift-only is a customer decision; the hint is
what keeps it findable.

### Hover hint chip

DOM, text only, same surface as the live readout chip (`--toolbar-bg`,
`--toolbar-icon`, 12 px). Appears after 600 ms of the pointer resting on a
handle, anchored once at 12 px up and right of the pointer (readout
placement and flipping), does not follow the pointer, disappears on
press, leave or any key. `pointer-events: none`. One short line per
modifier:

- Resize: "Resize" / "Shift: from center" / "Ctrl: keep proportions"
  (corners only) / "Double-click: type a size"
- Rotate corner: "Rotate" / "Shift: pivot at opposite corner" / "Ctrl:
  snap" / "Double-click: type an angle". Side: "opposite side".
- Skew: "Skew" / "Shift: from the center line" / "Ctrl: snap"
- Center: "Move"

Polygon and star resize drop the Ctrl line (always proportional).

### Center handle and "press inside the box moves"

The center handle is a visible name for a gesture that already works
everywhere inside the box; it does not own any behavior. A press on it is
a body press: same move drag, same click-writes-nothing rule, same
double-click handoff (criterion 3). It is not an exclusive hit target, so
it needs no priority rule against the others: it simply is not
hit-tested against them, and inside the box any press that is not on a
resize handle's edge band moves. It supplies hover feedback and the
`move` cursor. Below 48 px it is gone and the body still moves.

### Numeric entry control (criteria 18-32)

**Where:** an inline chip at the handle that was double-clicked, not next
to the readout. The pointer is on the handle at that moment, so the chip
is where the eyes already are; the readout chip is pointer-anchored and
exists only during drags. It is a DOM overlay (`docs/design-system.md`
allows DOM for text entry), above the Select tool's top bar.

**Placement:** upright, never rotated with the object (a rotated input is
not legible). Chip center goes outward from the handle, along the line
from the box center through the handle, with 10 px clear space between
the handle's glyph edge and the chip; then clamped inside the canvas with
the readout's flip and clamp rules. If the clamp would cover the handle,
it flips to the inner side. Because it lies outward, it never covers the
pivot marker (pivot is the center or the opposite point). It follows the
handle during wheel zoom and pan and on window resize; while the handle is
off screen the chip stays clamped at the canvas edge.

**Angle chip:** one field, 80 px wide, 28 px high, the `°` as a fixed
suffix inside the right edge (non-editable, `--toolbar-icon` at 70%).
Text `text-sm` (14 px), tabular numerals, right-aligned. Pre-filled with
the readout's value (one decimal, same sign), all text selected.

**Size chip:** a group on a `--toolbar-bg` card, 6 px padding, 8 px radius,
`--panel-elevation-shadow`. Fields 100 px wide (revised after the UI review: 84 px let the "mm" suffix touch the last digit and clipped values of 1000 mm and more), 28 px high, 4 px apart:
"W" and "H" (visible 12 px labels inside the left edge; `aria-label`
"Width" and "Height", measured along the object's own axes), the unit
("mm", the document's display unit) as a fixed suffix. Edge handles show
only the one field. Polygon and star: one field labelled "r" with
`aria-label` "Radius". When linked (Ctrl at the second press, criterion
29) a 12 px chain glyph sits between the fields and typing in one
updates the other live in the field; it is an indicator, not a toggle.
Width is focused and selected on open; Tab goes Width, Height, back to
Width (focus stays in the chip); Shift+Tab reverses.

**Field style:** white ground, `--toolbar-icon` text (11:1), the open
field has a 2 px `--editor-accent` border (this is also the focus
indicator, 3:1 or better against both ground and card), the other a 1 px
`--toolbar-icon` at 60% border. `inputmode="decimal"`, type text (not
number: spin buttons and decimal commas behave differently per engine),
`autocomplete="off"`, `spellcheck="false"`. Accepts "." or ",", an
optional trailing `°` on the angle; no unit suffix on sizes (out of scope).
Prefill uses ".".

**Keys:**

| Key | Effect |
|---|---|
| Enter | Validate. Valid: one commit, chip closes, focus returns to the canvas, selection and handles unchanged. Invalid: see below. Text never edited since opening: closes, writes nothing |
| Escape | Cancel, writes nothing, focus returns to the canvas |
| Tab, Shift+Tab | Move between the fields of the group, wrapping; single field: stays |
| Up, Down | Move the caret as in any field. No stepping (nudges are out of scope) |

Key events inside the chip never reach the canvas shortcuts (Space-pan,
tool letters, Delete) and never reach the app's undo: Ctrl+Z inside the
field is the text field's own. Outside the chip nothing is trapped.

**Blur and cancel (criterion 20, confirmed):** any press outside the
chip, a tool switch, a selection change, or loss of window focus closes
it and writes nothing. The press that closed it is not swallowed: it also
does what it would have done (selecting another object, toggling the
"Scale stroke width" switch). The chip is visibly transient (accent
border, anchored to a highlighted handle), so a vanished entry is not a
surprise, and a typo can never reach a machine job by blur. The cost, a
lost half-typed number, is small because the field holds one number.

**Validation (criteria 21, 30):** the chip stays open, all text in the
offending field is re-selected, the border becomes 2 px `--field-invalid`
(new token, 4.8:1 on `--toolbar-bg`), and a 12 px message line appears
inside the chip under the fields, `--toolbar-icon` text with a 12 px
exclamation icon, so the state is not carried by color alone. Messages:
"Enter a number" (empty, letters, more than one separator, not finite)
and "Must be above 0" (a size of zero or less). The input has
`aria-invalid="true"` and `aria-describedby` pointing at the message; the
message is a polite live region. The error clears on the next keystroke.
No shake, no sound.

**Pivot and handle state while open (criteria 22, 28):** the pivot marker
is shown at the fixed point for the whole time the chip is open, and the
grabbed handle stays solid. Shift or Ctrl pressed afterwards changes
nothing.

**Scale stroke width during entry:** the switch's value is read when the
chip opens (the analogue of slice 5 criterion 28's "read at pointer-down").
Clicking the switch while the chip is open closes the chip (blur rule).

### Skew (Part B)

**Primitives: hidden, not inert.** A handle on screen promises that
dragging it does something; an inert one needs a third visual state, a
reason the maker has to guess ("is it broken?") and a test of its own, to
protect a case the customer explicitly said can do nothing. Hidden means
the case "press that starts a skew on a primitive" cannot occur through
the UI at all, so criterion 51 holds trivially and nothing is converted,
written or shown. After "Object to path" the four arrows appear in place,
no animation; that appearance is the only hint a maker needs that the
converted shape can do something the primitive cannot.

**Position and glyph:** the radial stack above. The LightBurn
"next to the corner" proposal was dropped: the corners are taken by
rotate and resize, and the corner-adjacent stretch of a side cannot hold a
12 px target clear of both at 64 px. Midpoint at 16 px outward gives
every pair the clearances listed above at any box size.

**Live feedback while skewing:**

- Readout chip (same surface, same pointer anchor as the other readouts):
  "Skew x +12.5°", with a real minus sign (U+2212) for negative values.
  "x" for top/bottom handles, "y" for left/right; one decimal ("22.5°",
  "15°" under Ctrl). Sign as criterion 40.
- Pivot marker at the fixed point (criterion 40).
- A **fixed-line guide**: a 1 px dashed line (4 on / 3 off,
  `--accent` at 100%, token `--transform-guide`; the corner-radius guide token was too faint here) along the line that stays put (the fixed edge,
  or the line through the center under Shift), extended 16 px past each
  end of the box. A point does not tell a maker which edge holds still in
  a shear; the line does. Shown during a skew drag only.
- The selection box in the preview is the tight oriented box around the
  sheared preview geometry (criterion 45), so it grows with the shape.

**Double-click on a skew handle:** nothing; the cursor stays the skew
cursor; no chip, no hint change.

### Select tool top bar

Nothing is added. The bar holds tool-wide settings that persist across
drags ("Scale stroke width"); everything here is either a held modifier
(Shift, Ctrl) or a per-handle transient (entry chip, hint chip). A
permanent "show side rotate handles" or "snap" control would duplicate
what a held key already does, and would put a second way to the same
thing next to a switch that today has none. "Scale stroke width" stays
enabled and unchanged by skew (criterion 42): it is not greyed out
during a skew, because skew simply does not read it. If a typed entry or
a skew snap later needs a persistent setting, it is appended to the bar
under the existing pattern.

### Known gaps, deliberately not closed here

- Typed entry is reachable by double-click only. Canvas handles are not in
  the accessibility tree and have no keyboard route; a keyboard user has
  no way to open the chip. The Properties-panel transform form (listed as
  out of scope) is the accessible path and should be a follow-up spec
  (fields for rotation, width, height, skew).
- No live preview while typing; the result shows after Enter (one undo
  step either way).

## Sequencing

This spec, `specs/0007-stroke-and-fill-styling` and `specs/advanced-selection`
all change the same select/transform code (`select_tool`, the handle layout,
the Select tool's top bar, modifier handling) and must be built one after the
other, not in parallel. The order is for the lead to set. Criterion 11
touches `advanced-selection`'s Shift rules; slice 5's "Scale stroke width"
switch (slice 5 criterion 30) lives in the Select tool's top bar, not in the
Properties panel.

## Links

Requirements: R-EDIT-012 (`docs/requirements.md`); item 8 (Part B,
criteria 37–53, optional 54–56) is R-EDIT-015
Architecture: `adrs.md` in this folder
Builds on: `specs/0005-object-transform/specification.md` (criteria 1–3,
5, 7–9, 11, 13–14, 17–18, 22–24, 26–31) and its `adrs.md`;
`specs/0004-canvas-navigation-and-selection/specification.md` (criteria 20,
23); `specs/advanced-selection/specification.md`
PR:
