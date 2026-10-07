# Edit interaction polish: star and polygon angle, typed skew and move, copy and one-axis move, Escape, Split selection, keyboard shortcuts

Status: Ready
Priority: Must
Origin: Customer

## User value

As a maker I want the Select tool and the Node tool to do the small things I
reach for without a detour, so that I can set a star's angle exactly, type a
skew or a move instead of dragging, copy a part by dragging it, keep a move on
one axis, split a path and pull one end away at once, leave any tool with
Escape, and reach the typed move, angle, size and skew with one key, so that
placing parts for a machine takes keystrokes and not guesses.

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
   dashed lines before deciding. Part E (an experiment first; decided, criteria 63 to 68).
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

**Customer answers to the first round of questions, 2026-10-06.** They change
parts A, C, D and add Part F:

- Polygon and star angle (question 1): the first vertex pointing right is
  fine as the zero. The real problem is that the angle gets lost: "man hat
  keine chance ein polygon mit irgendeiner kante sauber an der achse
  auszurichten". Reading: the angle shown and typed must be the shape's real
  orientation, typing 0 must restore the zero position, and Ctrl must snap the
  shown angle absolutely. Part A.
- Axis lock (question 4): as in Inkscape, the axis is chosen again on every
  move event, no latch: "wenn man am anfang mal verreisst, kann man einfach
  wieder naeher an die achse gehen und es springt um". The origin axes are
  subtly visible. Part C.
- Modifiers before the press (question 5): Shift (axis lock) and Ctrl (copy)
  may also be held before the press. A copy is visibly marked with a plus; the
  marker disappears when Ctrl is released, and the same for Shift. Part C.
- Split (question 6): just select one of the two nodes (default: the new
  second one). He also wants shortcuts for a selection: M goes straight to the
  typed move; pressing R while doing something wrongly jumps to the Rectangle
  tool (a bug), he expected rotate. Parts D and F.

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
#38 are marked (#38). Facts are from `curvyo-ui-core/src/{select_tool,
select_tool/entry,poly_star_tool,node_tool,pen_tool,transform_math,
transform_entry,angle_snap}.rs` and `curvyo-editor-wasm/src/session/` and
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
| 9 Keyboard | `onKeyDown` on the canvas container (`frontend/src/hooks/useEditorSession.ts`): the letters S, B, N, R, E and `*` switch the tool on any key event whose target is not a form control, with no check for a drag in flight, an unfinished Pen path, key repeat, or Ctrl, Alt or Cmd held. So R, E and the others fire in the middle of a drag, and Ctrl+R and Ctrl+S switch tools. Delete likewise runs in the middle of a drag. Arrow keys do nothing. No key opens a typed entry. |

## Parts and dependencies

Six parts; each is independently testable. How they ship (2 or 3 PRs) is the
architect's decision; this spec does not decide it.

- **Part A, orientation of a created polygon or star** (item 1): criteria 1 to 8.
  Touches the document's reading of a polygon's rotation, the polygon/star
  tool and the Select tool's readouts. Carries a format question for the
  architect (criterion 2).
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
- **Part E, dashed selection box** (item 3): decided; criteria 63 to 68. A rendering
  change in the frontend plus the skew guide; no document change.
- **Part F, keyboard** (item 9): criteria 54 to 62. Opens the chips of Part B
  and of `object-transform-refinements` by key, and fixes the tool letters that
  fire in the middle of an operation. Needs Part B's chips (criteria 9 to 25).

## Changes to accepted behaviour, and what this spec supersedes

The lead mentions every customer-visible change at the demo.

1. **`specs/0006-path-merge-split-and-node-types/` criterion 15 is superseded**
   by criteria 50 to 52 (after Split exactly one node is selected, no longer
   both), and its UX note "Visual feedback for Split: yes, both new nodes render
   selected" with it. Criterion 15's workaround (click empty canvas first) is
   no longer needed. The old spec is not edited; its tests for "both nodes
   selected" are rewritten (`curvyo-ui-core/src/node_tool.rs` tests
   `split_selected_on_*`, `curvyo-editor-wasm/tests/acceptance_0006*.rs`).
2. **`specs/object-transform-refinements/` criterion 49 and the "typed skew
   entry" out-of-scope line are superseded** by criteria 9 to 14. Criterion 54's
   hint for a skew handle gains the line "Double-click or K: type an angle"
   (criterion 24).
3. **The centre handle's double-click changes.** `object-transform-refinements`
   criterion 3 (last sentence) and `unified-object-editing` criteria 31 and 32
   (the words "the centre handle" in both) and the centre-handle clause of 33
   no longer apply: a double-click on the drawn centre handle opens the typed
   move (criterion 15) for a path and a primitive alike. A double-click inside
   the box elsewhere, or on the outline, is unchanged (path: Node tool;
   primitive: the hint chip). Where the centre handle is not drawn, a
   double-click keeps the old rule and the key M opens the typed move instead
   (criteria 17 and 56).
4. **The centre handle gets a hit area for double-click and for modifier
   presses** (`object-transform-refinements` criterion 4 and
   `unified-object-editing` criterion 5 say it owns none). It is hit-tested
   last, only inside its own hover region, so it never wins against another
   handle and a press on it is still a move (criteria 15, 16 and 38).
5. **`specs/shape-creation-from-center/` criterion 17 and its out-of-scope line
   "Any change to polygon and star creation" are superseded for Ctrl only**
   (criterion 4 here). Shift stays without effect on a polygon or star, and
   the press-centre, pointer-vertex rule stays.
6. **The rotation of a polygon or star shows its real orientation** (criteria 1,
   2 and 7): the clockwise angle of its first vertex from straight right. Today
   the Select tool shows 0° for a freshly created shape whatever its direction.
   The slice-5 note that `rotation` is 0 for every shape the app creates
   (`PrimitiveSnapshot` doc) may no longer hold for polygons and stars; how the
   angle is stored is the architect's call (criterion 2). Ctrl rotate of a
   polygon or star snaps the shown angle (absolute), where
   `object-transform-refinements` criterion 33 measures from the angle at drag
   start; other kinds keep that rule.
7. **Shift at the press of a move changes meaning** (criterion 29). Today a
   Shift press on an object's outline toggles that object in the selection at
   the press (slice 4). Now the toggle happens on release, and only if the drag
   never left the dead zone; a Shift drag is an axis-locked move. A Shift-click
   behaves as before.
8. **Escape gains two levels** (criteria 42 to 49): in every tool the last Escape
   leaves for the Select tool, and in the Select tool Escape clears the
   selection. This extends `0002` (Node tool: "Escape with nothing selected is
   a no-op") and changes the Node tool's Escape during a drag (criterion 45).
   `0002` criterion 4 (Pen: Escape discards the unfinished path) stays.
9. **The advanced-selection note "A plain marquee that must start inside the
   box needs Esc first"** becomes true and useful: Escape in the Select tool
   clears the selection, after which the marquee arms anywhere (criterion 46).
10. **The letters R and S change meaning in the Select tool while something is
    selected** (criterion 54): they open the typed angle and the typed size
    instead of switching to the Rectangle and Select tools. To draw a rectangle
    then, press Escape first. This changes two accepted shortcuts; the customer
    approved it on 2026-10-06 (question 9). All tool letters and Delete are ignored during an
    operation (a drag, an unfinished Pen path), while typing and with Ctrl, Cmd
    or Alt held (criterion 55); today they fire.

11. **The selection box becomes dashed** (4 on / 3 off, criteria 63 to 67), and
    **the skew fixed-line guide changes from 4 / 3 to 2 / 2**, which amends
    `object-transform-refinements` criterion 56 (criterion 68). The lasso keeps
    4 / 3. The design system's rows ("Transform skew fixed-line guide", the
    selection box) are the `ux-engineer`'s to update.

## Acceptance criteria

Document-space millimetres, X to the right, Y down, as the rulers
(`document-size-and-rulers`). "Up" means the top of the screen. Angles are
clockwise on screen, as the existing rotation readout. Shift and Ctrl are read as
the host reports them (Ctrl is Cmd on macOS). A tolerance for comparing lengths
is the document's geometric tolerance. "A move drag" is a Select-tool drag that
moves objects (a press on an object's outline, inside the sole selected box, or
on the centre handle) and has left the 3 px dead zone.

### Part A: orientation of a created polygon or star (item 1)

1. Given any polygon or star, then the angle shown for it (the create-drag
   readout of criterion 6, the live readout while rotating, and the prefill of
   the typed angle entry) is its real orientation: the clockwise angle of its
   first outer vertex (for a star, its first tip) measured from straight
   right, in the range -180° to 180°. A shape whose first vertex points
   straight right shows 0°; down, 90°; up, -90°; left, 180°. It is the same
   number in all three places and it is never reset by a selection change, a
   save or a reopen. Examples: a triangle with its vertex up shows -90° and
   stands on its base; a 4-point polygon at 0° is a diamond, at 45° an upright
   square; a star with its first tip up shows -90°. Rotating a polygon or star by
   Δ turns it, and its selection box with it, by Δ about its centre, and the
   shown angle changes by Δ (wrapped into the range). Rectangles, ellipses and
   paths are unchanged.
2. Given a project saved before this change, then it opens with every polygon
   and star looking exactly as saved and shows criterion 1's angle for each.
   This is required. Given a project saved after this change, then a polygon or
   star reopens with the same shape and the same shown angle. (Should) Such a
   project also opens in a build from before this change with the same shapes.
   **Format question for the architect:** the angle of a created polygon or
   star is stored today in the shape's own frame (`StarFrame.angle`) while the
   `rotation` register stays 0, and the Select tool reads the register. Which
   stored fields carry the shown angle, whether `format_version` changes and how
   old files are read is the architect's decision, not a requirement here;
   existing files must keep opening unchanged whatever is chosen.
3. Given the Polygon/Star tool and a press at A, when the maker drags to B with
   no modifier, then the shape is created as today (centre A, one vertex at
   B, radius |AB|) and it shows criterion 1's angle for the direction from A to
   B: dragging straight right gives 0°, straight down 90°, straight up -90°.
   After the release the Select tool is active with the shape selected
   (`unified-object-editing` criterion 28) and its readouts agree with this
   (today they show 0° whatever the direction).
4. Given a create-drag in the Polygon/Star tool with Ctrl held, then the angle
   of A to B is replaced by the nearest stop of the table
   `object-transform-refinements` criteria 33 and 34 define (multiples of 15° and
   22.5°, every quadrant, positive and negative; the table is the same
   measured from right or from up, because 90° is a multiple of both steps),
   the radius stays |AB|, and the first vertex lies on the snapped direction at
   that radius. Example: A = (100, 50), B = (110, 48), Ctrl held: the raw angle
   is -11.3°, the nearest stop -15°, the first vertex (109.85, 47.36) to 0.01
   mm, radius 10.20 mm. Without Ctrl the angle is the raw one.
5. Given a create-drag with the pointer held still, when the maker presses or
   releases Ctrl, then the preview and the readout change on the next frame, and
   the shape committed on release is built from the pointer position and the
   Ctrl state of the release event, exactly as
   `shape-creation-from-center` criteria 8 to 10 define for Shift and Ctrl on a
   rectangle or ellipse. Escape cancels and writes nothing (`primitive-shapes`
   behaviour, unchanged). Shift has no effect on a polygon or star create-drag.
6. Given a create-drag in the Polygon/Star tool, then the readout also shows
   the angle of criterion 1, after the radius: "r 12.0 mm, -15°" for a polygon
   and "r 12.0 mm, ratio 0.50, -15°" for a star, one decimal at most ("22.5°"),
   the formatter of the rotate readout.
7. Given a polygon or star at any shown angle, when the maker rotates it in the
   Select tool with Ctrl held, then the shown angle (not the turn since the drag
   started) is snapped to the table of criterion 4, so a shape created at a free
   angle reaches clean angles. Other kinds keep the relative rule of
   `object-transform-refinements` criterion 33. Examples: a polygon created at
   45° and turned by a raw 10° ends at 60° (shown 55°, nearest stop 60°); a
   polygon at 78.7° turned by a raw 1° ends at 75°. Given a polygon or star at
   any angle, when the maker double-clicks a rotate handle (or presses R, criterion
   57) and types an angle A, then its shown angle becomes A, and with 0 its
   first vertex points straight right. The typed route is exact, so a maker can
   put an edge on an axis: a regular N-sided polygon has an edge on a horizontal
   or vertical axis at 0° when N is odd or N is 2 more than a multiple of 4
   (3, 5, 6, 7, 9, 10, ...), and at 180/N degrees when N is a multiple of 4
   (4 sides: 45°; 8 sides: 22.5°; 12 sides: 15°). Examples: a hexagon at 0° has
   a horizontal edge at the top and at the bottom; a pentagon at 0° has a
   vertical edge on its left; a square at 45° has four axis-parallel edges.
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
    before. The key M opens the same chip for an object of any size, placed
    where the centre handle is or would be (criterion 56), so the typed move is
    reachable for every single selected object and no gap is left for keyboard
    users.
18. Given the move chip, then it holds two number fields "X" and "Y" (visible
    labels; accessible names "Horizontal offset" and "Vertical offset" while the
    mode is Relative, "X position" and "Y position" while it is Absolute), each
    with the fixed suffix "mm", and a two-state mode switch "Relative | Absolute"
    (one control with `role="switch"`, `aria-checked` true in Absolute,
    accessible name "Absolute position"). It opens in Relative every time,
    whatever was used last. Focus is in X with its text selected. Tab moves X,
    then Y, then the mode switch, then (with criterion 23) the "Copy" check, then
    back to X (Shift+Tab reverses); on the mode switch Space and the arrow keys
    flip between Relative and Absolute. Enter in any of the controls applies the
    entry in the mode that is current; Escape cancels. The chip opens at the box
    centre, top left 16 px right of and 16 px below the centre glyph (flipped
    left or up like the readout, clamped to the window), never covering the glyph;
    the centre handle stays drawn in its dragging look while it is open; no pivot
    marker. Layout, focus ring, colours and error text follow the entry chips of
    `object-transform-refinements` and `docs/design-system.md` ("Move entry
    chip").
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
    "Axis-aligned bounds" are the tight bounds of the object's drawn outline in
    document space (curve-accurate for a path; for a primitive the outline as
    drawn, after its rotation), also for a rotated object; they are not the
    oriented box, not the unrotated frame box that the selection code keeps for
    a primitive, and they do not include the stroke width. For a star or
    polygon, absolute X, Y is therefore the top-left of the box around its
    vertices, not the corner of its circumscribed square. Example: a 30 x 40 rectangle at origin (10, 20), typed 100 and
    50 in Absolute: its origin becomes (100, 50). Which reference point is the
    open question 3.
22. Given a field with text that is not a finite number (empty after editing,
    letters, more than one separator), or a value whose result lies beyond
    ±1e7 mm, when the maker presses Enter, then the chip stays open, the offending
    field is marked invalid "Enter a number" and nothing is written. A decimal
    comma and a point are accepted; a sign is accepted in both modes (a negative
    absolute position is valid: an object may lie outside the page).
23. (Should, Proposal.) Given the move chip, then it holds a "Copy" check (a
    checkbox, accessible name "Copy"), the fourth control in the Tab order of
    criterion 18, unchecked on every open. When it is checked at Enter, the
    chip creates one copy of the object displaced as typed (Relative: by (X, Y);
    Absolute: with its bounds' top-left at (X, Y)) with the result of criterion
    34, and the original is untouched; the selection is then the copy, as in
    criterion 35. Given the second press of the
    double-click happens with Ctrl held, then the chip opens with the check
    already on. The check is the route for the key M, which cannot carry a Ctrl,
    and for a maker who does not hold Ctrl. Shift at the second press has no
    effect.
24. Given the pointer rests on the centre handle for 600 ms, then the hint chip
    names it ("Move", "Shift: keep one axis", "Ctrl: copy", "Double-click or M:
    type an offset"), and the hint chip of a skew handle gains "Double-click or K:
    type an angle" (the right and left handles: "Shift+K"). The hint lines that
    name the typed entry of the rotate and the resize handles gain "or R" and "or
    S" in the same way (refinements criterion 54, `unified-object-editing`
    criterion 20; keys per criterion 54 here).
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
    every indicator of criteria 27 and 33 appears and disappears in the frame of
    the key event. The result a release would commit is built from the pointer
    position and the modifier state of the release event; the blue outline is
    exactly that result (`unified-object-editing` criteria 10 to 13). Either key
    may already be down at the press (criteria 29 and 37). Escape cancels the
    drag and writes nothing, including no copy; a Ctrl that is still held then
    keeps showing the plus badge of criterion 33.
27. Given a move drag past the dead zone, then a readout chip at the pointer
    (same surface and placement as the other readouts) shows the offset the
    release would commit: "Δ 12.5, −3.0 mm" (X right, Y down, one decimal, a real
    minus sign), after any axis lock ("Δ 30.0, 0.0 mm" for a lock to x). Today a
    move shows no readout. In copy mode (criterion 33) it ends with the word
    "Copy": "Δ 12.5, −3.0 mm Copy". While the axis lock is engaged (criterion 29)
    and the drag is past the dead zone, then in addition:
    - **origin axes**: two lines through the selection's start centre (the
      position of the centre handle at the press), one horizontal and one
      vertical, 1 px solid, over the full viewport; the line of the axis the move
      is locked to is drawn in `--axis-guide` (`--accent` at 50%), the other in
      `--axis-guide-idle` (`--accent` at 20%). They are faint on purpose
      (informational; the motion, the badge and the readout say the same), they
      lie above the artwork and below the blue outline, the selection box and the
      handles, and they are gone in the frame Shift is released;
    - **lock badge** by the pointer, left of and below the hotspot, on the side
      opposite the readout, beside the plus badge when both show: white, a 1.5 px
      `--accent` ring, a horizontal or a vertical double arrow for the current
      axis, gone in the frame Shift is released. It does not show before the press
      (there Shift means "add to selection", criterion 29), and the lock still
      engages on the first frame past the dead zone, so no unlocked frame is
      drawn after Shift was down at the press.
    The `ux-engineer` owns the exact sizes (`docs/design-system.md`, "Modifier
    indicators in a move"); these criteria say what must be on screen and when.

#### One axis (Shift)

28. Given a move drag in which the axis lock is engaged (criterion 29), then the
    selected objects move along one document axis only: for the x axis the
    offset is (Dx, 0), for the y axis (0, Dy), where D is the pointer's
    displacement from the press point; along that axis the objects follow the
    pointer 1:1 and the other offset is exactly 0. The axes are the document's
    (the screen's), not the rotated axes of a rotated object.
29. Shift may be down before the press, or pressed or released at any moment of
    the drag; the Shift state of the current frame decides. Given Shift is down
    at the press that starts a move (on an object's outline, on an unselected
    object, on the centre handle, criterion 38), then the selection does not
    change at the press. If the button is released without the drag having left
    the dead zone, the press was a click and toggles the object in the selection
    as before (slice 4). Once the drag leaves the dead zone it is an axis-locked
    move of the selection, from the first frame, and an unselected pressed object
    joins the selection at that moment. Without Shift at the press, pressing
    Shift at any moment of the drag engages the lock on that frame and releasing
    it ends the lock on that frame: the objects at once follow the pointer
    freely again at offset D from the press point, with nothing accumulated. A
    Shift press inside the sole selected box that is not on the object starts no
    move (unchanged; with `advanced-selection` it arms the marquee).
30. Given the lock is engaged, then the axis is chosen again on every pointer
    event, as in Inkscape: the axis with the larger |D| (|Dx| against |Dy|); on
    an exact tie the previous axis is kept (x if there was none). There is no
    latch and no hysteresis: if the pointer starts out along the wrong axis it
    can simply come back nearer the other axis and the objects jump over to it,
    also across the diagonal. Pressing Shift again chooses anew from the
    displacement at that moment. Example: press at (100, 100); with Shift down the
    pointer is at (130, 110): the offset is (30, 0), the axis is x. The pointer
    moves to (131, 180) with Shift still down: the axis has switched, the offset
    is (0, 80). The pointer returns to (131, 105): the axis is x again, the
    offset is (31, 0). Shift is released with the pointer at (131, 180): the
    offset is (31, 80). Shift is pressed again: |Dy| 80 beats |Dx| 31, the offset
    is (0, 80).
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
    stored value untouched. While the drag runs and Ctrl is held (it may have
    been down since before the press), the originals are drawn unchanged, the
    **selection box and the handles stay on the originals**, and the copies are
    the blue outline travelling alone at the offset; in a move the box and the
    handles travel with the blue outline. Releasing Ctrl before the release turns
    the drag back into a move in the frame of the key event (the box and handles
    follow the blue outline again), pressing it turns it into a copy in that
    frame, also while the pointer holds still; the Ctrl state at the release
    decides. A **plus badge** marks a copy: 16 px, solid `--accent`, a white plus,
    its centre 16 px left of and 16 px below the pointer hotspot (clear of the
    arrow, move and crosshair cursors and opposite the readout chip). It is a
    DOM element that follows the key event, not a CSS cursor, and it is gone in
    the frame Ctrl is released. It shows (a) during a copy drag and (b) wherever
    a press with Ctrl held would start a move, using the hit test of the press
    (criterion 37): on an object's outline, on an unselected object, on the
    centre handle; not on empty canvas, where Ctrl belongs to the marquee. The
    word "Copy" in the readout (criterion 27) is the third marker. Pressing Ctrl
    hides a hint chip, as any key does.
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
    at the release; the plus badge of criterion 33 shows for exactly these
    presses. Where `advanced-selection` gives Ctrl at the press another
    meaning (a press on empty canvas inside the sole selected object's box arms
    a marquee that removes, its criterion 13), that meaning wins, no move and so
    no copy starts and no badge shows; the maker presses first and Ctrl then, or
    presses on the centre handle (criterion 38). Until `advanced-selection`
    ships a press inside the box with Ctrl held is a plain move. Shift at the
    press follows criterion 29.
38. (Follows from the answer to question 5.) Given a press on the drawn centre
    move handle with Shift or Ctrl held, then a move drag starts and the
    modifiers are read as move modifiers: Ctrl makes a copy; Shift locks the
    axis from the first frame past the dead zone and never toggles the
    selection (a click on the handle with Shift down and no movement is a click
    on the handle: nothing is toggled). This gives a maker who holds the key
    first a route that `advanced-selection`'s marquee rule does not take.
39. Given the Node tool, the Pen tool or a creation tool, then Shift and Ctrl have
    no new meaning in this part; only the Select tool's move drags are
    concerned.
40. Given a drag of a resize, rotate, skew or parameter handle, then Ctrl and Shift
    keep exactly the meaning they have there; no copy is made by those drags.
41. Given a copy of 200 selected objects (100 paths with 50 nodes each, 100
    rectangles), then the live preview follows the pointer within the 8 ms
    budget per draw of `unified-object-editing` criterion 15 on the reference
    desktop the architect names. The commit has no time budget: the implementer
    measures it in a release build and the number goes into the PR description
    and the demo message (about 30,000 keys and 200 tree nodes are written, which
    does not fit one frame in WebAssembly).

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
    the new second node: the copy that keeps the original outgoing handle (for
    an open path the first node of the second path object; for a closed path the
    new first node of the opened path). A press at the shared position hits the
    selected node, never the unselected one (a hit-test tie goes to the selected
    node), so that a plain press on it and a drag moves that node and only that
    node. The selected node's glyph is drawn above the unselected one at the
    same position (today the two glyphs overlap and the selected fill would be
    hidden). The customer asked for one selected node (answer to question 6); "the
    new second one" is the default he named.
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

Decided 2026-10-06 (see "Design experiment, decided"). The criteria are numbered
63 to 68 because Part E was decided after Part F was numbered. The measured
findings are in `specs/edit-interaction-polish/ux/dashed-notes.md`.

63. Given any selected object (several selected objects each show their own box),
    then its selection box line is dashed 4 px on / 3 px off, 1 px wide, in
    `--shape-handle-stroke` (`--accent`), as the lasso's pattern. Dash and gap
    lengths are screen pixels at every zoom. The dashes are static: no animation,
    no marching ants. The phase is a pure function of the box (it starts at the
    box's first corner in the box's own, possibly rotated, frame), so panning,
    zooming and a move drag shift the pattern only rigidly with the box and it
    never shimmers or crawls. Only the selection box changes: the marquee box
    stays solid, the lasso keeps 4 / 3, and the radius guide is unchanged.
64. Given a selection box, then every corner is closed: both edges that meet at a
    corner have a dash there. The nominal pattern stays the customer's V1, about
    4 px on / 3 px off (criterion 63). The pattern is fitted to each edge so that
    it is symmetric about the edge's centre: the gap stays between 2 and 4 px and
    the dash is 4 px wherever an exact fit exists. No 4 on / 3 off pattern closes
    both corners on every edge length (edges between 12 and 16 px and between
    20 and 22 px have no exact fit), so the flex rule is: on such an edge the gap
    is 2 px and the dash flexes to about 2.7 to 4 px, so the corners always
    close. A dash is never longer than 4 px and never shorter than 2.5 px. An
    edge too short for two dashes and a gap (under 10 px) is drawn solid. The method is the `ux-engineer`'s; this is
    what is observable. (Closed corners are the finding in `dashed-notes.md`:
    with a plain "start at corner 0" some corners end in a gap.)
65. (Should, Proposal; default yes, small.) Given an axis-aligned selection box
    or hover box, then its line covers whole device pixel rows and columns, so
    that it is never drawn as two rows at about 50% (today at 1x, wherever the
    edge falls between two pixel rows, 1.9:1 against the canvas): at full
    `--accent` the contrast against the canvas is 3.7:1 (at least 3:1) at every
    zoom and position, at 1x and at 2x. A rotated box is drawn anti-aliased at
    full `--accent` and 1 px width, as it can be no crisper.
66. Given an object under the pointer that is not selected, then its hover box
    stays solid at `--accent-hover` (default, flagged in question 8), so that
    "selected" (dashed) and "hovered" (solid) stay distinct. The hover box is
    pixel-aligned as in criterion 65.
67. Given a box that lies on the object's own outline (a rectangle's box lies on
    its stroke), then the dashes are drawn on top of the artwork and do not
    knock anything out: in each gap the object's own pixels show, so the line
    reads blue and black. This is the accepted consequence of the dashed box,
    and the box is drawn above the artwork and below the handles. Handles stay
    on top and fully readable.
68. Given the skew fixed-line guide (`object-transform-refinements` criterion 56,
    which says "1 px dashed guide"; the pattern was 4 on / 3 off,
    `docs/design-system.md` "Transform skew fixed-line guide"), then the guide
    is 1 px dashed 2 px on / 2 px off, in full `--accent`, in screen pixels at
    every zoom, so that it no longer looks like the box (the guide runs along a
    box edge). Its extent (16 px past each end of the box), its Shift variant (the
    line through the box centre) and its lifetime (until release or Escape) stay
    as criterion 56 says. This amends criterion 56; the lasso keeps 4 / 3.
    Note for the UX review: `dashed-notes.md` found 2 / 2 dissolves into a mid-tone
    on rotated edges at 1x; the review checks the guide on a rotated box and may
    propose a variant for the customer.

### Part F: keyboard (item 9)

"The canvas has focus" means a key event whose target is the canvas container
(not a form control). "A tool letter" is B, N, E, R, S or `*`; "an entry key" is M,
R, S, K or Shift+K. The scheme has one rule: one key has one meaning in one
visible state, and the only state that changes a letter is "Select tool with at
least one object selected". The reasoning and the key table for the design
system are in "UX notes round 2", section 1.

54. Given the canvas has focus and the gate of criterion 55 lets the key through,
    then:
    - B selects the Pen tool, N the Node tool, E the Ellipse tool and `*` the
      Polygon/Star tool, in every state, as today.
    - R: in the Select tool with at least one object selected, it opens the
      typed angle (criterion 57); in every other state (nothing selected, or any
      other tool) it selects the Rectangle tool.
    - S: in the Select tool with at least one object selected, it opens the
      typed size about the object's centre (criterion 57); in every other state
      it selects the Select tool.
    - M: in the Select tool with one object selected, it opens the typed move
      (criterion 56); otherwise criterion 59 applies.
    - K, and Shift+K: in the Select tool with one path selected, they open the
      typed skew x and skew y (criterion 58); otherwise criterion 59 applies.
    Letters are matched without regard to Caps Lock; Shift is read only for `*`
    and Shift+K. The tool rail and every other way of switching tools are
    unchanged. Example: after a rectangle is drawn the Select tool is active with
    it selected, so R opens its angle entry; to draw the next rectangle the maker
    presses Escape (the selection clears, criterion 46), then R.
55. Given a key event for a tool letter, an entry key, Delete or Backspace, then
    one gate decides, called first by the key handler, for all of them alike. The
    key has no effect at all, and the page's default for it is not prevented,
    when any of these holds:
    - Ctrl, Cmd or Alt is down (Shift is allowed only for `*` and Shift+K);
    - the event is a key repeat or part of an IME composition (the first press
      acts, a held key does not repeat the action);
    - focus is in a text field, select, button, switch or contenteditable element,
      or an entry chip is open;
    - a drag is in flight in any tool: move, resize, rotate, skew, parameter
      handle, marquee, lasso, create-drag, node or handle drag, or a pan;
    - the Pen tool has an unfinished path;
    - Space is held.
    Escape, Space (pan while held, never interrupting an operation), Enter
    (finishes a Pen path, unchanged) and the modifier keys themselves are never
    gated by this list (Escape against key repeat: criterion 60). Examples: during
    a Select-tool move drag, pressing R, E, B, N, S, M, K or Delete changes no tool
    and deletes nothing, the drag goes on and its commit equals the commit of the
    same drag without the key; Ctrl+R, Ctrl+S, Cmd+R and Alt+E change no tool;
    with a Pen path of three nodes open, B, N, E, R, S, `*` and Delete do nothing
    and Enter, a double-click or Escape end the path as today; typing "r" into
    the bar's number field or into an entry chip changes no tool.
56. Given the Select tool with one object selected, when the maker presses M, then
    the move chip of criterion 18 opens exactly as a double-click on the centre
    handle opens it (criteria 15, 18 to 22, 25), for an object of any size: where
    the centre handle is not drawn, the chip opens where it would be. Examples: M,
    5, Tab, -3, Enter moves the object by (5, -3) mm; M, 100, Tab, 50, Tab, Space,
    Enter puts the top-left of its axis-aligned bounds at (100, 50) (criterion
    21). A Copy check (criterion 23) is reachable by Tab.
57. Given the Select tool with one object selected, when the maker presses R or S,
    then the angle entry or the size entry of `object-transform-refinements`
    (criteria 18 to 32) opens as a double-click on a rotate or a resize handle
    opens it, at the top-right corner rotate handle (R) or the bottom-right
    corner resize handle (S) of the object's oriented box, whether that handle
    is drawn or not, with one difference for S, set by the customer on
    2026-10-07 ("wenn man einfach S drückt soll es zentrisch skalieren"): **the
    fixed point of the typed size is the centre of the object's oriented box**
    (criterion 57a), not the corner opposite the handle the chip sits at. For a
    polygon or star the angle prefill is the angle of criterion 1. Example: a
    star shown at -15° stands with its first tip straight right after R, 0,
    Enter. Example for S: a 40 x 20 mm rectangle at (10, 10), S, 60, Tab, 30,
    Enter, leaves a 60 x 30 mm rectangle at (0, 5): the centre (30, 20) stays.
57a. Given the typed size, then two rules apply, side by side, and no other:
    | Route | Fixed point of the typed size | Why |
    |---|---|---|
    | Double-click on a resize handle (`object-transform-refinements` criteria 25 to 32, unchanged) | exactly what a hand-drag of that handle holds: the opposite corner of a corner handle, the opposite edge of an edge handle, the box centre if Shift is held at the second press; the shape centre for a polygon or star | the maker chose a handle, so the typed size continues that drag |
    | The key S (criterion 57) | the box centre, as if Shift were held on a drag, always | no handle was chosen; the chip sits at the hidden bottom-right handle only as a place on screen |

    The S route never reads a modifier (criterion 59). Details:
    - **Rectangle, ellipse, path:** the oriented box grows or shrinks about its
      centre, each side by half the change of W and H; the document ends in the
      same state as a Shift-drag of the bottom-right corner handle ending at
      that size (the same resolving function, refinements criterion 27 with
      the centre as the fixed point). The "Scale stroke width" switch is
      honoured as in refinements criterion 27. A path is scaled in its own
      oriented frame about the box centre, like the other kinds.
    - **Polygon and star:** unchanged. The single field "r" scales about the
      shape's centre, as it does on every route.
    - **Ctrl linked W and H** (refinements criterion 29) is a Ctrl held at the
      second press of a double-click. The key S cannot carry it (Ctrl+S is
      gated by criterion 55), so after S the fields are independent. A link
      switch in the chip is not added (Out of scope).
    - **Pivot marker:** while the S chip is open the marker shows at the box
      centre (the fixed point). The double-click route keeps its marker where
      its fixed point is.
    - **Placement of the chip (decided 2026-10-07, UX review):** the S chip is
      placed as the move chip is, its top left 16 px right of and below the box
      centre (flipped and clamped as the move chip is), not at the hidden
      bottom-right handle. No handle takes its dragging look, the centre
      handle's glyph makes way for the pivot marker, and the marker at the
      centre is drawn at full `--accent` while the chip is open. The double-click
      route on a resize handle keeps its handle in the dragging look, its chip
      at the handle and its marker at the opposite point.
    - **Hint chip:** the resize hint lines read "Double-click or S: type a
      size" and a second line "S: from the center; double-click: opposite
      corner" (criterion 24), on the two corner resize hints.
58. Given the Select tool with one path selected, when the maker presses K, then
    the skew entry of criterion 9 for the top handle opens, accessible name "Skew
    angle x"; with Shift+K the one for the right handle, "Skew angle y". Both
    apply as criterion 10 describes. The key route never uses the Shift pivot of
    criterion 10: the line held fixed is the opposite side's line.
59. Given a chip opened by an entry key, then it behaves in every respect as the
    chip of the double-click route (fields, prefill, validation, Enter writes one
    commit, Escape, a click elsewhere or a tool switch write nothing), except the
    fixed point of S, the missing Ctrl link and the placement of the S chip
    (criterion 57a), it opens where that handle is or would be, the handle if
    drawn shows its dragging look (the S chip: by the box centre, no handle
    highlighted), focus returns to the canvas when it closes, and while a chip is open
    the Shift-revealed side rotate handles other than the open chip's own are
    hidden (so no key-opened chip sits on one; the side rotate handle that a
    double-click chip belongs to stays drawn in its dragging look until the chip
    closes, as `object-transform-refinements` says).
    The double-click route keeps its Shift pivot. Given an entry key that cannot
    act, then nothing is written, the tool and the selection stay as they are and
    a one-line hint chip shows for 2 s: "Select an object first" (nothing
    selected, or M and K outside the Select tool), "Select one object to type a
    value" (several objects selected), "Skew works on paths only" (K or Shift+K
    with one selected non-path).
60. Given Escape, then the key repeat of a held Escape does nothing: one physical
    press performs at most one step of criterion 42 (holding Escape in the Node
    tool clears the node selection once and does not go on to leave the tool).
61. Given Delete or Backspace, then the gate of criterion 55 applies and the effect
    is otherwise as today: in the Select tool the selection is deleted, in the
    Node tool the selected nodes, in the Pen tool nothing. During any drag
    nothing is deleted.
62. Given the tool rail, then its tooltips name the key, and the Escape step
    where the plain letter acts on the selection: "Rectangle tool (R)", or
    "Rectangle tool (Esc, R)" while the Select tool has an object selected. The
    tooltips of the Pen, Node, Ellipse and Polygon/Star tools are unchanged; the
    Select tool's reads "Select tool (S or Esc)". The hint chips of criterion 24
    name the keys.

## Design experiment, decided (item 3)

The customer asked to see the selection box dashed. The `ux-engineer` produced
screenshots of five variants (`specs/edit-interaction-polish/ux/`, notes in
`dashed-notes.md`; V0 solid, V1 4 / 3, V2 6 / 3, V3 2 / 2, V4 corner ticks) and
recommended keeping solid or taking V2. The customer decided on 2026-10-06:
"inkscape hat etwa 3 on 3 off, also richtung V1. lightburn hat deutlich laengere
ons und da bewegt sich die strichelung. finde ich jetzt erstmal zu unruhig und
unnoetig. ich wuerde sagen, wir gehen mit V1. die skew linie sieht tatsaechlich
dann zu aehnlich aus, die wuerde ich auf V3 aendern." Reading: the selection box
becomes dashed 4 on / 3 off (V1), static, no animation; the skew fixed-line
guide, which would look like the box, becomes 2 on / 2 off (V3). See criteria
63 to 68. Findings of the experiment that went into the criteria: closed corners
(criterion 64), the pixel-aligned line (criterion 65, proposed by me as small
and default yes), the box over the object's outline (criterion 67), and the
skew guide (criterion 68).

## Out of scope

- Marching ants or any animated box (the customer found LightBurn's moving
  dashes too restless); a dashed hover box, a dashed marquee box.
- Duplicate in place (Ctrl+D), copy and paste, Alt-drag, a copy route for the
  typed move other than the Ctrl at the second press and the Copy check of
  criterion 23 (Proposal), copies from the Node tool.
- Typed move for several selected objects (no centre handle for them), typed
  move from the Node tool or a Properties-panel form, snapping to grid, objects
  or guides, a nine-point reference grid for the absolute position (LightBurn).
- Move by arrow keys (UX proposal: Arrow = 1 mm, Shift+Arrow = 10 mm for the
  selection in the Select tool and nodes in the Node tool, behind the same gate
  as criterion 55; offered as a small follow-up story, the precise step being
  the typed move) and the `?` "Keyboard" overlay (its own story, built from
  the key table in `docs/design-system.md`). Reserved and not built: Ctrl+A,
  Ctrl+D, Ctrl+Z, Ctrl+Y, Ctrl+K (combine, `0006`), H and V (flip).
- An "Edge to axis" button in the polygon group of the Select bar (turn a
  polygon by the smallest angle that puts an edge on an axis; UX optional).
  Decided out of scope, 2026-10-06: typing the angle and the Ctrl stops of
  criterion 7 cover it.
- A Ctrl link or aspect-lock switch for the S key's chip (Ctrl+S is gated); a
  typed size that uses the opposite corner from the keyboard (the double-click
  route does it).
- Storing a skew, or any skew of primitives (`object-transform-refinements`
  decision P1 stands).
- Any other angle behaviour of polygon and star creation (the press-centre,
  pointer-vertex rule stays); a default size or angle for a click without a drag.
- Re-Join or undo of Split; `undo-redo` is its own slice.
- Escape closing dialogs or menus, Escape in the browser build outside the
  canvas, a different key for returning to the Select tool.
- A separate option to lock or copy by a toggle button instead of a held key.
- A stronger hover-box colour (`--accent-hover` at 20% is 1.1:1 against the
  canvas; a separate decision if the customer cares about hover feedback).

## Questions for the customer

Each open question has a default; none blocks the work. The lead sends them in
one German message. Resolved questions stay for the record.

1. **Zero angle of a polygon or star (criteria 1 to 3).** *Resolved 2026-10-06:
   (b), the first vertex points right.* The customer: the first vertex pointing
   right is good; the real problem is that the angle is lost and no edge can be
   aligned to an axis. So the shown and typed angle is the real orientation,
   typing 0 restores it, and Ctrl snaps absolutely (criteria 1, 7). (The earlier
   default was (a), first vertex up.)
2. **How the angle becomes deterministic while creating (criterion 4).**
   *Resolved 2026-10-06: (a), hold Ctrl.* The same key and table as the rotate
   handle, and with the answer to question 1 the Ctrl snap of a later rotate is
   absolute as well (criterion 7).
3. **What "absolute" means in the typed move (criterion 21).** (a) The top-left
   corner of the object's axis-aligned bounds, in document millimetres from the
   document's top-left corner, X right, Y down (default, recommended: it is
   what the rulers show and what a machine's work area measures from, and it
   works for a rotated object); (b) the object's centre, i.e. where the centre
   handle sits (it is what you drag); (c) a choice of nine anchor points later.
   The second part of this question, the typed move only being available
   while the centre handle is drawn, is closed: the key M opens it for an
   object of any size (criteria 17 and 56). If unanswered: (a).
4. **How "sticky" the axis lock is (criterion 30).** *Resolved 2026-10-06: the
   axis is chosen again on every move event, as in Inkscape, no latch* (the
   earlier option (b)). "Wenn man am anfang mal verreisst, kann man einfach
   wieder naeher an die achse gehen und es springt um." The origin axes are
   subtly visible (criterion 27).
5. **Modifier keys held before the press (criteria 29, 37, 38).** *Resolved
   2026-10-06: Shift and Ctrl may be held before the press* (the earlier option
   (b)), with one exception that keeps the old Shift-click: a Shift press
   released without leaving the dead zone still toggles the object in the
   selection (criterion 29). The copy is marked with a plus badge and the lock
   with a lock badge; both go away in the frame the key is released
   (criteria 27, 33).
6. **After Split (criteria 50 to 52).** *Resolved 2026-10-06: (a), one of the
   two nodes is selected* ("just select one"), default the new second one.
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
   press of the typed move, and the Copy check, make a copy (criterion 23,
   Should); the hover box stays solid (criterion 66); the selection and hover
   box line is pixel-aligned so that it is never drawn at 50% (criterion 65,
   Should); for a closed path the node selected after Split is the copy that
   keeps the outgoing handle (criterion 50).
9. **Keyboard scheme and modifier indicators.** *Resolved 2026-10-06.* The
   customer approved both as the `ux-engineer` proposed ("tastenkuerzel beim
   select und anzeige beim verschieben und gut so"): R and S act on the
   selection in the Select tool, so the Rectangle tool then needs Escape first
   (criterion 54), and the plus and lock badges and the origin axes of criteria
   27 and 33. The fallback (tool letters unchanged, the four entries on
   Shift+M, R, S, K) is no longer needed.
   *Amended 2026-10-07 after testing PR 1:* the customer: "wenn man einfach S
   drückt soll es zentrisch skalieren". The typed size of the key S scales about
   the box centre; the double-click on a resize handle keeps the drag's fixed
   point (criteria 57, 57a). A criterion 57 amendment, no new scope.

10. **Architect flags, resolved 2026-10-06 on the architect's defaults**
   (`adrs.md`, "Flagged to the PO"; no customer answer needed):
   1. Criterion 21 measures the tight bounds of the drawn outline (new
      `object_outline_bounds`), not `object_bounds`; a star's absolute X, Y is
      the top-left of the box around its vertices. Applied in criterion 21.
   2. Criterion 64 keeps V1 (about 4 on / 3 off) as the nominal pattern; gap 2
      px and dash 2.7 to 4 px on edges with no exact fit, so corners always
      close. Applied in criterion 64.
   3. Criterion 41: only the live preview has a budget (8 ms); the commit is
      measured and reported, no budget. Applied in criterion 41.
   4. Criterion 59: only the other side rotate handles are hidden while a chip
      is open. Applied in criterion 59 and in the UX notes.
   5. A typed copy (criterion 23) selects the copy, as criterion 35 does for a
      dragged copy. Applied in criterion 23.
   6. Two minus signs: the create-angle readout uses ASCII "-15°" (the angle
      entry's own format), the move readout uses U+2212 (criterion 27); the
      number parser also accepts U+2212 so a value copied from the move
      readout parses. Both stay as written.
   7. Criteria 37 and 42 name the marquee and lasso states of
      `advanced-selection`. Until that spec ships they are conditionals of it;
      this feature implements the states that exist, and `advanced-selection`
      honours them when built.
   8. Part A leaves a freshly created polygon or star's selection box
      axis-aligned (the stored frame angle is not folded into `rotation`).
      Criterion 1 holds. A box that turns with the shape at creation needs a
      file-format change and is a separate story.

## UX notes

(Filled in by the `ux-engineer` before Ready.) Open points for them: placement of
the move chip, which opens at the box centre and not at a handle outside the box
(it must not cover the centre handle or the pivot marker); the Relative |
Absolute control's look and focus order; a cursor cue and a guide for copy and
for a locked axis (criterion 27); the move readout's exact text; how the Escape
cascade is discoverable (nothing is shown today, nothing is proposed); the
design experiment's screenshots and recommendation (Part E, done: `ux/dashed-notes.md`).

### UX notes round 2

`ux-engineer`, 2026-10-06. Covers the customer's answers on axis lock, modifiers
before the press, Split, keyboard shortcuts and the polygon angle. Tokens, rows
and the key table are in `docs/design-system.md` ("Keyboard concept",
"Modifier indicators in a move", "Polygon/star angle", rows "Move axis guide",
"Modifier badge", "Move entry chip", "Polygon/star angle"). The criteria above
already apply these notes (2026-10-06); where wording differs, the criteria win.

#### 1. Keyboard concept

Facts today (`frontend/src/hooks/useEditorSession.ts`, `onKeyDown` on the canvas
container): the tool letters S, B, N, R, E, `*` switch tool on any key event
whose target is not a form control, with no check for a running drag, a pen
path in progress, key repeat, or Ctrl, Alt or Cmd held (Ctrl+R and Ctrl+S also
switch tool). That is the cause of "R while doing something jumps to the
Rectangle tool". Delete likewise runs mid-drag. Arrow keys do nothing.

**The scheme.** One key has one meaning in one visible state; the only state
that changes a letter is "Select tool with an object selected".

| Key | Select tool, object selected | Any other state |
|---|---|---|
| M | typed move (centre handle's chip) | nothing; hint "Select an object first" |
| R | typed angle (top-right corner rotate handle's chip) | Rectangle tool |
| S | typed size about the centre (chip at the bottom-right corner resize handle's position) | Select tool |
| K, Shift+K | typed skew x, skew y (one path; hint otherwise) | nothing; hint |
| B, N, E, `*` | Pen, Node, Ellipse, Polygon/star | the same |
| Delete, Backspace | delete the selection | Node: delete nodes; Pen: nothing |
| Escape | the cascade of criteria 42 to 49 | the same |
| Space | pan while held, never interrupts | the same |

**Why R and S change meaning and the others do not.** M and K are free letters.
R and S are the two tool letters the customer would also expect as transforms
(Illustrator and Blender: R rotate, S scale). Inkscape, Affinity, Figma and
LightBurn have no single-key typed transform at all, so there is no reference
to copy; Blender's G, R, S work only on a selection, which is the model the
customer asked for ("wenn man was markiert hat"). The price: after drawing a
shape the Select tool is active with it selected, so the next R opens the
rotate chip. To draw the next rectangle: Escape (clears the selection), R. The
rail tooltip says so while it applies ("Rectangle tool (Esc, R)"), and one
extra Escape is the same cost as the Shift variant. Rejected: Shift+letter for
tools (changes four accepted shortcuts and clashes with Shift as a modifier);
new tool letters (breaks the Inkscape parity earlier slices chose on purpose).

**Confirmed by the customer, 2026-10-06:** R and S act on the selection in the
Select tool; to pick the Rectangle tool then, press Escape first (question 9).
The fallback, tool letters unchanged and the four entries on Shift+M, Shift+R,
Shift+S, Shift+K, is not needed.

**No shortcut during an operation or while typing (the bug fix).** One gate,
called first by the key handler, ignores a letter or Delete (no effect, no
`preventDefault`) when any of these holds: Ctrl, Cmd or Alt is down (Shift only
for `*` and Shift+K); key repeat or IME composition; focus is in a text field,
select, button, switch or contenteditable, or an entry chip is open; a drag is
in flight in any tool (move, resize, rotate, skew, parameter, marquee, lasso,
create-drag, node or handle drag, pan); the Pen has an unfinished path; Space is
held. Caps Lock changes nothing. Never gated: Escape, Space, Enter (finishes
the Pen path), the modifier keys. A letter pressed while the Pen path is open
does nothing; Enter, a double-click or Escape end it. Where the rail is clicked
during an operation, today's behaviour stays (not changed here).

**The entries reuse the existing chips, placed as the double-click places
them.** M opens the centre handle's chip, R the top-right corner rotate
handle's, S the bottom-right corner resize handle's, K the top skew handle's,
Shift+K the right skew handle's. The chip appears where that handle is (or would
be, if it is hidden by size: this closes criterion 17's gap for every object),
the handle shows its dragging look, Enter commits, Escape cancels, focus returns
to the canvas. The key route never uses the Shift pivot (it does not read Shift
at all, apart from Shift+K meaning skew y); the double-click route keeps it.
*Amended 2026-10-07 (customer: "wenn man einfach S drückt soll es zentrisch
skalieren"):* S is the one entry whose fixed point is not that of its handle. No
handle was chosen, so the typed size scales about the box centre, as a drag
with Shift held would; the double-click on a resize handle keeps the drag's
fixed point (criterion 57a). Decided 2026-10-07 (UX review): the S chip opens
by the centre, placed as the move chip is, with no handle in its dragging look
and the pivot marker at the centre in full `--accent`.
While a chip is open the Shift-revealed side rotate handles other than the open
chip's own are hidden, so the Shift+K chip never sits on one. With several objects selected, or K on a
non-path, the key shows a one-line hint-chip message for 2 s and writes
nothing ("Select one object to type a value", "Skew works on paths only").
`R, 0, Enter` is the keyboard way to stand a polygon or star back to its zero
orientation.

**Discoverability.** Rail tooltips already name the letter; they gain the Escape
step while a selection makes the plain letter act on the selection, and the
Select tooltip reads "Select tool (S or Esc)". The handle hint chip names the key
next to double-click ("Double-click or M: type an offset"; or S, R, K). The
table in the design system lists every key. A `?` overlay ("Keyboard") is
proposed as its own story; it would be built from that table. Not part of this
one (scope).

**Proposal, flagged as scope growth: arrow-key nudge.** Arrow = 1 mm, Shift+Arrow
= 10 mm for the selection in the Select tool (and nodes in the Node tool),
same gate, key repeat allowed for arrows only, one document write per press
until `undo-redo` can coalesce a held run. The precise step is the typed move
("M, 5, Enter"). The spec lists "move by arrow keys" as out of scope; the
default is to keep it out and offer it as a small follow-up story.

**Reserved, not built:** Ctrl+A, Ctrl+D, Ctrl+Z, Ctrl+Y, Ctrl+K (combine, `0006`),
H and V (flip). A new tool letter must not be M, K, H or V.

#### 2. Move chip placement and controls

The move chip opens at the box centre, not at a handle outside it: top left 16
px right of and 16 px below the centre glyph, flipped left or up with the
readout's rule and clamped; it never covers the glyph. The centre handle stays
drawn in its dragging look while it is open, no pivot marker. Layout: row 1 the
fields "X" and "Y" (100 px each), row 2 the mode control, a two-segment pill
"Relative | Absolute" (active segment filled `--toolbar-icon-active-bg`, white
text). Semantics: one button with `role="switch"`, `aria-checked` = Absolute,
accessible name "Absolute position" (replaces criterion 18's "Position mode",
since a switch needs a name that is true when on); Space and the arrow keys
flip it. Tab: X, Y, mode, and (Should) a "Copy" check, off on every open.
A Copy check is the keyboard and discoverable route to criterion 23's typed
copy: Ctrl at the second click opens the chip with it checked, and M cannot
carry a Ctrl. Without the check, criterion 23 is reachable by mouse only.

#### 3. Modifier indicators for move and copy

Ctrl and Shift may be held before the press, and may be pressed or released at
any moment of the drag. The state of the current frame decides, and every
indicator appears and disappears in the frame of the key event, also with the
pointer at rest and also after Escape (a held Ctrl keeps predicting).

**Copy (Ctrl).** A **plus badge** by the pointer: 16 px, solid `--accent`, white
plus, centre 16 px left of and 16 px below the hotspot (clear of the arrow,
move and crosshair cursors and on the opposite side from the readout chip). It
shows wherever a press with Ctrl would start a move (it uses the press's own hit
test, so it never lies: not on empty canvas, where Ctrl belongs to the
marquee), and during a copy drag. The badge is DOM and follows the key event
directly; a CSS cursor would update only on the next mouse event in some
engines, so no `copy` cursor is used. Also: "Copy" at the end of the readout
("Δ 12.5, −3.0 mm Copy"), and the blue outline at the copy's position, as for
every move. A move and a copy otherwise look the same in blue-new/black-old
(the original stays in both), so the discriminator is: **in a copy the selection
box and handles stay on the originals, the blue outline travels alone; in a
move they travel with the blue outline.** Pressing Ctrl mid-drag makes the box
jump back, releasing it makes it follow, in the same frame.

**Axis lock (Shift).** While a move drag is past the dead zone with Shift down:
(a) the **origin axes**: two lines through the selection's start centre (the
centre handle's position at the press), one horizontal, one vertical, 1 px
solid, full viewport extent. The axis the move is locked to is
`--axis-guide` (`--accent` at 50%), the other `--axis-guide-idle`
(`--accent` at 20%, the hover tone). Faint on purpose (about 1.7:1 on the canvas;
informational only, the motion, badge and readout say the same). Not dashed.
(b) a **lock badge** next to the copy badge: white, 1.5 px `--accent` ring, a
horizontal or vertical double arrow showing the current axis. (c) the readout
shows the locked delta ("Δ 30.0, 0.0 mm"). The axis is re-chosen on every
pointer event: the line the pointer is nearer to (the larger of |Dx| and |Dy|)
wins, an exact tie keeps the previous axis (x if none); no latch, no
hysteresis, the object jumps when the pointer crosses the diagonal. The lock
badge is not shown before the press: before it, Shift means "add to selection"
and an axis claim would be wrong; the lock still engages on the first frame
past the dead zone, so no unlocked frame is ever drawn.

Draw order, bottom to top: artwork, axis lines, blue outline, selection box,
handles, then DOM (readout, badges). In copy mode the centre glyph of the
original sits on the axes' crossing and covers it.

#### 4. Clashes, and Shift or Ctrl at the press

- **Readout chip** (up and to the right of the pointer) and **badges** (left
  and below) are on opposite sides; if the readout flips below near the top
  edge and the badges mirror to the right, the badges drop 28 px.
- **Centre handle glyph and the parameter handles:** the glyph is drawn in its
  dragging look (at the original in copy mode, travelling in a move); parameter
  handles are not drawn during a move (unchanged); the axis lines pass under
  the glyph.
- **Hint chip:** it hides on any key, so pressing Ctrl or Shift removes it.
- **Shift at the press** (slice 4 toggles the object): the selection does not
  change at the press. Released inside the dead zone it toggles the object, as
  before. Once the drag leaves the dead zone it is an axis-locked move of the
  selection; an unselected pressed object joins the selection at that moment.
  So a Shift-click behaves as before, and a small drag with Shift down locks.
  The toggle's timing moves from press to release; nothing else changes.
- **Ctrl at the press** on an object's outline, an unselected object or the
  centre handle: a move that becomes a copy if Ctrl is down at the release.
  `advanced-selection`'s Ctrl-remove marquee starts on empty canvas only, so
  there is no clash. A press inside the sole selected box that is not on the
  object (empty interior) keeps whichever meaning `advanced-selection` gives it
  when built; the badge follows the same hit test.
- **The centre handle** is the explicit grip for "hold the key first": Shift or
  Ctrl at a press on it is always a move modifier and never toggles the
  selection (criterion 38 stands, with the rule above).

#### 5. Split (customer: "just select one")

One node, as criterion 50 says: the one a press at the shared position hits, so
that what is drawn highlighted is what the next drag moves. UX addition: the
selected node's glyph is drawn above the unselected one at the same position
(today both overlap at 14 px and the accent fill would be hidden). Better still,
and for the architect, a hit-test tie goes to the selected node.

#### 6. Polygon and star angle

The lost angle comes from two facts: the creation angle sits in the shape's own
frame, and the rotation register the Select tool reads stays 0; and Ctrl
rotate snaps relative to the start angle, so a shape created at 78.7° never
reaches a clean angle. Decisions:

1. The angle shown and typed in the Select tool is the real orientation:
   the clockwise angle of the first outer vertex (a star's first tip) from
   straight right, -180° to 180°. One number in the create readout, the rotate
   drag readout, the angle chip prefill and the oriented box. **Zero is "first
   vertex points right"**, the customer's "is fine", and it needs no change of
   what is stored (the first-vertex angle `atan2` already is this value). It
   is applied in criteria 1 and 7 and answers question 1.
   Dragging right shows 0°, down 90°, up -90°.
2. Typing 0 restores the first vertex pointing right (keyboard: `R, 0,
   Enter`), at any time.
3. Ctrl while rotating a polygon or star snaps the **shown** angle to the
   15°/22.5° table (absolute), so created or typed angles and stops agree.
   Other kinds keep the relative rule of refinements 33. This supersedes
   criterion 7's "S plus a stop" for polygons and stars.
4. For an N-gon an edge lies on an axis at 0° when N is odd or N is 2 more than
   a multiple of 4 (6, 10, ...), else at 180/N° (4: 45°, 8: 22.5°, 12: 15°).
   That is how a maker aligns an edge by typing. (The product owner corrected
   this note: it said "a multiple of 6", which is wrong for 12.) **Optional,
   scope:** a button "Edge to axis" in the polygon group of the Select bar
   (polygons only, not stars) that turns it by the smallest angle that puts an
   edge on an axis. Decided out of scope, 2026-10-06; typing and Ctrl stops
   cover it.

## Links

Requirements: R-EDIT-012, R-EDIT-014 (`docs/requirements.md`)
Builds on: `specs/unified-object-editing/`, `specs/object-transform-refinements/`,
`specs/shape-creation-from-center/`, `specs/0002-path-node-editing/`,
`specs/0006-path-merge-split-and-node-types/`, `specs/advanced-selection/`,
`docs/design-system.md`
PR:
