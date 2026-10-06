# Unified object editing: one Select tool for every object and its own handles

Status: Ready
Priority: Must
Origin: Customer

## User value

As a maker I want one way to edit an object, the Select tool, for a path, a
rectangle, an ellipse and a polygon or star alike, so that I never have to
guess which tool I must be in to change a corner radius, rotate a shape or
type an exact size, and I always see how an edit will turn out (new shape in
blue) next to how it was (old shape in black).

The customer's words after testing slice 5 and the transform refinements
(translated, 2026-10-06): "It works and I would merge it, but selecting an
object and the general transformations via the Select tool must be unified,
it feels confusing. I found corner rounding for rectangles; it exists only
in the 'rectangle mode'. I would like to be able to give individual corners
different radii. Editing primitives differs a bit from the Select tool. In
the primitive editing mode I like that I see in blue how it will become and
in black how it was; we take that over to the Select mode. And the primitive
editing mode also needs all the features of the Select edit (rotation,
double-click direct entry, ...); there should be only ONE edit mode. If it
is a rectangle, I also want to be able to see all handles. This is next:
unify the primitive edit mode and all transforms."

This spec is the unification. The two features the customer mentioned in the
same breath are separate specs that build on it:
`specs/rectangle-corner-radii/` (a radius per corner) and
`specs/ellipse-arcs-and-shaping/` (arcs, node shaping). Unification comes
first, as the customer asked.

**Field reference.** Inkscape separates the Selector (scale, rotate, skew) from
the Rectangle, Ellipse and Star tools (corner radii, arc angles, star
parameters); each tool has its own handles and its own toolbar. That is the
split the customer finds confusing, so we do not follow it. Illustrator's
Live Shapes and Figma show a selected rectangle's corner-radius handle
together with the bounding-box handles in one selection tool; Illustrator
also lets you double-click a handle for numbers. That is the model here. Where
we do better: the blue-new/black-old preview applies to every edit in the one
mode (move, resize, rotate, radius, ratio), the pivot is always shown, and a
typed value uses the same reference point as the drag (`specs/object-
transform-refinements/`).

## What exists today

Reference state: `main` after PR #29 (`specs/0005-object-transform/`), with
PR #35 (`specs/object-transform-refinements/`) assumed merged; items that
arrive only with #35 are marked (#35). The code facts are from
`vecmanf-ui-core/src/{select_tool,rectangle_tool,ellipse_tool,poly_star_tool,
shape_tool_common,shape_hit_test,handle_layout}.rs` and the `Session` in
`vecmanf-editor-wasm/src/session/`.

| | Select tool | Rectangle tool | Ellipse tool | Polygon/Star tool |
|---|---|---|---|---|
| Selectable objects | every kind; click on outline, Shift toggle, marquee, lasso, Alt-click cycle (advanced-selection) | rectangles only | ellipses only | polygons and stars only |
| Press on an object's outline | selects it and starts a move | selects it, no move | same | same |
| Press on empty canvas | clears selection, or starts marquee | starts a create-drag, clears selection first | same | same |
| Move by dragging | yes | no | no | no |
| Delete key | yes | no | no | no |
| Selection box | oriented box on every selected object, hover box | oriented box, own kind only | same | same |
| Resize handles | 8 on the box (corners and edge midpoints); polygon and star: 4 corners | 8 on the box | 8 on the box | 4 at the N/E/S/W points of the outer circle |
| Parameter handles | none | corner radius: one draggable handle on the NE diagonal, plus 3 display-only echoes once the radius is above 0 | none | star only: one inner-radius handle at the first inner vertex |
| Rotate, centre move, skew (paths), pivot marker, numeric entry by double-click, 22.5° snap | yes (#35: corner and Shift-side rotate handles, centre handle, skew on paths, typed angle and size, 22.5° stops) | none | none | none |
| Resize modifiers | Shift: about the centre; Ctrl: proportional | none | none | none |
| Resize and the corner radius | the radius scales by sqrt(sx·sy) (0005 criterion 9) | the radius keeps its absolute length (0003 criterion 3) | n/a | n/a |
| Preview while a handle drags | the object itself is redrawn at the new geometry in its normal black style; the old geometry is gone; box and handles follow | the committed shape stays black and unchanged; the new shape is drawn over it as a 1.5 px hollow blue (`--accent`) outline; nothing is written until release | same | same |
| Numeric readout during a handle drag | size (W × H, or r) and angle | only on a create-drag; none on resize, radius or ratio drags | same | same |
| Tool bar | "Scale stroke width" switch only | "Remove rounding", "Object to path" | "Object to path" | mode, Points, Ratio, "Object to path" |
| Double-click | on a path: Node tool; on a primitive: that primitive's own shape tool (0004 criteria 22, 23; #35: also anywhere inside the box and on the centre handle) | n/a | n/a | n/a |

Gaps that follow from this table, as the customer experiences them:

- A rectangle can only be rounded in the Rectangle tool; the Select tool shows
  no radius handle. The same holds for a star's inner radius.
- A primitive cannot be rotated, typed into, or resized from the centre while
  its own tool is active, and cannot be moved or deleted there.
- Handles differ per mode: a polygon has N/E/S/W resize handles in its tool
  and four corner handles in the Select tool; a rectangle's resize keeps the
  radius in one mode and scales it in the other.
- "Object to path" is reachable only from a shape tool's bar, not from the
  Select tool where the maker usually has the object selected.
- The blue-new/black-old preview exists only in the shape tools. In the Select
  tool the object itself moves, so the maker cannot compare with the original.

## Target

One edit mode: the Select tool, for every object kind.

- A selected primitive shows the transform handles (resize, rotate, centre,
  and for paths skew) and, at the same time, its own parameter handles:
  corner radius on a rectangle, inner radius on a star. Nothing needs a mode
  switch.
- Every drag in the Select tool previews the result in blue over the
  unchanged old shape in black.
- Every refinements feature works for primitives as it does for paths.
- The Rectangle, Ellipse and Polygon/Star tools only create. They have no
  handles, no selection and no editing role.
- The Node tool stays the place to edit a path's nodes. A path's own
  "parameters" are its nodes; showing them next to the transform handles would
  mean hundreds of glyphs on one box. This is not part of the unification.

Terms: **transform handles** are the Select tool's resize, rotate, skew and
centre handles. **Parameter handles** change a primitive's own parameter: the
corner radius, the star's inner radius (and later an ellipse's arc angles).

## Changes to accepted behaviour

Decided here by the `product-owner` as proposals; the customer confirms them
by answering the questions at the end. The lead updates the owning specs when
this one is accepted (this spec does not edit them).

1. A double-click on a primitive no longer switches to its own tool
   (`specs/0004-canvas-navigation-and-selection/` criterion 23; the
   primitive part of `specs/object-transform-refinements/` criterion 3 and of
   `specs/0005-object-transform/` criterion 25). Criteria 32 to 34.
2. The shape tools stop selecting and editing. `specs/0003-primitive-shapes/`
   criteria 3, 4 (handle in the tool), 9, 13, 14, 15 and the "tool mismatch"
   UX rule move to the Select tool (criteria 1 to 9, 20 to 22, 21a). Criterion 3
   of that spec (a tool resize keeps the radius) ends with the tool's editing
   role; the Select tool's rule is the only resize rule, and it is no longer
   always `0005` criterion 9: the customer asked (2026-10-06) for a switch
   "Scale corner radius", off by default, so a resize keeps the radius's
   absolute size unless the switch is on (criterion 23,
   `specs/rectangle-corner-radii/` criteria 12 and 15). This is a customer-
   visible change from slice 5, where the radius always scaled.
3. `specs/shape-creation-from-center/` criterion 16 (a press on an existing
   shape of the tool's kind selects or handle-drags) and the part of
   criterion 17 about handle drags under the tool's own tool are replaced by
   criterion 25 below. Its Shift and Ctrl creation rules stay as they are.
4. After a create-drag the Select tool becomes active (criterion 28), unless
   the customer chooses otherwise (question 2).
5. Amendments after the customer tested PR #38 (2026-10-06, both bugs, not new
   scope): choosing a creation tool clears the selection and draws no selection
   box under it (criterion 27 reversed); no hover highlight of other objects
   during any Select-tool drag (criterion 15a).

## Acceptance criteria

All criteria concern the Select tool with exactly one primitive selected
unless they say otherwise. "Handles drawn" follows the size tiers of criterion
7 (the transform-handle tiers of `specs/object-transform-refinements/` plus
the parameter-handle threshold).

### Parameter handles next to the transform handles

1. Given one selected rectangle, then the Select tool shows its transform
   handles (all rules of `specs/object-transform-refinements/`) and four
   corner-radius handles, one at each corner, at the same time (from the size
   in criterion 7 up). Given one
   selected star, then it shows its transform handles and one inner-radius
   handle at the star's first inner vertex. Given a polygon or an ellipse,
   then it shows its transform handles and no parameter handle (an ellipse's
   arc handles, and the curve handle of an ellipse, polygon and star, are
   `specs/ellipse-arcs-and-shaping/`). No tool or mode switch
   is needed to see or use any of them.
2. Given a rectangle at or above the size of criterion 7, then its four radius
   handles are drawn at radius 0 as well (the maker discovers rounding by
   finding them), at the position of the rule below, and all four are
   draggable. Dragging any of them changes the rectangle's one radius and all
   four corners follow; the other three handles move in step. The radius is
   clamped to half the shorter side (`0003` criterion 5). The handle stays
   under the pointer until a limit is reached (zero or the clamp). The radius
   is exactly 0 when the pointer's projection on the corner's inward diagonal
   is at or beyond the handle's zero-radius position, which is 15 screen
   pixels from the corner, not the corner itself (`0003` criterion 6, adapted
   to this position). Position rule: on the diagonal, at `p = 15 + ρ·L(s)`
   pixels from the corner, where `s` is the box's shorter side in pixels, `ρ`
   is the effective radius divided by `s/2` (0 to 1) and
   `L(s) = (s − 14)/√2 − 15`; one pixel of pointer movement along the diagonal
   changes the radius by `G(s) = (s/2)/L(s)` pixels' worth of length. The
   reason and the numbers are in the UX notes (section 1) and
   `docs/design-system.md`; the observable result is criterion 8. The rule is
   the one `specs/0003-primitive-shapes/` criteria 4 to 6 define for the
   Rectangle tool today, with this position map; independent radii per corner
   are `specs/rectangle-corner-radii/`.
3. Given a star, when the maker drags its inner-radius handle, then the inner
   radius changes and the outer radius stays fixed, the ratio is limited to
   0.01 to 0.99, and the shape updates live (`0003` criterion 14).
4. Given a rotated primitive, then its parameter handles are drawn at the
   positions its rotation implies, and a parameter-handle drag is measured
   along the primitive's own axes (the rule of `0005` criterion 25, now
   applied in the Select tool).
5. Given any press on the canvas, then one nearest-centre hit test over all
   drawn handles (transform and parameter handles together) decides which
   handle is hit, as `specs/object-transform-refinements/` criterion 9
   defines for transform handles. A parameter handle's hit radius is 12
   screen pixels (design-system row "Parameter handle hit-test radius"), not
   the 16 px of the resize handles. Over a parameter handle the cursor is the
   built-in pointer, hovering and dragging, whatever the modifiers. On an exact
   tie the order
   is parameter handle, resize, skew, rotate. A press inside the box that is
   on no handle moves the object; the centre handle is not hit-tested.
6. Given a handle that is not drawn (criterion 7), then it has no hit area and
   no hover or cursor state, so a press where it would be moves the object.
   One exception: the 4 edge resize handles of a box whose shorter side is
   under 24 px are not drawn (criterion 7) but remain hit-testable, with
   hover, cursor and drag, as `specs/object-transform-refinements/`
   criterion 9 decides. Parameter handles, the centre handle and the side
   rotate handles have no such exception.
7. Given the box's shorter side `s` in screen pixels, then the handles drawn on
   one selected primitive follow these tiers: under 24, the 4 corner resize
   and 4 corner rotate handles (Shift: the 4 side rotate handles); 24 to 47,
   plus the 4 edge resize handles; 48 to 71, plus the centre move handle;
   72 and up (the parameter-handle threshold T), plus the rectangle's 4 radius
   handles or the star's inner-radius handle (a polygon and an ellipse add
   none in this spec). Polygon and star have no edge resize handles. When a
   box shrinks, the parameter handles disappear first, then the centre handle,
   then the edge resize handles; corner resize and corner rotate handles are
   never hidden. There is no hysteresis. Parameter handles are not drawn
   while the same object is being moved, resized, rotated or skewed by drag,
   and during a parameter drag the transform handles stay drawn. Below T the
   primitive is still fully editable by zooming in, by the transform handles
   and by the bar controls (criteria 21 and 21a).
8. Given any size `s` of 72 px or more and any radius or ratio, then no two
   drawn glyphs overlap or come closer than 4 screen pixels, with one
   exception: the centre handle is not drawn while any parameter handle is
   within 20 px of the box centre, nor while a parameter handle is being
   dragged (the centre handle has no hit area, so hiding it loses no
   function). The worst cases are tested: four radius handles at the largest
   radius, and a star whose point count is a multiple of 4 at ratio 0.99. A
   parameter handle is distinguishable from a transform handle by silhouette
   at a glance, not only by position or size: it is the only round glyph with
   a centre dot (a "knob", design-system row "Parameter handle"); resize,
   centre, rotate and skew handles are not. The old N/E/S/W outer-circle
   handles of the polygon and star tool are not drawn anywhere.
9. Given a press and release on a parameter handle with less than 3 screen
   pixels of movement, then nothing is written (the dead zone of
   `specs/object-transform-refinements/`). Given Escape during a parameter-
   handle drag, then nothing is written and the preview disappears.

### Blue new, black old, in every edit

10. Given a Select-tool drag that changes geometry (move, resize, rotate, skew,
    corner radius, inner radius), then for the whole drag every affected
    object is drawn twice: its committed geometry unchanged in its own
    rendering with its own stroke and colour (black for the default stroke; a
    red stroke stays red), and the geometry the release would commit as a
    1.5 px hollow outline of constant screen width in `--preview-new`
    (`--accent`, blue), solid, no fill preview (a filled object keeps its fill
    and shows no new fill), drawn on top. The selection box, the handles, the
    pivot marker and the readout follow the new geometry. Today's Select-tool
    preview, which redraws the object itself at the new place, is replaced by
    this. The "Points" and "Ratio" sliders of criterion 21 use the same
    preview during a slider drag; typed values (entry chip, bar fields) do not
    preview and commit on Enter.
11. Given a drag of a multi-object selection (a move), then every selected
    object shows both renderings.
12. Given the pointer is released, then exactly one commit is made, the blue
    outline disappears and the object renders in its normal style at the new
    geometry. Given Escape, or a drag that returns to its start point, or a
    press under the 3 px dead zone, then nothing is written (in particular a move
    dragged back to its start writes no zero-offset commit and leaves every
    stored value untouched), no blue outline appears or remains, and the old
    geometry is unchanged.
13. Given a drag in progress, then the blue outline is the exact geometry a
    release at that moment, with the same pointer position and modifier keys,
    would commit: same function, same inputs (`specs/object-transform-
    refinements/` criterion 40 for every kind).
14. Given a drag whose pointer holds still while Shift or Ctrl is pressed or
    released, then the blue outline updates on the next frame (the pivot
    rules of the refinements spec).
15. Given a move drag of 200 selected objects (100 paths with 50 nodes each,
    100 rectangles), then the blue preview follows the pointer at 50 frames
    per second or better on the reference desktop the architect names. The
    blue outline is never stored: saving during a drag is not possible, and a
    saved file never contains preview data.
    Not in this spec: the Node tool keeps its own node-drag preview
    (question 6).
15a. Given any Select-tool drag in progress (move, resize, rotate, skew, a
    parameter-handle drag, and also a marquee or lasso drag), then for the
    whole drag no object shows a hover highlight, hover box, hint chip or
    hover cursor, other than the object and the handle being edited, which
    keep the look their drag gives them (the dragged handle its "dragging"
    ground). The hover state is recomputed from the pointer position at the
    release, so an object under the pointer after the release is highlighted
    from the next frame. A press on empty canvas and a press-and-release inside
    the dead zone also show no hover change between the press and the release.
    (Added 2026-10-06 after the customer tested PR #38, numbered 15a so the
    numbering of the other criteria stays stable.)

### The refinements features for primitives

16. Given a rectangle, ellipse, polygon or star, then every Part A criterion of
    `specs/object-transform-refinements/` holds for it exactly as for a path:
    centre move handle, corner rotate handles and Shift side rotate handles,
    the pivot rule, Ctrl snapping to the multiples of 15° and 22.5°, double-
    click typed angle, double-click typed size (W and H for rectangle and
    ellipse, r for polygon and star), the 3 px dead zone, the live readouts
    and the pivot marker. The polygon and star differences stay (corner
    resize handles only, always uniform, always about the centre).
17. Given a rectangle, ellipse, polygon or star, then it shows no skew handles
    and cannot be skewed (`specs/object-transform-refinements/` criteria 50
    and 51; the decision P1). Whether primitives should become skewable is
    question 5, because it would change the document model.
18. Given a double-click on a rectangle's radius handle, then a field labelled
    "r" (accessible name "Corner radius", unit mm) opens next to it, pre-filled with
    the current effective radius, text selected. Enter writes the value to the
    one radius (all four corners) in one commit. Zero is valid (sharp corners).
    A value above half the shorter side is limited to that value and the
    limited value is what is written. An empty or non-numeric value keeps the
    field open and marked invalid ("Enter a number"), a negative value likewise
    ("Must be 0 or more"), and nothing is written. An unedited Enter,
    Escape, a click elsewhere, a tool switch and a selection change close the
    field and write nothing, and the closing press is not swallowed. All of it
    is the behaviour of criteria 19 to 21 and 30 to 31 of `specs/object-
    transform-refinements/`; no double-click here switches tools.
19. Given a double-click on a star's inner-radius handle, then a field labelled
    "ratio" (accessible name "Inner ratio") opens, pre-filled with the current
    ratio (two decimals), with the same rules as criterion 18; a value outside
    0.01 to 0.99 is invalid ("Must be 0.01 to 0.99"). The polygon and star's
    outer-radius entry of the refinements spec is renamed "Outer radius" (its
    visible label stays "r"), so the three accessible names never collide.
20. Given a parameter-handle drag, then a live readout is shown at the pointer
    (same placement as the transform readouts): "r 3.5 mm" for a rectangle's
    radius, "ratio 0.45" for a star. Before this spec the shape tools showed
    no readout for these drags. Hovering a parameter handle for 600 ms shows
    the hint chip of the transform handles: "Corner radius" and
    "Double-click: type a value" on a radius handle, "Inner radius" and
    "Double-click: type a ratio" on a star's handle.

### Controls that move into the Select tool

21. Given the Select tool's top bar, then each kind control follows one rule: it
    is shown when the selection contains at least one object of the kind it
    acts on, it acts on exactly those objects (the others in the selection are
    left alone), and it is enabled when it would change something. It is never
    shown disabled for an unrelated selection. The controls:
    - "Remove rounding": shown when the selection contains a rectangle,
      enabled when at least one selected rectangle has a radius above 0;
      zeroes the radius of every selected rectangle in one commit.
    - "Points" (integer 3 to 1024): shown when the selection contains a
      polygon or a star; acts on the polygons and stars.
    - "Ratio" (0.01 to 0.99): shown when the selection contains a star; acts
      on the stars.

    Changing Points or Ratio updates every acted-on shape live and commits once
    per interaction (a slider drag is one commit, not one per tick; a stepper
    click is one commit), as `0003` criteria 10, 14 and 15 define. The Points
    number field commits typed text on Enter or on blur, and a stepper click
    immediately; each is one commit, and typing does not commit per
    keystroke. A value that is not valid on Enter or blur is not written. If the
    acted-on shapes hold different values, the field is empty with the
    placeholder "Mixed" and a typed value applies to all of them; a slider has
    no thumb while mixed until touched. With a rectangle and a star selected
    the bar shows Radius, Remove rounding, Points and Ratio. The tooltip of
    each control says what it acts on ("Remove rounding of the selected
    rectangles").
21a. Given a selection that contains a rectangle (any size, any zoom), then the
    bar shows a number field "Radius" with the fixed suffix "mm" (accessible
    name "Corner radius"), always enabled, acting on the selected rectangles.
    It shows the effective radius; Enter writes it to every selected
    rectangle in one commit (all four corners). Each rectangle gets
    min(typed value, half of its own shorter side), so with several
    rectangles of different sizes the limit applies per rectangle, not as one
    clamped value for the batch (limit as in criterion 18). Escape or a press elsewhere restores the shown
    value and writes nothing. An empty or non-numeric value marks the field
    invalid ("Enter a number") and writes nothing; a negative value is invalid
    ("Must be 0 or more"). Rectangles holding different radii show the field
    empty with "Mixed". A stored radius larger than the rectangle allows shows
    the effective value and a muted tag "limited" before the unit, and the
    tooltip names the stored value. It is the only route to a radius for a
    rectangle below the threshold of criterion 7 and the keyboard route. Its
    tooltip says the corner handles appear from 72 px across on screen, so
    "zoom in or type a value". Under `specs/rectangle-corner-radii/` it means
    "set all four".
22. Given the Select tool's top bar, then "Object to path" is shown when the
    selection contains at least one rectangle, ellipse, polygon or star,
    always enabled, and acts on exactly those objects, not on paths in the
    same selection; `0003` criteria 17 to 22 hold unchanged (one atomic
    commit, each primitive independently, no implicit conversion anywhere
    else). It is the last control of the bar and has no keyboard shortcut. The
    shape tools' bars no longer show it.
23. Given the Select tool is active, then two switches are the first group of
    the bar, shown and operable with or without a selection, never disabled and
    never dimmed, so that they stay where they are when the selection changes
    (the bar is left-aligned for the Select tool; only the groups to their
    right come and go): "Scale stroke width" (`0005` criterion 30) and "Scale
    corner radius" (customer decision 2026-10-06; off at program start). Both
    are per-session UI state, never written to the project. "Scale corner
    radius" is read at the press that starts a resize drag, and for a typed
    size at the moment the entry opens (a click on the switch closes an open
    entry without writing, `specs/object-transform-refinements/` criterion 31),
    which is the same rule as `specs/rectangle-corner-radii/` criterion 14. It
    decides whether a resize of a rectangle scales its radius by `√(sx·sy)`
    (on) or leaves it at its absolute size (off, the default); the behaviour is
    defined in `specs/rectangle-corner-radii/` criteria 12 and 15, and until
    that spec is built a rectangle's single radius follows the same rule. The
    tooltip of "Scale corner radius" is "Scale corner radius with the object.
    Off: a resize keeps the corner radius." Layout and sizes are in the UX
    notes and `docs/design-system.md`.
24. Given a primitive with a radius, a ratio or a point count set by any of
    criteria 2, 3, 18, 19, 21 or 21a, then the stored fields written are the ones
    the shape tools wrote before (`corner_radius`, `inner_ratio`,
    `point_count`, the frame and `rotation`); nothing new is stored and the
    `format_version` is unchanged.

### Shape tools only create

25. Given the Rectangle, Ellipse or Polygon/Star tool is active, when the maker
    presses and drags anywhere, including on the outline of an existing shape,
    inside a selected object's box or where a handle would be, then a new
    shape is created exactly as `0003` criteria 1, 2, 7, 8, 11, 12 and
    `specs/shape-creation-from-center/` define. A press in a creation tool
    never selects, toggles, moves or handle-drags an existing object, with or
    without Shift, and never shows a handle. A creation tool shows its
    crosshair cursor everywhere, including over existing objects, and no hover
    highlight, hint chip or hit state on existing objects.
26. Given a creation tool is active, when the maker presses and releases without
    moving, then nothing is created and the selection is unchanged (it is empty
    by criterion 27).
27. Given a creation tool (Rectangle, Ellipse or Polygon/Star) becomes active by
    any route (tool rail, `R`, `E`, `*`), then the selection is cleared in the
    same step: nothing is selected, and no selection box, handle, hover
    highlight, hint chip or hit state is drawn for any object for as long as
    that tool is active. Switching back to the Select tool does not restore the
    earlier selection. (Amended 2026-10-06 after the customer tested PR #38:
    this reverses the earlier rule that the selection stayed, with a plain box,
    under a creation tool. The Pen and Node tools are not creation tools in
    this sense and are unchanged.) Given a create-drag commits, then the new
    shape is selected by criterion 28, as before.
28. Given a create-drag commits (default of question 2), then the Select tool
    becomes active in the same step with the new shape as the only selected
    object and its full handle set drawn (`0003` criterion 1, "it becomes the
    selected object"), and the tool rail highlights Select. The `R`, `E` and
    `*` shortcuts and the rail order are unchanged; `S` returns to Select at
    any time.
29. Given the Polygon/Star tool, then its bar keeps the mode toggle, "Points"
    and "Ratio" as settings for the next shape, the two fields carrying the
    muted prefix "New:" so they are not read as acting on the selection: they persist across shapes
    (`0003` criterion 10) and never change a selected shape (that is
    criterion 21). `0003` criterion 15 (live point-count change of the selected
    shape under this tool) now belongs to criterion 21.
30. Given the Rectangle or Ellipse tool is active, then its bar no longer
    contains "Remove rounding" or "Object to path".

### Double-click

31. Given the Select tool and a path, when the maker double-clicks its outline,
    the centre handle or inside its box (not on another handle), then the Node
    tool is activated with that path selected, as before
    (`0004` criterion 22, `specs/object-transform-refinements/` criterion 3).
32. Given the Select tool and a primitive, when the maker double-clicks its
    outline, the centre handle or inside its box (not on a handle), then
    nothing changes in the tool, the document or the selection, and a hint
    chip appears at the pointer for 3 seconds (or until a press, a key or the
    pointer leaving) with three lines: "Drag a handle to edit this shape",
    "Double-click a handle to type a value", "Nodes: Object to path, then
    double-click". The chip is shown on every such double-click, ignores
    pointer events and writes nothing. This replaces `0004` criterion 23. A
    double-click on a skew handle or on empty canvas shows no hint.
33. Given a double-click on any handle of any object, then the numeric entry
    of the refinements spec or of criteria 18 and 19 opens where one is
    defined for that handle; where none is (skew handles), nothing happens;
    in no case does a tool switch happen.
34. Given a double-click on empty canvas, then nothing happens, as before.

### Selection gestures stay consistent

35. Given a press with exactly one primitive selected, then the order is: a hit
    on a drawn handle (criterion 5) starts that handle's drag, Shift or not
    (`specs/object-transform-refinements/` criterion 11, extended to parameter
    handles); else, with Shift held, an outline hit within 4 px is tried
    first and toggles that object in the selection, so Shift-click
    add-to-selection works over a filled shape; else a plain press (no Shift)
    inside the selected box starts a move; else an outline hit within 4 px
    selects, Shift toggles; else the marquee starts. The 4 px is the Select
    tool's existing outline tolerance (`SEGMENT_TOLERANCE_PX`, unchanged since
    slice 4); `specs/advanced-selection/` raises it to 8 px when it ships.
    Once `specs/advanced-selection/` ships, two clauses are added in front of
    and inside that order: Alt held at the press starts the lasso
    (`specs/advanced-selection/` criteria 16 and 17, also on a handle, and no
    handle drag starts), and Shift or Ctrl held at the press inside the
    selected box arms the marquee over the box instead of a move
    (`specs/advanced-selection/` UX notes). Until then those two clauses do
    not apply and are not tested here.
36. Once `specs/advanced-selection/` ships: given its marquee, lasso, Alt-click
    cycle and Shift or Ctrl combination, then they work on primitives
    exactly as on paths; their hit test and their touch and contain rules use
    the oriented box and the outline as defined there. Nothing in this spec
    changes them.
37. Given two or more objects selected, whatever their kinds, then no transform
    handle and no parameter handle is drawn (`0005` criterion 2); move and
    Delete work as before; criteria 21, 21a and 22 apply as stated.

### Nothing else changes

38. Given a project with primitives saved before this change, then it opens in
    the same state, and a project edited only through the Select tool's
    parameter handles saves with the same `format_version` and the same field
    names as one edited through the shape tools before (criterion 24).

## Out of scope

- Independent radii per corner: `specs/rectangle-corner-radii/`. Arc and
  node shaping of an ellipse: `specs/ellipse-arcs-and-shaping/`. This spec
  keeps the single radius and the full ellipse.
- Remembering either switch between sessions or per project. (The corner-radius
  scaling switch itself is in scope: criterion 23; its full behaviour with four
  radii is `specs/rectangle-corner-radii/`.)
- Skewing primitives (a stored per-object matrix, or conversion to a path):
  question 5, a document-model change that would be its own spec.
- Showing a path's nodes in the Select tool, or merging the Node tool into the
  Select tool.
- The blue-new/black-old preview in the Node tool (question 6).
- Multi-object transform handles, snapping, keyboard nudging, undo and redo
  (`undo-redo` slice), a Properties-panel form for the values above.
- Switching an existing shape between polygon and star.
- Any change to how shapes are created beyond criteria 25 to 30.

## Questions for the customer

Each has a default; none blocks the work.

Scope decision 2026-10-06 (lead, on the `ux-engineer`'s recommendation): the
bar's "Radius" field for rectangles is included (criterion 21a); it is the only
keyboard route and the only way to round a rectangle below 72 px on screen.

Status 2026-10-06: question 4 is resolved (confirmed, default off). The customer
has not objected to questions 1, 2, 3, 5 and 6; they stay on their defaults
(1a, 2a, 3a, 5a, 6a) until he says otherwise.

1. **"All handles on a rectangle."** Do you mean: the four corner-radius
   handles are all visible whenever the rectangle is large enough on screen
   (72 px, criterion 7; at radius 0 too) together with the resize, rotate and
   centre handles? (a) Yes, all of them (default, recommended; criteria 1, 2
   and 7); (b) Something else, for example resize
   handles on all edges even on small rectangles.
2. **After you draw a shape.** (a) The app switches to the Select tool with the
   new shape selected and all its handles visible (default, recommended: the
   next thing you do is adjust it, and there is one edit mode); costs one key
   press (`R`) per shape when you draw several in a row; (b) the drawing tool
   stays active and the shape shows no handles until you press `S`.
3. **Double-click on a primitive.** It used to open the shape's own tool. With
   one mode there is nothing to open. (a) Nothing happens (default,
   recommended); (b) it converts the shape to a path and opens the Node tool
   (rejected here: it would hide the "Object to path" decision the product
   keeps explicit). A double-click on a path still opens the Node tool, so path
   nodes remain a separate tool; please confirm that is fine.
4. **Corner radius when resizing.** Answered by the customer (2026-10-06):
   yes, a switch "Scale corner radius" next to "Scale stroke width" (criterion
   23). The default is off (a resize keeps the radius's absolute size, as in
   Inkscape and Illustrator), which changes today's always-proportional
   behaviour. Confirmed by the customer (2026-10-06): default off. Resolved.
5. **Skewing rectangles, ellipses, polygons, stars (document model).** Today
   they show no skew handles (your decision P1 of 2026-10-06, "until we refine
   the primitives"). Is it time? (a) Keep it, use "Object to path" first
   (default, recommended); (b) skew converts the shape to a path on first use
   (P2); (c) store a skew matrix per shape so it stays a rectangle (P3,
   changes the document model and `format_version`).
6. **Blue and black elsewhere.** Should the Node tool (moving nodes and
   handles) and the pen tool preview the same way, with the old path in black
   and the new one in blue? (a) Not now, only the Select tool (default);
   (b) yes, as a follow-up (small, recommended once you have tried it here).

## UX notes

Status: complete (`ux-engineer`, 2026-10-06). Sizes and tokens live in
`docs/design-system.md` (rows "Parameter handle ...", "Live preview outline",
"Select bar ..." and the rows this feature changes); this section gives the
decisions and the reasons. Distances are screen pixels; `s` is the shorter side
of the oriented box on screen; "diagonal" is the corner's inward diagonal in the
box's own (rotated) frame. Where a decision departs from an architect default in
`adrs.md` it says "ADR default:" so the architect can confirm it.

### 1. Handle tiers, small boxes, the radius handle (criteria 2, 7, 8)

**The conflict.** With the architect's rule (12 px inset, handle at inset plus
the radius, 1 pixel of handle per pixel of radius) the four radius handles clear
each other only from `s` of about 120 px, and a 12 px inset leaves the first
handle 1.3 px from the corner resize glyph (half-diagonal of the 8 px squircle
5.7, handle radius 5). Criteria 2, 7 (64 px) and 8 cannot all hold that way.
They hold with the rule below.

**Decision: one tier table, threshold T = 72 px, a normalised handle travel.**

| `s` (screen px) | Drawn on a rectangle (kinds without a part of it: see below) |
|---|---|
| under 24 | 4 corner resize, 4 corner rotate (Shift: 4 side rotate) |
| 24 to 47 | plus 4 edge resize |
| 48 to 71 | plus the centre move handle |
| 72 and up | plus the 4 radius handles; the centre handle yields when a radius handle is near (below) |

Polygon and star have no edge resize handles (unchanged); a star adds its
inner-radius handle at `s` of 72 and up; a polygon and an ellipse add nothing
(their parameter handles come with `specs/ellipse-arcs-and-shaping/` and use the
same `T` unless their own layout needs more).

**What is hidden first** when a box shrinks: the parameter handles (below 72),
then the centre handle (below 48), then the edge resize handles (below 24).
Corner resize and corner rotate are never hidden. Reason: parameter handles are
the only glyphs inside the box and the most space-hungry, and the radius has a
second route (the bar's "Radius" field, section 4); the others are the shipped
tiers and unchanged. There is no hysteresis: the tiers are discrete and change
only when the maker zooms or resizes.

**What the maker sees on a small rectangle** (`s` under 72, for example a
20 mm tag at 3 px/mm): the box, the corner and edge resize handles (by size),
the rotate handles, the rounded outline if it has a radius, and in the bar the
"Radius" field. Nothing else on the canvas: no ghost glyphs, no message. The
field's tooltip says why the corner handles are missing (section 4).

**Radius handle position.** On the corner's inward diagonal at distance
`p = 15 + ρ·L(s)` from the corner, with `ρ = effective radius / (s / 2)`
(0 for a sharp corner, 1 at the largest radius, in screen pixels) and
`L(s) = (s − 14) / √2 − 15`. So at `ρ = 0` the handle is 15 px from the corner
(glyph gap to the corner squircle 4.3 px) and at `ρ = 1` it is `(s − 14) / 2`
from its own corner on each axis: two neighbours are then 14 px apart centre to
centre, 4 px between the 10 px glyphs, at every `s`, which is what makes
criterion 8 hold by construction. Examples: `s` 72: far end 29 px from each
side, travel 26 px. `s` 100: travel 45.8 px.

**The drag stays pointer-exact, with a gain.** Radius change = the pointer's
displacement projected on the diagonal times `G(s) = (s / 2) / L(s)`. The handle
stays under the pointer until a limit is reached (zero or the clamp), as with
slope 1; the gain only says how many millimetres of radius one pixel buys: 1.38
at 72 px, 1.09 at 100, 0.86 at 200, 0.71 for a large box (finer control on a big
box, which is where precision is cheap). Zero position: exactly 0 when the
pointer's projection is at or beyond the 15 px point. ADR default: handle at
`12 + r` along the diagonal, gain 1, `T` about 100 to 120. The change in code is
one inset value, one far-end formula, one gain in `value_from_pointer`; the
clamps, the effective-radius start, the hit rule and `apply_param` are
unchanged. An unlinked corner (`specs/rectangle-corner-radii/`) may exceed
`ρ = 1` (up to its limit of criterion 4); the clearance still holds, because the
limit keeps the two radii on one side summing to at most that side's length and
the map is linear.

**The centre handle yields.** It is not drawn while any parameter handle centre
is within 20 px of the box centre (11.3 half-diagonal of the 16 px glyph, plus 5
handle radius, plus 4 clear, rounded up), and not while a parameter handle is
being dragged. On a square rectangle this hides it from `ρ` 0.61 at `s` 72, 0.78
at `s` 100 and 0.91 at `s` 200. It is the one glyph that yields because it owns
no hit area (confirms architect flag 2; same rule for the star's inner handle).

**Star inner-radius handle.** Same glyph, at the first inner vertex. Its worst
case against a corner resize glyph is a star whose point count is a multiple of
4 at ratio 0.99 (the inner vertex then sits on the box diagonal at 0.99 R, the
corner at 1.41 R): gap 4.6 px at `s` 72, 2.9 px at 64. That case, not the
rectangle, sets T at 72. The clearance property test must include it. At ratio
0.01 the handle is next to the centre handle, which yields.

**The old N/E/S/W handles of the polygon and star tool are gone.** A polygon or
star has four corner resize handles (criterion 16), the rotate handles, and the
star's inner-radius handle. The old four cardinal handles on the outer circle
are not drawn anywhere.

**Hit radius 12 px** for every parameter handle (24 px target, same as the skew
handle), not the 16 px of the resize handles. Reason: these handles are inside
the box; a 16 px radius would let four of them cover most of a 72 px box. Because
they are drawn only from 72 px up, the radius never has to shrink. Where two
parameter handles are 14 px apart (`ρ = 1`) the regions overlap by design;
nearest centre decides. Tie order (criterion 5) unchanged. Body reachable: the
point halfway between the box centre and each edge midpoint is a move at every
radius (nearest handle at least 12.7 px away at `s` 72, tested). The box centre
itself is not a move at large radii (the four handles meet there); any other
point away from a handle is.

**Parameter handles are not drawn during a move, resize, rotate or skew drag**
of the same object (they are not part of that gesture, and the box can cross 72
px mid-drag), and not for two or more selected objects. During a parameter drag
the transform handles stay drawn.

### 2. Glyph and states of a parameter handle (criterion 8)

One glyph family for every parameter handle, now and for the arc and curve
handles later: **a knob, a 10 px circle with a 1.5 px `--shape-handle-stroke`
(`--accent`) ring, `--shape-handle-fill` (white) ground and a 4 px `--accent`
centre dot.** It is the only round-with-a-dot glyph on the canvas. The others
are square (resize), square with arrows (centre), an arc arrow (rotate), two
arrows (skew), so the difference reads by silhouette and not by position or size.
The four radius handles are knobs, never squares.

- Idle: as above. Hover: ground `--accent-hover` over the white. Dragging: solid
  `--accent` ground, white dot. While its entry chip is open it stays in the
  dragging look (as the transform handles do).
- **Followers.** While a radius drag changes all four corners (criterion 2), the
  three other handles take the hover ground, so the maker sees they move in step.
  When `specs/rectangle-corner-radii/` makes an unlinked drag change one corner,
  the other three stay idle; nothing else on the canvas shows the link state (it
  is the bar's "Link corners" toggle, which has a reserved slot, section 4).
- **Guide.** While a radius handle is hovered or dragged, a dashed
  `--shape-handle-guide` line joins the corner to the handle. Not drawn at rest.
- Drawn on top of every other handle.
- **Cursor:** the built-in `pointer` on a parameter handle, hovering and
  dragging, whatever the modifiers. It is not a resize or a move cursor, so it
  does not promise one. Hint chip lines (same chip as the transform handles,
  600 ms): radius handle "Corner radius" / "Double-click: type a value"; star
  "Inner radius" / "Double-click: type a ratio". `specs/rectangle-corner-radii/`
  appends "Shift: this corner only". No Ctrl line (no snapping is defined).
- Readout during a drag: "r 3.5 mm" and "ratio 0.45" in the existing readout
  chip. When an unlinked drag is stopped by its limit, the chip reads "r 12.0 mm
  max" (`specs/rectangle-corner-radii/` open point); no other signal.
- Accessible names (architect flag 4, confirmed): corner radius "Corner radius",
  polygon and star outer radius "Outer radius" (renamed from "Radius"), star
  "Inner ratio". Visible labels stay "r", "r", "ratio". The canvas handles are
  not in the accessibility tree (known gap of the refinements spec); the bar
  fields are the accessible route.

### 3. Blue new, black old (criteria 10 to 15)

- **New:** the geometry the release would commit, drawn as a hollow outline, no
  fill, `--preview-new` (= `--accent`, `#2F6FEE`), **1.5 px constant screen
  width**, solid, drawn on top. Not scaled by the object's own stroke width and
  not dashed: it is a position marker, not the stroke.
- **Old:** the committed object exactly as it renders now, with its own stroke,
  colour and fill, nothing dimmed, nothing re-weighted. "Black" in the customer's
  words is that rendering for the default black stroke; a red stroke stays red,
  so what stays on screen is what would go to the machine. Blue is at least as
  heavy as the object's own hairline, so it reads on top where the two coincide.
- **No fill preview yet.** A filled object keeps its fill under the blue
  outline; the new fill is not shown. Revisit with `0007-stroke-and-fill-styling`.
- **Which edits:** every drag of criterion 10, a 200-object move included (every
  object shows both), and the Points and Ratio sliders of the bar (slider drag
  previews, release commits once; the architect's default, kept). **Typed
  numbers do not preview**: not the entry chip, not a bar field. They commit on
  Enter and the result shows then, as in the refinements spec (one number, no
  per-keystroke redraw); stepper arrows in a bar field commit once per click.
- **Selection box, handles, pivot marker, readout** follow the new geometry
  (criterion 10) and are drawn above the blue outline; the old geometry has no
  box. Nothing blue remains after release, Escape or a drag back to the start
  (criterion 12).

### 4. The Select bar (criteria 21, 22, 23)

**Layout.** One pill row as today (`--toolbar-bg`, `--panel-elevation-shadow`,
`rounded-lg`, floating over the canvas, never resizing it), controls 28 px high,
`text-sm`, groups separated by the existing 1 px 20 px 25 % divider, 12 px gap
inside a group. Left to right:

1. **Settings group, always shown, always first:** "Scale stroke width" and
   "Scale corner radius" (the `ToolbarSwitch`, same row 28 px, 8 px padding,
   label left of the track). They are tool state and never disabled, whatever is
   selected, and never dimmed when no rectangle is selected (the maker sets them
   before the resize). Because they come first and the bar is **left-aligned
   after the tool rail for this bar** (`justify-start`; the Node and Shape bars
   stay centred), they never move when the selection changes; only the groups to
   their right come and go.
2. **Kind groups, shown when the selection contains the kind** (below).
3. **"Object to path", last,** a text button, so it is far from "Remove rounding"
   (it changes the object kind and there is no undo yet).

**One rule for when a control shows (unifies criteria 21 and 22).** A control
is **shown when the selection contains at least one object of the kind it acts
on, and acts on exactly those objects**; the others in the selection are left
alone. It is **enabled when it would change something**. Never shown disabled for
an unrelated selection (the bar would carry three dead groups). So:

| Control | Shown when the selection contains | Enabled | Acts on |
|---|---|---|---|
| Radius | a rectangle | always | the rectangles |
| Remove rounding | a rectangle | at least one has a radius above 0 | the rectangles |
| Points | a polygon or star | always | the polygons and stars |
| Ratio | a star | always | the stars |
| Object to path | a rectangle, ellipse, polygon or star | always | those, not the paths |

With a rectangle and a star selected the bar shows Radius, Remove rounding,
Points, Ratio. The tooltip of each says what it acts on ("Remove rounding of the
selected rectangles"). Later groups (`Link corners`, `Pie / Arc / Chord`,
`Whole ellipse`, `Curve`) follow the same rule, in the order rectangle group,
ellipse group, polygon/star group, then "Object to path". The rectangle group
reserves a 28 x 28 px slot after "Radius" for the "Link corners" toggle (chain
icon, pressed state, same hover and focus as the switch).

**Radius field (architect flag 7; decided: included, criterion 21a).** Label "Radius", number field 80 px wide,
the fixed suffix "mm", accessible name "Corner radius". It shows the effective
radius of the rectangles; Enter writes one commit (all four corners, clamped to
half the shorter side as criterion 18), Escape or a press elsewhere restores the
shown value and writes nothing (the chip rule: a typo never reaches a machine job
by blur), an empty or non-numeric value marks the field `--field-invalid` with
"Enter a number" below it and writes nothing. It is the only route to a radius
on a rectangle below 72 px, the route for keyboard users, and under
`specs/rectangle-corner-radii/` it means "set all four". Tooltip: "Corner radius.
The corner handles appear on the canvas when the rectangle is at least 72 px
across on screen; zoom in or type a value." Cost: one control, no model change.

**Mixed and limited.** Selected objects that differ: the field is empty with the
muted placeholder "Mixed" (house rule); a typed value applies to all of them. A
slider shows no thumb while mixed until touched. A stored radius the bar cannot
show because it is larger than the rectangle allows: the field shows the
effective value and a 12 px muted tag "limited" inside the field before the unit;
tooltip "Stored 20 mm, limited to 15 mm by the size; enlarging brings it back."
Ratio shows two decimals; the stored value keeps its precision and no hint is
shown for that. The same "limited" tag is for the curve value later.

**Compactness.** Single row where it fits: for a rectangle about 760 px, for a
star about 780 px; fixed-width fields so nothing jitters while typing. No icons,
no group titles. Where the available width (canvas width minus 84 px) is smaller,
the row wraps by whole groups (`flex-wrap`, no script): the settings group stays
on row 1, the kind groups and "Object to path" go to row 2 (the bar grows
downward by 36 px, never covers the tool rail). DOM and tab order: settings,
kind groups, "Object to path". Disabled controls: 40 % opacity, `aria-disabled`,
tooltip kept.

**Switch tooltip** (new): "Scale corner radius with the object. Off: a resize
keeps the corner radius." `ToolbarSwitch` is the existing switch pattern used
twice; state in `useEditorSession`, off per session.

**Object to path has no shortcut.** The obvious one (Shift+Ctrl+C in Inkscape)
opens the inspector in a browser build, and a conversion with no undo yet should
not be a hotkey. Revisit with the undo slice.

### 5. Double-click, creation tools and discoverability (criteria 25 to 34)

- **Double-click on a primitive's outline, body or centre handle: nothing
  changes, plus a hint.** The shared hint chip appears at the pointer for 3
  seconds (or until a press, key or leave) with three lines: "Drag a handle to
  edit this shape" / "Double-click a handle to type a value" / "Nodes: Object to
  path, then double-click". It is a text chip, `pointer-events: none`, writes
  nothing and changes neither tool nor selection, so criterion 32 holds. Shown on
  every such double-click, not once: it explains the missing behaviour exactly
  when the maker expects it, and costs nothing otherwise. A double-click on a
  path still opens the Node tool; on a skew handle or empty canvas nothing and no
  hint.
- **Creation tools over an existing object:** the tool's crosshair everywhere, no
  hover highlight of existing objects, no hint chip, no handle, no hit state.
  A press creates (criterion 25). Choosing a creation tool clears the selection,
  so no selection box is drawn under it (criterion 27, amended 2026-10-06).
- **After a create-drag** the tool switches silently (criterion 28): the rail
  highlights Select, the Select bar replaces the shape bar in place, the new
  shape shows its handles. No toast; the handles appearing is the signal. `R`,
  `E`, `*` start the next shape.
- **Polygon/Star bar** keeps mode, "Points" and "Ratio" as next-shape settings;
  the same labels as in the Select bar would be read as acting on the selection
  (criterion 29 says they never do), so the creation bar shows a muted prefix
  "New:" before the two fields. The PO may choose other wording.

### 6. Changes the criteria needed (applied 2026-10-06)

The `product-owner` applied every item below to the criteria on 2026-10-06;
the list is kept as the record of what the review changed.

- **Criterion 2:** the zero-radius position is the handle's 15 px point on the
  diagonal (not "the corner"); add "the handle stays under the pointer until a
  limit"; the position rule belongs in the design system, not the criterion.
- **Criterion 5:** the parameter handle's hit radius is 12 px (design-system row
  "Parameter handle hit-test radius"), not "the shape-handle radius" (16 px).
- **Criterion 7:** threshold is 72 px (not 64); the tier order is: parameter
  handles hide first, then centre, then edge resize, as in section 1.
- **Criterion 8:** reword to "no two drawn glyphs overlap or come closer than 4
  px at any size `s` >= 72 and any radius, except that the centre handle is not
  drawn when a parameter handle is within 20 px of it" (the yield rule); the
  silhouette sentence is met by the knob glyph (section 2).
- **Criterion 10:** add that the new outline is `--preview-new`, 1.5 px constant,
  no fill preview, old geometry in its own committed style; and that the Points
  and Ratio sliders of criterion 21 use it (typed values do not).
- **Criteria 18 and 19:** accessible names "Corner radius" and "Inner ratio"
  (flag 4); validation texts "Enter a number", "Must be 0 or more", "Must be 0.01
  to 0.99".
- **Criteria 21 and 22:** replace both visibility rules with the one rule of
  section 4 (shown when the selection contains the kind, acts on those, enabled
  when it would change something); drop "consists only of" and "of any kinds";
  add the "Radius" field (flag 7) as criterion 21a (accepted); add the Mixed
  and "limited" displays.
- **Criterion 23:** replace the last sentence ("where and how the two switches
  sit ... is the `ux-engineer`'s call") with: both switches come first in the bar,
  are left-anchored, never disabled and never dimmed; tooltip texts of the design
  system.
- **Criterion 25:** add that the creation tools show their crosshair everywhere
  and no hover highlight on existing objects.
- **Criteria 31 to 34:** criterion 32 gets "a hint chip appears (design-system
  row), no tool, document or selection state changes".
- **Criterion 29:** the creation bar's two fields carry the prefix "New:" (the PO
  keeps this wording).
- Other specs: `specs/rectangle-corner-radii/` criterion 1 (the handle position
  rule, `ρ` may exceed 1 unlinked), criterion 14 (flag 5 rewording: the switch is
  read when the entry opens); `specs/object-transform-refinements/` UX note
  "Select tool top bar: Nothing is added" is superseded by section 4, and its
  size-chip accessible name "Radius" becomes "Outer radius";
  `specs/ellipse-arcs-and-shaping/` places its arc and curve handles with the knob
  glyph, tier `T` and the 4 px rule of section 1.

Needs the architect (departures from `adrs.md`): inset 15 px (not 12), far end
`(s − 14)/√2` and gain `G(s)` instead of slope 1, `T` 72, parameter hit radius 12
(`param_hit_px`, not `resize_radius`), parameter handles not drawn during other
drags, the clearance property test extended to the star worst case and the
body-reachability point of section 1.

### Known gaps, deliberately not closed here

- The radius, ratio and every other canvas handle have no keyboard route; the
  bar's Radius, Points and Ratio fields are the accessible path, the
  Properties-panel transform form (refinements spec) remains the follow-up.
- No live preview of typed values (the chip and the bar fields), no fill preview.
- The star's inner-radius handle at ratio 0.99 has only 4.6 px of clearance to
  the corner resize glyph at the threshold; below 72 px it is hidden.

## Sequencing

Builds after PR #35 is merged. Touches `select_tool`, the handle layout, the
three shape tools, `Session` and the front-end tool bars, the same code as
`specs/advanced-selection/`, `specs/0007-stroke-and-fill-styling/` and
`specs/shape-creation-from-center/`; the lead sets the order and none runs in
parallel. Recommendation: this spec before `shape-creation-from-center`, whose
criteria 16 and 17 would otherwise be built and then replaced (its Shift and
Ctrl creation logic is independent and can follow unchanged). It may ship in
two PRs without changing the story: (1) parameter handles, controls and the
blue/black preview in the Select tool (criteria 1 to 24); (2) creation-only
tools and the double-click change (criteria 25 to 34), which removes the old
mode. The story is done when both are in. Between the two PRs the product is
shippable but inconsistent (the shape tools still select and edit while the
Select tool also edits them), so PR 2 must merge in the same window as PR 1,
before any release. Tester note: assert that a move dragged back to its start
writes nothing (criterion 12). Tests that drive a primitive through
its own tool (`vecmanf-ui-core` shape-tool unit tests,
`vecmanf-editor-wasm/tests/acceptance_0003*.rs`, `acceptance_0004.rs`) are
rewritten against the Select tool, not deleted.

## Links
Requirements: R-EDIT-002, R-EDIT-011, R-EDIT-012 (`docs/requirements.md`)
Builds on: `specs/0003-primitive-shapes/`, `specs/0004-canvas-navigation-and-
selection/`, `specs/0005-object-transform/`, `specs/object-transform-
refinements/`, `specs/advanced-selection/`, `specs/shape-creation-from-
center/`
Followed by: `specs/rectangle-corner-radii/`, `specs/ellipse-arcs-and-
shaping/`
PR:
