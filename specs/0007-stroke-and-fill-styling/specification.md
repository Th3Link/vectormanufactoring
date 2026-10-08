# Stroke and fill styling: width/dash/join/cap/color, solid and gradient fill

Status: Done
Delivered in four PRs: #54 (model, format version 7), #55 (rendering, hit-testing), #58 (properties panel), #59 (gradients).
Priority: Must
Origin: Customer

> **Superseded in part by [`style-panel-rework`](../style-panel-rework/specification.md)
> (customer decisions after trying this slice, 2026-10-08).** Gradient fills are
> removed completely (criteria 16 to 22, 34, 35 and every gradient clause), the
> panel is empty when nothing is selected and hides controls instead of disabling
> them, colour is 8-digit RGBA hex chosen inline with no popups, dash has a custom
> text line, and number fields become drag-or-type value fields. Each affected
> criterion range carries a note below. Where the two specs disagree,
> `style-panel-rework` wins. The text below is kept as the record of what was
> built.

## User value

As a maker I want to set stroke width, dash pattern, line join, line cap and
color, and set fill to none, a solid color, or a linear/radial gradient, on
any path or primitive shape, so that my drawing looks like the thing I'm
actually making (a thick cut outline vs. a thin engrave guide, a filled
engrave area vs. an open cut line) instead of every object looking identical
on screen, the way `path-node-editing` (slice 2) and `primitive-shapes`
(slice 3) left it — matching the styling power of Inkscape's Fill & Stroke
dialog, which is the dialog a maker switching tools gradually (R-SYS-006)
already knows how to use.

**One property set, not two.** Per `primitive-shapes/adrs.md`'s framing
("primitives and paths share one z-order, so they share one tree"), paths and
primitives already render through the same stroke path in `render-core`; this
slice gives both the same style properties, read the same way, so a maker
styling a rectangle and styling a hand-drawn path reach for the identical
controls. Nothing here is primitive-only or path-only.

**What we do differently from the tool this replaces:** nothing at the
interaction level that a maker would notice day one — stroke width, color,
join, cap and gradient fill all match Inkscape's own Fill & Stroke dialog
closely enough that this is a swap-in replacement, not a relearn. Where this
slice simplifies relative to Inkscape's full dialog (custom numeric dash-array
entry, pattern/swatch fill, per-stop gradient on-canvas drag handles, variable
miter limit, fill-rule toggle), that is named per criterion and in "Out of
scope", not silently dropped. What stroke/fill *means* for a laser job (which
color cuts, which engraves, kerf compensation) is explicitly not this slice's
job — that's `manufacturing-roles` and later slices; this slice is purely
"what does it look like."

## Acceptance criteria

### Shared style model (paths and primitives alike)

1. Given any path object (slice 2) or any primitive object — rectangle,
   ellipse, polygon or star (slice 3) — when the maker selects it, then the
   identical set of stroke and fill controls is available and produces the
   identical rendering behavior for both, with no property that exists for
   one object kind but not the other.
2. Given a primitive that has never been converted with "object to path",
   when the maker changes any stroke or fill property on it, then the
   primitive keeps its own shape parameters (width/height, rx/ry, point
   count, ratio, corner radius — slice 3) completely unchanged, the new style
   renders immediately, and no implicit "object to path" conversion is
   triggered by a style change alone.
3. Given any object created by slice 2 or slice 3 carrying their stated
   placeholder default (0.25 mm solid black stroke, no fill), when this
   slice ships, then that object renders identically to before until the
   maker explicitly changes one of its style properties — the default
   becomes one explicit, editable value rather than a hardcoded rendering
   path, but nothing already drawn changes appearance on its own.

### Stroke: width and on/off

*Superseded in part by `style-panel-rework` criteria 5 to 10: criterion 5's
disabled Dash, Join and Cap (and enabled Colour and Width) become hidden rows;
width 0 sets Paint None and keeps the last non-zero width.*

4. Given a selected path or primitive, when the maker sets its stroke width
   to a value V mm (V > 0), then its stroke renders at width V, and the
   value is still V after deselecting and reselecting the object.
5. Given a selected object, when the maker sets stroke width to exactly 0,
   or sets the stroke's Paint switch to **None** ("No stroke"), then the
   object renders with no stroke at all — its previously set width, color,
   dash pattern, join and cap values are kept stored but unapplied, so
   turning the stroke back on restores them unchanged (matches Inkscape's
   "X" no-paint stroke swatch). The panel's stroke section has a Paint
   switch with the two states None and Solid. While the stroke is off,
   editing the stroke colour or typing a non-zero width turns the stroke
   on again in that same commit (Paint becomes Solid, the other stored
   values unchanged). While every selected object has its stroke off, the
   Dash, Join and Cap controls are disabled, because they would have no
   visible effect; Colour and Width stay enabled. With some selected
   objects on and some off, Paint shows no state pressed and the rows stay
   enabled.

### Stroke: color

*Superseded in part by `style-panel-rework` criteria 11 to 16: the hex field is
8 digits (`#RRGGBBAA`) and 8 digits are no longer refused; 3 and 6 digits keep
the alpha. The integer-percent opacity rule of criterion 6 is kept.*

6. Given a selected object with a stroke enabled, when the maker sets the
   stroke's color (as a hex RGB value) and its alpha (0–100%) independently,
   then the stroke renders in that color composited at that alpha over
   whatever is beneath it on canvas, and both values persist and read back
   unchanged on reselecting the object. Hex entry accepts 3 or 6 digits,
   with or without `#`, in any case (`#F80` is `#FF8800`); 8 digits are
   refused with a message, because colour and alpha are independent. Alpha
   is an integer percent: a typed or dragged N (0–100) is stored as exactly
   N/100 and reads back as N, and a typed decimal is rounded to the nearest
   integer. A stored alpha that is not a whole percent (from a file or a
   peer) is shown rounded to the nearest integer and is not rewritten by
   viewing or by pressing Enter on the unedited field.

### Stroke: dash pattern

*Superseded in part by `style-panel-rework` criteria 28 to 33: the dash choice is
an inline preset group plus a text line for a custom pattern; there is no
read-only "Custom" entry (criterion 8) and criterion 9's "no UI for a custom array"
no longer holds. The stored format of criterion 9 is unchanged.*

7. Given a selected object with a stroke enabled, when the maker chooses
   "Solid" (the default), then the stroke renders as one unbroken line with
   no gaps.
8. Given a selected object with a stroke enabled, when the maker chooses one
   of this slice's named dash presets (at least "Dash", "Dot" and
   "Dash-Dot", in addition to "Solid"), then the stroke renders with
   repeating on/off segments whose lengths are fixed multiples of the
   object's current stroke width (Inkscape's own dash-scaling convention),
   and if the maker subsequently changes the stroke width, the dash segment
   lengths rescale to match, keeping the pattern proportioned the same way.
   The presets, as stored lists of on, off, ... in multiples of the width:
   Dash `[6, 4]`, Dot `[1, 3]`, Dash-Dot `[6, 3, 1, 3]`; every "on" is
   greater than 0. A stored pattern that matches none of the presets (from
   a file) is shown as "Custom" in the panel, read only, and is not a
   choice. At a small width and zoom a pattern whose period is under
   2 screen pixels draws solid; that is the display, not a stored change.
9. A dash pattern (solid or any preset) is stored as an ordered list of one
   or more on/off lengths expressed as **multiples of the stroke width**, not
   as fixed millimetre lengths — this is also how Inkscape's own dash field
   works, and it is what makes criterion 8's rescale-on-width-change behavior
   hold without re-deriving or rewriting the dash values on every width edit;
   "Solid" is the empty-list case. This slice does not require a UI for
   entering an arbitrary custom array (see "Out of scope"), but the stored
   format places no limit on it, so a later numeric editor can read and
   write the same field without a format change.

### Stroke: join

10. Given a selected object with a stroke enabled and at least one sharp
    corner vertex (a path's corner node, or a rectangle/polygon/star corner
    at zero effective radius), when the maker sets the join style to
    **miter**, **round** or **bevel**, then that corner renders with the
    matching SVG join shape — miter: the two stroke edges extended to meet
    at a point; round: a filled arc of radius = half the stroke width
    centered on the vertex; bevel: a flat line directly connecting the two
    stroke edges — and a rounded rectangle's curved corners (effective
    radius > 0, no sharp vertex) are unaffected by the join setting, since
    there is no vertex for it to apply to.
11. Given join = miter, when the ratio of the miter length (from the tip of
    the miter to the inner corner where the two stroke edges meet) to the
    stroke width exceeds the miter limit, then that corner renders as a
    bevel instead of an unbounded spike. The limit is the SVG and Inkscape
    default of 4, so joins sharper than about 29 degrees (the angle at
    which the ratio is 4) fall back to a bevel. This slice fixes the limit
    at 4 and does not expose it as a separate numeric control (see "Out of
    scope").

### Stroke: cap

12. Given a selected object with a stroke enabled and at least one open
    stroke end (an open path's two endpoints, or each visible segment end
    produced by a dash pattern), when the maker sets the cap style to
    **butt**, **round** or **square**, then every such end renders per that
    style — butt: the stroke ends exactly at the endpoint; round: a
    semicircular cap of radius = half the stroke width beyond the endpoint;
    square: a square cap extending half the stroke width beyond the
    endpoint — matching SVG's three cap definitions. A closed path's stroke
    has no open ends and is visually unaffected by the cap setting.

### Fill: none and solid

*Superseded in part by `style-panel-rework` criteria 6, 49 to 54: fill is None or
Solid only; the gradient sentences of criterion 13 are void. Criterion 14's hex
rule follows the 8-digit rule of `style-panel-rework`.*

13. Given a selected object, when the maker sets fill to **None**, then its
    interior renders with no fill at all (slice 2/3's original default),
    regardless of any solid color or gradient configured earlier, which is
    kept stored but unapplied — re-enabling a fill restores the last value.
    Switching the fill mode None to Solid restores the stored solid colour
    (black the first time); Solid to Linear and back to Solid loses neither
    the solid colour nor the gradient stops.
14. Given a selected object, when the maker sets fill to a solid color (hex
    RGB) and an alpha (0–100%) independently of the color, then its interior
    renders filled with that color composited at that alpha, using the
    nonzero fill rule (SVG's default; see "Out of scope" for even-odd). Hex
    and alpha entry follow the rules of criterion 6 (3 or 6 digit hex;
    alpha an integer percent stored as N/100).
15. Given an open path object, when the maker applies any non-None fill to
    it, then the fill renders as if a straight closing segment ran from its
    last node back to its first (SVG's own rule for filling open paths),
    without changing the path's stored open/closed state or its own stroke
    rendering, which still stops at the real endpoints.

### Fill: gradients — shared stop model

*Removed (criteria 16 to 20) by `style-panel-rework` criteria 49 to 54. Built in
PR 4, deleted in the rework; not to be implemented.*

16. A gradient (linear or radial) holds an ordered list of stops. Each stop
    has its own position (0.0–1.0 along the gradient), its own color (hex
    RGB), and its own opacity (0–100%) independent of the object's fill as a
    whole — a gradient fill has no separate top-level alpha the way a solid
    fill does; each stop's own opacity is what varies transparency along the
    ramp. The editor enforces 2–16 stops when the maker adds or removes a
    stop through its own controls (criteria 18–19); this range is an
    edit-time UI limit, not a standing invariant the stored format itself
    guarantees — a file or a merged document that ends up with a stop count
    outside 2–16 (e.g. two peers each adding stops to the same gradient
    concurrently) is not rejected on open or on merge, and rendering simply
    follows whatever stops are present, in position order. A stop's opacity
    is an integer percent in the editor (a typed N is stored as N/100,
    criterion 6); its colour is entered as 3 or 6 digit hex (criterion 6);
    its position is shown in percent with at most one decimal ("12.5") and
    stored as typed.
17. Given an object with fill set to a new linear or radial gradient, then
    it starts with exactly 2 stops — position 0.0 and position 1.0. Stop 0
    takes the object's stored solid fill colour (black if none was ever
    set) at 100% opacity; stop 1 is white at 100% opacity, or black at 100%
    if stop 0 is white. So Solid red to Linear gives red to white. In a
    multi-selection each object takes its own stored colour. The fill
    renders interpolating between the stops.
18. Given a gradient fill with at least 2 stops, when the maker adds a stop
    — by the "Add stop" button, or by clicking the gradient bar away from a
    thumb at a position (clamped to 0.0–1.0, coincident positions allowed)
    — then it is inserted into the stop list in position order, and the fill
    re-renders interpolating across all stops in position order. The "Add
    stop" button takes the midpoint of the widest gap (the gaps being 0 to
    the first stop, between neighbours, and the last stop to 1; a tie goes
    to the gap nearest the start), rounded to 0.1%. A bar click takes the
    clicked position. In both cases the new stop gets the colour and
    opacity the ramp has at that position, so adding a stop changes nothing
    on screen until it is edited. With 1 stop, the same rule applies
    against the ends 0 and 1; with 0 stops, "Add stop" creates one stop at
    50% in the stored fill colour at 100% opacity. Attempting to add a 17th
    stop is refused: at 16 or more stops Add and the bar click are
    disabled with the tooltip "A gradient holds at most 16 stops".
19. Given a gradient fill with more than 2 stops, when the maker removes one
    of them, then it is deleted from the list and the fill re-renders
    interpolating across the remaining stops; removing a stop when only 2
    remain is refused (a gradient always keeps at least 2).
20. Given any one stop of a gradient fill, when the maker edits its
    position, color or opacity, then only that stop's value changes, the
    fill re-renders live reflecting the new interpolation, and every other
    stop's position/color/opacity is unchanged. Position, colour (3 or 6
    digit hex) and opacity (integer percent) follow the entry rules of
    criteria 6 and 16; a position change re-sorts the list and the edited
    stop stays the selected one.

### Fill: gradients — linear and radial rendering

*Removed (criteria 21, 22) by `style-panel-rework` criteria 49 to 54.*

21. Given an object with fill set to **linear gradient**, then its interior
    renders with the stop colors interpolated along a straight axis; this
    slice's default axis runs across the object's own **selection box** (the
    oriented box the Select tool draws around it), at angle 0: from the
    left edge to the right edge of the box in the object's own frame (from
    (0, 0.5) to (1, 0.5) in box fractions). There is no control to redirect
    it in this slice (see "Out of scope"); a top-to-bottom ramp on an
    upright rectangle needs the object rotated. The gradient is stored
    without geometry, so it follows the selection box: it turns with a
    rotation, and re-fits after a skew or a resize. Stated limit, accepted,
    and shown in the panel as a muted line ("Gradient spans the shape's
    selection box, which is the square around a polygon or star.") whenever
    the selection holds a polygon or star in a gradient mode: for a polygon
    or star the selection box is the square that circumscribes it, not a
    tight box, so a triangle's ramp starts about a quarter of the way along,
    where the shape begins; and
    "Object to path" turns such a shape into a path whose selection box is
    tight, so its gradient re-fits at the conversion (rectangles and
    ellipses do not visibly change).
22. Given an object with fill set to **radial gradient**, then its interior
    renders with the stop colors interpolated outward from a center point to
    an outer edge; this slice's default center is the center of the
    object's own selection box and its radius is half the box (elliptical
    on a non-square box). The ramp's t runs from the center (0) to the edge
    (1). Same turning, re-fit and polygon/star and "Object to path" limits
    (and the same panel line) as criterion 21; no control redirects it in
    this slice.

### Fill affects hit-testing

*Changed by `style-panel-rework` criterion 51 for criterion 23: a fill paints
exactly when it is Solid; "linear or radial" and "at least one stop" are void.
Criteria 27 to 29 are unchanged.*

23. Given an object with any non-None fill (solid, linear or radial), when
    the maker clicks anywhere inside its filled interior — not on its
    stroke/outline — then the object is selected, the same as clicking its
    outline would select it; this matches Inkscape, Illustrator and Figma,
    and is what makes a filled shape with no stroke (or a stroke too thin to
    reliably click) still selectable by its visible area. Given an object
    with fill set to **None**, then its interior is not part of its
    clickable area — the maker can only select it by clicking its stroke (or
    whatever hit-test area slices 2/3 already defined), unchanged from
    before this slice. Which object wins when several are under the point is
    criterion 27. An object counts as filled exactly when it paints a fill:
    fill is on and, for a gradient, at least one stop exists; opacity 0 still
    counts. The interior of an open path is the area its fill paints, that
    is, closed with a straight chord from its last node to its first (as
    criterion 15); a closed path's interior is bounded by its real closing
    segment.

### Multi-object editing and persistence

*Criterion 25: the "multi-stop gradient" and "ordered stop list" clauses are void
(`style-panel-rework` criteria 49 to 54); the rest holds.*

24. Given two or more objects selected together (any mix of paths and
    primitives), when the maker changes one style property through the
    style controls (e.g. stroke color), then that property is applied to
    each selected object independently — every other style property on each
    object stays as it was — and the whole change commits as one atomic
    operation, the same discipline `primitive-shapes` criterion 22 already
    established for "object to path".
25. Given an object with any combination of stroke and fill properties set,
    including a multi-stop gradient, when the maker saves the project and
    reopens it (on the same machine or another, per `project-file-foundation`
    and R-SYS-003), then every one of those properties, including the full
    ordered stop list, reads back exactly as set.

### Draw order

26. Given a document with paths and primitives, when it is drawn, then all
    objects draw in one pass in tree (z) order, bottom to top, paths and
    primitives interleaved; each object paints its fill, then its stroke,
    over everything lower in the tree and under everything higher. **This is
    a visible change for existing files**: until now every path drew under
    every primitive, so a path above a rectangle in the tree drew under it;
    now it draws over it. With fills, a filled shape also covers what lies
    below it in the tree. No key is written and no file is rewritten; only
    the picture changes.

### Hit order with fills

27. Given a point under which several objects have a filled interior or an
    outline within the Select tool's outline tolerance, when the maker
    clicks, hovers or double-clicks it, then one object wins by this rule
    (an object with stroke off and fill None draws nothing but is still
    selectable by clicking its outline, and hover and click agree on it,
    because the outline is its geometry, not its paint):
    let F be the topmost object whose filled interior contains the point
    (criterion 23); among the objects at or above F in tree order (all
    objects when there is no F), the one whose outline is nearest to the
    point within the tolerance wins, an exact distance tie going to the
    topmost; if no outline is within tolerance, F wins. An object below F
    never wins, because F covers it. When no object under the point is
    filled, this is exactly the rule accepted before this slice (nearest
    outline, a tie going to the topmost), with no change. Example: a filled
    rectangle with a hollow ellipse above it; a click on the ellipse's
    outline selects the ellipse, a click inside the ellipse but away from its
    outline selects the rectangle. Example: a hollow circle below a filled,
    opaque rectangle; a click on the part of the circle's outline that lies
    under the rectangle selects the rectangle.
28. Given the Select tool is active, when the maker presses, then the order of
    `unified-object-editing` criterion 35 applies with one change: where that
    criterion's clauses say "outline hit", they now mean "outline or
    filled-interior hit by criterion 27". With exactly one object selected
    the order is: a drawn handle starts that handle's drag; else, with Shift
    held, an outline or filled-interior hit toggles that object in the
    selection (this is what makes Shift-click add-to-selection work over a
    filled shape); else a plain press inside the selected box starts a move
    (subject to criterion 29); else an outline or filled-interior hit
    selects, Shift toggles; else the marquee starts. Consequences, all
    intended: a drag that starts inside an unselected filled shape moves
    that shape instead of starting a marquee; a Shift press inside the sole
    selected box away from every object still finds nothing. Hover and
    double-click use the same hit function, so the hover box also appears
    over a filled interior, and a double-click inside a filled path's
    interior hands it to the Node tool as a double-click on its outline does.
    Hover also follows the press: at a point inside the sole selected box
    where a press would move that object, hover lights no other object (a
    small fix, because filled shapes make today's mismatch visible), with
    one exception: where criterion 29 sends the press to a filled T above
    the selected S, hover lights T. In short, hover always lights the
    object a press at that point would select, and nothing where a press
    would move the selection or start a marquee. The cursor mirrors the
    press: the outline-hit cursor where a press would select an object
    (outline or filled interior), the move cursor where it would move the
    selected object, the arrow on empty canvas; no new cursor for a filled
    interior.
29. **Customer-visible change to an accepted behaviour** (decided by the lead
    on 2026-10-07; the customer may veto): given exactly one object S
    selected (filled or not) and a plain press (no Shift) at a point inside
    S's selection box, when the point also lies in the filled interior of
    another object T that is above S in tree order, then the press goes to T
    instead of moving S: T becomes the selection, and a drag from that press
    moves T. Hover at that point lights T (criterion 28). The centre move
    handle of the selected object S takes press priority over a filled T
    lying exactly under it (drawn handles come first, criterion 28); zooming
    in gets around it. This is intended. When there is no such T, the press moves S as before. If several
    objects qualify as T, the topmost wins. The outline of another object
    never takes a press inside the selected box (unchanged from
    `unified-object-editing` criterion 35), and neither does an unfilled
    object or one below S. Example: a 100 mm filled rectangle R is selected
    and a 10 mm filled ellipse E lies on top of it; pressing inside E
    selects E (a drag moves E), pressing on R away from E moves R. The older
    order would move R in both cases, leaving no way to reach E without
    deselecting first. If the customer vetoes this, the press goes back to
    the older order; that is one condition removed from the press code.

### Style across duplicate, split, join and "Object to path"

*Criteria 30 to 33: the gradient clauses (stops copied, ramp restarting per half,
re-fit at conversion) are void (`style-panel-rework` criteria 49 to 54); the rest
holds.*

30. Given a path or primitive with any stroke and fill style, including a
    multi-stop gradient, when the maker copies it (Ctrl-drag copy or a typed
    copy), then the copy has exactly the same style, the same gradient stops
    included; editing the copy's style leaves the original unchanged, and
    editing the original leaves the copy unchanged.
31. Given an open path with any style, when the maker splits it at a node
    into two objects, then both objects have the same stroke and fill style
    as the original (a gradient fill included; each half's gradient spans its
    own selection box, so the ramp restarts per half). Splitting a closed
    path keeps the same object and changes nothing in its style.
32. Given two paths joined at their endpoints, when the join completes, then
    the surviving path keeps its own style unchanged and the other path's
    style is discarded with it (a filled path joined onto an unfilled
    surviving path leaves an unfilled path). Closing a path onto its own
    other end keeps its style.
33. Given a primitive with any style, when the maker converts it with "Object
    to path", then the path has the same stroke and fill style, gradient
    stops included. The only visible change is the gradient re-fit that
    criterion 21 names for polygons and stars.

### Gradients across a multi-selection, and degenerate stop lists

*Removed (criteria 34, 35) by `style-panel-rework` criteria 49 to 54. Files that
hold gradient data: `style-panel-rework` criterion 53.*

34. Given two or more objects selected together that are all in the same
    gradient fill mode with the same stop count, when the maker edits a stop
    in the stop list, then the change is applied to the stop of the same rank
    in each object (rank = order by position; stops at the same position rank
    in the order they were created), every other stop of each object is
    unchanged, and the change is one atomic commit. When the selected
    objects differ in fill mode or in stop count, the stop list is not
    shown. What the editor displays for a multi-selection:
    - Same fill mode and same stop count: the stop **list** is shown, row k
      editing the stop of rank k of every object; a field whose values
      differ shows "Mixed" (a differing colour swatch a diagonal hatch).
      The gradient **bar** shows the ramp and its thumbs only when all the
      objects' stop lists are identical in value; otherwise it is a neutral
      hatched track with no thumbs. Add stop and Remove are hidden for more
      than one object, because they would break "same stop count".
    - Same mode, different stop counts: the Fill mode row shows that mode
      and the editor is replaced by the message "Selected gradients have
      different numbers of stops."
    - Different modes, or a mix of gradient and non-gradient: the Fill mode
      row shows no state pressed. Picking a mode then applies to all
      objects, each object keeping its own stops (new ones are created per
      object where it has none, criterion 17).
35. Given a document in which a gradient fill holds 0 stops, 1 stop or more
    than 16 stops (for example from two peers editing the same gradient
    concurrently, criterion 16), then it is not rejected, no error is
    shown and the document is not changed: with 0 stops no fill is painted
    and the object's interior is not clickable (criterion 23); with 1 stop
    the interior is painted uniformly in that stop's colour and opacity.
    Beyond the first and the last stop the colour of that stop continues
    (the "pad" edge rule). The panel shows these states as follows. 0
    stops: an empty checkerboard bar, an empty list and the line "No stops.
    Nothing is painted. Add a stop."; Add stop is enabled. 1 stop: one thumb
    and one row, the bar a flat colour, both editable; Remove is disabled.
    More than 16 stops: every stop gets a row, Add stop is disabled, Remove
    works down to 2. Remove is disabled at 2 stops or fewer with the
    tooltip "A gradient keeps at least 2 stops".

### Live preview and the panel's selection scope

*Superseded in part by `style-panel-rework`: in criterion 36 the colour-popover
sentences are void and the typed-value and no-arrow-stepping rules are replaced by
the value field (criteria 34 to 48); preview, one commit on release, Escape
revert, key-up commit and "commit goes to the objects the edit started on" are
kept. In criterion 37 the "disabled state" is replaced by an empty panel
(criteria 1 to 4).*

36. Given the Select tool's geometry preview (a resize, rotate, skew or move
    drag, and the bar's edits), when the maker is mid-drag, then it stays what
    `unified-object-editing` criteria 10 to 15 define: a hollow blue outline
    of the new geometry over the object, which keeps its committed style; no
    fill or stroke-width preview is drawn. The gradient box and a scaled
    stroke width apply on release. In contrast, when the maker drags a panel
    control (slider, colour area, stop position), the object itself is drawn
    in the new style on every move (ephemeral, not in the document), and
    exactly one commit follows on release, even if the pointer is outside
    the control by then; dragging away and back makes no commit in between.
    Panel previews are coalesced to one update per animation frame. The two
    cannot happen at once, because there is one pointer. Further rules:
    - **Escape during a panel drag** drops the preview, the object returns
      to its committed style, and the release then writes nothing. Escape
      does not undo a drag already released (there is no undo yet; each
      release is one commit).
    - **Keys on a slider, colour area or gradient thumb** (arrows, Shift =
      10x) preview on key-down and commit on key-up, so a held key is one
      commit.
    - **The commit goes to the objects the edit started on**, not to
      whatever is selected at release time. A canvas press that changes the
      selection while a colour popover is open dismisses the popover first
      (the press is not swallowed); a pending drag still commits to the
      old objects. A selection or tool change while a popover is open
      closes it.
    - **Typed values** (width, hex, opacity, position): no preview while
      typing. Enter commits and returns focus to the canvas; Tab commits
      and moves to the next field; Escape or a press elsewhere restores the
      shown value, writes nothing, and (Escape) returns focus to the
      canvas. Enter on a field the maker did not edit writes nothing.
      Invalid input keeps focus, selects the text and shows a message.
    - **Discrete controls** (toggle groups, selects, Add stop, Remove, the
      Paint switch): one click is one commit, no preview.
    - **Multi-selection:** each of the above is one commit for the whole
      selection (criterion 24).
37. Given the Node tool is active with nodes selected, then the panel edits
    the paths that own the selected nodes; given the Node tool is active
    with no node selected, then the panel edits the paths in the object
    selection (the path being edited); given the Pen tool is active, or
    nothing is selected (and, in the Node tool, no path either), then the
    panel shows its disabled state (every control disabled and not
    focusable, at the frozen defaults: 0.25 mm black solid stroke on, Dash
    Solid, Miter, Butt, no fill). With the Select tool, the panel edits the
    object selection. Choosing the rectangle, ellipse or polygon/star tool
    clears the selection, so while a creation tool is active the panel
    shows its disabled state, the same as for an empty selection. A drawn
    shape returns to the Select tool at once with the new object selected,
    so a maker can draw a rectangle and colour it immediately, without
    changing tool. The panel's subject line says what is being edited ("Nothing
    selected", "Pen: finish the path to style it", "Rectangle", "3
    rectangles", "4 objects", "2 paths"). The polygon/star tool keeps its
    own floating options bar; this slice adds no "Shape tool options"
    section to the panel.

### Panel keys and focus

*Superseded in part by `style-panel-rework` criteria 57 and 60: no popover or
select exists to close, the Escape order is in criterion 60, the gradient-thumb
sentence is void. The rest is kept.*

38. Given focus is in any panel control (field, select, toggle, slider,
    colour area, gradient thumb, button), when the maker presses
    Backspace, Delete or a letter key, then the key edits the field or does
    that control's own action and **never reaches the canvas**: it does
    not delete the selected object and does not switch the tool, with each
    of the Select, Node and a creation tool active. Given focus is on a
    gradient thumb (or its stop row's thumb), when the maker presses Delete
    or Backspace, then only that stop is removed (ignored at 2 stops or
    fewer) and the object is untouched. Given the maker presses Shift+Ctrl+F
    (Shift+Cmd+F on macOS) anywhere in the window except during a running
    canvas drag, then the panel expands if collapsed and focus moves to its
    first enabled control (the stroke Paint switch), or to the panel itself
    when every control is disabled; the shortcut never collapses the
    panel. Escape in the panel closes an open popover or select first,
    else restores an edited field and returns focus to the canvas, else
    just returns focus to the canvas; it never clears the selection. After a
    mouse interaction with a button-like control (toggle item, select
    choice, Add, Remove, a popover closed by the pointer) focus returns to
    the canvas so that tool letters work again; keyboard activation keeps
    focus where it is.

### Panel layout

39. Given the application window at any width from the minimum, then the
    canvas region and the 280 px properties panel sit side by side below
    the menu, the panel running down to the status bar; the tool rail and
    the bars' overlay row are anchored to the canvas region, so no bar,
    chip or readout is ever drawn under the panel (the bars wrap sooner
    instead). The window minimum is 800 x 600 (**Proposal**: the
    implementer measures the widest unbreakable bar row at 800 px and
    raises the minimum if one does not fit, rather than shrinking the
    panel). Given the maker collapses or expands the panel (16 x 48 px
    tab on its canvas-facing edge, first Tab stop in the panel; per
    session, default open, not saved), then the document does not move on
    screen: the view keeps its top-left origin and only the right edge
    reveals or hides canvas. The panel never auto-collapses and never
    becomes an overlay; when it is taller than the window it scrolls as a
    whole.

### Legibility of editor lines over fills

40. Given a selected, hovered or previewed object over a fill of black,
    white, the accent blue, mid-grey (`#808080`), red (`#FF0000`) or yellow
    (`#FFDC00`), then every accent-coloured editor line that has no white
    ground of its own is drawn over a white casing one line width wider on
    each side, under the accent line and above the artwork: the selection
    box (dashes only; rhythm and pixel snapping unchanged), the hover box,
    the blue preview outline (casing 1.5 px each side; always drawn above
    all artwork, including objects above the original in the tree), the
    rotate and skew glyphs, the Bézier handle lines, the Node tool's
    segment overlay, the skew fixed-line guide, the pivot marker and the
    guides. Handles that already have a white ground (resize, centre and
    parameter handles, node glyphs) are unchanged, and so are the marquee
    and lasso. Measured from the rendered buffer, the better of line and
    casing is at least 3:1 against each fill above for the selection box and
    the preview outline, and about 2:1 or better for the hover box on all
    six reference fills (weakest measured: yellow 1.97:1, accepted by the
    customer-delegated lead decision of 2026-10-08). On the plain
    canvas colour the lines look as before.
41. **Customer-visible change to an accepted look** (told to the customer,
    default applies): the hover box is raised from 20% to 65% accent
    (alpha 166/255), for both the line and its white casing, everywhere,
    filled or not. It stays solid (the selected box stays dashed), so
    hovered and selected stay apart. Measured from the rendered buffer, the
    better of line and casing is about 2:1 or better on all six reference
    fills of criterion 40; weakest measured: yellow 1.97:1, accepted by the
    customer-delegated lead decision of 2026-10-08 (red 2.1, canvas about
    2.0 to 2.5, others higher; on a fill with no stroke the line falls on
    the canvas pixel beside the edge).
    At 20% it measured 1.0 to 1.3:1 on every fill and on the canvas, which
    is no hover feedback over a filled shape. If the customer vetoes this,
    the hover box returns to 20% on unfilled objects and the casing rule of
    criterion 40 stays; that is one token (`--hover-box`) changed.

## Out of scope

- Undo/redo of any operation in this slice — `undo-redo` (slice 8). Every
  edit here is still one well-formed commit per interaction (one style-field
  change, one stop add/remove/edit, one multi-object batch), the same
  discipline every prior slice has kept, so slice 8 has a clean, single
  commit per action to attach undo to — it just isn't reachable yet.
- Boolean operations, grouping, layers — slices 8 and 9.
- How stroke/fill maps to a laser job (which color means cut vs. engrave,
  power/speed per color, kerf compensation) — `manufacturing-roles` (slice
  13) and later. This slice is appearance only.
- Custom numeric dash-array entry in the UI (typing an arbitrary on/off
  sequence rather than picking a preset). Criterion 9's stored format
  supports it; exposing it is a later refinement if a maker asks for a dash
  pattern none of the presets cover.
- A separate "miter limit" numeric control. Fixed at 4 (criterion 11),
  matching Inkscape's and SVG's own default; Inkscape does expose this as an
  adjustable field, but no acceptance criterion needs it adjustable yet.
- Fill-rule toggle (nonzero vs. even-odd). Nonzero only (criterion 14) —
  even-odd mainly matters for self-intersecting or multi-subpath geometry,
  and this product has no multi-subpath paths until boolean operations
  (slice 9) can produce them.
- Pattern fill and swatch/texture fill (Inkscape's other two fill types
  beyond flat color and the two gradients). Not needed for a laser-cutting
  MVP; revisit if a maker workflow asks for it.
- Variable-width ("Power Stroke") strokes, and start/mid/end markers
  (arrowheads and similar) on a stroke. Neither is named by R-EDIT-005/006.
- Any control that redirects a gradient: an angle field, numeric endpoints
  or on-canvas handles. This slice ships the default axis of criteria 21–22
  only; an angle field in the panel is the next story (customer question 2,
  default applies).
- Recent colours, a swatch library, an eyedropper (no web API in the Linux
  webview), CMYK and HSL colour entry.
- Stepping a field with the arrow keys (width, opacity, position): the
  caret moves; sliders and thumbs do take arrow keys (criterion 36).
- A "Shape tool options" section in the panel (criterion 37).
- Gradient spread/edge behavior beyond the default "pad" (clamp to the first
  and last stop's color past the gradient's own extent) — Inkscape's
  "reflect"/"repeat" edge modes are not reproduced here.
- Shared/linked gradient definitions across multiple objects (Inkscape's
  "fork on edit" shared `<linearGradient>` defs, so copying a gradient to a
  second object and editing one doesn't affect the other by default; here
  every object's gradient is its own independent copy from the start, which
  is simpler, not a reduction of any stated criterion).
- Styling text. No text tool exists yet (same deferral `primitive-shapes`
  stated for "object to path").
- Stroke/fill behavior under SVG export/import — `svg-import-export`
  (slice 11) is where this slice's properties first need to round-trip
  through actual SVG markup; this slice's persistence criterion (25) is
  about the project's own `.curvyo` file only.

## Dependencies and sequence

Refreshed 2026-10-07 against `main`: slices 5 and 6, `unified-object-editing`,
`object-transform-refinements`, `edit-interaction-polish`,
`shape-creation-from-center`, `polygon-star-box-refit` and the Curvyo rename
are merged, and their statuses read Done. Technical detail is in the
2026-10-07 readiness check at the end of `adrs.md`.

- **Delivery in four stacked PRs.** A maker sees nothing before PR 2 and
  can try styling only from PR 3.
  1. **Model**, no visible change: one `style` object on every object
     node, the stop list, validation, `format_version`, fixtures. Storage and
     commands of criteria 2 to 6, 9, 13, 16 to 20, 24, 25, 30 to 33.
  2. **Rendering and hit-testing**: tree-order drawing (the visible change
     of criterion 26), stroke on/off, alpha, dash, join, cap, solid fill, hit
     order, press, hover, double-click. Criteria 6 to 8, 10 to 15, 23, 26 to
     29.
  3. **Panel** with stroke and solid fill: the first demo. Criteria 1, 2,
     4 to 9, 13, 14, 24, 36 to 39 through the UI, plus 40 and 41 (the
     casing and hover box over fills), which belong with the rendering the
     panel makes visible.
  4. **Gradient**: stop editor, linear and radial. Criteria 16 to 22, 34, 35.

  Until the gradient slice (PR 4), a gradient fill is hit-testable (hover
  and select work) but is not painted, so such a shape looks hollow.

  The UX notes below are done (2026-10-07), so all four PRs can start.
- **`format_version`**: `main` is at 5, so this slice takes **6**,
  provisionally: the PR that merges first takes `main`'s value plus one
  (`rectangle-corner-radii` and `ellipse-arcs-and-shaping`, both Draft, also
  want a bump). PR 1 defines the complete format in one go, so no build
  between the PRs writes a file that another build of this slice misreads.
  Files at version 5 open with every object in the default style
  (criterion 3).
- **`advanced-selection` follows this slice.** Its criteria 3 to 5 (plain
  click, Alt-click cycling) are reworded to the cycle order that spec states
  under "Disambiguating overlapping candidates": the winner of criterion 27,
  then the other outlines within tolerance nearest first, then the filled
  shape(s) under the point, then the objects they hide. That order, not a
  "remaining outline hits, then interior-only hits" split, is the one that
  applies. `unified-object-editing` criterion 35 gets the 8 px rewording.
  Those edits are made in those two specs, not here.
- **Live preview**: as in criterion 36. A geometry drag keeps the blue
  outline with no fill or stroke preview; a panel drag draws the object
  itself in the new style.
- **Gradient frame**: the selection box, as in criterion 21. A tight box for
  polygons and stars is the separate story `polygon-star-box-refit` already
  names, not part of this one.

## UX notes

*Superseded in part by `style-panel-rework`: sections 2 (Select dropdown, Fill
mode with gradients, disabled rows), 3 (disabled state), 4 (colour popover, hex
rule, eyedropper "decided no"), 5 (gradients, entire), 7 (popover and typed-value
rules), 8 (popover names) and 9 (gradient question 2). The ux-engineer rewrites
them in that spec.*

Decided 2026-10-07 (ux-engineer), against `main` at `e285c2d`, after the
architect's readiness check (`adrs.md`, "2026-10-07 readiness check"). Sizes,
tokens and component rules are in `docs/design-system.md`, section "Properties
panel: Style section", and in the token rows that section adds; this part holds
the decisions and the behaviour. It replaces the 2026-10-05 notes. Kept from
them: the docked panel, 280 px, collapsible, `Shift+Ctrl+F`, one shared
`ColorAlphaPicker`, mixed-state placeholders, live preview with one commit on
release. Dropped as stale: the "Shape tool options" section (the panel is built
with the Style section only), the stop editor as a list without a gradient bar,
and the overlay row anchored at the window edge.

This is the first properties UI that is not a tool-scoped bar: styling is
orthogonal to drawing, so it must be reachable with any tool active. That is why
it is a docked panel and not a bar or a dialog (a dialog would have to be
reopened for every object).

### 1. Where things live, and the layout that keeps them apart

One rule: **a property lives in exactly one place.** Appearance (stroke, fill)
is the panel's. Geometry parameters and tool settings stay in the bars.

| Control | Place | Why |
|---|---|---|
| Stroke and fill (this slice) | `PropertiesPanel`, "Style" section | Appearance, tool-independent |
| "Scale stroke width", "Scale corner radius" | Select bar, settings group | Tool settings, not object properties; unchanged |
| Radius, Remove rounding, Points, Ratio, Object to path | Select bar, kind groups | Geometry of the selection; unchanged (`unified-object-editing`) |
| Node actions | Node bar | Unchanged |
| Polygon/star creation options | Polygon/star bar | Unchanged; the panel gets no "Shape tool options" section |

**Layout (the part the architect found broken).** `App.tsx` has two regions
side by side below the menu: the canvas region (`relative flex-1 min-w-0`) and
the panel (280 px, `shrink-0`, full height down to the status bar). The tool
rail and the bars' overlay row move *into* the canvas region and are anchored to
it (`absolute top-3 left-3` for the rail; `absolute top-3 right-3 left-[72px]`
for the overlay row, now measured from the canvas region's own right edge). So a
bar can never run under the panel: its right limit is the canvas region's right
edge minus 12 px, at every window width. Bars already wrap by whole groups; they
simply wrap sooner. Chips, readouts and hints render inside the canvas
container and clamp to it, so they need no change (verify at build). Popovers and tooltips of the
panel render in a portal over everything and may cover the canvas, never the
panel.

- **Opening and closing the panel** resizes the canvas through the existing
  `ResizeObserver` path. The document must not move on screen when it does: the
  view keeps its top-left origin, the right edge reveals or hides canvas. Check
  this at build (a regression test belongs to the slice, because it is the first
  thing that resizes the canvas by choice).
- **Collapse tab:** 16 x 48 px, on the panel's canvas-facing edge, vertically
  centred, chevron. Collapsed, the panel content is gone (`inert`, not in the tab
  order), the tab stays at the canvas region's right edge. It is the first Tab
  stop in the panel, so a keyboard user can collapse it. `Shift+Ctrl+F` only
  ever expands and focuses (never closes). The state is per session, default
  open, not saved (there is no preferences store yet).
- **Narrow windows.** Minimum window 800 x 600 (`tauri.conf.json`
  `minWidth`/`minHeight`; none is set today). At 800 px the canvas region is 520
  px, the Select bar's first row (two switches, about 400 px) still fits in the
  436 px between the rail and the edge, and later groups wrap. The implementer
  measures the widest unbreakable row of the three bars; if one exceeds the
  canvas region at 800, the window minimum goes up, the panel does not shrink.
  The panel never auto-collapses and never becomes an overlay. Vertically it
  scrolls as a whole (`overflow-y: auto`, one scrollbar, no inner scroll areas).

### 2. The Style section: rows and layout

Panel padding 12 px, content 244 px (a thin scrollbar takes the rest of 280),
rows 28 px high with 8 px between rows, 16 px between the two subsections. Label
column 60 px, 8 px gap, control column 176 px. Text 14 px (`text-sm`) in rows,
12 px in the stop rows and the subject line. Everything sits on `--panel-bg`
(`--toolbar-bg`); fields are white with a 1 px border (`--toolbar-icon` at 60%,
3.1:1 on the panel), as in the bars.

**Header.** "Style" (14 px semibold) and, below it, a muted **subject line** that
says what the panel is editing: "Nothing selected", "Pen: finish the path to
style it", "Rectangle", "3 rectangles", "4 objects" (mixed kinds), "2 paths"
(Node tool, from selected nodes). One line, 12 px, `--panel-muted-fg` (5.0:1 on the
panel). It exists because the scope
differs by tool (section 3) and a maker should see it, not infer it.

**Stroke** (sub-heading "Stroke", 12 px semibold):

| Row | Control | Notes |
|---|---|---|
| Paint | `ToggleGroup` None / Solid, 2 x 44 px, icons (slashed square, filled square) with names "No stroke", "Solid stroke" | The on/off. Same shape as Fill's mode row. Replaces the earlier "swatch is the on/off" idea: one visible switch, and the swatch is only a colour |
| Color | Swatch 28 px (opens the picker), hex field 84 px, opacity field 56 px with fixed "%" | Hex and opacity are inline so the common edits need no popover |
| Width | Number field 96 px with fixed "mm" | Up to 3 decimals shown ("0.125"); decimal point or comma; 0 is "no stroke" (AC 5) |
| Dash | `Select` 176 px: line sample 64 x 8 px and the name | Solid, Dash, Dot, Dash-Dot; "Custom" (read only, not a choice) when a file holds another pattern |
| Join | `ToggleGroup` 3 x 40 px: Miter, Round, Bevel | Glyphs are the Inkscape ones |
| Cap | `ToggleGroup` 3 x 40 px: Butt, Round, Square | |

**Fill** (sub-heading "Fill"):

| Row | Control | Notes |
|---|---|---|
| (none) | `ToggleGroup` None / Solid / Linear / Radial, 4 x 44 px, 32 px high, icons | No label column: the group is the section's first row and is as wide as the control column plus the label column |
| Color | As stroke | Solid only |
| Gradient | Gradient bar, stop list, Add stop | Linear and Radial only (section 5) |

**Dash presets.** Stored as multiples of the stroke width (AC 9), every "on"
greater than 0 (a zero dot shows nothing under butt caps):

| Preset | Stored list (on, off, ...) | Why |
|---|---|---|
| Solid | `[]` | AC 7 |
| Dash | `[6, 4]` | Period 10 w: at 0.25 mm and 100% zoom a dash is 5.7 px on, 3.8 px off |
| Dot | `[1, 3]` | A square dot of w x w under butt caps; under round caps 2 w long with 2 w of gap left, so it still reads as dots |
| Dash-Dot | `[6, 3, 1, 3]` | Period 13 w |

At 0.25 mm and below about 50% zoom the period of Dot falls under 2 screen
pixels and it draws solid (the architect's rule); that is the display, not a
bug, and the tooltip of the dash row says so ("Patterns scale with the stroke
width and draw solid when too small to see"). The line samples in the trigger
and the list are drawn at 2 px thickness with the same ratios (Dash 12 / 8,
Dot 2 / 6, Dash-Dot 12 / 6 / 2 / 6 px), in `--toolbar-icon`.

**Stroke off (Paint = None).** The rows keep their place (the panel's shape
does not change) and show the stored values. Color and Width stay enabled: an
edit of either turns the stroke on again in the same commit (typing a non-zero
width, choosing a colour), which is AC 5 as the ADR states it. Dash, Join and Cap
are disabled while *every* selected object has its stroke off, because they
would have no visible effect and nothing to turn on. Join and Cap are otherwise
always enabled, also on an object with no corner or no open end (AC 10, 12):
they are stored properties. The colour swatch of an off stroke shows the stored
colour with a slash over it (white 3 px under `--no-paint-slash` 1.5 px, visible
on any colour); the tooltip says "Stroke is off. Choose a color to turn it on."

**Solid fill.** Fill None shows nothing below the mode row (AC 13). Switching
None to Solid restores the stored colour (black the first time, the frozen
default); Solid to Linear to Solid loses neither the colour nor the stops
(AC 13).

### 3. States, scope and mixed values

**Scope (criterion 37, confirmed with one addition).**

| Active tool | The panel edits | Subject line |
|---|---|---|
| Select, Rectangle, Ellipse, Polygon/star | The object selection | kind or count |
| Node | The paths that own the selected nodes; with no node selected, the paths in the object selection (the path being edited); disabled only if there is neither | "2 paths" |
| Pen, or nothing selected | Nothing: disabled | "Pen: finish the path to style it" / "Nothing selected" |

The Node fallback is the addition to criterion 37: in the Node tool with a path
open and no node picked the panel would otherwise go grey while a path is plainly
being edited. A creation tool with a selection edits it, so a maker can draw a
rectangle and colour it without leaving the tool.

**Disabled (nothing to edit).** Every control stays in place at the frozen
defaults (0.25 mm, black, solid stroke on, Dash Solid, Miter, Butt, fill None),
`aria-disabled`, and does not take focus. A disabled field loses its white
ground (`--field-disabled-bg`) and shows its value in `--field-disabled-fg`
(4.6:1); labels stay at full `--toolbar-icon` (8.3:1). A disabled `ToggleGroup`
keeps its pressed item with a `--toolbar-icon` 30% ground, not the accent (4.9:1
for the glyph). The accent means "you can change this".

**Mixed values (multi-selection).** Per property, never "first selected wins":

| Control | Mixed look |
|---|---|
| Number or percent field | Empty, placeholder "Mixed" in `--field-placeholder` (5.3:1 on white) |
| Swatch | Diagonal hatch (45 degrees, 4 px stripes, white and `--mixed-hatch`, 3.3:1 between stripes), not a checkerboard. **A change to the earlier house rule:** transparent (opacity 0) is drawn as a checkerboard, and a "mixed" swatch that was also a checkerboard would be indistinguishable from it |
| Hex field | Placeholder "Mixed" |
| `ToggleGroup`, Select | No item pressed; the Select trigger reads "Mixed" in the placeholder colour |
| Gradient | Section 5 |

Colour and opacity are separate properties, so one can be shared while the other
is mixed. A typed or chosen value applies to every selected object and each
keeps its other properties (AC 24); that holds from a mixed start too. Stroke
rows are disabled only if every object has its stroke off; with some on and some
off, Paint shows nothing pressed and the rows stay enabled.

### 4. Colour picking

- **Swatch** (28 x 28 px, `rounded-[5px]`): the colour at its opacity over a
  checkerboard (`--checker-a` white, `--checker-b` `#C9C9CE`, 7 px cells), a 1 px
  `--swatch-border` (`--toolbar-icon`, 8.3:1 on the panel) so a white, a black or
  a grey swatch is bounded against the panel, and a 1 px white inner line so a
  near-black colour is not lost in the dark border. Click, Enter or Space opens
  the popover.
- **Popover** (`react-colorful`, the architect's default: MIT, no dependencies,
  keyboard support; the lead justifies it in the PR): 232 px wide, 12 px padding,
  non-modal, opens to the **left** of the panel (`side="left"`, `align="start"`,
  8 px offset, 8 px collision padding) so it never covers the row it came from
  and the inline hex and opacity fields stay visible and update live. Content,
  top to bottom: the saturation/value area (208 x 128), the hue slider (12 px
  high, 16 px thumb), the opacity slider (12 px high, checkerboard under the
  ramp). It holds no fields of its own: hex and opacity are the inline ones.
  Names: "Saturation and value", "Hue", "Opacity".
- **The picker's hue must not jump** when the pointer drags through a grey or
  black (hue is undefined there). Keep the picker's own HSV state and re-derive it
  from the hex only when the hex changes from outside it; test: drag the area to
  the left edge and back, the hue slider stays where it was.
- **Hex field:** `#RRGGBB`, shown upper case; accepts with or without `#`, any
  case, and the 3-digit short form. 8 digits are refused ("Use 6 digits; set
  opacity separately"), because colour and opacity are independent (AC 6).
  Invalid: `--field-invalid` border and a message chip below the field (an
  overlay, so no row shifts), cleared on the next keystroke.
- **Opacity** is an integer percent, 0 to 100. Stored as a fraction. **Rounding
  rule:** a typed or dragged N is stored as N / 100 exactly, so it reads back as
  N; a stored value that is not on the grid (a file, a peer) is *shown* rounded to
  the nearest integer and is never rewritten by looking at it (Enter on an
  unedited field writes nothing). A typed decimal is rounded to the nearest
  integer. The same for a stop's opacity. Stop positions are shown in percent
  with one decimal at most ("12.5") and stored as typed.
- **No paint** is the Paint row, not a swatch item. **Transparent** is opacity 0
  (a checkerboard swatch), still a paint (AC 23: a zero opacity fill is
  selectable).
- **Out of scope here, decided no:** recent colours (the PO's open "remember last
  style" item covers that ground), a swatch library, an eyedropper (no web API in
  the Linux webview), CMYK and HSL entry.
- **Contrast with the chrome.** Swatch, fields, toggles and the gradient bar all
  carry a 3:1 border against the panel; the pressed `ToggleGroup` item is
  `--toolbar-icon-active-bg` with a white glyph (4.5:1), 3.3:1 against the panel.

### 5. Gradients

**Defaults (AC 17, 18, 21, 22).**

- **New gradient colours.** Stop 0 takes the object's stored solid fill colour
  (black the first time), opacity 100%; stop 1 is white at 100%, or black if the
  fill colour is white. So Solid red to Linear gives red to white, a ramp that is
  opaque and visible against the canvas, with no hidden transparency. Each object
  of a multi-selection takes its own colour (AC 17 already creates the stops per
  object).
- **Default axis and extent.** Linear runs left to right in the object's own
  frame, across its selection box, angle 0 (the ADR's (0, 0.5) to (1, 0.5)).
  Radial is centred on the box centre with radius half the box, elliptical on a
  non-square box. Both turn with a rotation and re-fit after a skew or a resize
  (AC 21, 22). No redirect control in this slice; see the customer question.
- **Added stop (AC 18).** The "Add stop" button inserts into the **widest gap**
  (the gaps are 0 to the first stop, between neighbours, last stop to 1; ties go
  to the first from the top), at its midpoint rounded to 0.1%, with the colour
  and opacity the ramp has *there*, so adding a stop changes nothing on screen
  until it is edited. A click on the gradient bar (not on a thumb) adds a stop at
  the clicked position, same colour rule. With 1 stop: the same rule against the
  ends. With 0 stops: one stop at 50% in the fill colour, 100% opacity.
  "1.0 minus an offset" is dropped.
- **Limit 16** (AC 16, 18): at 16 or more stops, Add and the bar click are
  disabled with the tooltip "A gradient holds at most 16 stops". A merged
  document can hold more; then the list shows all of them, Add stays disabled,
  Remove works.

**The stop editor** is a gradient bar plus a list, both over the same stops.

- **Bar** (244 x 16 px, checkerboard under it, 1 px `--toolbar-icon` 60% border):
  the ramp drawn from the stops as an inline SVG `linearGradient`, which
  interpolates sRGB without premultiplying, as the renderer does (a CSS gradient
  premultiplies and would show a different ramp for stops with opacity). Under
  it, one **thumb** per stop (12 x 16 px pin, filled with the stop's colour, 1.5
  px `--toolbar-icon` outline). Drag moves the stop (live preview, one commit on
  release); click selects; the selected thumb is raised and ringed in `--accent`
  with a white casing. `role="slider"`, arrow keys 1%, Shift 10%, Home and End
  0 and 100%; Delete or Backspace on a focused thumb removes that stop (ignored
  at 2 or fewer). Coincident stops overlap as thumbs; the list is how they are
  told apart. For Radial the bar still shows the ramp along t (centre to edge).
- **List**, in position order (the ADR's stable sort; list order breaks ties),
  one 28 px row per stop: position (52 px, "%"), colour swatch (24 px), hex (72
  px), opacity (48 px, "%"), remove (28 px). All of a stop's values are editable
  in its row, so the keyboard needs nothing else. A row is **selected** by
  focusing any control in it or clicking the row; the selection is the thumb that
  is raised. Selection is ephemeral, held as `(NodeId, StopId)` (ADR), and
  survives the row moving when a position change re-sorts the list. The panel
  scrolls for 16 rows (about 450 px); nothing inside it does.
- **Add stop** (button below the list, 28 px, full width) and **Remove** (the
  row's button; the Delete key on a thumb). Remove is disabled at 2 stops or
  fewer (AC 19), with the tooltip "A gradient keeps at least 2 stops".
- **0, 1 or more than 16 stops (AC 35, a merged document).** 0: the bar shows an
  empty checkerboard, the list is empty and a line says "No stops. Nothing is
  painted. Add a stop." 1: one thumb and one row, the bar a flat colour; both
  editable, Remove disabled (at 2 or fewer). More than 16: all rows, Add
  disabled. None of these states is an error and none changes the document.
- **Multi-selection (AC 34).** The stop editor shows when every selected object
  has the same fill mode and the same stop count: the **list** is by rank (row k
  edits the stop of rank k of every object), a field that differs shows "Mixed",
  a differing swatch the hatch. The **bar** shows the ramp and the thumbs only
  if all the objects' stop lists are identical in value; otherwise it is a neutral
  hatched track with no thumbs, and the list is the way to edit. Add and Remove
  are hidden for several objects (they would break "same count"). Same mode but
  different counts: the Fill mode row shows that mode, the editor is replaced by
  "Selected gradients have different numbers of stops." Different modes (or a mix
  of gradient and not): the mode row shows nothing pressed. Picking a mode then
  applies to all, each object keeping its own stops (new ones are created per
  object where it has none).
- **Known limit, shown in the panel.** A polygon or star's selection box is the
  square around it, not a tight box (AC 21), so a triangle's ramp starts about a
  quarter of the way along. When the selection contains a polygon or star and the
  mode is a gradient, a muted 12 px line under the editor says: "Gradient spans
  the shape's selection box, which is the square around a polygon or star."
  "Object to path" gives a tight box and the ramp re-fits (not shown in the UI;
  `docs/technical-debt.md`).

### 6. Visual consequences of fills

Fills put artwork under the lines the editor draws on top. Every number in
`docs/design-system.md` was measured on the canvas colour only. Measured now
(WCAG contrast, 1 px `--accent` line against the fill, and white against it):

| Fill | `--accent` line | White |
|---|---|---|
| Black | 4.6 | 21.0 |
| White | 4.5 | 1.0 |
| `--accent` blue | 1.0 (invisible) | 4.5 |
| Mid grey `#808080` | 1.1 | 3.9 |
| Red `#FF0000` | 1.1 | 4.0 |
| Yellow `#FFDC00` | 3.3 | 1.4 |
| Navy `#141E50` | 3.5 | 15.8 |
| Canvas `#E8E8EB` | 3.7 | 1.2 |

So the accent line alone fails on blue, grey and red fills. **Rule (casing): every
`--accent` line or glyph stroke that has no white ground of its own is drawn over
a white casing, one line width wider on each side** (`--selection-casing`,
`#FFFFFF`), under the accent line and above the artwork. White and accent are
4.5:1 to each other, so the pair reads on any fill, and the better of the two is
at least 3.3:1 on every fill in the table. On the canvas the casing is nearly
invisible (1.2:1) and the line looks as it did.

- **Selection box (dashed, 1 px):** casing under the dashes only (the 4 / 3
  rhythm and the pixel snapping do not change). On a blue fill you see two thin
  white lines, on any other fill the blue line.
- **Hover box: raised from 20% to 65% accent (alpha 166/255), line and casing
  alike (customer question, below).** At 20% the hover box measured 1.0 to 1.3:1 on
  every fill in the table and on the canvas, which is no feedback, and AC 28
  promises hover feedback over a filled interior. At 65% the better of line and
  casing is about 2:1 or better on all six reference fills (weakest measured:
  yellow 1.97:1, accepted by the customer-delegated lead decision of
  2026-10-08; red 2.1, canvas about 2.0 to 2.5, others higher). It stays solid, so
  selected (dashed) and hovered (solid) stay apart. New token `--hover-box`;
  `--accent-hover` (20%) stays for ring, button and row backgrounds.
- **Blue preview outline (1.5 px):** casing 1.5 px each side, always drawn above
  all artwork, including objects above the original in the tree, so a hollow
  outline is never hidden by a fill.
- **Handles:** the resize, centre and parameter handles and the node glyphs have a
  white ground and need nothing. The rotate and skew glyphs (transparent ground),
  the Bézier handle lines, the Node tool's segment overlay, the skew fixed-line
  guide and the pivot marker get the casing. The marquee and lasso are not
  changed (they are a drag mode, not a selection state); the same casing is the
  fix if one is ever reported.
- **Black old, blue new, with fills.** Unchanged: the committed object stays
  exactly as it renders (fill, stroke, tree position, under whatever is above it)
  and the blue outline of the new geometry is drawn over everything. An object
  with a fill and no stroke has no black outline to compare against; its fill
  silhouette is the "old". No fill is previewed (AC 36).
- **Ctrl-copy and duplicate.** The preview is the hollow blue outline at the
  copy's position, as before. On release the copy lands **directly above its
  original** in tree order (`duplicate_objects`: A, A', B), so a filled copy
  dragged over a higher object B ends up *under* B. The blue outline cannot show
  that. Accepted, listed as a known look for the demo message.
- **Draw order change (AC 26)** is visible in existing files: a path above a
  rectangle now draws over it. It needs one line in the demo message, no UI.
- **Cursor.** The cursor mirrors the press: where a press would select an object
  (outline or filled interior), the same cursor an outline hit has today plus the
  hover box; where it would move the selected object, the move cursor as today;
  empty canvas, the arrow. No new cursor for a filled interior. Where AC 29
  applies (inside the selected box, over a filled T above it) the press goes to
  T, so **hover lights T**, which means AC 28's sentence "hover lights no other
  object" has that one exception.

### 7. Live preview, commit, focus and keys

**Preview and commit (AC 36).**

- **Drags in the panel** (opacity slider, colour area, hue slider, a gradient
  thumb): the selected objects render in the new style on every pointer move,
  coalesced to one update per animation frame, through the ephemeral
  `StyleOverride`; **one commit on pointer-up**, even if the pointer is outside
  the control. Dragging away and back makes no commits in between.
- **Escape during a drag reverts**: the override is dropped, the object returns to
  its committed style, and the release then writes nothing (the same rule as every
  canvas drag). Escape does not undo a drag that was already released; there is no
  undo yet, and each release is one commit.
- **The commit goes to the objects the edit started on**, not to whatever is
  selected at the release. A canvas press that changes the selection while the
  picker is open dismisses it first (the press is not swallowed, as with the entry
  chips); the commit of a pending drag still targets the old objects. A selection
  or tool change while a popover is open closes it.
- **Keyboard on a slider, area or thumb:** arrow keys preview on key-down and
  commit on key-up (the Ratio slider's rule), so a held key is one commit.
- **Typed values** (width, hex, opacity, position): no preview while typing.
  **Enter commits** and returns focus to the canvas; **Tab commits** and moves to
  the next field (a form that is tabbed through keeps focus); **Escape or a press
  elsewhere** restores the shown value, writes nothing, and (Escape) returns focus
  to the canvas. This is the entry-chip and bar-field rule plus Tab. Enter on text
  the maker did not edit writes nothing. Invalid input keeps focus, selects the
  text and shows the message; no stepping with arrow keys in this slice (the caret
  moves).
- **Discrete controls** (`ToggleGroup`, Select, Add and Remove stop, Paint): one
  click is one commit; no preview.
- **Multi-selection:** every one of the above is one commit for the whole
  selection (AC 24).

**Focus return.** A mouse interaction with a button-like control (`ToggleGroup`
item, Select choice, Add, Remove, a popover closed by the pointer) returns focus
to the canvas afterwards, as the tool rail does, so the letter keys keep working;
keyboard activation keeps focus where it is. A popover opened from the keyboard
returns focus to its swatch on close, one opened with the pointer to the canvas.
Escape in the panel: closes an open popover or select first; else restores a
field and returns to the canvas; else just returns focus to the canvas. It never
clears the selection and never reaches the canvas Escape cascade. Fields follow
the rule above.

**Keyboard map.**

| Key | Where | Does |
|---|---|---|
| `Shift+Ctrl+F` (Cmd on macOS) | Anywhere in the window | Expands the panel if collapsed and moves focus to its first enabled control (Paint of Stroke); with every control disabled, to the panel itself (`tabindex="-1"`, it reads the subject line). Never closes the panel. Ignored while a canvas drag is running |
| Tab / Shift+Tab | Panel | Collapse tab, Stroke Paint, Color (swatch, hex, opacity), Width, Dash, Join, Cap, Fill mode, then Fill color or the gradient bar thumbs and each row (position, swatch, hex, opacity, remove), Add stop |
| Arrows | `ToggleGroup` | Roving tabindex, one Tab stop per group; arrows move and select |
| Space, Enter, arrows, type-ahead | Select | Opens and picks |
| Arrows (Shift = 10x) | Colour area, hue, opacity, thumb | Preview on key-down, commit on key-up |
| Enter, Tab, Escape | Fields | As above |
| Delete, Backspace | Thumb | Removes the stop (more than 2) |
| Delete, Backspace, letters | Any panel control | **Never reach the canvas**: they edit the field, or do the control's own thing, and never delete the selected object or switch the tool |

**The keyboard gate (the rule of `edit-interaction-polish`: a focused control
blocks the shortcuts, confirmed).** The panel is a sibling of the canvas
container, not inside it, and its popovers render in a portal outside both, so the
canvas `onKeyDown` never sees its keys; `isFormControl` also blocks
`input`, `select`, `button` and `[role="switch"]` as today. Add `[role="slider"]`,
`[role="radio"]`, `[role="combobox"]`, `[role="option"]` and `[role="dialog"]`
to that list as a second guard (the colour area, the thumbs and the `ToggleGroup`
items are not form controls by tag). Test (the architect's): Backspace and Delete
in the width, hex, opacity and position fields leave the selected object, and a
letter key in a field types, with each of Select, Node and a creation tool
active. The window-level Shift and Ctrl listener (`applyModifiers`) still runs
while the panel has focus; that is harmless and expected.

### 8. Names, tooltips and the rail/panel interplay

- The panel is `<aside aria-label="Properties">`; the section is a `<section>`
  labelled "Style"; the subject line is not a live region.
- **Accessible names (every one visible text or an `aria-label`):** "Stroke
  paint" group with "No stroke", "Solid stroke"; "Stroke color" swatch button
  (value in its description: "#2F6FEE, 100% opacity" or "mixed"), "Stroke color
  hex", "Stroke opacity percent", "Stroke width, millimetres", "Stroke dash
  pattern" (options "Solid", "Dash", "Dot", "Dash-dot"), "Stroke join" group
  ("Miter join", "Round join", "Bevel join"), "Stroke cap" group ("Butt cap",
  "Round cap", "Square cap"); "Fill type" group ("No fill", "Solid fill",
  "Linear gradient", "Radial gradient"), "Fill color", "Fill color hex", "Fill
  opacity percent"; "Gradient" group, thumbs "Stop 2 of 3 position" with
  `aria-valuenow` and `aria-valuetext` "40%, #2F6FEE"; rows "Stop 2 position
  percent", "Stop 2 color", "Stop 2 color hex", "Stop 2 opacity percent",
  "Remove stop 2"; "Add stop"; popover parts "Saturation and value", "Hue",
  "Opacity"; collapse tab "Hide properties panel" / "Show properties panel"
  (`aria-expanded`, `aria-controls`).
- **Tooltips** (Radix `Tooltip`, 400 ms, `side="left"`, so they open over the
  canvas and never over the next row): on icon-only controls (`ToggleGroup`
  items, the swatches, Remove, the collapse tab) and on disabled or limited ones
  (why). Texts are plain: "Miter join: a sharp point; becomes a bevel at very
  sharp angles", "Round join", "Bevel join: a flat cut", "Butt cap: ends at the
  endpoint", "Round cap", "Square cap: extends half the width past the endpoint",
  "No fill", "Solid fill", "Linear gradient", "Radial gradient", collapse tab
  "Properties (Shift+Ctrl+F)". Labelled fields get none.
- **Rail and panel.** The rail is on the left over the canvas, the panel on the
  right; rail tooltips open right, panel tooltips left, so they never meet. A rail
  click while a field has unconfirmed text is a press elsewhere (restores). Tool
  letters are ignored while focus is in the panel; Escape then a canvas click
  returns to them. Switching tool changes the subject line and, for Pen, disables
  the panel; an open popover closes first.
- **Contrast of the pieces:** panel text 8.3:1 on `--panel-bg`; placeholder 5.3:1
  on white; disabled value 4.6:1; field border 3.1:1; focus ring 2 px
  `--editor-accent` with a 1 px `--toolbar-bg` offset on every control (the switch
  ring), never the grey `--ring`.

### 9. Questions for the customer

Batched, each with its default (nothing blocks on them).

*Both questions: told to the customer, default applies. Question 1 is
criterion 41; question 2 is the Out of scope entry on gradient direction.*

1. **Hover box strength.** Fills make the 20% hover box invisible (measured
   1.0 to 1.3:1 on every fill). *Option A (recommended, the default):* raise the
   hover box to 65% accent with a white casing at 65%, everywhere (at least 2:1
   on every reference fill, still softer than the selection). *Option B:* keep 20% on unfilled objects and use A
   only where the hovered object is filled (two looks to learn). *Option C:* keep
   20% and accept no hover feedback over filled shapes. This changes an accepted
   look (the customer called the hover "dezent").
2. **Gradient direction.** This slice has no control for it: a linear ramp always
   runs left to right in the object's own frame, so a top-to-bottom ramp on an
   upright rectangle needs the rotation of the object. Cheapest remedy: an angle
   field in the panel (needs one register, `fill_linear_axis`, no migration);
   on-canvas handles after that. *Recommendation:* ship this slice as specified
   and make the angle field the next story. *Default:* as specified.

### 10. Changes the criteria need (for the PO)

1. **AC 5, 13:** the stroke has a Paint None/Solid switch; typing a non-zero width
   or choosing a colour while the stroke is off turns it on in the same commit;
   Dash, Join and Cap are disabled while every selected stroke is off.
2. **AC 6, 14, 16, 20:** opacity is an integer percent, 0 to 100, stored as N / 100
   (reads back as set); hex accepts 3 and 6 digits, 8 digits are refused; a stop
   position is shown in percent with one decimal.
3. **AC 8:** the presets are Dash `[6, 4]`, Dot `[1, 3]`, Dash-Dot `[6, 3, 1, 3]`
   (multiples of the width, every on greater than 0); Solid is `[]`. A stored
   pattern outside the presets reads "Custom" in the UI.
4. **AC 17:** stop 0 is the stored solid fill colour (black if never set) at 100%,
   stop 1 white at 100% (black if stop 0 is white).
5. **AC 18:** the position is "chosen" by a click on the gradient bar, or by the
   Add stop button, which takes the midpoint of the widest gap (0.1% rounding);
   the new stop gets the ramp's colour and opacity at that position. With 0
   stops, one stop at 50% in the fill colour.
6. **AC 21, 22:** angle 0 is along the box's x axis in the object's frame; add the
   panel line about polygons and stars as a stated look.
7. **AC 28, 29:** the hover sentence of AC 28 ("hover lights no other object")
   gets AC 29's exception: where the press would go to a filled T above the
   selected S, hover lights T.
8. **AC 34:** the editor shows for the same fill mode and the same stop count; for
   the same mode and different counts the mode row shows the mode and a message
   replaces the editor; for different modes the mode row shows nothing pressed.
   Add and Remove are hidden for more than one object.
9. **AC 35:** the 0, 1 and more-than-16 displays of section 5 (Add disabled at 16
   or more, Remove disabled at 2 or fewer, rows still editable).
10. **AC 36:** add "Escape during a panel drag reverts and the release writes
    nothing"; "keys on a slider commit on key-up"; "the commit goes to the objects
    the edit started on"; "updates are coalesced per frame".
11. **AC 37:** the Node-tool fallback (selected nodes' owners, else the object
    selection); a creation tool with a selection edits it; the subject line.
12. **New, panel keys:** Backspace, Delete and letters in any panel control never
    reach the canvas (tests in section 7); Delete on a focused gradient thumb
    removes that stop only.
13. **New, layout:** at window widths from 800 px a bar never overlaps the panel;
    opening or closing the panel does not move the document on screen; minimum
    window 800 x 600.
14. **New, legibility over fills:** the selection box, the hover box, the blue
    preview outline and the rotate and skew glyphs stay visible over a black,
    white, `--accent` blue, mid-grey, red and yellow fill (the better of line and
    casing at least 3:1 for the box and the preview; 2:1 for hover), measured from
    the GL buffer as in the UX reviews; hover is 65% accent with casing (question
    1).
15. **Out of scope, add:** recent colours, swatch library, eyedropper, width
    stepping with the arrow keys, a gradient angle control (question 2), a
    "Shape tool options" panel section.
16. **Wording:** every "`StylePanel`" in the criteria and the PR descriptions is
    "the Style section of `PropertiesPanel`"; the "Needs UX notes before Ready"
    section below is answered.

## Needs UX notes before Ready

Answered 2026-10-07 by the UX notes above; nothing in this list is open on the
UX side. Where each gap is decided:

| Gap | Decided in |
|---|---|
| 1. Default gradient colours; "Add stop" position rule | UX notes 5 ("Defaults", "Added stop"), criteria changes 4, 5 |
| 2. Dash preset ratios, line samples | UX notes 2 ("Dash presets"), criteria change 3 |
| 3. Contrast of box, hover box, preview and handles over fills | UX notes 6 (casing rule, hover 65%), question 1, criteria change 14 |
| 4. Panel rows, tokens, popover, alpha percent, narrow window | UX notes 1, 2, 4, `docs/design-system.md` "Properties panel: Style section" |
| 5. Stop list: multi-selection, 0, 1, more than 16, selection | UX notes 5, criteria changes 8, 9 |
| 6. Scope under Node and Pen, focus return, Backspace and Delete | UX notes 3, 7, criteria changes 11, 12 |
| 7. Design-system corrections | `docs/design-system.md`, edited 2026-10-07 (shape options paragraph, panel and permanent bar side by side, overlay anchored to the canvas region) |
| 8. Default linear angle, radial centre and radius | UX notes 5 ("Default axis and extent"), criteria change 6, question 2 |

The PO applied "Changes the criteria need" (UX notes 10) on 2026-10-07 and set
the status to Ready: items 1 to 5 in criteria 5, 6, 8, 13, 14, 16, 17, 18 and
20; 6 in 21 and 22; 7 in 28 and 29; 8 and 9 in 34 and 35; 10 in 36; 11 in 37;
12 in the new criterion 38; 13 in 39; 14 in 40 and 41; 15 in "Out of scope";
16 is wording only (read "`StylePanel`" as the Style section of
`PropertiesPanel`). Existing criteria keep their numbers. The customer
questions (UX notes 9) have defaults, were told to the customer, and do not
block.

## Links
Requirements: R-EDIT-005, R-EDIT-006 (`docs/requirements.md`)
PR: TBD
