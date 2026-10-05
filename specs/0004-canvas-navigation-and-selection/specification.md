# Canvas navigation and a general Select tool

Status: Done
Priority: Must
Origin: Customer

## User value

As a maker I want to pan and zoom the canvas, and select, move or delete any
object I've already drawn without switching back into the tool that created
it, so that I can navigate a document larger than my screen, work at both
overview and fine-detail scale, and fix a mistake on any shape immediately
instead of hunting for its original tool.

This slice bundles two customer-reported gaps because they are the same
underlying problem - the canvas has no interaction layer beyond each
drawing tool's own click handling:

1. "The canvas isn't zoomable/scrollable yet" - there is no way to see more
   of a document than fits on screen at a fixed scale, and the window-resize
   handling that belongs with it has never been exercised end to end.
2. "Once you've created primitive geometry, you can't edit it anymore -
   there's no select tool" - every object-creation tool (Pen, Node,
   Rectangle, Ellipse, Polygon/Star) only acts on objects of its own kind;
   there is no general way to click an existing object and move or delete
   it without re-entering its specific creation tool.

Both gaps block ordinary use of everything built in `path-node-editing`
(slice 2) and `primitive-shapes` (slice 3), and both sit at the same layer
(general canvas interaction, independent of any one drawing tool), so this
slice builds them together rather than making the customer wait through a
second foundation slice.

**What we do differently from the tools this replaces, and what we match on
purpose:** at the interaction level, nothing by design - R-EDIT-010 and
R-EDIT-011 ask for navigation and selection every maker already has muscle
memory for, not a reinvention.

- **Pan**: scroll-to-pan (vertical; Shift+scroll for horizontal),
  middle-mouse-drag, and Space+drag are Inkscape's own canvas-navigation
  scheme, and this product already commits to Inkscape parity wherever it
  fits (`path-node-editing`'s B/N shortcuts, `primitive-shapes`'s R/E/*).
  It is also, independently, what Figma and Affinity Designer do today.
  Blender's scheme (plain scroll zooms, not pans; middle-drag pans/orbits)
  is deliberately not followed - Blender is a 3D tool solving a 3D
  navigation problem, and copying its 2D canvas behavior would break the
  Inkscape parity this product otherwise holds everywhere else.
- **Zoom toward the cursor, not the canvas center**: every reference tool
  named above does this; a zoom that centers on the canvas instead of the
  cursor is the one thing in this spec that would read as "broken" rather
  than "different", because it fights the maker's own intent (zoom in on
  *this* detail, not the middle of the document).
- **Select tool as the launch default**: Inkscape's own default tool is its
  Selector, not Pen. `path-node-editing` set Pen as the provisional launch
  default only because no Select tool existed yet, and explicitly flagged
  "revisit once a general selection tool exists" - this slice is that
  revisit.

## Acceptance criteria

### Pan

1. Given any tool is active and the canvas has focus, when the maker scrolls
   the mouse wheel or trackpad vertically with no modifier held, then the
   view pans vertically (document content moves up/down on screen) by an
   amount proportional to the scroll delta, and no document content (any
   object's position, size or shape) changes.
2. Given the same context, when the maker holds Shift while scrolling, then
   the view pans horizontally instead of vertically, by the same
   proportional rule.
3. Given any tool is active, when the maker presses and holds the middle
   mouse button and drags, then the view pans freely in both axes to follow
   the drag 1:1 - the document point under the cursor at press-down stays
   under the cursor for the duration of the drag - until the button is
   released.
4. Given any tool is active, when the maker holds the Space bar and drags
   with the primary mouse button, then the view pans identically to
   criterion 3 - an alternative input for a mouse with no middle button.
5. Given panning by any of criteria 1-4, then the active tool's own
   in-progress state (e.g. a pen path being drawn, a shape being dragged
   out, a node being dragged) is preserved and resumes exactly where it left
   off once panning stops - panning never cancels or commits an in-progress
   interaction.

### Zoom

6. Given any tool is active, when the maker scrolls the mouse wheel while
   holding Ctrl (Cmd on macOS), or performs a pinch gesture on a trackpad or
   touchscreen, then the view zooms in or out, centered on the document
   point currently under the cursor - that point stays under the cursor
   after the zoom; it does not jump toward the canvas center.
7. Given the zoom range, then the minimum selectable zoom is **2%** and the
   maximum is **8000%**, where "100%" means one document millimetre renders
   as 96 ÷ 25.4 ≈ 3.78 CSS pixels - the standard CSS reference-pixel
   convention this product already uses elsewhere (`CSS_PX_PER_MM`), not a
   physically-measured display size, which a webview has no way to read.
   This is the same conventional "actual size" mapping Inkscape's "1:1" and
   Illustrator's "100%" already use. At 8000%, a 0.1 mm feature renders at
   roughly 30 CSS pixels long - enough to place a node precisely by eye,
   with headroom over `path-node-editing`'s 8px hit-test radius. At 2%, a
   600 mm x 400 mm sheet (a common laser-bed size) renders at roughly
   45 x 30 CSS pixels total - small, but the floor's job is only that the
   whole sheet stays visible without scrolling, not that it's comfortable
   to inspect at that extreme. Together these bound the range a laser maker
   actually needs, from inspecting sub-millimetre detail to overviewing a
   full sheet.
8. Given the view is already at the minimum or maximum zoom, when the maker
   tries to zoom further in that direction, then the zoom stops exactly at
   that limit with no error and no overshoot-and-snap-back.
9. Given any tool is active, then the current zoom level is visible to the
   maker at all times, shown as a percentage, somewhere in the persistent UI
   (exact placement is the ux-engineer's call - `project-file-foundation`
   already reserved status-bar room for this).

### Canvas resize

10. Given a project is open at some zoom level and pan position, when the
    maker resizes the app window (or the canvas viewport otherwise changes
    size), then the zoom level does not change, and the document point that
    was at the center of the viewport before the resize is still at the
    center of the viewport after - the view grows or shrinks symmetrically
    around that center point rather than jumping, re-centering on the
    document origin, or resetting to a default zoom.
11. Given the same resize, then no blank or stale frame is ever visible
    during it - the canvas content tracks the window's new size within the
    same frame the resize happens. (This is the user-facing acceptance test
    for the wgpu-surface-reconfigure-on-resize behavior already measured and
    required by the `path-node-editing` spike,
    `specs/0002-path-node-editing/adrs.md` - not new work, but previously
    untested from the maker's own interaction with a real window.)

### Select tool - reachability and default

12. Given the app is running, then a Select tool exists as its own entry in
    the tool rail, with its own icon distinct from Pen/Node/Rectangle/
    Ellipse/Polygon-Star, reachable by clicking it and by the keyboard
    shortcut **`S`** - matching Inkscape's own Selector-tool binding, the
    same precedent `path-node-editing` and `primitive-shapes` already set
    for B/N/R/E/*.
13. Given a new project is created or an existing project is opened, then
    the Select tool is the active tool by default, replacing Pen as the
    launch default `path-node-editing` set - matching Inkscape's own
    default, and reflecting that viewing and selecting what is already
    there is the more common first action, especially when opening an
    existing file full of objects rather than starting from empty.

### Select tool - selecting

14. Given the Select tool is active, when the maker clicks on any existing
    object - a path (drawn with the pen tool or produced by "object to
    path") or a primitive (rectangle, ellipse, polygon, star) - regardless
    of which tool created it, then that object is selected and shown with a
    visible selection indicator (a bounding box, the same convention
    `primitive-shapes` already established for shape-tool selection,
    extended here to paths too).
15. Given the Select tool is active with an object selected, when the maker
    clicks on empty canvas, then the selection is cleared.
16. Given the Select tool is active with an object selected, when the maker
    clicks a different single object, then the previous selection is
    replaced by the newly clicked object - plain click is single-select,
    not additive.

### Select tool - multi-select

17. Given the Select tool is active with one object selected, when the maker
    shift-clicks a second, different object, then both objects become
    selected together - the same shift-click-to-add convention
    `path-node-editing` already uses for multi-node selection, extended
    here to whole objects.
18. Given two or more objects selected via criterion 17, when the maker
    drags any one of the selected objects, then every selected object moves
    by the same offset (criterion 20's move behavior, applied to the whole
    selection at once).
19. Given two or more objects selected via criterion 17, when the maker
    presses Delete or Backspace, then every selected object is removed from
    the document as one atomic operation.

### Select tool - move and delete

20. Given the Select tool is active with exactly one object selected, when
    the maker drags that object (not a tool-specific handle - the Select
    tool shows no shape handles and no path nodes, only the bounding box),
    then the object moves to follow the drag 1:1, live, and the move
    completes as a single operation when the mouse button is released.
21. Given the Select tool is active with one or more objects selected, when
    the maker presses Delete or Backspace, then the selected object(s) are
    removed from the document - identically for a path, a rectangle, an
    ellipse, and a polygon/star, with no need to switch into that object's
    own creation tool first. This is this slice's direct fix for the gap
    reported while testing `primitive-shapes`.

### Select tool - handoff to an object's own tool

22. Given the Select tool is active, when the maker double-clicks a path,
    then the tool switches to the Node tool with that path selected and
    ready for node editing - matching Inkscape's own double-click-to-Node
    convention.
23. Given the Select tool is active, when the maker double-clicks a
    primitive (rectangle, ellipse, polygon or star), then the tool switches
    to that primitive's own creation tool (Rectangle, Ellipse or
    Polygon/Star respectively) with the object selected and its shape
    handles visible and ready for adjustment - the same double-click
    convention as criterion 22, applied to whichever tool each object kind
    actually edits with.

### Pan and zoom work in every tool

24. Given any tool other than Select is active (Pen mid-drawing, Node,
    Rectangle, Ellipse, Polygon/Star), when the maker performs any of
    criteria 1, 2, 3, 4 or 6's pan/zoom input, then panning or zooming
    happens exactly as specified, with no need to first switch to the
    Select tool or a dedicated "hand" tool - matching every actively
    maintained reference tool named above, not older tools that gate
    navigation behind its own separate tool.

## Out of scope

- Marquee/rubber-band selection (dragging an empty-space rectangle to select
  multiple objects at once). `path-node-editing` already deferred this for
  nodes with the same rationale; deferred here for objects too.
  Shift-click (criterion 17) is this slice's only multi-select input.
- Resize/rotate handles on the Select tool's own bounding box (Illustrator's
  and Inkscape's selection-arrow transform handles, which scale or rotate
  any selected object uniformly, independent of its own kind-specific
  handles). Each object already resizes through its own tool (shape handles
  for primitives, node/handle dragging for paths); a second, tool-
  independent resize/rotate affordance is a real feature but not what
  either customer complaint asked for. Revisit as its own slice if asked.
- Keyboard-nudging a selected object with arrow keys - same deferral as
  `path-node-editing` and `primitive-shapes`.
- Snapping of any kind (to grid, to other objects, to guides) during a
  Select-tool move - same deferral as every earlier slice.
- Zoom presets ("fit page to window", "fit selection", "reset to 100%") and
  numeric zoom-percentage entry. Only scroll/pinch zoom (criterion 6) and
  the always-visible read-out (criterion 9) are in scope; a maker can reach
  any zoom level by scrolling, just not in one click yet.
- Keyboard zoom shortcuts (`+`/`-`, Ctrl+0 to reset to 100%). Not named by
  the customer's request; left for the ux-engineer to propose if judged a
  trivial, low-risk addition, not required by any criterion here.
- On-screen scrollbars. No scrollbar UI is introduced; pan is scroll/drag
  only, consistent with every reference tool this spec cites.
- Rotating or skewing the canvas view. `ViewTransform` is pan plus uniform
  zoom only (`path-node-editing`'s feature-local decision in
  `specs/0002-path-node-editing/adrs.md`); this slice does not ask for more
  than that shape already supports.
- Touch gestures beyond the pinch-to-zoom named in criterion 6 (e.g.
  two-finger pan, touch-drag-to-select). Desktop mouse/trackpad is this
  slice's target per `CLAUDE.md`'s platform order; touch refinement is a
  later pass once a touch-first platform is prioritized.
- Grouping-aware selection (clicking one member of a future group selects
  the whole group) - no groups exist yet (`layers-and-grouping`).
- A Select-tool-specific right-click context menu. Not asked for by either
  customer complaint; `path-node-editing`'s node-tool context-menu
  precedent is untouched by this slice.

## UX notes

This is the first slice built *against* the 2026-10-05 chrome direction
(floating left tool panel, fixed right Properties panel, no layout shift on
tool switch), not a redesign of existing chrome, so everything below is new
construction on that foundation rather than a migration note. Tokens and
conventions referenced here are extended in `docs/design-system.md`
alongside this file.

### Select tool - rail position, icon, shortcut

- **First in the floating left tool panel, above Pen** - a deliberate,
  one-time exception to the rail's own "new tools append in ship order,
  existing icons don't get reordered" rule (`primitive-shapes`). That rule
  protects against reshuffling the rail for a later tool's convenience;
  Select isn't a peer creation tool being slotted in for convenience, it's
  the rail's new default/master tool (criterion 13), and every reference
  tool this spec already commits to (Inkscape's Selector, Illustrator's and
  Affinity's selection arrow) puts it first, above the drawing tools, for
  exactly that reason. Pen, Node, Rectangle, Ellipse, Polygon/Star all shift
  down one slot; their own relative order is unchanged. Recorded as an
  explicit carve-out in `docs/design-system.md` so it reads as a one-time,
  reasoned exception rather than a precedent for reordering the rail again
  later.
- **Icon:** a solid arrow/pointer cursor glyph - the one icon every
  reference tool above uses for this exact tool, and visually unlike
  Pen/Node/Rectangle/Ellipse/Polygon-Star's own glyphs by construction (none
  of them are arrow-shaped). Same 24px glyph size inside the same 48x48
  button, same `--toolbar-icon`/`--toolbar-icon-active-bg`/
  `--toolbar-icon-active-fg` tokens, same hover/focus/active states as
  B/N/R/E/*.
- **Shortcut `S`**, bound at canvas-focus scope exactly like B/N/R/E/* (not
  a native-menu accelerator). `aria-label`: "Select tool (S)". Tooltip shows
  name + shortcut, same convention as every other rail button.

### Selection visual language: a new, unified "selected" indicator, distinct from "editing"

**Decision: the Select tool shows the same plain bounding-box outline on
every object type, with no handles and no nodes - not each type's own
existing editing look.** This is already what the acceptance criteria
require (criterion 14's "a bounding box... extended here to paths too";
criterion 20's "the Select tool shows no shape handles and no path nodes,
only the bounding box"), and it's the right call independent of that,
because it answers a question `path-node-editing` and `primitive-shapes`
never had to: "selected, but not yet committed to editing" is now a real,
distinct state from "actively editing with this object's own tool," and the
two need to look different or a maker can't tell which mode they're in.

- **Selected (Select tool active):** reuse the existing bounding-box token
  (`docs/design-system.md`'s "Bounding-box selection outline," 1px
  screen-space `--accent`) for every object kind, paths included - for a
  path, this is the axis-aligned bounding box of its geometry, computed
  fresh, not a per-node display. No shape handles (primitives), no
  node/handle glyphs (paths). Hover, tool not yet clicked into selection:
  same box at `--accent-hover`, the existing 20%-opacity rule - identical
  hover treatment to what `primitive-shapes` already does for its own tool.
- **Editing (after double-click handoff, criteria 22-23):** each type's
  existing look, unchanged - `path-node-editing`'s node/handle glyphs for a
  path (no bounding box; slice 2 never drew one), `primitive-shapes`'s
  bounding-box-plus-shape-handles for a primitive. Nothing new to build
  here; this is exactly what double-clicking switches *to*.
- **Why unified rather than type-specific for the Select tool itself:** a
  simpler, common "this is selected" look that doesn't vary by type is what
  makes the Select tool read as general-purpose, matching its own job
  (criterion 21's "identically for a path, a rectangle, an ellipse, and a
  polygon/star"). It also sets the right precedent for every future object
  type this product adds (text, embroidery stitch regions, whatever comes
  next): Select-tool selection is always "plain bounding box, no
  type-specific affordance," full stop, not a per-type decision each new
  object kind's slice has to re-litigate. The bounding-box-to-handles (or
  bounding-box-to-nodes) transition on double-click becomes that precedent's
  other half: it IS the "you're now editing" signal, on top of the tool-rail
  highlight below.

### Multi-select: each object keeps its own indicator, no group summary

Shift-clicking objects of different types (criterion 17) shows each
selected object with its own normal Select-tool indicator (the bounding box
above) simultaneously - a path and a rectangle selected together each get
their own box, at their own position and size, drawn at the same time. No
single box is drawn around the combined group, and no "N objects selected"
text replaces the per-object boxes.

This is the same philosophy `stroke-and-fill-styling`'s mixed-state
convention established for heterogeneous multi-select, applied to
selection display instead of property display: show each object's own real
state truthfully rather than collapsing a mixed selection into one
simplified summary. The UI mechanism differs (that spec's "Mixed" text/
checkerboard swatch is for a *shared control* that can't show two values at
once; selection indicators have no such conflict - every selected object
can just show its own box at the same time) but the underlying rule is the
same one: don't paper over heterogeneity, show it. Because the Select tool's
indicator is already unified-by-type (previous section), this falls out for
free - nothing extra to build for the mixed-type case specifically.

### Zoom readout

**Status bar, as a new center segment** - `project-file-foundation`'s
status bar already has cursor position on the left and document size on
the right, with room explicitly reserved for a zoom control between them.
Add the zoom level there: plain percentage text, integer (no decimals -
`path-node-editing`-grade precision doesn't help a number whose job is
situational awareness, not measurement), e.g. "100%", range 2%-8000% per
criterion 7. No input affordance in this slice (numeric zoom entry is
explicitly out of scope) - display-only text, same `--statusbar-bg`
treatment as the other two segments, updates live during scroll/pinch zoom
(criterion 6) and stays correct across window resize (criterion 10).

### Pan/zoom input feedback: cursor only, no new overlay

- **Space-held or middle-mouse-drag pan (criteria 3-4):** standard
  grab/grabbing cursor convention - open-hand cursor the moment Space is
  held down or the middle button is pressed (before any drag motion),
  closed-hand/grabbing cursor for the duration of the drag, reverting to
  the active tool's normal cursor on release or Space-up. This overrides
  whatever cursor the active tool would otherwise show (e.g. Pen's
  crosshair) for exactly the duration of the pan - the same "pan interrupts
  nothing" rule criterion 5 already states for the tool's in-progress
  state, applied to the cursor.
- **Scroll-wheel pan (criteria 1-2) and Ctrl+scroll/pinch zoom (criterion
  6):** no cursor change and no other visual indicator beyond the zoom
  readout above - these are instantaneous, not a "mode" the maker enters or
  holds, so there's nothing to signal before or after. The view moving (or
  the zoom readout updating) is the only feedback, and it's sufficient;
  don't add a transient on-canvas zoom-percentage popup or similar - that's
  chrome the criteria don't ask for and the status-bar readout above already
  covers.

### Double-click handoff: tool-rail highlight, confirmed as the mechanism, plus the selection-indicator change for free

Yes, and it already works by construction: `path-node-editing`'s rail
convention is "active tool: filled `--accent` background, white icon, stays
active until [changed]" - switching the active tool on double-click
(criteria 22-23) updates this highlight the same way clicking a rail button
does, no new mechanism needed. Select's icon un-highlights, the target
tool's icon highlights.

This is reinforced, not carried alone, by the selection-indicator change
from bounding-box-only to type-specific handles/nodes (previous section) -
that change happens right where the maker's eyes already are (at the
object they just double-clicked), which is a faster read than the rail
highlight off at the canvas edge. No additional flash, animation, or cursor
change at the moment of handoff; two existing, reinforcing signals are
enough, and no layout shift occurs on either (the rail doesn't move, the
Properties panel's shape-tool-options section swap, if any, follows the
same no-layout-shift rule already established 2026-10-05).

## Links
Requirements: R-EDIT-010, R-EDIT-011 (`docs/requirements.md`)
PR: https://github.com/Th3Link/vectormanufactoring/pull/25
