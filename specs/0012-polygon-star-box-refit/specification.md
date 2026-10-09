# Polygon and star: the selection box turns with the shape

Status: Done
Priority: Should
Origin: Customer (completes `specs/0010-edit-interaction-polish/` criterion 1, "its
selection box with it"; the shipped part A kept the old box as a known limit,
the customer was told at the demo and has not objected)

## User value

As a maker I want the selection box of a polygon or star to sit in the same
direction as the angle I see and type, so that a shape I turn to 0° has an
upright box, the corner handles and the rotate handles are where the angle says
they are, and no loose tilted square is left around it.

Reported in the review of `edit-interaction-polish` part A (2026-10-07): a
polygon made by a drag at 78.7° shows 78.7°. Typing R, 0, Enter turns the shape
upright, but its box keeps the 78.7° tilt and is now a loose, turned square
around an upright shape. The readout and the chip say 0°; the box says
otherwise.

## What exists today

- A polygon or star stores its created direction in its own frame
  (`StarFrame.angle`, the first outer vertex) and the Select tool's turn in the
  `rotation` register. The angle shown and typed is the sum
  (`ObjectSnapshot::orientation()`, `edit-interaction-polish` decision 1).
- The oriented selection box turns by `rotation` alone. A shape made by a drag
  at any angle starts with an axis-aligned box. After a rotation by Δ the box
  is off by the created direction.
- The box of a polygon or star is its circumscribed square, side 2R, centred on
  the shape's centre (the frame box of `primitive-shapes`, criterion 14 of
  `canvas-navigation-and-selection`). That size is accepted behaviour and stays.
  "Tight" in this spec means: that frame box, in the shape's own direction. A
  box shrink-wrapped to the outline is not part of this spec (Out of scope).

## Acceptance criteria

Terms: a polygon or star has centre C, outer radius R, shown angle α (the
clockwise angle of its first outer vertex, first tip for a star, from straight
right, -180° to 180°; `edit-interaction-polish` criterion 1).

1. Given any polygon or star, then its selection box is the square of side 2R
   centred on C whose sides run in the direction α: its corners are
   `C + R·(±1, ±1)` turned clockwise by α about C. The first outer vertex lies
   on the middle of the box's right-hand side (the side the E handle is on).
   At α = 0° the box is axis-aligned, `C ± (R, R)`. The box size is the one
   shown today. Example: C = (0, 0), R = 10, α = 30°: corners
   (-3.66, -13.66), (13.66, -3.66), (3.66, 13.66), (-13.66, 3.66) to 0.01 mm.
2. Given the box of criterion 1, then it is the box drawn for the selected
   shape, for the hovered shape and for every shape of a multi-selection, and
   the one the Select tool's hit rule, handles, readouts and cursors use. There
   is no second box (nothing is drawn from one box and hit-tested against
   another).
3. Given a polygon or star made by the create-drag at direction α (with or
   without Ctrl), when the drag is released and the Select tool takes over,
   then the selected shape's box has the direction α. Example: A = (0, 0),
   B = (8.66, 5.0) gives R = 10.00 mm, α = 30.0° and the corners of
   criterion 1. The rotate readout and the typed-angle prefill show 30°.
4. Given a polygon or star at any α, when the maker rotates it by typing an
   angle A (R, or a double-click on a rotate handle) and confirms, then the
   shown angle is A and the box has the direction A, to 1e-9 rad. Example: a
   shape made at 78.7°, R, 0, Enter: shown 0°, corners `C ± (R, R)`, the
   prefill of the next entry 0°. Given any A, the number in the readout, in the
   chip and the direction of the box edges are the same angle.
5. Given a polygon or star, when the maker drags a rotate handle (corner, or
   side with Shift as `object-transform-refinements` defines), then at every
   frame the live box has the direction of the live shown angle, and the box
   committed on release equals the last live box (corner positions within
   1e-9 mm, direction within 1e-12 rad). A rotation by Δ turns the box by Δ
   about the active pivot and changes the shown angle by Δ (wrapped).
6. Given a polygon or star and Ctrl held during a rotate drag, then the shown
   angle snaps as `edit-interaction-polish` criterion 7 defines, and the box
   direction is that snapped angle exactly (to 1e-12 rad), so the box edges are
   upright whenever the readout shows 0°, 90°, -90° or 180°. Example: a shape at
   78.7° turned by a raw 1° with Ctrl lands on 75° and the box has the
   direction 75°.
7. Given a polygon or star, when the maker drags a corner resize handle (now a
   corner of the turned box), then the shape scales uniformly about C, as
   today: the dragged handle follows the pointer exactly, R changes, α and C do
   not, and the box keeps the direction α. The typed radius (`object-transform-refinements`
   criterion 26) gives the same result as the drag to the same radius. Example: a hexagon with C = (0, 0),
   R = 10, α = 30°; the handle at (13.66, -3.66) dragged by (4.83, -1.29) (5 mm
   along its diagonal) gives R = 13.54 mm to 0.01 mm and α = 30.0°. The stroke
   width follows the "Scale stroke width" switch as today.
8. Given a star, then its inner-radius handle stays on its first inner vertex,
   at the same document position as today for the same file (to 1e-9 mm), and
   dragging it changes the ratio by one millimetre per millimetre of pointer
   travel along the vertex's direction, as today. A polygon has no parameter
   handle, as today. A point-count change keeps α, C and R, so the box does not
   move.
9. Given a polygon or star, then the pivot marker and the fixed point of each
   gesture are: resize, always C; rotate with no modifier, C; rotate with Shift
   from a corner rotate handle, the corner of the box (criterion 1)
   diagonally opposite the grabbed one. A rotation about a pivot other than C
   moves C along the circle about the pivot and turns the box by Δ, exactly as
   the shape turns. The centre move handle sits at C.
10. Given a polygon or star at any α, then no skew handle is drawn or
    hit-testable, as today (skew handles belong to paths only), and a skew
    gesture or entry cannot be started on it.
11. Given a polygon or star, then the cursor of a resize handle is the handle's
    base direction plus α, so the arrows follow the turned box, not the
    `rotation` register alone. Example: a hexagon at 30°: the NE handle's
    cursor is `resize:165.0`, the SE handle's `resize:75.0`; today the NE one
    is `resize:135.0`.
12. Given the Select tool and a press that is not on a handle, when the point
    lies inside the box of the only selected polygon or star, then the press
    starts a move, and a point outside it does not. The test region is the
    turned box. Example: a hexagon at 30°, C = (0, 0), R = 10: the point
    (0, -11) is inside the box and starts a move; today it is outside the
    axis-aligned box. (The corner handles' own hit areas win over the move, as
    today.)
13. Given a project saved before this change with a polygon at frame angle
    78.7° and rotation 0°, and a star at frame angle 10° and rotation 30°, then
    it opens with both outlines at the same positions as today (every outline
    vertex to 1e-9 mm), shows 78.7° and 40°, and the boxes have the directions
    78.7° and 40°. Opening, selecting, hovering, rotating and resizing never
    write a stored field that the same gesture did not write before this
    change: a rotation about C writes `rotation` only, a resize writes the
    frame (radius) only, neither writes the frame angle.
14. Given any gesture of this spec, then no new stored field or key exists,
    `format_version` is unchanged, the project written after a gesture can be
    opened by a build from before this change and shows the same shapes, and a
    gesture is one commit as today (a drag, a typed entry, a creation).
    Undo does not exist yet; the box is derived from stored values on every
    frame, so a later undo returns the box with the shape and has nothing of
    its own to undo.
15. Given a rectangle, an ellipse or a path, then its box, handles, readouts,
    cursors and hit rule are exactly as before this change (regression: the
    box of those kinds still turns by `rotation` alone, tight to the path).
16. Given a polygon or star converted by "Object to path", then the outline is
    unchanged (as today). The converted path gets the box rule of paths
    (tight, in the direction of its `rotation` register), which is not the
    shown angle of the shape before the conversion. This is the behaviour of
    today and is not changed here (Out of scope).

## Out of scope

- A box shrink-wrapped to the outline of a polygon or star (a hexagon's box
  `2R × 1.73R`, a triangle's box not centred on C). It would move the centre
  handle and the pivot off C and change the resize rule and the typed radius.
  Separate story if wanted.
- Folding the created direction into `rotation` or any change of what is
  stored (`edit-interaction-polish` decision 1, option (b), rejected there and
  again in `adrs.md` here).
- "Object to path" carrying the shown angle into the path's `rotation`
  (criterion 16). Tracked in `docs/technical-debt.md`.
- Marquee and lasso selection tests (`advanced-selection`); they take their
  box from the same function when they are built.
- Non-uniform scaling or skewing of a polygon or star (not representable, see
  `docs/technical-debt.md`, "Rotation is a stored angle").
- Any change to rectangles, ellipses or paths.

## UX notes

From the review of `edit-interaction-polish` part A, recommended by the
`ux-engineer`: refit the box when a polygon or star is rotated, so the box and
the readout never disagree. The rule is one sentence: the box direction is the
number in the readout. A regular polygon's box is its circumscribed square, so
a square at 45° (an upright square) sits in a diamond box and a hexagon at 0°
in a box slightly taller than the hexagon; both are the size of the box today.
The `ux-engineer` confirms the corner handle placement and the cursor
directions of criteria 1, 11 on the first build.

## Links

Requirements: R-EDIT-012 (`docs/requirements.md`)
Builds on: `specs/0010-edit-interaction-polish/` (part A, decision 1),
`specs/0005-object-transform/`, `specs/0008-object-transform-refinements/`,
`specs/0009-unified-object-editing/`
Debt: `docs/technical-debt.md`, "Rotation is a stored angle"
PR: https://github.com/curvyo/curvyo/pull/49
