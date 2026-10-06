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
| `--shape-handle-fill` | `#FFFFFF` (idle) / `--accent-hover` (hover) / `--accent` (being dragged) | **Parameter handle** ground (`unified-object-editing`; the name stays from `primitive-shapes`, where it was the shape tools' handle fill). See "Parameter handle" below |
| `--shape-handle-stroke` | `--accent` | Parameter handle ring and centre dot, and the 1px selection box of a selected object |
| `--shape-handle-guide` | `--accent` at 60%, dashed (was `--accent-hover`, 2026-10-06 review: at 20% it measured about 1.1:1 and was invisible on the canvas, the same finding as the skew guide) | Corner-radius guide from the corner to a hovered or dragged radius handle — dashed, to read as distinct from the solid Bézier handle line above |
| `--preview-new` | `--accent` (`#2F6FEE`) | The "new" half of blue-new/black-old (`unified-object-editing`): the hollow outline of the geometry a release would commit. An alias, not a second blue; named so the preview can be re-coloured without touching selection. The "old" half has no token: it is the committed object in its own style |
| `--panel-bg` | `--toolbar-bg` (`#DCDCE0`) | `PropertiesPanel` (formerly `StylePanel`) and any later section it hosts — reuses the one chrome color rather than adding a second (`stroke-and-fill-styling`) |
| `--panel-elevation-shadow` | `0 2px 8px rgba(0,0,0,0.24)` | Drop shadow on every floating chrome surface introduced 2026-10-05: the left tool panel and the per-selection contextual mini-toolbar — what makes them read as "floating over" the canvas rather than framing it. The right Properties panel is docked, not floating, and does not use this token. |
| `--marquee-touch` | `#2FAE57` | New semantic color (`advanced-selection`), not a reuse of `--accent`: the marquee box's border and fill, and the lasso line, whenever the active mode is "touch" (crosses or fully contains selects it) — green, matching the customer's own naming. Deliberately distinct from `--accent` because this is a transient drag-mode indicator, not a selection state; don't read it as "selected." |
| `--marquee-touch-fill` | `--marquee-touch` at 12% opacity | Marquee box interior fill in touch mode — low-opacity so canvas content underneath stays legible while the box is open, matching LightBurn's own semi-transparent-fill-plus-solid-border convention |
| `--marquee-contain` | `#E5484D` | Marquee box border/fill color whenever the active mode is "contain" (fully-inside-only selects it) — red, matching the customer's own naming |
| `--marquee-contain-fill` | `--marquee-contain` at 12% opacity | Marquee box interior fill in contain mode, same reasoning as `--marquee-touch-fill` |
| `--marquee-legend-bg` | `--toolbar-bg` (`#DCDCE0`) | Background of the small on-canvas modifier-state legend shown during a marquee/lasso drag (`advanced-selection`) — reuses the existing chrome tone rather than inventing a new surface color |
| `--marquee-legend-fg` | `--toolbar-icon` (`#3A3A3F`) | Legend text color, same pairing as the toolbar's own icon-on-chrome contrast |
| `--field-invalid` | `#B3261E` | Border of a text field whose content was refused (`object-transform-refinements` numeric entry chip). 4.8:1 on `--toolbar-bg`. Always paired with a message line and `aria-invalid`, never the only cue. Not `--marquee-contain` (`#E5484D`): that is a drag-mode color and is only 2.9:1 on the chrome tone |

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
| Shape handle | **Superseded 2026-10-06 (`unified-object-editing`): the shape tools no longer have handles; resize is the Transform resize handle, radius and inner radius are the Parameter handle below.** Former value: 8×8px screen-space | Hollow square, `primitive-shapes`: bounding-box resize, rectangle corner-radius, polygon/star inner-radius — deliberately square (never circular or diamond) so the two vocabularies never read as the same control, regardless of either one's size; no longer the larger of the two since the handle endpoint's 2026-10-05 doubling (12px), and now the *smaller* of the two since the node glyph's own 2026-10-05 doubling (14px) — this token's own size was not revisited either time |
| Shape handle hit-test radius | **Superseded 2026-10-06** (see "Parameter handle hit-test radius"). Former value: 16px screen-space | Same margin rule as the node radius (reused rather than invented fresh — `Session::shape_tolerances()` passes `self.point_tolerance()` straight through) — doubled from 8px as a direct, automatic consequence of the node hit-test radius doubling above, not a deliberate shape-handle-specific change. Now equal to the handle hit-test radius too (same coincidence as above) |
| Transform resize handle | 8×8px screen-space, 2px corner radius ("squircle") | Hollow `--accent` stroke / white fill idle, solid `--accent` fill while dragging — `object-transform`'s 8 Select-tool scale handles (4 corner + 4 edge-midpoint). Same footprint as the former `primitive-shapes` shape handle on purpose (criterion 1 of that spec pinned the hit size). Since `unified-object-editing` the parameter handle is on screen together with it, and differs by silhouette: a round knob (see "Parameter handle"), not a square |
| Transform resize handle hit-test radius | 16px screen-space | Resolved by `object-transform`'s own implementation (this row was flagged, not fixed, until now): criterion 1 pins this to "the same hit size as `primitive-shapes`' own shape handles, whatever that is at build time" — the shape handle's hit radius doubled to 16px in the 2026-10-05 sizing round (row above), so the transform resize handle takes that same 16px, not the stale 8px this row said when the spec was first written |
| Transform rotate handle | 12×12px screen-space, circular-arrow icon glyph (not a dot): a 270° arc at 90% of the footprint, 2px stroke, with an arrowhead | `--accent` stroke / transparent fill idle, `--accent-hover` fill on hover, solid `--accent` fill with white glyph while dragging (`object-transform`). Deliberately an icon rather than a plain circle, and not connected to the bounding box by a stalk line, so it never reads as a reuse of the Bézier handle's line-plus-circle composition (`path-node-editing`). Since `object-transform-refinements` there are up to eight, all identical and never rotated: four at the corners (always), four at the side midpoints (only while Shift is held, see "Transform handle layout") |
| Transform rotate handle offset | 32px screen-space, center to center | Distance from a corner resize handle to its corner rotate handle, measured along the box's own outward diagonal (22.6px on each local axis), and from a side midpoint to its side rotate handle along the outward normal. All measured in the box's own frame, so they rotate with it. Corrected 2026-10-06 (`object-transform` UX review): the original 20px was chosen against an 8px resize hit radius; with the resize radius now 16px (16 + 12 > 20) the two hit areas overlapped. 32px is the sum of the two 16px hit radii, so they touch but never overlap. (The old single rotate handle sat 32px above the top edge midpoint; that position is now the Shift-only top side handle.) |
| Transform rotate handle hit-test radius | 16px screen-space | Raised from 12px in the same review so the rotate handle is as easy to hit as the resize handles (its 12px glyph is the smaller target, a larger radius makes up for it); equal to the resize radius, which is what lets the 32px offset above keep the two areas apart |
| Transform handle hit priority | — | A resize handle's hit radius shrinks to a third of the box's smaller side (never below a quarter of 16px), and a press *inside* the box only grabs a handle within 6/16 of that (scaled) radius of the box's edge — 6px at the full 16px radius, less on a small box; outside the box the handle always wins (`SelectTool::handle_at`). A press inside the sole selected object's box that is not on a handle (resize or rotate) starts a **move**, so a small unfilled object — whose outline the handle radii tile completely — can still be moved; an *unselected* object still hits only on its outline, and a press on empty canvas outside the selected box still deselects (`object-transform` `adrs.md`, 2026-10-06). Below a 24px box side (3 × the 8px glyph) only the four corner handles are drawn, so the glyphs do not merge; hit-testing is unchanged. **2026-10-06 (`object-transform-refinements`):** with eight rotate handles, skew handles and a center handle the order "rotate first, then resize" is replaced by one nearest-center pass over every visible handle (caps and tie order in "Transform handle layout" below); the inner edge band above is unchanged, and no new handle has a hit area inside the box. **2026-10-06 (`unified-object-editing`):** parameter handles join the same pass at rank 0 (they win an exact tie, then resize, skew, rotate), with their own 12px cap and no inner band; they are the one handle family with a hit area inside the box, and exist only from a box side of 72px (see "Parameter handle layout") |
| Live transform readout | Pointer-anchored chip, 12px up and to the right of the pointer, `--toolbar-bg` / `--toolbar-icon`, 12px text | `object-transform` scale ("W × H mm", polygon/star "r R mm") and rotate ("37.4°", "45°" under Ctrl) readouts, and (`object-transform-refinements`) the skew readout "Skew x +12.5°" / "Skew y −8.0°" (x for top/bottom handles, y for left/right, real minus sign, one decimal, also under Ctrl: "22.5°"; the rotate readout likewise shows "22.5°" under Ctrl), and (`unified-object-editing`) the parameter readouts "r 3.5 mm" (rectangle corner radius; "r 12.0 mm max" when an unlinked drag is stopped by its limit, `rectangle-corner-radii`) and "ratio 0.45" (star). The offset is shared by every tool's live readout (rectangle/ellipse/polygon create drags moved from 8px to 12px in the same change). Stays fully inside the canvas: flips to the left of the pointer when it would cross the right edge and below it when it would cross the top edge, then clamps (`frontend/src/lib/readoutPlacement.ts`) |
| Transform pivot marker | 6px diameter screen-space, `--accent` at 60% opacity | Shown at the active scale/rotate pivot point for the duration of a drag only (`object-transform`) — its position is the feedback for Shift's pivot-swap modifier; see that spec's UX notes. A handle sitting exactly on the pivot is not drawn while the marker is shown, so the dot never lands on a handle that then looks pressed |
| Transform center move handle | 16×16px screen-space rounded square (3px radius), white fill, 1px `--accent` outline, four-way arrow glyph 10px wide (1.5px stroke, `--accent`) | `object-transform-refinements` item 1. Hover: `--accent-hover` over the white fill. Own move drag: solid `--accent` with a white glyph. Does not rotate with the box (a move runs along the screen axes). Drawn only for a single selected object whose shorter side is 48px or more, and not while a resize, rotate or skew drag or a numeric entry is active (the pivot marker lives at the center then). It is a *visible name* for "press inside the box moves": no hit area of its own, no priority against the other handles; it only gives hover feedback and the built-in `move` cursor, within min(12px, shorter side / 4) of the center. 48px (not 40): at that size the hover region is still 12px and the glyph keeps 12px clear of every resize handle |
| Transform skew handle | 18×12px screen-space footprint: two opposed parallel arrows (1.5px stroke, 3px heads) along the side | `object-transform-refinements` item 8, paths only. `--accent` stroke / transparent ground idle, `--accent-hover` rounded-rect ground (3px radius) on hover, solid `--accent` ground with white arrows while dragging. Rotates with the box: top and bottom handles point along the box's `u` axis, left and right along `v`. Two opposed arrows rather than one double arrow so it cannot be mistaken for a resize handle |
| Transform skew handle offset | 16px screen-space, center to center | From the side midpoint along the outward normal. Between the edge resize handle (0) and the Shift-only side rotate handle (32): glyph clearance 6px and 4px |
| Transform skew handle hit-test radius | 12px screen-space | 24px target. Overlaps its two neighbours on the same side on purpose; nearest center wins (see "Transform handle layout") |
| Transform handle layout | — | **One box, all in its own rotated frame.** Corner resize: at the corner. Edge resize: at the edge midpoint, `s` >= 24, not polygon/star (slice 5). Skew: path only, 16px outward from the side midpoint, shown per axis only when the box dimension across it is >= 24px (top/bottom arrows need the height, left/right the width: that is the lever of `atan(d / h)`). Corner rotate: 32px outward on the diagonal, always. Side rotate: 32px outward from the midpoint, only while Shift is held and no drag is running, all kinds. Center move: see its row, `s` >= 48. `s` = shorter side of the box on screen. **Hit rule:** one test serves press, hover and cursor; every visible handle competes by distance from the pointer to its center within its own cap (resize `min(16, s/3)` and at least 4, rotate 16, skew 12); ties resize, then skew, then rotate. Resize and rotate regions never overlap. Skew overlaps resize and side rotate; nearest center splits it: resize keeps 8px outward of the edge (plus slice 5's inner edge band), skew 8 to 24px (28 without Shift), side rotate from 24px. **Rules:** no handle added since slice 5 has a hit area inside the box, so a small box can always be moved by pressing it; Shift adds handles on the outermost ring and moves none; rotate handles are never hidden by size (they sit outside, and a tiny part still has to turn); clearance between any two glyphs is at least 4px at every size. Not hidden by size otherwise: slice 5's resize rules are unchanged. Off-screen handles are not pulled into the viewport. **Size tiers with parameter handles (`unified-object-editing`):** `s` < 24 corner resize + corner rotate; 24 to 47 + edge resize; 48 to 71 + center; >= 72 + parameter handles. Hidden first when a box shrinks: parameter handles, then center, then edge resize; corner resize and corner rotate never. Clearance of 4px between any two drawn glyphs holds at every `s` >= 72 and every radius, except that the center glyph is not drawn while a parameter handle center is within 20px of it |
| Transform skew fixed-line guide | 1px dashed (4px on / 3px off, as the lasso), `--accent` at 100% (new token `--transform-guide`; **not** `--shape-handle-guide`) | Drawn during a skew drag along the line that stays put (the fixed edge, or the line through the box center under Shift), extended 16px past each end of the box. The pivot marker alone cannot say which line holds still in a shear. Revised after the UI review (2026-10-06): at `--accent-hover` (20%) the guide measured about 1.2:1 against the canvas and was effectively invisible; the fixed edge also coincides with the selection box edge, so only the extension past the box ends and the Shift center line can show it, and both must read. Full `--accent` is 3.9:1 on `--canvas-bg` |
| Transform entry chip | DOM overlay, `--toolbar-bg`, `--panel-elevation-shadow`, 8px radius; fields 28px high, white ground, 14px tabular text; z above the Select tool's bar | `object-transform-refinements` items 4, 5. Angle: one 80px field, fixed `°` suffix. Size: 6px padding, fields 100px wide 4px apart (84 overflowed: the "mm" suffix overlapped the last digit and a value of 1000 mm or more did not fit; the field keeps 32px of right padding for the suffix and 20px of left padding for the label), visible labels "W" and "H" (polygon/star "r", accessible name "Outer radius" since `unified-object-editing`, was "Radius"), fixed unit suffix, 12px chain glyph between W and H when linked. Placed outward of the handle (center-to-handle direction), 10px clear of its glyph edge, upright (never rotated with the object), clamped in the canvas with the readout's flip rule, follows the handle on zoom and pan. Open field: 2px `--editor-accent` border; other field 1px `--toolbar-icon` at 60%; invalid: 2px `--field-invalid` plus a 12px message line under the fields. The handle it belongs to stays in its dragging look while the chip is open. See "Numeric entry chip" below |
| Transform handle hint chip | DOM, text only, `--toolbar-bg` / `--toolbar-icon`, 12px, 8px padding, 8px radius, `pointer-events: none` | Appears 600ms after the pointer rests on a transform handle, anchored once 12px up and right of the pointer (readout placement), gone on press, leave or any key. One line per modifier (Shift, Ctrl, double-click). Parameter handles (`unified-object-editing`): "Corner radius" / "Double-click: type a value"; star "Inner radius" / "Double-click: type a ratio". The same chip, with 3 lines and a 3s life, answers a double-click on a primitive's body: see "Edit hint chip" below. Not a tooltip of the Radix kind: it is positioned over WebGL content and carries no focusable target |
| Parameter handle | 10px circle screen-space: 1.5px `--shape-handle-stroke` ring, `--shape-handle-fill` ground, 4px `--accent` centre dot | `unified-object-editing`: the knob for a primitive's own parameter: the four rectangle corner radii, a star's inner radius, later an ellipse's arc and curve handles. Idle: white ground. Hover: `--accent-hover` ground. Dragging, or its entry chip open: solid `--accent` ground, white dot. **Followers:** while a radius drag moves all four corners, the other three take the hover ground. Drawn above every other handle. Round with a centre dot is the one silhouette no other canvas glyph has (resize and center are squares, rotate an arc arrow, skew two arrows), so the two vocabularies read apart by shape, not only by position. Not rotated (a circle). Cursor: the built-in `pointer`, hover and drag, never changed by modifiers |
| Parameter handle hit-test radius | 12px screen-space | 24px target, same as the skew handle; not the 16px of resize and rotate because these handles are inside the box and four at 16px would cover most of a 72px box. Never shrinks: the handles exist only from a box side of 72px. Rank 0 in ties. Overlaps its neighbour at the largest radius (14px apart) by design; nearest center decides |
| Parameter handle layout | Threshold `T` = 72px (shorter box side `s`, screen). Rectangle radius handle: on the corner's inward diagonal at `p = 15 + ρ·L(s)` from the corner, `ρ` = effective radius / (`s`/2), `L(s)` = `(s − 14)/√2 − 15`. Drag: radius change = pointer displacement along that diagonal × `G(s)` = (`s`/2)/`L(s)` (1.38 at 72px, 1.09 at 100, 0.86 at 200, 0.71 for large boxes), so the handle stays under the pointer until a limit; exactly 0 at or beyond the 15px point | `unified-object-editing`. 15px inset: 4.3px glyph gap to the corner resize squircle (a 12px inset left 1.3px). At `ρ` = 1 the handle is `(s − 14)/2` from each side of its corner, so two neighbours are 14px apart, 4px between the 10px glyphs, at every `s`: criterion 8 holds by construction and an unlinked corner (`rectangle-corner-radii`, `ρ` may pass 1) cannot break it either. Star inner-radius handle: at the first inner vertex; its worst case is a point count that is a multiple of 4 at ratio 0.99 against a corner resize glyph: 4.6px at `s` 72, 2.9px at 64, which is why `T` is 72 and not 64. The center glyph is not drawn while any parameter handle center is within 20px of it, nor during a parameter drag. Parameter handles are not drawn during a move, resize, rotate or skew drag of the same object, nor for two or more selected objects |
| Parameter handle guide | 1px dashed `--shape-handle-guide` | From the corner to a hovered or dragged radius handle only; not at rest |
| Live preview outline | 1.5px screen-space, `--preview-new`, hollow, no fill, solid, drawn on top of the committed geometry | `unified-object-editing`: for every Select-tool drag (move, resize, rotate, skew, radius, inner radius, also a slider drag in the bar) the geometry a release would commit is drawn in blue over the object **unchanged in its own committed style** ("black old": whatever it renders as now, not dimmed, not re-weighted). Constant screen width, independent of the object's stroke width. No fill preview yet. Typed values (entry chip, bar fields) do not preview. Selection box, handles, pivot marker and readout follow the new geometry and draw above the blue outline. Same weight as the marquee stroke, so at 1.5px it is at least as heavy as a hairline and reads on top where the two coincide |
| Select bar layout | Pill: every row is 36px (28px controls, 4px above and below), so the first row, and with it the switches, sits at the same y whether the bar has one row or two; controls 28px high, `text-sm`; 1px 20px 25% dividers between groups, 12px gap inside; **left-aligned after the tool rail** (`justify-start`) | `unified-object-editing`. Order: settings group ("Scale stroke width", "Scale corner radius": always shown, never disabled, never dimmed, first so they never move), kind groups (rectangle, later ellipse, polygon/star), "Object to path" last. A control shows when the selection contains the kind it acts on, and acts on exactly those objects; it is enabled when it would change something; never shown disabled for an unrelated selection. Wraps by whole groups when the canvas is narrower than the row; a divider sits between two groups on the same row only, never at the start of a row (about 760px for a rectangle, 780 for a star): settings stay on row 1. Fixed-width fields so nothing jitters. The Node and Shape bars stay centred. See "Select bar" below |
| Bar number field | 28px high, white ground, 14px tabular text, right-aligned, fixed `mm` suffix inside the right edge; Radius field 112px wide (2026-10-06 review: 80px cannot hold a value, the 12px "limited" tag and the unit; the number was clipped to one or two characters); Points field 80px ("Mixed" was clipped at 64px next to the native stepper) | The "Radius" field of the Select bar (rectangle selection): Enter commits once, Escape or a press elsewhere restores the shown value and writes nothing (the entry-chip rule), invalid: 2px `--field-invalid` and a 12px message line under the field ("Enter a number"), `aria-invalid`. Mixed: empty with the muted placeholder "Mixed" |
| Limited tag | 12px text inside a bar field, `--toolbar-icon` at 70%, before the unit | A stored value larger than the object allows (a radius above half the shorter side after a shrink): the field shows the effective value and the tag "limited"; tooltip "Stored 20 mm, limited to 15 mm by the size; enlarging brings it back." Also for the curve value later. Not for a ratio's second-decimal rounding |
| Edit hint chip | The hint chip surface, 3 lines, 3s life (or a press, key or leave), `pointer-events: none`, `aria-live="polite"` | Shown at the pointer after a double-click on a primitive's outline, body or center handle, where nothing else happens: "Drag a handle to edit this shape" / "Double-click a handle to type a value" / "Nodes: Object to path, then double-click". Writes nothing, changes neither tool nor selection. Shown on every such double-click; not on a path (it opens the Node tool), a skew handle or empty canvas |
| Bounding-box selection outline | 1px screen-space `--accent` (selected) / `--accent-hover` (hover) | Drawn around a selected/hovered primitive when its own matching tool is active (`primitive-shapes`; **ended 2026-10-06 by `unified-object-editing`: the shape tools only create; while one is active a selected object keeps this 1px `--accent` box only, with no hover box, no handles and no hit state**) — **and, as of `canvas-navigation-and-selection`, around any selected/hovered object of any type (path or primitive) when the Select tool is active, with no shape handles or path nodes added on top.** Same token, two contexts: a path's nodes appear only after double-click handoff into the Node tool; **a primitive's parameter handles are drawn by the Select tool itself, with its transform handles (`unified-object-editing`), and a double-click on a primitive hands off to nothing.** |
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
  customer decision. A skew does not tilt the box either: it stays the tight
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
  chip names both. No tool switch is involved anywhere. Below 72px the
  parameters stay editable through the bar fields and by zooming. A new
  parameter kind adds a knob position and a readout, not a new glyph.
- **Select bar** (`unified-object-editing`; row "Select bar layout" for the
  numbers). Three zones in this order: settings (the two switches), the groups
  of the kinds in the selection, "Object to path". A control shows when the
  selection contains at least one object of its kind and then acts on exactly
  those objects (others are left alone, the tooltip says so); it is enabled
  when it would change something ("Remove rounding" is disabled when every
  selected rectangle is already sharp). A bar never carries disabled groups for
  unrelated kinds. Mixed values show an empty field with "Mixed"; a typed value
  applies to all. A stored value the field cannot show shows the effective
  value and the "limited" tag. "Object to path" is last and has no shortcut
  (the browser build's inspector owns Shift+Ctrl+C; there is no undo yet).
  Overflow wraps by whole groups; the settings stay on the first row.
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
    left-[72px] z-20`, centred, right of the tool rail), pill `h-9`,
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
| Toggle "Scale stroke width" | none; Tab to the switch, Space | `object-transform`; Select tool's bar, tool state, off per session, never saved; no letter shortcut until usage shows one is needed |
| Reveal the four side rotate handles | hold Shift (Select tool, one object selected, no drag running) | `object-transform-refinements`; also Shift = pivot on the opposite point while rotating, resize/skew about the center |
| Type an exact angle or size | double-click a rotate or resize handle | `object-transform-refinements`; Enter commits, Escape cancels, Tab moves between W and H. No keyboard-only route yet (Properties-panel transform form is a follow-up) |
| Snap a rotation or skew to 15° and 22.5° stops | hold Ctrl while dragging | `object-transform-refinements`; with a resize, Ctrl keeps proportions |
| Type a radius or an inner ratio | double-click the radius handle or the star's inner handle (Select tool) | `unified-object-editing`; same chip rules as the size entry. The bar's "Radius", "Points" and "Ratio" fields are the keyboard route |
| Object to path | none; the "Object to path" button of the Select bar | `unified-object-editing`; no shortcut until undo exists (Shift+Ctrl+C opens the inspector in a browser build) |
| Open/focus the style panel | Shift+Ctrl+F | Matches Inkscape's Fill & Stroke binding; app-global, not canvas-focus-scoped (`stroke-and-fill-styling`) — see note below |

Single-letter tool shortcuts are a different category from the File menu's
native accelerators (table above) — they're bound at canvas-focus scope, the
same convention Inkscape/Illustrator/Affinity use, not native menu items.
The style-panel shortcut is a third category: app-global (works regardless
of canvas focus), since the panel it targets isn't canvas content.
