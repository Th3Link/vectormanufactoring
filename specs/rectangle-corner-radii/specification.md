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
   its limit in criterion 4): its handle then sits further along the diagonal,
   and the 4 px clearance of `specs/unified-object-editing/` criterion 8 still
   holds because the limit keeps the two radii on one side summing to at most
   that side's length.
2. Given the "Link corners" switch is on (Select tool setting, default on, kept
   for the session, never written to the project), when the maker drags any
   radius handle, then all four radii become the dragged value, limited to half
   the shorter side. Radii that differed before the drag are overwritten.
3. Given the switch is off, or Shift is held at the press (Shift inverts the
   switch for that drag), when the maker drags a radius handle, then only that
   corner's radius changes; the other three radii and effective radii do not
   change at any moment of the drag. Given the switch is on and Shift is held,
   then the drag is unlinked, as just said.
4. Given an unlinked drag, then the dragged corner's radius is limited to the
   largest value that leaves the three other effective radii unchanged:
   `min(W − r_h, H − r_v)`, where `W` and `H` are the box's width and height
   and `r_h`, `r_v` are the effective radii of the corner that shares the
   horizontal side and of the one that shares the vertical side with the
   dragged corner (for TL: TR and BL). The handle stops there; it does not
   overshoot and snap back. Given a linked drag, the limit is half the shorter
   side.
5. Given a radius drag, when the pointer returns to or beyond the handle's
   zero-radius position, then the radius (of the dragged corner, or of all four
   when linked) is exactly 0 and the corner is sharp.
6. Given a double-click on a radius handle, then a field "r" (accessible name
   "Corner radius", mm) opens as `specs/unified-object-editing/` criterion 18
   defines, and Enter applies the value to all four corners when linked or to
   that corner when unlinked (the state of switch and Shift at the second
   press, fixed when the field opens). A value above the limit of criterion 4
   (or half the shorter side when linked) is limited and the limited value is
   written. Zero is valid. Same cancel, invalid and untouched-Enter rules.
7. Given a radius drag, then a live readout "r 3.5 mm" at the pointer shows the
   dragged corner's radius; the blue preview and the black original of
   `specs/unified-object-editing/` criteria 10 to 13 apply.
8. Given "Remove rounding" (`specs/unified-object-editing/` criterion 21), then
   all four radii of every selected rectangle become 0 in one commit. Given
   the switch "Link corners", then switching it changes no radius and writes
   nothing.

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
11. Given a corner whose effective radius is 0 within the geometric tolerance,
    then it is a sharp corner everywhere (outline, "Object to path" node
    count).

### Resize, rotate and the switches

12. Given the switch "Scale corner radius" (criterion 15) is on, when a
    rectangle is resized by any transform handle with horizontal and vertical
    factors `sx`, `sy`, then all four stored radii are multiplied by
    `√(sx·sy)` (the rule of `specs/0005-object-transform/` criterion 9, applied
    to each corner), then evaluated per criterion 9; the radii keep their
    ratios to each other, and a radius driven to 0 is a valid sharp corner.
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
    rectangle, then it opens in this version as a rectangle with four equal
    radii and renders and converts to a path exactly as before (within the
    geometric tolerance). It is not rewritten on open.
19. Given a project saved after this change that contains per-corner radii,
    when an older build opens it, then it is refused with the "saved by a newer
    version" message (`project-file-foundation`), never shown with wrong
    corners. The `format_version` increases (the architect names the number
    at merge time).
20. Given a file whose radius is negative, not finite or otherwise invalid,
    then it is refused as damaged with a named error, not clamped silently
    (the validation rule of `0003`).
21. Given any selected rectangle, then hit-testing, the selection box and the
    marquee use the effective outline of criterion 9.

## Out of scope

- Remembering the "Scale corner radius" switch between sessions or per
  project (it is session state like "Scale stroke width"), a per-object flag.
- Elliptical corners (a different horizontal and vertical radius per corner,
  Inkscape's Rx/Ry), corner styles (chamfer, inverted or concave, smoothing
  such as Figma's), rounding of polygons and stars.
- A radii form in a Properties panel, copying radii between rectangles,
  keyboard nudging.
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

Answered: the "Scale corner radius" switch (customer, 2026-10-06: yes;
criterion 15, default off as in Inkscape and Illustrator, a change from
today's always-proportional resize, see "Changes to accepted behaviour").
Confirmed by the customer (2026-10-06): the default is off. Resolved; nothing
depends on it any more.

## Notes for the architect (document-model change)

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

(filled in by ux-engineer before Ready)

Decided in `specs/unified-object-editing/` UX notes (2026-10-06): the bar's
layout (the two switches first, never dimmed or disabled; the "Radius" field
and a reserved 28 px slot for the "Link corners" toggle in the rectangle
group), clearance of four radius handles and the centre handle (its section 1),
the hint line "Shift: this corner only", and the "r 12.0 mm max" readout when an
unlinked drag stops at its limit.

Still open for the `ux-engineer` when this spec is built: the look of the "Link
corners" toggle (a chain icon in the reserved slot) and the corner-linking
state shown on the handles of an unlinked drag.

## Links
Requirements: R-EDIT-002 (`docs/requirements.md`)
Builds on: `specs/unified-object-editing/` (must come first),
`specs/0003-primitive-shapes/` (criteria 3 to 6, 18),
`specs/0005-object-transform/` (criteria 9 and 31, radius parts superseded
for rectangles by criteria 12, 13, 15 here),
`specs/object-transform-refinements/`
PR:
