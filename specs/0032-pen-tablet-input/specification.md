# Pen tablet input: pressure from a stylus sets the width of Pencil strokes

Status: Ready (2026-10-10: `adrs.md` and the UX notes exist; customer decisions of 2026-10-10 applied; no customer question blocks; the device and OS questions are confirmed at the demo, see the section of that name)
Priority: Should (was Could; the customer does not want it at the back of the queue, 2026-10-10)
Origin: Customer ("later, not now", 2026-10-09; promoted on 2026-10-10 with the defaults below). The minimal Pencil tool inside this slice, the numbers and the texts are my proposals; the customer can veto the Pencil tool at the demo.

## User value

As a maker who draws with a pen tablet (Wacom, Huion, XP-Pen) I want the pen's pressure to change the width of what I draw, so that freehand lettering, sketches and inlay patterns look drawn and not traced with a mouse, and so that the result is still a clean outline that the laser, the plotter or the vinyl cutter can follow.

## What the customer decided (2026-10-10)

1. **Input path:** pressure and tilt come from the browser's Pointer Events in the webview. That works the same on every desktop platform, needs no new dependency, and the desktop shell uses the same webview as the browser build.
2. **Where pressure acts:** on the width of freehand, pencil-like strokes only, at first.
3. **Fallback:** a mouse, or a pen that sends no usable pressure, draws a plain path. Nobody is worse off.
4. **Order:** built after `0020-undo-redo` and `0047-transform-polish`. The customer does not want it pushed to the back.
5. **Hardware and OS questions** are confirmed at the demo, not before (section "To confirm at the demo").

## What exists today

Mouse input only. The editor receives pointer events from the webview (ADR 0001: Tauri, the editor core as wasm in the webview). The Pen tool places Bézier nodes by clicking. There is **no freehand tool**, so pressure has nothing to act on. Decision of mine, inside this slice: a **minimal Pencil tool** (press, drag, release; criteria 6 to 13) comes with it, because otherwise the slice cannot be tried end to end. It is useful with a mouse too. Question 1 offers the alternative.

## Reference tools

- **Inkscape:** the Pencil tool has "Use pressure input" (it applies the PowerStroke path effect with Min/Max width and end caps); the Calligraphy tool reads pressure and tilt and writes a filled outline with many nodes. Pressure support depends on the platform and has open bug reports (an inverted pressure curve, a width that stays fixed at light pressure). The width stays editable only through the path effect.
- **Illustrator:** a Blob Brush and the Paintbrush with a pressure-sensitive Calligraphic brush; the Width tool edits the profile afterwards.
- **Affinity Designer:** a Vector Brush tool with pressure and a pressure curve next to the brush; width profiles are editable on the path.
- **LightBurn:** no pen input; it draws nothing freehand.
- What we do better: one inline pressure curve with three named choices, the result is clean outline geometry that cuts well and round-trips through SVG, the controls appear only when a pen has been seen, and a mouse user is never affected.

## Words used below

- **Sample:** one pointer reading: position, `pointerType` (mouse, pen, touch), `pressure` 0 to 1, `tiltX` and `tiltY` in degrees, `buttons`, time.
- **Stroke:** the samples from the press (contact) to the release.
- **Pressure present:** criterion 3.
- **Centre line:** the Bézier path fitted to the samples.
- **Width at a sample:** Minimum + (Maximum − Minimum) x curve(smoothed pressure), criterion 9.

## Acceptance criteria

Tests simulate the device: pointer events built with `pointerType: "pen"`, `pressure`, `tiltX`, `tiltY` and `buttons`, dispatched on the canvas element (frontend tests), and the same records handed to the session function (Rust tests). No tablet is needed for the gate; a real tablet is for the demo.

### Input

1. Given a pointer event in the webview, then the editor's pointer input record carries, besides position and buttons, `pointerType`, `pressure`, `tiltX`, `tiltY` and the eraser flag (`buttons` bit 32), exactly as delivered. Given a mouse, then every existing behaviour is unchanged (all existing mouse tests pass without edits). Tilt and the eraser flag are carried and not used: nothing reacts to them (Question 3).
2. Given a Pencil stroke, then the tool takes every sample from `getCoalescedEvents()` of a move event when the webview provides it, and only the delivered event otherwise. Test: one move event with 5 coalesced samples gives 5 samples; one without the method gives 1.
3. Given a stroke, then it **has pressure** when its `pointerType` is "pen" and at least one sample has a pressure other than 0 and other than 0.5. A mouse, a pen driver that reports a fixed 0.5 (what the specification gives a device without pressure) and a pen that reports 0 on contact (reported for some Linux webviews, unconfirmed) have none. Tests: pen with pressures 0.1 to 0.9: present; pen constant 0.5: absent; pen constant 0 with contact: absent; mouse: absent.
4. Given a touch pointer (`pointerType` "touch"), then the Pencil ignores it: no stroke, nothing changes. (Touch drawing and palm rejection are for the mobile companion.)
5. Given a pen in range but not touching (`buttons` 0), then it moves the pointer as a mouse move does: no stroke starts, hover highlights and cursors work unchanged. Test: a pen move with `buttons` 0 starts nothing; a pen down with `buttons` 1 starts a stroke.

### The Pencil tool

6. Given the tool rail, then a Pencil tool exists next to the Pen (the seventh and last button of the Tools card, glyph Lucide `PencilLine`, key `P`; see the UX notes). With the Pencil active, press, drag and release draws a stroke with a mouse or a pen. The cursor is a crosshair.
7. Given a stroke without pressure, when it is released, then one new **open path** is created. Its centre line is fitted to the samples within 0.1 mm (every sample lies within 0.1 mm of the path) and has few nodes. It gets the style a new Pen path gets today. Tests: 100 samples on a straight line give a path of 2 nodes; 200 samples on a half circle of radius 30 mm give a path of fewer than 10 nodes.
8. Given a stroke with pressure, when it is released, then one new **closed path** is created: the outline of the centre line with the width of criterion 9, round ends, Fill set to the colour that a new Pen path has as stroke colour, stroke None. No model change and no format change (Question 2, A). The outline is a polyline within 0.02 mm of the exact variable-width outline of the centre line. Tests: constant pressure 1.0 along a straight line of 50 mm, Maximum 2 mm: a capsule of area 50 x 2 + π x 1 ≈ 103.1 mm² within 0.3 mm². Pressure rising linearly from 0 to 1 along 50 mm, Minimum 0.2 mm, Maximum 2 mm, curve Linear: the outline is 0.2 mm wide at the start (plus the round end) and 1.1 mm wide at x = 25 mm, within 0.05 mm.
9. Given the samples of a pressure stroke, then pressure is smoothed first and the curve applied second. Smoothing: the pressure of a sample is the mean of its own pressure and that of the two samples before it (fewer at the start). Curve: Soft = p^0.5, Linear = p, Firm = p². Tests: pressures 0.2, 0.2, 1.0, 0.2, 0.2 smooth to 0.2, 0.2, 0.467, 0.467, 0.467 (±0.001); a smoothed 0.25 gives 0.5 with Soft, 0.25 with Linear and 0.0625 with Firm, so with Minimum 0.2 mm and Maximum 2 mm the widths are 1.1, 0.65 and 0.3125 mm.
10. Given 2000 samples along a smooth S-curve 150 mm long, then the result of criterion 8 has fewer than 600 nodes (a third of the samples), and the release of the stroke completes in under 200 ms in a release build on the reference desktop, so the pointer never waits on it.
11. Given a stroke in progress, then the canvas shows it in blue (the blue-new/black-old convention of `0009`): the line for a stroke without pressure and the outline with its widths for a stroke with pressure, updated at least once per animation frame for strokes up to 2000 samples. On release the preview goes and the object appears in black; the object lies within 0.15 mm of the last preview.
12. Given a stroke in progress, when the maker presses Escape, then the stroke is cancelled and nothing is written (the first step of the Escape cascade, `0010` criterion 42). Given a stroke of fewer than 2 samples or a total length below 0.1 mm, then nothing is created and no commit is made.
13. Given a successful stroke, then exactly one commit is made, stored as `pencil_stroke`, which `0020` takes as one undo step named "Pencil stroke"; the new object is selected, as after a shape tool (`0009`); the active tool stays the Pencil.

### Settings, readout and privacy

14. Given no stroke with pressure yet in this session, then the Pencil bar contains no pressure control. Given the first stroke with pressure, then the bar shows **Minimum width** (mm, default 0.2, range 0.05 to 20), **Maximum width** (mm, default 2, range 0.1 to 50, never below Minimum: a value below it is raised to it) and **Curve** (Soft, Linear, Firm; default Linear), as value fields (`0017`) and a segmented group. A change applies from the next stroke. The values live for the session; they are not saved in the document. Test: simulate a pen stroke with pressure; the controls appear; a mouse-only session never shows them.
15. Given a pen in range or touching, then the Pencil bar shows a muted readout "Pen: pressure 0.62, tilt 12° / -3°" (for the demo; it tells the customer whether the webview delivers pressure and tilt on his system, and may be removed after it, Question 4). It is not in the tree for a mouse.
16. Given any input, then no device identifier, pointer id or tablet name is stored or sent: not in the document, not on disk, not over the network (R-SYS-007, R-SYS-002). The pressure curve choice and the widths are session settings.

## To confirm at the demo

These are the customer's, not criteria. The demo is the first run with a real tablet; the readout of criterion 15 shows the facts.

1. **Which tablet and which operating system** does the customer test with? Without a real device the pressure path cannot be verified; the gate tests only simulate it.
2. **Does pressure (and tilt) arrive in the webview** on Linux (WebKitGTK), Windows (WebView2) and macOS (WKWebView)? As far as I know Windows and macOS deliver it for Wacom-class tablets. For Linux I found a report of a Tauri app where `PointerEvent.pressure` stayed 0 while the position worked; I could not confirm it for the WebKitGTK version we ship.
3. **If Linux does not deliver pressure:** the pen then draws as a mouse (criterion 3, graceful). Options for pressure there: read the pen axes in the desktop shell (GDK) and forward them, which is an ADR (platform integration, possibly `unsafe`) and would be its own slice; or accept mouse behaviour on Linux until the webview improves. Default: accept, decide after the demo.
4. **Is the pressure curve right** (Soft, Linear, Firm) and the default widths 0.2 and 2 mm? Is the readout still wanted?
5. **Tilt and the eraser end:** do you want the eraser end to delete whole objects, and tilt to turn a nib? Both are carried in the input record and unused (Question 3).
6. **Editing the width afterwards:** is the expanded outline enough, or must the width stay editable (Question 2, B)?

## Out of scope

- **Eraser end action** (delete whole objects, or cut through paths with `0036-cut-at-crossings`) and **tilt effects** (a calligraphic nib): carried in the input, no behaviour (Question 3).
- **Editable width profiles on a path**, PowerStroke-like effects, brushes (`0033-stroke-brushes`); the stored result is an outline (Question 2).
- **Pencil options** beyond the three controls of criterion 14: smoothing strength, closing a shape, joining to an existing path, shape recognition.
- **Barrel buttons** (nothing is bound), **palm rejection** beyond ignoring touch (criterion 4), touch drawing, the mobile companion.
- **Pen input for the Node tool, the Select tool or the shape tools** beyond hover and clicking as a mouse does.
- Raster painting, pixel brushes, bristle simulation, smudge, blend modes; handwriting recognition.
- Tablet driver configuration, button mapping, calibration (the operating system's job).
- Storing pressure samples in the document for re-fitting; azimuth, twist, altitude.
- **Undo and redo** itself (`0020`); this slice only adds its commit label.

## Open questions (customer)

Each has a default; nothing blocks.

1. **The Pencil tool in this slice.** *A (default):* yes, minimal, because pressure needs a stroke to act on and the slice must be tryable. *B:* a separate story "freehand-draw" first (mouse only; Inkscape's Pencil is the reference), pressure after it. Recommendation: A; the Pencil here is 13 criteria and shares everything with the pressure path.
2. **Stored result.** *A (default):* the pressure stroke becomes a closed, filled outline (stroke off, fill on): no model or format change, exactly the geometry a laser or vinyl cutter wants, round-trips through SVG. The centre line and the width are gone afterwards. *B:* a width profile on the path, editable later (PowerStroke, Illustrator's Width tool): a model and format change that touches renderer, hit test, booleans, offset and export, and the same model the width-profile brushes of `0033-stroke-brushes` need. Recommendation: A now, B together with brushes.
3. **Eraser and tilt.** *A (default):* carried, unused. *B:* the eraser end deletes the whole object under the pen (the 8 px hit rule of `0014`), in any tool. *C:* the eraser cuts through paths (needs a freehand eraser path and Cut). Say after the demo.
4. **The readout (criterion 15).** Default: in the first version, removed when the customer says the platform question is settled.
5. **Priority.** Set to Should on 2026-10-10 at the customer's request.

Decided by the product owner (change if you disagree): the Pencil tool inside the slice; the stored result is an outline (2, A); the pressure rules of criteria 3 and 9 and the default widths; session-only settings; the commit label `pencil_stroke`; the controls appear after the first pressure stroke.

## UX notes

By `ux-engineer`, 2026-10-10. Numbers and rows are in `docs/design-system.md`: "Tool rail architecture", "Pencil tool", "Pencil bar", "Live preview outline", "Keyboard shortcuts established so far". Reference tools: Inkscape (Pencil on `P`, "Use pressure input" in its bar), Affinity Designer (separate Pen and Pencil tools, pressure curve beside the brush), Illustrator (Pencil and Pen as separate tools). All three keep Pen and Pencil apart.

### The seventh tool and the rail (the fix)

The Pencil is a seventh button in the Tools card (312 px instead of 268). With the Boolean card under it, column A would end at 12 + 312 + 8 + 224 = 556 px in a 546 px viewport: 10 px too tall, and any 12 buttons in two cards are 544 px, so only a second button size or 2 px gaps would fit it, and both are rejected below. **Proposed fix: the Boolean card moves from column A to column B**, below the Path card; the Group card (`0023`) takes its place under the Tools card. At 800 x 600:

| | Before (built) | With the Pencil, Group and all path commands |
|---|---|---|
| Column A | Tools 6 (268 px), Boolean 5 (224 px): ends 512 px | Tools 7 (312 px), Group 2 (92 px): ends 424 px, 88 px free |
| Column B | Path 2 (92 px): ends 104 px | Path 4 (180 px), Boolean 7 (312 px): ends 512 px |

Boolean is the only built card that moves, once, in the first release of `0023`, `0032` and `0037` to ship; every other change is an append (row "Tool rail architecture"). `--rail-right` stays 116 px, so the bars, notices and default view do not move. Rejected: Pen and Pencil on one button (a mode behind a glyph that changes; a tablet user wants the Pencil one press away), the Pencil in column B (a tool away from the tools), a second button size or 2 px gaps (a global change for 10 px, no margin left), a scrolling rail (hides dimmed commands). The Pencil is placed **last** in the Tools card, after Polygon/Star, because a new creation tool appends and no built tool changes slot (row "Pencil tool"); next to the Pen would have moved Node, Rectangle, Ellipse and Polygon/Star down by 44 px. The criterion 6 wording "the place, the glyph and the key are the ux-engineer's decision" is replaced by these.

### The tool

- **Button.** Lucide `PencilLine`, 20 px (not `Pencil`, which is the History tab's "Change" glyph; `PenTool` is the Pen). Name "Pencil tool", tooltip "Pencil tool (P)", and "Pencil tool (Esc, P)" while the Select tool has a selection, as the other letters. It is a tool: it takes the solid active ground.
- **Key.** `P`, Inkscape's Pencil key. `N` is the Node tool here and `M`, `K`, `H`, `V` are reserved; `P` was free. A tool letter in every state, like `B`, `N`, `E`, `*`; ignored while a drag, a chip, a focused control or an unfinished Pen path is active.
- **Cursor.** Crosshair (criterion 6), also for a pen in range but not touching, where hover and cursors behave as for a mouse (criterion 5).
- **Escape.** The first press cancels a running stroke and writes nothing (criterion 12); the next leaves the tool, as for the Pen. Nothing else in the cascade changes.
- **Input kinds.** Touch is ignored by the Pencil (criterion 4); the rail and the bars stay operable by touch. The eraser end and tilt are carried and unused; nothing in the UI mentions them until the customer decides (Question 3).
- **Freehand is pointer-only by nature.** The Pen is the click-based way to draw a path; no keyboard route for the Pencil is claimed, and none of the bar's controls needs a pointer.

### The stroke, live

A stroke in progress is the blue preview (row "Live preview outline"): a 1.5 px line for a stroke without pressure, the hollow outline with its widths for a stroke with pressure, white casing, one update per frame up to 2,000 samples (criterion 11). The preview changes kind in the frame the first usable pressure arrives (criterion 3: the first sample that is neither 0 nor 0.5), which can be a few samples into the stroke; the line becomes an outline and its width follows from then on. On release the preview goes and the black object appears in the same frame, within 0.15 mm of what was shown. The new object is selected and the tool stays the Pencil, so the next stroke starts at once.

### The Pencil bar

Hidden, not disabled: while only a mouse has been used there is no bar. When a pen is first seen (in range), the bar appears in the overlay row with the readout "Pen: pressure 0.62, tilt 12° / −3°"; after the first stroke with pressure it adds **Min** and **Max** width and the **Curve** choice (Soft, Linear, Firm; Linear preselected). Value fields use the Select bar's field (row "Bar number field"): Max is never below Min and a value below it is raised and shown raised; a change applies from the next stroke; everything lives for the session and is never saved (criterion 14). The readout is muted, a fixed 232 px box with tabular figures so it does not jitter, not a live region, and is removed after the demo if the customer says so (Question 4). No device name, id or pointer id is shown or stored (criterion 16). At 800 x 600 (bar row 356 px) the groups wrap to three rows (108 px); the bar is for tablet users, and the wrap is accepted.

### Keyboard, accessibility, contrast

Rail button: Tab reaches the Tools card (own stops until the customer makes it one roving toolbar, row "Rail keyboard model"), Space or Enter picks the tool. Bar: Tab order Min, Max, Curve; arrows in Curve (`role="radiogroup"`, one Tab stop); field names "Minimum width" and "Maximum width"; the unit mm in the field. Readout `--panel-muted-fg` 5.0:1 on `--toolbar-bg`; blue preview 1.5 px with a white casing reads on every fill. Targets: the rail button is 40 px; bar controls are 28 px high, above the 24 px minimum, which matters with a stylus.

### At 800 x 600, large documents

The rail ends 512 px down (34 px clear) in column B and 424 px in column A. A 2,000-sample stroke previews per frame and finishes in under 200 ms (criterion 10), so the pointer never waits; the result of a long stroke is under 600 nodes. Many strokes add many small objects: selecting the last one only, as after any shape tool, keeps the selection overlays cheap.

### Questions for the customer

1. **The Boolean card moves from column A to column B** (the fix above, needed by the Pencil, Group, and Fracture and Flatten). *Default: yes.* *Option:* Pen and Pencil share one button, nothing moves, the Pencil sits behind a mode in the Pen's bar and its glyph changes with the last used mode; rejected by me because a tablet user wants the Pencil one press away.
2. **Key `P` and the position at the bottom of the Tools card.** *Default: yes.* Illustrator uses `N` for the Pencil, but `N` is the Node tool here (Inkscape's key).
3. **Demo checks for the readout** (Question 4): keep the readout in the bar until the platform question is settled; it appears only after a pen has been seen.

## Links

Requirements: R-INP-001 (`docs/requirements.md`); related R-VEC-003, R-EDIT-021
Builds on: `specs/0020-undo-redo/` (one stroke is one step), `specs/0047-transform-polish/` (customer's order: after it), `specs/0009-unified-object-editing/` (blue-new/black-old), `specs/0010-edit-interaction-polish/` (Escape cascade, shortcut table), `specs/0017-style-panel-rework/` (value fields)
Related: `specs/0033-stroke-brushes/` (pressure and brushes share the width model), `specs/0036-cut-at-crossings/` (eraser, Question 3), `docs/adr/0001-ui-framework-and-canvas-rendering.md` (webview, WebKitGTK on Linux), `docs/adr/0002-document-model-units-and-svg-round-trip.md` (document model, unchanged by default)
ADRs: `adrs.md` (architect, to come; input record and coalesced events, the variable-width outline routine in `curvyo-geometry-core`; no document-model ADR is needed on the default of Question 2, A)
PR: TBD
