# Unified object editing: one Select tool for every object and its own handles

Status: Draft
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
   UX rule move to the Select tool (criteria 1 to 9, 20 to 22). Criterion 3
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

## Acceptance criteria

All criteria concern the Select tool with exactly one primitive selected
unless they say otherwise. "Handles drawn" follows the size tiers of
`specs/object-transform-refinements/` UX notes plus criterion 7.

### Parameter handles next to the transform handles

1. Given one selected rectangle, then the Select tool shows its transform
   handles (all rules of `specs/object-transform-refinements/`) and four
   corner-radius handles, one at each corner, at the same time. Given one
   selected star, then it shows its transform handles and one inner-radius
   handle at the star's first inner vertex. Given a polygon or an ellipse,
   then it shows its transform handles and no parameter handle (an ellipse's
   arc handles, and the curve handle of an ellipse, polygon and star, are
   `specs/ellipse-arcs-and-shaping/`). No tool or mode switch
   is needed to see or use any of them.
2. Given a rectangle, then its four radius handles are drawn at radius 0 as
   well (the maker discovers rounding by finding them) and all four are
   draggable. Dragging any of them changes the rectangle's one radius and all
   four corners follow; the other three handles move in step. The radius is
   clamped to half the shorter side (`0003` criterion 5), and is exactly 0
   when the pointer's projection on the corner's diagonal is at or beyond the
   handle's zero-radius position (`0003` criterion 6). The rule is the one
   `specs/0003-primitive-shapes/` criteria 4 to 6 define for the Rectangle
   tool today; independent radii per corner are
   `specs/rectangle-corner-radii/`.
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
   defines for transform handles. A parameter handle's hit radius is the
   shape-handle radius in `docs/design-system.md`. On an exact tie the order
   is parameter handle, resize, skew, rotate. A press inside the box that is
   on no handle moves the object; the centre handle is not hit-tested.
6. Given a handle that is not drawn (criterion 7), then it has no hit area and
   no hover or cursor state, so a press where it would be moves the object.
7. Given the box's shorter side `s` in screen pixels, then the radius and
   inner-radius handles are drawn only when `s` is at least the parameter-
   handle threshold (Proposal: 64 px; the `ux-engineer` sets the value). The
   transform-handle tiers of `specs/object-transform-refinements/` are
   unchanged. Below the threshold the primitive is still fully editable by
   zooming in, by the transform handles and by the bar controls
   (criterion 21).
8. Given any size or radius at which handles are drawn, then no two drawn
   glyphs overlap or come closer than 4 screen pixels, in particular a radius
   handle at the largest radius clears the centre handle and every transform
   handle; where that cannot hold at some size, the `ux-engineer` raises the
   threshold in criterion 7. A parameter handle is distinguishable from a
   transform handle by silhouette at a glance, not only by position or size
   (`ux-engineer`: two vocabularies are now on screen together).
9. Given a press and release on a parameter handle with less than 3 screen
   pixels of movement, then nothing is written (the dead zone of
   `specs/object-transform-refinements/`). Given Escape during a parameter-
   handle drag, then nothing is written and the preview disappears.

### Blue new, black old, in every edit

10. Given a Select-tool drag that changes geometry (move, resize, rotate, skew,
    corner radius, inner radius), then for the whole drag every affected
    object is drawn twice: its committed geometry unchanged in its normal
    rendering (today black), and the geometry the release would commit as a
    1.5 px hollow outline in `--accent` (blue), no fill, drawn on top. The
    selection box, the handles and the readout follow the new geometry.
    Today's Select-tool preview, which redraws the object itself at the new
    place, is replaced by this.
11. Given a drag of a multi-object selection (a move), then every selected
    object shows both renderings.
12. Given the pointer is released, then exactly one commit is made, the blue
    outline disappears and the object renders in its normal style at the new
    geometry. Given Escape, or a drag that returns to its start point, or a
    press under the 3 px dead zone, then nothing is written, no blue outline
    appears or remains, and the old geometry is unchanged.
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
    "r" (accessible name "Radius", unit mm) opens next to it, pre-filled with
    the current effective radius, text selected. Enter writes the value to the
    one radius (all four corners) in one commit. Zero is valid (sharp corners).
    A value above half the shorter side is limited to that value and the
    limited value is what is written. An empty or non-numeric value keeps the
    field open and marked invalid and writes nothing. An unedited Enter,
    Escape, a click elsewhere, a tool switch and a selection change close the
    field and write nothing, and the closing press is not swallowed. All of it
    is the behaviour of criteria 19 to 21 and 30 to 31 of `specs/object-
    transform-refinements/`; no double-click here switches tools.
19. Given a double-click on a star's inner-radius handle, then a field labelled
    "ratio" opens, pre-filled with the current ratio (two decimals), with the
    same rules as criterion 18; a value outside 0.01 to 0.99 is invalid.
20. Given a parameter-handle drag, then a live readout is shown at the pointer
    (same placement as the transform readouts): "r 3.5 mm" for a rectangle's
    radius, "ratio 0.45" for a star. Before this spec the shape tools showed
    no readout for these drags.

### Controls that move into the Select tool

21. Given a selection that consists only of rectangles, then the Select tool's
    top bar offers "Remove rounding", which zeroes the radius of every selected
    rectangle in one commit. Given a selection that consists only of
    polygons and stars, then it offers "Points" (integer 3 to 1024) and, if
    every selected shape is a star, "Ratio" (0.01 to 0.99). Changing either
    updates every selected shape of that kind live and commits once per
    interaction (a slider drag is one commit, not one per tick), as `0003`
    criteria 10, 14 and 15 define. If the selected shapes hold different
    values, the field shows "Mixed" and a typed value applies to all.
22. Given a selection of one or more primitives of any kinds, then the Select
    tool's top bar offers "Object to path"; `0003` criteria 17 to 22 hold
    unchanged (one atomic commit, each primitive independently, no implicit
    conversion anywhere else). The shape tools' bars no longer show it.
23. Given the Select tool is active, then two switches stay visible and
    operable with or without a selection: "Scale stroke width" (`0005`
    criterion 30) and "Scale corner radius" (customer decision 2026-10-06; off
    at program start). Both are per-session UI state, never written to the
    project, and read at the press that starts a resize drag. "Scale corner
    radius" decides whether a resize of a rectangle scales its radius by
    `√(sx·sy)` (on) or leaves it at its absolute size (off, the default);
    the behaviour is defined in `specs/rectangle-corner-radii/` criteria 12
    and 15, and until that spec is built a rectangle's single radius follows
    the same rule. The kind-specific controls above are shown only while they
    apply. Where and how the two switches sit on the bar, next to the other
    controls, is the `ux-engineer`'s call (a possible new top-bar layout).
24. Given a primitive with a radius, a ratio or a point count set by any of
    criteria 2, 3, 18, 19 or 21, then the stored fields written are the ones
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
    without Shift, and never shows a handle.
26. Given a creation tool is active, when the maker presses and releases without
    moving, then nothing is created and the selection is unchanged (today the
    press clears it).
27. Given objects are selected when a creation tool becomes active, then the
    selection stays and each selected object keeps only its plain selection box
    (no handles, no hover, no hit-testing), so the maker still sees what the
    Properties panel would act on.
28. Given a create-drag commits (default of question 2), then the Select tool
    becomes active in the same step with the new shape as the only selected
    object and its full handle set drawn (`0003` criterion 1, "it becomes the
    selected object"), and the tool rail highlights Select. The `R`, `E` and
    `*` shortcuts and the rail order are unchanged; `S` returns to Select at
    any time.
29. Given the Polygon/Star tool, then its bar keeps the mode toggle, "Points"
    and "Ratio" as settings for the next shape: they persist across shapes
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
    nothing happens: the tool does not change, nothing is written, the
    selection stays. This replaces `0004` criterion 23.
33. Given a double-click on any handle of any object, then the numeric entry
    of the refinements spec or of criteria 18 and 19 opens where one is
    defined for that handle; where none is (skew handles), nothing happens;
    in no case does a tool switch happen.
34. Given a double-click on empty canvas, then nothing happens, as before.

### Selection gestures stay consistent

35. Given a press with exactly one primitive selected, then the order is: Alt
    held at the press starts the lasso (`specs/advanced-selection/`
    criteria 16 and 17, also on a handle; no handle drag starts); else a hit
    on a drawn handle (criterion 5) starts that handle's drag, Shift or not
    (`specs/object-transform-refinements/` criterion 11, extended to parameter
    handles); else a press inside the selected box starts a move, except that
    Shift or Ctrl held at the press arms the marquee over the box
    (`specs/advanced-selection/` UX notes); else an outline hit within 8 px
    selects, Shift toggles; else the marquee starts.
36. Given the marquee, the lasso, the Alt-click cycle and Shift or Ctrl
    combination of `specs/advanced-selection/`, then they work on primitives
    exactly as on paths; their hit test and their touch and contain rules use
    the oriented box and the outline as defined there. Nothing in this spec
    changes them.
37. Given two or more objects selected, whatever their kinds, then no transform
    handle and no parameter handle is drawn (`0005` criterion 2); move and
    Delete work as before; criteria 21 and 22 apply as stated.

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

Status 2026-10-06: question 4 is resolved (confirmed, default off). The customer
has not objected to questions 1, 2, 3, 5 and 6; they stay on their defaults
(1a, 2a, 3a, 5a, 6a) until he says otherwise.

1. **"All handles on a rectangle."** Do you mean: the four corner-radius
   handles are all visible at all times (at radius 0 too) together with the
   resize, rotate and centre handles? (a) Yes, all of them always (default,
   recommended; criteria 1 and 2); (b) Something else, for example resize
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

(filled in by ux-engineer before Ready)

Open points for the `ux-engineer`, found while writing this spec:

- Two handle vocabularies are on screen together for the first time
  (transform: rounded square, arrow icons; parameter: the former shape-handle
  square). The 8 px hollow square and the 8 px squircle are nearly identical;
  pick a distinct silhouette for parameter handles (criterion 8).
- Radius-handle placement at radius 0: today it sits on the NE resize handle
  and wins the tie. With four of them and a centre handle, find positions that
  never collide (criteria 7 and 8); a fixed screen inset from the corner at
  small radii is one option.
- Where the kind-specific controls (criteria 21, 22) sit in the Select tool's
  top bar, how "Mixed" looks, and whether "Object to path" gets a shortcut.
  The bar now has two always-visible switches ("Scale stroke width", "Scale
  corner radius", criterion 23) and, for ellipses and rectangles, further
  controls from `specs/ellipse-arcs-and-shaping/` and
  `specs/rectangle-corner-radii/`: a layout for the whole bar is needed
  (grouping, order, what wraps on a narrow window), not two more loose
  checkboxes.
- Hover hint texts for parameter handles ("Drag: radius", "Double-click: type
  a value"), cursors for them, and the blue preview's exact weight (1.5 px
  today).
- Which cursor and hint a creation tool shows over an existing object, now
  that a press there creates (criterion 25).

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
mode. The story is done when both are in. Tests that drive a primitive through
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
