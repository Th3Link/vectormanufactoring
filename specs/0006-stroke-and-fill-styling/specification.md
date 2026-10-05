# Stroke and fill styling: width/dash/join/cap/color, solid and gradient fill

Status: Ready
Priority: Must
Origin: Customer

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

4. Given a selected path or primitive, when the maker sets its stroke width
   to a value V mm (V > 0), then its stroke renders at width V, and the
   value is still V after deselecting and reselecting the object.
5. Given a selected object, when the maker sets stroke width to exactly 0,
   or chooses "no stroke", then the object renders with no stroke at all —
   its previously set width, color, dash pattern, join and cap values are
   kept stored but unapplied, so turning the stroke back on restores them
   unchanged (matches Inkscape's "X" no-paint stroke swatch).

### Stroke: color

6. Given a selected object with a stroke enabled, when the maker sets the
   stroke's color (as a hex RGB value) and its alpha (0–100%) independently,
   then the stroke renders in that color composited at that alpha over
   whatever is beneath it on canvas, and both values persist and read back
   unchanged on reselecting the object.

### Stroke: dash pattern

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
11. Given join = miter on a corner sharp enough that the mitered point would
    extend more than 4 times the stroke width from the vertex (SVG's and
    Inkscape's default miter-limit ratio), then that corner renders as a
    bevel instead of an unbounded spike. This slice fixes the limit at 4 and
    does not expose it as a separate numeric control (see "Out of scope").

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

13. Given a selected object, when the maker sets fill to **None**, then its
    interior renders with no fill at all (slice 2/3's original default),
    regardless of any solid color or gradient configured earlier, which is
    kept stored but unapplied — re-enabling a fill restores the last value.
14. Given a selected object, when the maker sets fill to a solid color (hex
    RGB) and an alpha (0–100%) independently of the color, then its interior
    renders filled with that color composited at that alpha, using the
    nonzero fill rule (SVG's default; see "Out of scope" for even-odd).
15. Given an open path object, when the maker applies any non-None fill to
    it, then the fill renders as if a straight closing segment ran from its
    last node back to its first (SVG's own rule for filling open paths),
    without changing the path's stored open/closed state or its own stroke
    rendering, which still stops at the real endpoints.

### Fill: gradients — shared stop model

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
    follows whatever stops are present, in position order.
17. Given an object with fill set to a new linear or radial gradient, then
    it starts with exactly 2 stops — position 0.0 and position 1.0 — with a
    sensible default color pair (this slice does not mandate which colors;
    see "UX notes"), and the fill renders interpolating between them.
18. Given a gradient fill with at least 2 stops, when the maker adds a stop
    at a chosen position (clamped to 0.0–1.0, coincident positions allowed),
    then it is inserted into the stop list in position order with a default
    color/opacity, and the fill re-renders interpolating across all stops in
    position order; attempting to add a 17th stop is refused.
19. Given a gradient fill with more than 2 stops, when the maker removes one
    of them, then it is deleted from the list and the fill re-renders
    interpolating across the remaining stops; removing a stop when only 2
    remain is refused (a gradient always keeps at least 2).
20. Given any one stop of a gradient fill, when the maker edits its
    position, color or opacity, then only that stop's value changes, the
    fill re-renders live reflecting the new interpolation, and every other
    stop's position/color/opacity is unchanged.

### Fill: gradients — linear and radial rendering

21. Given an object with fill set to **linear gradient**, then its interior
    renders with the stop colors interpolated along a straight axis; this
    slice's default axis runs across the object's own bounding box (exact
    default angle, and whether the maker can redirect it by dragging an
    on-canvas handle vs. a panel control, is the ux-engineer's decision —
    see "UX notes" and "Out of scope").
22. Given an object with fill set to **radial gradient**, then its interior
    renders with the stop colors interpolated outward from a center point to
    an outer edge; this slice's default center and radius are derived from
    the object's own bounding box (same UX deferral as criterion 21).

### Fill affects hit-testing

23. Given an object with any non-None fill (solid, linear or radial), when
    the maker clicks anywhere inside its filled interior — not on its
    stroke/outline — then the object is selected, the same as clicking its
    outline would select it; this matches Inkscape, Illustrator and Figma,
    and is what makes a filled shape with no stroke (or a stroke too thin to
    reliably click) still selectable by its visible area. Given an object
    with fill set to **None**, then its interior is not part of its
    clickable area — the maker can only select it by clicking its stroke (or
    whatever hit-test area slices 2/3 already defined), unchanged from
    before this slice.

### Multi-object editing and persistence

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

## Out of scope

- Undo/redo of any operation in this slice — `undo-redo` (slice 7). Every
  edit here is still one well-formed commit per interaction (one style-field
  change, one stop add/remove/edit, one multi-object batch), the same
  discipline every prior slice has kept, so slice 7 has a clean, single
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
  (slice 8) can produce them.
- Pattern fill and swatch/texture fill (Inkscape's other two fill types
  beyond flat color and the two gradients). Not needed for a laser-cutting
  MVP; revisit if a maker workflow asks for it.
- Variable-width ("Power Stroke") strokes, and start/mid/end markers
  (arrowheads and similar) on a stroke. Neither is named by R-EDIT-005/006.
- Exact default gradient angle/center, and whether a maker redirects a
  gradient by dragging an on-canvas handle, typing numeric endpoints, or
  both. Criteria 21–22 state only what the data model and default rendering
  guarantee; the editing interaction is the ux-engineer's call.
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
  (slice 10) is where this slice's properties first need to round-trip
  through actual SVG markup; this slice's persistence criterion (25) is
  about the project's own `.vmf` file only.

## UX notes

**2026-10-05 amendment:** the customer asked for a Blender/Affinity-leaning
chrome direction across the whole app (`docs/design-system.md`'s
2026-10-05 section). The placement, sizing, collapsibility and shortcut
decided below all still hold exactly as written — this slice's own
reasoning for "docked right, fixed, not a toolbar or dialog" is what the
customer's "right side, everything configurable, fixed in place" ask
independently arrives at, so nothing here needed to change on the merits.
What changes is scope and naming only: this is no longer a one-off
`StylePanel` but the **first section of a general-purpose `PropertiesPanel`**
that later slices' object properties also plug into (one scrolling column
of named sections, not tabs — see `docs/design-system.md` for why). Every
"`StylePanel`" reference below means "the Style section of
`PropertiesPanel`"; every placement/sizing/collapse/shortcut decision
applies to the panel as a whole, not to this slice's section specifically.
No acceptance criterion, control, or interaction decided below changes.

This is the first slice with no new canvas tool and the first properties UI
that isn't a contextual tool-options bar (`path-node-editing`'s node-actions
bar, `primitive-shapes`' polygon/star bar) — those exist only while a
specific tool is active; stroke and fill must stay reachable regardless of
which tool is active, because styling is orthogonal to drawing. That's the
deciding fact below, not a stylistic preference.

### Where styling lives: a docked panel, not a toolbar or dialog

**A persistent, dockable `StylePanel`, docked right, following Inkscape's
own Fill & Stroke panel convention** (Inkscape ships this as a dockable
panel, not the modal dialog older versions used) — not a contextual toolbar
like `ShapeToolbar`, and not a shadcn `Dialog`/`AlertDialog`. Reasoning:

- A contextual bar (slice 2/3's pattern) is scoped to one tool being active.
  Stroke and fill apply to a selection regardless of which tool drew it —
  Pen, Node, Rectangle, Ellipse, Polygon/Star can all have a styled object
  selected, and AC1 explicitly requires the identical controls regardless of
  object kind or active tool. A tool-scoped bar would mean switching tools to
  restyle, which contradicts "selects it" in AC1 having no tool precondition.
- A modal dialog (this product's only existing precedent, slice 1's
  `AlertDialog`) is wrong for a different reason than "not blocking enough":
  `AlertDialog`'s whole point was *forcing* acknowledgment of a failure. Fill
  & Stroke is the opposite — the maker clicks between several objects
  comparing/adjusting style, so the panel must survive a selection change
  without being reopened. A dialog the maker has to reopen per object fails
  that immediately.
- Docked chrome at a fixed edge, not floating, keeps with "the canvas is the
  product, chrome stays out of the way": it sits beside the canvas (right
  edge, mirroring the tool rail's left edge) rather than over it, and it
  collapses (below) when the maker wants the width back.

**Placement and sizing:**

- Docks to the **right edge**, spanning from the native menu bar down to the
  status bar — the mirror image of the left tool rail, so the canvas keeps
  its own edge-to-edge rule between the two chrome strips, not framed by
  either.
- Width **280px**, fixed (not resizable in this slice — add a drag handle
  only if a later story asks). Canvas fills the remaining width.
- **Collapsible**, via a small chevron tab on the panel's canvas-facing
  edge and via the keyboard shortcut below. Collapsed state shows nothing
  but the chevron tab (0px content width) — not a narrower icon rail, since
  there's nothing useful to show narrower than the controls themselves.
- **Default state: open**, selection-independent — i.e. it's visible on
  first launch even with nothing selected (showing all controls disabled/
  blank, see "Multi-select and no-selection display" below), the same way
  Inkscape's panel is normally left open. It does not auto-open on selecting
  an object and auto-close on deselecting; the maker controls open/closed,
  the selection controls what's *in* it.
- This is ordinary DOM app chrome (React, like the tool rail and the
  tool-options bars), not canvas editing UI — the "WebGL draw list, never
  DOM" rule in `docs/design-system.md` is about glyphs drawn *over* document
  content, not about the application's own panels, so it doesn't apply here.

### Stroke section (top of panel)

- **Enable/width row:** a numeric field for width, unit suffix "mm" shown
  inline (this product's canonical unit, matching every other numeric field
  in the app), plus a **paint swatch** to its left that is the stroke's
  on/off control: clicking the swatch opens the shared color+alpha picker
  (below); the swatch itself shows a red diagonal line over it (Inkscape's
  "X" no-paint convention) when stroke is off. Setting width to exactly 0
  and choosing "no paint" from the swatch are the same stored state
  (AC5) — the UI doesn't need two separate switches that could disagree.
  Turning the stroke back on (clicking an actual color in the picker, or
  typing a nonzero width) restores the last-stored width/color/dash/join/cap
  together, per AC5.
- **Color:** the swatch opens a popover with a saturation/hue picker, a hex
  input, and an **alpha slider (0–100%)** — stroke color and alpha are both
  independently settable per AC6, so this needs the full picker, not a
  simplified alpha-less one. Because fill's solid color (AC14) needs the
  identical color+alpha shape, **this is one shared `ColorAlphaPicker`
  component**, used by the stroke swatch, the fill solid swatch, and (at
  smaller size, color+opacity only, no separate "alpha" framing since a
  stop's opacity *is* its alpha) each gradient stop row. One component, one
  place a maker learns it, per this product's running "don't invent a second
  control for the same job" rule (`docs/design-system.md`'s accent-color
  rule is the same instinct applied to color once already).
- **Dash preset:** a `Select` dropdown below the width/color row, options
  "Solid" (default), "Dash", "Dot", "Dash-Dot" — each option's trigger shows
  a small line-sample icon of that pattern, not just text, so the maker
  recognizes it visually before opening the menu (matches Inkscape's own
  dash dropdown). No custom-array entry control in this slice (per "Out of
  scope"); the dropdown is the entire UI for criterion 9's stored format.
- **Join:** a three-icon segmented control (`ToggleGroup`, single-select,
  `role="radiogroup"` semantics) — Miter / Round / Bevel, icons matching
  Inkscape's own join glyphs. Same segmented-control pattern
  `primitive-shapes` established for Polygon/Star's two-state mode toggle,
  extended to three states — reusing a pattern rather than inventing a new
  mutually-exclusive-icon widget.
- **Cap:** identical segmented-control treatment, Butt / Round / Square,
  directly below Join.
- Join and Cap rows stay visible and enabled even when the selection has no
  sharp corner (AC10) or no open end (AC12) to apply to — they describe a
  stored property of the object, not a property of its current geometry, so
  disabling them would hide a value the maker legitimately set (e.g. for
  later when the maker opens the path and adds a corner). They simply have
  no visible effect until geometry gives them something to act on.

### Fill section (below stroke, same panel)

- **Mode selector:** a four-option segmented control — None / Solid /
  Linear / Radial — same `ToggleGroup` pattern as Join/Cap, one level up in
  visual weight (slightly taller) since it's the section's primary choice
  and everything below it depends on which mode is active.
- **None:** no further controls shown below the mode row (nothing to edit).
- **Solid:** one `ColorAlphaPicker` swatch (same shared component as
  stroke), full color + alpha per AC14.
- **Linear / Radial:** a **gradient stop editor**, panel-only for this
  slice — no on-canvas drag handles. Decision and reasoning below.
- Switching away from a mode and back (e.g. Solid → Linear → Solid) must
  not lose the Solid color or the gradient's stop list — both stay stored
  per AC13's "kept stored but unapplied" discipline, same as stroke's
  on/off swatch.

### Gradient stop editor: panel-only, no on-canvas handles this slice

**Decision: panel-only.** A vertical list of stop rows, each with: a small
position slider or numeric field (0.0–1.0, shown as 0–100% to match the
alpha fields' percent framing elsewhere in the panel), the shared
`ColorAlphaPicker` swatch (color + that stop's own opacity — AC16), and a
remove button (disabled when exactly 2 stops remain, AC19). An "Add stop"
button appends at position 1.0 minus a small offset from the last stop
(simplest well-defined default; the maker repositions it). Stops are listed
top-to-bottom in position order, matching the order they'll render in
(AC18).

Reasoning for deferring on-canvas handles rather than building both now:

- The spec explicitly leaves this open (criteria 21–22's note) precisely
  because it's a separable chunk of work: on-canvas gradient handles (drag
  the axis endpoints for linear, drag center/radius for radial) are a real,
  valuable follow-up, but they're additive — a maker can set every value
  this slice's criteria require (stop position/color/opacity, linear vs.
  radial, default axis/center from the bounding box) through the list alone.
  Nothing in AC16–22 requires on-canvas redirection; it's named in "Out of
  scope" as a later call, not a gap in this slice's value.
- Building both now would mean a second, canvas-space hit-testing and
  dragging system (new handle glyphs, new drag state machine) in the same
  slice that's already introducing the product's first panel-only property
  UI. That's two new interaction surfaces at once for a feature whose own
  acceptance criteria don't ask for the second one — against this project's
  YAGNI rule (`CLAUDE.md` §5).
- This matches how `primitive-shapes` phased polygon/star itself: point
  count landed in the bar only, ratio got both bar and on-canvas handle
  together because AC14 specifically required the handle and the bar to
  stay in sync. No acceptance criterion here requires an on-canvas/panel
  sync story, so there's nothing yet to keep in sync.
- Flag for the product owner: on-canvas linear-axis and radial-center/radius
  dragging is a reasonable next slice once this one ships, the same way
  primitive resize handles followed primitive creation.

### Multi-select and no-selection display

**Mixed-state placeholders, not "show the first-selected object's
values"** (the Figma/Illustrator convention, not Inkscape's own, which this
slice intentionally deviates from Inkscape on because Inkscape's
"first-selected wins silently" is the more error-prone of the two — a maker
editing what looks like object A's width can unknowingly overwrite object
B's different width without ever seeing that B differed):

- **No selection at all:** every control shown disabled (not hidden) at its
  last-used/default value — disabled, so it reads as "nothing to edit"
  rather than "editable but blank."
- **Single object selected:** every control shows that object's actual
  stored values, per AC4/6/8/etc.
- **Multiple objects, a property matches across all of them:** show that
  shared value normally (e.g. all selected objects have 0.25mm stroke width
  → the field shows "0.25").
- **Multiple objects, a property differs:** show a mixed-state placeholder
  specific to the control type, never a guess:
  - Numeric field (width, stop position): empty, placeholder text "Mixed"
    in muted color.
  - `ColorAlphaPicker` swatch: a neutral checkerboard-pattern swatch (the
    standard "no single color" treatment) rather than guessing which
    object's color to preview.
  - Segmented controls (join, cap, fill mode, dash preset): no option shown
    selected/pressed.
  - Gradient stop list: shown only when every selected object is in the same
    fill mode (Linear or Radial) *and* has the same stop count — otherwise
    the fill-mode row itself shows mixed state and the stop list is hidden
    (nothing coherent to show row-by-row across differently-shaped
    gradients).
- **Committing a value while multiple objects are selected** applies that
  one property to every selected object independently, each keeping its own
  other properties (AC24) — this holds whether the field started at a
  shared value or a mixed-state placeholder; typing into a "Mixed" field and
  confirming sets all selected objects to the typed value, same as typing
  over a concrete shared value does.

### Live preview, commit on release — same discipline as the ratio slider

This product already has a rule for exactly this (`primitive-shapes`
`adrs.md`, 2026-10-04 note): a slider drag produces an **ephemeral preview**
and **one commit on release**. Apply it uniformly across every continuous
control in this panel, with no exceptions:

- **Alpha/opacity sliders, stop-position sliders:** live-update the
  rendered object on every pointer-move while dragging; one commit on
  pointer-up. Dragging off and back without releasing must not create
  intermediate commits.
- **Numeric fields (width, position typed directly, hex input):** live
  preview on every keystroke is *not* required here (unlike a slider, there's
  no continuous gesture to preview mid-motion) — commit on blur or Enter,
  matching ordinary form-field behavior elsewhere in the app. Escape while
  focused reverts to the last-committed value and blurs, without committing.
- **Color picker (saturation/hue area):** live preview while dragging inside
  the picker, same as a slider; one commit when the pointer is released
  inside the picker, or when the popover closes, whichever happens first.
- **Segmented controls (join, cap, dash preset, fill mode) and swatch
  on/off toggles:** these are discrete choices, not drags — each click
  commits immediately, there's no ephemeral state to speak of.
- **Multi-select commits:** per AC24, a drag-released or clicked change with
  multiple objects selected is still exactly one atomic commit touching every
  selected object, not one commit per object.

### Keyboard and accessibility

- **Shortcut to open/focus the panel: `Shift+Ctrl+F`**, matching Inkscape's
  own Fill & Stroke binding exactly — same reasoning this product has used
  for every prior shortcut (`B`, `N`, `R`, `E`, `*`): match Inkscape so a
  maker switching tools gradually finds it where they expect. If the panel
  is collapsed, this expands it and focuses its first control; if already
  open, it moves focus into the panel without toggling it closed (a toggle
  that can hide the panel the maker is actively using would be worse than a
  one-direction "bring it to me"). Add this to `docs/design-system.md`'s
  shortcut table (native-menu-accelerator category doesn't apply — this is
  a plain focus shortcut, not a file/edit command, and it's app-global
  rather than canvas-focus-scoped since the panel itself isn't canvas
  content).
- **Tab order** follows visual top-to-bottom reading order: stroke
  enable-swatch → width → dash preset → join group → cap group → fill mode
  group → fill controls (solid swatch, or the gradient stop list top to
  bottom, each stop's position → color-swatch → remove-button). Standard DOM
  order; no explicit `tabindex` overrides needed beyond what the segmented
  controls require below.
- **Segmented controls (`ToggleGroup`) use roving tabindex / native
  radiogroup behavior:** one Tab stop per group (not per icon); Left/Right
  (or Up/Down) arrow keys move the selection within the group, matching
  native `role="radiogroup"` and every desktop app's own icon-toggle
  convention (Inkscape included). `aria-label` on each icon button names the
  choice ("Miter join", "Round join", "Bevel join", etc.) since the icons
  alone aren't accessible-name-bearing.
- **`ColorAlphaPicker` popover:** opens on click or Enter/Space on the
  swatch, closes and commits on Escape (reverting to last-committed, same
  rule as numeric fields above) or on click-outside (commits, doesn't
  revert — click-outside is "I'm done," not "cancel," matching the hex-input
  blur rule). Focus moves into the popover's hex field on open; closing
  returns focus to the swatch that opened it.
- **Contrast:** mixed-state "Mixed" placeholder text and the disabled
  (no-selection) control state both need to meet the same body-text contrast
  minimum as every other label in the app against `--toolbar-bg`/panel
  background — verify against whatever panel background token is chosen
  when implemented (see "New tokens" below; no new background color exists
  yet for this panel).

### New tokens and precedent for `docs/design-system.md`

Add alongside this file, same as every prior slice:

- `StylePanel` background and width (280px), docked right — a new chrome
  region; reuse `--toolbar-bg` for its background unless contrast testing
  (above) says otherwise, rather than inventing a second chrome color.
- The `ToggleGroup` three/four-icon segmented-control pattern as the named,
  reusable component for Join/Cap/Fill-mode (extends `primitive-shapes`'
  two-state Polygon/Star toggle to a general n-state pattern — document it
  once as the general case).
- `ColorAlphaPicker` as a shared, named component (swatch + popover:
  saturation/hue area, hex input, alpha slider) — used by stroke color,
  solid fill color, and gradient stop color/opacity.
- Mixed-state convention (empty+"Mixed" placeholder for numeric fields,
  checkerboard swatch for color, no-selection for segmented controls) as the
  house style for multi-select display — the first slice with multi-select
  editing of differing values, so this is the baseline later multi-select
  features (e.g. future transform/align panels) should follow.
- `Shift+Ctrl+F` added to the shortcut table, tagged as app-global
  (non-canvas-focus-scoped), a new category distinct from both the existing
  native-menu accelerators and the canvas-focus single-letter tool keys.

## Links
Requirements: R-EDIT-005, R-EDIT-006 (`docs/requirements.md`)
PR: TBD
