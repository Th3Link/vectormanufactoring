# Document size and rulers

Status: Ready
Priority: Must
Origin: Customer (rulers, resize, fit to content, drawing outside the document). The display unit (Part C) is a **Proposal** and is not accepted until the customer says so.

## User value

As a maker I want rulers along the top and left of the canvas, to set my document to the size of my sheet, to fit it to what I have drawn, and to keep drawing freely outside the document edge the way I can in Inkscape, so that I can judge real size and position at a glance and prepare a sheet that matches my material or my artwork.

**Single document.** A project has one document with one size. Several pages in one project were asked for first and dropped on 2026-10-05 ("not important enough for MVP"); ADR 0012 (pages) was rejected. Nothing here prepares for pages.

**What we match on purpose, and what we do differently:**

- **Rulers in mm, origin at the document's top-left corner, position growing right and down.** This is Inkscape's default since 1.0, SVG's own convention and this product's Y-down rule (ADR 0002 §4). Checked, not assumed.
- **Resize keeps the centre fixed. This is a customer decision, not Inkscape parity.** Inkscape resizes the page from a corner (with a 3 × 3 anchor picker). Here the centre of the document stays fixed every time, with no picker, so old content stays centred instead of jumping into a corner. Do not "fix" it toward Inkscape.
- **Drawing outside the document is allowed, on a grey pasteboard.** Matches Inkscape and was named by the customer ("like in Inkscape").
- **Fit to content with a 0 mm margin.** Inkscape's "Resize page to content" has a margin setting, non-zero by default. A laser maker sizing a sheet to the parts on it wants the smallest honest sheet. A margin field is a small later addition (Out of scope).
- **Rulers show a pointer marker.** Inkscape and LightBurn both draw a tick that follows the pointer on each ruler; the customer did not ask for it, I add it because it costs little and makes the rulers useful while drawing (Part A, criterion 8).

**Consequence the customer should know (decided by the customer, repeated here once):** because the origin is the document's top-left corner and a resize keeps the centre, resizing from 210 mm to 300 mm wide moves every object 45 mm to the right in document coordinates. The drawing does not move relative to itself.

## Acceptance criteria

Distances are in millimetres unless a criterion names the display unit. Test values assume the default display unit (mm) and the zoom definition of `0004-canvas-navigation-and-selection` criterion 7: 100 % is 1 mm = 96 / 25.4 ≈ 3.78 CSS pixels, the range is 2 % to 8000 %.

### Part A: rulers

1. Given a project is open, then a horizontal ruler is shown along the top edge and a vertical ruler along the left edge of the canvas, and a square where they meet that shows the display unit ("mm", "cm" or "in"). Both rulers use the one viewport state (`ViewTransform`) the canvas uses, not a copy.
2. Given any pan or zoom (wheel, drag, Space-drag, window resize, opening or closing the Properties panel), then the tick marks and labels of both rulers are updated in the same frame as the canvas. Test: after each of 20 scripted pan and zoom steps, every major tick's screen position equals the canvas projection of its document value within 0.5 px.
3. Given the document, then ruler value 0 on both axes is at the document's top-left corner; the horizontal value grows to the right and the vertical value grows downward.
4. Given the pasteboard (left of or above the corner, or beyond the document width or height), then the rulers continue without a break: negative values left and above, values larger than the document size right and below.
5. Given any zoom from 2 % to 8000 %, then the major tick spacing is the smallest step of the form 1, 2 or 5 times a power of ten (in the display unit) whose on-screen length is at least 40 px and at least the width of the widest visible label plus 4 px. Without wide labels this gives a spacing of 40 px or more and less than 100 px. Each major interval is divided into 5 equal minor intervals. Examples in mm: 100 % gives a major step of 20 mm; 8000 % gives 0.2 mm; 2 % gives 1000 mm.
6. Given a major tick, then it carries a label with its value in the display unit, without a unit suffix, with a minus sign when negative, and written as the exact decimal of the step: at every zoom the tick at 0.3 reads "0.3", never "0.30000000000000004", and no label has trailing zeros ("2", not "2.0").
7. Given any zoom from 2 % to 8000 % and any value up to ±1,000,000 in the display unit, then no label is clipped by its ruler and no label overlaps another label or the corner square. A label that would not fit entirely inside the ruler's free span is not drawn. When labels are wider than the major spacing, labels are drawn on every second or every fifth major tick; ticks are never removed and numbers are never abbreviated.
8. Given the pointer is inside the canvas, then each ruler shows a marker at the pointer's position: a 1 px line across the ruler in the accent colour on the horizontal ruler for the pointer's x and on the vertical ruler for its y. The marker's position equals the status bar's x and y readout, within the readout's precision. Given the pointer leaves the canvas, then both markers disappear.
9. Given the rulers are shown, then a click at the screen position of the horizontal ruler tick "50" (vertical position anywhere in the canvas) creates or hits geometry at document x = 50 mm within 1 / scale mm. That is, the rulers shrink the canvas viewport and the screen-to-document conversion accounts for it. Test with the Rectangle tool at 100 % and at 800 %.
10. Given a press, drag or click on a ruler or on the corner square, then nothing happens (no guide, no selection change, no tool action). Guides are Out of scope.
10a. Given the wheel or a pinch gesture over a ruler, then it acts on the canvas as if the pointer were at the nearest point inside the canvas, so a zoom does not stop when the pointer slips onto a ruler.
11. Given a window size of 800 × 600 (the minimum), then the canvas is still usable: it is at least 400 px wide and 400 px high after the rulers, panel and status bar are placed.
11a. Given a project is created or opened, then the document's top-left corner is shown 72 px right of and 72 px below the canvas's top-left corner (so the edge and the 0 ticks are not hidden under the tool rail). This changes the initial view of `0004`.

### Part B: document size

12. Given a new project, then the document is 210 × 297 mm (A4 portrait, the size since `0001-project-file-foundation`), and the status bar shows "210.0 × 297.0 mm".
13. Given a project file saved by any earlier build (format versions 1 to 7), then it opens with the size stored in it (210 × 297 mm so far, since no build could change it) and every object where it was. Given a file in which either size value is missing, not a finite number, below 1 mm or above 100 000 mm, then it opens as 210 × 297 mm (both axes) and is not refused. Opening writes nothing to the file.
14. Given the Document settings (location: Question 1), then they contain a Width field and a Height field, typed text fields (not drag fields) that show the size in the display unit, and a Fit to content button. They show the current size at all times, including after a resize, a fit and a file open.
14a. Given no object is selected and the Pen has no unfinished path, then the Properties panel shows the Document settings and nothing else. Given one or more objects are selected, then it shows Style as before and no Document settings (Escape clears the selection and so reveals them). Given the Pen has an unfinished path, then the panel is empty, because a resize would move the committed objects but not the path being drawn. The panel's width and the canvas size never change between these states. This amends `0017-style-panel-rework` criterion 1 (see Links).
15. Given a Width or Height field, when the maker types a number and presses Enter or leaves the field, then the size is applied as one operation. Escape restores the last applied value. Enter on text the maker did not edit does nothing. A decimal point or a decimal comma is accepted; surrounding spaces are ignored.
16. Given a typed value that is empty, not a number, infinite, or outside 1 mm to 100 000 mm (converted from the display unit; a value within 1e-9 mm of a limit is accepted), then the size is not changed and the field shows the standard validation message of the panel, written in the display unit with the limits rounded inward: mm "Enter a number from 1 to 100000", cm "Enter a number from 0.1 to 10000", in "Enter a number from 0.04 to 3937". The message is cleared by the next keystroke. No dialog opens. Text with a unit ("21cm") is not a number.
17. Given a valid new size (w1, h1) for a document of size (w0, h0), then every object is moved by the shift ((w1 − w0) / 2, (h1 − h0) / 2) in document coordinates, so the centre of the document and every object's position relative to that centre stay as they were. Test: a rectangle at x = 10, y = 10, 50 × 50 mm in the 210 × 297 document; after setting 300 × 400, the rectangle is at x = 55, y = 61.5, still 50 × 50. This holds within 1e-9 mm for paths (all anchors; handles are relative and unchanged), rectangles, ellipses, polygons and stars, rotated or not.
18. Given criterion 17, then no object is scaled, cropped, restyled or rotated, including when the new size is smaller than the content: objects may then lie partly or wholly on the pasteboard.
19. Given criterion 17, then the resize is one atomic operation: one commit, whose label names the operation (stored as `resize_document`), that writes the new size and moves all objects together. No state exists in which only one of them has happened, however the operation ends. A typed value equal to the current size writes nothing.
20. Given a resize or a fit (criterion 24), then every object appears at the same screen position (within 0.5 px) immediately before and after. The document's edges move on screen instead, around the content. The change is instant, with no animation.
21. Given a resize or a fit, then the status bar size readout shows the new size at once. The status bar shows the cursor position followed by the display unit once, and the size with the unit, each with a fixed number of decimals so the text does not jitter: mm 1, cm 2, in 3 (all three the same 0.1 mm resolution). Example: "x: 12.3  y: 45.6 mm" and "210.0 × 297.0 mm". Stored values are not affected by the display.

### Part B: fit to content

22. Given the document has at least one object, then the Fit to content button is shown. Given it has none, then the button is removed from the panel (not greyed out), and the size cannot change by it.
23. Given the document has objects, when the maker activates Fit to content, then the new size is the extent of the axis-aligned bounding box of all objects (including those on the pasteboard), with a fixed margin of 0 mm. The box is of the objects' outline geometry in document coordinates: curves contribute their true extremes (not control points), rotated rectangles, ellipses, polygons and stars their rotated outline, and compound paths all their outlines. Stroke width is not included.
24. Given criterion 23, then every object is moved by −(the box's top-left corner), so that after the fit the box's top-left corner is at (0, 0) within 1e-9 mm. This rule is separate from criterion 17: the fitted document is centred on the content, not on the old document.
25. Given the box is narrower than 1 mm on one axis, then the document size on that axis is 1 mm and the content is centred on that axis. Test: one horizontal line from (0, 0) to (100, 0), 10 mm stroke width: the fit gives 100 × 1 mm and the line lies at y = 0.5.
26. Given Fit to content has just been applied, when it is applied again, then the size and all positions are unchanged (it is idempotent), nothing is committed, and a text notice "Already fits the content." shows under the button for 3 seconds.
27. Given Fit to content, then it is one atomic operation, one commit, whose label names the operation (stored as `fit_document_to_content`), with the same guarantee as criterion 19.
27a. Given the objects' bounding box is larger than 100 000 mm on an axis, when the maker activates Fit to content, then nothing changes and the panel shows a validation message that names the limit, for example "The content is larger than the largest document (100000 mm). Nothing was changed." (in the display unit, rounded inward as in criterion 16).

### Part B: pasteboard and drawing outside the document

28. Given a document of any size, then its area is painted in the canvas background colour (`--canvas-bg`) and the rest of the viewport, out to the edges of the window, in a grey pasteboard colour, `--pasteboard-bg` #B8B8BE, that is clearly distinct from `--canvas-bg` #E8E8EB and from the chrome colours (contrast ratio 1.61 to the document and 1.44 to the chrome tone, as measured by UX). Both follow pan and zoom. The edge between them is a flat colour change without shadow.
29. Given any drawing or editing tool (Select, Pen, Node, Rectangle, Ellipse, Polygon/Star), when the maker draws, places or drags an object so that part or all of it lies on the pasteboard, then it is created or edited exactly as inside the document: no warning, no clipping, no snapping to the edge.
30. Given an object with any part on the pasteboard, then it can be selected, moved, transformed and edited like any other object.
31. Given the Pen tool's close-target marker (the knockout dot) is shown over the pasteboard, then its fill matches the colour behind it, not the document colour.
32. Given any job, export or print, then this slice defines no behaviour for objects on the pasteboard and none for the document size: `manufacturing-roles` and `laser-job-preview-and-output` decide that later. The size is stored so those slices can use it.

### Part C: display unit (Proposal)

33. Given the Document settings, then a unit control with the choices mm, cm and in is shown (a segmented control; the panel has no dropdowns). A new project uses mm.
34. Given a unit is chosen, then these display values change and nothing else: the ruler labels and steps (criteria 5 to 7, with "power of ten" read in the chosen unit; at 100 % the major step is 2 cm in cm and 0.5 in in inches), the Width and Height fields, and the status bar's cursor and size readouts (decimals per unit as in criterion 21). Stored values stay in mm. Nothing in the document moves or changes. A Width or Height field with edited text that is not yet committed commits in the old unit when the maker presses the unit control, and then the unit changes.
35. Given the chosen unit is in, then 1 in is exactly 25.4 mm: typing 8.5 and 11 sets the size to 215.9 × 279.4 mm. Values are shown rounded to 3 decimals (mm) or 4 decimals (cm, in) without trailing zeros; showing a value never rewrites the stored value.
36. Given a unit is chosen, then it is saved with the document and is the same after closing and reopening. A file from before this setting opens in mm. The setting is a document setting that belongs to the file, not to the user.
37. Given Part C, then readouts and fields outside the three places of criterion 34 stay in mm for now: the transform and move entry chips, the Select and Node bar fields (Radius, Points, Ratio), the stroke Width in the Properties panel (`0017-style-panel-rework` criterion 48). This is a known inconsistency (Question 2), not a defect.

### Persistence

38. Given any of the above, then the size is stored in the project file as it has been since `0001-project-file-foundation` (width and height in mm). Part C adds one stored value, the display unit (absent or unknown reads as mm). There is no `format_version` bump: an earlier build opens a file with a display unit, shows mm (a correct view of the same document), and keeps the stored unit when it saves. If Part C is not accepted, no new stored value exists. Changing the unit is one commit, whose label names the operation (stored as `set_display_unit`), and moves nothing.
39. Given a resize or a fit followed by Save, Close and Open, then the size and all object positions are the same as before closing.

## Out of scope

- **Several pages or documents in one project.** Dropped by the customer on 2026-10-05. Revisit as its own spec only if the customer asks.
- **A resize-anchor picker** (Inkscape's 3 × 3). Centre-anchoring is the only behaviour, by customer request.
- **A fit-to-content margin setting.** Fixed at 0 mm.
- **Fit to content using stroke width** (visual bounds). See Question 3.
- **Guides, snapping to guides, snapping to the document edge**, dragging the ruler zero point, a ruler right-click menu. Proposal for a follow-up slice "guides and snapping": pull a guide from a ruler, snap objects to guides, the document edge and the grid. Snapping is a feature of its own and touches every tool.
- **Size presets** (A4, A3, Letter, the sheet sizes of common laser beds) and an orientation swap. Proposal for a small follow-up; `machine-profile` (R-MFG-001) may supply a work area to preset from. Until then document size and machine work area are independent.
- **Unit conversion of every other length in the app** (Question 2, option C).
- **Zoom presets** ("fit document to window", "fit selection", "reset to 100 %"). Still out of scope as in `0004`.
- **Export, print, SVG `width`/`height`/`viewBox`, job origin.** Later slices read the stored size (`svg-import-export`, `laser-job-preview-and-output`). This slice defines no output behaviour.
- **Undo and redo** (`0020-undo-redo`, not started). Resize, fit and unit change are each one commit, so undo can take them later.
- **Highlighting the document span on the rulers**, ruler colours for dark mode, a ruler that shows the pasteboard differently.

## Open questions

Each has a default; nothing blocks.

1. **Where the document settings live (criterion 14).** *A (default, recommended):* in the Properties panel, as a "Document" section that is shown whenever no object is selected and the Pen has no unfinished path (criterion 14a). With an object selected the panel shows Style as today. This changes the rule of `0017-style-panel-rework` criterion 1 that the panel is empty when nothing is selected (amended there, text only); the frame and the canvas size do not change. It follows Figma and Affinity. *B:* File > Document Properties… (Shift+Ctrl+D, as in Inkscape) opens a dialog; the panel stays empty when nothing is selected. Needs a first general dialog (only a blocking-error dialog exists). *C:* only the status bar size readout becomes clickable and edits Width and Height inline there; no unit control, no Fit button. Recommendation: A. A and B change where the controls are, not what they do.
2. **Display unit (Part C).** *A (default):* mm, cm and in for rulers, the document size fields and the status bar only (criteria 33 to 37), with the stated inconsistency. *B:* mm only now; units become their own slice that converts every length field at once. *C:* units for every length field in this slice (touches the transform chips, bars, Style panel and four specs). Per document (default) or per user: per document is how Inkscape does it and needs one new stored value but no `format_version` bump (criterion 38); per user is how LightBurn does it and needs an app settings store that does not exist yet. With collaboration later, a per-document unit is shared: whoever switches last switches it for everyone. Recommendation: A for the slice, then C as a follow-up if inches are used a lot. If you choose B, Part C disappears.
3. **Fit to content ignores stroke width (criterion 23).** *A (default):* geometric bounds. Right for cut lines; with a default stroke of 0.25 mm the stroke sticks out 0.125 mm past a tight sheet. *B:* include half the stroke width, right for artwork printed as drawn. Recommendation: A; B as a later switch.
4. **Largest document size (criterion 16).** Default: 100 000 mm (100 m) per side, so the field refuses nonsense without refusing a large CNC bed or banner. Say if you want it lower.

Decided by the product owner (change if you disagree): new documents stay A4 portrait (210 × 297 mm); the minimum size is 1 mm; invalid input is refused with the panel's validation message and not silently reverted; the view keeps the content still on screen when the size changes; rulers show the pointer marker; rulers do nothing when pressed (no guides); the Document section is absent while the Pen has an unfinished path (criterion 14a); a new view shows the document 72 px in from the corner (criterion 11a); Fit to content on content larger than 100 000 mm is refused (criterion 27a).

## Build order

From the architect's notes (`adrs.md`, decisions 15 and 16): three PRs. PR 1, the model (size, unit, resize, fit, parse and format, cargo tests only, no format bump), shares no crate with the kernel PR of `0016-boolean-operations` and can run in parallel with it. PR 2 (rulers and pasteboard) and PR 3 (Document section, unit control, Fit, status bar; after `0017-style-panel-rework` if that is built first) are staggered with the later PRs of 0016, because both features touch `document-core`, `ui-core`, `render-core`, `editor-wasm` and `frontend`. If 0016 merges first, this feature only rebases; neither order needs a version number change.

## UX notes

By `ux-engineer`, 2026-10-09; refreshes the notes of 2026-10-05, which predate the pages removal and `0017-style-panel-rework`. Numbers, tokens and rows are in `docs/design-system.md` ("Ruler", "Pasteboard and document edge", "Properties panel: Document section", "Status bar"). Criteria that need to change are listed at the end; until the product owner agrees, the criterion as written stands.

### 1. Rulers

- **Docked chrome, not an overlay.** The horizontal ruler runs along the top of the canvas region and the vertical ruler along its left edge, inside the region (the Properties panel is outside it). They shrink the viewport. Everything that was anchored to the canvas region (tool rail 12 px inset, the bars' overlay row `left-[72px] top-3 right-3`, the hint and entry chips' clamp, the collapse tab) is anchored to the **viewport** (the area inside the rulers), so no number in the existing rows changes. Tooltips, notices and chips may cover the rulers.
- **Thickness and surface.** 24 px (the status bar's height). Ground `--ruler-bg` (= `--statusbar-bg`, the chrome tone). A 1 px edge line `--ruler-edge` (`--toolbar-icon` at 25 %, the panel's edge-line convention) on the canvas-facing side of each ruler and of the corner. Rulers are chrome, so no shadow.
- **Tick hierarchy (two levels, as criterion 5 defines).** Major: 1 px `--ruler-tick` (`--toolbar-icon`, 8.3:1), full 24 px depth, growing from the canvas-facing edge. Minor: 4 ticks between two majors (5 intervals), 1 px `--ruler-tick-minor` (`--toolbar-icon` at 60 %, 3.1:1), 8 px deep. **The tick at 0 is the origin marker:** 2 px wide, so the document's corner can be found without reading labels. Ticks sit on whole device pixels.
- **Labels.** 12 px (`text-xs`) system font, tabular figures, `--ruler-label` (= `--toolbar-icon`, 8.3:1 on the ground), real minus sign (U+2212), no unit suffix, no digit grouping. Horizontal ruler: the label starts 4 px right of its tick, top aligned 3 px from the ruler's top (the text lies above the minor ticks, so labels and minor ticks never touch). **Vertical ruler: the label is rotated 90 degrees counter-clockwise** (reads bottom to top, as on Inkscape's and Figma's side rulers), occupies the span after its tick in the direction of growing values (starting 4 px below the tick, the end of the text next to the tick), and is 12 px thick, so the 24 px strip holds "-1000000" without widening it. Criterion 7: **a label is drawn only if its whole extent lies inside the ruler's free span** (not under the corner, not past the end). When the widest label of the current view plus 8 px exceeds the major spacing (about 53 px for "-999999.8" at 7 px a digit, against a minimum spacing of 40 px), labels are drawn on every second major tick, then every fifth; ticks are never removed and a number is never abbreviated or put in an exponent.
- **Corner square.** 24 x 24 px, `--ruler-bg`, both edge lines, shows the **display unit** ("mm", "cm", "in") in 12 px `--panel-muted-fg` (5.0:1), centred, `aria-hidden`. The unit has no other place on screen while an object is selected, and the labels carry no suffix (criterion 6), so this is the cheapest honest label. It does nothing when pressed (criterion 10).
- **Pointer marker.** 1 device-pixel line across the full ruler thickness in `--ruler-pointer` (= `--accent`, 3.3:1 on the ground), above the ticks and below the labels. It follows the same source as the status bar readout: shown exactly while the readout is live, gone when it is. A triangle or a second line would only repeat the readout.
- **Interaction.** Rulers and the corner are `aria-hidden`, not focusable, cursor `default`. A press on them is swallowed (no canvas action starts, no capture), criterion 10. The **wheel and pinch over a ruler act on the canvas** with the pointer position clamped to the nearest viewport point, so a zoom does not stop dead when the pointer slips onto the ruler edge (proposed criterion 10a). No hover state, no tooltip.
- **At 800 x 600.** Viewport about 496 x 546 px (800 - 280 panel - 24; about 570 - 24, the 570 being the panel viewport measured in `0007`): above the 400 x 400 of criterion 11. The bars' overlay row has 412 px (was 436) and so wraps one row earlier in places; the Boolean group (148 px) and every other group wrap whole. To be measured at build.
- **Default view.** On New and Open the document's top-left sits 72 px right of and below the viewport's top-left (the tool rail's 12 + 48 + 12), so the edge, the pasteboard and the 0 ticks are visible and not hidden under the rail. This is a change to the initial `ViewTransform` of `0004` (proposed criterion 11a, default accepted).
- **Dark theme.** Only the light theme exists. The six ruler tokens are aliases of existing ones, so a dark theme is a value swap with no rename.

### 2. Pasteboard and document edge

- The document area is `--canvas-bg` (#E8E8EB). The pasteboard is `--pasteboard-bg` #B8B8BE, flat. Measured (WCAG ratios): pasteboard against document 1.61, against the chrome tone 1.44 (document against chrome is 1.12 today, which already reads as two surfaces). Black strokes on the pasteboard 10.6; `--accent` lines on it 2.3, white casing on it 2.0, so the casing rule (better of line and casing at least 2.0) holds for every editor line drawn on the pasteboard; `--field-invalid` on it 3.3. Two darker candidates (#B0B0B6, #A8A8AE) lift the edge to 1.76 and 1.93 but drop the accent line itself to 2.1 and 1.9, so the casing (2.2, 2.4) would have to carry every editor line; #B8B8BE keeps the line readable on its own and stays.
- **No border, no shadow** (criterion 28; the 2026-10-05 decision). The edge is a colour step. Open design question 3 below offers a 1 px edge line.
- Painted first in the WebGL draw list (pasteboard fill, then the document rectangle), snapped to whole device pixels so the edge is never two half-tone rows.
- Objects, handles, selection lines and the Pen's close-target knockout dot read the colour under them; no editor line needs a new token on the pasteboard (the casing rule covers it).

### 3. Document section (Question 1, option A, refined)

**Placement: a section of the Properties panel that is the panel's whole content when nothing is selected, and is absent when anything is selected.** Not "always at the bottom": with a selection the Style block is 700 to 904 px and the Document block would sit below the fold at 800 x 600, and it would shift whenever Style rows come and go (a violation of the anchoring rule of `0017`). Not "always at the top": it would take 190 px from Style for a setting changed once per project. It follows Figma and Affinity.

**Coexistence with `0017` criterion 1.** The rule's intent (the Style area is empty when nothing is selected; the panel width and the canvas never change) holds. What changes is the literal text "no heading, no text, no control": with nothing selected the panel shows the Document section instead of nothing. The panel is empty (frame and collapse tab only) in exactly one case: **while the Pen has an unfinished path**, because resizing then would move the committed objects and not the path being drawn (default; needs the product owner's nod, proposed 14a). Switching between Style, Document and empty is instant and changes no size.

**Layout** (244 px content, 28 px rows, 8 px apart, label column 60 px, control column 176 px):

1. Header, 24 px: "Document" 14 px semibold. No subject line.
2. **Width** and **Height**: two rows, each a `TextField` (typed, not dragged) with the unit as a fixed 14 px `--panel-muted-fg` suffix inside the right edge, numbers right-aligned in 14 px tabular figures. Accessible names "Document width" and "Document height". Shown in the display unit (3 decimals mm, 4 cm and in, no trailing zeros; criterion 35).
3. **Unit** (Part C only): label "Unit", a `ToggleGroup` of three text items "mm", "cm", "in" (44 px each, 132 px), one Tab stop, arrows move and select. Without Part C the row is absent and the suffix is the fixed "mm".
4. **Fit to content**: a full-width 244 x 28 px text button (outline 1 px `--toolbar-icon` at 60 %, hover `--editor-accent-hover`, focus ring as the panel's). Removed from the tree when the document has no objects (criterion 22 "not offered"; the panel never shows disabled controls). It is the last row, so its removal moves nothing.

Whole section about 192 px (24 + 8 + 4 rows + 3 gaps + padding).

**Why Width and Height are not `ValueField`s.** The value field is for values the maker explores by dragging with a live preview. A size here is committed (one atomic operation that moves every object, criteria 17 and 19), its range is 1 to 100 000 mm so a fill bar would show nothing useful, and dragging would rewrite the whole document per frame. What is shared with the convention: label column, 28 px box, unit placement, typing rules (Enter commits and returns focus to the canvas, Tab commits and moves to Height, Escape or a press elsewhere restores, Enter on unedited text writes nothing), the validation chip, `aria-invalid`, the focus ring. No reset icon (there is no default), no drag, no fill bar.

**Validation (criterion 16).** The chip of the panel with the limits in the display unit, rounded inward so a message never promises a value the field refuses: mm "Enter a number from 1 to 100000"; cm "Enter a number from 0.1 to 10000"; in "Enter a number from 0.04 to 3937". The surrounding spaces, a decimal comma and `inputmode="decimal"` as in the panel's other typed fields. No unit text is parsed ("21cm" is "not a number").

**Tooltips** (text only, `side="left"`): Width/Height "Document size. A resize keeps the centre, so objects move with the document and nothing changes on screen." Unit "How lengths are shown on the rulers, here and in the status bar. Other fields stay in mm. Stored sizes are always mm." Fit to content "Resize the document to the extent of all objects, without margin. Objects move so the extent starts at 0, 0."

**Feedback.** The fields, the rulers and the status bar change in the frame of the commit; the canvas content does not move (criterion 20). The one silent case is Fit to content on a fitted document (criterion 26): a text-only notice under the button for 3 s, "Already fits the content." (the "Action notice" component). No dialog, no confirmation.

**Units.** A unit change is a display change and one commit (criterion 36). A field with edited, uncommitted text commits in the old unit when the maker presses the unit group (blur commits, criterion 15) before the unit changes. Rulers re-label in the same frame; the fields re-render their values; the corner shows the new unit; the status bar follows.

**Keyboard.** `Shift+Ctrl+F` with nothing selected expands the panel and focuses **Width** with its text selected (DOM order is visual order: Width, Height, Unit, Fit). Escape in a field restores it and returns focus to the canvas. There is no Inkscape-style `Shift+Ctrl+D` (it would have to deselect to be of any use).

**Options B and C, briefly.** B (a "Document Properties" dialog) needs the product's first general dialog, hides the canvas feedback that proves the resize kept the content, and breaks the rule that numbers live in the panel; its only gain is Inkscape familiarity. C (the status bar readout edits inline) is cramped, has no room for Unit or Fit, is invisible as a control, and would be the first interactive status bar item. Recommendation stays A. The one weakness of A is discoverability with an object selected (Escape reveals the section); open design question 2.

### 4. Status bar

Left: "x: 12.3  y: 45.6" followed by the unit once, "mm", fixed decimals so the text does not jitter (mm 1, cm 2, in 3 decimals, the same 0.1 mm resolution; today the readout has no unit). Centre: zoom (unchanged). Right: "210.0 × 297.0 mm" in the same decimals (criteria 12, 21, 34); the unit is the display unit. Display only, no pressing, no tooltip. Stored values are untouched by the unit.

### Criteria changes requested

(All applied by the product owner on 2026-10-09, in criteria 1, 7, 10a, 11a, 12, 14, 14a, 16, 21, 22, 26, 28, 34. Kept for the record.)

- **1:** "a plain square" becomes "a square that shows the display unit".
- **7:** add "a label that would not fit entirely in the ruler's free span is not drawn; when labels are wider than the major spacing, labels are drawn on every second or fifth major tick".
- **10a (new):** wheel and pinch over a ruler act on the canvas (pointer clamped to the viewport).
- **11a (new):** on New and Open the document's top-left is 72 px from the viewport's top-left.
- **14, 16:** the validation message is written in the display unit with the limits rounded inward (mm "1 to 100000", cm "0.1 to 10000", in "0.04 to 3937"); Width and Height are typed fields, not drag fields.
- **14a (new):** the Document section is shown when nothing is selected and the Pen has no unfinished path; otherwise the panel shows Style or is empty. Amends `0017` criteria 1 and 2 (text only; the empty frame rule for the Style area stays).
- **22:** "not offered" is the removal of the button from the tree.
- **26 (addition):** a notice "Already fits the content." for 3 s.
- **28 (addition):** colours fixed as `--canvas-bg` document and `--pasteboard-bg` #B8B8BE; contrast 1.61 and 1.44 measured.
- **12, 21, 34:** status bar decimals per unit as in section 4; the cursor readout gains the unit.

### Open design questions (defaults taken)

1. Pen with an unfinished path hides the Document section. Default: yes. Alternative: show it and let the resize move only the committed objects (surprising).
2. Discoverability of the section with an object selected. Default: Escape reveals it; no extra route. Alternative: the status bar size readout becomes a button that deselects and focuses Width.
3. A 1 px document edge line (`--toolbar-icon` at 35 %, under the artwork, outside the document area). Default: no (criterion 28). Alternative: yes, if the colour step proves too faint on real displays.
4. An aspect-ratio lock between Width and Height. Default: none (a sheet size is typed both ways; the spec has no proportional resize).
5. Initial view offset of 72 px (criterion 11a). Default: yes.

## As-built notes

- **PR 1 (model), 2026-10-09.** Resize and fit are atomic for one editor. Between collaborating peers they merge field by field (width, height and each object position are separate registers), so two concurrent resizes of different axes can leave the objects centred on neither peer's intent. Accepted as a collaboration limit while sync is not built; tracked in `docs/technical-debt.md` ("Resize and fit merge per field across peers").
- **Retyping shown text (criteria 16 and 35).** The text a field shows for a limit is accepted when typed back and clamped onto the limit. This matters in inches only: the largest size shows as "3937.0079" in, 0.0007 mm over 100 000 mm. Any other value past a limit is refused as criterion 16 says (1e-9 mm tolerance).
- **PR 2 (rulers and pasteboard), 2026-10-09.**
  - **11a:** a window resize keeps the document's top-left corner until the first pan or zoom; this amends `0004` criterion 10 for an untouched view. Measured in the browser: without it, a resize of a fresh view moves the corner to wherever keeping the centre puts it (a window shown, maximised or resized after the project was created does this). The size reports at attach do not drift: `attach_canvas` records the laid-out size.
  - **7:** the major step leaves the widest label plus 4 px (criterion 5); labels thin to every 2nd or 5th major tick when the spacing is below the widest label plus 8 px, so a label never touches the next major tick. The wording "labels wider than the major spacing" reads as "label plus 8 px wider".
  - **2:** the rulers repaint in the same task as the canvas on a window resize and on a Properties panel toggle (the editor session's view-resized hook), and in the animation frame for pan, zoom, pointer and unit changes.
- **PR 3 (panel), 2026-10-09.**
  - **14, 14a:** the Document section is the panel's content while no object is selected and the Pen has no unfinished path. Until `0017-style-panel-rework` is built, the Style area with nothing selected (all controls disabled) is replaced by it; with a selection the panel is Style as before; with an unfinished Pen path and nothing selected it is empty.
  - **15:** leaving a size field with edited text commits it (the field keeps a refused value and its message); Escape restores. `NumberField` has a `commitOnBlur` option for this; the Style fields still restore on leaving.
  - **33 to 37 (Part C):** built as option A of Question 2 (mm, cm, in for rulers, size fields and status bar). It stays a proposal until the customer accepts it; removing it means deleting the Unit row and `set_display_unit` calls.
  - **35:** typed inches are converted as `value * 254 / 10`, so 8.5 in is the double nearest to 215.9 mm.
  - **14a, 15:** the Pen's unfinished path ends as drawn when the Pen is left (a lone node is dropped), and no resize or fit runs while a path exists. A press on the canvas first blurs a focused panel field, so a size typed there is applied before the tool sees the press.
  - **20:** after a resize or fit the view moves by the same shift as the objects (`Viewport::pan_by_document_offset`); the untouched default view keeps its 72 px inset.

## Links

Requirements: R-EDIT-018 (new, added with this refresh).
Related: `specs/0001-project-file-foundation/specification.md` (the existing `DocumentSize` and status bar size field), `specs/0004-canvas-navigation-and-selection/specification.md` and `adrs.md` (`ViewTransform`, zoom range), `specs/0017-style-panel-rework/specification.md` (panel rules: no popups, validation chip, criterion 48; its criterion 1 is amended by criterion 14a here), `docs/design-system.md` (chrome, tokens), `docs/adr/0012-pages-in-the-document-model.md` (rejected), `docs/adr/0002-document-model-units-and-svg-round-trip.md` (mm, Y-down), `adrs.md` in this folder (rewritten 2026-10-09).
PR: #67 (model), #72 (rulers, pasteboard and the Document panel).
