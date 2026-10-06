# Object transform refinements: handles, pivots, numeric entry, 22.5° snap

Status: Draft
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
**Item 8 (shear) is a separate part, blocked only on an architect decision
(document model: stored matrix vs baked geometry)** — see "Part B". Items 1–5 and 7 must not wait for
it. Item 6 is out of scope; item 9 is a recorded decision.

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
   release on the center handle without movement writes nothing (slice 5
   criterion 3), and a double-click on it does what a double-click on the
   body does today (`canvas-navigation-and-selection` criterion 23,
   switch to the object's own tool).
4. Given a selection box whose shorter side is smaller than 40 screen
   pixels (Proposal; `ux-engineer` sets the value), then the center handle
   is not drawn and not hit-testable, so it never covers the resize handles.
   Pressing inside the box still moves the object (criterion 3).

### Rotate handles at corners and sides (item 2)

5. Given one selected object, then four rotate handles are always shown,
   one outside each corner of the selection box, on the diagonal through the
   box center. They replace the single rotate handle above the top edge
   (Proposal; see open question 2).
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
9. Given the pointer is within the hit area of two handles, then the handle
   whose center is nearest the pointer wins; on an exact tie a resize
   handle wins over a rotate handle, and any handle wins over the center
   handle (criterion 3's body move is the fallback inside the box). Proposed
   layout (`ux-engineer` to confirm or change): a corner rotate handle sits
   diagonally outside its corner resize handle, far enough that at the
   design-system hit radii the two hit areas do not overlap on an object
   whose box is at least 64 screen pixels on its shorter side; the resize
   handle keeps the corner itself and the rotate handle takes the zone just
   outside it.
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
    no pointer movement beyond the click tolerance), then a text field opens
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
    current rotation writes nothing.
20. Given the field is open, when the maker presses Escape, clicks anywhere
    else, switches tool, or changes the selection, then the field closes and
    nothing is written (Proposal: cancel on losing focus is the safe
    default; `ux-engineer` confirms).
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
    it with "Width" and "Height", pre-filled with the object's current
    size along its own axes (the values the live readout shows, slice 5
    criterion 14) in the document's display unit, the Width field focused
    and selected, Tab moving to Height. Given an edge resize handle, then
    only the one dimension that handle changes is shown (Width for a
    left or right handle, Height for a top or bottom one).
26. Given one selected polygon or star, when the maker double-clicks a
    corner resize handle, then a single field opens, "Radius", pre-filled
    with the outer radius the readout shows; entering a value scales the
    shape uniformly as slice 5 criterion 11 defines.
27. Given a size entry is committed with Enter, then the document ends in
    the same state as a hand-drag with the same handle and the same
    modifiers would have left it at that size: same fixed point, same
    per-kind rules (slice 5 criteria 9–12, polygon/star uniform), and the
    "Scale stroke width" switch (slice 5 criteria 8 and 26) honored as at
    that moment. One commit.
28. Given the entry opens, then the fixed reference is the one the hand-drag
    would use: for a corner handle the opposite corner, for an edge handle
    the opposite edge; if Shift is held at the second press of the
    double-click, the box center. It is fixed when the entry opens and
    shown with the pivot marker while it is open.
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
31. Given an entry equal to the current size, or Escape, or a click
    elsewhere, a tool switch or selection change, then nothing is written
    (as criterion 20).
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

## Part B: Shear handles (item 8) — BLOCKED on the architect

Status of this part: **customer question resolved 2026-10-06: option (a),
shear like Inkscape (the parallelogram).** The customer: "ja genau ich meine
schere wie bei inkscape". The part now needs only an architect ADR or dated
note (document model: stored matrix vs baked geometry, see below) before it
can be Ready. No acceptance criteria are written for it yet, on purpose;
they follow once that note exists. The taper option (b) and "both" (c) are
dropped.

The customer: "ich hätte gerne noch wie bei inkscape und lightburn gemischt
extra handles für 'ins trapez ziehen mit pfeilen. in x und y richtung'".

**What the reference tools actually do.** Inkscape: a second click on the
selected object swaps the handles to rotate and skew handles; the arrows on
the edge midpoints skew (shear) along x or y.
LightBurn: separate shear squares next to each corner, above or below for a
vertical skew, left or right for a horizontal one; its separate Warp/Deform
tool bends an object into any quadrilateral. Neither offers an on-canvas
"trapezoid" handle. In both, the on-canvas skew handles are an affine
shear, whose result is a parallelogram.

**Why this is ambiguous.** In German geometry a Parallelogramm is a special
Trapez (at least one pair of parallel sides), so "ins Trapez ziehen mit
Pfeilen in x und y Richtung" may simply mean Inkscape's skew arrows. It may
also mean a taper (one edge shorter than the opposite one, as in a lampshade
or a tapered cup), which is not an affine map. The two need different
machinery.

| Option | What the arrows do | Needs |
|---|---|---|
| (a) Shear, like Inkscape and LightBurn | drag along an edge slides it sideways: parallelogram; x and y | an affine map; exact for paths (a Bézier stays a Bézier) |
| (b) Taper to a trapezoid | drag changes one edge's length relative to the opposite edge: isosceles trapezoid; x and y | a non-affine map: straight lines stay straight only along the taper axes, other lines become curves, so a path must be re-fitted within a tolerance (lossy, more nodes) |
| (c) Both | both handle sets | everything above, plus a handle layout for two sets of arrows on the same edges (the edge midpoints are already resize handles) |

**What each does to objects.**

- **Paths:** the transform is baked into the anchors and handle vectors, as
  rotate already is (`specs/0005-object-transform/adrs.md`). (a) is exact.
  (b) needs flatten-and-refit within a stated tolerance, which belongs to
  the geometry kernel (ADR 0003). No stored matrix is needed for paths in
  either case; the existing `rotation` register keeps the box orientation.
- **Primitives:** a rectangle, polygon or star cannot represent a shear or a
  taper (their frame is axis-aligned plus a rotation). An ellipse can
  represent some shears (a sheared ellipse is a rotated ellipse with
  other radii) but not all. Three ways to handle this, which the architect
  and customer decide: **P1** refuse: the arrow handles are not shown on
  primitives, and the maker uses "Object to path" first (no document-model
  change, no exception to `primitive-shapes` criterion 21's "no implicit
  conversion"); **P2** convert to a path on the first use, with a visible
  message (an explicit exception to criterion 21); **P3** store a general
  per-node affine (ADR 0002 §5) so primitives stay primitives. P3 is a
  document-model decision (a new stored field, a format version bump, and
  every reader of an outline composing it) and is due anyway with
  `layers-and-grouping`.
- **The selection box after a shear:** slice 5's oriented box assumes
  perpendicular local axes. After a shear the box stays tight in the
  object's `rotation` frame; the architect confirms that is sufficient.

**Decision:** option (a), shear, for paths; the arrow layout follows
LightBurn (a separate small arrow handle next to each corner, since the edge
midpoints are taken by resize handles) mixed with Inkscape's edge-axis
meaning, which is what "gemischt" most plausibly asks for (`ux-engineer`
confirms). Taper (b) becomes its own `Proposal` spec only if the customer
later says they want a real taper (for example for tapered cups on a rotary).

**Primitives: P1 stays the default** (refuse; the maker uses "Object to path"
first). Note that the customer said they want to rework the primitives
anyway ("ich möchte an die primitive eh nochmal ran"), so the primitive
question (P1 vs P2 vs P3) can be revisited together with that work; P3 in
particular is a document-model change that fits there. Part B does not wait
for that rework: it is blocked only on the architect's note, and items 1–5
and 7 ship without it.

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
15° and 22.5° multiples, see criteria 33–36); item 8 shear vs taper (option
(a), shear like Inkscape, see Part B); corner-radius switch (deferred, see
"Out of scope").

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
- Shear handles until Part B is unblocked; taper (trapezoid) handles
  altogether.
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

(filled in by ux-engineer before Ready)

Points the `ux-engineer` is asked to settle, from the criteria above:

- Glyph for the center move handle (four-way arrow is the proposal), its
  hide threshold (criterion 4), and its tokens in `docs/design-system.md`.
- Corner and side rotate handle placement and distance, so they are easier
  to reach than the shipped single handle (criterion 9), and whether the
  shipped rotate offset and hit radii change. Note: the design system lists
  a 20px rotate offset while slice 5 shipped a different value; the doc
  needs reconciling.
- The numeric entry fields: placement near the handle (DOM text input, which
  `docs/design-system.md` permits for text entry), focus, validation
  display, and cancel-on-blur (criteria 20, 31).
- Whether Shift revealing the side handles needs a visible hint.

## Sequencing

This spec, `specs/0007-stroke-and-fill-styling` and `specs/advanced-selection`
all change the same select/transform code (`select_tool`, the handle layout,
the Select tool's top bar, modifier handling) and must be built one after the
other, not in parallel. The order is for the lead to set. Criterion 11
touches `advanced-selection`'s Shift rules; slice 5's "Scale stroke width"
switch (slice 5 criterion 30) lives in the Select tool's top bar, not in the
Properties panel.

## Links

Requirements: R-EDIT-012 (`docs/requirements.md`); item 8 is R-EDIT-015
Builds on: `specs/0005-object-transform/specification.md` (criteria 1–3,
5, 7–9, 11, 13–14, 17–18, 22–24, 26–31) and its `adrs.md`;
`specs/0004-canvas-navigation-and-selection/specification.md` (criteria 20,
23); `specs/advanced-selection/specification.md`
PR:
