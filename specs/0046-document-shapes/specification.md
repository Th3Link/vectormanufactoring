# Document shapes: a circle, a rounded rectangle or an outline from a file as the document area

Status: Draft (a sketch with numbered criteria; not Ready: it needs the customer's document-model decision, the architect's ADR, the UX notes, and for parts of it the SVG importer of `0024` and a DXF importer that nobody has specified yet)
Priority: Should
Origin: Customer (request of 2026-10-10, final: templates that are not square, custom shapes with a circle or a path loaded from SVG or DXF, again no popup, an add and save button inside the sidebar). Everything about how a shape behaves is the product owner's proposal and is marked in the questions.

## User value

As a maker who cuts key rings, coasters, labels and boards, or embroiders inside a hoop, I want the document area itself to have the shape of my blank (a circle, a rounded rectangle, or the outline of a key ring loaded from a file) so that I draw inside the real material, see what falls outside it, and pick the blank from my formats like any other size.

**Reference tools.** Inkscape and Illustrator have a rectangular page only; a non-rectangular blank is drawn as an object and used as a guide or a clipping path. LightBurn shows the machine bed as a rectangle and has no document shape. Ink/Stitch takes the hoop from a template or a preference. Cricut and Silhouette design spaces offer mat and material templates. Here the area is part of the document and comes from the formats library, the same list the rectangles come from.

## Words used below

- **Area:** the part of the plane that belongs to the document. Today it is the rectangle of the document size.
- **Shape:** the kind of area: **Rectangle** (today's), **Rounded rectangle** (corner radius), **Ellipse** (a circle is an ellipse with equal sides), **Outline** (a closed path, possibly with holes, loaded from a file).
- **Size:** the width and height of the area's **bounding box**. The document size of `0015` is the size of that box, whatever the shape.
- **Pasteboard:** everything outside the area (`0015`). Objects there are drawn and edited as now.

## Dependencies and stages

| Stage | Delivers | Needs first |
|---|---|---|
| 1 | Ellipse (circle) and rounded rectangle as the area; shape in a format; the Shape row in the Document section | `0045` formats library, `0040` background (the fill of the area), a customer and architect decision on the document model (Question 1) |
| 2 | Outline from an **SVG** file | Stage 1 and `0024-svg-import-export` (the importer; not started) |
| 3 | Outline from a **DXF** file | Stage 1 and a DXF importer. **There is no spec, no number and no requirement for DXF import yet**; `R-SYS-006` covers SVG only. It becomes its own entry when the customer schedules it (Question 3) |

Stage 1 can ship without any importer. Stages 2 and 3 are separate PRs in the sense of "one story per PR", each with its own milestones.

## Acceptance criteria

Sizes in millimetres. "Bounding box" is the tight box of the area's outline (for a rounded rectangle and an ellipse, their own box).

### Part A: the document has a shape

1. Given a document without a stored shape (every file before this slice), then its shape is Rectangle and it opens and draws exactly as before; nothing is written by opening it.
2. Given a document with a shape, then the document size (width, height) is the bounding box of the shape and the document stores the shape beside the size: the kind, the corner radius for a rounded rectangle, and the outline's anchors (the compound path model of `0016`) for an Outline. This is a document-model and file-format change: `format_version` goes to the next free number at merge, and an older build refuses the file as "saved by a newer version" (it would otherwise draw a rectangle where a key ring is). **`needs-customer`** (`CLAUDE.md` §3, Question 1).
3. Given a shape, then the document edge, the rulers and the status bar use the bounding box: the ruler origin is its top-left corner, the size readout is its width and height, as in `0015`.
4. Given an Ellipse, then it fills its bounding box (a circle when width and height are equal); a Rounded rectangle has one corner radius, 0 to half of the shorter side; an Outline is a closed path, or a compound path whose holes are not part of the area.

### Part B: how the area is drawn

5. Given a shape, then the area is filled with the document background (`0040`: a colour with alpha, or the checkerboard for None) and the pasteboard around it, and the holes of an Outline, show the canvas colour. The edge of the area is drawn as the document edge is today (`0015`), along the outline, one constant screen width.
6. Given a shape that is not a rectangle, then objects are **not clipped**: an object, or part of one, outside the area is drawn in full, as on a rectangular document's pasteboard. The area is a guide and a manufacturing boundary, not a mask. (Question 4: dimming what lies outside.)
7. Given the pointer, then a click on an object outside the area selects it as on the pasteboard; the area is not an object and cannot be selected or moved (`0040` criterion on the background).
8. Given a zoom from 1 % to 6400 %, then the edge of the area stays one line wide and the outline of a circle stays smooth (flattened to within 0.01 mm at 100 %, the tolerance of the render pipeline).

### Part C: the Document section

9. Given the Document section, then it has a **Shape** row with a segmented group: Rectangle, Rounded, Ellipse, and, once stage 2 exists, Outline (an inline group, no dropdown). The pressed item is the document's shape. Pressing another item changes the shape and keeps the bounding box: width and height stay, the centre stays, no object moves, one commit `set_document_shape`.
10. Given Rounded, then a **Radius** value field (`0017` value field, 0 to half of the shorter side, mm) is in the tree, otherwise it is not. Given Ellipse with unequal sides, then nothing links them; the format library offers a **Circle** that has one size (criterion 17).
11. Given Width or Height edited on an Ellipse or Rounded rectangle, then it resizes about the centre as `0015` criteria 17 to 19 say, and the corner radius is clamped to half of the shorter side.
12. Given an Outline, then Width and Height still resize it, but the outline is **scaled** with the box and keeps its proportions: changing one field sets the other (**Proposal**, Question 5: the alternative is a fixed outline with only a Scale field). Objects move about the centre as in `0015`, the outline scales; nothing else changes. The scale is one commit, `resize_document`.
13. Given **Fit to content** (`0015`), then it is offered for Rectangle and Rounded rectangle only and is not in the tree for Ellipse and Outline: a fitted circle or key ring has no obvious meaning (the smallest enclosing circle is not what a maker means). The rule is stated in the tooltip of the Shape row.
14. Given the shape controls, then all of this is in the panel's own column: no popup, nothing disabled, hidden when it does not apply (`0017`).

### Part D: shapes in the formats library (`0045`)

15. Given a format, then it is a name, a size and a **shape**: `rectangle` (the default, so every existing file is unchanged), `rounded` with `corner_radius`, `ellipse`, or `outline` with `outline` (an SVG path `d` string in millimetres, origin at the top-left of its bounding box, at most 64 KiB and 2,000 anchors). A new optional key is added to the file schema, so the built-in file goes to schema 3 and the user file to `format = 2` (`0045`, `adrs.md` decision 9). A pick applies the size and the shape in one `resize_document` commit; `0030` criterion 11 (objects move about the centre) holds.
16. Given the selected state of `0030`, then a document matches a format when size **and shape** match. A circle and a square of the same box are two formats and may coexist (this lifts `0045` criterion 5's size-only duplicate rule to size plus shape).
17. Given the Add format form of `0045`, then it gains a **Shape** row after Unit: a group of four glyph-only items (Rectangle, Rounded, Circle, Outline from file), 36 px each, each with a tooltip and an accessible name. Circle replaces the Width and Height rows by one number, **Diameter**, and stores an ellipse with equal sides. Rounded adds **Radius**. Outline from file replaces the size rows by the **Choose file** button of criterion 19.
18. Given a document whose shape and size match no format, then the Document tab has the button **Save as format** that opens the same inline form of `0045`, prefilled from the document (shape, size, outline), so the maker can name what he has just built. It shares the row of **Fit to content**: two half-width buttons of 118 px each, Fit on the left. The row exists whenever either button is in the tree and each keeps its slot, so neither jumps when the other leaves. The button is not in the tree when a format already matches, and it is never placed above Width, because its coming and going while the maker types a size would move the field he is typing in.

### Part E: an outline from a file (stages 2 and 3)

19. Given the Add format form with Shape set to Outline, then the form has a button **Choose file** that opens the OS file dialog (the only layer, as Open). The dialog accepts `.svg` (stage 2) and `.dxf` (stage 3). The chosen file is read by a platform crate and parsed by the importer (`0024`; a DXF reader) in a core crate; the core crate opens no file.
20. Given a chosen file, then the importer finds the closed outlines: for an SVG every closed path, rectangle, ellipse, polygon in the file's first level and inside groups, flattened to the model of `0016`; for a DXF every closed `LWPOLYLINE`, `POLYLINE`, `CIRCLE`, `ELLIPSE` and closed chain of `LINE` and `ARC` (stage 3). Units: SVG user units at 96 per inch (ADR 0002); DXF `$INSUNITS` when present, otherwise the form's **File units** group (mm, in, px) which is shown in the tree only when the file does not say (**Proposal**).
21. Given the outlines, then the **outermost** closed outline is the area and every outline inside it is a hole. When the file has several outermost outlines, the largest by area is used and the form says "The file has 3 outlines. The largest is used." (Question 6: choose one.) A file with no closed outline is refused with "No closed outline found. Nothing was changed." An open path is never closed implicitly (`0016` criterion 15's rule).
22. Given an outline, then it is normalised: moved so its bounding box starts at 0, 0 and kept in millimetres; text, images, strokes and fills of the file are ignored; limits 2,000 anchors (curves kept) and a size from 1 to 100 000 mm per side, otherwise "The outline is too complex (limit 2,000 nodes)." or the size message of `0015`.
23. Given Add with a valid outline, then the format is stored with the outline inline in the user file (criterion 15) and picking it applies the shape. The source file is not referenced afterwards: moving or deleting it changes nothing.
24. Every importer gets golden-file tests in `tests/fixtures/` (`CLAUDE.md` §5): at least a key ring with a hole, a circle, a rounded rectangle, a file with three outlines, a file without a closed outline, and for DXF one file per entity kind.

### Part F: links to manufacturing (stated, not built)

25. The area is a boundary for later work and nothing in this slice uses it for output: a job whose cut paths lie outside the area gets a warning in the job preview (`0029`); export clips nothing (`0024`); a laser bed (`machine-profile`, `0026`) is a separate rectangle and the shape sits inside it; an embroidery hoop (a circle or a rectangle with a safe margin) is a format in the library; nesting parts on a blank (R-MFG-004) can use the area as its container. Each is a later entry that reads the stored shape; none changes this spec's model.

### Part G: how the shape is stored (direction from `adrs.md`; `needs-customer`, Question 1)

26. Given a document with an Outline, then the outline is stored relative to its bounding box (normalised to a unit box), and the size registers stay the bounding box; a resize writes only width and height. Test: two replicas, one resizes the document while the other changes the shape, merge: box and outline agree on both. The outline is replaced as a whole value, never mixed across peers. The proportion lock of criterion 12 is a rule of the resize command for an Outline only.
27. Given the area is drawn, then the background fill, the eyedropper and "is this point on the document" use a point-in-shape test on the real outline, not the bounding box (`0040` background pick); a pick on a hole of an Outline reads the pasteboard.

## Out of scope

- Clipping or masking export, print or jobs to the area; warnings about objects outside it (criterion 25).
- Snapping to the edge of the area, "keep inside" constraints, margins and safe areas.
- Several areas in one document, one area per page; several pages are dropped (2026-10-05).
- Editing the outline's nodes in the document. The area is not an object (`0040`). To change it, load another outline or draw the object and use it as a template later.
- Using a drawn object as the area ("make this path the document shape"). A natural follow-up (**Proposal**); not now.
- A DXF importer in general (any entity, layers, blocks, 3D). Stage 3 reads outlines only.
- First-party shape formats (hoop diameters, coaster sizes): the customer's list (Question 2).
- Per-material or per-machine shapes from the material database (R-MAT).
- A perspective, bleed or a second, inner area.

## Open questions (customer; each has a default)

1. **The document model (criterion 2).** A shape register next to the size, a `format_version` bump, the outline stored in the document, older builds refusing the file. *A (default, recommended):* one register `page_shape` (kind, radius, outline anchors) absent for Rectangle. *B:* the shape is not stored; it is a view setting and the file stays a rectangle. Rejected: the shape would be lost on reopening, and the next maker would not see the blank. Needs the customer and the architect.
2. **The customer's real list.** Which first-party shape formats ship: hoop diameters (for example 100, 130, 180 mm), coaster sizes (90, 100 mm), key rings, label sizes? *Default:* none beyond A0 to A6 and the slides; the maker adds his own. Send the list and each becomes one block in the built-in file.
3. **DXF timing.** DXF import is not specified anywhere. *A (default):* stage 3 waits until the customer schedules a DXF importer (its own spec with entity coverage and golden files); stages 1 and 2 ship without it. *B:* DXF outlines come first, before SVG, because the customer's blanks come from DXF. Tell me which file type the real templates are.
4. **Outside the area.** *A (default):* objects outside are drawn normally (criterion 6). *B:* the part outside is dimmed to 35 % like outside an entered group (`0023`), a quiet warning for a laser job; it costs a clip pass in the renderer.
5. **Resizing an outline (criterion 12).** *A (default):* Width and Height scale it, proportions kept. *B:* the outline is fixed; Width and Height are replaced by a read-only size and a Scale value field. Recommendation A for one mental model.
6. **A file with several outlines (criterion 21).** *A (default):* the largest is the area, the others ignored with a notice. *B:* an inline list with a thumbnail per outline to choose from. *C:* every outermost outline is an area (disconnected areas). Recommendation A for the first version.
7. **Ellipse versus circle.** *A (default):* one Ellipse shape; "Circle" is a format with equal sides. *B:* a separate Circle shape that keeps width and height equal when edited. Recommendation A.
8. **Fit to content on a circle (criterion 13).** *A (default):* not offered. *B:* the smallest enclosing circle. Recommendation A.

## UX notes

A short note by the ux-engineer, 2026-10-10, written against `0045`'s UX notes. It is a sketch: the full notes follow when the customer has decided Question 1 (the document model) and the stages are scheduled.

- **Shape glyphs.** One set of 16 px Lucide glyphs, 1.5 px stroke, used everywhere a shape is named: Rectangle `Square`, Rounded `SquareRoundCorner`, Circle and Ellipse `Circle`, Outline `Spline` (a freeform curve). Glyph only, with a tooltip and an accessible name.
- **A shape format in the list and the strips.** A format row shows the glyph (12 px) before the size when the shape is not a rectangle; a circle's size reads "⌀ 130 mm" (diameter sign, one number), an ellipse "130 × 90 mm", a rounded rectangle "100 × 60 mm, r 8". A cell of the quick selection shows the same 12 px glyph before the name ("◯ Hoop 130"); the cell keeps its 32 px minimum. The pressed and selected look follows size **and** shape (criterion 16).
- **Add format form.** After Unit, a **Shape** row: a `ToggleGroup` of four glyph-only items (Rectangle, Rounded, Circle, Outline), 36 px each. Circle replaces the Width and Height rows by one **Diameter** row; Rounded adds a **Radius** row (value field, as the Document tab's Radius). Outline replaces the size rows by a **Choose file** button (outline, 244 × 28, opens the OS file dialog, the only layer) and, once a file is read, a muted line "Outline: 42 × 30 mm, 38 nodes" and the validation chips of criteria 20 to 22. The form is cancelled and kept as a draft like any form of `0045`.
- **Document tab Shape row** (criterion 9): the same glyph group, 36 px items (four with Outline), label "Shape", directly above Width. Radius appears under it only for Rounded. Width and Height keep their place; for a circle the two fields stay editable and unlinked (Question 7 A).
- **Save as format** (criterion 18): placed in the row of **Fit to content**, the two as half-width buttons (118 px each, Fit on the left). The row exists whenever either is in the tree and each keeps its slot, so neither jumps when the other leaves. It must not sit above Width: its coming and going while the maker types a size would move the field he is typing in.
- **The area on the canvas.** The edge is the document edge of `0015` along the outline, one constant screen width; the inside is the background (`0040`), the outside and holes the pasteboard colour. Nothing is clipped; no cursor change over the area (the area is not an object). The canvas shows the shape; the status bar and rulers show the bounding box.

Criteria changes requested and applied by the product owner on 2026-10-10: **18** (the button shares the Fit row); **17** (the Shape group is glyph-only, 36 px items).

## Links

Requirements: R-EDIT-028
Depends on: `specs/0045-document-formats-library/`, `specs/0040-document-background/`, `specs/0015-document-size-and-rulers/`; stage 2 `0024-svg-import-export` (not started); stage 3 a DXF importer (no spec)
Related: `specs/0016-boolean-operations/` (compound path model), ADR 0002 (units, SVG at 96 dpi), ADR 0004 §9 (format versioning)
ADRs: `adrs.md` (architect, a sketch; a short ADR, `needs-customer`, is written when stage 1 is scheduled)
PR: -
