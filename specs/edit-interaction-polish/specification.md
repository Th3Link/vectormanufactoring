# Edit interaction polish: star and polygon angle, typed skew and move, copy and one-axis move, Escape, Split selection

Status: Draft
Priority: Must
Origin: Customer

## User value

As a maker I want the Select tool and the Node tool to do the small things I
reach for without a detour, so that I can set a star's angle exactly, type a
skew or a move instead of dragging, copy a part by dragging it, keep a move on
one axis, split a path and pull one end away at once, and leave any tool with
Escape, so that placing parts for a machine takes keystrokes and not guesses.

This is the customer's second change list from testing PR #38
(`specs/unified-object-editing/`, 2026-10-06). His words, with my reading
after each (the repo is English, the customer talks German):

1. "bei star und polygon rastet die rotation beim erstellen des objekts nicht
   ein und die initiale rotation später der nullpunkt. ich fände das schon
   gut, wenn man das deterministisch auf einen winkel bringen kann." Reading:
   while a polygon or star is being created its angle does not snap, and
   afterwards the shape's "rotation 0" is not a recognisable position. He wants
   a way to land on a defined angle. Part A.
2. "auf skew kann man nicht doppelt klicken um einen wert einzugeben." Reading:
   a double-click on a skew handle should open a numeric entry like the rotate
   and resize handles do. Part B.
3. "ich würde gerne mal sehen, wie es aussieht, wenn die bounding box
   gestrichelte linien hat." Reading: he wants to see the selection box with
   dashed lines before deciding. Part E (an experiment, no criteria yet).
4. "bei verschieben möchte ich gerne beim doppelclick auch die verschiebung
   eingeben können. relativ und absolut, default auf relativ. das eingabe
   schema ist doppelclick -> erster wert -> tab -> zweiter wert ->
   (return/enter für relativ || tab -> space für absolut -> return/enter führt
   absolut durch)". Reading: a double-click on the move (centre) handle opens
   a typed move: X, Tab, Y, Enter applies a relative offset (the default); or
   Tab to a Relative/Absolute toggle, Space switches to Absolute, Enter
   applies an absolute position. Part B.
5. "beim verschieben möchte ich gerne mit strg gedrückt ein clone erzeugen
   können." Reading: holding Ctrl while moving makes a copy. Part C.
6. "beim verschieben mit shift soll es an der achse 'sticky' sein. verschieben
   und shift drücken, und dann mit der maus nach rechts oder links / oben oder
   unter halt die relative position der jeweiligen achse bei 0, je nach dem
   welche achse vom objekt ursprung näher ist." Reading: Shift pressed during
   a move locks the move to one axis (the other offset stays 0), the axis being
   the one the pointer is further along, and it stays on that axis. Part C.
7. "wenn man einen node splitted sind beide nodes selektiert" (an
   "Unschönheit"). Reading: after Split both new nodes are selected
   (`specs/0006-path-merge-split-and-node-types/` criterion 15) and he now
   finds that ugly. Part D.
8. "ich möchte generell mit ESC zum auswahlmodus zurückspringen können. das
   ist bei den primitiven einfach: ich muss das objekt eh ziehen. wenn ich
   während des ziehens esc drücke wird das abgebrochen (das ist schon so) und
   wenn ich dann esc drücke gehts zurück zum auswahl tool. beim node tool geht
   es erst zurück, wenn kein objekt mehr aktiv ausgewählt ist." Reading: Escape
   leaves a tool for the Select tool; during a drag it cancels the drag first;
   in the Node tool it first clears what is selected. Part D.

**Field reference.** Inkscape: Ctrl-drag constrains a move to an axis. Illustrator:
Alt-drag copies, Shift-drag constrains a move, and Object > Transform > Move takes
a typed horizontal and vertical distance with a Copy button. Figma: Alt-drag
copies, Shift constrains a move to an axis. LightBurn: its X/Y position fields
take absolute positions against a chosen anchor point of the selection. Where we
do better: the typed move opens on the object itself with one chip and the key
order the customer named, a copy or an axis lock shows in the same
blue-new/black-old preview as every other edit, and Escape has one documented
order in every tool.

## What exists today

Reference state: `main` after PR #35, with PR #38
(`specs/unified-object-editing/`) assumed merged; items that arrive only with
#38 are marked (#38). Facts are from `vecmanf-ui-core/src/{select_tool,
select_tool/entry,poly_star_tool,node_tool,pen_tool,transform_math,
transform_entry,angle_snap}.rs` and `vecmanf-editor-wasm/src/session/` and
`frontend/src/hooks/useEditorSession.ts`.

| Item | What the code does today |
|---|---|
| 1 Star/polygon angle | Create-drag: the press is the centre, the pointer is the first outer vertex (the tip of a star), radius = distance, the angle of the shape's first vertex = `atan2` of the drag vector, stored in the shape's own frame (`StarFrame.angle`). The object's `rotation` register stays 0. No modifier does anything (`shape-creation-from-center` criterion 17). The readout shows only "r 12.0 mm". Afterwards the Select tool's rotation readout and angle-entry prefill show 0°, because they read the `rotation` register, while the shape visibly points anywhere; its oriented box is the circumscribed square turned by that register only. The Ctrl rotate snap measures from the angle at drag start (`object-transform-refinements` criterion 33), so a shape created at an arbitrary angle never reaches clean angles. |
| 2 Skew entry | A double-click on a skew handle is `Ignored`: no entry, no handoff (`SelectDoubleClickOutcome::Ignored`; `object-transform-refinements` criterion 49, "Typed skew entry ... does not exist"). Skew is baked into the path's anchors; no skew is stored, so an entry can only apply a skew *by* an angle, not set one. |
| 3 Dashed box | The selection box is a 1 px solid `--shape-handle-stroke` (`--accent`) outline; the hover box is the same shape at `--accent-hover`. Dashes exist only on the lasso line (4 px on, 3 px off), the skew fixed-line guide and the radius guide. |
| 4 Typed move | The centre move handle has no hit area of its own (a press on it is a body press; hover only, radius `min(12, s/4)` px). A double-click on it, or anywhere inside the sole selected object's box, is the object-to-own-tool handoff (path: Node tool; primitive: its shape tool, and with #38 a hint chip instead). The first press of a handle double-click is only recognised for handles `handle_at` returns, which excludes the centre. A move has no readout (`TransformHandle::Move => None`). |
| 5, 6 Move modifiers | A move (body, outline or centre handle; dead zone 3 px) reads no modifier after the press. Shift at the press: on an object's outline it toggles that object in the selection and a move drag starts as usual; inside the sole selected box (not on an outline) a Shift press starts no move. Ctrl at the press has no meaning for object presses. With `advanced-selection` (not built yet), Shift or Ctrl at a press inside the selected box arms a marquee (add or remove) instead of a move. The document has no duplicate operation. |
| 7 Split | `NodeTool::split_selected` selects both coincident nodes. A plain press on a selected node keeps the whole selection and drags every selected node, so the pair moves together; the maker has to click empty canvas first (criterion 15's own workaround). When two nodes coincide the hit test returns the first in document order. |
| 8 Escape | One `Session::escape`, per tool: Select: cancels the drag and closes an open entry chip, never clears the selection or leaves the tool. Pen: discards the unfinished path (`0002` criterion 4) and does nothing more. Node: clears the node and segment selection and cancels a drag in the same press. Rectangle, Ellipse, Polygon/Star: cancels the create-drag. No Escape leaves a tool for another. The frontend ignores Escape typed into a form control (bar field, entry chip). |

## Parts and dependencies

Five parts; each is independently testable. How they ship (2 or 3 PRs) is the
architect's decision; this spec does not decide it.

- **Part A, orientation of a created polygon or star** (item 1): criteria 1 to 8.
  Touches the document's reading of a polygon's rotation, the polygon/star
  tool and the Select tool's readouts.
- **Part B, typed skew and typed move** (items 2 and 4): criteria 9 to 25. Both
  are new entry chips on the existing entry machinery
  (`object-transform-refinements` criteria 18 to 32). Needs #38 for the centre
  handle's hint (criterion 24).
- **Part C, move modifiers** (items 5 and 6): criteria 26 to 41. Needs a new
  document operation (duplicate objects with an offset, new `NodeId`s, z-order,
  one commit); the architect decides where it lives. Needs #38's blue-new/
  black-old preview (`unified-object-editing` criteria 10 to 13). The typed
  copy (criterion 23) builds on both Part B and Part C.
- **Part D, Escape and Split selection** (items 7 and 8): criteria 42 to 52.
- **Part E, dashed selection box** (item 3): an experiment; no criteria.

## Changes to accepted behaviour, and what this spec supersedes

The lead mentions every customer-visible change at the demo.

1. **`specs/0006-path-merge-split-and-node-types/` criterion 15 is superseded**
   by criteria 50 to 52 (after Split exactly one node is selected, no longer
   both), and its UX note "Visual feedback for Split: yes, both new nodes render
   selected" with it. Criterion 15's workaround (click empty canvas first) is
   no longer needed. The old spec is not edited; its tests for "both nodes
   selected" are rewritten (`vecmanf-ui-core/src/node_tool.rs` tests
   `split_selected_on_*`, `vecmanf-editor-wasm/tests/acceptance_0006*.rs`).
2. **`specs/object-transform-refinements/` criterion 49 and the "typed skew
   entry" out-of-scope line are superseded** by criteria 9 to 14. Criterion 54's
   hint for a skew handle gains the line "Double-click: type an angle".
3. **The centre handle's double-click changes.** `object-transform-refinements`
   criterion 3 (last sentence) and `unified-object-editing` criteria 31 and 32
   (the words "the centre handle" in both) and the centre-handle clause of 33
   no longer apply: a double-click on the drawn centre handle opens the typed
   move (criterion 15) for a path and a primitive alike. A double-click inside
   the box elsewhere, or on the outline, is unchanged (path: Node tool;
   primitive: the hint chip). Where the centre handle is not drawn the old rule
   holds (criterion 17).
4. **The centre handle gets a hit area for double-click and, as a proposal, for
   modifier presses** (`object-transform-refinements` criterion 4 and
   `unified-object-editing` criterion 5 say it owns none). It is hit-tested
   last, only inside its own hover region, so it never wins against another
   handle and a press on it is still a move (criteria 15 and 38).
5. **`specs/shape-creation-from-center/` criterion 17 and its out-of-scope line
   "Any change to polygon and star creation" are superseded for Ctrl only**
   (criterion 4 here). Shift stays without effect on a polygon or star, and
   the press-centre, pointer-vertex rule stays.
6. **The rotation of a polygon or star is shown against a new zero**
   (criteria 1 and 2). The slice-5 note that `rotation` is 0 for every shape
   the app creates (`PrimitiveSnapshot` doc) no longer holds for polygons and
   stars. How it is stored is the architect's call (criterion 2).
7. **Escape gains two levels** (criteria 42 to 49): in every tool the last Escape
   leaves for the Select tool, and in the Select tool Escape clears the
   selection. This extends `0002` (Node tool: "Escape with nothing selected is
   a no-op") and changes the Node tool's Escape during a drag (criterion 45).
   `0002` criterion 4 (Pen: Escape discards the unfinished path) stays.
8. **The advanced-selection note "A plain marquee that must start inside the
   box needs Esc first"** becomes true and useful: Escape in the Select tool
   clears the selection, after which the marquee arms anywhere (criterion 46).

## Acceptance criteria

Document-space millimetres, X to the right, Y down, as the rulers
(`document-size-and-rulers`). "Up" means the top of the screen. Angles are
clockwise on screen, as the existing rotation readout. Shift and Ctrl are read as
the host reports them (Ctrl is Cmd on macOS). A tolerance for comparing lengths
is the document's geometric tolerance. "A move drag" is a Select-tool drag that
moves objects (a press on an object's outline, inside the sole selected box, or
on the centre handle) and has left the 3 px dead zone.

### Part A: orientation of a created polygon or star (item 1)

1. Given any polygon or star, then the rotation shown for it (the live readout
   while rotating, the prefill of the typed angle entry, and the angle of its
   oriented selection box) is the clockwise angle of its first outer vertex
   (for a star, its first tip) measured from straight up, in the range -180°
   to 180°. A shape whose first vertex points straight up shows 0°; right,
   90°; left, -90°; down, 180°. A 4-point polygon at 0° is a diamond, at 45° an
   upright square. Rectangles, ellipses and paths are unchanged.
2. Given a project saved before this change, then it opens with every polygon
   and star looking exactly as saved, shows criterion 1's angle for each, and
   saves with the same `format_version`; a project saved after this change
   opens in an older build of the same `format_version` with the same shapes.
   Which stored fields carry the angle is the architect's decision, not a
   requirement.
3. Given the Polygon/Star tool and a press at A, when the maker drags to B with
   no modifier, then the shape is created as today (centre A, one vertex at
   B, radius |AB|) and it shows the angle of criterion 1 for the direction
   from A to B: dragging straight up gives 0°, straight right 90°. After the
   release the Select tool is active with the shape selected
   (`unified-object-editing` criterion 28) and its readouts agree with this
   (today they show 0° whatever the direction).
4. Given a create-drag in the Polygon/Star tool with Ctrl held, then the angle
   of A to B is replaced by the nearest stop of the table
   `object-transform-refinements` criteria 33 and 34 define (multiples of 15° and
   22.5°, every quadrant, positive and negative, measured from straight up),
   the radius stays |AB|, and the first vertex lies on the snapped direction at
   that radius. Example: A = (100, 50), B = (110, 48), Ctrl held: the raw angle
   is 78.7°, the nearest stop 75°, the first vertex (109.85, 47.36) to 0.01
   mm, radius 10.20 mm. Without Ctrl the angle is the raw one.
5. Given a create-drag with the pointer held still, when the maker presses or
   releases Ctrl, then the preview and the readout change on the next frame, and
   the shape committed on release is built from the pointer position and the
   Ctrl state of the release event, exactly as
   `shape-creation-from-center` criteria 8 to 10 define for Shift and Ctrl on a
   rectangle or ellipse. Escape cancels and writes nothing (`primitive-shapes`
   behaviour, unchanged). Shift has no effect on a polygon or star create-drag.
6. Given a create-drag in the Polygon/Star tool, then the readout also shows
   the angle, after the radius: "r 12.0 mm, 75°" for a polygon and "r 12.0 mm,
   ratio 0.50, 75°" for a star, one decimal at most ("22.5°"), the formatter
   of the rotate readout.
7. Given a polygon or star created with Ctrl at a stop angle S, when the maker
   rotates it in the Select tool with Ctrl held, then it ends on stops
   (refinements criterion 33 measures from the angle at drag start, so the
   result is S plus a stop). Example: created at 45°, a Ctrl rotate drag with a
   raw 10° turn ends at 60°. Given a polygon at any angle, when the maker
   double-clicks a rotate handle and types 0, then its first vertex points
   straight up.
8. Given the Rectangle or Ellipse tool, then creation is unchanged, with Ctrl
   meaning 1:1 and Shift meaning from the centre
   (`shape-creation-from-center` criteria 1 to 7).

### Part B: typed skew and typed move (items 2 and 4)

#### Typed skew

9. Given one selected path that shows skew handles, when the maker double-clicks
   a skew handle (two presses on the same handle within the system double-click
   interval, each moving less than 3 screen pixels, as `object-transform-
   refinements` criterion 18), then an entry chip opens next to the handle with
   one field, the fixed suffix "°", visible label empty, accessible name "Skew
   angle x" for a top or bottom handle and "Skew angle y" for a left or right
   one, pre-filled "0" with the text selected. The first press and release
   writes nothing. No handoff to the Node tool happens, and nothing else
   changes (replaces refinements criterion 49).
10. Given the entry is open and the maker types an angle α and presses Enter, then
    the path is skewed by α about the line that a drag of the same handle holds
    fixed (the opposite side's line; with Shift held at the second press, the
    line through the box centre, refinements criteria 38 and 39, fixed when the
    entry opens and shown by the pivot marker while it is open), in one commit,
    and the document ends in the same state as a drag of that handle ending at
    skew angle α would leave it (one resolving function for both). A positive α
    moves the grabbed side toward +u (top and bottom handles) or +v (left and
    right), the box's own axes, as the drag readout's sign does. The typed
    angle is relative to the current geometry: no skew is stored, so the field
    cannot show or set an absolute skew. Example: a path whose oriented box is
    20 mm wide and 10 mm high at rotation 0, top handle, 45 typed: points on
    the top edge move +10 mm in x, points on the bottom edge do not move, a
    point halfway moves +5 mm, straight segments stay straight, node kinds,
    count, order and stroke width are unchanged, `rotation` is unchanged
    (refinements criteria 42 to 46).
11. Given the field contains text that is not a finite number, the entry stays
    open, is marked invalid "Enter a number" and writes nothing. Given an angle
    of -90° or less or 90° or more, it is marked invalid "Must be between -90
    and 90". Given an angle inside that range whose result would put a
    coordinate beyond the document's coordinate limit (1e7 mm), it is invalid
    "Too large". A decimal comma, a decimal point and a trailing "°" are
    accepted. Ctrl has no effect on the entry; the ±75° cap of a Ctrl-snapped
    drag (refinements criterion 47) does not apply to a typed value.
12. Given an unedited field, a typed 0, an angle equal to no change, Escape, a
    click elsewhere, a tool switch, a selection change or a click on a bar
    switch, then nothing is written and the entry closes; the press that closes
    it is not swallowed (refinements criteria 19, 20, 31 apply word for word).
13. Given a skew by entry followed by a skew by the negated value with the same
    handle and the same Shift state, then every anchor and handle vector is back
    at its original value within the geometric tolerance (refinements criterion
    43). A path skewed by entry saves and reopens as an ordinary path.
14. Given a rectangle, ellipse, polygon, star or a multi-object selection, then
    no skew handle exists and nothing in this part applies
    (`object-transform-refinements` criteria 50 to 53 unchanged).

#### Typed move

15. Given one selected object whose centre move handle is drawn, when the maker
    double-clicks the centre handle (two presses within the double-click
    interval, each inside the hit area of criterion 16 and moving less than 3
    screen pixels), then the move chip of criterion 18 opens next to it, the
    object is not moved by the presses, and no tool switch, hint chip or
    selection change happens. This holds for a path and for every primitive.
16. The centre handle's hit area for this purpose is its hover region (radius
    `min(12, s/4)` screen pixels, `s` the shorter box side in screen pixels).
    It is tested after every other handle and never wins against one, and a
    press on it is a move press (it does not change what a drag from it does).
    A double-click inside the box but outside that region, or on the object's
    outline, keeps the behaviour it has (path: Node tool; primitive: the hint
    chip of `unified-object-editing` criterion 32).
17. Given the centre handle is not drawn (the box's shorter side under 48
    screen pixels, or hidden because a parameter handle is within 20 px of the
    centre, `unified-object-editing` criterion 8), then no typed move can be
    opened by double-click and a double-click inside the box behaves as
    before. This is a known gap, not closed here (question 3).
18. Given the move chip, then it holds two number fields "X" and "Y" (visible
    labels; accessible names "Horizontal offset" and "Vertical offset" while the
    mode is Relative, "X position" and "Y position" while it is Absolute), each
    with the fixed suffix "mm", and a two-state mode control "Relative |
    Absolute" (accessible name "Position mode"). It opens in Relative every
    time, whatever was used last. Focus is in X with its text selected. Tab
    moves X, then Y, then the mode control, then back to X (Shift+Tab reverses);
    on the mode control Space switches between Relative and Absolute (the
    arrow keys do the same). Enter in any of the three controls applies the
    entry in the mode that is current; Escape cancels. Placement, focus ring,
    colours and error text follow the entry chips of
    `object-transform-refinements` (the `ux-engineer` sets the placement for a
    chip that opens at the box centre).
19. Given the chip is open, then a field the maker has not edited means "no change
    on that axis". While untouched, a field shows "0" in Relative mode and the
    object's current X or Y in Absolute mode (criterion 21), and it is
    re-rendered in the new mode's value when the mode switches; a field the
    maker has edited keeps its typed text when the mode switches, and that text
    is read in whatever mode is current at Enter. So "5, Tab, -3, Enter" applies
    a relative offset, and "100, Tab, 50, Tab, Space, Enter" applies an
    absolute position.
20. Given Relative mode and Enter, then the selected object is translated by
    (X, Y) millimetres, X to the right and Y down, by the same operation and with
    the same writes as a drag of the same offset (one commit; rotation, size,
    parameters unchanged). Example: an object whose bounds have top-left (10,
    20), typed 5 and -3: top-left (15, 17).
21. Given Absolute mode and Enter, then the object is translated so that the
    top-left corner of its axis-aligned bounds lies at (X, Y) in document
    coordinates (origin at the document's top-left corner, as the rulers).
    "Axis-aligned bounds" are the tight bounds of the object's outline in
    document space (curve-accurate, as the selection's plain box), also for a
    rotated object; they are not the oriented box and they do not include the
    stroke width. Example: a 30 x 40 rectangle at origin (10, 20), typed 100 and
    50 in Absolute: its origin becomes (100, 50). Which reference point is the
    open question 3.
22. Given a field with text that is not a finite number (empty after editing,
    letters, more than one separator), or a value whose result lies beyond
    ±1e7 mm, when the maker presses Enter, then the chip stays open, the offending
    field is marked invalid "Enter a number" and nothing is written. A decimal
    comma and a point are accepted; a sign is accepted in both modes (a negative
    absolute position is valid: an object may lie outside the page).
23. (Should, Proposal.) Given the second press of the double-click happens with Ctrl
    held, then the chip opens in copy mode, marked with the word "Copy"; Enter
    creates one copy of the object displaced as typed (Relative: by (X, Y);
    Absolute: with its bounds' top-left at (X, Y)) with the result of criterion
    34, and the original is untouched. The Ctrl state is fixed when the chip
    opens; Shift at the second press has no effect.
24. Given the pointer rests on the centre handle for 600 ms, then the hint chip
    names it ("Move", "Shift: keep one axis", "Ctrl: copy", "Double-click: type an
    offset"), and the hint chip of a skew handle gains "Double-click: type an
    angle" (refinements criterion 54, `unified-object-editing` criterion 20).
25. Given the result equals the object's current position (relative 0 and 0,
    absolute equal to the current top-left, or an unedited chip), Escape, a
    click elsewhere, a tool switch, a selection change or a click on a bar
    switch, then nothing is written, the chip closes and the press that closed
    it is not swallowed (refinements criteria 19, 20, 31). Given a committed
    typed move, then there is one commit, the selection and the active tool are
    unchanged, the centre handle is at the object's new centre, and a saved
    project holds the moved object. The typed move exists only for a single
    selected object (the centre handle is not drawn for several, slice 5
    criterion 2).

### Part C: move modifiers (items 5 and 6)

26. Given a move drag in progress, then Shift and Ctrl are read live on every
    frame, also while the pointer holds still (refinements criterion 14), and
    the result a release would commit is built from the pointer position and
    the modifier state of the release event; the blue outline is exactly that
    result (`unified-object-editing` criteria 10 to 13). Escape cancels the
    drag and writes nothing, including no copy.
27. Given a move drag past the dead zone, then a readout chip at the pointer
    (same surface and placement as the other readouts) shows the offset the
    release would commit: "Δ 12.5, −3.0 mm" (X right, Y down, one decimal, a real
    minus sign), after any axis lock. Today a move shows no readout. In copy
    mode (criterion 33) it ends with the word "Copy". The `ux-engineer` may add
    a cursor cue for copy and a guide line for a locked axis; criteria 27 and 33
    say only what must be readable.

#### One axis (Shift)

28. Given a move drag in which the axis lock is engaged (criterion 29), then the
    selected objects move along one document axis only: for the x axis the
    offset is (Dx, 0), for the y axis (0, Dy), where D is the pointer's
    displacement from the press point; along that axis the objects follow the
    pointer 1:1 and the other offset is exactly 0. The axes are the document's
    (the screen's), not the rotated axes of a rotated object.
29. Shift arms the lock only if it was up at the press. A Shift held at the
    press is the selection modifier (slice 4 and `advanced-selection`: it
    toggles an object, or arms the marquee) and does not lock; it locks only
    after it has been released and pressed again during the drag. Without Shift
    at the press, pressing Shift at any moment of the drag engages the lock on
    that frame and releasing it ends the lock on that frame: the objects at
    once follow the pointer freely again at offset D from the press point, with
    nothing accumulated.
30. Given the lock is engaged, then the axis is the one with the larger |D|
    (x on an exact tie), re-evaluated every frame while max(|Dx|, |Dy|) is
    under 8 screen pixels, and from the first frame at which it is 8 screen
    pixels or more it is frozen ("sticky") until Shift is released: it does
    not change however the pointer moves afterwards, also across the diagonal.
    Pressing Shift again chooses anew from the displacement at that moment.
    Example, at a zoom where 30 mm is more than 8 screen pixels: press at (100,
    100); with Shift down the pointer reaches (130, 110):
    the offset is (30, 0) and the axis is x. The pointer goes on to (131, 180)
    with Shift still down: the offset is (31, 0). Shift is released: the offset
    is (31, 80). Shift is pressed again: |Dy| 80 beats |Dx| 31, the offset is
    (0, 80).
31. Given the lock and the 3 px dead zone, then the dead zone is tested on the raw
    pointer movement as for every move; a move whose locked offset is zero when
    released (including a drag back to the start) writes nothing, as
    `unified-object-editing` criterion 12 states for a move dragged back.
32. Given Shift and Ctrl together, then both apply: a copy displaced along the
    locked axis.

#### Copy (Ctrl)

33. Given a move drag, when Ctrl is held at the release, then no object is moved:
    for every selected object one copy is created at the displaced position
    (after any axis lock) and the originals stay exactly as they were, every
    stored value untouched. While the drag runs and Ctrl is held, the originals are drawn
    unchanged and the copies as the blue outline at the offset (the move
    preview; only the readout's word "Copy" and any cursor cue tell it apart
    from a move). Releasing Ctrl before the release turns the drag back into a
    move on the next frame, pressing it turns it into a copy; the Ctrl state at
    the release decides.
34. Given a copy is committed, then each copy has a new `NodeId` (the
    originals keep theirs) and every stored field of its original: kind,
    geometry, `rotation`, stroke width and colour, fill, corner radius, point
    count and ratio, and for a path its anchors with their handle vectors and
    kinds, its closed state; only the position differs. Each copy sits directly
    above its own original in z-order and a selection of several keeps its
    relative order (A, B becomes A, A', B, B'). Everything is one commit (one
    undo step once `undo-redo` exists) and a saved project holds the copies.
35. Given a copy is committed, then the selection is exactly the copies (the
    originals are no longer selected), the Select tool stays active, and a
    single copy shows its full handle set at once. A selection of several kinds
    (paths and primitives) is copied the same way; the copy of a primitive is a
    primitive of the same kind (no conversion to a path).
36. Given a copy whose offset is zero at the release (a drag back to the start, a
    locked axis ending at zero), then no copy is made: duplicates that lie
    exactly on their originals are never created by a drag. Given a press and
    release inside the dead zone, then nothing is written.
37. Given Ctrl is held at the press, then Ctrl keeps its role as a move modifier
    for a press that starts a move (on an object's outline, on an unselected
    object, on the centre handle), and the copy is made if Ctrl is still held
    at the release. Where `advanced-selection` gives Ctrl at the press another
    meaning (a press on empty canvas inside the sole selected object's box arms
    a marquee that removes, its criterion 13), that meaning wins and no move,
    so no copy, starts; the maker presses first and Ctrl then, or presses on the
    centre handle (criterion 38). Until `advanced-selection` ships a press
    inside the box with Ctrl held is a plain move.
38. (Proposal, question 5.) Given a press on the drawn centre move handle with Shift
    or Ctrl held, then a move drag starts and the modifiers are read as move
    modifiers (Ctrl: copy; Shift: a Shift held at that press does not lock, as
    criterion 29, and does not toggle the selection). This gives a maker who
    holds the key first a route that `advanced-selection`'s marquee rule does
    not take.
39. Given the Node tool, the Pen tool or a creation tool, then Shift and Ctrl have
    no new meaning in this part; only the Select tool's move drags are
    concerned.
40. Given a drag of a resize, rotate, skew or parameter handle, then Ctrl and Shift
    keep exactly the meaning they have there; no copy is made by those drags.
41. Given a copy of 200 selected objects (100 paths with 50 nodes each, 100
    rectangles), then the preview follows the pointer at the frame rate of
    `unified-object-editing` criterion 15, and the commit finishes before the next
    frame is drawn on the reference desktop the architect names.

### Part D: Escape and Split (items 7 and 8)

#### Escape

42. Given Escape is pressed with the canvas focused, then exactly one of the
    following happens, the first that applies (one step per key press, the
    document is never written by any step):
    1. an open numeric entry chip or a bar field with focus handles it
       itself (the chip closes, a bar field restores its value) and focus
       returns to the canvas; nothing else happens in that press;
    2. a drag in flight (move, resize, rotate, skew, parameter handle,
       marquee, lasso, a create-drag, a node or handle drag) is cancelled:
       nothing is written, no preview remains; the selection is as it was
       after the press;
    3. the active tool's own state is cleared, in this order for each tool
       (criteria 43 to 46);
    4. otherwise the Select tool becomes active (criterion 47).
    A hint chip or the tooltip on screen is not a step: it disappears as it does
    today, and the same Escape press acts on the next applicable step.
43. Pen tool: Escape discards an unfinished path with at least one node (`0002`
    criterion 4), also while a node's handle is being dragged: that is the
    pen's step 2 and 3 in one, as today, and nothing is written. A second
    Escape, with no path in progress, switches to the Select tool. An
    unfinished path is never kept by Escape; Enter and the double-click finish
    it (unchanged).
44. Rectangle, Ellipse and Polygon/Star tools have no step 3 (their selection is
    empty, `unified-object-editing` criterion 27): the first Escape after a
    cancelled drag, or with no drag, switches to the Select tool.
45. Node tool: step 3 is the node and segment selection: if at least one node or
    segment is selected, Escape clears it (the path stays as it is, the tool
    stays Node); with none selected ("nothing actively selected") the next
    Escape switches to the Select tool. In the Node tool Escape during a node
    or handle drag cancels the drag only and keeps the selection (today it
    clears the selection in the same press, `0002`'s baseline note, superseded
    here: one step per key).
46. Select tool: step 3 is the object selection: if at least one object is
    selected, Escape clears it (no box, no handles, the per-kind bar groups go);
    with none selected, Escape does nothing and the tool stays Select. This
    makes the marquee reachable anywhere after one Escape
    (`advanced-selection`, "needs Esc first").
47. Given Escape switches to the Select tool, then the tool rail highlights Select,
    the Select bar replaces the other bar, the object selection is exactly what
    it was (a path that was open in the Node tool stays selected if it was
    selected), no preview or drag state remains, and `S` and the tool rail keep
    working as before. Switching tools by the rail or a letter is unchanged
    and is not an Escape step.
48. Given the Select tool becomes active again after Escape from a creation tool,
    then nothing is selected (criterion 27 of `unified-object-editing`), so a
    second Escape does nothing.
49. Given the Node, Rectangle, Ellipse or Polygon/Star tool with a pointer drag
    in progress (the button is held) when Escape is pressed, then only that
    drag is cancelled (step 2) and the tool does not change; only an Escape
    pressed with no drag in progress can leave the tool. Pressing Escape again
    while the button is still held does nothing more.

#### Split selection

50. Given a successful Split (`0006` criteria 13 and 14), then exactly one of the
    two resulting coincident nodes is selected and the other is not, on the
    same node-selection model as any single-node selection. The selected one is
    the one a press at the shared position hits, so that a plain press on it and
    a drag moves that node and only that node (an open path: the end of the
    original object's remaining part; a closed path: the new first node of the
    opened path); the architect may instead make the selected node win the
    hit-test tie, and then either node may be the selected one.
51. Given Split on an interior node of an open path at (10, 0), then a drag from
    (10, 0) to (10, 8) leaves one piece's end at (10, 8) and the other at (10,
    0), without any prior click on empty canvas. Given Split on a node of a
    closed path, the same.
52. Given the selection after Split, then the node-tool bar shows Split and Join
    disabled (the selected node is an end of a path, `0006` criterion 12; Join
    needs two), "Make corner", "Make symmetric" and "Make asymmetric" act on the
    one selected node, and nothing else changes in `0006`'s Split rules (the two
    copies, handles zeroed as stated, kinds Corner, z-order above the original).
    Re-Join of a just-split pair is no longer a one-click action here: that is the
    job of Undo (`undo-redo`) or of selecting the two nodes after one has been
    pulled away.

### Part E: dashed selection box (item 3)

53. Not a criterion yet. See "Design experiment" below. Criteria for the chosen
    variant follow after the customer decides; until then the selection box is
    unchanged.

## Design experiment: a dashed selection box (item 3)

The customer asked to see it, not to have it. The `ux-engineer` produces
screenshots of the variants below for the lead to show him; he decides (a
variant, "keep solid", or a different idea). Then the `product-owner` writes
criteria and the design system gets its row. No code is merged for it before
that decision, except what the screenshots need on a throwaway branch.

Variants (the selection box; the hover box is shown both ways, solid and the same
dash, so that the customer sees whether "selected" and "hovered" stay distinct):

| Variant | Box line |
|---|---|
| V0 control | today: 1 px solid `--accent` |
| V1 | 1 px dashed, 4 on / 3 off (the lasso's pattern) |
| V2 | 1 px dashed, 6 on / 3 off |
| V3 | 1 px dashed, 2 on / 2 off |
| V4 | 1 px solid corner ticks 8 px long, dashed 4 / 3 between them |

Scenes, each in light and dark theme at 1x and 2x pixel density: (1) one
rectangle about 120 px across with every handle drawn and a radius set; (2) a
path rotated 30° with its skew handles; (3) three selected objects (each its
own box) with a fourth hovered; (4) a very small object (about 20 px) where the
box and the handles meet.

What to judge, and what must hold in whatever he chooses: the dash lengths and
the 1 px weight are constant screen pixels at every zoom; the dash phase is
anchored at the box's first corner in its own rotated frame, so panning, zooming
or rotating never makes it shimmer or crawl; no animation (no marching ants);
the box must not be mistaken for the lasso line (V1 and V4 share its pattern;
the lasso is only on screen during an Alt-drag, so confusion is unlikely, to be
confirmed by the screenshot); the handles stay readable on top of it; the
contrast against the canvas is at least 3:1 in both themes; it is crisp at 2x.
Deliverable: PNG files and a one-paragraph recommendation by the `ux-engineer`
in `specs/edit-interaction-polish/ux/` (the folder is created by that
deliverable); the lead presents them in one message.

## Out of scope

- Marching ants or any animated box; any decision on dashes before the customer
  chooses (Part E).
- Duplicate in place (Ctrl+D), copy and paste, Alt-drag, a "copy" button next
  to the typed move other than the Ctrl at the second press of criterion 23
  (Proposal), copies from the Node tool.
- Typed move for several selected objects (no centre handle for them), typed
  move from the Node tool or a Properties-panel form, move by arrow keys,
  snapping to grid, objects or guides, a nine-point reference grid for the
  absolute position (LightBurn).
- Storing a skew, or any skew of primitives (`object-transform-refinements`
  decision P1 stands).
- Any other angle behaviour of polygon and star creation (the press-centre,
  pointer-vertex rule stays); a default size or angle for a click without a drag.
- Re-Join or undo of Split; `undo-redo` is its own slice.
- Escape closing dialogs or menus, Escape in the browser build outside the
  canvas, a different key for returning to the Select tool.
- A separate option to lock or copy by a toggle button instead of a held key.

## Questions for the customer

Each has a default; none blocks the work. The lead sends them in one German
message.

1. **Zero angle of a polygon or star (criteria 1 to 3).** What does "rotation
   0°" mean? (a) The first vertex (a star's first tip) points up: a triangle and
   a star stand on their base, a hexagon is pointy-top (default, recommended:
   the picture people have of these shapes; a four-sided polygon at 0° is a
   diamond, at 45° a square); (b) the first vertex points right, as the data is
   stored today (0° = east, the maths convention; a drag to the right gives
   0°); (c) polygons with an even number of sides get a flat edge at the bottom,
   others a vertex up (more rules, rejected unless you want it). If unanswered:
   (a).
2. **How the angle becomes deterministic while creating (criterion 4).**
   (a) Hold Ctrl: the angle snaps to the 15°/22.5° table, a plain drag stays
   free (default, recommended: the same key and the same table as the rotate
   handle's Ctrl; the typed angle after creation stays the exact route, and with
   question 1 (a) typing 0 stands the shape upright); (b) always snapped, and a
   modifier frees it (rejected: free angles are legitimate, and Ctrl would
   then mean the opposite of what it means on the rotate handle). If
   unanswered: (a).
3. **What "absolute" means in the typed move (criterion 21).** (a) The top-left
   corner of the object's axis-aligned bounds, in document millimetres from the
   document's top-left corner, X right, Y down (default, recommended: it is
   what the rulers show and what a machine's work area measures from, and it
   works for a rotated object); (b) the object's centre, i.e. where the centre
   handle sits (it is what you drag); (c) a choice of nine anchor points later.
   Also: the typed move is only available while the centre handle is drawn
   (48 px and up); for a smaller object zoom in (default) or tell me if a
   double-click anywhere in the box should open it for primitives instead.
4. **How "sticky" the axis lock is (criterion 30).** (a) The axis is chosen as the
   one the pointer is further along, decided for good once the pointer is 8
   px away, until you release Shift (default, recommended: no jump when the
   pointer crosses the diagonal; to change axis release Shift and press it
   again); (b) the axis is re-chosen every frame by which of the two offsets is
   larger (Inkscape's constrain; the object jumps from one axis to the other
   at the diagonal); (c) chosen on the very first frame after the dead zone
   (simpler, but decided from 3 px of movement, which is mostly noise). If
   unanswered: (a).
5. **Modifier keys held before the press (criteria 29, 37, 38).** Shift at the
   press selects (adds the object) and Ctrl at a press inside a selected box
   will arm a removing marquee once `advanced-selection` ships; that is why both
   keys are read during the drag. (a) Shift only locks when pressed after the drag
   started; Ctrl copies whenever it is down at the release, and a press on the
   centre handle with either key held is a move (default, recommended); (b)
   Shift always locks, even when held at the press (then a Shift-click to add an
   object and a small drag locks the move). If unanswered: (a).
6. **After Split (criteria 50 to 52).** (a) Exactly one of the two nodes is selected,
   so the next drag separates them (default, recommended: with both selected a
   drag moves both, which is why it felt wrong); (b) none selected; (c) both, as
   now. Either (a) or (b) loses the one-click re-Join of a just-split pair
   (that is what Undo is for, slice 8). If unanswered: (a).
7. **Escape in the Select tool (criterion 46).** Escape clears the selection and,
   with nothing selected, does nothing. (a) Yes (default, recommended: it makes
   the cascade complete and is Inkscape's and Figma's habit; it also gives the
   marquee a start inside a selected box); (b) no, the Select tool ignores
   Escape except for a drag. If unanswered: (a).
8. **Smaller defaults you do not need to answer** (change them if you disagree):
   in the Node tool Escape during a drag cancels only the drag (criterion 45); an
   unfinished Pen path is still discarded by Escape, not finished (criterion
   43); "nothing actively selected" in the Node tool means no node and no
   segment is selected (the path itself being "the object" is not counted);
   the typed move always opens in Relative (criterion 18); Ctrl at the second
   press of the typed move makes a copy (criterion 23, Should).

## UX notes

(Filled in by the `ux-engineer` before Ready.) Open points for them: placement of
the move chip, which opens at the box centre and not at a handle outside the box
(it must not cover the centre handle or the pivot marker); the Relative |
Absolute control's look and focus order; a cursor cue and a guide for copy and
for a locked axis (criterion 27); the move readout's exact text; how the Escape
cascade is discoverable (nothing is shown today, nothing is proposed); the
design experiment's screenshots and recommendation (Part E).

## Links

Requirements: R-EDIT-012, R-EDIT-014 (`docs/requirements.md`)
Builds on: `specs/unified-object-editing/`, `specs/object-transform-refinements/`,
`specs/shape-creation-from-center/`, `specs/0002-path-node-editing/`,
`specs/0006-path-merge-split-and-node-types/`, `specs/advanced-selection/`,
`docs/design-system.md`
PR:
