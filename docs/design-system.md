# Design system

Tokens and interaction conventions for the desktop app shell (Tauri + React +
Tailwind + shadcn/ui, ADR 0001). Seeded by `path-node-editing`
(`specs/0002-path-node-editing/specification.md`), the first feature that needs
real color/spacing tokens rather than the one inline `--canvas-bg` value
`project-file-foundation` used as a placeholder. Extend this file in place as
each later feature introduces new components or states; don't invent tokens
inline in a spec once they exist here.

Only one theme exists (light). Dark mode is undecided (no ADR yet) — token
*names* are chosen so a future dark theme is a value swap, not a rename.

## 2026-10-05: chrome architecture change (Blender/Affinity direction)

The customer tested slices 1–3 on a real display and asked for a direction
change, in their own words: tools on the side as over-canvas buttons, the
right side fixed and holding everything configurable, no menu bar at the
top, and the tool-specific contextual row floating over the canvas
(Affinity's convention) instead of shifting layout on tool switch. This
section is the decision, replacing the per-slice chrome choices below where
they conflict (the `project-file-foundation`, `path-node-editing` and
`primitive-shapes` UX notes still describe what shipped; this is what it
changes to).

**Native OS menu: unchanged, stays native.** "No menu bar at top" is read
here as targeting the in-canvas contextual tool-options bars
(`NodeToolbar`, `ShapeToolbar`) — the things a maker actually watched
appear, disappear and shift the canvas — not the native OS File menu. The
native menu lives in OS chrome (the title bar on Windows/Linux, the screen
top on macOS), claims no canvas layout space, and never shifts anything; it
isn't what "the tools shift position" was describing, and the customer's
Blender reference itself keeps a persistent top application menu bar
(File/Edit/Render/Window/Help) separate from per-tool floating controls. If
this reading is wrong, it's a one-line follow-up (move File into the right
panel as a menu button) — but nothing observed points at the native menu,
so it is not touched here.

**Left side: floating icon buttons, not a docked rail.** The tool rail
(`ToolRail.tsx`: Pen, Node, Rectangle, Ellipse, Polygon/Star) stops claiming
permanent layout width next to the canvas and becomes a floating panel
*positioned over* the canvas — inset 12px from the left edge, vertically
centered or inset from the top (match Blender/Affinity's left-edge-flush
toolbar, not a free-floating palette the maker can drag around; this one
stays pinned). Same 48×48px buttons, same icons, same order, same
`--toolbar-bg`/`--toolbar-icon*` colors, but now on a rounded card with a
drop shadow (`--panel-elevation-shadow`, new token below) so it reads as
"floating over" rather than "framing" the canvas. This is CSS positioning
only (`position: absolute`/`fixed` within the canvas's own stacking
context) — DOM order, focusability, `aria-label`s, tooltips and the `B`/
`N`/`R`/`E`/`*` shortcuts are all unchanged, so the keyboard-access and
discoverability properties `path-node-editing` and `primitive-shapes`
already established still hold without modification. One new requirement
this introduces: `--toolbar-bg` must stay fully opaque (not a translucent
overlay), so the floating panel's own contrast and its buttons' focus rings
are guaranteed regardless of whatever canvas content sits behind it.

**Right side: a persistent, fixed "Properties" panel — generalized, not
`StylePanel`-only.** What `stroke-and-fill-styling` designed as a
dedicated, dockable `StylePanel` becomes the first occupant of a
general-purpose properties-panel architecture that every later slice's
object properties (dimensions, future transform/align, machine/material
role) also plugs into. Decided now, concretely:

- **One scrolling panel of stacked, named sections, not tabs.** Blender's
  actual Properties editor is tabbed because it holds 15+ categories
  (Object, Modifiers, Material, Physics, ...); this product has exactly one
  category today (style). Building a tab strip for one tab is the kind of
  speculative abstraction `CLAUDE.md` §5 rules out ("no generic parameter
  unless at least two concrete types use it now" — the same instinct
  applies to a tab strip with one tab). Each section is a labeled
  card/divider (first: "Style", covering stroke + fill exactly as
  `stroke-and-fill-styling` specified); add tabs only once a second section
  makes a single scroll genuinely unwieldy — revisit then, not speculatively
  now.
- **Empty/placeholder state, generalized from `stroke-and-fill-styling`'s
  own rule:** when nothing selected, or when a section has nothing relevant
  to the current selection, that section's controls show disabled/blank
  (per-control mixed-state rules already specified), never collapse or
  disappear. This keeps the panel's own shape stable across every selection
  change — required by the no-layout-shift rule below, not just a style
  preference.
- Placement, width (280px), collapsibility and the `Shift+Ctrl+F` shortcut
  from `stroke-and-fill-styling`'s UX notes all carry over unchanged; see
  that spec's amended UX notes for the renamed `PropertiesPanel` framing.

**Tool-specific contextual controls: split by what they actually are, not
one bucket.** The customer's ask ("the tool-specific row... floating over
the canvas, like Affinity") and the "everything configurable on the right"
rule point at two different kinds of control that today's `NodeToolbar` and
`ShapeToolbar` conflate:

- **Transient per-action controls** — `NodeToolbar`'s Insert node, Delete
  node, Make corner, Make smooth, Make line, Make curve. These only mean
  anything relative to a current node/handle/segment selection and have
  nothing to show when nothing applicable is selected. These move to a
  **small floating mini-toolbar anchored near the current selection**
  (Affinity's own convention, exactly as asked for) — canvas-space anchored
  like the existing rubber-band preview, appears beside the selected
  node/segment, disappears with the selection. Still also reachable via the
  existing right-click context menu (belt-and-suspenders, unchanged from
  `path-node-editing`'s UX notes) and the Delete/Backspace shortcut.
- **Persistent tool configuration** — `ShapeToolbar`'s mode toggle
  (Polygon/Star), point-count stepper and ratio field. Acceptance criterion
  10 requires point count to persist "not reset between shapes," which is
  exactly what a fixed properties panel is for and exactly what a
  per-selection floating bar cannot do (there's nothing selected yet while
  choosing a point count before the first drag). These move into the
  **right Properties panel**, as a contextual "Shape tool options" section
  visible while a shape-creation tool is active, syncing live with the
  on-canvas ratio handle exactly as `primitive-shapes` already specified.

**Hard rule: no layout shift on tool switch.** The canvas viewport's own
size and position never change when the maker switches tools. Every piece
of contextual chrome this section describes — the floating left tool panel,
the floating per-selection mini-toolbar, the right panel's *section*
contents changing — is an overlay or an in-place content swap, never a
layout participant that resizes the canvas. (The right panel's own
collapse/expand, user-triggered via its chevron tab or `Shift+Ctrl+F`, is a
deliberate width change the maker asked for — not a tool-switch side
effect, and not what this rule forbids.) Concretely for implementation:
`App.tsx`'s current `<NodeToolbar>`/`<ShapeToolbar>` siblings stacked above
the `flex min-h-0 flex-1` row containing `<ToolRail>`/`<Canvas>` are exactly
the mechanism that violates this rule today — see the punch list reported
alongside this change.

**2026-10-03, frontend wiring note:** `--accent`/`--accent-hover` below are
implemented in `frontend/src/index.css` as `--editor-accent`/
`--editor-accent-hover`, same values. This project's shadcn setup already
reserves plain `--accent`/`--accent-foreground` for its own component hover
states (menus, dropdowns); redefining it here would silently recolor those
too. `--node-fill`/`--node-stroke`/`--handle-fill`/`--handle-stroke` need no
CSS variable at all — the canvas is WebGL, not DOM, so `vecmanf-render-core`'s
`theme.rs` is where these values actually live; this table documents the
values, not an implementation site, for those four rows.

## Color tokens

| Token | Value | Used for |
|---|---|---|
| `--canvas-bg` | `#E8E8EB` | Canvas background (set in `project-file-foundation`, value fixed here) |
| `--statusbar-bg` | `#DCDCE0` | Status bar background |
| `--toolbar-bg` | `#DCDCE0` | Tool rail background (same tone as status bar — both are chrome, not canvas) |
| `--toolbar-icon` | `#3A3A3F` | Default tool icon color |
| `--toolbar-icon-active-bg` | `#2F6FEE` | Selected tool's button background |
| `--toolbar-icon-active-fg` | `#FFFFFF` | Selected tool's icon color |
| `--accent` | `#2F6FEE` | The one accent color for "selected"/"active" across the whole app: selected nodes/handles/segments, the active tool button, future selection of any kind. Do not introduce a second accent color for a new kind of selection — reuse this one. |
| `--accent-hover` | `#2F6FEE` at 20% opacity | Hover state for anything `--accent` can select: node/handle hover ring, segment hover highlight, toolbar button hover background. One rule, not a per-component choice: hover = accent at 20%, selected = accent at 100%. |
| `--node-fill` | `#FFFFFF` (unselected) / `--accent` (selected) | Node glyph fill |
| `--node-stroke` | `#3A3A3F` | Node glyph outline, both states |
| `--handle-fill` | `#FFFFFF` (idle) / `--accent` (selected or being dragged) | Handle endpoint fill |
| `--handle-stroke` | `--accent` | Handle endpoint outline and handle line color |
| `--shape-handle-fill` | `#FFFFFF` (idle) / `--accent` (being dragged) | Shape handle fill (`primitive-shapes`) |
| `--shape-handle-stroke` | `--accent` | Shape handle outline, and the primitive bounding-box selection outline |
| `--shape-handle-guide` | `--accent-hover`, dashed | Corner-radius connecting guide — dashed, to read as distinct from the solid Bézier handle line above |
| `--panel-bg` | `--toolbar-bg` (`#DCDCE0`) | `PropertiesPanel` (formerly `StylePanel`) and any later section it hosts — reuses the one chrome color rather than adding a second (`stroke-and-fill-styling`) |
| `--panel-elevation-shadow` | `0 2px 8px rgba(0,0,0,0.24)` | Drop shadow on every floating chrome surface introduced 2026-10-05: the left tool panel and the per-selection contextual mini-toolbar — what makes them read as "floating over" the canvas rather than framing it. The right Properties panel is docked, not floating, and does not use this token. |

## Spacing and sizing

| Token | Value | Used for |
|---|---|---|
| Left tool panel width | 48px | Floating vertical icon panel (2026-10-05: floats over the canvas, inset 12px from the left edge; no longer a docked rail claiming layout width) |
| Left tool panel inset | 12px | Distance from the canvas's left and top edges to the floating panel |
| Tool icon size | 24px | Icon glyph inside a 48×48 button |
| Node glyph | 7×7px screen-space | Corner (square) and smooth (diamond) node markers |
| Handle endpoint | 6px diameter screen-space | Circle |
| Handle line weight | 1px screen-space | Node-to-handle connector |
| Segment selection overlay | +2px screen-space over the geometry's own stroke | Drawn on top, doesn't replace the real stroke |
| Point hit-test radius | 8px screen-space | Minimum clickable radius around any node/handle, even though the visual glyph is smaller (Fitts's-law margin for mouse precision) |
| Segment hit-test tolerance | 4px screen-space perpendicular distance | Clicking "on" a curve/line segment |
| Shape handle | 8×8px screen-space | Hollow square, `primitive-shapes`: bounding-box resize, rectangle corner-radius, polygon/star inner-radius — deliberately square and larger than the 7px/6px node-tool glyphs so the two vocabularies never read as the same control |
| Shape handle hit-test radius | 8px screen-space | Same margin rule as node/handle hit-testing, reused rather than invented fresh |
| Bounding-box selection outline | 1px screen-space `--accent` (selected) / `--accent-hover` (hover) | Drawn around a selected/hovered primitive when its own matching tool is active (`primitive-shapes`) — **and, as of `canvas-navigation-and-selection`, around any selected/hovered object of any type (path or primitive) when the Select tool is active, with no shape handles or path nodes added on top.** Same token, two contexts: Select-tool selection is deliberately plain; the type-specific handles/nodes layer in only after double-click handoff into the object's own tool. |
| `PropertiesPanel` width | 280px, fixed | Right-docked panel (`stroke-and-fill-styling`'s `StylePanel` is its first section); canvas fills the remaining width |
| Contextual mini-toolbar padding | 6px | Floating per-selection toolbar (`NodeToolbar`'s actions, 2026-10-05), anchored near the current canvas selection rather than docked |
| Status bar zoom field | integer percentage, no decimals | New center segment (`canvas-navigation-and-selection`), between the existing cursor-position (left) and document-size (right) fields `project-file-foundation` already shipped; e.g. "100%", range 2%–8000% |

## Interaction conventions (apply to every later tool, not just this one)

- **All canvas editing-UI** — node/handle glyphs, handle lines, selection and
  hover highlights, and anything like them a later tool adds (primitive
  resize handles, alignment guides) — is drawn in the WebGL draw list
  (`vecmanf-render-core`, ADR 0001 §4), sized in constant screen pixels
  regardless of zoom level, and is never part of the document's exported
  content. DOM overlays are reserved for text input and for the
  accessibility cases ADR 0001 §4 names; none of this slice's UI needs one
  yet.
- **One accent, two states**: `--accent` at full opacity means
  selected/active; `--accent` at `--accent-hover`'s 20% means hovered/
  hoverable. Don't add a third selection color for a new widget — reuse this
  rule.
- **Left tool panel** (2026-10-05, supersedes the earlier docked-rail
  convention): floats *over* the canvas, inset 12px from the left and top
  edges, elevated with `--panel-elevation-shadow`, fixed 48px icon-column
  width. The canvas is never resized or repositioned to make room for it —
  it is an overlay, not a layout participant, which is what makes the
  no-layout-shift rule hold by construction. DOM order (hence tab order)
  stays the same as when it was a docked rail; only its CSS positioning
  changed. New tools are still appended top-to-bottom in ship order;
  existing icons don't get reordered for a later feature's convenience.
  **One exception, made explicit so it doesn't read as a precedent:** the
  Select tool (`canvas-navigation-and-selection`) goes *first*, above Pen,
  because it's the rail's new default/master tool (every reference tool
  this product tracks — Inkscape, Illustrator, Affinity — puts its
  selection tool at the top of its toolbox), not a peer creation tool being
  slotted in for convenience. Pen/Node/Rectangle/Ellipse/Polygon-Star all
  shift down one slot, keeping their own relative order. This is a one-time
  carve-out for "the tool that replaces the launch default"; a later tool
  that is just another creation tool still appends at the bottom.
- **Floating contextual mini-toolbar** (2026-10-05, new): the pattern for
  *transient*, selection-dependent tool actions — today, `NodeToolbar`'s
  Insert/Delete/Make-corner/Make-smooth/Make-line/Make-curve. Anchored in
  canvas space next to the current node/handle/segment selection (same
  anchoring idea as the rubber-band preview), elevated with
  `--panel-elevation-shadow`, appears only while a relevant selection
  exists and disappears with it — never a fixed-position, always-present
  bar, and never a layout participant either. Actions here remain
  additionally reachable via the right-click context menu and their own
  keyboard shortcuts where one exists (e.g. Delete/Backspace), unchanged
  from `path-node-editing`. Use this pattern for a control only when it is
  truly transient and tied to "what's selected right now" — a control that
  should persist across selections or across newly created objects belongs
  in the Properties panel below instead (see `primitive-shapes`' point
  count, moved there 2026-10-05).
- **`PropertiesPanel`** (2026-10-05, generalizes the `StylePanel` dock):
  docks to the right edge, native-menu-bar to status-bar span, fixed 280px
  width, canvas fills the remaining width edge-to-edge — this one *is*
  docked, not floating, because its whole point is to stay put regardless
  of selection or tool (the customer's "right side, fixed" ask). Collapsible
  via a chevron tab on its canvas-facing edge and `Shift+Ctrl+F`; collapsed
  width is 0. Structured as one scrolling column of named, stacked sections
  (not tabs — see the 2026-10-05 section above for why), each independently
  showing a disabled/placeholder state when nothing relevant is selected,
  rather than collapsing or disappearing — so the panel's own shape never
  changes as selection or active tool changes, keeping with the
  no-layout-shift rule. `stroke-and-fill-styling`'s stroke/fill controls are
  its first section; a shape tool's persistent configuration (point count,
  ratio) is a second, tool-contextual section. This is ordinary DOM app
  chrome, not canvas editing UI — it does not go through the WebGL draw
  list rule above.
- **Segmented icon control (`ToggleGroup`)**: the pattern for any small set
  of mutually-exclusive icon choices — started as a two-state toggle
  (`primitive-shapes`' Polygon/Star mode), generalized here to n states
  (join, cap, fill mode). One Tab stop per group; arrow keys move the
  selection within it (native `role="radiogroup"` behavior); each icon has
  its own `aria-label`. Reuse this instead of a `Select` dropdown whenever
  the choice set is small (≤4) and icons read faster than words — Join/Cap/
  Fill-mode all qualify, a longer list (e.g. the dash preset, 4 options but
  pattern samples read better in a list) uses `Select` instead.
- **`ColorAlphaPicker`**: the one shared color-with-alpha control
  (swatch + popover: saturation/hue area, hex input, alpha or opacity
  slider), introduced in `stroke-and-fill-styling` for stroke color, solid
  fill color, and gradient stop color/opacity. Don't add a second color
  picker component for a future feature that also needs color+alpha — reuse
  this one, same instinct as the single `--accent` rule above applied to
  color controls instead of selection color.
- **Mixed-state display on multi-select**: when selected objects differ on
  a property, show a type-appropriate placeholder rather than the
  first-selected object's value — empty field with muted "Mixed" text for
  numeric fields, a checkerboard swatch for `ColorAlphaPicker`, no option
  highlighted for a segmented control. Established in
  `stroke-and-fill-styling` as the house convention for every later
  multi-select-editing panel. **Extended by `canvas-navigation-and-
  selection`'s Select tool to selection *indicators*, not just property
  controls:** a heterogeneous multi-select (e.g. a path and a rectangle
  together) shows each object's own real selection box simultaneously,
  never a single merged box or an "N objects selected" text summary in its
  place — same underlying rule (show the true, possibly-mixed state rather
  than collapsing it), applied to the one UI where it costs nothing extra,
  since every selected object already draws the same plain Select-tool box
  regardless of its type (see "Bounding-box selection outline" above).
- **Pan cursor** (`canvas-navigation-and-selection`): standard grab/grabbing
  convention for the drag-initiated pans only — open-hand cursor from the
  moment Space is held or the middle mouse button is pressed, closed-hand/
  grabbing cursor for the drag's duration, reverting to the active tool's
  own cursor on release. Overrides the active tool's cursor for exactly the
  pan's duration, consistent with "panning never cancels or interrupts the
  active tool's in-progress state." Scroll-wheel pan and Ctrl+scroll/pinch
  zoom get no cursor change and no other overlay — they're instantaneous,
  not a held "mode," so the view (or the status-bar zoom readout) moving is
  feedback enough; don't add a transient on-canvas zoom popup.

## Keyboard shortcuts established so far

| Action | Shortcut | Notes |
|---|---|---|
| New / Open / Save / Save As | Ctrl/Cmd+N/O/S/Shift+S | `project-file-foundation`, native menu accelerators |
| Select tool | `S` | Matches Inkscape's Selector key (`canvas-navigation-and-selection`); launch default, replacing Pen |
| Pen tool | `B` | Matches Inkscape's Bezier/pen tool key |
| Node tool | `N` | Matches Inkscape |
| Rectangle tool | `R` | Matches Inkscape (`primitive-shapes`) |
| Ellipse tool | `E` | Matches Inkscape (`primitive-shapes`) |
| Polygon/star tool | `*` | Matches Inkscape (`primitive-shapes`); not a letter, kept anyway for the same parity reason as the others |
| Finish path | Enter (or double-click) | Matches Inkscape |
| Cancel in-progress path | Escape | |
| Delete selected node(s) | Delete or Backspace | Both bound; macOS keyboards label the backspace key "delete" |
| Open/focus the style panel | Shift+Ctrl+F | Matches Inkscape's Fill & Stroke binding; app-global, not canvas-focus-scoped (`stroke-and-fill-styling`) — see note below |

Single-letter tool shortcuts are a different category from the File menu's
native accelerators (table above) — they're bound at canvas-focus scope, the
same convention Inkscape/Illustrator/Affinity use, not native menu items.
The style-panel shortcut is a third category: app-global (works regardless
of canvas focus), since the panel it targets isn't canvas content.
