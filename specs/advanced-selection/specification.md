# Advanced selection: bigger hit area, candidate disambiguation, marquee and lasso select

Status: In progress
Priority: Must
Origin: Customer

## User value

As a maker I want objects to be easier to click precisely, a clear way to
pick the right one when several lie close together, and the ability to drag
out a box or a freehand line to select many objects at once the way I
already do in LightBurn and Inkscape, so that selecting the right thing (or
group of things) in a crowded drawing stops being fiddly and matches muscle
memory I already have from both tools.

The customer's own words (translated): "Selecting is still a bit fiddly. We
probably need to make the hit area bigger first. At the same time, when
several objects lie within the hit area, I want a nice way to pick the
right one (node, object, path, ...). I also want to be able to drag out a
selection with the mouse. I'm orienting this a bit on LightBurn: if you drag
toward the left, the selection box draws green and selects everything it
touches. Dragging right makes it red, and it only selects what lies
completely inside the selection box. With Shift you can swap green and red.
With Ctrl you can add to the selection. I also want Inkscape's Alt-selector
for selection — holding it down, you draw a freehand line, and every object
the line touches gets selected. So you can also press Ctrl+Shift, and
Ctrl+Alt — if while adding you want to swap the red/green selection mode, or
add using the line-touch mode."

This extends `canvas-navigation-and-selection` (slice 4)'s Select tool
directly — the same `hit_test_object`, `ObjectSelection` and click/
shift-click mechanics, not a new tool. Slice 4's Out-of-scope list
explicitly deferred "marquee/rubber-band selection... for objects" with the
note that shift-click was "this slice's only multi-select input" for now.
This is that deferral's revisit, plus two gaps slice 4 never addressed: the
object hit-test tolerance it shipped (4px, reused from the Node tool's
segment test) and what happens when more than one object lies within it.

## Acceptance criteria

### Hit-area enlargement

1. Given the Select tool is active, when the maker clicks or hovers within
   **8 screen pixels** of any object's outline (a path's anchor-run
   segments, or a primitive's outline), then that object is hit, exactly as
   if the click had landed on the outline itself. The 8 px is the constant
   `OBJECT_TOLERANCE_PX`, the Select tool's own; it doubles the 4px
   tolerance slice 4 shipped (`SEGMENT_TOLERANCE_PX`, still the Node tool's
   segment tolerance, criterion 2). This is
   the same Fitts's-law-margin doubling `canvas-navigation-and-selection`
   already applied to the node (8px→16px) and handle (8px→16px) hit-test
   radii, applied here to whole-object selection for the first time.
2. Given the Node tool is active, then its own segment hit-test tolerance
   for picking a specific curve segment to edit stays at 4px, unchanged —
   criterion 1's enlargement is the Select tool's own, separate tolerance.
   A tighter tolerance for the Node tool's segment pick remains correct
   there: when several segments of the *same* path run close together, a
   wider tolerance would make it harder, not easier, to pick the right one
   to edit — the opposite problem from Select-tool object picking, which
   criteria 3-7 below solve a different way.

### Disambiguating overlapping candidates

Reference tools solve "more than one thing is under the cursor" differently.
Blender cycles the object under a *stationary, unmodified* repeated click —
natural in a 3D viewport with no other use for a plain click-in-place, but
it would collide here with this tool's own existing rule that a plain click
on an already-selected single object is a no-op (slice 4 criterion 16), and
with telling a second click-in-place apart from the start of a tiny drag.
Inkscape already solves exactly this with **Alt**: a plain click selects the
one candidate `hit_test_object` returns, as today; each further Alt-click at
the same point steps to the next candidate in the stack, wrapping around.
Inkscape orders that stack by z-order, because its hit test is fill-based.
Since `stroke-and-fill-styling` shipped, this product's plain click is
fill-aware too (its criterion 27): an outline within tolerance wins, and a
filled shape's interior is hit where no outline is. The candidate set is
the set `hit_test_objects` computes for that same click, and the cycle
continues through it instead of stopping at the first. Its order, with F
the filled shape(s) under the point:

1. the plain-click answer (what `hit_test_object` returns);
2. the other outlines within tolerance, nearest first, that lie at or above F;
3. F itself, the filled shape(s) under the point;
4. the objects F hides: their outlines nearest first, then objects hit only
   through their filled interior, topmost first.

This is "visible before covered". It differs from the wording in
`stroke-and-fill-styling` only when an outline below F is in tolerance;
that spec now points here. With no filled shape under the point the order
is simply nearest outline first. Grounding this in Inkscape rather than Blender also keeps one
coherent story for the Alt key: it is the modifier for "look past the
obvious candidate," both for a single point (this section) and, held
through a drag, for the freehand line (criteria 16-20 below) — the same key
Inkscape itself already overloads the same way, disambiguated the same way
(held-in-place vs. held-through-a-drag).

3. Given the Select tool is active and two or more objects are candidates
   at the same screen point (each within criterion 1's 8px tolerance, or
   hit through a filled interior), when the maker clicks that point once
   with no modifier, then the plain-click answer is selected — unchanged
   from today's rule (`stroke-and-fill-styling` criterion 27; among
   outlines the nearest wins, and an exact distance tie favors the topmost
   in z-order). This is the first element of the cycle order above.
4. Given the same situation, when the maker holds Alt and clicks the same
   screen point again without dragging, then the next candidate in the cycle
   order above is selected instead (other outlines within tolerance, nearest
   first, then the filled shape(s) under the point, then the objects they
   hide), replacing the previous selection.
5. Given a cycle has started (criterion 4), when the maker Alt-clicks the
   same point again, then the cycle advances by exactly one more candidate
   each time, in the same order, so that after as many Alt-clicks as there
   are candidates at that point, the selection returns to the first one (the
   plain-click answer).
6. Given a cycle has started, when the maker clicks or Alt-clicks at a
   screen point more than 8px from the point the cycle started at, then the
   cycle resets — the next Alt-click there starts again from the first
   candidate at the *new* point, not wherever the old cycle left off.
7. Given only one object lies within tolerance of a point, then Alt-clicking
   it repeatedly leaves it selected — a one-candidate "cycle" is a no-op,
   not an error. Given no candidate lies at the point (empty canvas), then
   an Alt-click leaves the selection unchanged; unlike a plain click on
   empty canvas (criterion 8), it does not clear.

### Marquee (drag-box) select

The customer's LightBurn reference, checked against LightBurn's own
documentation: dragging **right** (the box's end point has a greater
screen-space x than its start point, regardless of vertical motion) draws a
**red "Enclosing Selection"** box that selects only objects fully contained
within it; dragging **left** draws a **green "Crossing Selection"** box that
selects every object the box fully contains *or* crosses. This matches the
customer's own description exactly, and this direction-to-mode default is
unchanged by the modifier rework below.

**Modifier scheme, reworked 2026-10-05.** The spec originally shipped here
had Shift swap the color/mode mapping and Ctrl add to the selection, per the
customer's first description — flagged under "Open questions" at the time
because it didn't match LightBurn's own documented modifiers (Shift adds,
Ctrl toggles, neither swaps direction-to-color). The customer has since
confirmed LightBurn's documented behaviour is correct and called the lack of
a mode-swap "annoying." Their own words (translated): "Let's separate it
more simply: Shift always adds to the selection, Ctrl always removes from
the selection (strictly — not a toggle)... Alt does the inverting of the
selection mode [touch vs. contain]... Alt is always 'the selection mode'
key, Shift always adds, Ctrl always removes." This resolves "Open questions"
1 and 2 below and replaces the swap/add scheme entirely. The three
modifiers are now independent and combine:

- **Shift**: the drag's result is **added** to the current selection,
  never replacing it.
- **Ctrl**: the drag's result is **strictly removed/subtracted** from the
  current selection instead — every object in the result is deselected if
  it was selected, and every object not in the result is left exactly as
  it was. Not a toggle: an object the drag touches that was *not* already
  selected is never added by Ctrl.
- **Alt**: inverts the touch/contain mode the drag direction would
  otherwise produce (leftward's default touch becomes contain, rightward's
  default contain becomes touch) — see "Alt's two jobs" below for how this
  coexists with Alt also arming the lasso (criteria 16-20).
- **Combinations**: Alt+Shift = inverted mode, add. Alt+Ctrl = inverted
  mode, remove. **Shift+Ctrl held together** is not a combination the
  customer described, and the two operations are mutually exclusive in
  effect (add vs. remove the same result). This spec resolves it
  deterministically: **Ctrl wins** — holding both removes the drag's
  result, exactly as Ctrl alone would, with Shift having no further effect.
  **Confirmed by the `ux-engineer`, 2026-10-05** (see "Modifier-state
  legend" and "Cursor changes" under UX notes for the fuller reasoning):
  Ctrl-wins stays, but not for the PO's original "removing is the smaller
  mistake" framing alone — that framing only holds when the *stray*
  modifier is Shift (an unwanted add left in place is harmless to undo by
  hand). It does not hold symmetrically: if the stray modifier were instead
  Ctrl, "Shift wins" would silently deselect objects the maker meant to
  keep, which is the actually destructive failure mode, not the safer one.
  Ctrl-wins is right for a more concrete reason instead: the realistic way
  a maker ends up holding both at once is mid-session — keeping Shift held
  down across a string of additive drags, then pressing Ctrl *in addition*
  for one drag they want to subtract instead. Ctrl is the just-pressed,
  most-recent modifier in that sequence, so "last-pressed wins" and
  "Ctrl wins" agree. No dedicated cursor or legend state for this
  combination: its effect is byte-for-byte identical to Ctrl held alone, so
  the legend renders exactly the same `−Remove` it would for Ctrl alone
  (see "Modifier-state legend" below) — inventing a fourth, visually
  distinct state for a combination that behaves identically to an existing
  one would imply a behavior difference that does not exist.

This also removes the previous section's one deliberate LightBurn deviation:
Shift now always adds, matching LightBurn's own documented Shift exactly;
only Ctrl's strict-remove (vs. LightBurn's toggle/Ctrl+Shift-remove) and
Alt's mode-invert (which LightBurn has no equivalent of at all) remain
genuine differences from LightBurn, both by this round's explicit customer
request.

"Fully contained" and "crossed by" are both evaluated against the box the
Select tool already draws as that object's own selection indicator (the
object's bounds, `canvas-navigation-and-selection`; since `object-transform`
an oriented box for a rotated object, see Out of scope) — not a true
curve-intersection test against the marquee's edges. This is the same
simplification every reference tool's own rubber-band select makes in
practice (testing a Bézier curve against four line segments for every
object on every drag frame is unnecessary), and "touches" means "touches the
object's bounding extent," never "touches the filled interior."

| Modifier(s) held | Drag direction | Box color / mode | Selects | Combines with existing selection |
|---|---|---|---|---|
| none | leftward | green / touch | intersects the drag rectangle at all | replaces |
| none | rightward | red / contain | lies entirely inside the drag rectangle | replaces |
| Alt | leftward | red / contain (inverted) | fully inside | replaces |
| Alt | rightward | green / touch (inverted) | intersects at all | replaces |
| Shift | leftward | green / touch | intersects at all | **adds** |
| Shift | rightward | red / contain | fully inside | **adds** |
| Ctrl | leftward | green / touch | intersects at all | **removes** |
| Ctrl | rightward | red / contain | fully inside | **removes** |
| Alt+Shift | leftward | red / contain (inverted) | fully inside | **adds** |
| Alt+Shift | rightward | green / touch (inverted) | intersects at all | **adds** |
| Alt+Ctrl | leftward | red / contain (inverted) | fully inside | **removes** |
| Alt+Ctrl | rightward | green / touch (inverted) | intersects at all | **removes** |
| Shift+Ctrl (any direction/Alt state) | — | mode per direction/Alt as above | per mode | **removes** (Ctrl wins, see above) |
| Alt, held at press (freehand line, not a box) | n/a | touch only, no mode to invert | line crosses the object's outline | replaces |
| Alt+Shift, held at press (freehand line) | n/a | touch only | line crosses the object's outline | **adds** |
| Alt+Ctrl, held at press (freehand line) | n/a | touch only | line crosses the object's outline | **removes** |

### Alt's two jobs: deciding lasso-vs-invert at press time

Alt already had a job in this spec before this rework: holding it (from the
press, not added mid-drag) turns the whole gesture into a freehand lasso
instead of a box (criterion 16). Making Alt also invert a box's mode creates
a real conflict the customer did not address — a lasso has no "contain"
state to invert, so "Alt inverts the mode" and "Alt means lasso" cannot both
be live for the same drag at the same instant.

**Resolution: Alt's role is decided once, at the moment a drag starts
(press time), exactly as criterion 16 already decided "box vs. lasso."**

- **Alt held already at press**: the drag is a lasso from the start,
  unchanged from criterion 16 — there is no box mode to invert, so Alt's
  mode-inversion job simply does not apply to a lasso (moot, not
  skipped-but-latent).
- **Alt not held at press, so a box drag is already underway, then Alt is
  pressed before release**: the drag stays a box — it does not
  retroactively become a lasso partway through — and Alt now inverts that
  box's mode live, exactly as a direction reversal or a Shift/Ctrl change
  already updates the box's rendered color/mode and combine behaviour
  mid-drag (criterion 14, generalized to all three modifiers by this
  rework). Releasing Alt before release reverts the mode to the
  un-inverted mapping; the mode in effect at release is what is applied.
- Releasing Alt mid-drag during an already-started **lasso** has no defined
  effect on that lasso — the gesture was decided as a lasso at press and
  does not revert to a box mid-drag. Only the forward direction (box, then
  Alt inverts its mode) is live-updating; "box vs. lasso" itself is locked
  at press.

This keeps one mental model across the whole spec: **Alt decides what a
drag means, once, at the instant it starts** (box or lasso); for a drag
that started as a box, Alt's meaning while held thereafter is "invert this
box's mode," live, same as every other modifier already updates live. The
alternative — Alt always means "lasso," full stop, so pressing it mid-box
would abort the box and restart as a lasso from the *original* press point —
was considered and rejected: it would make Alt-while-dragging behave
completely differently depending on exact timing (a few frames after press
vs. later), is a bigger mid-gesture behavior change than any other modifier
in this spec ever causes, and gives up the "fix the mode without restarting
the drag" job the customer specifically asked Alt to do, for a consistency
criterion 16 never actually promised (criterion 16 only states Alt's
behavior for drags that were already Alt-held at press).

8. Given the Select tool is active, when the maker presses down on empty
   canvas (no object within criterion 1's tolerance) and the pointer then
   moves more than **3 screen pixels** from the press point before release
   — the click-vs-drag threshold the Select tool's other drags use
   (`DRAG_THRESHOLD_PX`, `curvyo-ui-core/src/transform_handle_layout.rs`) —
   then a marquee drag begins instead of clearing the selection outright. A
   press-and-release within that 3px threshold is a plain click: criterion
   15 of `canvas-navigation-and-selection` ("clicking empty canvas clears
   the selection") still applies — except that a Ctrl-held
   click-without-movement leaves the current selection unchanged instead of
   clearing it, since "clear on empty click" is a no-modifier behavior and
   Ctrl must not remove more than the click/drag actually touches. The
   clear happens **at the release, not at the press**, so that Shift and
   Ctrl are read at the moment of release, as criteria 12 and 13 need.
   Given Esc is pressed while the button is still held after such a press,
   then the gesture is cancelled and the selection is kept. This threshold
   also protects criterion 4's
   Alt-click cycling: without it, a pixel or two of hand jitter during a
   stationary Alt-click would register as a (zero-length) lasso and select
   every candidate along that accidental line instead of cycling to one.
9. Given a marquee drag begins with no object under the press point, when
   the maker's drag vector points leftward (end point's x is less than the
   start point's x, any y), then the box renders green and, on release,
   every object whose bounding box intersects the drag rectangle at all is
   selected, replacing the current selection.
10. Given the same start, when the drag vector points rightward (end
    point's x is greater than the start point's x; a drag with no net
    horizontal movement at all counts as not-rightward, i.e. green/touch,
    the same default as a leftward drag), then the box renders red and only
    objects whose bounding box lies entirely inside the drag rectangle are
    selected on release, replacing the current selection.
11. Given a box drag is underway (Alt was not held at the press that
    started it, so criterion 16 did not turn it into a lasso), when the
    maker holds Alt at any point before release, then criteria 9 and 10's
    direction-to-mode mapping inverts for as long as Alt stays held: a
    leftward drag renders red and selects only fully-contained objects, a
    rightward drag renders green and selects every touched object.
    Releasing Alt before release reverts the mapping to criteria 9-10's
    un-inverted default. The mode in effect at the moment of release is
    what is applied (see criterion 14's live-update rule). Otherwise
    identical to criteria 9-10: still replacing the current selection
    unless Shift or Ctrl is also held (criteria 12-13).
12. Given the maker holds Shift at the moment of release (regardless of
    when during the drag it was pressed), then the result selected under
    whichever mode is in effect at release (criteria 9-11) is **added** to
    the current selection rather than replacing it — nothing already
    selected is deselected, even if it lies outside the drag rectangle.
13. Given the maker holds Ctrl at the moment of release instead of Shift,
    then the result selected under whichever mode is in effect at release
    is **strictly removed** from the current selection instead: every
    object in that result is deselected if it was selected, and every
    object not in that result is left exactly as it was. Given the maker
    holds both Shift and Ctrl at release, then Ctrl takes precedence —
    the result is removed, exactly as if Shift were not held at all (see
    the modifier table's note above).
14. Given the box's rendered color/mode and pending combine behaviour are
    visible feedback while the drag is in progress (not only after
    release), when the maker reverses the drag's net horizontal direction,
    or presses or releases Alt, Shift or Ctrl, at any point before
    releasing, then the box's color, mode and combine behaviour update live
    to match the new state — the color, mode and combine behaviour in
    effect at the moment of release are what is applied, not whatever was
    shown earlier in the drag.
15. Given a marquee selection is made via criteria 9-14, then it is a
    single selection-replacing, selection-adding or selection-removing
    action, not a drag-to-move — no object moves as a result of a marquee
    drag that started on empty canvas, even if the box ends up overlapping
    or containing objects.

### Lasso (freehand touch-line) select

Grounded in Inkscape's own Touch Selector gesture, checked against
Inkscape's behaviour: holding Alt and dragging already works as a modifier
on the Selector tool itself in Inkscape (not only as Inkscape's separate `W`
tool) — so framing it here as an Alt-drag modifier on this product's one
Select tool, rather than a seventh tool-rail entry, matches Inkscape's own
modifier gesture, not a deviation from it. Inkscape's own touch-line
selects **on release**, not continuously as the line is drawn (every object
the finished line touches is selected the moment the mouse button comes
up) — the drawn line itself is live visual feedback only.

**No deviation from Inkscape remains here after the 2026-10-05 modifier
rework** (see "Modifier scheme, reworked 2026-10-05" under Marquee above).
The previous revision used Ctrl+Alt to add to the running lasso selection,
deliberately deviating from Inkscape's own Shift+Alt+drag, so that Ctrl
meant "add" consistently everywhere. Now that Shift always adds and Ctrl
always removes (per the rework), this spec's own add-to-lasso combination
is **Shift+Alt** — which happens to match Inkscape's own Shift+Alt+drag
exactly. No remaining deviation to call out; **Ctrl+Alt** now means
*remove* for the lasso, a genuinely new capability Inkscape's own Touch
Selector has no equivalent of.

16. Given the Select tool is active, when the maker holds Alt already at
    the press that starts a drag, and the pointer then moves more than
    criterion 8's 3px threshold from the press point while Alt stays held,
    then a freehand line is drawn live as visual feedback following the
    pointer, with no selection change yet — this happens **regardless of
    whether the press point landed on an object or on empty canvas**
    (unlike the marquee, criterion 8, which only arms over empty canvas).
    Per "Alt's two jobs" above, holding Alt at press locks the gesture as a
    lasso for the whole drag; Alt pressed only *after* a drag has already
    started as a box does not retroactively turn it into a lasso
    (criterion 11 covers that case instead).
17. Given the press point of an Alt-drag lands directly on an object (within
    criterion 1's 8px tolerance), then it still becomes a lasso per
    criterion 16, never a grab-to-move of that object — Alt held down at
    press always means "draw a touch-line," even on top of an object; the
    existing plain-press move-drag (`canvas-navigation-and-selection`
    criterion 20) only ever starts from a press with no Alt held.
18. Given the maker releases the mouse button at the end of an Alt-drag
    (criteria 16-17), then every object whose outline comes within criterion
    1's 8px tolerance of any point along the drawn line is selected at that
    moment, replacing the current selection — the same outline-proximity
    test `hit_test_object` already uses for a click, evaluated along the
    whole line rather than at one point.
19. Given the maker holds Shift in addition to Alt for the whole drag, then
    criterion 18's result is **added** to the current selection on release
    rather than replacing it, identically to the marquee's Shift behaviour
    (criterion 12). Given the maker holds Ctrl in addition to Alt for the
    whole drag instead, then criterion 18's result is **strictly removed**
    from the current selection on release instead, identically to the
    marquee's Ctrl behaviour (criterion 13) — the lasso has no "contain"
    mode for Alt to invert (it only ever tests "touch"), so Alt's
    mode-inversion role is moot here; only its lasso-arming role (criterion
    16) and Shift's/Ctrl's combine roles apply. If both Shift and Ctrl are
    held, Ctrl takes precedence, matching the marquee's own resolution of
    that combination.
20. Given an Alt-drag's line never comes within tolerance of any object
    (e.g. drawn entirely through empty canvas, or entirely through a
    primitive's interior, filled or not, without crossing its outline), then
    nothing is selected on release, and the effect on the prior selection
    depends on which combine modifier was held at release: for a plain
    Alt-drag (no Shift or Ctrl), the prior selection is cleared, matching
    criterion 9's "replaces" behaviour for a marquee that touches nothing;
    for Alt+Shift or Alt+Ctrl, the prior selection is left exactly as it
    was (adding nothing, or removing nothing, is a no-op either way).

## Out of scope

- **True curve-vs-box intersection for the marquee.** "Touches" and
  "fully contained" are both evaluated against each object's own bounding
  box (`object_bounds`), not its exact outline geometry, for every marquee
  criterion above. A long diagonal stroke's bounding box can extend into
  the drag rectangle even where the stroke itself does not — an accepted,
  standard simplification (see rationale above), not a bug to fix here.
- **Filled-interior-based touch/contain testing**, for both the marquee and
  the lasso. Decided 2026-10-08, now that `stroke-and-fill-styling` has
  shipped: the marquee tests the object's (oriented) bounding box and the
  lasso tests outlines only, even over a filled shape. A lasso that passes
  through a filled interior without crossing its outline selects nothing
  (criterion 20). A fill-aware lasso is a separate story if wanted.
- **A pure z-order Alt-click cycle.** Decided 2026-10-08: the cycle order
  is the one stated under "Disambiguating overlapping candidates" (plain
  click answer, other outlines nearest first, the filled shape(s) F under
  the point, then the objects F hides). The two ordering rules the earlier
  revision flagged (nearest-first here, topmost-first in
  `stroke-and-fill-styling`) are reconciled in `hit_test_objects`, whose
  first element always equals `hit_test_object`.
- **Toggle semantics for Ctrl, matching LightBurn's literal behaviour**
  (LightBurn's Ctrl alone toggles objects the box catches). This spec
  implements the customer's own explicit request instead: Ctrl always
  strictly removes, never toggles. Resolved 2026-10-05 — see "Open
  questions."
- **Auto-scroll/auto-pan while a marquee or lasso drag nears the canvas
  edge.** A real feature some tools add, not asked for here.
- **Touch-screen gestures** for either the marquee or the lasso (e.g. a
  two-finger drag). Desktop mouse/trackpad only, per `CLAUDE.md`'s platform
  order, same deferral `canvas-navigation-and-selection` already made.
- **Any other press order against `object-transform`'s handles.** Decided
  2026-10-08, now that `object-transform` has shipped. A press on the
  Select tool is classified in this order: Alt held starts the lasso
  (also on a handle, no handle drag starts; `unified-object-editing`
  criterion 35); else a drawn handle starts its drag; else a press inside
  the sole selected object's box starts a move, unless Shift or Ctrl is
  held; else an outline hit within criterion 1's tolerance; else the
  marquee. Ctrl copy-move (`edit-interaction-polish` criterion 38) stays on
  the outline and on the centre handle.
- **Oriented box for rotated objects.** Built: since `object-transform` an
  object's box is the oriented (rotated) box. The marquee's "contain"
  (criterion 10) means all four corners of that box lie inside the marquee
  rectangle; "touch" (criterion 9) tests the axis-aligned bounding box
  around those four corners, not the oriented box itself. This is not a
  true rotated-rectangle intersection test, by choice.
- **Node-level disambiguation cycling** (the customer's "node, object,
  path..." phrasing also names nodes). The Node tool's own node/handle
  picking already has its own 16px tolerance and its own tie-break rule
  (`canvas-navigation-and-selection`); extending Alt-click cycling to nodes
  specifically is a separate decision for whoever next touches the Node
  tool, not bundled into this Select-tool-scoped slice.
- **Keyboard-only invocation** of either gesture. Both are mouse-drag only,
  matching every reference tool cited.

## Open questions

Both open questions below are resolved as of 2026-10-05. The customer
confirmed LightBurn's documented behaviour (no mode-swap modifier) was
correct and found the original swap/add scheme ("Shift swaps, Ctrl adds")
annoying on reflection. The resolution replaces that scheme entirely with
the independent Shift-adds/Ctrl-removes/Alt-inverts-mode design specified
above (see "Modifier scheme, reworked 2026-10-05" under Marquee); neither
original question's options (a)/(b) were taken as-is — the customer's own
new proposal superseded both. No open questions remain for this spec.

## UX notes

### Marquee box styling

New semantic tokens (`docs/design-system.md`), not a reuse of `--accent`:
`--marquee-touch` (`#2FAE57`, green) and `--marquee-contain` (`#E5484D`,
red). Neither is a repurposed existing color — nothing in the system so far
means "touch" or "contain," and `--accent` is reserved for actual selection
state (§"One accent, two states"), which this is not: the box itself is
never selected, it is a transient tool overlay.

Treatment confirms LightBurn's own convention: a semi-transparent fill at
12% opacity (`--marquee-touch-fill`/`--marquee-contain-fill`) plus a solid
1.5px border in the same mode color. The fill keeps whatever canvas content
sits under the box legible while dragging (important here specifically
because Shift-add and Ctrl-remove both leave prior selections' own boxes
visible at the same time, so the maker can see what they are adding to or
subtracting from); the border is weighted 1.5px, heavier than the 1px
bounding-box selection outline, so the drag box reads as the topmost,
currently-active layer rather than blending with any selected object's own
box it happens to overlap.

### Lasso line styling

Same color family as the marquee border — specifically always
`--marquee-touch` (green), never red: the lasso's selection rule is
outline-touch only (table above, "touch only, no mode to invert" for every
lasso row), so it never has a "contain" state to color for, and reusing
green rather than inventing a third hue keeps one consistent meaning for
that color ("touch-mode selection in progress") across both gestures.

Distinctness comes from line style, not color: the lasso renders as a
**dashed** line (4px on / 3px off, same 1.5px weight as the marquee
border), where the marquee box border is solid. A maker glancing at the
screen mid-drag sees "green dashed freehand trail" vs. "green or red solid
box" — different enough to tell the gestures apart at a glance, without
needing a second color vocabulary for what is, underneath, the same
touch-selection logic the marquee's green state already uses.

### Alt-click cycle feedback

No separate transient highlight distinct from the final selection
indicator. Each Alt-click is a discrete, already-committed action — unlike
a pen/node/shape drag (live preview *before* commit-on-release) or a
marquee/lasso drag (same), a click has no in-progress state to preview: the
moment it registers, the next candidate *is* selected, so the normal
`--accent` selection box appearing on that candidate immediately is already
the feedback. This follows `path-merge-split`'s "no unneeded animation for
discrete actions" precedent, not the drag-preview precedent — there is
nothing to preview ahead of, only a sequence of committed selections, and
adding a flash/highlight on top of a selection change that already redraws
the accent box would be animating something already visibly different.

### Cursor changes

Three cursor states over the canvas while the Select tool is active,
**revised by the 2026-10-05 modifier rework** (see "Alt's two jobs" under
Marquee above) so the cursor reflects the gesture Alt actually locked in at
press, not live Alt state during an already-started box drag:

- **Normal** (no button down; or hovering with no drag yet and Alt not
  held): the existing Select-tool arrow/move cursor, unchanged
  (`canvas-navigation-and-selection` criterion 20).
- **Marquee-armed / dragging** (mouse down on empty canvas, Alt **not held
  at press**, whether or not movement has started yet — including for the
  rest of the drag even if Alt is pressed afterward to invert the box's
  mode, criterion 11): standard `crosshair` cursor, unchanged from before
  this rework — the same convention LightBurn, Illustrator and Inkscape's
  own rubber-band select all use. Pressing Alt mid-box-drag does **not**
  switch this to the lasso cursor below — the gesture is already locked as
  a box at press, and the box's own color/mode change is the feedback for
  Alt's effect here, not the cursor.
- **Lasso-armed / dragging** (Alt held **at press**, before any drag
  starts, with or without a drag yet in progress): the custom lasso-glyph
  cursor (small freehand-loop icon, same custom-cursor mechanism
  `object-transform` already uses for its rotated resize cursors), applied
  the instant Alt is pressed while hovering with no drag active, so the
  maker knows which gesture they're about to get *before* they commit to
  the drag — unchanged reasoning from before this rework, matching
  Inkscape's own behavior of changing the Selector's cursor on Alt-down.
  Once a drag has started as a lasso this way, the cursor stays the lasso
  glyph for the rest of that drag even if Alt is released mid-drag (the
  gesture is locked at press, same as the box case above; releasing Alt
  mid-lasso has no defined effect on the lasso itself, per "Alt's two
  jobs"). Reverts to Normal the moment Alt is released **while hovering
  with no drag in progress** — releasing Alt mid-lasso-drag does not revert
  the cursor, per the previous sentence.

Shift and Ctrl (in any combination, with or without Alt) change neither
cursor state above — they change the drag's *combine* behavior (replace,
add or remove), not which gesture is being performed or the box's mode,
and the modifier-state legend below already covers that distinction.
Keeping the cursor vocabulary to "which gesture, and for a box, which
mode" rather than "every modifier combination" matches the
`object-transform` cursor rule's own scope (cursors distinguish
manipulation kind, not every modifier nuance).

**Confirmed by the `ux-engineer`, 2026-10-05:** the plain crosshair stays,
with no added mode glyph on the cursor itself when Alt inverts a box's
mode mid-drag. The box's own color swap (green↔red) is already the
feedback for that inversion, and it sits exactly where the maker's
attention already is — right under the cursor, filling the whole drag
rectangle — so a second, smaller signal on the cursor glyph itself would
duplicate information already given more visibly, for no gain. It would
also break the cursor-vocabulary rule this section already states for
`object-transform` (cursors distinguish manipulation *kind*, not every
modifier nuance): mode-invert is a nuance of the box gesture, not a
different gesture, and the box recoloring is that nuance's one correct
feedback channel, not the cursor.

### Modifier-state legend

Yes — a small on-canvas text label, shown for the duration of any marquee
or lasso drag, positioned near the live cursor position. Same convention as
`primitive-shapes`' drag-to-create numeric readout (on-canvas, not
status-bar, so the maker's eyes don't have to leave the drag to read it),
styled with the existing chrome tones (`--marquee-legend-bg` /
`--marquee-legend-fg`, i.e. `--toolbar-bg`/`--toolbar-icon`) rather than a
new surface color.

Reasoning, **updated for the 2026-10-05 modifier rework**: color signals
the drag's *mode* only (touch = green, contain = red; 2 states, direction
and Alt's inversion both already fully reflected in that color per the
table above), but it does **not** signal the *combine* behaviour (replace,
add or remove — 3 states, one more than before this rework, now that
Ctrl's strict-remove exists) — Shift and Ctrl both render in exactly the
same green/red as an unmodified drag of the same direction/Alt-state, so
"am I about to replace, add to, or remove from the selection" has no color
signal at all. This gap matters more than before, not less: getting
Ctrl's new remove wrong on a crowded drawing now silently *deselects*
objects the maker meant to keep — a destructive mistake the old scheme's
add-only Ctrl could never make. The label reads e.g. `Touch · Replace`,
`Contain · −Remove`, `Touch (line) · +Add`, updating live as modifiers
change, the drag direction reverses, or Alt is pressed/released mid-drag
to invert a box's mode (criterion 14) — one line, matching the existing
on-canvas readout precedent rather than adding a new status-bar segment.

**Confirmed by the `ux-engineer`, 2026-10-05:** plain text alone is not
quite enough now that Remove is destructive, but the fix stays small and
reuses nothing new. The combine word gets a one-character prefix only —
`+Add`, `−Remove`, plain `Replace` (no prefix: it is the default,
non-modified state, so it needs no marker). No color change:
`--marquee-contain` (red) already means "contain mode" on the box itself,
and coloring the word "Remove" with that same token would overload one
color with a second, unrelated meaning (mode vs. combine) on the same
label — exactly the kind of reuse `docs/design-system.md` already avoids
elsewhere (`--marquee-touch`/`--marquee-contain` were deliberately
introduced rather than reusing `--accent`, for this same reason). A plain
`−` carries "subtraction" on its own, independent of hue, so it needs no
new token and stays legible for a maker who can't distinguish red from
green. This also settles the Shift+Ctrl tie-break's own legend question
(see "Combinations" under Marquee above): that combination renders the
identical `−Remove`, with no further distinction, since its effect is
identical to Ctrl held alone.

### Marquee over a selected object's box (`object-transform`, 2026-10-06)

Since `object-transform`, a press on empty canvas *inside the sole selected
object's oriented box* starts a **move**, not a marquee (a small unfilled
object could not be moved otherwise). Decision for criterion 8: a plain
marquee arms only outside that box. Shift and Ctrl held at press bypass the
move and arm the marquee, so additive and remove modes still work over a
large selected frame; Alt already locks a lasso at any press point
(criterion 16). `object-transform` bypassed for Shift only; this story adds
Ctrl. This is accepted behaviour (`edit-interaction-polish` criterion 37,
`unified-object-editing` criterion 35): Ctrl inside the sole selected box
arms the remove marquee, while Ctrl copy-move stays on the outline and on the
centre handle. A plain marquee that must start inside the box needs Esc first.

### Modifier badge of the Ctrl marquee (2026-10-07, from the `edit-interaction-polish` UX review)

`edit-interaction-polish` adds a plus badge (`MoveBadges.tsx`, "Modifier badge"
in `docs/design-system.md`) wherever a press with Ctrl would start a move: it
means "copy". The Ctrl marquee of this spec (it removes from the selection)
gets a **minus badge from the same component**, at the same place by the
pointer, so a Ctrl press never shows the copy plus where it would remove. The
badge shows wherever a Ctrl press would arm that marquee (the same press
classification, `PressTarget`), so the plus and the minus never show for the
same press. The minus badge also shows for Ctrl+Alt, where the press arms the
remove lasso (criteria 16 and 19).

### Status

With these notes filled in, this spec is ready for the architect's
`adrs.md`; flip Status to Ready once that file exists (if not already
present when this was written, the lead flips it).

## Links
Requirements: R-EDIT-010, R-EDIT-011 (`docs/requirements.md`)
PR:
