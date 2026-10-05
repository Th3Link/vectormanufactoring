# Canvas navigation and a general Select tool

Status: Draft
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
   as one physical millimetre on the maker's display (the display's
   reported DPI) - the same "actual size" convention as Inkscape's "1:1"
   and Illustrator's "100%". At 8000%, a 0.1 mm feature renders at least 24
   physical screen pixels long on a standard 96 DPI display - enough to
   place a node precisely by eye, with headroom over `path-node-editing`'s
   8px hit-test radius. At 2%, a 600 mm x 400 mm sheet (a common laser-bed
   size) renders at 12 mm x 8 mm, fitting inside any window at least that
   large with margin to spare. Together these bound the range a laser maker
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
(filled in by ux-engineer before Ready)

## Links
Requirements: R-EDIT-010, R-EDIT-011 (`docs/requirements.md`)
PR:
