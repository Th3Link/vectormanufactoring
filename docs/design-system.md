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
| `--marquee-touch` | `#2FAE57` | New semantic color (`advanced-selection`), not a reuse of `--accent`: the marquee box's border and fill, and the lasso line, whenever the active mode is "touch" (crosses or fully contains selects it) — green, matching the customer's own naming. Deliberately distinct from `--accent` because this is a transient drag-mode indicator, not a selection state; don't read it as "selected." |
| `--marquee-touch-fill` | `--marquee-touch` at 12% opacity | Marquee box interior fill in touch mode — low-opacity so canvas content underneath stays legible while the box is open, matching LightBurn's own semi-transparent-fill-plus-solid-border convention |
| `--marquee-contain` | `#E5484D` | Marquee box border/fill color whenever the active mode is "contain" (fully-inside-only selects it) — red, matching the customer's own naming |
| `--marquee-contain-fill` | `--marquee-contain` at 12% opacity | Marquee box interior fill in contain mode, same reasoning as `--marquee-touch-fill` |
| `--marquee-legend-bg` | `--toolbar-bg` (`#DCDCE0`) | Background of the small on-canvas modifier-state legend shown during a marquee/lasso drag (`advanced-selection`) — reuses the existing chrome tone rather than inventing a new surface color |
| `--marquee-legend-fg` | `--toolbar-icon` (`#3A3A3F`) | Legend text color, same pairing as the toolbar's own icon-on-chrome contrast |

## Spacing and sizing

| Token | Value | Used for |
|---|---|---|
| Left tool panel width | 48px | Floating vertical icon panel (2026-10-05: floats over the canvas, inset 12px from the left edge; no longer a docked rail claiming layout width) |
| Left tool panel inset | 12px | Distance from the canvas's left and top edges to the floating panel |
| Tool icon size | 24px | Icon glyph inside a 48×48 button |
| Node glyph | 14×14px screen-space | Corner (square), Symmetric (diamond) and Asymmetric (triangle) node markers. Doubled from 7px (2026-10-05, customer feedback: "you can click on the nodes too — the node squares and diamonds need to be bigger too," the same complaint and fix already applied to the handle endpoint earlier the same round). `path-merge-split-and-node-types` separately renamed "smooth" to "Symmetric" and added the Asymmetric triangle — a genuinely different-sided polygon, not a further rotation of the square/diamond pair, so it stays distinct from both at a glance, sized against the doubled 14px box. |
| Handle endpoint | 12px diameter screen-space | Circle (2026-10-05: doubled from 6px — customer feedback called the handles "hard to hit... and very delicate/thin"; node glyphs followed with the same doubling later the same round, see above) |
| Handle line weight | 1px screen-space | Node-to-handle connector |
| Node hover ring | 18px diameter screen-space | Faint outer ring on a hovered node, or the pen tool's own placed-node/close-target rings — all rings drawn around a *node* (2026-10-05: grew from 10px, now computed as the node glyph's own 14px plus a 4px margin — once the node glyph doubled it exceeded the old fixed 10px ring, which would have hidden the ring fully behind the glyph's own opaque fill, the same bug class the handle hover ring split off to fix earlier the same round) |
| Handle hover ring | 16px diameter screen-space | Faint outer ring on a hovered handle (2026-10-05: split from the node hover ring, sized relative to the handle's own 12px glyph, and drawn after it — once the handle doubled past the old shared 10px ring, the ring drew fully behind, and so fully hidden by, the handle's own opaque fill). Kept as its own independent token rather than re-merged with the node hover ring now that both exist — the two glyphs can resize independently again in future, same reasoning as the hit-test radii's independent tokens |
| Segment selection overlay | +2px screen-space over the geometry's own stroke | Drawn on top, doesn't replace the real stroke |
| Node hit-test radius | 16px screen-space | Minimum clickable radius around a node, even though the visual glyph is smaller (Fitts's-law margin for mouse precision) (2026-10-05: doubled from 8px alongside the node glyph's own doubling, same request) |
| Handle hit-test radius | 16px screen-space | Same margin rule as the node radius, doubled alongside the handle's own doubled visual size (2026-10-05) — a visual-only size change would look right but still feel exactly as hard to hit, which is the opposite of the request. Now numerically equal to the node hit-test radius above (coincidence of two independent doublings, not a merge) — harmless: `vecmanf-ui-core::hit_test` picks the nearer candidate regardless of either tolerance's value, a handle winning only an exact tie |
| Segment hit-test tolerance | 4px screen-space perpendicular distance | Clicking "on" a curve/line segment |
| Shape handle | 8×8px screen-space | Hollow square, `primitive-shapes`: bounding-box resize, rectangle corner-radius, polygon/star inner-radius — deliberately square (never circular or diamond) so the two vocabularies never read as the same control, regardless of either one's size; no longer the larger of the two since the handle endpoint's 2026-10-05 doubling (12px), and now the *smaller* of the two since the node glyph's own 2026-10-05 doubling (14px) — this token's own size was not revisited either time |
| Shape handle hit-test radius | 16px screen-space | Same margin rule as the node radius (reused rather than invented fresh — `Session::shape_tolerances()` passes `self.point_tolerance()` straight through) — doubled from 8px as a direct, automatic consequence of the node hit-test radius doubling above, not a deliberate shape-handle-specific change. Now equal to the handle hit-test radius too (same coincidence as above) |
| Transform resize handle | 8×8px screen-space, 2px corner radius ("squircle") | Hollow `--accent` stroke / white fill idle, solid `--accent` fill while dragging — `object-transform`'s 8 Select-tool scale handles (4 corner + 4 edge-midpoint). Same footprint as the `primitive-shapes` shape handle on purpose (criterion 1 of that spec pins the hit size); the rounded corner is the one deliberate silhouette difference, since the two can never be on screen at once (different tools) but shouldn't read as pixel-identical regardless |
| Transform resize handle hit-test radius | 16px screen-space | Resolved by `object-transform`'s own implementation (this row was flagged, not fixed, until now): criterion 1 pins this to "the same hit size as `primitive-shapes`' own shape handles, whatever that is at build time" — the shape handle's hit radius doubled to 16px in the 2026-10-05 sizing round (row above), so the transform resize handle takes that same 16px, not the stale 8px this row said when the spec was first written |
| Transform rotate handle | 12×12px screen-space, circular-arrow icon glyph (not a dot): a 270° arc at 90% of the footprint, 2px stroke, with an arrowhead | `--accent` stroke / transparent fill idle, `--accent-hover` fill on hover, solid `--accent` fill with white glyph while dragging (`object-transform`). Deliberately an icon rather than a plain circle, and not connected to the bounding box by a stalk line, so it never reads as a reuse of the Bézier handle's line-plus-circle composition (`path-node-editing`) |
| Transform rotate handle offset | 32px screen-space, center to center | Distance from the top-edge resize handle to the rotate handle, measured along the bounding box's own local "up" axis (rotates with the box). Corrected 2026-10-06 (`object-transform` UX review): the original 20px was chosen against an 8px resize hit radius; with the resize radius now 16px (16 + 12 > 20) the two hit areas overlapped. 32px is the sum of the two 16px hit radii, so they touch but never overlap |
| Transform rotate handle hit-test radius | 16px screen-space | Raised from 12px in the same review so the rotate handle is as easy to hit as the resize handles (its 12px glyph is the smaller target, a larger radius makes up for it); equal to the resize radius, which is what lets the 32px offset above keep the two areas apart |
| Transform handle hit priority | — | A resize handle's hit radius shrinks to a third of the box's smaller side (never below a quarter of 16px), and a press *inside* the box only grabs a handle within 6/16 of that (scaled) radius of the box's edge — 6px at the full 16px radius, less on a small box; outside the box the handle always wins (`SelectTool::handle_at`). A press inside the sole selected object's box that is not on a handle (resize or rotate) starts a **move**, so a small unfilled object — whose outline the handle radii tile completely — can still be moved; an *unselected* object still hits only on its outline, and a press on empty canvas outside the selected box still deselects (`object-transform` `adrs.md`, 2026-10-06). Below a 24px box side (3 × the 8px glyph) only the four corner handles are drawn, so the glyphs do not merge; hit-testing is unchanged |
| Live transform readout | Pointer-anchored chip, 12px up and to the right of the pointer, `--toolbar-bg` / `--toolbar-icon`, 12px text | `object-transform` scale ("W × H mm", polygon/star "r R mm") and rotate ("37.4°", "45°" under Ctrl) readouts. The offset is shared by every tool's live readout (rectangle/ellipse/polygon create drags moved from 8px to 12px in the same change). Stays fully inside the canvas: flips to the left of the pointer when it would cross the right edge and below it when it would cross the top edge, then clamps (`frontend/src/lib/readoutPlacement.ts`) |
| Transform pivot marker | 6px diameter screen-space, `--accent` at 60% opacity | Shown at the active scale/rotate pivot point for the duration of a drag only (`object-transform`) — its position is the feedback for Shift's pivot-swap modifier; see that spec's UX notes. A handle sitting exactly on the pivot is not drawn while the marker is shown, so the dot never lands on a handle that then looks pressed |
| Bounding-box selection outline | 1px screen-space `--accent` (selected) / `--accent-hover` (hover) | Drawn around a selected/hovered primitive when its own matching tool is active (`primitive-shapes`) — **and, as of `canvas-navigation-and-selection`, around any selected/hovered object of any type (path or primitive) when the Select tool is active, with no shape handles or path nodes added on top.** Same token, two contexts: Select-tool selection is deliberately plain; the type-specific handles/nodes layer in only after double-click handoff into the object's own tool. |
| `PropertiesPanel` width | 280px, fixed | Right-docked panel (`stroke-and-fill-styling`'s `StylePanel` is its first section); canvas fills the remaining width |
| Contextual mini-toolbar padding | 6px | Floating per-selection toolbar (`NodeToolbar`'s actions, 2026-10-05), anchored near the current canvas selection rather than docked |
| Status bar zoom field | integer percentage, no decimals | New center segment (`canvas-navigation-and-selection`), between the existing cursor-position (left) and document-size (right) fields `project-file-foundation` already shipped; e.g. "100%", range 2%–8000% |
| Marquee/lasso stroke weight | 1.5px screen-space | Marquee box border and lasso line (`advanced-selection`) — deliberately heavier than the 1px bounding-box selection outline so the drag-feedback shape reads as a distinct, topmost layer over any selected objects' own 1px boxes still visible underneath (e.g. during a Ctrl-add drag) |
| Lasso line dash pattern | 4px on / 3px off, screen-space | Distinguishes the lasso's freehand line from the marquee box's solid border at a glance, despite sharing a color family (`advanced-selection`) |
| Marquee/lasso modifier-state legend | small on-canvas label, positioned near the live cursor position | `advanced-selection`'s drag-mode readout, same convention as `primitive-shapes`' drag-to-create numeric readout (on-canvas, not status-bar) |

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
  pattern samples read better in a list) uses `Select` instead. Also used
  for the node-tool's kind control (`path-merge-split-and-node-types`):
  Make corner / Make symmetric / Make asymmetric, icons being the node
  glyphs themselves (square/diamond/triangle), replacing the two flat
  "Make corner"/"Make smooth" buttons `path-node-editing` shipped.
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
- **Transform-handle cursors** (`object-transform`, new): each of the 8
  resize handles shows a custom, rotated double-headed-arrow cursor, not one
  of the browser's four fixed resize cursors — the oriented bounding box
  means a handle's screen-space direction is `object rotation + handle's
  own base angle`, almost never one of those four fixed angles once the
  object is rotated, so the cursor image is rotated live to match. The
  rotate handle shows a separate, non-rotating circular-arrow rotate cursor
  (rotate has no single axis to align to, so it stays static regardless of
  handle angle). Both apply only within the handle's own hit radius;
  elsewhere inside the bounding box the cursor is the Select tool's normal
  move/arrow cursor (object-body drag, unchanged from
  `canvas-navigation-and-selection` criterion 20).
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

- **Switch (`ScaleStrokeSwitch`)** (2026-10-06, `object-transform`
  criteria 8, 26-31, new): the pattern for one persistent on/off setting
  that is tool state, not an object property. First use: "Scale stroke
  width". Spec of the control, which is identical wherever it is hosted:
  - **Semantics:** Radix `Switch` (`radix-ui` is already a dependency),
    `role="switch"`, `aria-checked`, wrapped in a `<label>` so the whole
    row (text and track) is the click target. One Tab stop; Space toggles
    (Enter does not, native switch behaviour). Never disabled, whatever is
    selected, including nothing (criterion 30). No keyboard shortcut is
    assigned now; Tab + Space is the keyboard path.
  - **Label:** "Scale stroke width", sentence case, `text-sm` (14px),
    `--toolbar-icon`, to the *left* of the track, 8px gap. The label is the
    accessible name; do not add "On/Off" text, state is shown by the track.
  - **Track:** 32 x 18px, fully rounded. **Off:** transparent fill, 1.5px
    `--toolbar-icon` border, 12px `--toolbar-icon` thumb at the left, 2px
    inset. **On:** `--toolbar-icon-active-bg` fill and border, 12px
    `--toolbar-icon-active-fg` thumb at the right. State is carried by thumb
    position and fill, never by color alone. Off border is 8:1 and on fill
    is 3.3:1 against `--toolbar-bg`, both above the 3:1 non-text minimum.
    Thumb moves 100ms ease-out; no motion under `prefers-reduced-motion`.
  - **Row:** 28px high, 8px horizontal padding, `rounded-md`. Hover:
    `--editor-accent-hover` behind the whole row. Focus-visible: 2px
    `--editor-accent` ring with 1px `--toolbar-bg` offset (the grey `--ring`
    the other toolbar buttons use is only 2.4:1 on `--toolbar-bg`; bring
    those up to this ring when next touched).
  - **Tooltip** (Radix `Tooltip`, 400ms delay, same styling as the tool
    rail's, `side="bottom"`): "Scale stroke width with the object. Off: a
    resize keeps the stroke thickness." Also on the focus ring's element
    for keyboard users (Radix shows it on focus).
  - **State** lives in `useEditorSession` (`scaleStrokeWidth`,
    `setScaleStrokeWidth`), not in the component, defaults to off, and is
    reset to off by `newProject()` and `openProject()` (criterion 27). It is
    never persisted. The press handler reads it once at pointer-down
    (criterion 28).
  - **Hosts.** *Interim (slice 5):* `SelectToolbar`, a contextual bar shown
    whenever `editor.tool === "select"`, whether or not anything is
    selected. It uses the same slot, container and look as `NodeToolbar`/
    `ShapeToolbar` (the `pointer-events-none` overlay row at `top-3`, left
    of which sits the tool rail: pill `h-9`, `rounded-lg`, `px-2`,
    `--toolbar-bg`, `--panel-elevation-shadow`, `pointer-events-auto`), so
    no canvas resize and the same tab order: tool rail, canvas, bar. It
    holds this one control. The key handler for Space-to-pan must not
    `preventDefault` when focus is inside the bar. *Final (slice 7,
    `stroke-and-fill-styling`):* the "Transform" section of
    `PropertiesPanel`, below "Style": one full-width row, label left,
    same switch right-aligned, optional 12px `--toolbar-icon` hint line
    below ("Off: a resize keeps stroke thickness."). The same component and
    the same session state, so no behaviour change; `SelectToolbar` is
    deleted in that slice, never shown alongside the panel row.

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
| Toggle "Scale stroke width" | none; Tab to the switch, Space | `object-transform`; tool state, off per session, never saved; no letter shortcut until usage shows one is needed |
| Open/focus the style panel | Shift+Ctrl+F | Matches Inkscape's Fill & Stroke binding; app-global, not canvas-focus-scoped (`stroke-and-fill-styling`) — see note below |

Single-letter tool shortcuts are a different category from the File menu's
native accelerators (table above) — they're bound at canvas-focus scope, the
same convention Inkscape/Illustrator/Affinity use, not native menu items.
The style-panel shortcut is a third category: app-global (works regardless
of canvas focus), since the panel it targets isn't canvas content.
