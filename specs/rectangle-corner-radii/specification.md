# Rectangle corner radii: a separate radius for each corner

Status: Draft
Priority: Should
Origin: Customer

## User value

As a maker I want to give each corner of a rectangle its own radius so that a
name tag with two round corners, a panel with one rounded corner for a finger
hole, or a tab with a sharp and a rounded end stays one editable rectangle
instead of a path I have to build node by node.

The customer's words (translated, 2026-10-06): "I found corner rounding for
rectangles ... I would like to be able to give individual corners different
radii." Today a rectangle has one radius for all four corners
(`specs/0003-primitive-shapes/` criterion 4).

**Depends on `specs/unified-object-editing/`.** The radius handles live in the
Select tool next to the transform handles; this spec does not exist without
that one. Unification keeps the single radius first; this spec makes the four
radii independent.

**Field reference.**

- Inkscape: a rectangle has one Rx/Ry pair for all corners (elliptical, not
  per corner); different corners need a path or a path effect.
- Illustrator (Live Corners): a widget at every corner; dragging one drags all,
  clicking a widget first selects it so that only that corner changes; a dialog
  gives a number per corner.
- Figma: one radius handle for all corners; Alt-drag changes one corner; a
  toggle in the properties panel switches to four independent fields.
- CSS and SVG: CSS `border-radius` takes up to four radii and shrinks them all
  by one factor when neighbours would overlap (CSS Backgrounds and Borders
  Level 3, "Overlapping curves"). SVG's `<rect>` has a single `rx`/`ry` pair
  clamped to half the width and half the height.
- Resizing a rounded rectangle: Inkscape's selector bar has a toggle "scale
  rounded corners in rectangles" (off by default); Illustrator has "Scale
  Corners" (Preferences and the Transform panel, off by default). Both keep the
  radius at its absolute size unless the toggle is on. Figma keeps it too.

Where we do better: all four handles are always visible, whether corners move
together is a visible switch (not only a hidden modifier), the dragged value is
shown in millimetres while dragging, and a drag never shrinks the other
corners under the maker's hand (criterion 4), where the CSS rule alone would.
Whether a resize scales the radius is a switch in the same bar as "Scale stroke
width", visible before the drag and not buried in preferences (criterion 15).

**Customer decision (2026-10-06):** "yes, we need such a switch". The switch
"Scale corner radius" is a requirement, not a proposal. Its default is off
(confirmed by the customer, 2026-10-06; see "Changes to accepted behaviour").

## Changes to accepted behaviour

**Customer-visible change.** Today (slice 5, `specs/0005-object-transform/`
criterion 9) resizing a rectangle in the Select tool always scales its radius
by `√(sx·sy)`. With this spec the default is the opposite: the switch "Scale
corner radius" is off at program start and a resize keeps every radius at its
absolute size in millimetres. A maker who relies on proportional rounding
turns the switch on. The radius parts of `0005` criteria 9 and 31 are
superseded for rectangles by criteria 12, 13 and 15 below (the stroke parts
stay). This matches what the Rectangle tool did before the unification
(`0003` criterion 3). The lead updates the owning specs when this one is
accepted; this spec does not edit them.

Terms: corners are named top-left, top-right, bottom-right, bottom-left in the
rectangle's own, possibly rotated, frame (**TL, TR, BR, BL**). "Radius" is
always circular (one value per corner, a quarter circle).

## Acceptance criteria

All criteria concern the Select tool with exactly one rectangle selected and
its handles drawn (the size rules of `specs/unified-object-editing/`
criteria 6 to 8). "Effective radius" is the radius after the clamping rule of
criterion 9.

### Setting the radii

1. Given a rectangle, then four radius handles are shown, one per corner, each
   at the position its own effective radius implies by the position rule of
   `specs/unified-object-editing/` criterion 2 (on the corner's diagonal, 15 px
   from the corner at radius 0), each draggable (at radius 0 too). The
   rectangle stores four radii. The rule maps the largest linked radius to the
   far end of the travel, so an unlinked corner may exceed that value (up to
   its limit in criterion 4): its handle then sits further along the diagonal.
   For two adjacent corners the 4 px clearance of
   `specs/unified-object-editing/` criterion 8 holds because the limit keeps
   the two radii on one side summing to at most that side's length. Two
   diagonal corners (TL/BR, TR/BL) have no limit between them, so a knob is
   drawn at the capped position `ρ' = min(ρ, max(1, Σ − ρ_partner))`, where
   `ρ` is the corner's effective radius divided by `s/2`, `s` and `S` are the
   shorter and the longer side of the box on screen, `L(s)` is the travel
   length of the design system (row "Parameter handle layout"), the partner is
   the diagonally opposite corner, and `Σ = 2 + √2·(S − s)/L(s)` (on a square
   `Σ = 2`). Two diagonal knobs then never overlap: at least 14 px apart
   centre to centre and 4 px between the glyphs, at `s` of 72 px and above. Only
   the drawn position (and the guide line to it) changes; the stored and
   effective radii, the outline, the readout and the bar field are not
   affected, and a knob whose `ρ` is at most 1 is always drawn at its own
   radius. A knob alone may still reach `ρ' = 2`. (The claim of
   `specs/unified-object-editing/` UX section 1 that the limit keeps the
   clearance for any unlinked corner held for adjacent corners only; corrected
   there by a dated note.)
2. Given the "Link corners" switch is on, when the maker drags any
   radius handle, then all four radii become the dragged value, limited to half
   the shorter side. Radii that differed before the drag are overwritten. The
   switch is a Select tool setting: on by default, session UI state of the same
   class as the two Scale switches (`0005` criteria 27 and 30), never written
   to the project, and reset to on by New and Open. Its place is the rectangle
   group of the Select bar, between the "Radius" field and "Remove rounding"
   (28 px square icon button, no text label, shown whenever the selection
   contains a rectangle and never disabled; one Tab stop in the order Radius
   field, Link corners, Remove rounding; Space and Enter toggle it). Look:
   Lucide `Link` (closed chain, filled active ground) when on and `Unlink`
   (broken chain, transparent ground) when off, so the state is never carried
   by colour alone. It has `aria-pressed` and the accessible name "Link
   corners". Tooltip, on: "Link corners: on. A corner handle sets all four
   radii. Hold Shift to change one corner."; off: "Link corners: off. A
   corner handle changes its own corner. Hold Shift to set all four."; both
   with a second muted line "Applies to the corner handles of one selected
   rectangle. The Radius field always sets all four." Sizes, tokens and the
   bar width are in `docs/design-system.md` (row "Link corners toggle").
3. Given the switch is off and Shift is not held at the press, or the switch is
   on and Shift is held at the press, when the maker drags a radius handle,
   then the drag is unlinked: only that corner's radius changes; the other
   three radii and effective radii do not change at any moment of the drag.
   Shift is an exclusive-or with the switch, not an or: Shift inverts the
   switch for that one drag, so with the switch off and Shift held all four
   corners change as when the switch is on. The state is read once at the press
   (for a typed entry, at the second press of the double-click) and frozen; a
   Shift pressed or released mid-drag, or a click on the switch, changes
   nothing until the next drag. (The architect's default in `adrs.md` was OR;
   the `ux-engineer` overruled it on 2026-10-07 and the architect's flag 3 is
   resolved this way.) Given a rectangle whose radii exceed what its size
   allows (`f < 1`, criterion 9, after a resize with "Scale corner radius"
   off), an unlinked drag or entry also writes the three other corners at
   their effective values, so that they stay as drawn; nothing visible changes
   at the press or at the release, but enlarging the rectangle afterwards no
   longer brings the old stored radii of those three corners back (criterion
   10).
4. Given an unlinked drag, then the dragged corner's radius is limited to the
   largest value that leaves the three other effective radii unchanged:
   `min(W − r_h, H − r_v)`, where `W` and `H` are the box's width and height
   and `r_h`, `r_v` are the effective radii of the corner that shares the
   horizontal side and of the one that shares the vertical side with the
   dragged corner (for TL: TR and BL). The handle stops there; it does not
   overshoot and snap back. Given a linked drag, the limit is half the shorter
   side. While a limit stops the drag, the readout of criterion 7 carries the
   suffix "max" (also for a linked drag at half the shorter side).
5. Given a radius drag, when the pointer returns to or beyond the handle's
   zero-radius position, then the radius (of the dragged corner, or of all four
   when linked) is exactly 0 and the corner is sharp.
6. Given a double-click on a radius handle, then a field "r" (mm) opens at the
   knob as `specs/unified-object-editing/` criterion 18 defines, showing that
   corner's effective radius, with a second row of muted text naming the scope:
   "All four corners" or "This corner only". The scope is the state of switch
   and Shift at the second press (exclusive-or, criterion 3), fixed when the
   field opens. The field's accessible name is "Corner radius, all corners"
   when the scope is all four and, when it is one corner, "Top-left corner
   radius", "Top-right corner radius", "Bottom-right corner radius" or
   "Bottom-left corner radius" (the corner names of the rectangle's own frame).
   Enter applies the value to all four corners or to that corner according to
   the scope. A value above the limit of criterion 4 (or half the shorter side
   for all four) is limited, the limited value is written, and the readout
   "r 12.0 mm max" shows at the knob for 1.5 s (a limit is never silent). Zero
   is valid. Same cancel, invalid and untouched-Enter rules.
7. Given a radius drag, then a live readout at the pointer shows the dragged
   corner's radius: "r 3.5 mm"; "r 12.0 mm max" while a limit stops the drag
   (criterion 4); and, for a linked drag whose press found unequal radii (the
   drag overwrites them), the suffix " · all corners", joined to the other
   suffix as "r 12.0 mm max · all corners". Escape cancels the drag and writes
   nothing. The blue preview and the black original of
   `specs/unified-object-editing/` criteria 10 to 13 apply.
8. Given "Remove rounding" (`specs/unified-object-editing/` criterion 21), then
   all four radii of every selected rectangle become 0 in one commit,
   whatever the state of the Link switch and of Shift. Given the switch "Link
   corners", then switching it changes no radius, writes nothing, and closes an
   open entry field without writing (the open entry keeps the scope it was
   opened with).

### Clamping

9. Given radii `r_TL, r_TR, r_BR, r_BL` on a rectangle of width `W` and height
   `H`, then the effective radii are `f · r_i` with
   `f = min(1, W / (r_TL + r_TR), W / (r_BL + r_BR), H / (r_TL + r_BL),
   H / (r_TR + r_BR))`, a term being skipped when its denominator is 0. This is
   the CSS `border-radius` rule: all four radii shrink by the same factor, so
   the arcs never overlap and the shape keeps its proportions. With four equal
   radii it gives the clamp the product has today (half the shorter side,
   `0003` criterion 5). Examples on a 100 × 40 mm rectangle: TL 30, TR 30,
   BR 0, BL 0 gives `f = 1`; TL 30, BL 30, TR 30, BR 0 gives
   `f = 40 / 60`, so the effective radii are 20, 20, 0, 20 (TR scales as well).
   The SVG rule (clamp `rx` to `W/2`, `ry` to `H/2`) cannot express neighbour
   interaction because SVG has one radius pair; the CSS rule is the standard
   one for several radii.
10. Given the stored radii, then the clamp is applied only where a radius is
    evaluated (outline, rendering, hit-testing, handle placement, selection
    box, "Object to path"), never written back: a rectangle shrunk so that
    `f < 1` and enlarged again has its stored radii back (the rule of
    `specs/0003-primitive-shapes/adrs.md`, extended to four radii). A radius
    drag or entry writes a value already limited by criteria 4 and 6, so it
    never produces `f < 1` by itself; only resizing and opening a file can.
    The one exception is the consequence of criterion 3: an unlinked drag or
    entry on a rectangle with `f < 1` writes the other three corners at their
    effective values (nothing visible changes), so their old stored radii are
    not restored by a later enlargement.
11. Given a corner whose effective radius is 0 within the geometric tolerance,
    then it is a sharp corner everywhere (outline, "Object to path" node
    count).

### Resize, rotate and the switches

12. Given the switch "Scale corner radius" (criterion 15) is on, when a
    rectangle is resized by any transform handle with horizontal and vertical
    factors `sx`, `sy`, then all four stored radii are multiplied by
    `√(sx·sy)` (the rule of `specs/0005-object-transform/` criterion 9, applied
    to each corner), then evaluated per criterion 9; the radii keep their
    ratios to each other (the factor is applied to the stored values, not the
    effective ones), and a radius driven to 0 is a valid sharp corner.
    Ctrl (proportional) and Shift (about the centre) change nothing here.
    Given the switch is off, criterion 15 applies.
13. Given the "Scale stroke width" switch in either state, then it has no
    effect on the radii, and the radii have no effect on the stroke; the two
    switches are independent of each other (`0005` criterion 31, stroke part).
14. Given a move, a rotation or a numeric size entry (W and H), then the radii
    follow criteria 12 and 15 for a size entry, with the switch read when the
    entry opens, the same moment `specs/unified-object-editing/` criterion 23
    fixes for a typed size (the equivalent drag reads it at the press,
    `specs/object-transform-refinements/` criterion 27; a click on the switch
    closes an open entry without writing), and are unchanged by a move or a
    rotation. The radius handles follow the rectangle's rotation
    (`specs/unified-object-editing/` criterion 4).
15. Given the Select tool is active, then its top bar shows a switch "Scale
    corner radius" next to "Scale stroke width", visible and operable with or
    without a selection, **off** when the program starts. It is per-session UI
    state of the same class as "Scale stroke width" (`0005` criteria 28 and
    30): never written to the project, not restored when a project is reopened,
    read once at the press that starts a resize drag (a change of the switch
    mid-drag applies from the next drag), and switching it writes nothing.
    Given it is off when a resize drag begins (or a size is typed), then all
    four stored radii are exactly as before the drag, so every corner keeps
    its absolute size in millimetres; the effective radii shrink only where
    criterion 9 requires it and come back when the rectangle is enlarged
    (criterion 10). Given it is on, criterion 12 applies. Example: a 100 × 40 mm
    rectangle with four radii of 5 mm, resized with `sx = sy = 2`: off gives
    200 × 80 mm with radii 5, 5, 5, 5; on gives radii 10, 10, 10, 10; with
    `sx = 2`, `sy = 1`, on gives `5·√2 = 7.07` mm for each.

### Object to path

16. Given a rectangle, when "Object to path" runs, then the path starts at the
    end of the top-left corner's arc (or at the top-left corner point if it is
    sharp) and runs clockwise on screen. Each corner with a positive effective
    radius contributes two nodes joined by one cubic Bézier arc using the
    quarter-circle control ratio (kappa, about 0.5523; deviation from the true
    quarter circle at most 0.1 % of that corner's radius); each sharp corner
    contributes one node. The path therefore has 4 to 8 nodes, all of the
    Corner kind, joined by straight segments between corners. With one radius
    for all corners the result is the same as today (4 nodes sharp, 8
    rounded), so `0003` criterion 18 holds. A straight segment of length 0
    (two arcs meeting) stays, as today; removing it is not part of this spec.

### Persistence and compatibility (document model)

17. Given a rectangle with different radii, when the maker saves, closes and
    reopens the project, then the four radii are exactly as saved, the object is
    still a rectangle, and its rotation, size and other parameters are
    unchanged.
18. Given a project saved by the previous version with a rounded or sharp
    rectangle (the single legacy `corner_radius`), then it opens in this version
    as a rectangle with four equal radii and renders and converts to a path
    exactly as before (within the geometric tolerance). The legacy value is read
    as the radius of all four corners, is never written again, and the file is
    not rewritten on open.
19. Given any project saved by this version, when an older build opens it, then
    it is refused with the "saved by a newer version" message
    (`project-file-foundation`, `OpenError::FormatTooNew`), never shown with
    wrong corners. `format_version` increases once: to 6 if this feature merges
    first, otherwise `main`'s current number plus one (the number is
    provisional, renumbered at merge as for every earlier bump, with an empty
    migration from version 5). This holds for every file the new build saves,
    not only those with per-corner radii.
20. Given a file whose radius is negative, not finite or otherwise invalid,
    then it is refused as damaged with `OpenError::Damaged`, not clamped
    silently (the validation rule of `0003`). A stored sum of radii above a
    side is not damaged (criterion 9 shrinks it on evaluation; a merge of two
    peers' edits can produce it).
21. Given any selected rectangle, then hit-testing, the selection box and the
    marquee use the effective outline of criterion 9.

### Mixed display and hint texts

22. Given one selected rectangle whose four effective radii are not all equal
    (within the geometric tolerance), then the Radius field in the Select bar
    is empty with the muted placeholder "Mixed", as for several objects, and
    its tooltip reads "Corner radius. Typing sets all four." followed by a
    second line with the four effective values in the rectangle's own frame,
    for example "Top-left 12, top-right 0, bottom-right 12, bottom-left 0 mm."
    (rounded to the field's precision), and the line "Zoom in to change one
    corner." Typing a value and Enter writes it to all four corners (limited to
    half the shorter side), ignoring the Link switch and Shift. When the four
    effective radii are equal the field shows that value, with the tag "limited"
    only when a stored radius is above its effective one (tooltip "Stored 20 mm,
    limited to 15 mm by the size; enlarging brings it back."); Mixed shows no
    tag.
23. Given the pointer rests on a radius knob for 600 ms, then the hint chip
    (DOM text, as for the transform handles) shows three lines. With the switch
    on: "Corner radius, all four", "Shift: this corner only", "Double-click:
    type a value". With the switch off: "Corner radius, this corner", "Shift:
    all four corners", "Double-click: type a value". A corner whose stored
    radius is above its effective one adds a first muted line "Limited by the
    size. Stored 30 mm, shown 20 mm." (values of that corner) and, when the
    next drag of that knob would be unlinked, the line "Editing one corner
    fixes the other three at their shown size." Followers: during a drag that
    changes all four corners (decided at the press, criterion 3), the other
    three knobs take the hover ground; during a one-corner drag they stay idle
    and in place, and the dashed guide runs to the dragged knob only. No hint
    names a corner and no hint tracks Shift live.

## Out of scope

- Remembering the "Scale corner radius" switch between sessions or per
  project (it is session state like "Scale stroke width"), a per-object flag.
- Elliptical corners (a different horizontal and vertical radius per corner,
  Inkscape's Rx/Ry), corner styles (chamfer, inverted or concave, smoothing
  such as Figma's), rounding of polygons and stars.
- A radii form in a Properties panel, copying radii between rectangles,
  keyboard nudging.
- A per-corner field or a four-field form in the Select bar: below 72 px on
  screen no knob is drawn, so one corner cannot be set without zooming (the bar
  field sets all four).
- A live indication of Shift on hover (the hint names what Shift does relative
  to the switch, not the current key state).
- A persistent mark of the link state on the canvas at rest (the bar's toggle
  and the knob hint show it).
- Writing a rectangle with different radii to SVG as `<rect rx ry>`: it cannot
  be expressed there and `svg-import-export` must export it as a path
  (noted for that slice).
- Undo and redo (`undo-redo` slice).

## Questions for the customer

1. **How do you change one corner?** (a) A visible "Link corners" switch
   (default on: dragging any corner changes all four) plus Shift to invert it
   for one drag (default, recommended; Alt cannot be used because Alt starts
   the freehand selection line); (b) switch only; (c) the switch starts off,
   so a handle changes its own corner and you switch "Link corners" on for
   uniform rounding (closer to "each handle is its corner"). The default
   follows Figma and Illustrator and the present behaviour.
2. **Elliptical corners.** Needed? (a) No, circular per corner (default,
   recommended); (b) yes, a separate horizontal and vertical radius per corner
   (a bigger model and handle change).

Unanswered: the defaults, 1 (a) and 2 (a), apply and are what the criteria
above specify. Shift is an exclusive-or with the switch (criterion 3).

Answered: the "Scale corner radius" switch (customer, 2026-10-06: yes;
criterion 15, default off as in Inkscape and Illustrator, a change from
today's always-proportional resize, see "Changes to accepted behaviour").
Confirmed by the customer (2026-10-06): the default is off. Resolved; nothing
depends on it any more.

## Notes for the architect (document-model change)

Answered in `adrs.md` (decisions 1 to 11, 2026-10-07): four flat registers, a
legacy fallback per corner, `format_version` 6 (provisional), the CSS clamp in
one function. The questions below are kept as the record of what was asked.

The stored `Shape::Rect { bounds, corner_radius }` (one `corner_radius` LWW
register, stored raw, clamped on evaluation) has to carry four radii. Questions:

1. Fields: four registers (`corner_radius_tl`, `_tr`, `_br`, `_bl`) or one
   list register? Recommendation: four registers, so two peers editing
   different corners merge without losing either edit (ADR 0009 last-writer-
   wins is per register).
2. Legacy: read an absent per-corner set as the old `corner_radius` for all
   four, never rewrite in place (precedent: the legacy `smooth` tag read as
   `Symmetric`); what writers do when both are present.
3. `format_version`: increment to the next free number at merge time
   (`main` is at 5; `specs/0007-stroke-and-fill-styling/` already plans the
   next one), with a dated note in this feature's `adrs.md`; the empty
   migration for old files (criterion 18).
4. API: `corner_radius: Length` becomes a `CornerRadii` newtype (four lengths);
   `effective_corner_radius` becomes a function returning four effective radii
   by the CSS rule; `Document::set_corner_radius` keeps a "set all" form and
   gains a per-corner form; every consumer (outline, render, hit test, handle
   layout, "Object to path", `oriented_bounds`) goes through the one
   evaluation function (the rule already in `primitive_outline.rs`).
5. Validation on open: each radius finite and at least 0, else damaged
   (criterion 20); the rule that a rectangle with all four radii equal is
   stored the same way as any other (no special "uniform" encoding).
6. Corner order TL, TR, BR, BL in the local frame, matching the outline's
   direction; golden fixtures: an outline with mixed radii, a `.curvyo` before and
   one after the bump (`CLAUDE.md` §5).
7. Whether the field addition needs a full ADR or a feature-local note;
   `CLAUDE.md` §3 puts document-model ADRs to the customer.

## UX notes

Status: complete (`ux-engineer`, 2026-10-07). Sizes and tokens live in
`docs/design-system.md` (rows "Parameter handle", "Parameter handle layout",
"Select bar layout", "Bar number field", "Limited tag" and the new "Link
corners toggle"); this section gives the decisions and the reasons. Distances
are screen pixels; `s` is the shorter side of the oriented box on screen; `ρ`
is as in the design system (`ρ = effective radius / (s/2)`). Where a decision
departs from an architect default in `adrs.md` it says "ADR default:".

Already decided in `specs/unified-object-editing/` UX notes (2026-10-06) and
kept: the bar's layout (the two Scale switches first, never dimmed or disabled;
the kind groups after them), the knob glyph, the 72 px threshold, the 12 px hit
radius, clearance of four knobs and the centre handle (its section 1), the hint
chip, "Shift: this corner only", the "r 12.0 mm max" readout. This section
corrects one claim of that section 1 (see 2) and decides what it left open.

### 1. The "Link corners" toggle (criteria 2, 3, 6, 8)

- **Look.** Icon only, a 28 x 28 px button, no text label. Glyph: Lucide `Link`
  (linked) and `Unlink` (unlinked), 16 px, 1.5 px absolute stroke. **On
  (linked, the default):** `--toolbar-icon-active-bg` fill, `--toolbar-icon-active-fg`
  glyph (same as the active tool button, 4.7:1), closed chain. **Off:**
  transparent ground, `--toolbar-icon` glyph (8:1 on `--toolbar-bg`), broken
  chain. State is carried by the glyph shape and the fill, never by colour
  alone. Hover (off): `--editor-accent-hover` behind it; hover (on): no change,
  as on a pressed tool button. Focus-visible: 2 px `--editor-accent` ring, 1 px
  `--toolbar-bg` offset, as the switches. `rounded-md`, no motion.
  *Why no label:* it is an attribute of the Radius field next to it, the chain
  icon is the established vocabulary (Figma, Illustrator, Affinity), and a text
  label would add about 140 px to a row that already wraps at about 760 px.
  The cost is discoverability; the knob hint chip (section 6) names the state
  in words, which is where the maker is looking when the question arises.
- **Slot.** The reserved 28 px slot in the **rectangle group**, between the
  "Radius" field and "Remove rounding", 12 px gap each side. Not in the
  settings group: the two Scale switches are the settings that act on a resize,
  always shown, and the group must not change when the selection changes; this
  toggle acts on the corner handles and the rectangle group is where those
  controls live. It follows the common rule: **shown when the selection contains
  a rectangle, never disabled** (a setting is operable before the maker needs
  it; with several rectangles or a rectangle under 72 px the handles are not
  drawn, so the tooltip says what it applies to). It adds 40 px (28 + 12) to the
  rectangle row; the implementer measures the single-row width and writes it
  into the "Select bar layout" row.
- **Tooltip** (Radix, 400 ms, `side="bottom"`, text changes with the state).
  On: "Link corners: on. A corner handle sets all four radii. Hold Shift to
  change one corner." Off: "Link corners: off. A corner handle changes its own
  corner. Hold Shift to set all four." Second muted line in both: "Applies to
  the corner handles of one selected rectangle. The Radius field always sets
  all four."
- **Keyboard.** One Tab stop, order: Radius field, Link corners, Remove
  rounding. Space and Enter toggle (it is a button). `aria-pressed`,
  accessible name "Link corners" (Radix `Toggle`; the visible state is the
  pressed look). No letter shortcut (letters are reserved by the keyboard
  concept; nothing in usage asks for one).
- **State meaning.** Session UI state of the class of the two Scale switches:
  `linked` by default, never written to the project, **reset to linked by New
  and Open** (same as the switches, `0005` criterion 27; the PO aligns the
  wording of criterion 2, "kept for the session"). Toggling writes nothing and
  changes no radius. It does not change any drawing on the canvas at rest: the
  state is shown in the bar (the toggle) and in the knob hint (section 6).
  **ADR default kept:** a click closes an open entry chip without writing, like
  the other two switches; the open entry keeps the state it was opened with.
- **Shift: judged, XOR instead of OR (ADR default: OR).** A drag or an entry
  is unlinked when the switch and Shift **differ**: switch on and no Shift =
  all four; switch on and Shift = this corner only; switch off and no Shift =
  this corner only; switch off and Shift = all four. Reasons: (a) one rule the
  maker can state ("Shift inverts the toggle for one drag"), the wording of
  criterion 3's parenthetical and question 1 (a), and the convention of
  Illustrator's and Figma's proportion locks; (b) with OR, Shift is a dead key
  in the unlinked state and a maker who works unlinked has no quick way to set
  all four; (c) the architect's objection (a second hint) is answered by a hint
  that names what Shift does *now*, one line either way (section 6). Cost: one
  boolean inequality instead of an OR, and Shift on an unlinked rectangle can
  overwrite four radii, which is what the maker asked for with that key.
  Read once at the press (the entry: at the second press) and frozen; a Shift
  pressed or released mid-drag changes nothing, so the other corners never
  move under the maker's hand (criterion 3). If the architect keeps OR, only
  the hint line "Shift: all four corners" disappears.

### 2. Handle clearance when radii differ: the diagonal pair (flag 1; criterion 1)

The claim in `unified-object-editing` UX notes section 1 and in the design
system that an unlinked corner "cannot break" the clearance is **true for
adjacent corners and false for diagonal ones** (checked: architect decision 9).
Two diagonal knobs (TL/BR, TR/BL) have no radius limit between them; TL = BR =
0.6 s on a square is valid and puts both knobs on the same diagonal, where
they overlap (their offsets from their own corners add up to more than the
side).

**Decision: cap the displayed position, never the value.** A knob is drawn at
`ρ'_i = min(ρ_i, max(1, Σ − ρ_j))`, `j` the diagonal partner, with
`Σ = 2 + √2·(S − s)/L(s)` (`S` the longer side on screen, `L(s)` as in the
design system); on a square `Σ = 2`, so this is the architect's rule
`min(ρ_i, max(1, 2 − ρ_j))`. Stored and effective radii, the outline, the
readout and the bar field are untouched; only the circle is drawn nearer the
corner. Properties checked:

- Two diagonal knobs are at least 14 px apart along the longer side of the box
  (on a square: along both axes), so the distance is at least 14 px and the
  glyph gap at least 4 px (19.8 px and 9.8 on a square when `ρ'` sums to
  `Σ`), at `s` = 72 and above, for every pair. Adjacent pairs keep the
  guarantee of the unified spec (their radii on a shared side sum to at most
  that side, the map is linear: 14 px on the shorter side, more on the longer).
  The 72 px threshold, the tier table and the 12 px hit radius are unchanged.
- A knob at or below `ρ = 1` is always exactly at its radius, whatever the
  other corners are: the cap only applies to a corner above half the shorter
  side whose diagonal partner is also large. Where both exceed 1 on a square
  both rest at the `ρ = 1` position.
- A lone corner still reaches `ρ = 2` (the CSS limit with three zeros: the
  radius equals the shorter side), so the "leaf" and "finger hole" shapes are
  unaffected.
- `Σ` instead of a flat 2 (**departure from ADR default**): on a 180 x 72 box a
  flat 2 would pull two knobs back to `ρ = 1` although they are 80 px apart.
  The change is one term in `knob_rho` (it takes the box's longer side as well);
  if the architect prefers the flat 2, the only cost is a few false lags on
  wide boxes with both diagonal corners above half the shorter side.
- **What the maker sees:** while the knob is capped it stays put and the radius
  keeps changing (the readout and the blue outline show it), the same visible
  rule as a handle that has reached a limit. The partner can give way too: with
  one corner at `ρ = 1.6`, dragging its diagonal partner out moves that knob
  inward from `ρ = 0.4` of the partner on. Rare, and consistent: two knobs on
  one diagonal never overlap. Dragging a capped knob back inward starts from
  the true radius, so the knob stays still until the radius falls below the
  cap; this is the one place where "the handle stays under the pointer" does
  not hold, and it holds again as soon as the cap is left.
- The guide line is drawn to the capped position (the line joins the corner to
  the knob that is drawn).

Corrected design-system text: row "Parameter handle layout" (done in this
change).

### 3. During an unlinked drag (criteria 3, 4, 7)

- **Followers** take the hover ground only while the drag changes all four
  corners (decided at the press, so by section 1's rule). In a one-corner drag
  the other three knobs stay idle (white ground, in place: their radii do not
  change). The dragged knob is solid `--accent` with the white dot in both. The
  dashed guide runs to the dragged knob only. Nothing else on the canvas shows
  the link state at rest; no persistent badge, because the bar is always in
  view while a rectangle is selected.
- **Linked drag on unequal radii.** The dragged knob stays under the pointer
  (the drag starts from its own effective radius) and the other three jump to
  the same value at the first movement. That jump is the meaning of "linked",
  it is shown (followers highlighted, blue outline with four equal corners
  before the release), and Escape cancels the drag with nothing written. There
  is no undo yet, so the readout adds the line below.
- **Readout** (existing readout chip, at the pointer): one corner "r 12.3 mm";
  all four "r 12.3 mm" when they were equal at the press, and
  **"r 12.3 mm · all corners"** when the press found unequal radii (the drag
  overwrites them: say so before the release). At a limit, "r 12.0 mm max" for
  both modes (the unified UX note had it for an unlinked drag; a linked drag
  that stops at half the shorter side says the same). The two suffixes join
  with " · " ("r 12.0 mm max · all corners"). The pointer sits on the dragged
  corner's knob, so no corner name is needed.
- **Limit and clamp feedback.** The unlinked limit `min(W − r_h, H − r_v)` is
  computed from the neighbours' effective radii, so the CSS factor stays at
  `f = 1` for the whole drag and nothing shrinks under the maker's hand; the
  knob stops, the radius does not move, "max" appears. The maker therefore
  never sees `f` change during a drag. What is shown is always the effective
  value: knob position, readout, blue outline, and the field in the entry chip.
  **"limited" per corner** (a stored radius larger than the corner can have
  after a resize): the bar's tag covers a Uniform field only (architect 8); for
  one corner it is the knob's hint chip (section 6), which says "limited" and
  gives the stored value. There is no mark on the canvas at rest.
- **Live preview.** Unchanged: `--preview-new` outline (1.5 px, constant), the
  black original underneath. In a one-corner drag only the dragged corner
  differs between the two, which is the clearest signal that the others are
  fixed; no extra overlay.

### 4. Mixed, the entry chip and Remove rounding (criteria 6, 8; bar field)

- **Bar Radius field with unequal corners:** empty with the muted placeholder
  "Mixed", exactly as for several objects; four values do not fit the 112 px
  field and would read as four fields. The tooltip carries the numbers: line 1
  "Corner radius. Typing sets all four." line 2 "Top-left 12, top-right 0,
  bottom-right 12, bottom-left 0 mm." (effective values, the rectangle's own
  orientation, rounded to the field's precision). Typing a value and Enter
  writes all four (the field is the explicit "set all" route and ignores the
  Link toggle and Shift). The tooltip also gains: "Zoom in to change one
  corner." (below 72 px there is no per-corner route: a four-value form is out
  of scope, see "Known gaps").
- **"limited" tag:** Uniform value only, as the architect has it (stored value
  of a corner above its effective one; tooltip "Stored 20 mm, limited to 15 mm
  by the size; enlarging brings it back."). Mixed shows no tag.
- **Typed entry chip for a corner.** Opens at the knob as the unified entry
  chip does: one field "r", 80 px wide, `mm` suffix, with a second row (6 px
  below, 12 px muted text, as the Move chip's mode row) that names the scope
  fixed when the chip opened: **"All four corners"** or **"This corner only"**.
  No second field and no corner picker; the knob you double-clicked is the
  corner. Value shown: that corner's effective radius. Enter writes per
  criterion 6; a value above the limit is applied at the limit and the readout
  chip "r 12.0 mm max" shows at the knob for 1.5 s (a clamp must not be
  silent); Escape, invalid, untouched Enter as in the unified spec. Accessible
  names of the field: **linked "Corner radius, all corners"; unlinked
  "Top-left corner radius", "Top-right corner radius", "Bottom-right corner
  radius", "Bottom-left corner radius"** (the criteria's own frame). The bar
  field keeps "Corner radius".
- **Remove rounding:** all four radii of every selected rectangle to 0 in one
  commit; independent of the Link toggle and Shift; enabled when some selected
  rectangle has some stored radius above the sharp tolerance (architect 8).
  Tooltip "Remove rounding of the selected rectangles (all corners)". One
  corner is made sharp by dragging its knob to zero (criterion 5) or typing 0.

### 5. Editing a shrunk rectangle without a surprise (flag 2; criteria 3, 10)

A rectangle shrunk with "Scale corner radius" off can have `f < 1`: the stored
radii exceed what the size allows and only the effective ones are drawn. An
unlinked edit writes the other three corners at their effective values, because
the maker sees those values and criterion 3 promises they stay. **Does the maker
see a jump? No.** Knobs, outline, readout and the entry chip all start from the
effective values and nothing on screen moves at the press or the release. The
surprise is delayed: enlarging the rectangle later no longer restores the old
stored radii of the three neighbours (the "enlarging brings it back" promise of
the "limited" tooltip holds only until such an edit). Decision: accept it, state
it where the maker decides, add no dialog. Where: the hint chip of a knob whose
corner is limited and whose next drag is unlinked (section 6) says "Editing one
corner fixes the other three at their shown size". A linked edit needs no note
(it writes all four anyway). Rejected: writing only the dragged corner (the
other three would change on screen, against criterion 3); writing the
effective values silently and nothing more (the promise would break unseen).
The case needs a shrunk rectangle, the Keep setting and an unlinked edit; no
more UI is justified.

### 6. Names and hint chip texts (criteria 6, 7)

Same chip as the transform handles (DOM text, 600 ms, `pointer-events: none`).
Which lines depend on the toggle (Shift is not tracked live; the lines say
what Shift does relative to the toggle):

| Knob, toggle on (linked) | Knob, toggle off |
|---|---|
| "Corner radius, all four" | "Corner radius, this corner" |
| "Shift: this corner only" | "Shift: all four corners" |
| "Double-click: type a value" | "Double-click: type a value" |

A corner with a stored radius above its effective one adds a first muted line
"Limited by the size. Stored 30 mm, shown 20 mm." and, when the toggle is off
(or after Shift), the line "Editing one corner fixes the other three at their
shown size." No Ctrl line. The star's chip is unchanged.
Accessible names: bar field "Corner radius" (sets all four); toggle "Link
corners"; entry chip field per section 4; the canvas knobs are not in the
accessibility tree (known gap of the unified spec), so the bar and the chip are
the accessible route; the star's "Inner ratio" and the polygon's "Outer radius"
stay.

### 7. The toggle and the bar width (criterion 23 of the unified spec)

The toggle belongs to the rectangle group and the group is one unbreakable unit
(`flex-wrap` by whole groups): the toggle never wraps away from the Radius
field. Where the row does not fit, the rectangle group goes to row 2 with the
other kind groups and "Object to path", the settings group stays on row 1 (as
the unified UX note). The toggle adds 40 px to the single-row width. Tab order:
settings, Radius, Link corners, Remove rounding, then the rest. The toggle shows
and hides with the rectangle group; its state persists across that.

### 8. Changes the criteria need (for the PO)

1. **Criterion 1, last sentence:** delete the clearance claim based on the
   limit. Replace with: a knob is drawn at the position of its own effective
   radius, except that a knob above half the shorter side whose diagonal
   partner is also large is drawn nearer its corner so that two diagonal knobs
   never overlap (design-system row "Parameter handle layout"); values are not
   affected. (Flag 1.)
2. **Criterion 2:** the switch is reset to on by New and Open like the two
   Scale switches (replace "kept for the session"). Add its place and look:
   rectangle group, after the "Radius" field, icon button, tooltip and
   keyboard of section 1.
3. **Criterion 3:** reword to XOR: "the drag is unlinked when the switch is on
   and Shift is held at the press, or the switch is off and Shift is not held;
   with the switch off and Shift held all four corners change as when the
   switch is on". Read once at the press and frozen. Question 1 (a) keeps
   "Shift inverts it for one drag". (If the lead keeps OR: say "the switch is
   off, or Shift is held" and drop the parenthetical.)
4. **Criteria 3 and 10 (flag 2):** add the sentence "an unlinked drag or entry
   on a rectangle with `f < 1` writes the other three corners at their
   effective values; nothing visible changes at the press or release".
5. **Criterion 4:** add that the readout shows "max" when a limit stops the
   drag (also a linked drag at half the shorter side).
6. **Criterion 6:** the field has a second row "All four corners" / "This corner
   only", accessible names as in section 4, scope fixed when the field opens
   (switch and Shift at the second press, XOR); a value above the limit is
   limited and the "max" readout shows for 1.5 s.
7. **Criterion 7:** readout texts: "r 3.5 mm", "r 12.0 mm max", and "r 3.5 mm ·
   all corners" when a linked drag finds unequal radii.
8. **Criterion 8:** "switching it changes no radius and writes nothing" gains
   "and closes an open entry without writing"; Remove rounding ignores the
   toggle and Shift.
9. **New criterion (the bar's Radius field):** with one rectangle whose
   effective radii differ the field is empty with "Mixed" and a tooltip with
   the four values; typing sets all four. The "limited" tag only for a Uniform
   value.
10. **New criterion (hint and followers):** the knob hint chip lines of
    section 6 (state-dependent Shift line, limited line); followers take the
    hover ground only in a drag that changes all four corners.
11. **Out of scope, add:** a per-corner field or four-field form in the bar
    (below 72 px a single corner cannot be set without zooming), a live Shift
    indication on hover, a visible mark of the link state on the canvas at rest.

### Known gaps, deliberately not closed here

- Below 72 px on screen no knob is drawn, so one corner cannot be set without
  zooming; the bar field sets all four only. A four-value form belongs to a
  Properties panel.
- There is no undo yet: a linked drag or the bar field overwrites unequal
  radii for good once released. The readout suffix and Escape are the only
  protection; revisit with the `undo-redo` slice.
- The one-corner "limited" note exists only in the hover chip; a rectangle with
  `f < 1` shows nothing at rest.

## Links
Requirements: R-EDIT-002 (`docs/requirements.md`)
Builds on: `specs/unified-object-editing/` (must come first),
`specs/0003-primitive-shapes/` (criteria 3 to 6, 18),
`specs/0005-object-transform/` (criteria 9 and 31, radius parts superseded
for rectangles by criteria 12, 13, 15 here),
`specs/object-transform-refinements/`
PR:
