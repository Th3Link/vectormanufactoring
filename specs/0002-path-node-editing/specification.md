# Path and node editing: draw and edit a Bézier path with a pen tool

Status: Done
Priority: Must
Origin: Customer

## User value

As a maker I want to draw a path by placing nodes and handles, and then move,
add, delete and reshape those nodes and handles afterwards, with the same
node/handle/segment mental model I already have from Inkscape, so that I can
produce or correct my own vector geometry without learning a different tool
or dropping back to Inkscape for anything beyond the simplest shape.

This is the foundation every later editing slice builds on (`specs/index.md`):
primitive shapes become editable paths over this same node/handle surface
(slice 3), styling attaches to the paths drawn here (slice 4), undo wraps the
interactions defined here (slice 5), and booleans operate on the closed paths
this slice can produce (slice 6). Getting the node/handle/segment model right
here, rather than approximating it, is what keeps those slices additive
instead of a rewrite.

**What we do differently from the tool this replaces:** nothing, by design,
at the interaction level — R-EDIT-001 asks for parity with Inkscape's pen and
node tools, not a reinvention, and a maker switching tools gradually (per
R-SYS-006) should not have to relearn how a node behaves. The difference is
underneath: every node is a CRDT-tracked entity with a stable identity
(ADR 0002 §5), not an array index, so the same node/handle model that feels
identical to Inkscape today is what makes concurrent multi-peer path editing
(a later slice) additive rather than a redesign. A path's anchors are a
movable list, each anchor a map of fields (ADR 0009 §3) — which is also why
this slice can state field-level behaviour (move this node, drag that handle)
as independent, well-defined actions.

## Acceptance criteria

### Drawing with the pen tool

1. Given the pen tool is active and no path is in progress, when the maker
   clicks once at point A and then clicks once at a different point B (no
   drag), then a two-node open path is drawn with a straight line segment
   between A and B, and both A and B are corner nodes (no handles pulled
   out).
2. Given a path is in progress and the maker's last placed node is B, when
   the maker presses the mouse button down at point C, drags before
   releasing, then releases, then a new node is added at C with two handles
   positioned symmetrically along the drag direction (same distance from C
   on each side, opposite directions), and the segment between B and C is a
   curve shaped by those handles.
3. Given a path is in progress with at least one segment, when the maker
   double-clicks (or presses the dedicated "finish path" action) instead of
   placing another node, then the path is committed as a selectable, open
   path object exactly as drawn, and the pen tool is ready to start a new,
   separate path object on the next click — it does not continue the
   just-finished path.
4. Given a path is in progress with at least one node placed, when the maker
   presses Escape, then the entire in-progress path is discarded and no path
   object is added to the document — the canvas returns to its state before
   the maker started drawing.
5. Given a path is in progress with three or more nodes, when the maker
   clicks on that path's own first node, then the path closes (a segment is
   added from the last placed node back to the first node), the result is a
   closed path object, and drawing ends automatically (equivalent to
   criterion 3, but closed instead of open).
6. Given any path created by criteria 1, 2 or 5, when it is rendered on
   canvas, then it shows a 0.25 mm solid black stroke and no fill regardless
   of its node types or curvature — this is this slice's stated placeholder
   default; stroke and fill the maker can change are `stroke-and-fill-
   styling` (slice 4, `specs/index.md`), not this one.

### Selecting and moving nodes and handles

7. Given a finished path and the node tool active, when the maker clicks on
   one of its nodes, then that node is shown as selected (visibly distinct
   from unselected nodes) and, if it has handles, both of its handles become
   visible as draggable points connected to it; an unselected node's handles
   are not shown, and a corner node with no handles pulled out shows none
   until the maker drags one out.
8. Given a selected node, when the maker drags the node itself (not one of
   its handles) to a new position and releases, then the node moves to that
   position together with both of its handles, unchanged relative to the
   node — the local curve shape around that node is preserved, and the two
   adjoining segments update live during the drag.
9. Given a selected node with two handles that are mirrored (equal distance
   from the node, opposite directions — the type created by criterion 2),
   when the maker drags one of its handles, then the opposite handle moves
   to stay collinear through the node at the same distance, and both
   adjoining segments reshape live. Given a selected corner node (independent
   handles, created by criterion 1 or by criterion 11's conversion), when the
   maker drags one of its handles, then only that handle moves; the other
   handle on the same node, if present, stays exactly where it was.
10. Given two or more nodes selected (first node clicked, each additional
    node added with shift-click), when the maker drags any one of the
    selected nodes, then every selected node moves by the same offset, each
    keeping its own handles unchanged relative to itself.

### Adding, converting and removing nodes

11. Given a selected node, when the maker chooses "convert to corner" or
    "convert to smooth" from the node tool's actions, then: converting a
    corner node (criterion 1's type) to smooth pulls out two handles of equal
    default length, collinear through the node along the path's local
    tangent, mirrored as in criterion 9; converting a smooth node (criterion
    2's or 9's type) to corner leaves its two handles exactly where they are
    but they no longer move together — each is independently draggable from
    then on, per criterion 9's corner behaviour.
12. Given a finished path and the node tool active, when the maker
    double-clicks a point on one of its segments (not on an existing node),
    then a new corner node is inserted at that exact point, splitting the one
    segment into two, with the path's visible shape unchanged at the instant
    of insertion.
13. Given one or more nodes selected, when the maker presses the delete
    action, then those nodes are removed and each pair of nodes that becomes
    directly adjacent as a result is joined by one segment computed from
    their own existing handles — deletion may visibly change the path's
    shape at that point (this slice does not reconstruct the pre-deletion
    curve the way Inkscape's shape-preserving delete does; see "Out of
    scope"). Given a deletion that would leave a path with fewer than two
    nodes, then the entire path object is removed from the document instead.

### Segments

14. Given a finished path and the node tool active, when the maker clicks on
    a point of one of its segments, between two nodes rather than on either
    one, then that segment is shown as selected, distinct from either
    endpoint node being selected. Given a selected segment, when the maker
    chooses "make line" or "make curve", then that one segment switches
    between a straight line and a curve by retracting or extending the
    relevant handles on its two endpoint nodes, without moving either
    endpoint node's position.

## Out of scope

- Primitive shapes (rectangle, circle/ellipse, polygon/star) — `primitive-
  shapes` (slice 3). This slice only has freehand path drawing.
- Stroke width, dash, join/cap, color and fill beyond the one stated default
  (criterion 6) — `stroke-and-fill-styling` (slice 4).
- Undo/redo of any operation in this slice — `undo-redo` (slice 5). Nodes and
  handles can be added, moved, converted and deleted, but not reverted
  through this slice alone.
- Boolean operations, grouping, layers — slices 6 and 7.
- Extending an already-finished open path by clicking back onto one of its
  endpoints with the pen tool. Inkscape supports this; here, a finished path
  is only reachable through the node tool, and the pen tool always starts a
  new path object. Revisit once a maker workflow actually needs it.
- Compound paths: a single path object with more than one subpath (e.g. a
  letter "O" with a separate hole contour drawn in one object). Each pen-tool
  drawing session produces exactly one path object with exactly one subpath.
  Multi-subpath objects may arise later from boolean operations (slice 6);
  authoring one directly with the pen tool is not this slice's job.
- Rubber-band/marquee selection of nodes (dragging an empty-space rectangle
  to select everything inside it). This slice supports click and shift-click
  only (criterion 10).
- The distinction between Inkscape's "smooth" and "symmetric" node types.
  This slice merges them into one "smooth" type with mirrored, equal-length
  handles (criterion 9); splitting that into independently-adjustable-length
  "smooth" versus always-equal "symmetric" is a later refinement if the
  customer asks for exact Inkscape parity here.
- Shape-preserving node deletion (Inkscape's curve-refitting behaviour on
  Delete, and its separate Ctrl+Delete for the simpler kind). Criterion 13
  defines one, simpler deletion behaviour for this slice.
- Keyboard nudging of selected nodes (arrow keys), numeric entry of node
  coordinates or handle angle/length, and node alignment/distribution tools.
- "Break path at node" and "join two endpoint nodes" operations.
- Snapping (to grid, to other nodes, to guides).

## UX notes

This is the first feature with any canvas interaction and the first toolbar
this product has (`project-file-foundation` had none). Tokens introduced here
(colors, sizes, the hover/selection rule) now live in
`docs/design-system.md`, seeded by this slice — later features extend that
file instead of inventing tokens inline.

### Tool rail and tool switching

- First toolbar: a vertical icon rail, 48px wide, docked to the left edge,
  spanning from the native menu down to the status bar. Canvas fills the
  remaining width edge-to-edge — the rail is chrome *beside* the canvas, not
  a frame around it (`project-file-foundation`'s "chrome never frames the
  canvas" precedent holds; see `docs/design-system.md`).
- Two buttons only, top to bottom: **Pen** then **Node**. Later tools append
  below; this slice doesn't reorder for them.
- Each button is a real focusable control (tab-reachable, `aria-label`
  "Pen tool (B)" / "Node tool (N)"), tooltip on hover shows name + shortcut.
  Active tool: filled `--accent` background, white icon, stays active until
  another tool is chosen. No separate "Tools" native menu in this slice —
  the rail is the only entry point; revisit if a keyboard-only/screen-reader
  gap shows up later.
- **Shortcuts: `B` for Pen, `N` for Node** — matches Inkscape exactly rather
  than inventing new bindings. R-EDIT-001 asks for Inkscape's mental model and
  the user value explicitly calls out a maker switching tools gradually; the
  one thing that would undermine that is shortcuts that almost match but
  don't. These are canvas-focus single-letter shortcuts (Inkscape/Illustrator
  convention), a different mechanism from slice 1's native-menu Ctrl/Cmd
  accelerators — see `docs/design-system.md`'s shortcut table, which now
  tracks both kinds in one place.
- **Default tool on an empty/new canvas: Pen.** There's nothing to select or
  edit yet, and Pen is what lets the maker start immediately. (Revisit once a
  general selection tool exists — out of scope here.)
- **Cursors:**
  - Pen tool, idle/placing: pen-nib cursor.
  - Pen tool, hovering the in-progress path's own first node (the AC5 close
    target): cursor swaps to a pen-with-small-circle ("close path") variant,
    *and* that first node gets the hover ring (below) — two signals, not one,
    since closing a path is a one-way commit the maker should be confident
    about before clicking.
  - Pen tool mid-drag (placing a curved node, AC2): cursor stays the plain
    pen-nib; no extra swap during the drag itself.
  - Node tool: standard arrow, always — including over nodes, handles and
    segments. Shape/hover feedback comes from the glyphs themselves (below),
    not from cursor changes, so there's one fewer state to keep in sync.

### Node and handle visual convention

**2026-10-05 note:** the pixel sizes below (handle endpoint, hit-test
radii, hover ring) are what this slice shipped with; several have since
changed (the handle endpoint doubled to 12px, its hit-test radius to
16px, and it gained its own 16px hover ring, `canvas-interaction-bugs`
follow-up). `docs/design-system.md`'s own token table is the live source
of truth for current sizes — this file is not updated in place, so it
stays an accurate record of what was true when this slice shipped.

Follows Inkscape's shape convention exactly, minus the circle/"symmetric"
node shape — this slice merges smooth and symmetric into one type (see "Out
of scope"), so only two node shapes exist:

- **Corner node:** 7×7px square, screen-space constant (never scales with
  zoom). Unselected: white fill, `--node-stroke` outline. Selected: filled
  solid `--accent`.
- **Smooth node:** same 7px glyph, rotated 45° to a diamond. Same fill rule.
- **Handle endpoint:** 6px circle. Idle (node selected but handle not being
  dragged): white fill, `--accent` outline. Being dragged: filled `--accent`.
- **Handle line:** 1px solid `--accent`, from node center to handle endpoint.
- **Visibility (AC7):** handles and handle lines render only when their node
  is selected. An unselected node shows no handles regardless of type; a
  selected corner node with no handles pulled out shows none until the maker
  drags one out (nothing to draw yet).
- **Hover ring:** any node or handle under the cursor, whether or not
  selected, gets a faint outer ring — a 10px circle at `--accent-hover` — so
  "about to click" is always visible without a cursor swap (node tool's
  cursor never changes, per above).
- **Hit-testing:** 8px screen-space radius around every node/handle center,
  independent of the 7px/6px glyph size — the glyph can stay small and
  precise-looking while the clickable area stays forgiving. Segment
  hit-testing (AC14, AC12's double-click-to-insert): 4px perpendicular
  screen-space tolerance. Both values are in `docs/design-system.md` so
  later point/curve-editing tools (primitives, slice 3) reuse them rather
  than picking their own.
- **In-progress path (actively drawing, before commit):** already-placed
  nodes/handles render in the same shapes but hollow/outline-only in
  `--accent` (not the filled "selected" style) — distinct from a committed
  path's node-tool selection state, so a maker never reads "still drawing"
  as "already selected with the node tool". The most-recently-placed node
  additionally gets the hover ring treatment permanently (not just on
  hover), marking it as where the next click/drag extends from.

### In-progress path feedback (pen tool)

- **Rubber-band preview:** while the mouse is up and the pointer moves after
  at least one node is placed, a 1px dashed `--accent` line runs from the
  last placed node to the current cursor position — showing where a plain
  click would land (AC1's straight-segment case).
- **Live curve preview while dragging a node's handles (AC2):** as the maker
  drags from the new point C, show both symmetric handle lines/endpoints
  growing from C in real time, *and* reshape the B→C segment live as a curve
  using C's handle (standard rubber-band-with-curve feedback, same as
  Inkscape) — not just a straight dashed placeholder that snaps to curved
  only on release.
- These previews are editing UI per the conventions above: screen-space,
  WebGL draw list, never part of the committed document.

### Selection feedback (nodes, handles, segments)

First feature with any selectable canvas content, so this is the baseline
every later tool's selection feedback follows:

- Selected node/handle: filled `--accent` (above).
- **Selected segment (AC14):** a 2px screen-space `--accent` line drawn on
  top of the segment's own 0.25mm stroke, for its full length between the
  two endpoint nodes — additive, so the real stroke stays visible under it
  and nothing about the document's own rendering changes.
- Multi-selected nodes (AC10): every selected node shows the identical
  "selected" glyph; no primary/secondary distinction — the ACs don't need
  one and inventing one here would be a precedent nothing asks for.
- Hover vs. selected use the same accent at two opacities (`--accent-hover`
  vs. `--accent`), per `docs/design-system.md` — one rule reused everywhere,
  not a per-glyph choice.

### Node-tool actions (AC11, AC13, AC14)

AC11's "chooses ... from the node tool's actions" and AC14's "make line" /
"make curve" are left open by the spec on purpose — decided here:

- A **contextual tool-controls bar** appears directly under the main menu
  (above the canvas, full width, same row Inkscape itself uses for this)
  whenever the node tool is active: icon buttons for Insert node, Delete
  node, Make corner, Make smooth, Make line, Make curve. Buttons disable
  (not hide) when nothing selected/applicable, e.g. "Make line" disables when
  the selected segment is already a line.
- Same actions are also on a right-click context menu over a selected
  node/segment — belt-and-suspenders discoverability, no memorization
  required.
- Delete is additionally bound to the Delete and Backspace keys (both — see
  `docs/design-system.md`). Insert/convert/make-line/make-curve get no
  keyboard shortcut in this slice: Inkscape's own bindings for these aren't
  single obvious letters, and guessing wrong would be worse than the
  toolbar/context-menu being the only path for now. Revisit if a later
  story asks for them.
- Not specified by any AC, decided here for a consistent baseline: Escape
  with the node tool active and a selection present clears the selection
  (no path is discarded — that's pen-tool-only, AC4); Escape with nothing
  selected is a no-op.

## Links
Requirements: R-EDIT-001 (`docs/requirements.md`)
ADRs: ADR 0002 §5 (node identity, movable-list anchors), §6 (curve
representation), §9 (command journal / one commit per interaction); ADR 0009
§3 (merge granularity for anchors — not exercised single-user, but shapes
which operations are well-defined per node/handle)
PR: https://github.com/Th3Link/vectormanufactoring/pull/7
