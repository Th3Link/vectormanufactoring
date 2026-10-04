# Design system

Tokens and interaction conventions for the desktop app shell (Tauri + React +
Tailwind + shadcn/ui, ADR 0001). Seeded by `path-node-editing`
(`specs/path-node-editing/specification.md`), the first feature that needs
real color/spacing tokens rather than the one inline `--canvas-bg` value
`project-file-foundation` used as a placeholder. Extend this file in place as
each later feature introduces new components or states; don't invent tokens
inline in a spec once they exist here.

Only one theme exists (light). Dark mode is undecided (no ADR yet) — token
*names* are chosen so a future dark theme is a value swap, not a rename.

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
| `--panel-bg` | `--toolbar-bg` (`#DCDCE0`) | `StylePanel` and any later docked property panel's background — reuses the one chrome color rather than adding a second (`stroke-and-fill-styling`) |

## Spacing and sizing

| Token | Value | Used for |
|---|---|---|
| Tool rail width | 48px | Left-docked vertical toolbar |
| Tool icon size | 24px | Icon glyph inside a 48×48 button |
| Node glyph | 7×7px screen-space | Corner (square) and smooth (diamond) node markers |
| Handle endpoint | 6px diameter screen-space | Circle |
| Handle line weight | 1px screen-space | Node-to-handle connector |
| Segment selection overlay | +2px screen-space over the geometry's own stroke | Drawn on top, doesn't replace the real stroke |
| Point hit-test radius | 8px screen-space | Minimum clickable radius around any node/handle, even though the visual glyph is smaller (Fitts's-law margin for mouse precision) |
| Segment hit-test tolerance | 4px screen-space perpendicular distance | Clicking "on" a curve/line segment |
| Shape handle | 8×8px screen-space | Hollow square, `primitive-shapes`: bounding-box resize, rectangle corner-radius, polygon/star inner-radius — deliberately square and larger than the 7px/6px node-tool glyphs so the two vocabularies never read as the same control |
| Shape handle hit-test radius | 8px screen-space | Same margin rule as node/handle hit-testing, reused rather than invented fresh |
| Bounding-box selection outline | 1px screen-space `--accent` (selected) / `--accent-hover` (hover) | Drawn around a selected/hovered primitive — the primitive equivalent of slice 2's node/segment selection, scoped to the primitive's own matching tool being active |
| `StylePanel` width | 280px, fixed | Right-docked property panel (`stroke-and-fill-styling`), mirrors the left tool rail; canvas fills the remaining width |

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
- **Tool rail**: docks to the left, between the native menu bar and the
  status bar, fixed 48px width, canvas fills the remaining width
  edge-to-edge (the `project-file-foundation` "chrome never frames the
  canvas" precedent — the rail is chrome beside the canvas, not a border
  around it). New tools are appended to the rail top-to-bottom in the order
  they ship; existing icons don't get reordered for a later feature's
  convenience.
- **Docked property panels** (`StylePanel`, first instance in
  `stroke-and-fill-styling`): dock to the right edge, same span as the tool
  rail (native menu bar to status bar), same "chrome beside the canvas, not
  framing it" rule, mirrored to the opposite side. Collapsible via a chevron
  tab on the canvas-facing edge; collapsed width is 0, not a narrower icon
  strip. This is ordinary DOM app chrome, not canvas editing UI — it does
  not go through the WebGL draw list rule above.
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
  multi-select-editing panel.

## Keyboard shortcuts established so far

| Action | Shortcut | Notes |
|---|---|---|
| New / Open / Save / Save As | Ctrl/Cmd+N/O/S/Shift+S | `project-file-foundation`, native menu accelerators |
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
