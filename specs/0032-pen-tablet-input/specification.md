# Pen tablet input: pressure, tilt and eraser for freehand drawing

Status: Draft
Priority: Could
Origin: Customer ("later, not now", 2026-10-09). Everything below is a sketch for the customer to react to, not a story to build yet. Numbers and names are proposals until the customer has read them. No `adrs.md` content yet.

## User value

As a maker who draws with a pen tablet (Wacom, Huion, XP-Pen; later an iPad or Android tablet with a stylus) I want the pen's pressure to change the width of what I draw, and the eraser end of the pen to erase, so that freehand lettering, sketches and inlay patterns look drawn and not traced with a mouse, and so that the result can still go to the laser, the plotter or the vinyl cutter as clean outlines.

## What exists today

Mouse input only. The editor receives pointer events from the webview (ADR 0001: Tauri, the editor core as wasm in the webview). The Pen tool places Bézier nodes by clicking; there is **no freehand drawing tool** at all. Without one, pressure has nothing to act on, so this feature needs a freehand tool first (see Question 4).

## Reference tools

- **Inkscape:** the Pencil tool has "Use pressure input" (it applies the PowerStroke path effect, with Min/Max width and a choice of end caps); the Calligraphy tool reads pressure and tilt and writes a filled outline with many nodes. Pressure support depends on the platform and has open bug reports (a pressure curve that inverts, a width that stays fixed at light pressure). The width stays editable only through the path effect.
- **Illustrator:** a Blob Brush and the Paintbrush with a pressure-sensitive Calligraphic brush; the Width tool edits the profile afterwards.
- **Affinity Designer:** a Vector Brush tool with pressure and a pressure curve that is edited next to the brush; stroke width profiles are editable on the path.
- **LightBurn:** no pen input; it draws nothing freehand.
- What we can do better, if built: the pressure curve is one inline control, the result is either clean outline geometry that cuts well or an editable width profile (Question 2), and a mouse user is never affected.

## Rough acceptance criteria (sketch; each gets numbers and tests before Ready)

1. **Input events carry the pen.** The editor's pointer input has, besides position and buttons: pointer type (mouse, pen, touch), pressure 0 to 1, tilt (x and y, degrees) and "eraser end" (the W3C Pointer Events fields `pointerType`, `pressure`, `tiltX`, `tiltY` and `buttons` bit 32). A mouse reports no pressure and behaves exactly as today.
2. **A feasibility check per platform comes first.** Windows (WebView2) and macOS (WKWebView) expose pen pressure through Pointer Events for Wacom-class tablets as far as I know; Linux (WebKitGTK, the customer's primary platform) is doubtful: I found a report of a Tauri app on Linux where `PointerEvent.pressure` stayed 0 while position worked, and Firefox's own Linux backend needed explicit GDK axis support added in 2024. This is a report I could not confirm for the WebKitGTK version we ship. The first task is a one-page spike with a real tablet on Linux, Windows and macOS: does pressure, tilt and the eraser arrive in the webview? If not on Linux, the fallback is to read the pen axes natively in the Tauri host (GDK on Linux) and forward them to the editor. Which path is used is an ADR (platform integration).
3. **Pressure changes width, as a setting of the freehand tool.** Two inline fields in the tool's bar: Minimum width and Maximum width (value fields as in `0017-style-panel-rework`), and a pressure curve with three choices (Soft, Linear, Firm; a freely drawn curve is out of scope). Pressure 0 gives the minimum width, pressure 1 the maximum. Smoothing: pressure is smoothed over the last few samples so a jittery tablet does not make a wavy line.
4. **Samples are fitted, not stored raw.** The sampled path (position, pressure, time) is turned into a few Bézier segments within a stated deviation (R-VEC-003 style simplification, default 0.1 mm), with pressure interpolated along the path. A 10-second stroke at 200 Hz must not produce 2000 nodes.
5. **Hover works as with a mouse.** A pen in range but not touching moves the pointer with pressure 0 and no button: hover highlights, cursors and the close-path and continue-path cues work unchanged.
6. **Eraser end.** Touching the canvas with the eraser end deletes the object under the pen, with the Select tool's 8 px hit rule (`0014-advanced-selection`), in any tool. It deletes whole objects, not parts of them (this is a vector editor). Because there is no undo yet, the first use shows a one-line notice. A part-erasing eraser is a different feature (Question 6).
7. **Barrel buttons and touch.** The pen's barrel button acts as a right click (nothing is bound to it yet). While a pen is in range, touch input on the canvas does not draw (palm rejection at application level, in addition to what the operating system does); two-finger pan and pinch zoom still work. Mostly relevant once a touch device runs the app.
8. **No pressure device, no change.** All of this is invisible to a maker with a mouse: no setting, no extra control in a bar until a pen has been seen in the current session.
9. **Privacy and offline.** No device identifiers leave the machine, nothing is stored about the tablet (R-SYS-007, R-SYS-002). A pressure curve choice is an app setting, not a document value.

## What is stored (the real design question)

A pressure stroke has a width that varies along the path. The document model has one stroke width per object today (`0007`, `0017`), so there are two ways, and the choice is an ADR on the document model (customer decision, `CLAUDE.md` §3):

- **A (recommended first step): expand at draw time.** The stroke becomes an ordinary closed, filled path (an outline of the variable-width line), with the stroke off and the fill on. No model change, no format change, exactly the geometry a laser or vinyl cutter wants (a contour), and it round-trips through SVG. Cost: the centre line and the width are gone, so the maker cannot change the width afterwards; the outline is dense in nodes unless it is fitted (criterion 4).
- **B: a width profile on the path.** Each node gets a width (or a width profile along the path), the stroke style gets a "variable width" switch, and the renderer, hit test, bounds, Booleans, offsetting and export all learn it. Editable afterwards, comparable to Inkscape's PowerStroke and Illustrator's Width tool. A model and format change that touches almost everything. It is the same model that pressure-driven brushes need (`0033-stroke-brushes`), so the two features should share one decision.

## Open questions (customer)

1. **Hardware.** Which tablet do you have, and on which operating system will you test it? The spike (criterion 2) needs one real device; without it the feature cannot be verified. Default: none until you name one.
2. **Stored result (A or B above).** Default: A first. B only when a story needs editable widths, together with brushes.
3. **Platforms in order.** Default: Windows and macOS through the webview, Linux after the spike shows what WebKitGTK does; the browser build gets pressure for free where the browser supports it; mobile after the desktop.
4. **A freehand tool.** There is no freehand drawing tool today. Default: a separate story "freehand-draw" (mouse first: draw, smooth, fit curves, Inkscape's Pencil tool as the reference) that comes before this one. I have not written it; say if you want it specified now.
5. **Tilt.** Default: read and ignored. Possible later use: a calligraphic nib whose angle follows the tilt (`0033-stroke-brushes`).
6. **Eraser.** *A (default):* the eraser end deletes whole objects it touches. *B:* it cuts through paths (needs the Split command, `0036-split-at-crossings`, and a freehand eraser path). *C:* ignore the eraser end.
7. **Priority.** Default: Could, after the laser MVP. Raise it to Should if you draw with a tablet every day.

## Out of scope

- Raster painting, pixel brushes, bristle simulation, smudge, blend modes.
- Handwriting recognition, shape recognition ("draw a rough circle, get a circle").
- Tablet driver configuration, button mapping, calibration screens (the operating system's job).
- Stylus hover tilt preview, azimuth and twist, barometric altitude.
- Pen input for the Node tool, the Select tool or the shape tools beyond hover and clicking as a mouse does.
- Touch drawing with a finger; the mobile companion app.
- Storing pressure samples in the document for later re-fitting.

## UX notes

(filled in by ux-engineer before Ready)

## Links

Requirements: R-INP-001 (`docs/requirements.md`); related R-VEC-003, R-EDIT-021
Related: `specs/0033-stroke-brushes/` (pressure and brushes share the width model), `specs/0036-split-at-crossings/` (eraser, Question 6), `docs/adr/0001-ui-framework-and-canvas-rendering.md` (webview, WebKitGTK on Linux), `docs/adr/0002-document-model-units-and-svg-round-trip.md` (document model)
ADRs: `adrs.md` (architect, to come; a document-model ADR is `needs-customer`)
PR: TBD
