# Shape creation from the center: Shift and Shift+Ctrl while drawing

Status: Done
Done, with: criteria 16 and the first sentence of 17 are superseded by `unified-object-editing` (criterion 25); the shape tools are creation-only.
Priority: Must
Origin: Customer

## User value

As a maker I want to hold Shift while dragging out a rectangle or ellipse so
that the point where I pressed becomes the shape's center, and hold Shift and
Ctrl to get a square or circle around that point, so that I can place a
mounting hole or a centered pocket exactly where I click instead of
calculating its corner.

The customer's words: "mir ist aufgefallen, dass beim Erstellen der Primitive
bei Shift+Ziehen das Objekt aus der Mitte gezogen werden sollte (und
entsprechend macht Shift+Strg aus der Mitte ein Quadrat / einen Kreis)."

**Field reference.** Inkscape's rectangle and ellipse tools: Ctrl makes a
square or circle, Shift draws around the press point, both together draw a
square or circle around it. LightBurn: Shift draws a square or circle,
Ctrl draws from the center. Inkscape's bindings are what this product already
follows for Ctrl (`specs/0003-primitive-shapes/specification.md` criteria 2
and 8) and for resizing (`specs/0005-object-transform/specification.md`
criteria 5 and 7: Ctrl proportional, Shift about the center), so this spec
adds the one missing piece and keeps the rule the same everywhere: **Ctrl =
1:1, Shift = around the center.** Where we do better: the numeric readout
already on screen during the drag shows the final size under each mode, and
both modifiers can be pressed and released mid-drag with the pointer held
still.

## The rule

Let A be the press point and B the pointer. Define the effective endpoint E:

- Ctrl not held: E = B.
- Ctrl held: E = A + (s_x · m, s_y · m), where m = max(|Bx − Ax|, |By − Ay|)
  and s_x, s_y are the signs of Bx − Ax and By − Ay (an axis with zero
  movement counts as positive). This is the shipped rule of `primitive-shapes`
  criteria 2 and 8; it is not changed here.

The shape's box is then:

- Shift not held: the axis-aligned box with opposite corners A and E (shipped
  behaviour).
- Shift held: the axis-aligned box with opposite corners E and its mirror
  through A, 2A − E. A is the box center.

Polygon and star are not part of this rule: their create-drag is unchanged
(criterion 17).

## Acceptance criteria

All criteria use document-space millimeters; "the pointer" is the pointer
position of the event in question, and tolerance for comparing lengths is the
document's own geometric tolerance. Shift and Ctrl are read as the host
reports them (Ctrl is Cmd on macOS, as everywhere else in the product).

### Rectangle tool

1. Given the rectangle tool is active and nothing is being dragged, when the
   maker presses at A = (100, 50), holds Shift, drags to B = (130, 40) and
   releases, then one rectangle is created with origin (70, 40), width 60 and
   height 20 (center (100, 50)), zero corner radius and rotation 0.
2. Given the same press, when the maker drags to each of B = (70, 60),
   (70, 40) and (130, 60) with Shift held, then the result is the same
   rectangle as in criterion 1 in every case: the box depends only on
   |Bx − Ax| and |By − Ay|, never on which side of A the pointer is.
3. Given the rectangle tool, press at A = (100, 50), when the maker holds
   Shift and Ctrl, drags to B = (130, 40) (|dx| = 30, |dy| = 10) and
   releases, then a square is created with origin (70, 20) and width and
   height 60: the larger of the two extents wins, as with Ctrl alone.
   Dragging to (110, 20) (|dx| = 10, |dy| = 30) gives origin (70, 20), width
   and height 60 too.
4. Given Ctrl alone (no Shift), when the maker presses at A = (100, 50) and
   drags to B = (130, 40), then the result is the same as before this spec: a
   square with corners A and (130, 20), origin (100, 20), width and height 30.
   With neither modifier, the result is the same as before this spec:
   corners A and B.

### Ellipse tool

5. Given the ellipse tool is active, when the maker presses at A = (100, 50),
   holds Shift, drags to B = (130, 40) and releases, then one ellipse is
   created with center (100, 50), rx 30, ry 10 and rotation 0. As with the
   rectangle (criterion 2), the result does not depend on which side of A the
   pointer is.
6. Given the same press, when the maker holds Shift and Ctrl and drags to
   B = (130, 40), then a circle is created with center (100, 50) and
   rx = ry = 30 (the larger extent wins). Dragging to (110, 20) gives center
   (100, 50) and rx = ry = 30 too.
7. Given Ctrl alone or neither modifier, the ellipse is created exactly as
   before this spec (`primitive-shapes` criteria 7 and 8): the bounding box
   has corners A and E, so a drag from (100, 50) to (130, 40) gives center
   (115, 45), rx 15, ry 5.

### Live preview, readout and modifier changes

8. Given a create-drag with the pointer held still at B, when the maker
   presses or releases Shift or Ctrl (canvas keyboard focus, as for
   `object-transform` criteria 5 and 7), then the preview outline and the
   numeric readout change to the mode now in effect on the next rendered
   frame, with no pointer movement. This holds for every combination:
   none, Ctrl, Shift, Shift+Ctrl, in any order of pressing and releasing.
9. Given a create-drag in progress, then the preview is the exact shape that
   would be committed if the button were released at that moment with the
   same pointer position and modifiers: same outline, same position, same
   size, to the tolerance above. Preview and commit use one computation, so
   for any sequence of moves and modifier changes the committed shape equals
   the last preview.
10. Given the maker releases the button, then the shape is built from the
    pointer position and the modifier state of the release event, as the
    shipped Ctrl behaviour already does (`primitive-shapes` criterion 2). If
    the key state changed since the last preview without any event reaching
    the canvas (for example the window lost focus), the release event's state
    wins.
11. Given a create-drag in centered mode, then the numeric readout keeps the
    shipped format and meaning and reports the final shape, not the half-box:
    for a rectangle "W × H mm" is the full width and height (criterion 1:
    "60.0 × 20.0 mm"); for an ellipse "rx × ry mm" is the radii (criterion 5:
    "30.0 × 10.0 mm"). It stays anchored at the pointer position, which in
    Ctrl modes is E (the constrained corner), as now.
12. Given a create-drag where the pointer is at A (including no movement at
    all), when any combination of modifiers is pressed or released, then no
    preview is drawn and nothing is created (`primitive-shapes` criteria 1
    and 7). A drag with only one axis of movement is treated exactly as
    without Shift: it is not refused, and under Shift without Ctrl the
    shape's other dimension is zero as it is today. (Pre-existing; see "Out
    of scope".)

### Commit, cancel and unaffected behaviour

13. Given a completed create-drag in any mode, then exactly one object is
    written to the document in one commit. The stored fields are the existing
    ones (rectangle origin, width, height, corner radius 0; ellipse center,
    rx, ry) and rotation 0; nothing records which modifiers were used, and
    the saved `.curvyo` file has the same `format_version` as before.
14. Given a create-drag in progress with Shift, Ctrl or both held, when the
    maker presses Escape, then the drag is cancelled, no object is written,
    no preview remains, and pressing or releasing modifiers afterwards does
    not bring it back (`primitive-shapes` Escape behaviour, unchanged).
15. Given a create-drag in progress in centered mode, when the maker pans or
    zooms the canvas mid-drag (`canvas-navigation-and-selection` criterion
    24), then A stays fixed in document space and remains the box center.
16. **Superseded by `specs/0009-unified-object-editing/` criterion 25** (a press in
    a creation tool never selects, toggles, moves or handle-drags an existing
    object; it always creates). Original text kept for history: Given a press that lands on an existing shape of the active tool's kind
    (its outline or, when selected, a handle) within the existing hit
    tolerances, then nothing changes: with no modifier it selects or starts a
    handle drag; with Shift it toggles that shape's selection
    (`canvas-navigation-and-selection`, `advanced-selection`); no create-drag
    starts. Shift pressed on empty canvas starts a create-drag and leaves the
    selection handling it has today. Only a press that starts a create-drag
    is affected by this spec.
17. The first sentence is **superseded by `specs/0009-unified-object-editing/`**
    (the shape tools have no handles any more, so there is no handle drag under
    a tool's own tool; the Select tool's own modifiers stay as in
    `object-transform`). Original first sentence, kept for history: Given the
    maker drags a resize or corner-radius handle of a selected rectangle or
    ellipse under its own tool, or any drag in the Select tool, then Shift and
    Ctrl have no effect from this spec. The rest of the criterion stands:
    given the polygon/star tool,
    then a create-drag behaves exactly as it does today and as before this
    spec (verified in the running app, build of `main`): the press point is
    the shape's center, the pointer is one vertex (the tip, for a star), the
    pointer's angle from the press point sets the shape's rotation, Shift has
    no effect, and Ctrl snaps the angle (`edit-interaction-polish` criterion
    4). This is unchanged by this spec, and the customer is happy with it.

## Out of scope

- Alt, further modifiers, and ratio presets (golden ratio, 1:2, ...); only
  the 1:1 constraint exists (`primitive-shapes` "Out of scope").
- Any change to polygon and star creation. Current behaviour, verified in
  the app: press = the shape's center, pointer = one vertex (the tip, for a
  star), the pointer's angle sets the rotation, Shift has no effect, and
  Ctrl snaps the angle (`edit-interaction-polish` criterion 4). Unchanged by this spec; the customer is happy with it.
- Modifier behaviour on shape-tool handle drags (resize, corner radius,
  inner radius). The Select tool already has Ctrl and Shift for resizing.
  (Superseded: the shape tools no longer have handles,
  `specs/0009-unified-object-editing/` criterion 25.)
- Changing what happens with a one-axis-only drag (zero height or width). It
  is created today, with or without Shift; whether to refuse it is a separate
  question and not part of this request.
- Fixing that a newly created shape is not selected after release, although
  `primitive-shapes` criterion 1 says it becomes the selected object. This
  predates this spec and is unchanged by it. (Superseded:
  `specs/0009-unified-object-editing/` criterion 28 selects the new shape and
  switches to the Select tool after a create-drag.)
- A drag threshold for shape creation: only A equal to B is refused.
- Numeric entry of size or position, snapping, a persistent "draw from
  center" option or toggle.
- Any document-model or file-format change, new crate, or ADR.

## UX notes

Pre-filled by the product owner for the ux-engineer to review; nothing here
is final.

- Cursor: unchanged for all four modifier states.
- A Shift press on an existing shape's outline in the rectangle tool starts a
  create-drag around the press point, like any other press (criterion 16 as
  superseded by `unified-object-editing` criterion 25; covered by test
  `ac16`).
- Modifier legend: the shape tools show none today (only the numeric readout
  near the pointer, per `primitive-shapes` UX notes), so none is added. The
  preview outline and the readout are the feedback, as Ctrl is today
  (`object-transform` UX notes: no per-modifier chrome where the geometry
  already shows the effect). `ux-engineer` to confirm; if a hint is wanted,
  the existing marquee-style legend (`docs/design-system.md`) is the
  precedent, with text such as "Shift: from center  Ctrl: 1:1".
- Preview: no new element. In centered mode the outline simply grows around
  the press point; no extra center mark (Proposal; `ux-engineer` may add one
  if the center is hard to read at small sizes).
- Readout: unchanged format and position (criterion 11).
- Discoverability: Inkscape and LightBurn users expect the modifiers; the tool
  tooltips for Rectangle (R) and Ellipse (E) may mention them, left to
  `ux-engineer`.

**UX review decisions (ux-engineer, 2026-10-07, checked in the running build):**

- No legend, no hint chip and no readout wording during the drag; the outline
  and readout change in the frame of the key and read clearly in all four modes.
- Centre mark (supersedes "no extra center mark"): while Shift is down in a
  rectangle or ellipse create-drag, the existing pivot marker is drawn at the
  press point (`docs/design-system.md`, "Modifiers in a rectangle or ellipse
  create-drag"). Same meaning as in a transform: Shift = about this point.
- Tooltip: the Rectangle and Ellipse rail tooltips get a second line,
  "Shift: from centre. Ctrl: square or circle".

## Sequencing

Touches the shape tools in `curvyo-ui-core` (`rectangle_tool.rs`,
`ellipse_tool.rs`, `shape_tool_common.rs`) and `Session`'s pointer paths in
`curvyo-editor-wasm` (`session/mod.rs`, `session/shapes.rs`) plus the
`wasm_api` and host key handling. `specs/0008-object-transform-refinements/` edits
the same pointer paths, so this is built after it.

## Open questions for the customer

Customer remark about polygon/star read as: pointer pulls a vertex, keep as
is (to be confirmed); default: unchanged either way.

## Links
Requirements: R-EDIT-002 (`docs/requirements.md`); extends
`specs/0003-primitive-shapes/` criteria 2 and 8.
PR: https://github.com/curvyo/curvyo/pull/47
