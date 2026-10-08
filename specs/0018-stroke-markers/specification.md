# Stroke markers: arrows and dots at the start, end, along and on the nodes of a path

Status: Ready
Priority: Should
Origin: Customer

Ready (2026-10-08): the criteria below are complete and testable, the
architect's `adrs.md` exists and the ux-engineer's "UX notes" are filled in
(`CLAUDE.md` §4); their requests (UX notes section 5, `adrs.md` "Flagged to the
lead") are applied to the criteria. It builds on `style-panel-rework` (panel
layout, value field, no popups, Paint None hides the rest) and must be scheduled
after it.

## User value

As a maker I want to put an arrow or a thick dot at the start, at the end, at
a number of evenly spaced places along, or on every node of a path, so that I
can mark direction, cut order, hole positions and dimension lines in my drawing
without drawing the marks by hand.

**Reference tools.** Inkscape sets a start, a mid and an end marker from three
dropdowns in Fill & Stroke. Its mid marker sits on every node of the path; there
is no "N markers spread along the path". Its arrow stays at the path direction at
the start unless the marker is made with `auto-start-reverse`, so a start arrow
often points the wrong way and the usual fix is to reverse the path. Markers
take the stroke colour in current Inkscape versions (`context-stroke`). What we
do better: evenly spaced mid markers by a count, a start arrow that points
outward by default, a marker that never darkens where it overlaps its stroke,
and all choices in the panel without a dropdown.

**What markers are.** Part of the stroke style of a path. They are decoration
that follows the geometry: they are not objects, not selectable, and not
geometry for any later cut or engrave job (see the open question on
manufacturing).

## Acceptance criteria

### Shapes, slots and defaults

1. Given the Stroke section of the panel, with the stroke on and the selection
   containing at least one path, then a Markers group shows, under Cap, a title
   "Markers" and three rows labelled **Start**, **Middle** and **End**. Each row
   is a choice of **None**, **Arrow** and **Dot** (an inline icon group like Join
   and Cap, with those three names as accessible names; no dropdown, no popup).
   The default of every slot is None.
2. Given a Middle shape other than None, then two more controls appear under it:
   **Place**, a text group with the choices **Spaced** and **At nodes** (default
   Spaced), and, only while Spaced is chosen, **Count**, a value field
   (`style-panel-rework` criteria 34 to 45 and 61) labelled "Count", with no unit
   and a linear scale: whole numbers from 1 to 500 typed, drag range 1 to 50,
   default and reset value 1. Given the Middle slot is None, then Place and Count
   are not shown. Stored Place and Count are kept while hidden, and are kept while
   Count is hidden by At nodes.
3. Given a stroke with Paint None or width 0, then the Markers group is not
   shown and no marker is drawn (`style-panel-rework` criterion 5). The marker
   settings are kept and come back with the stroke.
4. Given a Start, Middle or End choice, then the shape is a **filled arrow**: an
   isosceles triangle, length 4 x w and base width 3 x w, or a **filled dot**: a
   circle of diameter 3 x w, where w is the object's current stroke width in mm.
   Both have their centre (for the arrow, the middle of its length) on the anchor
   point of the slot. Example, w = 0.5 mm: arrow 2 mm long and 1.5 mm wide,
   dot 1.5 mm across. Changing the width resizes every marker at once.
5. Given a marker, then its colour is the stroke's RGB at the stroke's alpha.
   Where the marker and the stroke overlap, the colour is applied once: with a
   black stroke at 50% opacity, a pixel in the overlap has the same value as a
   pixel in the marker alone (+/- 1 of 255). Markers have no outline of their own.
6. Given a path with the Start, Middle and End slots set, then markers of
   different slots can differ (for example Start Arrow, Middle Dot, End Arrow) and
   the maker changes one slot without affecting the others.

### Where the markers go

7. Given an open path with at least 2 nodes, then the Start marker is anchored on
   the first node, the End marker on the last node, in the order the nodes are
   stored (the direction of drawing).
8. Given an open path with Middle set to Spaced and Count N, then N markers are
   anchored at the arc-length fractions `k / (N + 1)` of the total path length,
   for k = 1 to N. Arc length is measured along the path as drawn, curves
   included. Test: a straight 100 mm path with N = 3 has markers at 25, 50 and
   75 mm (+/- 0.05 mm); a curve of 200 mm with N = 1 has its marker at 100 mm of
   arc length (+/- 0.5 mm). The count is for the whole path, not per segment.
9. Given an open path with Middle set to At nodes, then one marker is anchored on
   each node except the first and the last, so the Start and End slots do not
   double up at the ends. A path with 2 nodes has none. The Count is not used.
10. Given a closed path, then it has no start and no end: the Start and End slots
    draw nothing, as Cap draws nothing on a closed stroke (`0007` criterion 12),
    and their settings stay stored. Middle Spaced with Count N anchors N markers at
    the fractions `k / N` of the total length, k = 0 to N - 1, counted from the
    first node (so N = 1 is one marker on the first node). Middle At nodes anchors
    one on every node including the first. Test: a closed square of 40 mm sides,
    first node at a corner, N = 4: markers on the four corners.
11. Given a path with fewer than 2 nodes, or whose total length is below
    0.001 mm, then no marker is drawn for it in any slot. Given zero nodes, the
    path draws nothing, as before.
12. Given a marker anchor, then dashes, gaps, caps and joins do not move or hide
    it. A marker is drawn even where a dash pattern has a gap at that point.
13. Given the same object, then its markers are derived from its geometry on every
    draw. Moving a node, adding or deleting a node, or transforming the object
    moves, adds or removes markers to match; a transform never skews or stretches
    a marker (its shape and size come from w only).

### Direction

14. Given a marker anchor, then **direction of travel** is the direction of the
    path at the anchor, following the node order. At a point inside a segment it
    is the curve's tangent there. On the first node it is the outgoing tangent of
    the first segment, on the last node the incoming tangent of the last segment.
    On a node inside the path it is the bisector of the incoming and outgoing
    tangents (the unit vectors added, normalized); if the two point in opposite
    directions (within 1 degree of a reversal), the outgoing tangent is used. A
    tangent with a zero-length handle uses the direction to the next control point
    or node that differs from the anchor. On a closed path the node's incoming
    tangent comes from the closing segment.
15. Given an Arrow, then its tip points along the direction of travel for Middle
    and End. For **Start it points the opposite way**, outward from the path, so
    a Start and an End arrow on a line make a double-headed arrow. A Dot has no
    orientation. Example: a path from (0, 0) to (100, 0) mm, w = 0.5 mm, has an End
    arrow with its tip at x = 101 mm and its base at x = 99 mm, and a Start arrow
    with its tip at x = -1 mm.
16. Given the path (0, 0), (10, 0), (10, 10) mm with Middle Arrow At nodes, then
    the arrow on (10, 0) points along (1, 1), at 45 degrees. Given a smooth node,
    then the arrow lies along the common tangent.

### Draw order, hit-testing, selection

17. Given an object with a fill and markers, then it paints its fill, then its
    stroke and markers together, in the object's place in tree order
    (`0007` criterion 26). A marker is never above a higher object and never below
    the object's own fill.
18. Given a click or hover on a marker, where the point is farther from the path's
    outline than the Select tool's outline tolerance, then nothing is hit: markers
    do not take part in hit-testing, in the selection box, or in the bounds that
    resize and rotate handles use. A marker may therefore stick out of the
    selection box.
19. Given the Select tool preview of a move, resize, rotate or skew, then it stays
    the hollow blue outline of the new geometry (`0007` criterion 36); markers
    are not drawn in it. The black committed object keeps its markers at the old
    place until release.
20. Given the eyedropper of `style-panel-rework`, then markers are not pickable on
    their own: a point on a marker that is not also on a stroke or fill by the rule
    of its criterion 24 picks nothing.

### Which objects

21. Given the selection holds only primitives (rectangle, ellipse, polygon, star),
    then the Markers group is not shown. A primitive has no markers and draws
    none. **Default, open question 1.**
22. Given a selection of paths and primitives together, then the Markers group is
    shown for the paths; an edit applies to the paths in the selection and changes
    nothing on the primitives. One commit.
23. Given "Object to path" on a primitive, then the new path has no markers.
24. Given several selected paths with different settings, then a slot shows no
    choice pressed, a Count field shows "Mixed", and a choice or value applies to
    all selected paths, each keeping its other settings. One commit.

### Copy, split, join, persistence

25. Given a path with marker settings, when the maker copies it (Ctrl-drag copy
    or typed copy), then the copy has the same settings; editing one leaves the
    other unchanged (`0007` criterion 30).
26. Given an open path with markers, when the maker splits it at a node, then both
    halves keep the settings, and the split node becomes the End of the first half
    and the Start of the second: each shows its End and Start marker there.
    Splitting a closed path keeps the one object and its settings (`0007`
    criterion 31).
27. Given two paths joined at their endpoints, then the surviving path keeps its
    settings and the other's are dropped with it (`0007` criterion 32); the joined
    node is now an inner node, so it gets a Middle At nodes marker if that is set
    and no End marker. A path closed onto its own other end keeps its settings,
    and its Start and End markers stop drawing (criterion 10).
28. Given a path with any marker settings, when the maker saves and reopens the
    project, then all settings read back as set, and the markers draw the same.
29. Given a project file written before this feature (an older format version),
    then it opens with every slot None, Place Spaced and Count 1, and no change in
    what is drawn, including the gradient and dash fixtures of `style-panel-rework`.
    Given a file written by a later build (a higher format version), then it is
    refused as "saved by a newer version", the same as any such file (not as
    damaged), before any marker key is read. Given a file of this format version
    whose marker keys hold an unknown shape or Place value, or a Count that is not a
    whole number or is below 1, then it is refused as damaged on open, as an
    invalid join or cap is. A Count above 500 opens, is kept as stored (not
    rewritten by viewing), and at most 500 markers per slot are drawn.

### Panel behaviour

30. Given the Markers rows, then they follow `style-panel-rework`: icon groups
    commit on the click, Count follows the value field rules (drag with preview,
    one commit on release, Escape reverts, click to type), the rows are removed
    from the panel (not disabled) when the stroke is off or the selection has no
    path, and no popup opens.
31. Given a typed Count of 0, 2.5, 501 or text, then it is refused with focus kept
    and the message "Enter a whole number from 1 to 500", and nothing is written.
    Given a Count dragged to the end of its range, then it is 50; larger values
    are typed.
32. Given a Start or End shape other than None and every selected path closed
    (criterion 10), then one muted line under the End row reads "Closed paths have
    no start or end." It is not an error and no control is disabled. Given at least
    one selected path is open, or both Start and End are None, then the line is not
    shown.

## Out of scope

- More shapes than Arrow and Dot (open arrowheads, bars, diamonds, user-drawn
  markers).
- Marker size or colour set apart from the stroke (the size is fixed at 4 w and
  3 w; see the proposal in open question 4), marker rotation offsets, markers with
  an outline or a fill of their own.
- Markers on primitives (open question 1) and on text (no text tool).
- Markers spaced by a distance in mm instead of by a count.
- A different count per segment, or markers on selected nodes only.
- Markers in a laser, plotter, embroidery or CNC job: until `manufacturing-roles`
  decides, a marker is appearance only (open question 5).
- SVG import and export of markers (`svg-import-export`).
- Undo and redo (`undo-redo`).

## Open questions (customer; each has a default, nothing blocks)

1. **Primitives.** *A (default):* paths only; "Object to path" first. This
   departs from `0007` criterion 1 ("no property exists for one kind but not the
   other") on purpose, because a closed primitive has no ends and the useful cases
   (arrows on a line, dots on nodes) are path cases. *B:* primitives get markers
   too, as for a closed path (Middle only: spaced along, or on the corners).
   Rounded corners and ellipse nodes make "on the nodes" unclear. Recommendation: A.
2. **"On the nodes" and "N between".** *A (default):* they exclude each other: the
   Middle slot is either N evenly spaced or on the nodes, chosen by Place.
   *B:* both at once. Recommendation: A, simpler to read and to test.
3. **Direction of the Start arrow.** *A (default):* outward, so Start plus End give a
   double arrow. *B:* along the path as in Inkscape's older markers. Recommendation: A.
4. **Marker size.** Proposal, not in the criteria: a Size value field, as a
   multiple of the stroke width (default 4, as now), so a thin line can carry a big
   arrow. Needs one more stored value. Default: not included; the size is 4 w
   and 3 w. Recommendation: include it when you find the fixed ratio too limiting
   on thin lines.
5. **Markers in manufacturing.** Does an arrow or dot become something the machine
   cuts or engraves? Default: no. Appearance only, decided again in
   `manufacturing-roles`.
6. **When.** Priority is Should, not part of the laser MVP sequence.
   Recommendation: build right after `style-panel-rework`, because it needs that
   panel layout.

## Format

New stroke style values: Start, Middle and End shape (None, Arrow, Dot), Middle
place (Spaced, At nodes) and Middle count (whole number, 1 or more). Defaults as
above; an absent value is the default. This is a **format change: the
`format_version` goes up**. `main` is at 7; `style-panel-rework` takes 8 (odd dash
lists), so this feature takes 9, expected (by the merge rule, the number is the
next free one at merge). Key names: `adrs.md` decision 1. `advanced-selection` has no bump. `rectangle-corner-radii` and
`ellipse-arcs-and-shaping` (both Draft) also want one; the PR that merges first
takes the next number. A golden fixture holds a project with all values set,
another one with a version-7 file.

## UX notes

Decided 2026-10-08 (ux-engineer). Builds on the Style section of
`style-panel-rework` (rows, value field, hidden-not-disabled, no popups, instant
show and hide); sizes and tokens are in `docs/design-system.md`, "Markers
block". Fixed by this spec and not reopened: shapes and ratios (criterion 4),
outward Start arrow, no popup, rows removed while the stroke is off.

### 1. Layout

The block sits under Cap in the Stroke section, 8 px below it. Rows are 28 px
high with 8 px between them; the label column is 60 px, the control column
176 px, as in the rest of the section.

| Row | Control | Shown when |
|---|---|---|
| "Markers" (12 px semibold title, 16 px high) | none | the Markers group is shown (criteria 1, 3, 21) |
| Start | icon group None / Arrow / Dot, 3 x 40 px | with the title |
| Middle | the same | with the title |
| End | the same | with the title |
| Place | two-segment text group "Spaced" / "At nodes", 2 x 88 px | Middle is not None |
| Count | value field, full width (244 px), label "Count", no unit | Middle is not None and Place is Spaced |

Heights: the block is 132 px with Middle None (8 gap, 16 title, 8, then three
rows with 8 between), 204 px with Place and Count. The Stroke section is then
536 px or 608 px. These rows come and go instantly; the rows above never move,
the Fill section below does (that is the price of "removed, not disabled").
Tab order: Start, Middle, End, Place, Count (each group is one Tab stop;
arrows move and select inside it).

The three slots are three rows, not one row of three groups: one row would need
9 icon buttons in 244 px and the slot names would not fit. The title row is
there because "Start" alone, under Cap, does not say what starts.

### 2. Glyphs (preview in the buttons)

All glyphs are 16 px, drawn in `--toolbar-icon` (pressed: `--toolbar-icon-active-fg`)
with a 1.5 px absolute stroke, and show what the choice does to a line:

| Slot | None | Arrow | Dot |
|---|---|---|---|
| Start | a plain horizontal line | the line with a filled triangle at its left end, tip pointing left (away from the line) | the line with a filled circle at its left end |
| Middle | the plain line | the line with a filled triangle in the middle, pointing right | the line with a filled circle in the middle |
| End | the plain line | the line with a filled triangle at its right end, tip pointing right | the line with a filled circle at its right end |

Triangle 6 px long and 5 px wide, circle 5 px across: a drawing of the choice
in a fixed proportion, not scaled by the stroke width. The Start arrow pointing
outward shows the customer's rule (criterion 15) before it is tried. The shapes
differ by silhouette, and the pressed state is a ground and a glyph colour, so
nothing depends on colour alone.

### 3. States and behaviour

- **Default** None in every slot; pressing a pressed item does nothing (radio
  semantics, as Join and Cap). Choosing is one commit on the click; no preview.
- **Mixed** (several paths with different settings, criterion 24): nothing
  pressed in that slot, Place shows nothing pressed, Count shows "Mixed". A
  choice applies to all selected paths. A mixed Middle (some None, some not)
  shows Place and Count, with their own mixed state; the objects whose Middle is
  None are written like the others (their stored Place and Count change,
  nothing draws).
- **Place "At nodes"** removes the Count row (criterion 2); the stored Count is
  kept and returns with Spaced. Focus stays on the Place group.
- **Count** is the `style-panel-rework` value field with a linear scale:
  integer, drag range 1 to 50 (5 px per step at 244 px), typed 1 to 500, Shift
  coarse, Ctrl fine, click to type, Home 1, End 50, reset to 1 (icon and
  `Ctrl+Backspace`), `aria-valuetext` "3 markers". Message: "Enter a whole
  number from 1 to 500". A stored Count above 500 (from a file) shows as stored
  with a full bar.
- **Closed paths.** The Start and End slots do nothing on a closed path
  (criterion 10). When a Start or End shape is set and every selected path is
  closed, one muted 12 px line under the End row reads "Closed paths have no
  start or end." (`--panel-muted-fg`, 5.0:1). It is not an error and nothing is
  disabled; it explains why the canvas shows nothing. Not shown for a mix of open
  and closed paths.
- **Hidden:** with Paint None or width 0, with only primitives selected, and
  with nothing selected, the whole block is absent from the tree. Settings stay
  stored (criteria 3, 21).
- **Focus** follows the `style-panel-rework` rules: a mouse press returns focus
  to the canvas; a keyboard choice keeps it. If the block leaves while a control
  of it has focus, focus goes to the Stroke Paint group.

### 4. Names, tooltips, keys

- Groups: "Start marker", "Middle marker", "End marker" (items "None",
  "Arrow", "Dot"); "Marker placement" (items "Spaced", "At nodes"); value field
  "Marker count".
- Tooltips (text only, 400 ms, `side="left"`): Arrow "Arrow: a filled triangle,
  4 x the stroke width long" and, on Start only, "Points away from the path";
  Dot "Dot: a filled circle, 3 x the stroke width across"; Spaced "Evenly spaced
  along the whole path"; At nodes "On every node between the ends. Count is not
  used"; the Count field the value-field legend.
- Keys: arrows move and select inside a group, Tab leaves it; Count as in the
  Style section. No shortcut letters.
- Large documents: the panel only writes the values; drawing 500 markers on
  many paths is the renderer's job (criterion 29 caps it).

### 5. Changes the criteria need (for the PO)

All applied by the PO on 2026-10-08: M1 as criterion 32 (numbered 32 so that
1 to 31 stay stable), M2 in criterion 2, M3 in criterion 1. The text below is the
request as made.

- **M1, new criterion in "Panel behaviour":** the closed-path line of section 3
  (shown when a Start or End shape is set and every selected path is closed).
- **M2, criterion 2:** name the Place choices as "Spaced" and "At nodes" exactly
  (they are the button texts) and the Count reset value 1.
- **M3, criterion 1:** the group has a title row "Markers" and the slot labels
  are "Start", "Middle", "End" (wording).

### 6. Open design question (default)

- **Glyph legibility at 16 px.** The Start/Middle/End arrows differ by position
  and direction only. Decided by the lead, 2026-10-08: ship as drawn above and
  check legibility at 16 px at implementation (the ux-engineer reviews the
  built panel); fallback is to label the three slots by position in the glyph
  with a tick mark. No customer decision is needed.

## Links
Requirements: R-EDIT-005 (stroke styling), R-EDIT-016 (`docs/requirements.md`)
Depends on: `specs/0017-style-panel-rework/`
PR: TBD
