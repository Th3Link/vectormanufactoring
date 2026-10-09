# Document size and rulers

Status: Draft
Priority: Must
Origin: Customer

## User value

As a maker I want to see a ruler along the top and left of the canvas,
resize my document (with old content staying centered rather than jumping to
a corner), fit the document to what I've drawn, and keep drawing freely
outside the document edge the way I already can in Inkscape, so that I can
judge real size and position at a glance and prepare a sheet that actually
matches my material or my artwork.

**What we do differently, and what we match on purpose:**

- **Rulers in mm, origin at the document's top-left corner, position
  increasing right/down**: this matches Inkscape's own current default
  (SVG's own coordinate convention, which Inkscape adopted as its default
  origin in 1.0, after years of an optional bottom-left/Y-up mode) and this
  product's own existing Y-down convention (ADR 0002 §4). Not a deviation —
  confirmed before writing this spec, not assumed.
- **Center-anchored resize is a deliberate customer choice, not Inkscape
  parity.** Inkscape's own default anchors a page resize at a corner (with a
  3x3 anchor picker to choose another point); this spec's acceptance
  criteria require the *center* of our single document to stay fixed
  instead, every time, with no picker. Call this out anywhere it looks like
  a missed Inkscape-parity opportunity — it isn't one.
- **Draw outside the document freely, pasteboard rendered grey**: Inkscape's
  own "canvas" (its page) sits on a pasteboard the maker can draw on without
  restriction; this spec matches that directly, because the customer named
  it directly ("like in Inkscape").
- **Fit-to-content with a 0 mm margin**: Inkscape's own "Resize page to
  content" has a configurable margin, default non-zero. This spec ships a
  fixed 0 mm (tight bounding box) instead, because the common laser-cutting
  use case this product targets most — sizing a sheet to exactly the parts
  on it — wants the smallest honest sheet, not padding the maker didn't ask
  for; a configurable margin is a small, safe addition for later if the
  customer wants one (see Out of scope).

## Acceptance criteria

### Rulers

1. Given a project is open, then a horizontal ruler is shown along the top
   edge of the canvas viewport and a vertical ruler along the left edge,
   both in millimetres, both updating live as the view pans or zooms — the
   same `ViewTransform`/viewport state `canvas-navigation-and-selection`
   already tracks, not a second copy of it.
2. Given the document, then ruler position 0 mm on both axes aligns with
   the document's top-left corner; position increases rightward on the
   horizontal ruler and downward on the vertical ruler.
3. Given the pasteboard area outside the document (left of or above the
   origin, or beyond the document's width/height), then each ruler
   continues past 0 mm (showing negative values) or past the document's own
   dimension (showing values larger than the document size) with no visual
   break in the ruler's track — position stays readable anywhere a maker
   can draw, not only inside the document.
4. Given any zoom level, then each ruler's labeled major tick marks are
   spaced between 40 and 120 CSS pixels apart on screen, with each labeled
   value chosen from the sequence 1, 2 or 5 x10^n mm (the "nice numbers"
   rule standard rulers use, including Inkscape's) — e.g. at 100% zoom
   (1 mm ≈ 3.78 CSS px) major ticks land every 20 mm; near the 8000% zoom
   ceiling they land at 1 mm or finer; near the 2% floor they land at
   500 mm or coarser. Minor, unlabeled ticks subdivide each major interval
   into 5 equal parts.

### Document resize with center-anchored content

5. Given the Properties panel's document-size fields (width and height in
   mm), when the maker edits either field and confirms it, then the
   document's size changes to the new value as one operation.
6. Given a document resize via criterion 5, when the document had one or
   more objects immediately before the resize, then the point in the
   document that was the document's geometric center before the resize is
   still the document's geometric center afterward, and every object's
   position relative to that center point is unchanged — the document grows
   or shrinks symmetrically around its own center rather than around a
   corner or the document origin. This is a deliberate customer-specific
   choice (see User value); no corner or other anchor option is offered,
   and **it governs manual resize only** — fit-to-content (criterion 10)
   uses a separate, content-centered rule instead, stated there, because
   the two can require different results whenever content is not already
   centered on the old document.
7. Given criterion 6, when the new document size is smaller than the
   content's own bounding box, then content is only ever repositioned by
   criterion 6's rule, never scaled or cropped to fit — it may extend into,
   or past, the pasteboard (see "Pasteboard and drawing outside the
   document" below, which explicitly allows this).
8. Given any action that sets the document's size — a manual resize
   (criterion 5) or fit-to-content (criterion 10) — when the resulting
   width or height would be less than 1 mm, then it is clamped to 1 mm
   instead: the document can never end up with a zero or negative
   dimension.
9. Given a document resize (criterion 6) or a fit-to-content (criterion 10),
   then the viewport's pan adjusts so that the document point at the center
   of the screen immediately before the change is still at the center of
   the screen immediately afterward — the maker sees the document (and, for
   fit-to-content, its content) resize in place around the view's own
   center, rather than the content appearing to jump elsewhere in the window
   or needing a manual re-pan to find it again.

### Fit to content

10. Given the document has at least one object, when the maker triggers
    "Fit to content" (an action in the Properties panel's document
    section), then the document's new size is set to the bounding box of
    every object in the document with a fixed 0 mm margin (tight fit), and
    the document's new center is set to that bounding box's own center —
    a separate, content-centered rule from criterion 6's old-center rule,
    deliberately: the new document's edges touch the content's bounding
    box exactly, regardless of where the old document's center happened to
    be. (Criterion 8 bounds the minimum size this can produce; criterion 9
    describes the resulting on-screen effect.)
11. Given fit-to-content (criterion 10), then a stroked object's
    contribution to the bounding box is its geometric outline (the path's
    own fill geometry) only, not expanded by stroke width — e.g. a straight
    line stroked 10 mm wide contributes a zero-area outline along the
    line's own path, not a 10 mm-wide band.
12. Given the document has no objects, when the maker triggers "Fit to
    content", then the action is disabled (or is a no-op with no error)
    and the document's size is left unchanged — there is no bounding box to
    fit to.

### Pasteboard and drawing outside the document

13. Given a document of any size, then the document's own area renders with
    the existing canvas background color (`--canvas-bg`), and every part of
    the viewport outside the document's bounds — out to the edges of the
    window, following pan and zoom — renders a distinct grey pasteboard
    color, visually distinguishable at a glance from `--canvas-bg` and from
    the existing chrome colors (`--statusbar-bg`/`--toolbar-bg`).
14. Given any drawing or editing tool is active (Select, Pen, Node,
    Rectangle, Ellipse, Polygon/Star), when the maker creates, places or
    drags an object so that part or all of it lies in the pasteboard
    (outside the document's bounds), then the object is created or edited
    exactly as it would be inside the document — no error, no warning, no
    clipping to the document edge.
15. Given an object with any part in the pasteboard, then it remains
    selectable, movable and editable exactly as an object fully inside the
    document — no special-cased behavior for pasteboard-located geometry.
    (What a manufacturing job does with pasteboard-located geometry is
    explicitly not this slice's concern — `manufacturing-roles` and
    `laser-job-preview-and-output` decide that later; this slice must not be
    read as pre-deciding it either way.)

## Out of scope

- **Multiple pages or documents within one project.** The customer asked
  for this originally; dropped for MVP (2026-10-05 — "not important enough
  for MVP"). This slice is single-document only: no page list, no
  per-document settings, no switching between documents. Revisit as its own
  spec later if the customer wants it back.
- **A resize-anchor picker** (Inkscape's 3x3 corner/edge/center choice).
  Center-anchoring is the only behavior this spec asks for, by explicit
  customer request; an anchor picker is a real but unrequested feature.
- **A configurable fit-to-content margin.** Fixed at 0 mm for this slice
  (see User value for reasoning); a numeric margin field is a small later
  addition if asked.
- **Ruler unit switching (mm/inch toggle), moving the ruler's zero point by
  dragging, or a ruler right-click menu** (all things Inkscape's own ruler
  supports). This product's canonical unit is mm everywhere (`CLAUDE.md`,
  ADR 0002 §2); no unit switcher exists anywhere yet, and nothing here asks
  for one.
- **Guides** (draggable guide lines pulled off a ruler, snapping to them).
  Inkscape's ruler doubles as a guide source; this product has no guide
  concept yet at all. A later spec's job, not folded in here just because
  rulers are adjacent to it.
- **A live cursor-position marker drawn on the ruler itself** (Inkscape
  shows a small tick following the pointer on each ruler). Not asked for;
  the existing status-bar cursor-position readout (`project-file-foundation`)
  already covers "where is my cursor in mm."
- **What a manufacturing job includes from the pasteboard vs. the document**
  (criterion 15's explicit carve-out). Decided by `manufacturing-roles` and
  `laser-job-preview-and-output`, much later in the sequence; this spec must
  not be read as having decided it either way already.
- **Zoom presets** ("fit document to window", "fit selection"). Still out
  of scope per `canvas-navigation-and-selection`; unaffected by anything
  here.
- **Undo for any operation in this spec** (resize, fit-to-content).
  `undo-redo` (slice 8) is cross-cutting infrastructure built once there is
  more than a couple of operations to cover; this spec does not special-case
  undo ahead of that slice, same deferral pattern slice 4 used for pan/zoom.

## UX notes

### Rulers: docked chrome, not floating overlay — placement and coexistence with the left tool panel

The horizontal and vertical rulers are **docked**, the same category as the
right `PropertiesPanel` and the status bar, not an overlay like the floating
left tool panel or the per-selection mini-toolbar. They claim real layout
space at the top and left edges of the canvas viewport, shrinking the
viewport by their own thickness — this is a one-time, permanent chrome
addition, not a per-tool layout shift, so it does not conflict with
`docs/design-system.md`'s "no layout shift on tool switch" rule (that rule
governs what happens when the maker switches tools; the rulers are present
regardless of tool).

Because the floating left tool panel's 12px inset is already defined
relative to "the canvas's left and top edges" (`docs/design-system.md`),
and the rulers shrink the canvas viewport rather than drawing on top of it,
the tool panel's inset is automatically measured from the new, post-ruler
viewport edge once this ships — it ends up sitting 12px inside the ruler's
own edge, never overlapping it, with no change needed to the panel's own
positioning rule. This is the deliberate choice that answers "how do the
ruler and the floating panel coexist": the ruler owns the strip, the
floating panel's existing relative-inset math takes care of the rest.

- **Thickness: 24px**, reusing the status bar's existing `h-6` (24px)
  chrome-strip convention (`frontend/src/components/StatusBar.tsx`) rather
  than inventing a new strip size — one "thin chrome bar" scale used on
  three edges (bottom: status bar, now top + left: rulers).
- **Corner square**: a plain 24×24px square where the two rulers meet
  (top-left of the viewport), filled with the ruler background color, no
  ticks or labels — yes, the standard ruler convention, confirmed as
  intended.
- **Background**: `--statusbar-bg` (`#DCDCE0`) — reuses the existing
  "chrome, not canvas" tone already shared by the status bar and tool rail,
  rather than a new token. The document/pasteboard colors (next section)
  are canvas content and stay visually distinct from this chrome strip.
- **Tick marks**: 1px screen-space lines (matching every other 1px
  screen-space line already in the system — handle lines, selection
  outline), color `--toolbar-icon` (`#3A3A3F`, the existing default chrome
  icon/text color). Major ticks run the full 24px depth; minor ticks run
  8px from the viewport-facing edge inward (roughly a third of the strip,
  enough to read as subdivision without crowding the label).
- **Labels**: the status bar's existing `text-xs` (12px) convention and
  `--toolbar-icon` color — no new font-size token needed, same reuse
  instinct as the background. One label per major tick, placed just inside
  the tick, oriented horizontally on both rulers (the vertical ruler's
  labels are not rotated — faster to read, matches Inkscape's default).
- **Major-tick spacing and "nice numbers"** (criterion 4) is a rendering
  computation, not a new visual token — reuses the tick/label styling above
  at whatever interval the 40–120px/1-2-5×10^n rule picks for the current
  zoom.
- **Rendering location, flagged for the architect/implementer**: rulers are
  mostly numeric text at a fixed screen position — closer to the status bar
  (ordinary DOM app chrome, per `docs/design-system.md`'s own categorization
  of `PropertiesPanel`) than to the WebGL draw list reserved for canvas
  editing UI. Recommend a DOM component sibling to `StatusBar.tsx`, fed by
  the same `ViewTransform`/viewport state, not a new WebGL text-rendering
  path. Flagging rather than mandating since rendering-location calls are
  the architect's, not UX's.

### Document resize UI: a Properties-panel section, above Style

Properties-panel section, not a dialog — the panel's whole role is
"everything configurable, fixed in place" (`docs/design-system.md`), and a
routine, frequent settings change like document size is exactly what that
role covers; a dialog would need justification this spec doesn't have (the
one existing modal, `AlertDialog`, is reserved for blocking errors, a
different shape of interruption).

A new **"Document" section**, **always shown at full, enabled state** —
unlike Style or Shape-tool-options, it doesn't depend on any object
selection, only on "a project is open," which is always true — so it needs
no disabled/placeholder state. Placed **at the top of the panel, above
Style**: every other section goes disabled/placeholder-blank until
something is selected, so a freshly opened project with nothing selected
would otherwise open the panel to an almost entirely inert first
impression. Leading with the section that is always live avoids that.

- Two numeric fields, **Width** and **Height**, mm, laid out side by side
  (`W: [____] mm`  `H: [____] mm`), same "mm" unit framing used everywhere
  else in this product. Commit on blur or Enter, Escape reverts to the
  last-committed value — the exact numeric-field discipline
  `stroke-and-fill-styling`'s UX notes already established for typed
  numeric input (no live-preview-while-typing requirement, unlike a
  slider).
- **"Fit to content" button** directly below the two fields, full-width
  button matching other full-width action buttons in the panel. Disabled
  (not hidden) when the document has no objects (criterion 12) — same
  "disabled, not hidden" convention as the gradient stop list's remove
  button at the 2-stop floor.
- The status bar's existing document-size field (`project-file-foundation`)
  needs no new UX decision for this spec — it already reads whatever
  `sizeMm` the document reports, the same prop `StatusBar.tsx` takes today;
  once this spec's resize/fit actions are wired through, it stays in sync
  automatically.
- **Invalid input (non-finite, or below the 1mm floor `adrs.md` sets):**
  the field reverts to the last-committed value on blur/Enter, exactly as
  Escape already does — no error toast or dialog for this. Routine,
  recoverable input mistakes don't earn an interruption in this product
  (the one existing modal is reserved for blocking errors); silently
  snapping back to the last good value, the same outcome as canceling,
  is enough feedback for a maker who typed "0" or "-5".

### Resize view behavior: the view follows the shift, same as fit-to-content

`adrs.md` fixes this for fit-to-content (AC8's "exactly where it was on
screen" requires it) but leaves plain resize (criteria 5-7) open for this
spec to decide. **Decision: the view follows the shift for resize too** —
after typing a new width/height, the content stays visually static on
screen and the document's own edges visibly grow or shrink around it,
exactly the same camera behavior as fit-to-content, not a second behavior
for a sibling operation. Reasoning: "center-anchored resize" is specified
from the content's point of view (criterion 6: object positions relative to
each other are unchanged, only the whole document's content shifts to stay
centered) — the one framing consistent with that is "my artwork didn't
move, my sheet did." The alternative (view fixed, content visibly jumps by
the shift) would make every resize look like the maker's own drawing
moved, for a value they didn't touch, which is the more confusing reading
of "center-anchored" even though the underlying document math is identical
either way. Both operations are instant, no animation — consistent with
this product's established precedent against unneeded animation
(`path-merge-split-and-node-types`'s "Visual feedback for Join: instant, no
animation" — "a tween for exactly this one command would be a new animation
vocabulary for a single interaction") — so this is a one-frame jump in both
cases, not a tween to design.

### Pen-preview knockout color, flagged by `adrs.md` as a UX check

`pen_preview.rs`'s close-target knockout currently fills with the single
constant `CANVAS_BG`. Once the pasteboard is a distinct, darker color from
`--canvas-bg`, a knockout drawn while the pen tool is active over the
pasteboard (legal per criterion 11 — every tool works there identically)
would render as a mismatched light dot instead of blending into whatever's
actually behind it. **Decision: the knockout fill must match whichever
background is under it** — `--canvas-bg` over the document, `--pasteboard-bg`
over the pasteboard — not a hardcoded constant. This is a correctness fix
riding on this spec, not a new visual element: without it, the pasteboard
work introduces a visible regression in a tool `path-node-editing` already
shipped.

### Pasteboard visual: a third, darker neutral grey, flat — no shadow

- **New token, `--pasteboard-bg: #B8B8BE`.** Checked against the existing
  neutral-grey family first (`--canvas-bg` `#E8E8EB`, `--statusbar-bg`/
  `--toolbar-bg` `#DCDCE0`) rather than reusing one of them, because
  criterion 10 requires the pasteboard to be "visually distinguishable at a
  glance" from *both* — reusing `--toolbar-bg` would make the pasteboard
  read as "more chrome" rather than "canvas surroundings," which is the
  wrong signal (it's still canvas space the maker draws on, criteria 11-12).
  `#B8B8BE` stays in the same low-saturation neutral family (reads as "part
  of the same calm palette," not a jarring new hue) while sitting a clear
  step darker than both existing tones, matching the direction (darker =
  further from the work surface) every reference tool in this space uses.
- **No drop shadow, no border.** Inkscape's subtle shadow under the page is
  explicitly not matched here: the document/pasteboard boundary is canvas
  content (WebGL draw list, not DOM), and `--panel-elevation-shadow` is
  documented as a DOM `box-shadow` for floating chrome specifically — reusing
  it here would both misuse a token outside its stated scope and add a
  decorative effect this product's flat, calm visual language (no
  unnecessary gradients/shadows on canvas content anywhere today) doesn't
  otherwise have. The flat color-contrast step above is sufficient to read
  the document/pasteboard boundary at a glance; add a border or shadow
  later only if real usage shows the flat contrast isn't enough.

## Links
Requirements: none yet in `docs/requirements.md` — this is a new customer
ask explored ahead of being sequenced into `specs/index.md`; add a
requirement id when it is formally scheduled.
Related: `specs/0001-project-file-foundation/specification.md` (existing
`DocumentSize` concept, status-bar document-size field),
`specs/0004-canvas-navigation-and-selection/specification.md` +
`adrs.md` (`ViewTransform`/viewport pan-zoom state rulers must track),
`docs/design-system.md` (2026-10-05 chrome: right `PropertiesPanel`,
where the document-size fields live).
