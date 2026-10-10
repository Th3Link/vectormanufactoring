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

- **Superseded 2026-10-10 (`0043-properties-tabs`, customer): the panel has a
  strip of three microtabs (Document, Style, History) and shows one context at
  a time.** The customer asked for Blender's microtabs once Style and History
  both existed. The rules are in "Properties panel: tabs"; the paragraph that
  follows is the reasoning that held until then and is kept as the record.
- ~~**One scrolling panel of stacked, named sections, not tabs.**~~ Blender's
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
- **Empty/placeholder state — superseded 2026-10-08 (`style-panel-rework`,
  customer):** the first version of this rule showed every control disabled
  when nothing applied. The panel is now empty when nothing is selected (frame
  and collapse tab only), and a control that cannot apply is removed, not
  disabled. The panel's width and the canvas never change with the selection,
  which is what the no-layout-shift rule below needs; the height of the content
  does change, and the panel scrolls. See "Properties panel: Style section".
  **Amended 2026-10-09 (`document-size-and-rulers`, UX):** "empty" now means
  that the Style area is empty. With nothing selected the panel shows the
  **Document section** instead (see "Properties panel: Document section"); it is
  fully empty only while the Pen has an unfinished path. Nothing else of the
  rule changes: no Style control, no subject line, no layout change.
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

  **Superseded 2026-10-07 (UX, `stroke-and-fill-styling`):** `unified-object-editing`
  made the shape tools creation-only and gave the Select tool a permanent bar
  (Radius, Points, Ratio, Object to path and the two switches); the polygon/star
  tool keeps its own bar for its creation options. Nothing in this bullet moved
  into the panel, and the panel has **no "Shape tool options" section**. The panel
  is built with the Style section only. A later story may move geometry
  parameters into a panel section; until one does, a property lives in exactly one
  place: appearance in the panel, geometry parameters and tool settings in the
  bars.

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
CSS variable at all — the canvas is WebGL, not DOM, so `curvyo-render-core`'s
`theme.rs` is where these values actually live; this table documents the
values, not an implementation site, for those four rows.

## Color tokens

| Token | Value | Used for |
|---|---|---|
| `--canvas-bg` | `#E8E8EB` | Canvas background (set in `project-file-foundation`, value fixed here). Since `0040` it is also the **default value of the document background** (`#E8E8EBFF`, stored per document): the document rectangle is drawn in the stored background, which equals this token until the maker changes it. A dark theme swaps the token, not stored backgrounds |
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
| `--shape-handle-fill` | `#FFFFFF` (idle) / `--accent-hover` (hover) / `--accent` (being dragged) | **Parameter handle** ground (`unified-object-editing`; the name stays from `primitive-shapes`, where it was the shape tools' handle fill). See "Parameter handle" below |
| `--shape-handle-stroke` | `--accent` | Parameter handle ring and center dot, and the 1px dashed selection box of a selected object |
| `--shape-handle-guide` | `--accent` at 60%, dashed (was `--accent-hover`, 2026-10-06 review: at 20% it measured about 1.1:1 and was invisible on the canvas, the same finding as the skew guide) | Corner-radius guide from the corner to a hovered or dragged radius handle — dashed, to read as distinct from the solid Bézier handle line above |
| `--preview-new` | `--accent` (`#2F6FEE`) | The "new" half of blue-new/black-old (`unified-object-editing`): the hollow outline of the geometry a release would commit. An alias, not a second blue; named so the preview can be re-coloured without touching selection. The "old" half has no token: it is the committed object in its own style |
| `--panel-bg` | `--toolbar-bg` (`#DCDCE0`) | `PropertiesPanel` (formerly `StylePanel`) and any later section it hosts — reuses the one chrome color rather than adding a second (`stroke-and-fill-styling`) |
| `--panel-elevation-shadow` | `0 2px 8px rgba(0,0,0,0.24)` | Drop shadow on every floating chrome surface introduced 2026-10-05: the left tool panel and the per-selection contextual mini-toolbar — what makes them read as "floating over" the canvas rather than framing it. The right Properties panel is docked, not floating, and does not use this token. |
| `--marquee-touch` | `#1C9347` | New semantic color (`advanced-selection`), not a reuse of `--accent`: the marquee box's border and fill, and the lasso line, whenever the active mode is "touch" (crosses or fully contains selects it) — green, matching the customer's own naming. Deliberately distinct from `--accent` because this is a transient drag-mode indicator, not a selection state; don't read it as "selected." **2026-10-08 (UX review):** darkened from `#2FAE57` (2.35:1 on `--canvas-bg`) to `#1C9347` (3.2:1 on the canvas, 3.95:1 on white, 5.3:1 on black), the same contrast as `--marquee-contain`. |
| `--marquee-touch-fill` | `--marquee-touch` at 12% opacity | Marquee box interior fill in touch mode — low-opacity so canvas content underneath stays legible while the box is open, matching LightBurn's own semi-transparent-fill-plus-solid-border convention |
| `--marquee-contain` | `#E5484D` | Marquee box border/fill color whenever the active mode is "contain" (fully-inside-only selects it) — red, matching the customer's own naming |
| `--marquee-contain-fill` | `--marquee-contain` at 12% opacity | Marquee box interior fill in contain mode, same reasoning as `--marquee-touch-fill` |
| `--marquee-legend-bg` | `--toolbar-bg` (`#DCDCE0`) | Background of the small on-canvas modifier-state legend shown during a marquee/lasso drag (`advanced-selection`) — reuses the existing chrome tone rather than inventing a new surface color |
| `--marquee-legend-fg` | `--toolbar-icon` (`#3A3A3F`) | Legend text color, same pairing as the toolbar's own icon-on-chrome contrast |
| `--field-invalid` | `#B3261E` | Border of a text field whose content was refused (`object-transform-refinements` numeric entry chip). 4.8:1 on `--toolbar-bg`. Always paired with a message line and `aria-invalid`, never the only cue. Not `--marquee-contain` (`#E5484D`): that is a drag-mode color and is only 2.9:1 on the chrome tone |
| `--destructive` | `--field-invalid` (`#B3261E`), an alias | `0020`, `0045` (2026-10-10): border and label of the action button of an inline confirm (Wipe, Delete), 4.8:1 on `--toolbar-bg`, 6.5:1 on white; fills with it (white label) on press and focus. An alias so the destructive and the error meaning can be split later without a rename. The word on the button names the action; the colour is never the only cue |
| `--lane-line` | `--toolbar-icon` at 60 % = `#7B7B7F` on `--panel-bg` | `0042` (2026-10-10): the lines of branch lanes and the dotted segment of undone steps in the History list. 3.1:1. The main lane uses `--toolbar-icon` at 100 %, 2 px |
| `--preview-ghost` | the previewed version's own fill and stroke at 40 % | `0042`: the lighter copy drawn under the `--preview-new` outline of an object preview. A constant, not a colour: it keeps the version's own colours so the maker sees what he would get |
| `--axis-guide` | `--accent` at 50% opacity (blended on `--canvas-bg` about 1.7:1; deliberately faint) | `edit-interaction-polish` (2026-10-06): the origin axis line the locked move runs along (see "Move axis guide"). Informational only: the lock is also readable from the motion, the lock badge and the readout, so it is exempt from the 3:1 non-text rule on purpose. Not a new colour: `--accent` with the opacity step between the hover (20%) and selected (100%) states |
| `--axis-guide-idle` | `--accent-hover` (`--accent` at 20%) | The other origin axis line, the one the move is not locked to |
| `--segment-hover` | `--accent` at 50% opacity | `0031-segment-drag-bending` (2026-10-10): the 4px hover band on a Node-tool segment that a press would bend (row "Segment hover highlight"). Not `--accent-hover` (20%): over a black stroke 20% accent is about RGB 9, 22, 48 and the flanks measure 1.25:1 on `--canvas-bg`, so the hover would be invisible next to a thin line (the skew guide had the same defect). The same opacity step as `--axis-guide`; about 2.0:1 on `--canvas-bg` and 1.55:1 on `--pasteboard-bg`, measured at build, raise toward 65% if a read falls under 1.5:1. Informational (the press result is the control), so exempt from the 3:1 non-text rule like `--axis-guide` |
| `--panel-muted-fg` | `--toolbar-icon` at 80% on `--panel-bg` = `#5A5A5F` | `0007`: subject line, info lines and other muted panel text. 5.0:1 on the panel |
| `--field-placeholder` | `--toolbar-icon` at 75% on white = `#6B6B6F` | `0007`: the "Mixed" placeholder of a field. 5.3:1 on a white field (the browser default grey is 4.6:1; this one is owned) |
| `--field-disabled-bg`, `--field-disabled-fg` | `#D0D0D4` (no border), `--toolbar-icon` at 80% on it = `#58585D` | `0007`: a disabled field. **Not used in the properties panel since `style-panel-rework`** (no control is disabled there); kept for the bars. Value 4.6:1; labels stay at full `--toolbar-icon` (8.3:1). The ground is not a 3:1 boundary on purpose: a disabled control is not operable |
| `--swatch-border` | `--toolbar-icon` | `0007`: 1px border of a colour swatch, 8.3:1 on the panel, so any swatch colour is bounded against it |
| `--checker-a`, `--checker-b` | `#FFFFFF`, `#C9C9CE` | `0007`: the checkerboard under a translucent swatch and the eyedropper chip's swatch (7px cells in a 28px swatch, 4px in the 16px chip swatch). Means "transparency shows through", nothing else. Since `0040` also the checkerboard of the document area (cells of 8 CSS px, row "Canvas checkerboard"); a to pasteboard 1.98:1, b to pasteboard 1.20:1, a to b 1.65:1 |
| `--mixed-hatch` | `#8E8E93` | `0007`: the 45 degree hatch (4px stripes of `--checker-a` and this, 3.3:1 between stripes) of a swatch whose colour differs across the selection. **Not a checkerboard**: a checkerboard is how a transparent colour looks, and the two must not be confusable |
| `--no-paint-slash` | removed | `style-panel-rework`: there is no swatch of an off paint any more (Paint None hides the colour). Delete from `index.css` |
| `--value-fill` | `--accent` at 28% over white = `#C5D7FA` | `style-panel-rework`: the filled part of a value field. Label and value on it 7.8:1, unit 4.7:1; 1.45:1 to the white ground, so the position is carried by the number and by `--value-edge`. A dark theme swaps the value, not the name |
| `--value-edge` | `--accent` | 2px line at the right end of the fill bar; 4.5:1 on white |
| `--picker-thumb-ring`, `--picker-thumb-casing` | `#FFFFFF`, `#000000` at 40% | `style-panel-rework`: the 2px ring and the 1px casing of the colour area and hue thumbs, so a thumb reads on every colour |
| `--selection-casing` | `#FFFFFF` | `0007`: white casing one line width wider on each side under every `--accent` line or glyph stroke without a white ground, so it stays visible over a fill (see "Casing over artwork") |
| `--hover-box` | `--accent` at 65%, with `--selection-casing` at 65% | `0007`: hover box of an object under the Select tool. Raised from `--accent-hover` (20%), which measured 1.0 to 1.3:1 on every fill. `--accent-hover` stays for rings, buttons and rows |
| `--member-box` | `--accent` at 60%, with `--selection-casing` at 60% | `multi-object-transform`: the box each object of a multi-selection keeps next to the group box (row "Member box"). 2.1:1 on `--canvas-bg`, below 3:1 on purpose (the group box, its handles and the panel's count carry the selection); fills not yet measured, the UX review accepts 1.9:1 or better. Same opacity as `--shape-handle-guide`, but dashed and cased, so the two never meet. Not a new hue |
| `--ruler-bg` | `--statusbar-bg` (`#DCDCE0`) | `document-size-and-rulers`: ground of both rulers and the corner square. Rulers are chrome, so the chrome tone; no new surface colour |
| `--ruler-tick` | `--toolbar-icon` | Major ticks and the 2px origin tick. 8.3:1 on `--ruler-bg` |
| `--ruler-tick-minor` | `--toolbar-icon` at 60% = `#7B7B7F` on `--ruler-bg` | Minor ticks. 3.1:1 |
| `--ruler-label` | `--toolbar-icon` | Ruler labels. 8.3:1 |
| `--ruler-edge` | `--toolbar-icon` at 25% = `#B4B4B8` | 1px line on the canvas-facing side of each ruler and of the corner; the panel's edge-line convention |
| `--ruler-pointer` | `--accent` | The pointer marker on each ruler. 3.3:1 on `--ruler-bg` (above the 3:1 non-text minimum). An alias, so it can be re-coloured without touching selection |
| `--pasteboard-bg` | `#B8B8BE`, flat | `document-size-and-rulers`: everything outside the document rectangle. The document rectangle is the document background (`--canvas-bg` by default, `0040`). Never picked by the eyedropper: app chrome, not a colour of the document. Measured: 1.61:1 to `--canvas-bg`, 1.44:1 to `--statusbar-bg` / `--toolbar-bg` (canvas to chrome is 1.12:1). On it: black 10.6:1, `--toolbar-icon` 5.7:1, `--accent` 2.3:1, white casing 2.0:1, `--field-invalid` 3.3:1. No shadow, no border; `--panel-elevation-shadow` stays for floating chrome. A dark theme swaps the value |

## Spacing and sizing

| Token | Value | Used for |
|---|---|---|
| Left tool panel width | 48px per card column | Floating vertical icon panels (2026-10-05: float over the canvas, inset 12px from the left edge; no longer a docked rail claiming layout width). Since 2026-10-10 the rail is a set of toolbox cards in one or two columns (rows "Tool rail architecture", "Toolbox card"); the tools card is 48px wide as before |
| Left tool panel inset | 12px | Distance from the canvas's left and top edges to the floating panel |
| Tool icon size | 20px glyph in a 40 x 40px button, 4px between buttons, 4px padding top and bottom of the 48px card (as built in `ToolRail.tsx`; the 24px glyph in a 48 x 48px button this row used to state was never built) | Lucide `size={20}`. The Boolean toolbox of the rail uses the same sizes (row "Boolean toolbox") |
| Node glyph | 14×14px screen-space | Corner (square), Symmetric (diamond) and Asymmetric (triangle) node markers. Doubled from 7px (2026-10-05, customer feedback: "you can click on the nodes too — the node squares and diamonds need to be bigger too," the same complaint and fix already applied to the handle endpoint earlier the same round). `path-merge-split-and-node-types` separately renamed "smooth" to "Symmetric" and added the Asymmetric triangle — a genuinely different-sided polygon, not a further rotation of the square/diamond pair, so it stays distinct from both at a glance, sized against the doubled 14px box. |
| Handle endpoint | 12px diameter screen-space | Circle (2026-10-05: doubled from 6px — customer feedback called the handles "hard to hit... and very delicate/thin"; node glyphs followed with the same doubling later the same round, see above) |
| Handle line weight | 1px screen-space | Node-to-handle connector |
| Node hover ring | 18px diameter screen-space | Faint outer ring on a hovered node, or the pen tool's own placed-node/close-target rings — all rings drawn around a *node* (2026-10-05: grew from 10px, now computed as the node glyph's own 14px plus a 4px margin — once the node glyph doubled it exceeded the old fixed 10px ring, which would have hidden the ring fully behind the glyph's own opaque fill, the same bug class the handle hover ring split off to fix earlier the same round) |
| Handle hover ring | 16px diameter screen-space | Faint outer ring on a hovered handle (2026-10-05: split from the node hover ring, sized relative to the handle's own 12px glyph, and drawn after it — once the handle doubled past the old shared 10px ring, the ring drew fully behind, and so fully hidden by, the handle's own opaque fill). Kept as its own independent token rather than re-merged with the node hover ring now that both exist — the two glyphs can resize independently again in future, same reasoning as the hit-test radii's independent tokens |
| Segment selection overlay | +2px screen-space over the geometry's own stroke | Drawn on top, doesn't replace the real stroke |
| Segment hover highlight | A **4px** wide band centered on the segment, `--segment-hover`, over its whole length between its two nodes (round caps at the node centers), drawn on top of the path's stroke and under the nodes, handles and every other overlay; constant screen width; clipped at the viewport edge, never drawn off screen. Shown with the Node tool, no button down, the pointer within the segment hit tolerance (4px) of a segment of an open or closed path and outside the node and handle radii of that path (what a press would bend, `0031` criterion 1). Absent for a compound path, a primitive, a zero-length segment, the already selected segment (it keeps the 2px `--accent` overlay) and while any drag runs. Cursor stays the arrow | `0031-segment-drag-bending`. Wider and paler than the selected overlay (2px, solid `--accent`), so hover and selected stay apart without colour alone. A segment under about 32px on screen lies inside its two nodes' 16px radii: no hover, cannot be bent (zoom in). The cost per frame does not depend on the node count of the path (spatial lookup) |
| Segment bend preview | While a segment is dragged to bend it, bottom to top: artwork exactly as committed ("black old") / for every segment whose shape the bend changes now (the dragged one and a neighbour changed through a Symmetric or Asymmetric node) the row "Live preview outline" (1.5px hollow `--preview-new`, white casing over filled artwork, no fill preview) / handle lines and endpoints of the two end nodes of the dragged segment on both sides, at their live positions every frame, in the **idle** look (1px `--accent` line, 12px circle, white fill, `--accent` ring; a handle of length zero is not drawn) / node glyphs / DOM (readout, badge). **The 2px selected-segment overlay of the dragged segment is not drawn while the bend runs** (two blue lines of different weight would not say which is old); it returns on the new shape on release and on the old one on Escape. No axis guide lines, no pivot marker, no pointer-carried dot. Cursor: the arrow | `0031`. Handles are idle on purpose: the `--accent` fill means "being dragged by the pointer" and the pointer drags the curve. The old handle positions are not drawn. The blue line starts and ends on the black one because no node moves |
| Bend readout and badge | The "Live transform readout" chip (12px up and right of the pointer) with the displacement d from the press point, in the move format: "Δ 12.5, −3.0 mm" (X right, Y down, one decimal, real minus sign, the locked axis "0.0"). While Shift limits d to one axis the Lock badge of the row "Modifier badge" shows (horizontal double arrow for a horizontal limit, vertical for vertical, switching in the frame the axis does). Both are gone on release, on Escape and on a cancelled pointer. The Node tool's rail tooltip gets a second line: "Node tool (N)" / "Drag a segment to bend it. Shift: one axis". No hint chip over a segment | `0031`. The readout is the one checkable number; it says nothing about handle lengths (they are the result). On WebKitGTK the Shift key-up is not delivered (`0014` plan.md): the lock and its badge are corrected at the next pointer move. Alt is not used (Linux window managers take Alt+drag); Ctrl is free and has no meaning in a bend |
| Node hit-test radius | 16px screen-space | Minimum clickable radius around a node, even though the visual glyph is smaller (Fitts's-law margin for mouse precision) (2026-10-05: doubled from 8px alongside the node glyph's own doubling, same request) |
| Handle hit-test radius | 16px screen-space | Same margin rule as the node radius, doubled alongside the handle's own doubled visual size (2026-10-05) — a visual-only size change would look right but still feel exactly as hard to hit, which is the opposite of the request. Now numerically equal to the node hit-test radius above (coincidence of two independent doublings, not a merge) — harmless: `curvyo-ui-core::hit_test` picks the nearer candidate regardless of either tolerance's value, a handle winning only an exact tie |
| Segment hit-test tolerance | 4px screen-space perpendicular distance | Clicking "on" a curve/line segment: the Node tool's segment pick and the shape tools' outline test. Stays 4px so that, where several segments of one path run close together, a wider radius does not make the right one harder to pick (`advanced-selection` criterion 2) |
| Object hit-test tolerance | 8px screen-space perpendicular distance | The Select tool's press, hover, cursor, copy and remove badges and double-click: an object is hit from 8px of its outline (`advanced-selection` criterion 1). Twice the segment tolerance, the same Fitts's-law doubling the node and handle radii got. The Alt-click cycle's reset radius is the same 8px |
| Shape handle | **Superseded 2026-10-06 (`unified-object-editing`): the shape tools no longer have handles; resize is the Transform resize handle, radius and inner radius are the Parameter handle below.** Former value: 8×8px screen-space | Hollow square, `primitive-shapes`: bounding-box resize, rectangle corner-radius, polygon/star inner-radius — deliberately square (never circular or diamond) so the two vocabularies never read as the same control, regardless of either one's size; no longer the larger of the two since the handle endpoint's 2026-10-05 doubling (12px), and now the *smaller* of the two since the node glyph's own 2026-10-05 doubling (14px) — this token's own size was not revisited either time |
| Shape handle hit-test radius | **Superseded 2026-10-06** (see "Parameter handle hit-test radius"). Former value: 16px screen-space | Same margin rule as the node radius (reused rather than invented fresh — `Session::shape_tolerances()` passes `self.point_tolerance()` straight through) — doubled from 8px as a direct, automatic consequence of the node hit-test radius doubling above, not a deliberate shape-handle-specific change. Now equal to the handle hit-test radius too (same coincidence as above) |
| Transform resize handle | 8×8px screen-space, 2px corner radius ("squircle") | Hollow `--accent` stroke / white fill idle, solid `--accent` fill while dragging — `object-transform`'s 8 Select-tool scale handles (4 corner + 4 edge-midpoint). Same footprint as the former `primitive-shapes` shape handle on purpose (criterion 1 of that spec pinned the hit size). Since `unified-object-editing` the parameter handle is on screen together with it, and differs by silhouette: a round knob (see "Parameter handle"), not a square |
| Transform resize handle hit-test radius | 16px screen-space | Resolved by `object-transform`'s own implementation (this row was flagged, not fixed, until now): criterion 1 pins this to "the same hit size as `primitive-shapes`' own shape handles, whatever that is at build time" — the shape handle's hit radius doubled to 16px in the 2026-10-05 sizing round (row above), so the transform resize handle takes that same 16px, not the stale 8px this row said when the spec was first written |
| Transform rotate handle | 12×12px screen-space, circular-arrow icon glyph (not a dot): a 270° arc at 90% of the footprint, 2px stroke, with an arrowhead | `--accent` stroke / transparent fill idle, `--accent-hover` fill on hover, solid `--accent` fill with white glyph while dragging (`object-transform`). Deliberately an icon rather than a plain circle, and not connected to the bounding box by a stalk line, so it never reads as a reuse of the Bézier handle's line-plus-circle composition (`path-node-editing`). Since `object-transform-refinements` there are up to eight, all identical and never rotated: four at the corners (always), four at the side midpoints (only while Shift is held, see "Transform handle layout") **Over filled artwork (`0007`):** the transparent ground is the one weak point; the arc and arrowhead get the white casing (the same rule for the skew handle) |
| Transform rotate handle offset | 32px screen-space, center to center | Distance from a corner resize handle to its corner rotate handle, measured along the box's own outward diagonal (22.6px on each local axis), and from a side midpoint to its side rotate handle along the outward normal. All measured in the box's own frame, so they rotate with it. Corrected 2026-10-06 (`object-transform` UX review): the original 20px was chosen against an 8px resize hit radius; with the resize radius now 16px (16 + 12 > 20) the two hit areas overlapped. 32px is the sum of the two 16px hit radii, so they touch but never overlap. (The old single rotate handle sat 32px above the top edge midpoint; that position is now the Shift-only top side handle.) |
| Transform rotate handle hit-test radius | 16px screen-space | Raised from 12px in the same review so the rotate handle is as easy to hit as the resize handles (its 12px glyph is the smaller target, a larger radius makes up for it); equal to the resize radius, which is what lets the 32px offset above keep the two areas apart |
| Transform handle hit priority | — | A resize handle's hit radius shrinks to a third of the box's smaller side (never below a quarter of 16px), and a press *inside* the box only grabs a handle within 6/16 of that (scaled) radius of the box's edge — 6px at the full 16px radius, less on a small box; outside the box the handle always wins (`SelectTool::handle_at`). A press inside the sole selected object's box that is not on a handle (resize or rotate) starts a **move**, so a small unfilled object — whose outline the handle radii tile completely — can still be moved; an *unselected* object still hits only on its outline, and a press on empty canvas outside the selected box still deselects (`object-transform` `adrs.md`, 2026-10-06). Below a 24px box side (3 × the 8px glyph) only the four corner handles are drawn, so the glyphs do not merge; hit-testing is unchanged. **2026-10-06 (`object-transform-refinements`):** with eight rotate handles, skew handles and a center handle the order "rotate first, then resize" is replaced by one nearest-center pass over every visible handle (caps and tie order in "Transform handle layout" below); the inner edge band above is unchanged, and no new handle has a hit area inside the box. **2026-10-06 (`unified-object-editing`):** parameter handles join the same pass at rank 0 (they win an exact tie, then resize, skew, rotate), with their own 12px cap and no inner band; they are the one handle family with a hit area inside the box, and exist only from a box side of 72px (see "Parameter handle layout") |
| Live transform readout | Pointer-anchored chip, 12px up and to the right of the pointer, `--toolbar-bg` / `--toolbar-icon`, 12px text | `object-transform` scale ("W × H mm", polygon/star "r R mm") and rotate ("37.4°", "45°" under Ctrl) readouts, and (`object-transform-refinements`) the skew readout "Skew x +12.5°" / "Skew y −8.0°" (x for top/bottom handles, y for left/right, real minus sign, one decimal, also under Ctrl: "22.5°"; the rotate readout likewise shows "22.5°" under Ctrl), and (`unified-object-editing`) the parameter readouts "r 3.5 mm" (rectangle corner radius; "r 12.0 mm max" when an unlinked drag is stopped by its limit, `rectangle-corner-radii`) and "ratio 0.45" (star). and (`edit-interaction-polish`) the move readout "Δ 12.5, −3.0 mm" after any axis lock (X right, Y down, one decimal, real minus sign; a locked axis shows "0.0"), with " Copy" appended while Ctrl is down, and the polygon/star create readout "r 12.0 mm, 75°" / "r 12.0 mm, ratio 0.50, 75°", and (`0044`) the keyboard nudge readout "Δ 3.0, 0.0 mm" (the distance of the running step, always millimetres whatever the display unit, not anchored at the pointer but 16px right of and below the centre of the selection's bounds like the typed-move chip, `aria-hidden`, gone 800 ms after the last move or at once on Escape, clamped inside the canvas). The offset is shared by every tool's live readout (rectangle/ellipse/polygon create drags moved from 8px to 12px in the same change). Stays fully inside the canvas: flips to the left of the pointer when it would cross the right edge and below it when it would cross the top edge, then clamps (`frontend/src/lib/readoutPlacement.ts`) |
| Transform pivot marker | 6px diameter screen-space, `--accent` at 60% opacity | Shown at the active scale/rotate pivot point for the duration of a drag only (`object-transform`) — its position is the feedback for Shift's pivot-swap modifier; see that spec's UX notes. A handle sitting exactly on the pivot is not drawn while the marker is shown, so the dot never lands on a handle that then looks pressed |
| Transform center move handle | 16×16px screen-space rounded square (3px radius), white fill, 1px `--accent` outline, four-way arrow glyph 10px wide (1.5px stroke, `--accent`) | `object-transform-refinements` item 1. Hover: `--accent-hover` over the white fill. Own move drag: solid `--accent` with a white glyph. Does not rotate with the box (a move runs along the screen axes). Drawn for a single selected object, or for the group box of a multi-selection (`multi-object-transform`; there it also has a real hit area, row "Group handle layout"), whose shorter side is 48px or more, and not while a resize, rotate or skew drag or a numeric entry is active (the pivot marker lives at the center then). It is a *visible name* for "press inside the box moves": no hit area of its own for a single object (a multi-selection's center handle has one, row "Group handle layout"), no priority against the other handles; it only gives hover feedback and the built-in `move` cursor, within min(12px, shorter side / 4) of the center. 48px (not 40): at that size the hover region is still 12px and the glyph keeps 12px clear of every resize handle |
| Transform skew handle | 18×12px screen-space footprint: two opposed parallel arrows (1.5px stroke, 3px heads) along the side | `object-transform-refinements` item 8, paths only. `--accent` stroke / transparent ground idle, `--accent-hover` rounded-rect ground (3px radius) on hover, solid `--accent` ground with white arrows while dragging. Rotates with the box: top and bottom handles point along the box's `u` axis, left and right along `v`. Two opposed arrows rather than one double arrow so it cannot be mistaken for a resize handle |
| Transform skew handle offset | 16px screen-space, center to center | From the side midpoint along the outward normal. Between the edge resize handle (0) and the Shift-only side rotate handle (32): glyph clearance 6px and 4px |
| Transform skew handle hit-test radius | 12px screen-space | 24px target. Overlaps its two neighbours on the same side on purpose; nearest center wins (see "Transform handle layout") |
| Transform handle layout | — | **One box, all in its own rotated frame.** Corner resize: at the corner. Edge resize: at the edge midpoint, `s` >= 24, for every object and every selection (a polygon or star gets its edge handles with `multi-object-transform` since 2026-10-10, if the customer accepts question 5; a stretch turns it into a path). Skew: path only, 16px outward from the side midpoint, shown per axis only when the box dimension across it is >= 24px (top/bottom arrows need the height, left/right the width: that is the lever of `atan(d / h)`). Corner rotate: 32px outward on the diagonal, always. Side rotate: 32px outward from the midpoint, only while Shift is held and no drag is running, all kinds. Center move: see its row, `s` >= 48. `s` = shorter side of the box on screen. **Hit rule:** one test serves press, hover and cursor; every visible handle competes by distance from the pointer to its center within its own cap (resize `min(16, s/3)` and at least 4, rotate 16, skew 12); ties resize, then skew, then rotate. Resize and rotate regions never overlap. Skew overlaps resize and side rotate; nearest center splits it: resize keeps 8px outward of the edge (plus slice 5's inner edge band), skew 8 to 24px (28 without Shift), side rotate from 24px. **Rules:** no handle added since slice 5 has a hit area inside the box, so a small box can always be moved by pressing it; Shift adds handles on the outermost ring and moves none; rotate handles are never hidden by size (they sit outside, and a tiny part still has to turn); clearance between any two glyphs is at least 4px at every size. Not hidden by size otherwise: slice 5's resize rules are unchanged. Off-screen handles are not pulled into the viewport. **Size tiers with parameter handles (`unified-object-editing`):** `s` < 24 corner resize + corner rotate; 24 to 47 + edge resize; 48 to 71 + center; >= 72 + parameter handles. Hidden first when a box shrinks: parameter handles, then center, then edge resize; corner resize and corner rotate never. Clearance of 4px between any two drawn glyphs holds at every `s` >= 72 and every radius, except that the center glyph is not drawn while a parameter handle center is within 20px of it |
| Transform skew fixed-line guide | 1px dashed (2px on / 2px off since `edit-interaction-polish` criterion 68, so it does not look like the 4 / 3 selection box it runs along; was 4 / 3 as the lasso), `--accent` at 100% (new token `--transform-guide`; **not** `--shape-handle-guide`) | Drawn during a skew drag along the line that stays put (the fixed edge, or the line through the box center under Shift), extended 16px past each end of the box. The pivot marker alone cannot say which line holds still in a shear. Revised after the UI review (2026-10-06): at `--accent-hover` (20%) the guide measured about 1.2:1 against the canvas and was effectively invisible; the fixed edge also coincides with the selection box edge, so only the extension past the box ends and the Shift center line can show it, and both must read. Full `--accent` is 3.9:1 on `--canvas-bg`. **UX review of PR 2 (2026-10-07, 1x pixels read from the GL buffer; CHANGES REQUESTED, fixed in the same PR and re-measured: the guide is one full-accent device row of whole 2 / 2 pixels at ratio 1 and 1.5, and the fixed edge shows 2 / 2 only):** (1) the guide is not pixel-snapped like the box, so at 1x its 2px dashes render as 2x2 blocks at about 50% on two rows, much fainter than the box; the extension past the box ends is the only place the guide shows and it reads as a pale dotted tail. (2) On the fixed edge the guide's 2 / 2 dashes fill the gaps of the box's 4 / 3 dashes and the edge turns into a heavy 2px nearly solid line, so it does not read as a guide and the box looks different on that one side. Rule to implement: the guide is snapped to whole device pixels with the same function as the box (full `--accent`, one device-pixel row), and while a skew drag is running the box does not draw its own dashes along the fixed edge (the guide replaces them: that side shows 2 / 2, the other three sides 4 / 3). On a rotated box the guide stays anti-aliased **`0007`:** also cased. |
| Transform entry chip | DOM overlay, `--toolbar-bg`, `--panel-elevation-shadow`, 8px radius; fields 28px high, white ground, 14px tabular text; z above the Select tool's bar | `object-transform-refinements` items 4, 5. Angle: one 80px field, fixed `°` suffix. Size: 6px padding, fields 100px wide 4px apart (84 overflowed: the "mm" suffix overlapped the last digit and a value of 1000 mm or more did not fit; the field keeps 32px of right padding for the suffix and 20px of left padding for the label), visible labels "W" and "H" (polygon/star "r", accessible name "Outer radius" since `unified-object-editing`, was "Radius"), fixed unit suffix, 12px chain glyph between W and H when linked. Placed outward of the handle (center-to-handle direction), 10px clear of its glyph edge, upright (never rotated with the object), clamped in the canvas with the readout's flip rule, follows the handle on zoom and pan. Open field: 2px `--editor-accent` border; other field 1px `--toolbar-icon` at 60%; invalid: 2px `--field-invalid` plus a 12px message line under the fields. The handle it belongs to stays in its dragging look while the chip is open. A chip opened by a key (`edit-interaction-polish`, "Keyboard concept") is placed exactly as if the maker had double-clicked the handle that key stands for, also when that handle is not drawn (a small box) or hidden. See "Numeric entry chip" below |
| Transform handle hint chip | DOM, text only, `--toolbar-bg` / `--toolbar-icon`, 12px, 8px padding, 8px radius, `pointer-events: none` | Appears 600ms after the pointer rests on a transform handle, anchored once 12px up and right of the pointer (readout placement), gone on press, leave or any key. One line per modifier (Shift, Ctrl, double-click). Key hints (`edit-interaction-polish`): the line that names double-click also names the key, "Double-click or M: type an offset" (center handle), "... or S: type a size" (resize), "... or R: type an angle" (rotate), "... or K: type an angle" (skew, paths); "or S", "or R" (PR 1), "or M" and "or K" (PR 3) are built; the left and right skew handles read "Double-click or Shift+K: type an angle". The center handle's hint lists "Shift: keep one axis" and "Ctrl: copy" only once PR 4 builds them. Parameter handles (`unified-object-editing`): "Corner radius" / "Double-click: type a value"; star "Inner radius" / "Double-click: type a ratio". The same chip, with 3 lines and a 3s life, answers a double-click on a primitive's body: see "Edit hint chip" below. Not a tooltip of the Radix kind: it is positioned over WebGL content and carries no focusable target |
| Parameter handle | 10px circle screen-space: 1.5px `--shape-handle-stroke` ring, `--shape-handle-fill` ground, 4px `--accent` center dot | `unified-object-editing`: the knob for a primitive's own parameter: the four rectangle corner radii, a star's inner radius, later an ellipse's arc and curve handles. Idle: white ground. Hover: `--accent-hover` ground. Dragging, or its entry chip open: solid `--accent` ground, white dot. **Followers:** while a radius drag changes all four corners (decided at the press: Link corners on and no Shift, or off and Shift), the other three take the hover ground; in a one-corner drag they stay idle. Drawn above every other handle. Round with a center dot is the one silhouette no other canvas glyph has (resize and center are squares, rotate an arc arrow, skew two arrows), so the two vocabularies read apart by shape, not only by position. Not rotated (a circle). Cursor: the built-in `pointer`, hover and drag, never changed by modifiers |
| Parameter handle hit-test radius | 12px screen-space | 24px target, same as the skew handle; not the 16px of resize and rotate because these handles are inside the box and four at 16px would cover most of a 72px box. Never shrinks: the handles exist only from a box side of 72px. Rank 0 in ties. Overlaps its neighbour at the largest radius (14px apart) by design; nearest center decides |
| Parameter handle layout | Threshold `T` = 72px (shorter box side `s`, screen). Rectangle radius handle: on the corner's inward diagonal at `p = 15 + ρ·L(s)` from the corner, `ρ` = effective radius / (`s`/2), `L(s)` = `(s − 14)/√2 − 15`. Drag: radius change = pointer displacement along that diagonal × `G(s)` = (`s`/2)/`L(s)` (1.38 at 72px, 1.09 at 100, 0.86 at 200, 0.71 for large boxes), so the handle stays under the pointer until a limit; exactly 0 at or beyond the 15px point | `unified-object-editing`. 15px inset: 4.3px glyph gap to the corner resize squircle (a 12px inset left 1.3px). At `ρ` = 1 the handle is `(s − 14)/2` from each side of its corner, so two neighbours are 14px apart, 4px between the 10px glyphs, at every `s`: criterion 8 holds by construction for corners that share a side, also when an unlinked corner (`rectangle-corner-radii`) has `ρ` above 1 (up to 2: a lone corner may equal the shorter side), because the limit keeps two radii on a side within that side and the map is linear. **Corrected 2026-10-07: this does not hold for diagonal corners** (TL/BR, TR/BL), which no radius limit couples: TL = BR = 0.6 s on a square is valid and both knobs would overlap. Rule: the knob is *drawn* at `ρ'ᵢ = min(ρᵢ, max(1, Σ − ρⱼ))`, `j` the diagonal partner, `Σ = 2 + √2·(S − s)/L(s)` (`S` longer box side on screen, so `Σ` = 2 on a square). Stored and effective values, outline, readout and bar field are unchanged; only the circle sits nearer its corner. Two diagonal knobs are then at least 14px apart along the longer side (distance 19.8px, glyph gap 9.8px, on a square), so the 4px glyph rule and `T` = 72 hold. A knob at or below `ρ` = 1 is always at its true position; where both partners exceed 1 on a square both rest at `ρ` = 1 and lag the pointer (the visible rule of a handle at a limit); a lone corner still reaches `ρ` = 2. The guide line follows the drawn knob. Drag arithmetic is unchanged (from the effective radius, frozen gain) Star inner-radius handle: at the first inner vertex; its worst case is a point count that is a multiple of 4 at ratio 0.99 against a corner resize glyph: 4.6px at `s` 72, 2.9px at 64, which is why `T` is 72 and not 64. The center glyph is not drawn while any parameter handle center is within 20px of it, nor during a parameter drag. Parameter handles are not drawn during a move, resize, rotate or skew drag of the same object, nor for two or more selected objects |
| Parameter handle guide | 1px dashed `--shape-handle-guide` | From the corner to a hovered or dragged radius handle only; not at rest |
| Live preview outline | 1.5px screen-space, `--preview-new`, hollow, no fill, solid, drawn on top of the committed geometry | `unified-object-editing`: for every Select-tool drag (move, resize, rotate, skew, radius, inner radius, also a slider drag in the bar) the geometry a release would commit is drawn in blue over the object **unchanged in its own committed style** ("black old": whatever it renders as now, not dimmed, not re-weighted). Constant screen width, independent of the object's stroke width. No fill preview yet. Typed values (entry chip, bar fields) do not preview. Selection box, handles, pivot marker and readout follow the new geometry and draw above the blue outline. A multi-selection draws one such outline per selected object at any count (no simplified preview, row "Group selection box"). Same weight as the marquee stroke, so at 1.5px it is at least as heavy as a hairline and reads on top where the two coincide **Over filled artwork (`0007`):** white casing 1.5px each side, drawn above all artwork including objects above the original in the tree. The committed object, fill included, stays as it is (a fill with no stroke has no black outline: its silhouette is the "old") |
| Select bar layout | Pill: every row is 36px (28px controls, 4px above and below), so the first row, and with it the switches, sits at the same y whether the bar has one row or two; controls 28px high, `text-sm`; 1px 20px 25% dividers between groups, 12px gap inside; **left-aligned after the tool rail** (`justify-start`) | `unified-object-editing`. Order: settings group ("Scale stroke width", "Scale corner radius": always shown, never disabled, never dimmed, first so they never move), kind groups (rectangle: Radius, Link corners toggle, Remove rounding; later ellipse, polygon/star), "Object to path" last. There is no Boolean group: the boolean operations are a section of the tool rail (customer, 2026-10-09; row "Boolean toolbox"). A control shows when the selection contains the kind it acts on, and acts on exactly those objects; it is enabled when it would change something; never shown disabled for an unrelated selection. Wraps by whole groups when the canvas is narrower than the row; a divider sits between two groups on the same row only, never at the start of a row (measured at build of `rectangle-corner-radii` PR 2 with a rectangle selected: the bar is 880px wide with the Link corners toggle and the 128px Radius field (the toggle is 40px of it), and stays on one row from a window width of about 964px (it wraps at 960px); the star row, 780px before, was not re-measured and does not change): settings stay on row 1. **Measured at build of `0035`** (800 x 600, panel open, rulers on, the bars' row 356px with the second rail column; switch labels `px-1`, 4px between the two switches, Radius field 96px): both switches fit one row, and every kind group wraps its controls inside itself when it does not (labels never wrap, `whitespace-nowrap`; the first row keeps its y); nothing selected 1 row (36px), one ellipse or polygon 2, one rectangle 3 (switches, Radius with Link corners and Remove rounding, Object to path: 108px), a rectangle with a polygon 3. Groups are top-aligned so a taller group does not move its neighbours. Fixed-width fields so nothing jitters. The Node and Shape bars stay centered. See "Select bar" below |
| Bar number field | 28px high, white ground, 14px tabular text, right-aligned, fixed `mm` suffix inside the right edge; Radius field 128px wide, 96px since `0035` (2026-10-06 review: 80px cannot hold a value, the 12px "limited" tag and the unit; 2026-10-07 `rectangle-corner-radii` review: at 112px a two-decimal value such as 11.34 was clipped next to the tag); Points field 80px ("Mixed" was clipped at 64px next to the native stepper) | The "Radius" field of the Select bar (rectangle selection): Enter commits once, Escape or a press elsewhere restores the shown value and writes nothing (the entry-chip rule), invalid: 2px `--field-invalid` and a 12px message line under the field ("Enter a number"), `aria-invalid`. Mixed: empty with the muted placeholder "Mixed"; one rectangle with unequal corners is Mixed too, its tooltip lists the four effective values ("Top-left 12, top-right 0, bottom-right 12, bottom-left 0 mm."), typing sets all four and ignores the Link toggle and Shift (`rectangle-corner-radii`) |
| Move axis guide | Two full-viewport lines through the selection's start center (the center handle's position at the press), one horizontal and one vertical, 1px solid screen-space, `--axis-guide` on the axis the move is locked to and `--axis-guide-idle` on the other; WebGL draw list, not exported, not hit-testable | `edit-interaction-polish` (customer: "dezent sichtbar, wie beim Object-Select-Hover"). Shown only while a move drag is past the dead zone with the axis lock engaged (Shift down); gone in the same frame Shift is released, the drag ends or Escape cancels. Re-chosen on every pointer event: the line the pointer is nearer to (the larger of |Dx| and |Dy|) takes `--axis-guide`, no latch, no hysteresis. Solid, not dashed: dashes mean "guide that carries a fixed-line meaning" (skew guide, lasso) and are held back until the dashed-box decision (Part E). Draw order, bottom to top: artwork, axis lines, blue preview outline, selection box, handles (the center handle glyph covers the crossing in copy mode), then DOM (readout, badges). It crosses the object it belongs to on purpose; at 1px and 50% it does not hide it |
| Modifier badge | DOM, `pointer-events: none`, `aria-hidden`; 16px circle, 1px white outer ring. **Copy:** solid `--accent` ground, white plus (8px, 1.5px stroke). **Lock:** white ground, 1.5px `--accent` ring, `--accent` arrow glyph 10px, horizontal double arrow while locked to x, vertical while locked to y, switching in the same frame the axis does | `edit-interaction-polish`: the visible "this will be a copy" (Ctrl) and "this is locked to one axis" (Shift). Placed with its center 16px left of and 16px below the pointer hotspot (clear of the arrow, move and crosshair cursors, and on the opposite side from the readout chip, which sits up and to the right); the second badge goes 20px further left; mirrored to the right of the pointer when the left side would leave the canvas; if it then meets the readout chip (readout flipped below near the top edge) it moves 28px further down. Updated from window-level key events directly, not from the next pointer move, so it is gone in the frame Ctrl or Shift is released and appears in the frame it is pressed. Copy badge: shown wherever a press with Ctrl would start a move (the press hit test, so it never lies), and during a copy drag. Lock badge: shown only while a lock is engaged in a move drag, not before the press (before the press Shift means "add to selection"). **Remove (`advanced-selection`):** the same badge with a white minus (8px, 1.5px stroke) on the solid `--accent` ground, so plus and minus differ only in the sign; shown wherever a press with Ctrl would arm the marquee (empty canvas, and inside the sole selected box away from its outline and center handle, where Ctrl bypasses the move as Shift does; the same `classify_press` the press acts on) or, with Alt also held, the lasso, and while a Ctrl marquee or lasso runs. A Ctrl press that would start a move shows the plus, one that would arm a marquee shows the minus, never both. The built-in cursor stays as it is: no `copy` cursor, no custom lock cursor, because engines apply a changed `cursor` only on the next mouse event and the badge must follow the key at once |
| Move entry chip | The "Transform entry chip" surface. Row 1: fields "X" and "Y", 100px wide, 4px apart, fixed `mm` suffix, visible labels. Row 2, 6px below: the mode control, a 28px high two-segment pill "Relative | Absolute" (active segment `--toolbar-icon-active-bg` / `--toolbar-icon-active-fg`, inactive transparent with `--toolbar-icon` text; 12px text; 1.5px `--toolbar-icon` border; focus ring as the Switch row), then, as the last Tab stop, a "Copy" check (a 14px checkbox with the label "Copy", 12px text, 6px below the pill; off on every open, on when Ctrl was held at the second press of the double-click). Placed with its top left 16px right of and 16px below the center glyph (not centered outward from a handle: the center handle has no outward direction), flipped left or up with the readout's flip rule, clamped in the canvas; never covers the center glyph | `edit-interaction-polish`. Opens at the center handle by double-click or by `M`. The center handle stays drawn in its dragging look while the chip is open. No pivot marker (a move has no pivot). Tab order X, Y, mode, Copy; Space or an arrow key on the mode control switches Relative and Absolute; the untouched fields re-render their prefill when the mode switches, edited text stays |
| Polygon/star angle | Shown and typed as the shape's real orientation: the clockwise angle of its first outer vertex (a star's first tip) from straight right, -180° to 180°, one decimal at most | `edit-interaction-polish`. One number everywhere: the create readout, the rotate drag readout, the angle chip prefill and the oriented box. 0° = first vertex points right (the customer: "first vertex pointing right is fine as the zero angle"), so the angle shown is also the direction the maker dragged. Typing 0 always restores that orientation. See "Polygon/star angle" under "Interaction conventions" |
| Limited tag | 12px text inside a bar field, `--toolbar-icon` at 70%, before the unit | A stored value larger than the object allows (a radius above half the shorter side after a shrink): the field shows the effective value and the tag "limited"; tooltip "Stored 20 mm, limited to 15 mm by the size; enlarging brings it back." Also for the curve value later. Not for a ratio's second-decimal rounding |
| Link corners toggle | 28×28px icon button, no label, `rounded-md`; Lucide `Link` / `Unlink`, 16px, 1.5px absolute stroke. **On (linked, default):** `--toolbar-icon-active-bg` fill, `--toolbar-icon-active-fg` glyph, closed chain. **Off:** transparent, `--toolbar-icon` glyph, broken chain. Hover (off) `--editor-accent-hover`; focus-visible 2px `--editor-accent` ring, 1px `--toolbar-bg` offset. `aria-pressed`, name "Link corners". Tooltip 400ms, `side="bottom"`, text follows the state: "Link corners: on. A corner handle sets all four radii. Hold Shift to change one corner." / "Link corners: off. A corner handle changes its own corner. Hold Shift to set all four." plus a muted line "Applies to the corner handles of one selected rectangle. The Radius field always sets all four." | `rectangle-corner-radii` (2026-10-07). Slot: rectangle group, between "Radius" (12px gap) and "Remove rounding" (12px gap). Shown when the selection contains a rectangle, never disabled. Session state (default linked, reset by New and Open like the two Scale switches, never saved); a click closes an open entry chip without writing and changes no radius. Space and Enter toggle; no letter shortcut. State by glyph and fill, not colour alone (on 4.7:1, off 8:1). Shift at a handle press **inverts** it for that drag or entry (read once, frozen): the toggle and Shift differing means one corner; this is the one rule, so no dead modifier. Label omitted on purpose (it belongs to the Radius field, the chain is the established icon, a label would cost about 140px); the knob hint names the state |
| Corner radius readout and chip texts | Existing readout chip at the pointer, hint chip (600ms) and entry chip. Readout: "r 3.5 mm"; at a limit "r 12.0 mm max" (also a linked drag at half the shorter side); a linked drag that finds unequal radii adds " · all corners" ("r 12.3 mm · all corners"). Hint, toggle on: "Corner radius, all four" / "Shift: this corner only" / "Double-click: type a value"; toggle off: "Corner radius, this corner" / "Shift: all four corners" / "Double-click: type a value". A corner whose stored radius exceeds its effective one adds "Limited by the size. Stored 30 mm, shown 20 mm." and, when the next drag would be one-corner, "Editing one corner fixes the other three at their shown size." Entry chip: field "r" 80px with a muted 12px second row, 6px below, "All four corners" or "This corner only" (fixed when it opens); accessible names "Corner radius, all corners" (linked) and "Top-left corner radius", "Top-right ...", "Bottom-right ...", "Bottom-left ..." (unlinked, the rectangle's own frame); a typed value above the limit is applied at the limit and the "max" readout shows for 1.5s | `rectangle-corner-radii`. Shift is not tracked live in the hint (the lines say what Shift does relative to the toggle). Followers: see "Parameter handle". The bar field keeps "Corner radius" and always sets all four |
| Edit hint chip | The hint chip surface, 3 lines, 3s life (or a press, key or leave), `pointer-events: none`, `aria-live="polite"` | Shown at the pointer after a double-click on a primitive's outline, body or center handle, where nothing else happens: "Drag a handle to edit this shape" / "Double-click a handle to type a value" / "Nodes: Object to path, then double-click". Writes nothing, changes neither tool nor selection. Shown on every such double-click; not on a path (it opens the Node tool), a skew handle or empty canvas |
| Bounding-box selection outline | 1px screen-space `--accent`, **dashed** (selected); 1px solid `--accent-hover` (hover) | **`edit-interaction-polish` Part E, criteria 63 to 67 (customer decision 2026-10-06, variant V1).** Selected: nominal 4px on / 3px off, static (no animation), in screen pixels at every zoom. The pattern is laid out from the box's first corner in the box's own, possibly rotated, frame and fitted per edge so it is symmetric and every corner is closed (a dash at both ends of every edge): gap 2 to 4px, dash 4px; on edges strictly between 12 and 16px and between 20 and 22px, which no 4 / 3 pattern fits, the gap is 2px and the dash flexes to about 2.7 to 4px. An edge under 10px is solid. End dashes reach half a line width past the corner so the corner pixel is covered. Hover stays solid so selected (dashed) and hovered (solid) stay distinct; a hover over an already selected object adds no solid line (checked). UX review of PR 2 (2026-10-07): corners closed and rhythm even at 1x and at device pixel ratio 2 (dashes 8 / 6 device px, 2 px thick); on a rotated box the dashes are anti-aliased and paler but still read as a 4 / 3 line; boxes of 8px height or width have a solid short side as specified. **Known look, tell the customer:** a rectangle's box lies on its 1px black stroke, and at 1x that stroke covers two pixel rows at 50%, so the blue dashes alternate with grey stroke and the line reads as one slate-blue line, dashes visible only when zoomed. Ellipses, polygons, stars and paths show the dashes plainly (the box is not on their stroke). **Pixel-aligned:** an axis-aligned selected or hover box is snapped to the device pixel grid and drawn a whole number of device pixels wide (`max(1, round(devicePixelRatio))`), so it never renders as two rows at about 50% (1.9:1 on the canvas; full `--accent` is 3.7:1); a rotated box keeps its true corners and 1px width, anti-aliased. Drawn above the artwork and below the handles: over an object's own outline (a rectangle's box lies on its stroke) the gaps show the artwork, so the line reads blue and black. The marquee stays solid and the lasso keeps 4 / 3. Earlier rule, still true for the colours and the contexts: Drawn around a selected/hovered primitive when its own matching tool is active (`primitive-shapes`; **ended 2026-10-06 by `unified-object-editing`: the shape tools only create; while one is active a selected object keeps this 1px `--accent` box only, with no hover box, no handles and no hit state**) — **and, as of `canvas-navigation-and-selection`, around any selected/hovered object of any type (path or primitive) when the Select tool is active, with no shape handles or path nodes added on top.** Same token, two contexts: a path's nodes appear only after double-click handoff into the Node tool; **a primitive's parameter handles are drawn by the Select tool itself, with its transform handles (`unified-object-editing`), and a double-click on a primitive hands off to nothing.** **Over filled artwork (`0007`, 2026-10-07):** drawn over a white casing one line width wider on each side, under the dashes only (rhythm and pixel snapping unchanged); the hover box is `--hover-box` (`--accent` at 65%, casing at 65%, solid), not 20%, so it shows on a fill. See "Casing over artwork" in "Properties panel: Style section" |
| Group selection box | The look of a single object's box: 1px `--accent`, dashed 4px on / 3px off (V1), laid out from the first corner and fitted per edge, every corner closed, pixel-snapped (it is always axis-aligned), white casing under the dashes | `multi-object-transform` (2026-10-08). Drawn for two or more selected objects around the tight bounds of the drawn outlines (stroke width not included), axis-aligned, recomputed from the committed or previewed geometry in the same frame. Not heavier than a single object's box: 1.5px `--accent` hollow lines are reserved for the blue preview. It is dominant by being outermost and the only box with handles. Degenerate in one axis: one dashed line, drawn once (the two coincident edges are not both drawn). Degenerate in both: no dashes; a 6px solid square outline, 1px `--accent` with casing. During a skew drag it omits its dashes along the fixed edge (the guide replaces them, as for one object). In a rotate drag the preview box is the box at the press turned by the live angle; on release it is replaced in one frame by the axis-aligned box around the result (no animation; a 120ms corner interpolation is the proposed fix if the jump is found jarring). In a Ctrl copy drag it stays on the originals. No simplified preview in `multi-object-transform` (`adrs.md` decision 3). Dormant rule, not built: if a threshold is ever added, a simplified preview draws no per-object outline, only a solid 1.5px `--preview-new` rectangle around the preview group box (the one other use of that outline) and the readout line "Preview simplified" |
| Member box | Each selected object's own oriented box in a multi-selection: 1px `--member-box`, dashed 4 / 3 | `multi-object-transform`. Below the group box, above the blue preview and the artwork. The part of an edge that lies within 1.5 device pixels of a group box edge is not drawn (two 4 / 3 patterns on one edge would fill each other's gaps into a heavy line). Not drawn: when more than 500 objects are selected (a count of the selection, not of what is visible, so pan and zoom never make it flicker); for a member whose box is under 6px on both sides; for a member outside the viewport. No handles, no hit area. May stick out of the group box when the member is a turned path with a stale oriented box. A single selected object has no member box: it draws the full box as before |
| Group handle layout | The glyphs, offsets, caps and tie order of "Transform handle layout", in the axis-aligned group box | `multi-object-transform`. Tiers on `s` (shorter side of the group box on screen): under 24 corner resize and corner rotate; 24 to 47 plus edge resize; 48 and up plus the center handle. No 72px tier (no parameter handles). Skew handles only for a selection of paths, per axis under the 24px lever rule; a selection with a rectangle, ellipse, polygon or star has none (hidden, not inert). Edge resize handles are drawn for every selection at the tiers above (since 2026-10-10 a selection with a polygon, star, or a rectangle or ellipse that is not aligned has them too; the glyph is the same, the consequence is told by the hint chip, the readout and the notice). Degenerate in one axis: the resize handles that would change that axis are not drawn and not hit; the other extent is `s`. Degenerate in both axes: no handle. Cursors: the four stock resize angles, the arc cursor, the two-arrow skew cursor at 0 and 90 degrees, `move` on the center handle, the Select arrow elsewhere; modifiers change none. Center handle: not drawn while a rotate (default pivot) or Shift scale runs, the marker is there. The center handle has a real hit area (min(12px, s/4)) because it is a move grip in a multi-selection; the empty interior of the box does not move the selection (see "Multi-selection press model" under "Interaction conventions"). Inside the box on empty canvas there is no hover and the cursor is the Select arrow |
| Group gestures | Readouts and indicators of a multi-selection drag | `multi-object-transform`. Scale readout: "123.4 mm × 67.8 mm", the preview group box size; a second line, "2 shapes become paths" ("1 shape becomes a path"), from the first frame of an edge drag in which the preview converts a polygon, star or rotated rectangle or ellipse into a path and while it does (the commit's own count, so none at a factor of 1; shown at once, the 600ms rule is for idle hover only; `aria-hidden` like the readout). Rotate readout: "Δ 37.4°" ("Δ 45°" under Ctrl), clockwise positive, real minus sign, one decimal at most, in (−180°, 180°]; the Δ says it is relative to the press. Skew readout as for a path. Move readout, Ctrl copy badge, Shift lock badge and origin axes as "Modifier badge" and "Move axis guide", with the axes through the group box center at the press (where the center handle is drawn, drawn or not). From the release of a gesture to the presented commit the cursor is `wait` and the preview stays on screen (no frame of the old geometry) |
| Group hint chip | The "Transform handle hint chip" surface, up to five lines, wraps at 240px | `multi-object-transform`. Corner resize: "Resize selection" / "Shift: from the center" / "Ctrl: keep proportions" / "Double-click or S: type a size"; edge resize without the Ctrl line. Selection with converting objects (a polygon, star, rectangle or ellipse turned by something other than a multiple of 90 degrees; a circle and an aligned shape never count): corner drags stay proportional, so the corner chip is "Resize selection, proportional" / "Stretch with an edge handle" (only while edge handles are drawn, `s` of 24 or more) / "Shift: from the center" / "Double-click or S: type a size" (no Ctrl line); the edge chip is "Resize selection" / "Stretching turns 2 shapes into paths" ("... turns 1 shape into a path") / "Shift: from the center" / "Double-click or S: type a size". Corner rotate: "Rotate selection" / "Shift: pivot at opposite corner" / "Ctrl: snap" / "Double-click or R: type an angle"; side rotate has "Pivot: opposite side" in place of the Shift line. Skew: "Skew selection" / "Shift: from the center line" / "Ctrl: snap" / "Double-click or K: type an angle" (left and right: "Shift+K"). Center: "Move selection" / "Shift: keep one axis" / "Ctrl: copy" / "Double-click or M: type an offset". UI text says "selection", never "group" (reserved for the group object of `layers-and-grouping`) |
| Group entry chips | The "Transform entry chip" and "Move entry chip" surfaces | `multi-object-transform`. Angle: one 80px field with the visible label "Δ" at the left edge (12px, like "W" and "H"), "°" suffix, prefilled "0", accessible name "Rotate selection by" (a selection has no absolute angle). Size: "W" and "H" prefilled with the group box size, accessible group name "Resize selection"; the fields are independent as for any selection (Ctrl-linked chip: chain glyph, aspect ratio kept); while the typed size is a stretch and the selection holds converting objects, a muted 12px note "Turns 2 shapes into paths." shows in the message line's place (also in the one-field chip of an edge double-click; polite live region and `aria-describedby`, not `aria-invalid`; an error replaces it). Move: the move chip; the "Absolute" segment carries the description "Top-left corner of the selection". Keys M, R, S, K open them as for one object; pivot marker shown while open |
| `PropertiesPanel` width | 280px, fixed; content 244px | Right-docked panel (`stroke-and-fill-styling`'s `StylePanel` is its first section), full height between the menu and the status bar, a sibling of the canvas region, which fills the rest. Collapsible (0px, tab stays). Rows, controls and anchoring: "Properties panel: Style section" |
| Contextual mini-toolbar padding | 6px | Floating per-selection toolbar (`NodeToolbar`'s actions, 2026-10-05), anchored near the current canvas selection rather than docked |
| Status bar zoom field | integer percentage, no decimals | New center segment (`canvas-navigation-and-selection`), between the existing cursor-position (left) and document-size (right) fields `project-file-foundation` already shipped; e.g. "100%", range 2%–8000% |
| Marquee/lasso stroke weight | 1.5px screen-space; the marquee border is laid on whole device pixels and a whole number of them wide, `max(1, round(1.5 * devicePixelRatio))` (2 px at ratio 1, 1.5 px at ratio 2), like the selection box, so it is crisp at 1x; the lasso line keeps 1.5px | Marquee box border and lasso line (`advanced-selection`) — deliberately heavier than the 1px bounding-box selection outline so the drag-feedback shape reads as a distinct, topmost layer over any selected objects' own 1px boxes still visible underneath (e.g. during a Ctrl-add drag) |
| Lasso line dash pattern | 4px on / 3px off, screen-space | Distinguishes the lasso's freehand line from the marquee box's solid border at a glance, despite sharing a color family (`advanced-selection`) |
| Marquee/lasso modifier-state legend | small on-canvas label, 12px, `--toolbar-icon` on `--toolbar-bg` with a 1px border (`--toolbar-icon` at 25%) so it reads as its own surface wherever it lands. Placed 12px up and to the right of the pointer; flips to the left of the pointer at the canvas's right edge, below it at the top edge and when it would meet the Select bar (4px clear), and below the bar when the pointer is over the bar; never reaches into the last 14px of the canvas, so it cannot cover the properties panel's collapse tab when the pointer leaves the canvas | `advanced-selection`'s drag-mode readout, same convention as `primitive-shapes`' drag-to-create numeric readout (on-canvas, not status-bar) |
| Viewport and rulers | The canvas region holds, in a grid: the corner square (24 x 24, top left), the horizontal ruler (24px high, to the right of the corner), the vertical ruler (24px wide, under the corner) and the **viewport** (the rest). The tool rail, the bars' overlay row, chips, the collapse tab and every clamp ("stays fully inside the canvas") are anchored to the viewport, so their numbers (12px inset, `left-[72px]`) do not change. Tooltips, notices and chips may cover the rulers. At 800 x 600 the viewport is about 496 x 546 (to be measured at build); the bars' overlay row gets 412px (356px once the second rail column ships, `--rail-right` 116px) | `document-size-and-rulers`. The rulers are ordinary DOM or Canvas2D chrome, not the WebGL draw list (architect). Not part of the no-layout-shift rule's concern: they are always present, and tool or selection changes never resize them |
| Ruler | 24px thick, ground `--ruler-bg`, 1px `--ruler-edge` line on the canvas-facing side (bottom of the horizontal, right of the vertical ruler, and the corner's right and bottom). `aria-hidden`, not focusable, cursor `default`. A press on a ruler or the corner is swallowed (no capture, nothing reaches the canvas); wheel and pinch over a ruler act on the canvas with the pointer clamped to the nearest viewport point | `document-size-and-rulers`. No hover state, no tooltip, no context menu (guides are out of scope) |
| Ruler ticks | Grow from the canvas-facing edge. Major: 1px `--ruler-tick`, full 24px. Minor: 4 between majors, 1px `--ruler-tick-minor`, 8px. Origin tick (value 0): 2px `--ruler-tick`, full depth. All on whole device pixels. Spacing: criterion 5 (1, 2 or 5 x 10^k in the display unit, 40 to under 100px) | The "origin marker" is the 2px tick; no separate glyph. The document span is not highlighted (out of scope) |
| Ruler labels | 12px (`text-xs`), tabular figures, `--ruler-label`, U+2212 for minus, no unit, no grouping, exact decimals. Horizontal: starts 4px right of the major tick, top at 3px (above the minor ticks, which start 15px from the top). Vertical: rotated 90 degrees counter-clockwise, reads bottom to top, starts 4px below the tick with the end of the text next to it, 12px thick in the 24px strip. A label is drawn only if its whole extent lies in the ruler's free span (not under the corner or beyond the end); if the widest label plus 8px exceeds the major spacing (about 53px for "-999999.8" at 7px per digit, spacing minimum 40px) labels go on every 2nd, then every 5th major tick; ticks stay, numbers are never abbreviated | `document-size-and-rulers` criteria 6, 7 |
| Ruler corner | 24 x 24, `--ruler-bg`, shows the display unit ("mm", "cm", "in") centered in 12px `--panel-muted-fg` (5.0:1), `aria-hidden`, does nothing when pressed | The only place that names the unit while an object is selected |
| Ruler pointer marker | 1 device pixel wide, the full thickness of its ruler, `--ruler-pointer`; above the ticks, below the labels. Horizontal ruler: the pointer's x; vertical: its y. At the position of the status bar's cursor readout; it goes when the pointer leaves the canvas, while the readout keeps its last value (`0001` behaviour). No triangle, no second line | `document-size-and-rulers` criterion 8 |
| Pasteboard and document edge | Draw order, bottom first: pasteboard over the whole viewport (`--pasteboard-bg`), the document rectangle at its size in the stored background (`--canvas-bg` by default; with None or alpha below 255 the canvas checkerboard first, then the colour over it, `0040`), artwork, editor overlays. Both rectangles snapped to whole device pixels. No border, no shadow, no document name, no margin guides. Pen close-target knockout, hover and selection casings read the colour under them | `document-size-and-rulers` criterion 28. Open question: a 1px edge line `--toolbar-icon` at 35% under the artwork, outside the document area |
| Canvas checkerboard | `0040-document-background`. Drawn inside the document rectangle only, under the background colour and the artwork, when the background Paint is None or the colour's alpha is below 255. Cells `--checker-a` `#FFFFFF` and `--checker-b` `#C9C9CE`, squares of **8 CSS px = round(8 x devicePixelRatio) device px**: fixed on screen at every zoom (a zoomed cell would read as part of the drawing), cell edges on whole device pixels, no antialiasing. Anchored at the document's top-left corner (snapped as the document edge), the corner cell is `--checker-a`; partial cells are clipped at the right and bottom edges; never on the pasteboard, never under the rulers. On a resize the pattern re-anchors to the new corner in the same frame, no animation. One draw command whatever the zoom. Contrast: a to pasteboard 1.98:1, b to pasteboard 1.20:1 (half of the edge of a None document is 1.2:1, accepted; the pattern ending carries it; the remedy if a review disagrees is the 1px edge line of the open question above, never a darker `--checker-b`, which the swatches share). On the pattern: black 21:1 (a) and 12.7:1 (b), `--toolbar-icon` 11.3:1 and 6.8:1, `--accent` 4.5:1 and 2.7:1. The swatch (7px cells) and the chip swatch (4px cells) keep their sizes. Means transparent, nothing else (`--mixed-hatch` stays distinct) |
| Default view | On New and Open the document's top-left sits 72px right of and below the viewport's top-left (`--rail-right` + 12: tool rail 12 + 48 + 12 = 72px with one column, 128px with two), so the edge and the 0 ticks are visible | Proposed (`document-size-and-rulers` 11a) |
| Status bar | Left: cursor "x: 12.3  y: 45.6 mm". Center: zoom, integer percent. Right: document size "210.0 × 297.0 mm". The unit is the display unit and is written once at the end of the left and of the right segment; decimals are fixed (not trimmed), spaces are kept (`white-space: pre`) and digits are tabular (`tabular-nums`) so the text does not jitter: mm 1, cm 2, in 3 (all about 0.1 mm). `--statusbar-bg`, 24px, 12px text, display only: not pressable, no tooltip | `project-file-foundation`, `canvas-navigation-and-selection`, `document-size-and-rulers`. The cursor readout had no unit before |
| Boolean toolbox | **A card of its own** in the left tool rail, directly below the tools card, **8px** under it (the divider is gone). The card of the row "Toolbox card": 48px wide, 224px high (4px padding, five 40 x 40px icon-only buttons, `rounded-md`, 4px apart, one column, the glyph at 20px, row "Boolean glyphs"). Always rendered, whatever the tool or selection, no heading or caption. `role="toolbar"`, `aria-orientation="vertical"`, name "Boolean operations". Order Union, Difference, Intersection, Exclusion, Reverse difference. With the tools card the column is 500px: from the 12px inset it ends 512px below the viewport top (800 x 600: 34px clear in the Tauri window without rulers, 40px with rulers, 4px in the browser build; `minHeight` of the Tauri window is 600). **Commands, not tools:** never the active tool, no `aria-pressed`, never `--toolbar-icon-active-bg`; the only pressed look is the transient `:active` ground (`--editor-accent-hover`), kept on the pressed button while the operation is busy. Idle `--toolbar-icon`, hover ground `--editor-accent-hover`, focus-visible 2px `--editor-accent` ring with 1px `--toolbar-bg` offset (the tool buttons take the same hover and ring in the same PR). **Enabled** with two or more objects selected and the Select tool active, any kinds; an open path in the selection does not dim anything. **Dimmed** (`aria-disabled="true"`, 40% opacity on the buttons, the card itself never dims, no hover ground, default cursor, still focusable, tooltip stays open on a press, activation does nothing) with fewer than two objects selected, or with any tool other than Select active (note "Select two or more closed objects.", followed by "Use the Select tool." when the tool is not Select). **Keyboard:** the rail keyboard model (row "Rail keyboard model"): one Tab stop, roving focus; Union first. Mouse activation returns focus to the canvas (`onReturnFocus`), key activation leaves it on the button, after a success and after a refusal. While a kernel call runs: `aria-busy` on the toolbar, other activations ignored, cursor `wait` over the window; the busy state is painted (two frames) before the call, and the call runs on the UI thread (`docs/technical-debt.md`) | `0016-boolean-operations`, customer decisions 2026-10-09 (the Select bar group is gone) and **2026-10-10 (its own toolbox, not a section behind a divider)**. Why a card: the maker should see "these are commands, those are tools" at a glance, and a divider inside one card reads as a separator of equals; two surfaces with a gap say two kinds of thing. The one place a control is dimmed instead of removed ("Hidden, not disabled" holds for the panel and the Select bar): the rail is a fixed block and must not move. Dimming means "not available now" and nothing else. Names (also the accessible names, no shortcut suffix): Union, Difference, Intersection, Exclusion, Reverse difference. No shortcut, no menu entry assigned yet |
| Tool rail architecture | The rail is a set of **toolbox cards** (row "Toolbox card") in **at most two fixed columns**, 8px apart. **Column A:** Tools (6 buttons, 268px), Boolean (5, 224px): 500px with the 8px card gap. **Column B**, drawn only from the release in which its first command exists (that is `0035`: it ships the Path card with Combine and Break apart, row "Path toolbox"; Group is inserted above it by `0023`), left edge at 12 + 48 + 8 = 68px, top level with column A (12px inset): Group (Group, Ungroup, Enter group: 3 buttons, 136px), then Path (Combine, Break apart, Split at crossings, Fracture, Flatten, Offset: 6, 268px): 412px. A toolbox is drawn once its first command exists, never as an empty placeholder; a toolbox keeps its place for good (nothing is reflowed by the window size or by a later release). **Budget:** one column holds 500px = 11 buttons at the 800 x 600 minimum; planned 20, rail full at 22; the 23rd command goes to the Select bar or the Properties panel, a third column or a flyout is a customer decision. **`--rail-right`:** the rail's right edge, 60px with one column, 116px with two. The Select, Node and shape bars start at `--rail-right` + 12 (72px, then 128px), the default view puts the document's top-left at the same offset, notices and tooltips open from it. **Too short** (browser build only; the Tauri window cannot be shorter than 600px): the tools card stays fixed and the cards below it scroll inside their column (`overflow-y: auto`, thin scrollbar, a focused button is scrolled into view); no card is collapsed, hidden or reordered. No collapsing, no tabs, no flyouts | Customer decision 2026-10-10 (Boolean in its own toolbox) and the growth plan of `0023`, `0035` to `0038`. **Rejected, and why:** one column (20 buttons need 880px); collapsible toolboxes (a hidden state that moves buttons under the pointer; the customer asked for Boolean always visible); flyouts (the "no popups" rule of the panel is scoped to the panel, so a flyout is allowed by the letter, but it hides commands behind a second click, covers the canvas where the operands are, and competes with tooltips and notices for the same space); a scrolling rail (hides the dimmed commands the customer wants to see); two-button-wide cards (92px wide, an orphan button in an odd-numbered card, and the same height saving as a column); 32px buttons (a second size in one rail). Group sits above Path because it is used far more often; if Path ships first it takes the top of column B and Group is inserted above it later, a single move announced in that release. Column B is deterministic rather than an automatic wrap: muscle memory depends on a command staying where it was |
| Toolbox card | Surface of the tools card: `--toolbar-bg` (opaque), `rounded-lg` (8px), `--panel-elevation-shadow`, 48px wide, 4px padding above and below. Buttons 40 x 40px, `rounded-md`, 4px apart, **one column**, glyph 20px (`size={20}`), `--toolbar-icon`; 8px between cards. **No heading, caption or divider**: the cards are icon-only, 48px cannot hold a legible label (10px text would be the smallest in the app), and the tooltip names every button; the toolbox gets its accessible name from `aria-label`. A card never dims, collapses or scrolls on its own (the column may, row "Tool rail architecture"). Hover ground `--editor-accent-hover`, focus-visible 2px `--editor-accent` ring with 1px `--toolbar-bg` offset on every rail button, tools included | Customer decision 2026-10-10: "its own surface, border and spacing". There is no 1px border: the rail's elevation is the shadow (`--panel-elevation-shadow`), as for every floating surface here; the card edge and the 8px gap carry the separation, and a border would be the only outlined thing on the canvas. The tools card keeps `--toolbar-icon-active-bg` for the active tool; no command ever shows it |
| Rail keyboard model | Every toolbox is `role="toolbar"`, `aria-orientation="vertical"`, with an accessible name ("Tools", "Boolean operations", "Grouping", "Path operations") and is **one Tab stop** with roving focus: Tab lands on the button that last had focus (the first the first time; in Tools, the active tool); Up, Down, Home and End move the focus, no wrap, and never activate; Space and Enter activate; Left and Right do nothing; dimmed buttons stay in the set; tooltips open on keyboard focus. Tab order is the DOM order: Tools, Boolean, Group, Path, then the canvas and the bars. Tools card: until the customer decides (`0016` Question 2) the six tool buttons keep their own Tab stops; making Tools one roving toolbar takes the whole rail from 9 stops to 4 and is recommended for the release in which column B ships. Tool letters stay with the canvas; no command has a letter yet (the destructive commands get none until a follow-up decides, `0020` Question 10) | The pattern of the Boolean toolbox (`0016` criterion 2), generalised to every toolbox so `0023` and `0035` to `0038` do not each invent one. Letters are ignored while focus is in a toolbox ("focused control" rule) |
| Rail tooltip and notice placement | Tooltips: Radix `Tooltip`, 400ms, `side="right"`, vertically centered on the button, opening **past the rail's right edge**: offset 6px from the button in column A while there is no column B, 6px from `--rail-right` once there is (so the tooltip never covers the other column). Three lines for a command: name (semibold), rule, note (`--panel-muted-fg`), no shortcut text, max width 260px. Tools keep their one- or two-line tooltips. Action notices: left edge 12px right of `--rail-right`, top level with the first button of the card that issued them. No popup, flyout or menu opens from a rail button | The hover ground on the pressed button ties a tooltip that is a column away back to its button. The notice anchor of the row "Action notice" follows |
| Command glyphs | The glyphs of the rail's commands: 16 x 16 viewBox drawn at 20px (scale 1.25), 1.5px stroke kept absolute (`vector-effect: non-scaling-stroke`), `currentColor`, round joins, no colour, no gradient, at most two shapes and one modifier mark, no feature under 2px, solid = result or subject, outline = removed or context, as the Boolean glyphs. Briefs (art finalised at build and approved on a glyph sheet at 1x, 1.25x and 2x, greyscale): **Group** a rounded frame around two small solid squares; **Ungroup** the two squares moved apart, the frame reduced to corner ticks; **Enter group** the framed squares with an arrow pointing into the frame; **Combine** one solid rounded square (11 x 11) with a concentric square hole (5 x 5, evenodd); **Break apart** a smaller solid square with a hole (8 x 8, hole 3 x 3) at the upper left and, 1.6 gap away, a separate solid 4 x 4 square at the lower right (a plate keeps its hole; only the separate piece leaves; the outline and its hole as two pieces would show the hole released, the opposite of the rule); **Split at crossings** an X made of four stubs with a gap at the crossing; **Fracture** two overlapping squares cut into three solid pieces by hairline gaps; **Flatten** two overlapping squares, the lower one trimmed to the L-shape that shows, both solid, no gap line; **Offset** two concentric rounded outlines, both hollow. Pairs that must read apart at 1x: Union / Combine (overlap against nesting), Combine / Break apart (one ring against a ring plus a separate piece), Combine / Offset (solid with hole against two hollows), Fracture / Flatten (three pieces with gaps against two touching pieces), Group / Ungroup | Not final art: a list of what each glyph shows so the sheet can be judged. Acceptance at build: someone who has not seen the names tells the nine apart on the sheet at 1x. If two cannot be told apart at 20px, the glyph changes; the button never grows and no text label is added |
| Command with parameters | A rail command that needs a value (the first is Offset, `0038`) does not open a popup, dialog or chip from the rail. Its button opens a **section at the top of the Properties panel** (expanding the panel if collapsed, focus in the first field), above the Style area, with the panel's own components: the value field of the panel for the distance (drag and type), segmented groups for Join and Cap, the buttons Apply and Cancel at the end of the section; Enter in a field applies, Escape cancels and is the first step of the cascade, a change of tool or selection closes it, nothing is written until Apply. The result is previewed per frame as the row "Live preview outline" (blue new, black old). While the section is open the command's rail button shows the open ground (`--editor-accent-hover`) and `aria-expanded="true"`, never the solid blue of the active tool; its tooltip note reads "Opens in the Properties panel.". Pressing the button again closes the section | Why the panel, not a chip at the selection or a Select bar group: the panel is where everything configurable lives (2026-10-05 decision), it is always in view, it holds the existing value field, segmented groups and validation rules, it does not cover the preview, and Join and Cap persist for the session there. The eye travel from the left rail to the right panel is the cost; the section opening and the focus move are the cue. The one command with a persistent "open" look; it is a mode of the panel, not a tool |
| Boolean glyphs | 16 x 16 viewBox, two 9 x 9 squares on the center line, corner radius 1, 1.5px stroke, `currentColor`: the first-drawn one (the **lower object**) at (1.75, 1.75), the second (the upper) at (5.25, 5.25), overlap 5.5 x 5.5. **Drawn at 20px on the rail** (scale 1.25, stroke kept at 1.5px absolute with `vector-effect: non-scaling-stroke`), the size of the tool icons. Solid = the result, outline only (1.5px, no fill) = what disappears. Union: both solid, one silhouette. Difference: lower solid minus the overlap, upper outline. Intersection: both outlines, overlap solid. Exclusion: both solid, overlap empty. Reverse difference: upper solid minus the overlap, lower outline. Difference and Reverse difference are mirror images by design; the lower object is always the upper left | The Inkscape convention. At 20px the overlap interior is 5.4px (4px at 16px, the limit of legibility, and smaller than every neighbouring tool icon); Exclusion and Intersection must stay apart at 1x and 2x, check at build. Custom SVGs (Lucide has none), 1.5px absolute stroke like every icon |
| Boolean tooltip | The rail's tooltip surface (Radix `Tooltip`, 400ms, `side="right"`, opening past the rail's right edge, row "Rail tooltip and notice placement", `bg-popover`, `text-xs`, ring), three lines, max-width 260px: name (semibold), rule, note (`--panel-muted-fg`). Name and rule never wrap, the note may wrap to a second line. No shortcut text. Rules: Union "Everything covered by any selected object."; Difference "The lowest selected object minus the others."; Intersection "Only what every selected object covers."; Exclusion "Areas covered an odd number of times."; Reverse difference "The top selected object minus the others." Note, first match wins: fewer than two selected, or a tool other than Select active, "Select two or more closed objects." (with another tool followed by "Use the Select tool."); open path "Needs closed paths: 2 of 3 selected are open."; preview computed and empty "Would be empty. Nothing would change."; too large to preview "Too large to preview. Replaces the selection."; else "Replaces the selection." Also shown on keyboard focus | "Lowest" and "top" are stacking order. Radix closes a tooltip on a press; a press on a dimmed button leaves it open. |
| Boolean preview | The result's outlines (every outline of a compound result) as a hollow 1.5px `--preview-new` line with the white casing, over the operands, which stay as committed ("black old"); no fill; constant screen width; below the selection box and handles. Starts 100ms after the pointer rests on an enabled button of the Boolean toolbox, or at once when a button gains focus from the keyboard (Tab into the section, an arrow key); ends in the frame the pointer leaves, focus moves, the button is activated or Escape is pressed. The computation runs after the frame and a stale result is dropped, never drawn. No preview for a dimmed button, for a selection with an open path, over 2,000 operand nodes, or when the operation would be refused | Proposals P1 to P3 of `0016`. The same blue as the move or resize preview: it means "this is what a release would commit". The tooltip (400ms, right of the rail) may cover the left part of the canvas after the preview has appeared; accepted, it is text only and its note line carries "Would be empty" |
| Action notice | DOM, text only, `pointer-events: none`, no controls, never takes focus, instant in and out. Anchored to the control that caused it: **for a rail toolbox, to the right of the rail, top edge level with the top of the card's first button (Union; Combine for the Path card), left edge 12px right of the rail's right edge, `--rail-right` + 12 (72px from the viewport's left edge with one rail column, 128px with two, like the bars); an absolutely positioned child of the toolbox wrapper positioned from `--rail-right`, not from the card**; for a panel button or a Node bar button, 4px below it, right-aligned (the validation chip rule); clamped to the viewport or panel; max-width 360px (244px in the panel); 12px; z above the overlay row, below tooltips. **Success:** `--toolbar-bg` ground, `--toolbar-icon` text, 8px padding and radius, `--panel-elevation-shadow`, `role="status"`, 3s (5s when the sentence is longer than 70 characters) or until the next action. **Refusal:** the validation chip's look (`--popover` ground, 1px `--field-invalid` ring, `--field-invalid` text 6.5:1, 12px alert glyph), `role="alert"`, 8s or until the next action. "Next action" is a press anywhere, a key, a selection change or a tool change (not a pointer move or hover). The live region is in the tree from the start so the announcement is the text change. Busy over 150ms: "Union: working..." (`role="status"`), only for a call that runs off the UI thread (none does yet, see `docs/technical-debt.md`). Texts (the Path toolbox's are in the row "Path tooltips and notices"): "Union: 3 objects became 1 path." / "... 1 compound path."; refusals end with "Nothing was changed."; out of range: "Union works only within 10 km of the point 0, 0. 1 of 3 selected objects reaches further. Nothing was changed." (plural "2 of 3 selected objects reach further."); "Already fits the content." (3s, panel) | `0016`, `document-size-and-rulers`. Why beside the rail and not the status bar or the pointer: the eye is on the rail; the tooltip closes on the press and the notice takes its place; it does not depend on the active tool (the Select bar exists only with the Select tool); the status bar is display only, holds the live cursor readout and is far from the rail; a pointer chip would cover the neighbouring buttons. Why the refusal lasts longer: the sentence is 80 characters |
| Refusal outline | While a refusal notice about open paths, objects without area or objects out of range is on screen, each offending operand is redrawn as a hollow 2px `--field-invalid` outline with the white casing over the canvas. Not stored, not selected, not hit-testable. 5.3:1 on `--canvas-bg`, 3.3:1 on `--pasteboard-bg` | Also used by Combine (open paths, shapes that touch, a shape that crosses itself); at most 200 offenders are drawn, the count in the sentence stays exact. Keeps the promise "say which object is open" without changing the selection; an out-of-range operand is outlined at its own coordinates, which are off screen, so its notice gives only the count (`docs/technical-debt.md`) |
| Compound path in the UI | Subject line "Compound path" / "3 compound paths" (all compound) / "N paths" (mixed with paths) / "N objects" (mixed with primitives). Style sections as for a path; the Markers block is removed while one is selected (default, open). The Select bar offers no kind group and no "Object to path". Node tool with a compound path as the only selected object: the Node bar slot holds a text-only pill (bar surface, 14px `--toolbar-icon`, no controls, `role="status"`) "Nodes of compound paths cannot be edited yet."; no node, handle or segment overlay. A double-click on a compound path in the Select tool does not switch tools; the hint chip (3s) shows the same sentence. Hover follows what a press would pick (over a hole, the object behind) | `0016` criteria 31 to 38 |
| Path toolbox | **A card of its own in column B**, the first card there until `0023` puts Group above it. The card of the row "Toolbox card": 48px wide, **92px high** (4px padding, two 40 x 40px icon-only buttons, 4px apart, glyphs 20px, row "Command glyphs"), left edge 68px, top at the 12px inset, ending 104px below the viewport top. `role="toolbar"`, `aria-orientation="vertical"`, name "Path operations", one Tab stop (Tools, Boolean, Path, then the canvas), Combine first, Break apart second. The rules of the row "Boolean toolbox" apply unchanged: commands, never the active tool, no `aria-pressed`, no active ground; dimmed means `aria-disabled="true"` at 40% opacity, still focusable, no hover ground, activation does nothing; a mouse press returns focus to the canvas, a key press leaves it on the button; `aria-busy` and cursor `wait` while a call runs. **`--rail-right` is 116px from this release** (60px before). By 56px it moves: the bars' overlay row (`left` 128px), the default view's document offset (128px), the left edge of every rail notice (128px), the tooltips of Tools and Boolean (they open 6px right of the 116px edge, row "Rail tooltip and notice placement"). At 800 x 600 (panel open, rulers on, viewport about 496 x 546) the bars' row is **356px** (was 412px): the Node bar (about 300px) and the polygon/star bar (about 310px) stay on one row; the Select bar wraps by whole groups and is measured at build (if its settings group alone is wider than 356px the two switches wrap inside the group, the first row keeps its y). A notice of 360px at 128px needs 488px of 496px and clamps if it does not fit. Column A is unchanged. **Enabled / dimmed:** Combine dimmed with fewer than two objects selected or a tool other than Select; Break apart dimmed with no compound path in the selection or a tool other than Select. An open path or touching shapes do not dim Combine (pressing answers, with the offenders outlined) | `0035-combine-and-break-apart`, customer decision 2026-10-10 (two columns). Why column B and not a third card in column A: column A uses 500 of the 546px and would not fit a third card at 800 x 600; column B also puts the two commands to the right of Select and Pen, where the hand is after selecting. Why dim Break apart without a compound path and not refuse it: the interface knows without running anything, and a red alert for it would be noise; Combine with touching shapes needs the kernel, so it is refused on press. The column costs 56px of the 496px viewport at the minimum window; the Path card covers only 48 x 92px of it |
| Path tooltips and notices | Tooltips of the row "Boolean tooltip" (name, rule, note; rules under 45 characters so they never wrap). **Combine** "One object. Inner shapes become holes." Note: fewer than two selected "Select two or more closed shapes." (another tool than Select adds "Use the Select tool."); an open path "Needs closed paths: 1 of 3 selected is open."; else "Replaces the selection.". **Break apart** "Splits a compound path into its pieces." Note: no compound path selected "Select a compound path." (another tool adds "Use the Select tool."); else "Holes stay with their piece. Replaces the selection." (may wrap). **Success notices** (Action notice, level with Combine): "Combine: 4 objects became 1 compound path with 3 holes." (holes omitted at zero; when the operand styles differed, "It uses the style of the lowest object." at the end of the notice); "Break apart: 1 compound path became 3 objects." (with a hole in a piece: "Holes stayed with their piece."; with a one-piece compound path left alone: "1 compound path is one piece and was left as it is."). **Refusals**, alert, 8s, the offenders in the Refusal outline: "Combine needs closed paths. 1 of 3 selected objects is open. Nothing was changed."; "Combine needs shapes that do not touch. 2 of 3 selected objects touch each other. Use Union to merge overlapping shapes. Nothing was changed."; "Combine needs shapes that do not cross themselves. 1 of 3 selected objects does. Nothing was changed."; no area, out of range, group: the sentences of `0016` with the operation name "Combine" and "Combine does not work on groups. Ungroup first."; "Nothing to break apart. The compound path is one piece with its holes. Nothing was changed." (plural "The 2 selected compound paths are one piece each, with their holes."; no outline) | `0035`. "Compound path" stays in the notices because it is the name of the object in the panel's subject line; "with 3 holes" explains it. Names rejected: "Merge into one object" (merge already means Union, Join and connect), "Separate pieces" (a second name for what the glyph shows) |
| Pen target cue | While the Pen is over a target (an end node of an open path with the Pen idle or drawing, or the close target of the path in progress): the node drawn as its idle glyph (14px, white fill, `--node-stroke` outline, the shape of its kind) inside the 18px `--accent-hover` ring, both under the pointer within the 16px node hit radius; while over a join target or the close target the rubber band ends on the node's center. The glyph supplies the contrast the ring lacks (`--accent` at 20% measures about 1.2:1 on `--canvas-bg`). With Shift held over a continue or join target the ring and glyph are not drawn and the cursor is the plain nib (the cue shows what the press does). **Close preview:** over the close target the rubber band is replaced by the closing segment as it will be committed (1px dashed `--accent`), with the closing node's handles in their idle look (1px `--accent` line, 12px white circle with `--accent` ring, none for a handle of length zero) and the closing node drawn as the hollow glyph of its resolved kind (square Corner, diamond Symmetric, triangle Asymmetric); it follows Shift in the same frame. Draw order: artwork, rubber band or closing segment, node glyphs, ring, chip | `0034-pen-path-extension`. Sharp and Smooth must differ on screen before the click: Sharp shows no handle on the closing side and a square, Smooth two lined-up handles and a triangle or diamond |
| Pen hint chip | The "Transform handle hint chip" surface, two lines, the action in semibold and the Shift line below it, a third muted line only on a join. Placed once, 12px up and right of the pointer when it appears (readout placement, flips at the edges), and stays while the pointer stays on the same target; it does not follow the pointer. **Delay:** continue and join 600ms of rest, or at once when Shift changes over the target; close at once. Gone on leaving, on the press and on any key but Shift. `aria-hidden`, `pointer-events: none`. Texts, without and with Shift: continue "Continue path" / "Shift: start a new path", then "Start a new path" / "Release Shift: continue path"; join "Join with path" / "Shift: place a node", then "Place a node" / "Release Shift: join with path"; third line when continuing a path whose style differs from the one joined: "The result keeps the style of the path you continue."; close, default sharp (Corner node) "Close path with a sharp corner" / "Shift: smooth curve", then "Close path with a smooth curve" / "Release Shift: sharp corner"; close, default smooth (Symmetric or Asymmetric node) the same two pairs swapped | `0034`. 600ms keeps a Pen moving through a drawing with many open ends from flashing a chip at every one; close is immediate because it names the one-way commit. On WebKitGTK the Shift key-up is not delivered (`0014` plan.md): the cue keeps the Shift state until the next pointer event, the press reads the modifier from its own event and redraws the cue in the press frame |
| Pen cursors | Four, 24px, white halo under black like the rotate and lasso cursors, hotspot at the nib tip, `crosshair` where custom images are ignored. **Nib** (new path, as built). **Close**: the nib with a small hollow circle at the lower right (as built). **Continue**: the nib with a short stub ending in a solid 4px dot at the lower right. **Join**: the nib with two solid 3px dots 4px apart joined by a 2px bar. No feature under 2px; the four are told apart at 1x on a glyph sheet before the PR is accepted. The cursor always matches the cue and the press; Shift over a continue or join target gives the nib, over the close target the close cursor stays; unchanged during a press | `0034`. Select and Node cursors do not change |
| Close path group | The last group of the Node bar, after a 1px 20px 25% divider and 12px gap: the label "Close path" (14px `--toolbar-icon`, not a control) and two text buttons **Sharp** and **Smooth** (28px high, 1px `--toolbar-icon` at 60% outline, `rounded-[5px]`, 14px label, 12px side padding, 4px apart, the look of "Fit to content"; hover `--editor-accent-hover`, focus-visible 2px `--editor-accent` ring with 1px offset); about 208px. `role="group"` name "Close path"; button names "Close path, sharp corner" and "Close path, smooth curve". Commands: no pressed state, nothing remembered. Greyed and inert when the editing set holds no open ordinary path with 3 or more nodes (the Node bar's rule, done as Join and Split are done); a greyed button still shows its tooltip on hover, so the reason can be read. Tooltips `side="bottom"`, 260px: "Close path with a sharp corner" / "Joins the last node to the first. The first node becomes a corner." / note, and "Close path with a smooth curve" / "Joins the last node to the first. The first node becomes smooth." / note. Note by state: none applicable "Select an open path with 3 or more nodes."; "Closes 2 open paths."; with a skip "Closes 2 of 3 open paths. 1 has fewer than 3 nodes.". Notice (Action notice, below the pressed button, right-aligned): "Closed 3 paths." / "Closed 2 paths. 1 path has fewer than 3 nodes and was not closed." (5s). With the group the Node bar is about 530px; at 800 x 600 with the panel open (356px row) the group wraps to a second row of its own and the bar is 72px high while the Node tool is active; from a window about 1000px wide, or with the panel collapsed, it is one row | `0034`. In the Node bar and not the rail (22 of 22 buttons for an open-path-only command, and words do not fit icon-only cards), not the Select bar (already three rows), not the panel (a section that comes and goes shifts the Style rows). The Sharp / Smooth pair is the node-kind choice (the 3-way kind control sits in the same bar); a hidden group would move the centered bar's buttons, so it wraps and stays |

## Interaction conventions (apply to every later tool, not just this one)

- **All canvas editing-UI** — node/handle glyphs, handle lines, selection and
  hover highlights, and anything like them a later tool adds (primitive
  resize handles, alignment guides) — is drawn in the WebGL draw list
  (`curvyo-render-core`, ADR 0001 §4), sized in constant screen pixels
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
- **Toolboxes** (customer decision 2026-10-10; rows "Tool rail architecture",
  "Toolbox card", "Rail keyboard model", "Rail tooltip and notice placement",
  "Boolean toolbox"). The left rail is no longer one card: tools are one card,
  and every family of commands on the selection is a **toolbox**, a card of its
  own below or beside it, with its own surface, an 8px gap, no heading, one
  Tab stop. Commands run and return: they never become the active tool and
  never show the solid blue of the active tool. Toolboxes are fixed in place
  (Tools and Boolean in column A; Group and Path in a second column B once
  their first command ships), always rendered once they exist, dimmed and not
  removed when a command does not apply (the one place where "Hidden, not
  disabled" is reversed, because a fixed block must not move). The rail is
  capped at two columns and 22 buttons at 800 x 600; a command that would be
  the 23rd belongs in the Select bar or the Properties panel. A command with a
  value opens a section of the Properties panel (row "Command with
  parameters"); there are no flyouts or popups from the rail. The panel's "no
  popups" rule is scoped to the panel; keeping the rail free of them is a
  separate choice made here, to keep tooltips and notices the only transient
  layers beside the rail.
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
  (superseded 2026-10-10: a strip of three tabs, one section each, "Properties
  panel: tabs"). When nothing
  relevant is selected the Style area is empty (superseded 2026-10-08: it no
  longer shows disabled controls; since 2026-10-09 the Document section takes
  its place when nothing is selected, and the two never show together), so its width never changes as selection or
  active tool changes, keeping with the no-layout-shift rule. `stroke-and-fill-styling`'s stroke/fill controls are
  its first and, since 2026-10-07, only section (no "Shape tool options"
  section; see the correction in the 2026-10-05 section). It sits beside the
  canvas region, not over it: the tool rail and the bars' overlay row are
  anchored to the canvas region, so a bar never runs under the panel. This is
  ordinary DOM app chrome, not canvas editing UI — it does not go through the
  WebGL draw list rule above. Reference: "Properties panel: Style section".
- **Segmented icon control (`ToggleGroup`)**: the pattern for any small set
  of mutually-exclusive choices (a Paint switch, Join, Cap, the dash presets, the
  marker choices) — started as a two-state toggle (`primitive-shapes`'
  Polygon/Star mode), generalized to n states. One Tab stop per group; arrow
  keys move the selection within it (`role="radiogroup"`); each item has its
  own `aria-label`; items show an icon or, where the choice has no icon that
  reads ("Spaced", "At nodes"), text. In the properties panel this is the
  only way to pick from a set: a `Select` dropdown is a popup and is not used
  there (see the rule below). Also used for the node-tool's kind control
  (`path-merge-split-and-node-types`): Make corner / Make symmetric / Make
  asymmetric, icons being the node glyphs themselves (square/diamond/triangle).
- **No popups in the properties panel** (customer, final, 2026-10-08,
  `style-panel-rework`). No control in the panel opens a popover, dropdown list,
  menu, dialog or any other layer over the canvas or over other panel content.
  Everything the maker can set is a control in the panel's own column. Text-only
  tooltips and the field validation message are the only transient layers; they
  contain no controls and never take focus. It holds for every later section of
  the panel, not only Style. Consequences: choices are segmented groups, text
  lines and value fields, never a `Select`; a colour picker, a palette or a
  history is inline (tabs in the panel, not a popover); the eyedropper's colour
  chip over the canvas is a readout, like the transform readout, with no
  control in it. Reference: "Properties panel: Style section".
- **Value field** (`style-panel-rework`): the one component for a number the
  panel edits by dragging: GIMP's tool-option field. Label inside at the left,
  value and unit at the right, a bar filled to the value (a slightly
  logarithmic scale for Width and Opacity), a small reset icon. Drag inside the
  field, even outside its bounds, or click and type; Shift is ten times coarser,
  Ctrl (Cmd) ten times finer; no +/- buttons, no steppers, no text selection by
  dragging. A spinbutton for the keyboard; preview per frame and one commit on
  release. The bars' number fields (Radius, Points, Ratio, entry chips) keep
  their own look until a story moves them (open question 5 of the spec). Rows
  and numbers: "Value field".
- **Colour block** (`style-panel-rework`; replaces `ColorAlphaPicker`): per
  colour a display-only swatch, an eyedropper button, an 8-digit RGBA hex field
  and, below, an inline saturation/value area and hue slider. Opacity is its
  own value field. One implementation for stroke and fill; do not add a second
  colour control for a future feature. A later palette, history or colour model
  goes into tabs of this block (`color-management`), never into a popover.
- **Inline confirm** (2026-10-10, `0020`, `0045`): how a destructive or
  data-removing action asks without a popup. The control that was pressed is
  replaced in place by a block: one sentence of consequence (12 or 14 px, up to
  four lines, plain words, ends with what cannot be undone) and two buttons,
  **Cancel** on the left and the action on the right, equal width, 8 px apart.
  The action button uses the `--destructive` look (1 px `--destructive` border
  and label on white; focus or press fills it `--destructive` with a white
  label). Keyboard focus goes to **Cancel**; the action button ignores presses
  for the first 400 ms (a double-click on the old button cannot confirm).
  Escape, Cancel, a change of tab, of selection or a new step cancel without
  writing. The block is a `role="group"` named by its sentence, not a dialog.
  It grows the content, never floats; in a bottom-anchored footer it grows
  upward. Used by: Wipe history (document and object), Delete format, Delete
  group. A reversible action (anything Ctrl+Z takes back) never asks.
- **List row** (2026-10-10, `0020`, `0045`): one row component for the History
  list and the formats list. Fixed height per row type (History 44 px, 32 px for
  the Now row, 24 px for a branch header; formats 28 px) so a list can be
  virtualised. States: hover `--editor-accent-hover`; **selected** (the row
  whose state is shown or applied; `aria-current` or `aria-selected`)
  `--value-fill` ground plus a 3 px `--accent` bar at the left edge, which
  hover does not override; focus 2 px `--editor-accent` ring inside the row;
  muted (undone, off) ink `--panel-muted-fg` plus a word, never opacity alone.
  A row is a button; extra actions are sibling buttons in the same row, reached
  by Left and Right while the list stays one Tab stop with roving focus (Up,
  Down, Home, End, Page Up, Page Down between rows).
- **Canvas notice slot** (2026-10-10, `0020`): the one place for messages that
  a key produces and for persistent canvas status. Centred in the free part of
  the viewport (right of the rail), 12 px above its bottom edge, stacked: the
  persistent line (preview readout or banner) at the bottom, the transient
  notice 8 px above it. `--toolbar-bg`, 12 px text, 8 px padding and radius,
  `--panel-elevation-shadow`, max width 480 px, z above the overlay row below
  tooltips. A notice has no controls and takes no focus, `role="status"`, 3 s
  (5 s above 70 characters, 2 s for a hint), replaced by the next one; a refusal
  uses the validation chip look and `role="alert"`, 8 s. The persistent
  banner of a Document preview may hold one text button (**Back to live**) and is
  the only slot content with a control. A notice caused by a panel button is not
  put here: it stays 4 px under the button ("Action notice").
- **A preview is never a commit** (2026-10-10, `0042`): looking at an earlier
  version changes nothing and must not look like a change. Object preview: the
  version is the hollow 1.5 px `--preview-new` outline with its casing (blue
  new, black old, as in a drag) plus a 40 % copy of its fill and stroke
  (`--preview-ghost`); the current object keeps its look. Document preview: a 2 px
  inset `--preview-new` frame, the dimmed inert rail, the banner text "Editing
  is paused", the plain arrow cursor; objects drawn plainly. Both end in the
  frame of the event that ends them; neither writes, replicates or is saved.
- **Mixed-state display on multi-select**: when selected objects differ on
  a property, show a type-appropriate placeholder rather than the
  first-selected object's value — muted "Mixed" text in place of the value (and
  no fill bar in a value field) for numeric fields, a 45 degree **hatch** colour swatch
  (changed 2026-10-07 from a checkerboard: a transparent colour is drawn as a
  checkerboard, and a mixed swatch must not look like one), no option
  highlighted for a segmented control. Established in
  `stroke-and-fill-styling` as the house convention for every later
  multi-select-editing panel. **Extended by `canvas-navigation-and-
  selection`'s Select tool to selection *indicators*, not just property
  controls:** a heterogeneous multi-select (e.g. a path and a rectangle
  together) shows each object's own real selection box simultaneously,
  never a merged box in its place or an "N objects selected" text summary —
  same underlying rule (show the true, possibly-mixed state rather than
  collapsing it), applied to the one UI where it costs nothing extra, since
  every selected object already draws the same plain Select-tool box
  regardless of its type (see "Bounding-box selection outline" above).
  **Amended 2026-10-08 (`multi-object-transform`):** the per-object boxes
  stay (lighter, row "Member box") and a group box with handles is drawn
  *in addition* around them (row "Group selection box"); it is not a
  replacement and not a merged oriented box. The count stays in the Properties
  panel's subject line, not on the canvas or in the Select bar.
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
  `canvas-navigation-and-selection` criterion 20). All eight rotate handles
  (`object-transform-refinements`) use that same rotate cursor. The skew
  handles show a custom cursor of two opposed parallel arrows, rotated live
  like the resize cursor (box rotation for the top/bottom handles, plus 90°
  for left/right; hint `skew:<deg>`; fallback `ew-resize`/`ns-resize`). The
  center move handle shows the built-in `move` cursor. Parameter handles
  (`unified-object-editing`) show the built-in `pointer` cursor, hovering and
  dragging: a knob is neither a resize nor a move, so it borrows neither
  cursor. A creation tool (Rectangle, Ellipse, Polygon/Star) shows its
  crosshair everywhere, also over existing objects and a selected object's box.
  Modifiers (Shift, Ctrl) never change a handle cursor.
- **Shift-revealed handles** (`object-transform-refinements`): a handle kind
  that exists only while a modifier is held (the four side rotate handles)
  appears and disappears in the same frame as the key, without a fade, in the
  same glyph and on the outermost ring of its side, so revealing it moves
  nothing. The reveal state is cleared when the canvas or window loses focus.
  It is not triggered by pressing the key during a drag. The pivot marker
  previews the pivot a handle would use while the modifier is held and the
  pointer rests on that handle.
- **Numeric entry chip** (`object-transform-refinements`): the pattern for
  typing an exact value into an on-canvas manipulation. A double-click on the
  handle opens a small DOM chip next to it (not next to the readout, not in
  a dialog), upright whatever the object's rotation, the first field focused
  with its text selected. Enter commits in one undo step and returns focus to
  the canvas; Escape, a press elsewhere (which is not swallowed), a tool
  switch, a selection change or window blur cancels and writes nothing; Enter
  on text the user did not edit writes nothing. A refused value keeps the chip
  open, re-selects the field text, marks the field with `--field-invalid` and
  a message line (never color alone) and sets `aria-invalid`; the error
  clears on the next keystroke. Decimal point or comma accepted, `inputmode=
  "decimal"`, type text. Key events inside the chip do not reach canvas
  shortcuts or the app's undo. No stepping with arrow keys (nudges are out
  of scope). Chip content and sizes: the "Transform entry chip" row.
- **Oriented selection box is intentional** (customer decision 2026-10-06,
  item 9 of `object-transform-refinements`): the selection box of a rotated
  object rotates with it and is not re-squared to the screen axes, unlike
  Inkscape's axis-parallel box. The customer: it lets the maker see and
  restore the original orientation, and it reads as intuitive. Every handle
  position, hit region, cursor angle and numeric-entry meaning (W and H are
  along the object's own axes) is defined on the oriented box. Do not "fix"
  it into an axis-aligned box in a later story; changing it needs a new
  customer decision. **A multi-selection is the exception by construction,
  not by reversal** (`multi-object-transform`): it has no register to hold an
  orientation, so its group box is axis-aligned, turns with a rotate drag
  only while the drag runs and is refit on release. Customer question 1 of
  that spec decides whether it ever keeps a turn. A skew does not tilt the box either: it stays the tight
  oriented rectangle around the skewed geometry in the object's own frame.
- **Blue new, black old** (`unified-object-editing`, customer request
  2026-10-06): during any drag in the Select tool the object stays on screen
  exactly as committed (its own stroke, colour and fill: "black old") and the
  geometry a release would commit is drawn over it as a hollow 1.5px outline in
  `--preview-new` (`--accent`). The maker compares before and after without
  undo. One mechanism for every edit (move, resize, rotate, skew, radius, inner
  radius, and the Points and Ratio sliders of the bar); the Node tool's own node
  preview is not part of it yet. The preview is never stored and never exported.
  No fill preview. Typed values do not preview (they commit on Enter and show
  then). The same blue is the selection accent: it means "this is what you are
  editing", not a second meaning, and nothing else may draw 1.5px `--accent`
  hollow outlines.
- **Parameter handles** (`unified-object-editing`): the one pattern for a
  primitive's own parameters (corner radius, inner radius; later arc and
  curve): a knob glyph (see "Parameter handle"), drawn by the Select tool
  together with the transform handles, from a box side of 72px; drag changes
  the value with the handle under the pointer, double-click opens the numeric
  entry chip ("r" or "ratio", accessible names "Corner radius" and "Inner
  ratio"; a polygon or star's outer-radius chip is "Outer radius"), the hint
  chip names both. A rectangle's four radius knobs are linked or not by the "Link corners" toggle, inverted by Shift at the press (rows "Link corners toggle" and "Parameter handle layout" for the diagonal-pair rule). No tool switch is involved anywhere. Below 72px the
  parameters stay editable through the bar fields and by zooming. A new
  parameter kind adds a knob position and a readout, not a new glyph.
- **Select bar** (`unified-object-editing`; row "Select bar layout" for the
  numbers). Three zones in this order: settings (the two switches), the groups
  of the kinds in the selection, "Object to path". (The boolean operations are
  not in the bar: they are the Boolean toolbox of the tool rail, row "Boolean
  tool section", customer decision 2026-10-09.) A control shows when the
  selection contains at least one object of its kind and then acts on exactly
  those objects (others are left alone, the tooltip says so); it is enabled
  when it would change something ("Remove rounding" is disabled when every
  selected rectangle is already sharp). A bar never carries disabled groups for
  unrelated kinds. Mixed values show an empty field with "Mixed"; a typed value
  applies to all (also one rectangle with unequal corners: it sets all four). A stored value the field cannot show shows the effective
  value and the "limited" tag. "Object to path" is last and has no shortcut
  (the browser build's inspector owns Shift+Ctrl+C).
  Overflow wraps by whole groups; the settings stay on the first row.
- **Multi-selection press model** (`multi-object-transform`, 2026-10-08;
  customer decision, question 3 (b)). A group box is the bounds
  of many objects and is mostly other space, so unlike a single object's box
  its empty interior does not move the selection. Order of a press, after Alt
  (lasso): a drawn group handle; then, by the usual topmost hit (outline within
  8px or filled interior), a selected object moves the selection, an unselected
  object is selected and replaces it; on nothing, a marquee or, by a click, a
  cleared selection; Shift and Ctrl keep the meanings they have today. The
  center handle (from a group box side of 48px) is a real move grip with its
  own hit region. This leaves an unselected object inside the group box
  selectable by a click, which the alternative (empty interior moves) does not,
  and keeps a click on empty canvas as the way to deselect. Cursor: the Select
  arrow inside the box, `move` over the center handle. UI text says "selection",
  never "group".
- **Keyboard concept** (`edit-interaction-polish`, 2026-10-06; the table is
  "Keyboard shortcuts established so far"). One key has one meaning in one
  visible state. The only state that changes a letter's meaning is "Select tool
  active and at least one object selected" (rail, selection box and handles
  show it). There, **M, R, S and K act on the selection** and open the typed
  entry chips (R and S built in PR 1, M and K in PR 3); in every other state R
  and S are tool letters and M and K change no tool and show a hint ("Select an
  object first"). `B`, `N`, `E` and `*` are tool letters in every state. To pick a
  tool while something is selected, press Escape (clears the selection, last
  step of the cascade) and then the letter, or click the rail; the rail
  tooltip names that sequence while it applies ("Rectangle tool (Esc, R)").
  *Customer to confirm; fallback: tool letters unchanged and the four entries
  on Shift+M, Shift+R, Shift+S, Shift+K.*
  - **No shortcut fires during an operation or while typing.** A letter or
    Delete is ignored, with no effect and no `preventDefault`, when any of
    these holds: Ctrl, Cmd or Alt is down (Shift is allowed only for `*` and
    Shift+K); the event is a key repeat or an IME composition; focus is in a
    text field, select, button, switch or contenteditable, or an entry chip is
    open; a drag is in flight in any tool (move, resize, rotate, skew,
    parameter handle, marquee, lasso, create-drag, node or handle drag, pen
    handle); the Pen has an unfinished path; Space is held. Caps Lock changes
    nothing (read the Shift flag, not the letter's case). Four keys are never
    gated because they are how an operation is left or continued: Escape,
    Space (pan; never interrupts the operation), Enter (finishes the Pen path)
    and the modifier keys. The gate is one function the key handler calls
    first, not a check per shortcut. Letters match the typed character
    (layout-aware, so a German keyboard works); where the key is not a Latin
    letter the physical key position is the fallback.
  - **A key that opens an entry opens the same chip as a double-click on one
    named handle**, with that handle's pivot rules (never the Shift pivot):
    M the center handle, R the top-right corner rotate handle, S the
    bottom-right corner resize handle (the one exception to the rule: S
    scales about the box center, as a Shift drag would, because no handle was
    chosen, so its chip is placed by the center like the move chip, with no
    handle highlighted and the pivot marker solid there; the double-click
    keeps the drag's fixed point, its handle and its chip at the handle), K the top skew handle (skew x),
    Shift+K the right skew handle (skew y). It works when that handle is
    hidden by size or by a neighbouring glyph (the chip goes where the handle
    would be), which closes the small-object gap of the typed move. The
    chip behaves as in "Numeric entry chip": Enter commits, Escape cancels
    (the first Escape only closes the chip), focus returns to the canvas, the
    owning handle shows its dragging look, Shift-revealed handles are hidden
    while any chip is open. M, R, S, K with an object that cannot take the
    entry (K on a selection that holds a rectangle, ellipse, polygon or star:
    "Skew works on paths only"; M, R, S or K with nothing selected: "Select an
    object first") show a hint-chip line for 2s and change nothing. Since
    `multi-object-transform` M, R, S, K act on several selected objects as well
    (the group box's chips, row "Group entry chips"); "Select one object to type
    a value" no longer exists.
  - **Escape cascade** (one step per key press, key repeat ignored): an open
    chip or a focused bar field; else a drag in flight; else the active tool's
    own state (Pen path, Node selection, Select selection); else the Select
    tool. Nothing is written by any step.
  - **Discoverability:** the rail tooltip carries the letter, and the Esc
    sequence while a selection blocks the plain letter; the Select tool's
    tooltip reads "Select tool (S or Esc)" with, since `0044`, a muted second line "Arrows nudge 1 mm, Shift 10 mm. Ctrl+A selects all."; the handle hint chip names the key
    next to double-click; every shortcut is in the table below. A `?` help
    overlay is proposed as its own story (the table is its content); nothing
    on screen lists shortcuts until then.
  - **Three classes of key, one gate (2026-10-10, `0020`, `0044`).** *Letters*
    are the keys above. *Chords* are Ctrl or Cmd with a letter: Ctrl+Z,
    Ctrl+Shift+Z, Ctrl+Y (not macOS), Ctrl+U, Ctrl+Shift+U, Ctrl+A. They need
    Ctrl or Cmd, refuse Alt, ignore key repeat except undo, redo and the two
    object-undo chords (a held Ctrl+Z walks back), pass the same gate as the
    letters, but a focused panel control that is not a typing field (a value
    field outside its typing state, a toggle) does not block them; a typing
    field keeps its own text undo and select-all. *Arrows* nudge (Arrow 1 mm,
    Shift+Arrow 10 mm), act on every repeat event, refuse Ctrl, Cmd and Alt.
    Matching is by the typed character, so Z and Y are right on a German
    keyboard. An ignored key gives no feedback and no `preventDefault`, except
    where a spec names a hint. Feedback for a key that has no control (undo,
    redo, object undo, nudge limits) is the **canvas notice slot** (row in
    "Interaction conventions" below).
  - **Reserved, not built:** Ctrl+D (duplicate in place), Ctrl+K (combine,
    `0006`), H and V (flip), `?`. A new tool letter must not be M, K, H or V,
    and any later selection action takes a letter from that list before it
    takes one from the tool letters. Specified, not yet built: Ctrl+Z,
    Ctrl+Shift+Z, Ctrl+Y, Ctrl+U, Ctrl+Shift+U (`0020`, `0041`). Built in
    `0044`: Ctrl+A and the arrow nudge.
- **Modifier indicators in a move** (`edit-interaction-polish`; rows "Modifier
  badge" and "Move axis guide"). Ctrl and Shift may be held before the press,
  pressed or released at any moment of the drag, and the result follows the
  state of the current frame. **Ctrl (copy):** a plus badge by the pointer
  wherever a press with Ctrl would start a move, the word "Copy" at the end of
  the readout, the blue outline at the copy's position as in any move, and,
  as the distinguishing mark between a move and a copy, **the selection box
  and handles stay on the originals while a copy is dragged** (in a move they
  travel with the blue outline). **Shift (axis lock):** while a move drag is
  past the dead zone the lock badge and the two origin axis lines show, the
  axis is re-chosen on every pointer event (the nearer axis wins; the object
  jumps when the pointer crosses the diagonal, as in Inkscape; no latch), the
  readout shows the locked delta. All of it appears and disappears in the
  frame of the key event, also with the pointer at rest. Escape, the end of
  the drag and window blur clear it; Ctrl stays shown as a prediction while it
  is held over something a press would move.
  - **Shift or Ctrl at the press.** Ctrl at a press that starts a move (an
    object's outline, an unselected object, the center handle) is a move
    modifier: a copy if Ctrl is still down at the release. Ctrl on empty canvas
    belongs to `advanced-selection`'s marquee and never starts a move, so the
    two do not clash. Shift at a press on an object does not change the
    selection at the press: released without leaving the 3px dead zone it
    toggles that object as in slice 4; once the drag has left the dead zone it
    is an axis-locked move of the selection (an unselected pressed object
    joins the selection at that moment). Shift or Ctrl at a press on the
    drawn center handle is always a move modifier and never toggles.
- **Modifiers in a rectangle or ellipse create-drag**
  (`shape-creation-from-center`, UX review 2026-10-07). One rule across the
  product: **Ctrl = 1:1, Shift = about the center** (resize, create), plus the
  move's Shift = one axis and Ctrl = copy. Held before the press, pressed or
  released at any moment of the drag, with the pointer at rest: the blue
  outline and the readout follow in the frame of the key. The outline and the
  readout are the feedback (no badge, no legend, no word in the readout: the
  geometry already shows both effects, unlike a copy). While **Shift** is
  down in a create-drag the **pivot marker** (row "Transform pivot marker")
  is drawn at the press point, because that point is the center and nothing
  else shows it (at small sizes and when the far side leaves the canvas the
  outline does not). Gone in the frame Shift is released, the drag ends or
  Escape cancels. Discovery is the rail tooltip of Rectangle and Ellipse, a
  second line "Shift: from center. Ctrl: square or circle". The ellipse
  readout shows radii ("rx × ry mm"), the rectangle's the full size, as
  shipped; both unchanged by the modifiers.
- **Polygon/star angle** (`edit-interaction-polish`): a polygon or star's
  rotation is its real orientation, not a register that reads 0 whatever it
  looks like. The Select tool's rotate readout, the angle chip prefill and the
  oriented box all use it; 0° is the first outer vertex pointing right, angles
  run clockwise on screen. A create-drag stores the drag direction as this
  angle (dragging right is 0°, down 90°), so what the maker sees after the
  release is what the readout said during the drag. Typing 0 into the angle
  chip (shortcut: R, 0, Enter) restores the first vertex pointing right at any
  time; Ctrl while rotating a polygon or star snaps the **shown** angle to the
  15°/22.5° stops (absolute, so a shape created at 78.7° reaches 75° and 90°,
  not 78.7° plus a stop). For an N-gon an edge lies on an axis at 0° when N is
  odd or a multiple of 6, and at 180/N° (45° for 4, 22.5° for 8) otherwise.
  Optional, flagged as scope: a polygon-group button "Edge to axis" in the
  Select bar that turns the polygon by the smallest angle that puts an edge on
  an axis; not for stars.
- **Modifier keys follow the keyboard, with one known gap** (`advanced-selection`):
  the marquee's mode and combine, the lasso, the badges and the hover follow
  Shift, Ctrl and Alt the frame the key changes, with the pointer at rest. The
  host tracks the keys by `code` (left and right separately), corrects the
  record from every pointer event and forgets it on blur. **Known limit:** on
  the customer's Linux/WebKitGTK setup the Shift keyup is never delivered to
  the page, so after Shift is released a stationary pointer keeps the `+Add`
  legend until the next pointer event corrects it. Ctrl and Alt are
  unaffected.
- **Marquee and lasso cursors** (`advanced-selection`): the built-in `crosshair`
  from the press of a marquee (empty canvas, Alt up) to its release, whatever
  Alt does afterwards (a box inverted by Alt is told by its colour, not by the
  cursor). The lasso glyph (24px, the same white-halo-under-black
  construction as the rotate and resize cursors, hotspot at the arrow tip,
  `crosshair` where custom images are ignored) while Alt is held with no
  button down and from the press of a lasso (Alt down at the press) to its
  release, also if Alt is let go mid-drag. Shift and Ctrl change neither.
  *(2026-10-08, UX review N4: the loop-with-tail glyph read as a magnifier.)*
  The lasso glyph is the pointer arrow (white fill, black outline, tip at the
  hotspot) with a short dashed squiggle trailing from its lower right, drawn
  white-halo-under-black like the other cursors; the dashes echo the 4 / 3
  line the drag draws.
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

- **Pen targets and cursors** (`pen-path-extension`): over an end node the Pen
  can act on, the node is drawn with its glyph and the hover ring, the cursor
  names the action (nib, close, continue, join), and a two-line hint chip says
  what a press does and what Shift does instead. One rule for the key: Shift is
  always "the alternative at this target". Rows "Pen target cue", "Pen hint
  chip" and "Pen cursors". Close path, the command, is a group of the Node
  bar (row "Close path group").

- **Switch (`ScaleStrokeSwitch`)** (2026-10-06, `object-transform`
  criteria 8, 26-31, new): the pattern for one persistent on/off setting
  that is tool state, not an object property. First use: "Scale stroke
  width". Spec of the control:
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
  - **Host and bar layout** (customer decision 2026-10-06: the switch is a
    Select tool setting). `SelectToolbar` is the Select tool's own
    permanent bar, shown whenever `editor.tool === "select"`, with or
    without a selection (criterion 30); later transform-tool settings are
    appended to it. It is the third bar after `NodeToolbar` and
    `ShapeToolbar` and looks identical: it lives in the same
    `pointer-events-none` overlay row in `App.tsx` (`absolute top-3 right-3
    left-[72px] z-20`, centered, right of the tool rail; since 2026-10-07
    inside the canvas region, not at the window edge, so it never runs under the
    Properties panel), pill `h-9`,
    `rounded-lg`, `px-2`, `--toolbar-bg`, `--panel-elevation-shadow`,
    `pointer-events-auto`. It floats over the canvas, so the canvas never
    resizes. It holds this one control, with room to append later Select-
    tool options after it at 12px gaps (a vertical 1px `--toolbar-icon` at
    25% opacity divider between items, 20px high, once there are two).
  - **Superseded 2026-10-06 (`unified-object-editing`):** the bar no longer
    holds one control. Its layout, order, wrapping, the second switch "Scale
    corner radius" and the kind groups are in "Select bar" under "Interaction
    conventions" and in the row "Select bar layout". The switch tooltips are
    "Scale stroke width with the object. Off: a resize keeps the stroke
    thickness." and "Scale corner radius with the object. Off: a resize keeps
    the corner radius." The pill is `min-h-9`, not `h-9`, and left-aligned for
    this bar.
  - **One bar at a time.** The three bars are the same slot, rendered by
    mutually exclusive conditions on the active tool (select, node,
    rectangle/ellipse/polygon-star); Pen shows none. Switching tools swaps
    the bar in place, at the same position, with no animation. A bar is
    never stacked or shown next to another one. Because the overlay row
    is `flex justify-center`, the bar's width differs per tool but its top
    edge does not.
  - **Focus and keys.** Tab order: tool rail, canvas, bar, same as the
    other bars. The Space-to-pan key handler and the single-letter tool
    shortcuts must ignore key events whose target is inside the bar
    (no `preventDefault`), or Space could not toggle the switch. Switching
    tools while the switch has focus unmounts it; focus falls back to the
    canvas as it does for the other bars.

## Properties panel: Style section (`stroke-and-fill-styling`, rewritten by `style-panel-rework`, 2026-10-08)

Sizes, tokens and component rules for the right-docked `PropertiesPanel` and its
first section, "Style". The decisions and reasoning are in
`specs/0017-style-panel-rework/specification.md`, "UX notes" (and
`specs/0018-stroke-markers/specification.md` for the Markers block); this section is
the reference an implementer builds from. It replaces the `0007` version: there
is no colour popover, no dash dropdown, no gradient bar or stop list, and no
disabled control any more. The history of those is in git.

**The rule (customer, 2026-10-08): no popups in the properties panel.** See the
convention of that name under "Interaction conventions"; it is binding for
every row below and for every later section.

**Layout.**

| Item | Value |
|---|---|
| Regions | Below the native menu, above the status bar: the canvas region (`relative flex-1 min-w-0`) and the panel (280px, `shrink-0`, full height). The tool rail and the bars' overlay row are children of the canvas region |
| Overlay row | `absolute top-3 right-3 left-[72px] z-20` (`left` is `--rail-right` + 12: 72px with one rail column, 128px once the second column of the toolbox rail ships, row "Tool rail architecture") **inside the canvas region**. Its right limit is the canvas region's right edge less 12px, so a bar never runs under the panel. Bars wrap by whole groups as before |
| Panel | `<aside aria-label="Properties">`, `--panel-bg`, a flex column since `0043`: the **header row with the tab strip** (`shrink-0`, does not scroll) and the **body** below it (`overflow-y: auto`, `overscroll-behavior: contain`, `scrollbar-width: thin`, one scrollbar for the whole body, no inner scroll areas; the scrollbar starts under the strip). **Exception: the History tab**, whose list is the one inner scroll area (the tab fills the body height, header rows and footer stay in view; row "History tab layout"); no shadow (docked); 1px left edge line `--toolbar-icon` at 25%; `user-select: none` except in its text inputs |
| Padding, content width | 12px, content 244px (280 less 24 less a thin scrollbar) |
| Collapse tab | 16 x 48px, on the panel's canvas-facing edge, vertically centered, `--toolbar-bg`, `--panel-elevation-shadow` on its canvas side, 1px `--toolbar-icon` at 25% outline, chevron 12px; first Tab stop of the panel; collapsed it sits on the canvas region's right edge and the panel content is `inert`. Name "Hide properties panel" / "Show properties panel", `aria-expanded`, `aria-controls`; tooltip "Properties (Shift+Ctrl+F)". State per session, default open, not saved |
| Opening and closing | Resizes the canvas region through the existing `ResizeObserver` path; the document does not move on screen (the view keeps its top-left origin) |
| Window minimum | 800 x 600 (`tauri.conf.json` `minWidth`, `minHeight`). Measured at the build of `0007` PR 3, no raise needed: at 800 the canvas region is 520px (with the rulers of `document-size-and-rulers` the viewport is about 496px wide, row "Viewport and rulers"); the Select bar wraps to three rows, the Node bar is one row of about 300px, the polygon/star bar about 310px. The panel never shrinks, never auto-collapses and is never an overlay |
| Empty | While the **Pen has an unfinished path** (the only case since 2026-10-09) the Document and Style bodies are empty: the frame, the collapse tab and, since `0043`, the tab strip stay; no heading, text, icon or control in the body, no live region. The History body is shown, read-only (presses on its rows are ignored with the Pen hint). With nothing selected (any tool, including the Pen without a path and a Node tool with nothing to edit) the Document tab is shown instead ("Properties panel: Document section"), never an empty Style body. Switching between tabs is instant and changes no size |
| Hidden, not disabled | A control that cannot apply is removed from the tree (not `disabled`, not `aria-disabled`, not greyed). **One recorded exception (`0043`): the Style tab is dimmed, `aria-disabled` and not focusable while nothing is selected**, because a tab strip whose icons come and go is worse than one dimmed icon; the toolbox buttons already dim. Rows leave and enter instantly: no height animation, no reserved space. The section title row (with the Paint switch) is the anchor: it never moves when its own rows come and go |
| Tooltips and validation chips | The only layers that may appear over content, and only text: tooltips (Radix `Tooltip`, 400ms, `side="left"`, no controls, no focus) and the validation chip (see below). Both render in a portal over everything (z above the overlay row), may cover the canvas, never take focus |
| Heights | Header 24px + 8; Stroke (Solid, no markers) 404px; Markers block 132px (204px with Place and Count); Fill (Solid) 224px; section gap 16px; paddings 12px top and bottom. Whole panel 700px (no markers), 832px (markers), 904px (markers with a Middle shape); each 4px more since `0043` (the strip row is 28px, the old header 24px). At 800 x 600 the viewport is about 570px: the header and Stroke to the Cap row show, Fill is reached by scrolling |

**Rows.**

| Item | Value |
|---|---|
| Row | 28px high, 8px between rows; label column 60px, 8px gap, control column 176px; labels 14px `--toolbar-icon` (8.3:1). Rows without a label (Color, picker, the value fields) use the full 244px |
| Subsection gap, rule | 16px between Stroke and Fill, a 1px `--toolbar-icon` at 25% line in the middle |
| Header | One row since `0043`: the **tab strip** at the left (three 36 x 28px tabs, row "Properties panel: tabs") and the **subject line** right-aligned in the remaining 120px, 12px `--panel-muted-fg` ("Rectangle", "3 rectangles", "4 objects", "2 paths"), ellipsis with the full text in a tooltip. The title word "Style" (14px semibold) of earlier versions is gone: the pressed tab names the context. Not a live region. 28px + 8px, 4px more than the 24px + 8 before |
| Section title row | The title ("Stroke", "Fill"; 12px semibold) in the label column, the Paint group None / Solid (2 x 44px) in the control column. With Paint None this row is the whole section |
| Color row | Swatch 28, 4px, eyedropper button 28, 4px, hex field (the rest, about 180px). No label |
| Picker | Under the Color row, 8px gap: saturation/value area 244 x 96, 8px gap, hue slider 244 x 12. Always visible while the paint is Solid. Block height 116px |
| Value field | See "Value field" below. 244 x 28 |
| Dash | Preset group 4 x 44px (176px) in the control column; under it the Pattern row |
| Pattern | Label "Pattern", text input 176px |
| Join, Cap | `ToggleGroup`, 3 x 40px |
| Markers | `stroke-markers`: "Markers block" below |

**Text field (hex, pattern).** 28px high, white ground, 14px tabular text, 1px
border `--toolbar-icon` at 60% (3.1:1 on the panel), `rounded-[5px]`; hex
left-aligned and about 180px; pattern left-aligned, 176px, with a fixed suffix
"x width" inside the right edge (12px, `--panel-muted-fg`). Enter commits and
returns focus to the canvas, Tab commits and moves on, Escape or a press elsewhere
restores and writes nothing, Enter on text the maker did not edit writes nothing.
Focus: 1px `--editor-accent` border plus inset 1px. Mixed: empty with the
placeholder "Mixed" (`--field-placeholder`, 5.3:1). Invalid: 2px `--field-invalid`,
`aria-invalid`, validation chip, cleared on the next keystroke.

**Validation chip.** An overlay below the field, right-aligned to it, so no row
shifts: 12px text, `--field-invalid` text and 1px ring, a 12px alert glyph,
`--popover` ground, `role="alert"`, `pointer-events: none`, maximum width 244px.
Messages: hex "Enter 3, 4, 6 or 8 hex digits"; Width "Enter a number from 0 to
1000"; Opacity "Enter a number from 0 to 100"; Count "Enter a whole number from
1 to 500"; Pattern "Enter 1 to 16 numbers from 0 to 1000, for example 1 2 4 2".

**Value field** (`ValueField`; Width, Opacity, Count; the model for every
number the panel edits by dragging). The GIMP tool-option field: a label at the
left, the value at the right, a bar filled to the value. Customer decisions:
drag inside the field, even outside its bounds, or type; no +/- buttons; no text
selection by dragging; a slightly logarithmic scale for Width and Opacity.

| Part | Rule |
|---|---|
| Box | 244 x 28px, `rounded-[5px]`, white ground, 1px border `--toolbar-icon` at 60%; a `div` with `role="spinbutton"` and `tabindex="0"` outside the typing state (not an `input`); `user-select: none`, `-webkit-user-drag: none`, `touch-action: pan-y`; cursor `ew-resize` |
| Fill bar | Behind the content from the left edge to the value's position `p` on the field's scale; `--value-fill`; the right edge has a 2px `--value-edge` line (4.5:1 on white). Nothing is drawn at 0; "Mixed" draws no bar. No thumb, no tick marks, no stepper |
| Label | 8px from the left, 14px `--toolbar-icon`: "Width", "Opacity", "Count" |
| Value and unit | Right-aligned, value 14px tabular `--toolbar-icon`, unit ("mm", "%") 14px `--panel-muted-fg`, 4px apart, ending 32px from the right edge. Width: up to 3 decimals, no trailing zeros; Opacity and Count: integers. Count has no unit |
| Reset slot | 24px at the right end, always reserved. Icon Lucide `RotateCcw` 12px, 1.5px stroke, `--toolbar-icon` at 80% (6.2:1 on white, 4.3:1 on the bar); shown when the value differs from the default or is Mixed. A `button`, `tabindex="-1"`, name "Reset stroke width to 0.25 mm", sibling of the spinbutton in the DOM, not inside it. Defaults: Width 0.25 mm, Opacity 100 %, Count 1 |
| Hover | Border `--toolbar-icon` at 100% (8.3:1). The bar does not change (a darker tint would put the unit below 4.5:1) |
| Focus | The panel's focus ring: 2px `--editor-accent` with a 1px `--toolbar-bg` offset, `focus-visible` only |
| Drag | Pointer captured; the cursor stays `ew-resize` in the whole window until release; with Shift or Ctrl down, the muted 12px word "coarse" or "fine" shows centered in the field (`aria-hidden`); no tooltip while dragging |
| Typing | Bar hidden, border 2px `--editor-accent`, number selected, unit stays, text right-aligned where the value was, cursor `text`. The label is not drawn while typing (the input covers the box; the accessible name stays; decided 2026-10-10). Invalid: border `--field-invalid` |
| Mixed | "Mixed" in `--field-placeholder` in place of value and unit; no bar; reset icon shown |
| Disabled | Does not exist |
| Mapping | `p` is 0 at the left end, 1 at the right end. Width (mm): `v = 20 (100^p - 1) / 99`, inverse `p = ln(1 + 99 v / 20) / ln 100`, drag 0 to 20, typed 0 to 1000, grid 0.01 (Shift 0.1, Ctrl 0.001). Opacity (%): `v = 100 (4^p - 1) / 3`, inverse `p = ln(1 + 3 v / 100) / ln 4`, integers. Count: linear, `p = (v - 1) / 49`, drag 1 to 50, typed 1 to 500. A value above the drag maximum draws a full bar |
| Modifiers | Shift: `p` moves ten times as fast (arrow keys: step x10). Ctrl (Cmd on macOS): ten times slower (step never below the grid). Same keys for mouse and keyboard. Value is `clamp(p0 + dx / W)` from the press, no re-basing at the ends; a modifier change mid-drag re-bases. A Mixed field has no `p0`: the drag is absolute, `p = (x - left) / W`, and stays so for the whole gesture even though the preview makes the objects equal after the first tick (`lib/dragPosition.ts`). Shift on an arrow key steps ten times and then rounds to the Shift grid, so 97 % goes to 90 %, not 87 % (it lands on round numbers; decided 2026-10-10) |
| Keys | Arrows step on the grid (preview on key-down, one commit on key-up); Home / End: scale minimum / end; Enter, F2, a digit, `.`, `,`, `-` start typing; `Ctrl+Backspace` (Cmd on macOS) resets; Backspace and Delete alone do nothing and never reach the canvas |
| Semantics | `aria-label` "Stroke width" (contains the visible "Width"), `aria-valuemin`, `aria-valuemax` (typed maximum), `aria-valuenow`, `aria-valuetext` ("0.25 millimetres", "50 percent", "3 markers"; Mixed: "Mixed", no `valuenow`), `aria-keyshortcuts="Control+Backspace"` |
| Tooltip | "Drag to change, click to type. Shift: coarse. Ctrl: fine. Ctrl+Backspace: reset." (Cmd on macOS) |

**Swatch.** 28 x 28px, `rounded-[5px]`, 1px `--swatch-border`, 1px white inner
line; shows the colour at its alpha over the checkerboard (`--checker-a`,
`--checker-b`, 7px cells); mixed: 45 degree hatch, 4px stripes (`--checker-a`,
`--mixed-hatch`). Display only: `aria-hidden`, no hover state, not a Tab stop.

**Eyedropper button.** 28 x 28px, Lucide `Pipette` 16px, 1.5px stroke, 1px
`--toolbar-icon` at 60% outline, hover `--editor-accent-hover`, focus ring as
above. Pressed (`aria-pressed="true"`, while picking): `--toolbar-icon-active-bg`
ground, `--toolbar-icon-active-fg` glyph. Names "Pick stroke color from the
drawing" / "Pick fill color from the drawing". Tooltip "Pick a color from the
drawing (Esc cancels)". While picking: canvas cursor an eyedropper image (24px,
black with a white casing, hotspot at the tip, fallback `crosshair`); no object
hover highlight; the **colour chip** follows the pointer: the "Live transform
readout" surface (`--toolbar-bg`, 8px radius, 12px up and right of the pointer,
flips at the canvas edges, `pointer-events: none`) with a 16px swatch and the
colour as `#RRGGBBAA` in 12px tabular text, a muted word "stroke" or "fill" after
it when the architect's sampling rule has one, and "No paint here" without a
swatch where nothing is painted. Updated at most once per animation frame. It is
a readout, not a popup: no control, no focus. Pan, zoom and the wheel work while
picking.

**Colour picker (inline).** Area 244 x 96, `crosshair`, `role="slider"`, name
"Stroke saturation and value" (Fill likewise), `aria-valuetext` "Saturation 50 %,
value 40 %"; hue slider 244 x 12 with the sRGB rainbow ramp, `role="slider"`,
"Stroke hue", `aria-valuetext` "Hue 215 degrees". Thumbs: area 14px, hue 16px,
a 2px `--picker-thumb-ring` inside a 1px `--picker-thumb-casing`, so they read on
every colour. No alpha slider. The picker keeps its own HSV state and re-derives
it from the stored colour only when the colour changed from outside it, so the
hue does not jump through greys; mixed colours show no thumb. A press on the area
moves the thumb to the press point. Arrow keys 1 % (Shift 10 %), preview on
key-down, commit on key-up. Our own component (no colour-picker dependency): the area
and the hue ramp are drawn from Rust's HSV conversion; there is no alpha part.

**`ToggleGroup` item.** 40px wide (Join, Cap, marker choices), 44px (Paint, Dash
presets), 28px high; icon 16px, 1.5px absolute stroke; the group is one 1px
`--toolbar-icon` at 60% bordered strip, items separated by 1px; pressed:
`--toolbar-icon-active-bg` ground, `--toolbar-icon-active-fg` glyph (4.5:1; 3.3:1
against the panel); unpressed `--toolbar-icon` glyph; hover `--editor-accent-hover`;
focus-visible 2px `--editor-accent` ring with a 1px `--toolbar-bg` offset (inside
the strip for the roving item); `role="radiogroup"` / `radio`, one Tab stop per
group, arrows move and select; mixed: nothing pressed. Built on Radix
`RadioGroup`, not Radix's `ToggleGroup` (it moves and selects with the arrows,
keeps one Tab stop and handles Home and End).

**Dash preset group.** Four items (Solid, Dash, Dot, Dash-Dot) in one
`ToggleGroup` strip, 44 x 28px each. Each shows a 2px line sample in
`--toolbar-icon` (pressed: `--toolbar-icon-active-fg`), centered, 32 to 34px wide,
at the stored ratios with whole repeats: Solid unbroken; Dash 8 on / 5 off (three
dashes); Dot 2 / 6 (five dots); Dash-Dot 9 / 4 / 2 / 4 (dash, dot, dash, dot).
The stored lists are multiples of the stroke width: Solid `[]`, Dash `[6, 4]`,
Dot `[1, 3]`, Dash-Dot `[6, 3, 1, 3]` (every "on" above 0); a pattern whose
period is under 2 screen px draws solid. The group's tooltip carries the note
"Patterns scale with the stroke width and draw solid when too small to see". A
list that equals no preset leaves all four unpressed and shows its numbers in
the Pattern line; there is no "Custom" entry. Pattern line: numbers separated by
spaces (a comma is refused with the error chip), 1 to 16 numbers from 0 to 1000, shown back as numbers
with one space ("1 2 4 2"); tooltip "Lengths in multiples of the stroke width: on,
off, on, off. Example: 1 2 4 2".

**Markers block** (`stroke-markers`). Under Cap, 8px below it: a title row
"Markers" (12px semibold, 16px), then Start, Middle, End (label column + a
three-item `ToggleGroup` of 40px items: None, Arrow, Dot; 16px glyphs showing a
line with the decoration at its left end, in the middle, or at its right end; the
Start arrow points away from the line), then, when Middle is not None, Place (two
88px text items "Spaced", "At nodes") and, when Place is Spaced, Count (value
field, linear 1 to 50, typed 1 to 500). When every selected path is closed and a
Start or End shape is set, one muted 12px line under End: "Closed paths have no
start or end." The block is removed with the stroke (Paint None), with only
primitives selected, and when nothing is selected.

**Colour model.** The picker works in sRGB. The hex field shows and takes
`#RRGGBBAA`; the Opacity field shows alpha as an integer percent. Alpha set by
the hex field is stored as `AA / 255`; alpha set by the Opacity field as `N / 100`;
a value off both grids is shown rounded and is never rewritten by looking at it.
Opacity 0 is still a paint (the swatch is a checkerboard). No paint is the Paint
switch, not a swatch item.

**Behaviour.** Preview and commit: drags (picker area and hue, value fields)
render the selected objects in the new style on every pointer move (one update
per animation frame, through the ephemeral `StyleOverride`) and make one commit
on pointer-up, also outside the control; Escape during a drag reverts and the
release writes nothing; keys on a slider or value field preview on key-down and
commit on key-up; typed values do not preview; discrete controls (Paint, Dash
presets, Join, Cap, marker choices, reset) commit on the click. The commit goes
to the objects the edit started on. Focus: a mouse press on a button-like
control or the end of a drag returns focus to the canvas, keyboard use does not;
when a focused control leaves the tree focus goes to its section's Paint group
(the canvas if the panel became empty). Escape in the panel: ends eyedropper
picking, else reverts a running drag, else restores an edited field and
returns focus to the canvas; it never clears the selection and never reaches
the canvas cascade. Backspace, Delete and letters in any panel control never
reach the canvas. The panel is a sibling of the canvas container, so canvas key
handling does not see its keys; `isFormControl` also lists `[role="slider"]`,
`[role="spinbutton"]`, `[role="radio"]` as a second guard.

**Frontend components** (`frontend/src/components/`): `ValueField`, `TextField`
(the typed-field rules of the former `NumberField`, for hex and Pattern),
`ColourPicker` (area and hue), `EyedropperButton`, `DashPresets`, `MarkerRows`,
`ToggleGroup` (over Radix `RadioGroup`), `Tooltip`. `ui/popover.tsx` and
`ui/select.tsx` are deleted and must not come back; so are the colour popover,
the dash dropdown and the gradient components. Fed by `useStylePanel.ts`, not
`useEditorSession.ts`.

**Carried over from the `0007` build.** Panel details found in the UX review that
still hold: the pressed look of a toggle item is keyed on `aria-checked` (the
tooltip trigger replaces `data-state`); the collapse tab is drawn 16px wide inside
a 24px hit target; Escape in the panel returns focus to the canvas even when a
tooltip took the key; a press on dead space in the panel leaves focus where it was
(the canvas); a control focused by `Shift+Ctrl+F` carries a ring of its own; the
tab's tooltip reads "Shift+Ctrl+F" on every platform like every shortcut label in
the app; opening and closing the panel keeps the document where it is on screen
(the toggle announces its width change to the viewport first, and that one resize
keeps the top-left origin); the Node tool with no node selected edits the paths of
the object selection.

**Casing over artwork.** Fills make artwork the background of every line the
editor draws on top. Measured 2026-10-07 (WCAG ratio, 1px `--accent` against the
fill / white against the fill): black 4.6 / 21.0, white 4.5 / 1.0, `--accent`
blue 1.0 / 4.5, mid grey 1.1 / 3.9, red 1.1 / 4.0, yellow 3.3 / 1.4, navy 3.5 /
15.8, canvas 3.7 / 1.2. Rule: every `--accent` line or glyph stroke without a
white ground of its own is drawn over a white casing (`--selection-casing`) one
line width wider on each side, under the line, above the artwork. Applies to the
selection box (under the dashes only, so the 4 / 3 rhythm and the pixel snapping
are unchanged), the hover box, the blue preview outline, the rotate and skew
glyph strokes, Bézier handle lines, the Node tool's segment overlay, the skew
fixed-line guide and the pivot marker. Not to glyphs with a white ground (resize,
center, parameter handles, node glyphs, handle endpoints), the marquee and lasso
(a drag mode), or the axis guide (informational). Hover box: `--hover-box`
(`--accent` at 65%, casing at 65%; both, because a 65% line over a 60% casing
leaves red at 1.96:1), pending customer confirmation (default accepted); better
of line and casing at least 2.0:1 analytically on the fills above (yellow 2.04, red 2.14,
canvas 2.16; yellow read from the GL buffer 1.97), where 20% measured 1.0 to 1.3:1 and 60% measured 1.92 on yellow.

**Document background (`0040`).** The ratios above were measured on the default background (`--canvas-bg`). For any other opaque background the rule still guarantees the full-strength lines: the better of the `--accent` line and the white casing is at least 2.1:1 against every colour (the worst case is a colour of relative luminance 0.445, where both are 2.12:1), and over the canvas checkerboard 4.5:1 on `--checker-a` and 2.7:1 on `--checker-b` for the line. The hover box and the member box (60 to 65 %) are weaker, as measured per fill; a translucent background is judged on the colour it composites to. The rule does not change with the background; the overlays are not recoloured.

## Properties panel: Document section (`document-size-and-rulers`, 2026-10-09)

The content of the **Document tab** (`0043`): shown when nothing is selected, or when the maker pressed the tab with a selection present (the new case: size, preset, unit and background are editable while objects stay selected). Decisions and reasoning:
`specs/0015-document-size-and-rulers/specification.md`, "UX notes". The rules of the
Style section apply unchanged: no popups, nothing disabled, text-only tooltips
and validation chip, focus returns to the canvas after a mouse action.

| Item | Value |
|---|---|
| Shown when | The Document tab is active (`0043`): by default when no object is selected, in any tool, and by the maker's press at any time. **Unless the Pen has an unfinished path** (then the body is empty: a resize would move the committed objects and not the path being drawn). Never together with the Style area. The panel's width, the canvas and the rulers do not change when it comes and goes. Not at the bottom of the Style block and not at its top: with a selection it would sit below the fold at 800 x 600 and would shift as Style rows come and go |
| Header | The header row of the panel (tab strip at the left, since `0043`; the title word "Document" is gone) and, since `0030`, the **subject line** right-aligned (12px `--panel-muted-fg`, as the Style header): the matching preset and orientation ("A4, portrait", "16:9, landscape"; a square gives the name only) or "Custom" when no preset matches. Derived from the size on every change, not stored, not a live region |
| Rows | Same grid as Style: 28px rows, 8px apart, label column 60px, control column 176px, labels 14px `--toolbar-icon`. Order and DOM order since `0030`: the format strips, Orientation, a 16px gap with a 1px `--toolbar-icon` at 25% rule, then Width, Height, Unit (Part C only), Fit to content. Since `0045` the strips are the favourites of the groups that are on (Paper by default, Slides off) and the row **All formats** (disclosure, 28px) follows Orientation; row "Document formats library". The rows above the Background block are about 352px (192px before `0030`); they and the Background title row, Color row and picker area show at 800 x 600 (viewport about 546px) without scrolling. With a longer preset file, or with the Background block (Solid, 592px in all, 616px with padding), the panel scrolls as a whole, as it always does |
| Width, Height | `TextField` (the typed-field rules of the Style section), **not** `ValueField`: a size is committed in one operation that moves every object, spans 1 to 100000 mm, and is not explored by dragging. 176px, right-aligned 14px tabular text, the unit as a fixed 14px `--panel-muted-fg` suffix inside the right edge (32px right padding). Accessible names "Document width", "Document height". Display: 3 decimals (mm), 4 (cm, in), no trailing zeros; showing a value never rewrites the stored one. Enter commits and returns focus to the canvas; Tab commits and moves on; **leaving the field with edited text commits it** (criterion 15; a refused value stays in the field with its message); Escape restores; Enter on unedited text writes nothing; decimal point or comma, surrounding spaces ignored, `inputmode="decimal"`, no unit text parsed. No reset icon, no fill bar, no drag, no stepping |
| Validation | The validation chip, limits in the display unit and rounded inward: mm "Enter a number from 1 to 100000"; cm "Enter a number from 0.1 to 10000"; in "Enter a number from 0.04 to 3937". 2px `--field-invalid` border, `aria-invalid`, cleared by the next keystroke. The size is not changed |
| Unit (Part C) | Label "Unit"; `ToggleGroup` with text items "mm", "cm", "in", 44 x 28px each (132px) in the control column, `role="radiogroup"`, one Tab stop, arrows move and select. Not a `Select`. A field with edited text commits in the old unit when the group is pressed, then the unit changes. Without Part C the row is absent and the suffix is a fixed "mm". Tooltip "How lengths are shown on the rulers, here and in the status bar. Other fields stay in mm. Stored sizes are always mm." |
| Fit to content | A 244 x 28px text button: 1px `--toolbar-icon` at 60% outline, `rounded-[5px]`, 14px `--toolbar-icon` label, hover `--editor-accent-hover`, focus ring as the panel's. Removed from the tree when the document has no objects (never disabled); it is the last row so its removal moves nothing. Activating it on a document that already fits writes nothing and shows the Action notice "Already fits the content." (3s) under the button. Tooltip "Resize the document to the extent of all objects, without margin. Objects move so the extent starts at 0, 0." |
| Background block | `0040-document-background`. **Last block of the section, below Fit to content**, after a 16px gap with the 1px `--toolbar-icon` at 25% rule (as Fill under Stroke). Frequent edits (a format, a size) keep the top; a fill touched once per project sits under them. Title row 28px: "Background" (12px semibold `--toolbar-icon`) at the left, the Paint group **None / Solid** (the Style `ToggleGroup`, 2 x 44px) right-aligned to the content edge (x 156 to 244), because the title is about 66px and does not fit the 60px label column. The row is the block's anchor and does not move when the rows below come and go. Solid: Color row (swatch 28, eyedropper 28, hex), picker (area 244 x 96, hue 244 x 12, always visible), Opacity (`ValueField`, default 100 %, reset slot): **the Fill section's rows and sizes, unchanged**, block height 224px (None: 28px). Rows leave the tree on None, nothing is disabled, the colour is remembered. The block moves 36px when Fit to content enters or leaves the tree (0 and 1 object), in no other case, and never during an edit |
| Background scroll | **Revised for `0043` and `0045`:** the strip row costs +4px (28px + 8 instead of 24px + 8), Slides (60px) are off by default and the All formats row (36px) is new, so the default Document tab is about as tall as before and the scroll stays about 70px (to be measured at build). With a separate 36px strip row (option A of `0043` Question 1) it would be about 106px. The body scrolls under the fixed strip; the scroll position resets to the top on every tab change. *Earlier text, 2026-10-09:* At 800 x 600 the Solid block scrolls the panel by about 70px: title row, Color row and the area are in view (the area to within 2px of its bottom), the hue slider and Opacity are one wheel notch down; with no Fit row the hue slider is in view too. From a window about 670px high up nothing scrolls. One scroll for the whole panel, no inner scroll area, **no sticky row** (it would cost 32 of 546px to repeat a heading), `scroll-padding-block: 12px` so a focus ring is never cut, no control captures the wheel, the scroll position resets to the top when the shown section changes. The picker is not folded: one picker behaviour in the app (`0017` U1); the alternative, one disclosure arrow per picker, is an open customer question |
| Background names, tooltips | Subject line unchanged (size only). Radiogroup labelled by the title "Background", items "None", "Solid"; eyedropper "Pick background color from the drawing"; hex "Background color hex (RRGGBBAA)"; "Background saturation and value", "Background hue", "Background opacity". Tooltips (400ms, `side="left"`, text only): Paint group "Fill of the whole document. None shows a checkerboard. The background is not an object; it cannot be selected or moved."; eyedropper "Pick a color from the drawing (Esc cancels)"; Opacity as every value field. Validation chip texts as Style. Tab order after Fit to content: Paint group, eyedropper, hex, area, hue, Opacity. No shortcut |
| Background picking | The block's eyedropper is the Fill section's button; pressed (`aria-pressed`) it is the only sign of the target, as the Document section is the only panel content while picking (a selection would show Style). No extra row (it would shift the block). Cursor: the 24px eyedropper everywhere on the canvas including the pasteboard; it does not change over "no paint". **Chip** (surface and rules of the Style eyedropper): object paint: swatch 16, `#RRGGBBAA`, muted "stroke" or "fill" after it (unchanged); background: swatch 16 (stored colour at its alpha over the 4px checkerboard), the muted word "Background" (`--panel-muted-fg`, 12px), 4px, `#RRGGBBAA` (12px tabular), e.g. "Background #E8E8EBFF"; nothing: "No paint here", no swatch (pasteboard; None background with no object). About 170px at the longest. Status line after a pick: "Background color set to #RRGGBBAA" |
| Presets (quick selection, Orientation) | `0030-document-size-presets`; **since `0045` the strips are the quick selection**: one block per group that is on and has a favourite (heading = the group name from the library, cells = its favourites in list order), then Orientation. Paper A0 to A6 by default, Slides off, user groups after the built-ins. A cell is pressed only when its format is in the quick selection and matches; the subject line names the first matching format of any group that is on ("Custom" otherwise); the full list is "All formats" (row "Document formats library"). Cells of a shape format (`0046`) carry a 12px shape glyph before the name. The rest of this row is unchanged. Blocks of the same shape, 12px apart: a 16px heading line (12px semibold `--toolbar-icon`: "Paper", "Slides", "Orientation"; the group `name` from the data file), 4px, then a **preset strip** of 28px: the look of the Style section's `ToggleGroup` (one 1px `--toolbar-icon` at 60% border, `rounded-[5px]`, cells separated by 1px, 14px text). Cells `flex: 1 1 auto`, **minimum 32px**, 6px side padding, text never truncated. At 244px: Paper seven cells of about 34.9px (7 x 32 + 6 dividers = 230px, so a font difference cannot wrap it), Slides three of about 81px, Orientation two of about 122px. A group that does not fit one row wraps to a further strip row of the same look, rows spanning the full width, the last row's cells growing to fill it (nine presets give 5 + 4). A name of 24 characters (the data limit) takes a row of its own. Block height 48px (heading 16, gap 4, strip 28); more rows add 32px each. A group without presets draws nothing; with no list at all only Orientation shows. Pressed, hover and focus as the `ToggleGroup` item (`--toolbar-icon-active-bg` / `-fg`, `--editor-accent-hover`, 2px `--editor-accent` ring inside the strip); `role="radiogroup"` named by the heading (`aria-labelledby`), cells `role="radio"` with `aria-checked`, names "Paper A4", "Slides 16:9", "Orientation Portrait"; one Tab stop per group, Tab lands on the pressed cell or the first when none is pressed; Left, Right, Up, Down, Home, End move and press (one commit per step; a center-fixed resize is exact to step back). A press with the mouse returns focus to the canvas. **State is derived:** at most one preset cell is pressed over Paper and Slides, none for a custom size (no "Custom" cell; the subject line says it); Orientation presses Portrait when width < height, Landscape when width > height, neither for a square. **Orientation cells** show a 16px page glyph (1.5px stroke, `currentColor`; Portrait a 9 x 12 rounded rectangle, Landscape 12 x 9) and the name 6px right of it. Edited text in Width or Height is committed first (`0015`), then the press applies. Not in the tree while the section is not shown |
| Preset tooltips | Radix `Tooltip`, 400ms, `side="left"`, text only, also on keyboard focus. Line 1: the size **as a press would set it** (the orientation of `0030` criterion 13) in the display unit with the number rules of `0015` and U+00D7 as the status bar: "210 × 297 mm", in a landscape document "297 × 210 mm", in inches "8.2677 × 11.6929 in". A px preset names the pixels first: "1920 × 1080 px = 508 × 285.75 mm". Line 2, muted, only with a `note`: "ISO 216", "Full HD". Portrait "Taller than wide. Swaps width and height." and Landscape "Wider than tall. Swaps width and height." with the muted line "Objects keep their place relative to the center; nothing rotates." |
| Tooltips | Width and Height: "Document size. A resize keeps the center, so objects move with the document and nothing changes on screen." Text only, `side="left"` |
| Feedback | The fields, the rulers' labels and the status bar change in the frame of the commit; the artwork does not move on screen (document edges move instead); no animation, no dialog, no confirmation. One commit per operation ("Resize document", "Fit document to content", the unit change) |
| Focus | `Shift+Ctrl+F` with nothing selected expands the panel and focuses Width with its text selected. **`Shift+Ctrl+D` (2026-10-10, `0043`; Inkscape's Document Properties key) expands the panel, activates the Document tab and focuses the first control, also with a selection.** With an object selected and another tab active the section is reached by that key or by the tab; Escape clears the selection and shows it (unless History or a pinned Document tab is active) |

Options considered and not taken: a Document Properties dialog (the product's
first general dialog, hides the feedback, breaks "numbers live in the panel");
the status bar size readout as an inline editor (no room for Unit or Fit, an
invisible control, the first interactive status bar item). An aspect-ratio lock
between Width and Height is not offered (no proportional resize in the spec).

## Properties panel: tabs (`0043-properties-tabs`, 2026-10-10)

Decisions and reasoning: `specs/0043-properties-tabs/specification.md`, "UX notes". The customer asked for Blender's microtabs. The rules of the Style section (no popups, hidden not disabled, tooltips text only) apply to every tab; the one exception is recorded in the row "Hidden, not disabled".

| Item | Value |
|---|---|
| Strip | In the header row, at the left of the panel content: three tabs (Document, Style, History, in this order), each **36 x 28px**, 4px apart (116px), `rounded-[5px]`. Until `0020` ships the History tab is not built and the strip has two. The subject line fills the other 120px, right-aligned, 12px `--panel-muted-fg`, ellipsis, full text in a tooltip. Row height 28px + 8px gap |
| Orientation | Horizontal, merged into the header row (`0043` Question 1, option C). Own row above the header (option A) would add 36px; a vertical strip on the panel edge (option B) would add 36px of width (canvas 520 to 484px at 800 x 600, or content 208px, which breaks every 244px component) |
| Glyphs | Lucide, 18px, 1.5px stroke: Document `File`, Style `Droplet`, History `History`. Icon only, no visible text; a text tooltip each (Radix, 400ms, `side="left"`): "Document (Shift+Ctrl+D)", "Style (Shift+Ctrl+F)", "History (Shift+Ctrl+H)" |
| States | Unpressed: `--toolbar-icon` glyph, no ground; hover `--editor-accent-hover`; pressed (`aria-selected`): `--toolbar-icon-active-bg` ground, `--toolbar-icon-active-fg` glyph, the look of the active tool on the rail (3.3:1 against the panel), shape not colour; focus the panel ring (2px `--editor-accent`, 1px `--toolbar-bg` offset). **Dimmed Style** (nothing selected): glyph at 40 %, no hover, `aria-disabled="true"`, `tabindex="-1"`, tooltip "Style: select an object first", a press does nothing and says nothing. Exempt from the 3:1 non-text rule while dimmed |
| Semantics | `role="tablist"` named "Panel"; tabs `role="tab"`, `aria-selected`, `aria-controls`; body `role="tabpanel"` `aria-labelledby` the active tab. One Tab stop (the active tab); Left and Right move and activate (instant content), Home and End jump, a dimmed tab is skipped, no wrap. Tab order in the panel: collapse tab, strip, body. A mouse press returns focus to the canvas, a key press keeps it on the strip |
| Active tab | New session, New, Open: Document with no selection, Style with one. Empty to non-empty while Document: Style. Non-empty to empty while Style: Document. Non-empty to non-empty: unchanged. **History never switches by itself.** A manual Document with a selection stays until the selection goes empty and then non-empty. Session state only, kept across collapse and expand and resize. Tab and body change in the same frame as the selection |
| Keys | Shift+Ctrl+F: expand, Style with a selection and Document without, first control focused (also leaves History). **Shift+Ctrl+D: expand, Document, first control.** Shift+Ctrl+H: expand, History, focus on the list. All ignored during a canvas drag; Cmd on macOS |
| Body | Scrolls under the fixed strip (one scrollbar), scroll position reset on every tab change, no animation. The panel keeps 280px and 244px content; the canvas never changes width. A collapsed panel shows only the collapse tab; expanding restores the previous tab |
| Drafts | An inline form (`0045`) keeps its draft when the tab changes; Escape or Cancel drops it; New and Open drop it |
| Height at 800 x 600 | Document tab about as tall as before `0045` (+4px strip, -60px Slides off, +36px All formats); Style +4px on every figure; History: list about 340px in the Object scope, 410px in the Document scope |

## Properties panel: History tab (`0020`, `0041`, `0042`, 2026-10-10)

Decisions: the UX notes of `specs/0020-undo-redo/`, `0041-object-history/`, `0042-history-branches/`. The list is the **one inner scroll area** of the panel: with a long history a panel-wide scroll would put the Undo and Wipe controls thousands of pixels from each other.

| Item | Value |
|---|---|
| History tab layout | A column that fills the body height: header row 1 (scope group at the left, Undo and Redo at the right), Mode row and caption (Object scope only), the list (flex, scrolls), the footer (a muted 12px line "The history is saved in the project file.", then the Wipe button). Header rows and footer never scroll. Minimum list height 120px; below that the body scrolls as a whole |
| Header row 1 | 28px. Scope group (`ToggleGroup`, text items "Document" 76px and "Object" 56px, `role="radiogroup"` "History scope") in the tree only with exactly one object selected; its slot stays empty otherwise. **Undo and Redo**: two 28 x 28 icon buttons (Lucide `Undo2`, `Redo2`, 16px) in two reserved 28px slots at the right edge; a button that cannot apply is not in the tree, its slot stays. Tooltips "Undo: Move, You, 12:03 (Ctrl+Z)" (Object scope: "(Ctrl+U)"), "Redo: Move (Ctrl+Shift+Z)". Scope default and stickiness: `0041` notes |
| Mode row | Object scope only. Label "Mode" in the label column, `ToggleGroup` "Preview" and "Live", 88px each. Caption slot 32px below it (two 12px muted lines): Preview "A click shows the version. Nothing changes." Live "A click puts the object into that version. Ctrl+Z undoes it."; during an exploration the slot holds the 244 x 28 outline button "Restore previous state" |
| Row | 44px high (row "List row"). Gutter (lanes): 16px + 12px per extra lane, at most 3 branch lanes. Glyph 16px at the gutter's right. Line 1 (20px): operation name 14px, ellipsis; step id 12px monospace `--panel-muted-fg` at the right, never truncated, selectable. Line 2 (16px): 12px `--panel-muted-fg`: status word (semibold; only "Undone", "Undone by <name>", "Branch", or "from 3f9a1c" for an inherited row), summary, author ("You", "Earlier"), clock time ("12:03"; "9 Oct 12:03" before today). Row-action slot 28px at the right end of line 2: a 24px icon button, shown on hover, focus within and on the selected row |
| Kind glyphs | Create `Plus`, Delete `Minus`, Replace `Replace`, Change `Pencil`, Document `File`, object undo `Undo2`, object redo `Redo2`, clone start `Copy`, go to version `Locate`, wipe `Eraser`. Five kinds, not one glyph per operation |
| Pinned and special rows | **Now**: 32px, first, pinned, not a step, filled disc, id of the newest applied step; selected when nothing is previewed. **Branch header**: 24px, "Branch from a1b2c3d, 2 steps", 12px semibold muted, chevron (fold). **N more branches**: 28px, beyond three branch lanes. **Objects sub-row** (Document scope disclosure): 28px, at most 20 then "and N more". **Floor**: "History wiped" at the end of a wiped object timeline |
| Lanes | Pitch 12px, main lane at x = 8, branches at 20, 32, 44. Main lane 2px `--toolbar-icon`; branch lanes 1.5px `--lane-line`; undone segment dotted; inherited segment dashed. Marks 8px: applied filled disc, undone and on-branch ring (1.5px). Fork: a horizontal stub with a 4px corner radius from the main lane to the branch lane, then up the branch lane to its newest row; no crossings. Each virtualised row draws its own slice (inline SVG) |
| Statuses | Applied (no word), Undone (ring, dotted, "Undone"), On a branch (ring in a branch lane, "Branch", branch header). No "Wiped" state: wiped rows leave the list. Never opacity or colour alone; muted ink is `--panel-muted-fg` (5.0:1) |
| Preview and exploration | Pressing a row (Enter, Space) previews in Preview mode and goes to the version in Live mode. While a preview shows, Up and Down move the preview along with the focus; without one they only move the focus. Escape ends a preview, a second Escape returns focus to the canvas. The pinned Now row ends it. Canvas look: "A preview is never a commit" |
| Row actions | Object scope: `CopyPlus` "Clone this version as a new object". Document scope: chevron "Objects of this step" (`aria-expanded`) expanding sub-rows each with `CopyPlus`; none for a step with no object or for a group |
| Footer and Wipe | Footer 12px padding, the saved-in-file line 16px, 8px, then the Wipe button 244 x 28 (outline, label `--destructive`): "Wipe history" in the Document scope, "Wipe object history" in the Object scope. Pressing it opens an **Inline confirm** in its place, growing upward. Not in the tree when there is nothing to wipe; in a shared document a muted line "Wiping is not available for shared documents yet." takes its place |
| Empty | "No steps yet." 14px and a muted "Steps appear here as you edit."; the Now row is still pinned; Wipe not in the tree. An object whose timeline is not in the file: "No steps recorded for this object." |
| Semantics | List `role="list"` "History", rows `role="listitem"` holding a button (`aria-posinset`, `aria-setsize`, selected row `aria-current="true"`), one Tab stop with roving focus. Name: "Move, 3 paths, You, 12:03, id a1b2c3d, undone, on branch from a1b2c3d". No live region for the list; the canvas notice is the only announcement |
| Scale | The scrollbar is exact from the first frame (row heights known); the first 50 rows at once; a placeholder row (two muted bars, no spinner) for rows not yet delivered; a new step at the top keeps the scroll position of a scrolled list; the scroll position is remembered per scope |
| Hover link | A hovered or focused row shows the `0014` hover box on its existing touched objects and removes it in the frame the pointer or focus leaves; a deleted object shows nothing |
| Notices | Canvas notice slot for keys; 4px under the button for a panel button. Texts: the tables in `0020`, `0041`, `0042` UX notes |
| Edit menu | Native: Undo <name> (Ctrl+Z), Redo <name> (Ctrl+Shift+Z), separator, Undo Step of Selected Object (Ctrl+U), Redo Step of Selected Object (Ctrl+Shift+U), separator, Select All (Ctrl+A). Greyed when not applicable; Ctrl+Y is a hidden alias |

## Document formats library (`0045`, `0046`, 2026-10-10)

Decisions: `specs/0045-document-formats-library/specification.md`, "UX notes". Inside the Document tab; no popup.

| Item | Value |
|---|---|
| All formats | Disclosure row after Orientation: 244 x 28, chevron 12px, label 14px, muted count at the right, `aria-expanded`; collapsed at every start; expanding scrolls the row to the top of the body. The list is in the flow of the body scroll |
| Group header | 28px: fold chevron button, name 14px semibold (ellipsis, tooltip), muted count ("Off" when the group is off), edit and delete icon buttons (24px, user groups only), the **Show** `Switch` (label "Show", name "Show group <name>"). Off group: header only. Fold state kept for the session |
| Format row | 28px: star toggle 28 x 28 (`Star` 16px, outline or filled `--toolbar-icon`, `aria-pressed`, constant name "Quick selection: A4"), apply button (name 14px + size 12px tabular muted right-aligned, "210 × 297 mm"), edit and delete 24px icon buttons for user formats (52px reserved). Matching format: the selected row look |
| Add and Edit form | A block with a 1px `--toolbar-icon` at 25 % border, `rounded-[5px]`, 8px padding. Rows (label 60, control 158): Name; Width and Height (text fields, unit suffix inside); Unit (`ToggleGroup` mm, cm, in, px, 36px items); Group (16px heading, wrapped radio strip of group names plus "New group..."); with a new group "Group name" and "Opens as"; "Add to quick selection" (`Switch`); **Add** or **Save** (primary button) and **Cancel**, 109px each. Opens prefilled, focus in Name, scrolled into view; about 290px high. One validation chip at a time (the first invalid field, focused); texts in the spec |
| Primary button | `--toolbar-icon-active-bg` ground, `--toolbar-icon-active-fg` label, 28px, `rounded-[5px]`; only in inline forms. Cancel and all other buttons are the outline button of "Fit to content" |
| Delete | Inline confirm in place of the row: "Delete Key ring?" or "Delete group Laser and its 2 formats?" |
| Import, Export | Two full-width 28px outline buttons under Add format, 8px apart: "Import formats", "Export my formats" (not in the tree without a user format). OS file dialog only. Notices 4px under the button |
| Broken file | Block at the top of the format area: 1px `--field-invalid` border, alert glyph, `--field-invalid` 12px text, outline button "Set file aside". Stars, edit, delete, Add, Import, Export and the Show switches are not in the tree while it shows |
| Keyboard | The list body is one Tab stop with roving focus; Up and Down between headers and rows, Left and Right between a row's controls, Enter or Space presses (the switch: Space). Add format, Import and Export are separate stops |
| Shape formats (`0046`) | 16px shape glyphs (`Square`, `SquareRoundCorner`, `Circle`, `Spline`); a row and a strip cell show a 12px glyph before a non-rectangular format; a circle reads "⌀ 130 mm"; the Add form gets a glyph-only Shape group after Unit; "Save as format" shares the Fit-to-content row (two 118px buttons) |

## Keyboard shortcuts established so far

| Action | Shortcut | Notes |
|---|---|---|
| New / Open / Save / Save As | Ctrl/Cmd+N/O/S/Shift+S | `project-file-foundation`, native menu accelerators |
| Select tool | `S`, `Escape` | `S` matches Inkscape's Selector key (`canvas-navigation-and-selection`); launch default, replacing Pen. In the Select tool with an object selected `S` opens the size entry instead (`edit-interaction-polish`, built in PR 1). `Escape` reaches the Select tool as the last step of its cascade. Rail tooltip "Select tool (S or Esc)" |
| Pen tool | `B` | Matches Inkscape's Bezier/pen tool key |
| Node tool | `N` | Matches Inkscape |
| Rectangle tool | `R` | Matches Inkscape (`primitive-shapes`). In the Select tool with an object selected `R` opens the rotate entry instead; Escape, then `R`, picks the tool (`edit-interaction-polish`, built in PR 1; customer confirmed 2026-10-06). Rail tooltip "Rectangle tool (R)", and "Rectangle tool (Esc, R)" while the Select tool has a selection |
| Ellipse tool | `E` | Matches Inkscape (`primitive-shapes`) |
| Polygon/star tool | `*` | Matches Inkscape (`primitive-shapes`); not a letter, kept anyway for the same parity reason as the others |
| Finish path | Enter (or double-click) | Matches Inkscape |
| Escape cascade | `Escape` | One step per press, repeat ignored: close an open chip or restore a bar field; else cancel a drag (the button may still be down; the release then writes nothing); else clear the tool's state (Pen path discarded, Node selection cleared, Select selection cleared); else switch to the Select tool (`edit-interaction-polish` Part D, built in PR 1). Pen, Rectangle, Ellipse and Polygon/Star: first Escape after a drag or with none leaves the tool. Node: clear nodes, then Select (the path stays selected there), then a third Escape clears it. Select with nothing selected: nothing. The Pen path is not kept and nothing is written, so Ctrl+Z has nothing to take back for it |
| Delete selected node(s) | Delete or Backspace | Both bound; macOS keyboards label the backspace key "delete". Gated like every letter (no delete during a drag, with a chip open or in the Pen) |
| Bend a segment | drag a segment in the Node tool; hold Shift to limit the drag to one axis | `0031-segment-drag-bending`; pointer only (a keyboard bend is proposed with the nudge story). No Alt or Ctrl variant: Alt is avoided on Linux (window managers take Alt+drag, `0014` plan.md), Ctrl is free. Escape cancels the drag, the first step of the cascade. The Shift release is not delivered on WebKitGTK and takes effect at the next pointer move |
| Toggle "Scale stroke width" | none; Tab to the switch, Space | `object-transform`; Select tool's bar, tool state, off per session, never saved; no letter shortcut until usage shows one is needed |
| Toggle "Link corners" | none; Tab to the toggle, Space or Enter. Shift while pressing a radius handle inverts it for that drag or entry | `rectangle-corner-radii`; session state, linked by default, reset by New and Open, never saved; no letter shortcut |
| Reveal the four side rotate handles | hold Shift (Select tool, one or more objects selected, no drag running) | `object-transform-refinements`; also Shift = pivot on the opposite point while rotating, resize/skew about the center |
| Type an exact angle or size | double-click a rotate or resize handle | `object-transform-refinements`; Enter commits, Escape cancels, Tab moves between W and H. The keyboard route is `R` and `S` (rows below); a Properties-panel transform form is a follow-up |
| Snap a rotation or skew to 15° and 22.5° stops | hold Ctrl while dragging | `object-transform-refinements`; with a resize, Ctrl keeps proportions. Since `edit-interaction-polish` (PR 1) Ctrl also snaps the create-drag of a polygon or star and, for these two kinds, the rotate drag snaps the shown (absolute) angle, not the turn since the press. Stops: 0, 15, 22.5, 30, 45, 60, 67.5, 75, 90 and the mirrored values; 180 and -180 are one stop, shown as 180 |
| Type a radius or an inner ratio | double-click the radius handle or the star's inner handle (Select tool) | `unified-object-editing`; same chip rules as the size entry. The bar's "Radius", "Points" and "Ratio" fields are the keyboard route |
| Object to path | none; the "Object to path" button of the Select bar | `unified-object-editing`; no shortcut assigned (Shift+Ctrl+C opens the inspector in a browser build); Ctrl+Z takes it back (`0020`) |
| Boolean operations (Union, Difference, Intersection, Exclusion, Reverse difference) | none; the Boolean toolbox of the tool rail (Tab to the toolbox, Up and Down inside it, Space or Enter; row "Rail keyboard model") | `0016-boolean-operations`; no shortcut and no menu entry assigned. Ctrl+Z takes any of them back (`0020`), so the reason to wait is gone; a key is a follow-up decision (`0020` Question 10; Ctrl+Plus and Ctrl+Minus zoom a browser page) |
| Combine, Break apart | none; the Path toolbox of the tool rail (Tab to the toolbox, Up and Down inside it, Space or Enter) | `0035`; no shortcut and no menu entry assigned, as the Boolean operations |
| Close path (Sharp, Smooth) | none; the two buttons at the end of the Node bar (Tab, Space or Enter) | `0034`; Node tool only; no shortcut assigned |
| Pen: the alternative at a target | hold Shift over an end node (start a new path), over another path's end node (place a node) or over the close target (flip sharp and smooth); read live, also with the pointer at rest | `0034`; the chip names it. On WebKitGTK the key-up is delivered with the next pointer event. Alt and Ctrl have no meaning at a target |
| Pick a document size | none; Tab to the Paper, Slides or Orientation group in the Document section, arrows, Home and End; `Shift+Ctrl+F` focuses Width | `0030`; each arrow step is one commit |
| Type a move | `M` (Select tool, one or more objects selected), or double-click the drawn center handle | Built in PR 3. `edit-interaction-polish`; X, Tab, Y, Enter is relative; Tab to the mode control, Space, Enter is absolute (top-left of the object's, or for several objects the selection's, tight outline bounds, document origin top-left, Y down); works for any object size (the chip opens where the center handle would be). The Copy check is the last Tab stop: off on every open, on when Ctrl is held at the second press of the double-click (the key M cannot carry a Ctrl); checked at Enter it makes one copy displaced as typed, leaves the original and selects the copy. Nothing selected, or outside the Select tool: hint "Select an object first" for 2s |
| Type an angle | `R` (Select tool, one or more objects selected), or double-click a rotate handle | Built in PR 1. Opens as the top-right corner rotate handle's chip, placed there even when the handle is not drawn (small object) or hidden; for a polygon or star the prefill is its real orientation, and 0 stands the first vertex to the right. Several objects selected (`multi-object-transform`): the angle chip is relative, label "Δ", prefilled 0, turning about the group box center |
| Type a size | `S` (Select tool, one or more objects selected), or double-click a resize handle | Built in PR 1. Opens as a size chip by the box center (W and H, or "r" for a polygon or star), for any object size, and scales about the box center (as a Shift drag would; customer, 2026-10-07): the chip is placed as the move chip is (16px right of and below the center), no handle takes its dragging look, the pivot marker shows at the center at full `--accent`, and the fields are independent (Ctrl+S is gated). A double-click on a resize handle keeps the drag's fixed point. Several objects (`multi-object-transform`): W and H of the group box, about its center; the fields are independent for every selection; a typed stretch converts and shows the note of the row "Group entry chips" |
| Type a skew | `K` (skew x) and `Shift+K` (skew y), one path or a selection of paths only, or double-click a skew handle | Built in PR 3. Fixed line is the opposite side's, never the Shift pivot (a double-click with Shift at the second press uses the center line). The chip opens at the skew handle's position also where the handle is not drawn. A selection that holds a non-path: hint "Skew works on paths only" for 2s |
| Copy while moving | hold Ctrl (before the press or during the drag), release the pointer with it down | Plus badge, "Copy" in the readout; Ctrl released before the release makes it a move again |
| Keep a move on one axis | hold Shift (before the press or during the drag) | Axis re-chosen on every pointer event; origin axes shown; a Shift-click without movement still toggles the selection |
| Pan | hold `Space` and drag | Never interrupts the running operation |
| Undo | `Ctrl+Z` (`Cmd+Z`) | `0020`; chord class of the key concept; acts on every repeat event; ignored during a drag, with a chip open, in a typing field, during a long operation, with an unfinished Pen path (hint "Finish the path (Enter) or cancel it (Esc) first."). Notice "Undid: Move (3 paths)." in the canvas notice slot. Native Edit menu item "Undo <name>" |
| Redo | `Ctrl+Shift+Z` (`Cmd+Shift+Z`); `Ctrl+Y` on Linux and Windows as a hidden alias | `0020`; as Undo. The tooltips and the menu name Ctrl+Shift+Z only |
| Undo or redo one object's step | `Ctrl+U` and `Ctrl+Shift+U` (`Cmd` on macOS), Select or Node tool, exactly one object selected | `0041`; gated as Undo; "Select one object." otherwise. Edit menu items "Undo Step of Selected Object" and "Redo Step of Selected Object" are the fallback if a platform eats Ctrl+Shift+U |
| Select all | `Ctrl+A` (`Cmd+A`), Select tool | Built in `0044`; chord, once per press; replaces the selection; ignored in other tools without `preventDefault`; a typing field keeps its own select-all. A hidden live region says "Selected 1 object."; for two or more the group announcement of `0019` speaks ("5 objects selected, 46.2 by 18.7 mm"). Edit menu "Select All" comes with the native menu of `0020` |
| Nudge | Arrow = 1 mm, `Shift+Arrow` = 10 mm in document millimetres (Select tool, selection, canvas focused) | Built in `0044`; every repeat event moves once more and is one `translate_objects` commit (a held run becomes one step when `0020` exists); Ctrl, Cmd, Alt: ignored; move readout "Δ 3.0, 0.0 mm" beside the selection box, 800 ms after the last move; a hidden live region says "Moved 11 mm right." when the step ends; past the coordinate limit the key hint chip says "Too far from the document. Nothing was changed." until the canvas notice slot of `0020` exists; page never scrolls on a handled key |
| Show the History tab | `Shift+Ctrl+H` (`Cmd`) | `0043`, `0020`; Inkscape's Undo History key. Expands the panel, activates History, focus on the list. Ignored during a drag |
| Show the Document tab | `Shift+Ctrl+D` (`Cmd`) | `0043`; Inkscape's Document Properties key. Expands the panel, activates Document, focus on its first control, also with a selection |
| Switch tab | Left and Right on the focused strip | `0043`; activates at once; Home and End; a dimmed Style tab is skipped |
| Keyboard help | not built; proposal `?` | Separate story |
| Any letter shortcut | ignored while a drag, a chip, a focused control or an unfinished Pen path is active, with Ctrl, Cmd or Alt down, and on key repeat | Not a per-shortcut rule; see "Keyboard concept". Built in PR 1. An ignored key gives no feedback (no hint, no `preventDefault`). A tool switched by clicking a rail button leaves focus on that button, so letters are ignored until the canvas is clicked or tabbed to (existing behaviour, not changed by PR 1) |
| Open/focus the style panel | Shift+Ctrl+F (Cmd on macOS); since `0043` it also activates Style (or Document with nothing selected) when the History tab is active | Matches Inkscape's Fill & Stroke binding; app-global, not canvas-focus-scoped (`stroke-and-fill-styling`). Expands the panel if collapsed and moves focus to its first control (Stroke Paint; with nothing selected, the Width field of the Document section with its text selected; `document-size-and-rulers`), or to the panel itself when it is empty (`style-panel-rework`); never closes it; ignored while a canvas drag runs. Collapse and expand by the panel's edge tab. In the panel: Escape ends eyedropper picking, else reverts a running drag, else restores an edited field and returns focus to the canvas, and never clears the selection; Enter commits a field and returns to the canvas, Tab commits and moves on; Delete and Backspace never reach the canvas. See "Properties panel: Style section" |
| Change a panel value field | drag in the field; or focus it and use arrows (Shift = 10x, Ctrl/Cmd = 1/10), Home, End; click, Enter, F2 or a digit to type | `style-panel-rework`; preview per frame, one commit on release or key-up; Escape reverts a drag |
| Reset a panel value field | `Ctrl+Backspace` (`Cmd+Backspace` on macOS) on the focused field, or its reset icon | `style-panel-rework`; Width 0.25 mm, Opacity 100 %, Count 1. Bare Backspace and Delete do nothing |
| Set the document background | none; Tab to the Background Paint group in the Document section (last block), arrows, then the Color row | `0040`; no shortcut (once per project). The block's eyedropper also picks the background itself |
| Pick a colour from the drawing | the eyedropper button next to the swatch; no key | `style-panel-rework`; Escape or a right-click cancels; pointer-only in this slice |

Single-letter shortcuts (tools and, since `edit-interaction-polish`, the selection entries M, R, S, K) are a different category from the File menu's
native accelerators (table above) — they're bound at canvas-focus scope, the
same convention Inkscape/Illustrator/Affinity use, not native menu items.
The style-panel shortcut is a third category: app-global (works regardless
of canvas focus), since the panel it targets isn't canvas content.
