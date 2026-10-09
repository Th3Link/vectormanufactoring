# Style panel rework: empty when idle, hidden instead of disabled, RGBA hex, inline colour, eyedropper, custom dash, value fields, no gradients

Status: Ready
Priority: Must
Origin: Customer

Ready (2026-10-08): the criteria below are complete and testable, the
architect's `adrs.md` exists and the ux-engineer's "UX notes" are filled in
(`CLAUDE.md` §4). The requests of both (UX notes section 11, `adrs.md` "Flagged
to the lead") are applied to the criteria.

## User value

As a maker I want the Style panel to show only what I can change right now,
let me type or drag every number, pick a colour from a drawing with one click,
and type a dash pattern, so that styling an object takes fewer clicks and the
panel does not distract me with controls that do nothing.

The customer tested `0007-stroke-and-fill-styling` (all four PRs) on the desktop
app and found no bug. These are decisions about how the panel should work,
made by the customer and final. What changes:

- Nothing selected: the panel is empty. Stroke or fill paint None: everything
  below the Paint switch is gone, not greyed out.
- The colour field shows the colour as 8 hex digits, `#RRGGBBAA`.
- The colour is chosen inline, with no popup. An eyedropper next to the colour
  picks a colour from the drawing.
- Dash keeps its presets and gains a text line for a custom pattern.
- Number fields become GIMP-style value fields: drag to change, click to type,
  no text selection by dragging, no +/- buttons. Width and opacity drag on a
  slightly logarithmic scale.
- No popups anywhere in the properties panel.
- Gradients are removed completely (YAGNI: no application of ours needs them).

**Reference tools.** Inkscape's Fill & Stroke shows every control all the time,
puts dash and marker choices into dropdown lists, and hides its colour picker
behind a tab. Its RGBA field is 8 hex digits, the same as the customer asked
for. Its dash field is a free text line next to the presets, which is what
criterion 29 copies. LightBurn has no per-object style panel; colour belongs to
a layer. GIMP's value fields (label left, value right, bar filled to the value)
are the model for criteria 34 to 48. What we do better than Inkscape: the panel
contains only controls that apply, nothing opens over the canvas, and every
number can be dragged with a fine and a coarse modifier.

## Supersedes (`specs/0007-stroke-and-fill-styling/`)

`0007` stays as the record of what was built. Its body is not rewritten; each
range carries a "Superseded by" note pointing here. Where this spec and `0007`
disagree, this spec wins.

| `0007` criteria | What happens |
|---|---|
| 5 | Changed. Paint switch kept. Dash, Join and Cap are hidden, not disabled, while the stroke is off, and so are Color and Width. Criteria 5 to 10 and 16 here replace it |
| 6 | Changed. Hex is 8 digits (RGBA), 8 digits are no longer refused. Integer-percent opacity rule kept. Criteria 11 to 16 here |
| 8, 9 | Changed. "Custom" read-only entry and "this slice does not require a UI for a custom array" are gone. Criteria 28 to 33 here. The stored format (criterion 9) is unchanged |
| 13, 14 | Changed. Fill has None and Solid only; the gradient clauses of 13 are void |
| 16 to 22 | Removed. Gradient stop model, defaults, add and remove stop, linear and radial rendering |
| 23 | Changed. "Solid, linear or radial" becomes "solid"; the "at least one stop" clause is void |
| 25, 30 to 33 | Changed. Wherever they name gradient stops they now name nothing; the rest holds |
| 34, 35 | Removed. Gradients across a multi-selection and degenerate stop lists |
| 36 | Changed. Popover sentences void. Typed-value and "no stepping with arrow keys" parts replaced by the value field (criteria 34 to 48). Preview, one commit on release, Escape revert and "commit goes to the objects the edit started on" are kept |
| 37 | Changed. The "disabled state" is gone; the panel is empty (criteria 1, 2). The Node tool scope and the subject line (except "Nothing selected" and the Pen text) are kept |
| 38 | Changed. Escape order (criterion 60). Gradient-thumb sentence void. The rest (panel keys never reach the canvas, `Shift+Ctrl+F`, focus return) is kept |
| 1 to 4, 7, 11, 12, 15, 24, 26 to 29, 39 to 41 | Unchanged |
| UX notes 2 to 5 and 7 to 9 | Superseded where they describe popovers, the disabled state, gradients, and the Select dropdown. The ux-engineer rewrites them in this spec's "UX notes" and in `docs/design-system.md` |
| "Out of scope" of `0007` | Custom numeric dash entry and eyedropper are now in scope (here). Recent colours, swatch library, CMYK and HSL stay out, until `color-management` |

## Panel rule (customer, final, 2026-10-08)

**No popups in the properties panel.** No control in the panel opens a
popover, dropdown list, menu, dialog or any other layer over the canvas or over
other panel content. Everything the maker can set is a control in the panel's
own column. Text-only tooltips and the field validation message are the only
transient layers; they contain no controls and never take focus. The
ux-engineer records this rule in `docs/design-system.md` (criterion 58). It
holds for every later section of the panel, not only Style.

## Acceptance criteria

### Panel content follows the selection

1. Given nothing is selected, or the Pen tool is active, or the Node tool is
   active and neither a selected node nor an object selection leads to a path
   (the scope rules of `0007` criterion 37 otherwise unchanged), when the panel
   is open, then its body shows no heading, no subject line, no text and no
   control. Only the panel frame (280 px wide) and the collapse tab remain.
   **Amended 2026-10-09 (`document-size-and-rulers` criterion 14a, text only):**
   the Style area is empty in these cases; when nothing is selected and the
   Pen has no unfinished path, the panel body shows the Document section of
   `0015-document-size-and-rulers` instead of nothing. In every other case
   above the body stays empty as written. If that spec's Question 1 is
   answered B or C, this amendment is dropped. Test: the panel element holds no focusable element except the collapse tab
   and no visible text. Given a control that has keyboard focus leaves the tree
   (rows are hidden by criterion 5, 6 or 8, or the selection becomes empty), then
   focus moves to the Paint group of that control's section (Stroke or Fill; the
   marker rows of `stroke-markers` belong to Stroke), or to the canvas when the
   panel became empty. Test: focus the Width field, press Home, release; the
   active element is the Stroke Paint group. After a mouse press, focus goes to
   the canvas (criterion 57).
2. Given the panel switches between empty and filled because the selection or
   tool changed, then the canvas does not move or resize and the panel width
   stays 280 px (`0007` criterion 39).
3. Given any selection, tool or stored value, then no control in the panel is
   rendered disabled (`disabled`, `aria-disabled`, greyed). A control that
   cannot apply is not rendered.
4. Given a selection, then the panel shows the subject line of `0007`
   criterion 37 ("Rectangle", "3 rectangles", "4 objects", "2 paths") and the
   Stroke and Fill sections. The texts "Nothing selected" and "Pen: finish the
   path to style it" no longer exist.

### Paint None hides the rest

5. Given the stroke Paint is None for every selected object, then the Stroke
   section is one visual row: its heading ("Stroke") and the Paint switch sit
   together on that row. Color, the colour picker, Opacity, Width, Dash, Join, Cap
   and any marker rows (`stroke-markers`) are not in the document tree, not
   focusable and not in the accessibility tree. When these controls disappear or
   reappear, the heading and the Paint switch do not move (same position in CSS
   pixels before and after), so a mouse press on the switch stays under the
   pointer.
6. Given the fill Paint is None for every selected object, then the Fill section
   is one visual row: its heading ("Fill") and the Paint switch, which do not
   move when the controls below disappear or reappear (criterion 5). Fill has the
   Paint switch with two states, None and Solid; there is no Linear and no
   Radial.
7. Given a stroke with width 2 mm, colour `#FF000080`, dash Dot and join Round,
   when the maker sets Paint to None and then back to Solid, then every row
   returns with exactly those values and the stroke renders as before.
8. Given a stroke whose width is set to 0, by typing `0` or by dragging to the
   left end of the field (criterion 47), when the edit is committed, then the
   same commit sets Paint to None and keeps the last non-zero width stored. The
   Width row disappears (criterion 5). Pressing Solid then restores the stroke at
   that last non-zero width, or at 0.25 mm if the width was never above 0 (for
   example from a file). While a drag is still running the row stays, and the
   preview shows no stroke at 0.
9. Given some selected objects have the stroke on and some off, then Paint shows
   no state pressed and the rows below are shown with the stored values (a
   field whose values differ shows "Mixed"). An edit of Color (hex, picker or
   eyedropper), Opacity or Width (typed, dragged, stepped or reset, criterion 61)
   applies to all selected objects and turns the off ones on in the same commit.
   An edit of Dash, Join or Cap sets the stored value of all selected objects and
   changes no object's on or off state. This turning-on is a **stroke** rule only:
   a fill colour or fill opacity edit never turns a fill on (as in `0007`); the
   fill is turned on only by its Paint switch (criterion 15).
10. Given the maker presses None or Solid in the Paint row, then every selected
    object's stroke (or fill) is set to it in one commit.

### Colour: 8-digit hex

11. Given a selected object with a solid stroke or fill, then the hex field
    shows the colour as `#RRGGBBAA`, upper case, always 8 digits. AA is the
    alpha as `round(255 x alpha)`, with .5 rounded up. Example: `#2F6FEE` at 100%
    shows `#2F6FEEFF`; at 50% it shows `#2F6FEE80`.
12. Given the hex field is edited, then these forms are accepted, with or without
    `#`, in any case, with surrounding spaces ignored: 3 digits (`F80`), 4 digits
    (`F80C`), 6 digits (`FF8800`), 8 digits (`FF8800CC`). 3 and 6 digits set the
    RGB and leave the alpha as it is. 4 and 8 digits set RGB and alpha. A 3 or 4
    digit form doubles each digit (`F80C` is `FF8800CC`). Any other length or a
    non-hex character is refused: focus stays, the text is selected, the message
    "Enter 3, 4, 6 or 8 hex digits" shows in the error chip (a text-only chip
    below the field, at most 244 px wide, `role="alert"`), nothing is written.
13. Given an alpha set through the hex field, then it is stored as `AA / 255`
    and is not rounded to a whole percent. Given the Opacity field, then a typed
    or dragged N is stored as exactly `N / 100` (`0007` criterion 6). Example:
    typing `2F6FEE80` gives Opacity 50 (shown rounded) with stored alpha
    128/255; typing 50 in Opacity afterwards stores 0.5 and the hex still reads
    `#2F6FEE80`. A stored alpha off both grids (from a file) is shown rounded in
    both fields and is not rewritten by looking at it or by Enter on an
    unedited field.
14. Given several selected objects, then the hex field shows "Mixed" when any
    RGBA value differs from another. Typing 3 or 6 digits then sets the RGB of
    every object and each keeps its own alpha; typing 4 or 8 digits sets
    everything. One commit.
15. Given the Fill section, then it has the same colour controls, with the same
    rules, as the Stroke section, with one difference: a fill colour edit (hex,
    picker, eyedropper) or a Fill Opacity edit never turns the fill on. It sets
    the stored colour or opacity of all selected objects and changes no object's
    on or off state (as in `0007`); only the Fill Paint switch turns a fill on or
    off (criteria 6, 10). With the fill Paint None for every selected object the
    colour controls are not shown (criterion 6), so this matters for a mixed
    selection, where objects whose fill is off keep it off.
16. Given the colour swatch next to the hex field, then it shows the colour at its
    alpha over a checkerboard (a diagonal hatch for a mixed colour, as in
    `0007`). It is a display only: no click action, not a tab stop.

### Colour: inline picker, no popup

17. Given a Solid paint, then a saturation/value area and a hue slider are
    visible in the panel under the Color row, with no click needed to reveal them
    and nothing opening over other content. They are `role="slider"` controls
    named "Saturation and value" and "Hue". There is no alpha slider in them; the
    Opacity field owns alpha.
18. Given a drag in the area or the hue slider, then the selected objects render
    in the new RGB on every pointer move, one update per animation frame, and
    one commit follows on release even if the pointer is outside the control.
    Dragging away and back makes no commit in between. Escape during the drag
    reverts and the release writes nothing. The commit changes RGB only; each
    object's alpha stays.
19. Given the arrow keys on the area or the hue slider, then each step is 1% of
    the range, Shift makes it 10%, the object previews on key-down and commits
    on key-up, so a held key is one commit.
20. Given a drag through a grey or black, then the hue thumb does not jump: it
    stays where it was. Test: drag the area to its left edge and back; the hue
    slider has not moved.
21. Given several objects with different colours, then the area and the hue
    slider show no thumb. The first interaction sets the RGB of all selected
    objects to the chosen colour, each keeping its alpha.

### Colour: eyedropper

22. Given the Color row of Stroke or Fill, then an eyedropper button (28 px, icon,
    name "Pick stroke color from the drawing" / "Pick fill color from the
    drawing", tooltip) sits between the swatch and the hex field. Pressing it
    starts picking for that colour. The colour swatch is display only and is not a
    tab stop: the Tab order in the Color row is the eyedropper button, then the
    hex field (criterion 16).
23. Given picking is active, then the cursor is an eyedropper, a press on the
    canvas selects nothing, moves nothing and draws nothing, and the colour a
    click would pick is shown while the pointer hovers (position and form decided
    by the ux-engineer).
24. Given picking is active, when the maker clicks a point, then the colour of
    the topmost object whose painted stroke or painted fill is under the point is
    taken, complete with its alpha, and written as the colour of the target
    (stroke or fill) of all selected objects in one commit. Picking then ends. As
    with every fill colour edit, a pick for the fill does not turn a fill on
    (criterion 15); a pick for the stroke turns an off stroke on (criterion 9).
    "Painted stroke under the point" means the point is within the stroke as the
    Select tool hit-tests it (outline tolerance included); on an object with both,
    a point on the stroke picks the stroke colour and any other point inside
    picks the fill colour. An object with the paint None is not pickable by that
    paint. A paint with opacity 0 counts as painted (`0007` criterion 23). The
    selected objects themselves can be picked. Editor overlays (selection box,
    handles, guides) are never picked.
25. Given picking is active, when the maker clicks a point where no object is
    painted, then nothing is written and picking stays active.
26. Given picking is active, when the maker presses Escape, changes the tool,
    presses a rail tool, or presses anywhere in the panel other than the
    eyedropper button, then picking ends and nothing is written. Pressing the
    eyedropper button again also ends it. A right-click on the canvas also ends
    picking, writes nothing and opens no context menu. Pan and zoom (including the
    wheel) keep working while picking and do not end it.
27. Given the eyedropper picks from the drawing only, then colours outside the
    app window are not available. The hex field is the way to enter any other
    colour. Picking has no keyboard path in this slice.

### Dash: presets and custom pattern

28. Given a stroke is on, then the Dash row shows four preset buttons in one
    group, Solid, Dash, Dot and Dash-Dot, each with its line sample (the samples
    and ratios of `0007` UX notes). The group replaces the dropdown. The stored
    lists are those of `0007` criterion 8.
29. Given the Dash row, then below the preset buttons a single text line shows
    the stored pattern as numbers separated by one space: Solid shows empty text
    with the placeholder "Solid. Example: 6 4", Dash `6 4`, Dot `1 3`, Dash-Dot
    `6 3 1 3`. Its label says the numbers are multiples of the stroke width, on
    then off. The group and the line show the same stored value at all times.
30. Given the text line is edited and committed with Enter or Tab, then the text
    is read as follows: numbers separated by spaces only (one or more; `1 2 4 2`
    and `1   2 4  2` are the same), 1 to 16 numbers, each from 0 to 1000 with a
    decimal point as the decimal mark, the sum above 0. A comma is not a
    separator and not a decimal mark: `1,2,4,2`, `1, 2` and `1,5` are refused. A
    valid text is stored as the list in the order typed; the stroke renders with
    it. An empty text stores the empty list, which is Solid. Any other text is
    refused with focus kept, the text selected and a message in the error chip (at
    most 244 px wide; "Enter 1 to 16 numbers from 0 to 1000, for example 1 2 4
    2"), and nothing is written.
31. Given the stored list equals a preset exactly, then that preset button is
    pressed. A stored list that equals none (typed, or from a file) leaves no
    button pressed and shows its numbers in the text line. There is no
    "Custom" entry.
32. Given a custom list, then it renders as `0007` criteria 8 and 9 define for
    presets: lengths are multiples of the current stroke width and rescale when
    the width changes; an odd count repeats the list (`1 2 4` draws as
    `1 2 4 1 2 4`), as SVG defines; a period under 2 screen pixels draws solid on
    screen without changing the stored list. A zero in an "on" position is
    accepted (with round caps it draws a dot), which the presets do not use.
33. Given several selected objects with different dash lists, then no preset is
    pressed and the text line shows "Mixed"; a typed valid text or a preset press
    sets all, in one commit. A stored list with more than 16 numbers (from a file)
    is shown in full and is not rewritten until the maker edits the line.

### Value fields

These are the width and opacity fields. `stroke-markers` uses the same field
for its count. The hex field and the dash text line are ordinary text inputs.
The reset slot and the reset action are criterion 61.

34. Given the Stroke Opacity, Stroke Width and Fill Opacity rows, then each is a
    value field: one row as wide as the control column (the panel content
    width, 244 px), 28 px high, the label inside it at the left, the value and its
    fixed unit (`mm`, `%`) at the right, and a bar behind them filled from the left
    edge to the share of the field width that the value has on the field's scale
    (criterion 46). At the right end, inside the border, a 24 px reset slot is
    always reserved (criterion 61), so the value does not move when the reset icon
    appears. There is no +/- button, no stepper arrow and no slider thumb.
35. Given the field is not being typed in, then it is not a text input: pressing
    and dragging over it, or over any label or value in the panel, selects no
    text and drags no text. Test: after a drag across the panel,
    `window.getSelection()` is empty. The pointer over the field is a horizontal
    resize cursor.
36. Given a press in a value field followed by pointer movement, then once the
    pointer has moved 3 px from the press point a drag starts. It changes the
    value relative to the value at the press (the value does not jump to the
    pointer). The field's scale position is `p` (0 at the left end, 1 at the right
    end), `p0` is its value at the press and `W` is the field's rendered width in
    CSS pixels. At a pointer `dx` px from the press point (to the right is
    positive; `dx` is measured from the press point, not from where the 3 px
    threshold was crossed), `p = clamp(p0 + dx / W, 0, 1)`. Example, Width, W =
    244: from 0.25 mm, moving 24.4 px right gives 0.51 mm (+/- 0.01). Example,
    Opacity: from 100, moving 24.4 px left gives 83.
37. Given a drag is running, then the pointer is captured: moving outside the
    field, outside the panel or outside the window keeps changing the value by the
    rule of criterion 36. The value is clamped at the ends of the scale and
    there is **no re-basing at the ends**: after the pointer has gone past an end,
    the value stays at the end until the pointer is back at the position where
    `p0 + dx / W` is again inside [0, 1]. Test: from Width 0.25 mm (W = 244),
    move 300 px right, then 100 px back; the value is the one at `dx` = 200 px
    (`p` = 1, 20 mm), not the one at 100 px back from the end. Releasing anywhere
    ends the drag.
38. Given Shift is held during a drag, then `p` changes ten times as fast
    (coarse: `p = clamp(p_b + 10 x (x - x_b) / W)`); given Ctrl (Cmd on macOS) is
    held, then ten times slower (fine: factor 0.1), where `p_b` and `x_b` start as
    `p0` and the press position. If a modifier is pressed or released mid-drag,
    the drag re-bases at that moment: `p_b` becomes the current (clamped) `p` and
    `x_b` the current pointer position, so the value does not jump. Re-basing
    happens only at a modifier change, never at the ends of the scale. The
    modifiers also change the rounding grid of the width (criterion 47).
39. Given a drag, then the selected objects render in the new value on every
    pointer move (one update per animation frame, ephemeral, not in the
    document) and exactly one commit follows on release, also when the release
    is outside the field. Dragging away and back makes no commit in between. The
    commit goes to the objects the drag started on. A drag that ends at the value
    it started with commits nothing.
40. Given Escape is pressed during a drag, or the pointer is cancelled by the
    system, then the preview is dropped, the object shows its committed value and
    the release writes nothing.
41. Given a press and release in a value field with less than 3 px of movement,
    then the field enters typing: the number is shown without its unit (the unit
    stays as a fixed suffix), all of it selected, and a caret. This needs one
    click; double-click is not required. In typing, the mouse selects text like any
    input.
42. Given typing, then: Enter commits and returns focus to the canvas; Tab commits
    and moves to the next field; Escape restores the shown value, writes nothing
    and returns focus to the canvas; a press elsewhere restores and writes
    nothing; Enter on text the maker did not edit writes nothing. A decimal point
    or comma is accepted; a trailing unit (`mm`, `%`) and surrounding spaces are
    ignored. Input that is not a number, or outside the field's typed range, is
    refused: focus stays, the text is selected, a message shows in the error chip
    (at most 244 px wide; "Enter a number from 0 to 1000" for Width, "Enter a
    number from 0 to 100" for Opacity),
    nothing is written. Typed Opacity with decimals is rounded to the nearest
    integer (`0007` criterion 6). There is no preview while typing.
43. Given the field has keyboard focus and is not being typed in, then it is one
    Tab stop with `role="spinbutton"`, a name that contains its label,
    `aria-valuemin` (the scale minimum), `aria-valuemax` (the **typed** maximum:
    1000 for Width, 100 for Opacity), `aria-valuenow`, an `aria-valuetext` with
    the unit and `aria-keyshortcuts` naming Ctrl+Backspace. ArrowRight
    and ArrowUp raise the value by one step, ArrowLeft and ArrowDown lower it;
    Shift makes the step ten times larger, Ctrl (Cmd on macOS) ten times smaller
    (never below the rounding grid). Steps are the grid of criterion 47 (Width:
    0.01 mm, Shift 0.1 mm, Ctrl 0.001 mm; Opacity: 1 %, Shift 10 %, Ctrl 1 %).
    Home goes to the minimum and End to the **end of the scale** (Width 20 mm,
    Opacity 100 %), which for Width is below `aria-valuemax`. The object previews
    on key-down and one commit follows on key-up, so a held key is one commit.
    Enter, F2 or typing a digit, a decimal separator or a minus sign starts typing
    (the digit replaces the text). Ctrl+Backspace (Cmd+Backspace on macOS) resets
    the value (criterion 61). Backspace and Delete never reach the canvas and
    change nothing.
44. Given several selected objects with different values, then the field shows the
    word "Mixed" in place of the value and an empty bar. A drag then sets all
    selected objects to the value under the pointer on the field's scale (the
    pointer's position in the field is `p`, absolute), a typed value sets all,
    the arrow keys do nothing, Home and End set all to the scale minimum or the
    end of the scale, Ctrl+Backspace and the reset icon set all to the default
    (criterion 61). Each is one commit.
45. Given the field's value is above the drag maximum (typed, or from a file), then
    the bar is full and the value reads as stored. A drag starts at `p` = 1; the
    first movement past the 3 px threshold sets the value that `p` gives, so the
    value drops into the scale.

### Value field scales

46. Given the scales, then the field maps `p` in [0, 1] to a value `v` as follows
    and its inverse draws the bar. Both curves are slightly logarithmic: fine near
    zero, coarser towards the top. The base values are a proposal for the
    ux-engineer to tune by hand; a change needs the test values below updated.
    - **Width (mm):** `v = 20 x (100^p - 1) / 99`. Drag range 0 to 20 mm. Typed
      range 0 to 1000 mm. Check values: p = 0 gives 0; p = 0.1749 gives 0.25;
      p = 0.5 gives 1.82; p = 1 gives 20.
    - **Opacity (%):** `v = 100 x (4^p - 1) / 3`. Range 0 to 100 for both drag and
      typing. Check values: p = 0 gives 0; p = 0.5 gives 33; p = 0.9 gives 83;
      p = 1 gives 100.
47. Given a value from a drag or the arrow keys, then it is rounded to a grid:
    Opacity to whole percent; Width to 0.01 mm, to 0.1 mm while Shift is held
    and to 0.001 mm while Ctrl is held. A value that rounds to 0 is 0, so the
    left end of the scale is reachable exactly: a drag to the field's left end
    gives Width 0 and Opacity 0. Width 0 turns the stroke off at commit
    (criterion 8); Opacity 0 stays a paint (`0007` criterion 23). The field shows
    Width with up to 3 decimals and no trailing zeros ("0.25", "1.82", "0.125")
    and Opacity as an integer.
48. Given the unit, then Width is shown in mm whatever the document unit.
    (Document units other than mm do not exist yet; `document-size-and-rulers`
    owns that decision.)

### Gradients removed

49. Given any file, tool or panel state, then there is no gradient anywhere in the
    product: the Fill Paint switch has None and Solid only (criterion 6), and no
    gradient bar, thumb, stop row, Add stop button, gradient message or
    polygon/star gradient line exists in the panel.
50. Given the document model, then it has no gradient fill kind, no stop, no stop
    identifier, no stop list, no gradient frame and no register for them. Writing
    a document never produces a gradient key. A fill is None or Solid.
51. Given the renderer and hit-testing, then no code draws, tessellates, textures
    or hit-tests a gradient. A fill "paints" exactly when it is Solid (opacity 0
    included). Rendering of all solid fills is unchanged: the existing golden
    images of solid fills still pass without being regenerated.
52. Given tests, fixtures, docs and specs, then every gradient test, fixture and
    golden file is deleted; `docs/design-system.md` and `docs/technical-debt.md`
    lose their gradient sections and entries; `docs/requirements.md` R-EDIT-006
    reads "solid color" only (done in this PR); `specs/README.md` slice 7 no longer
    promises gradients (done in this PR); `0007` carries the "Superseded by"
    notes (done in this PR). Deleted means deleted: no commented-out code, no
    feature flag, no `cfg` switch, no `#[allow(dead_code)]` left behind. Check:
    a case-insensitive search for `gradient` in the Rust crates, `frontend/src`
    and every `tests` directory finds only the legacy-read path and its one fixture of criterion 53,
    if the architect keeps one.
53. Given a project file that contains gradient fill data (written by a build of
    `0007` PR 4), then it opens without an error, a prompt or a message; every
    object keeps its geometry, stroke and other style; an object with a gradient
    fill is shown with no fill (nothing drawn, interior not clickable, Fill
    Paint None in the panel); the file is not changed by opening it. **Default,
    for the architect to confirm or change:** the gradient data stays in the file
    untouched, ignored by the app, until the maker edits that object's fill, at
    which point the fill is replaced by the new one (Solid with the object's
    stored solid colour, black if none). Saving without such an edit leaves the
    gradient data in the file. A fixture of such a file is the one gradient file
    the repository keeps.
54. Given the format, then the gradient removal needs no format version bump (a
    gradient fill kind read as None is a read-side rule). Storing odd-length dash
    lists (criteria 30 to 33) does need one: an older reader would refuse them as
    damaged. The format version goes to `CURRENT_FORMAT_VERSION + 1` at merge in
    the PR that first writes an odd list (the next free number; `adrs.md`
    decision 2; `boolean-operations` takes 8 first, see the `format_version` plan in
    `specs/README.md`). A file of any earlier version opens unchanged.
    `stroke-markers` takes the next free number after that.

### No popups, interaction rules

55. Given every control of the panel (all rows of both sections, the eyedropper
    button, the collapse tab), when it is operated by mouse and by keyboard, then
    no element with role `dialog`, `listbox` or `menu` and no popover portal
    appears, and nothing is drawn over the canvas or over other panel content
    except: text-only tooltips; validation messages (criteria 12, 30, 42); the
    eyedropper's readout chip (criterion 23); and other canvas readouts that hold
    no control (such as the existing transform readout). None of these contains a
    control, takes focus or takes pointer events.
56. Given the popup components that only the old colour popover and dash dropdown
    used (`ui/popover.tsx`, `ui/select.tsx`, the colour popover wrapper), then
    they are deleted if nothing else uses them, in this PR.
57. Given the rules of `0007` criteria 36 and 38 that are not named in the table
    above (preview coalesced per frame, one commit on release, the commit goes to
    the objects the edit started on, discrete controls commit on click, one commit
    for the whole multi-selection, Backspace, Delete and letters in any panel
    control never reach the canvas, `Shift+Ctrl+F`, focus returns to the canvas
    after a mouse press on a button-like control), then they hold for every control
    of this spec. `Shift+Ctrl+F` moves focus to the first control (Stroke Paint) or,
    when the panel is empty, to the panel itself.
58. Given `docs/design-system.md`, then it states the "no popups in the properties
    panel" rule of this spec word for word, the value field (labelled bar, drag,
    modifiers, scales) as a component, and no longer describes the colour popover,
    the dash dropdown or gradients. (The ux-engineer does this before Ready.)
59. Given a press on the canvas, a tool change or a selection change while a value
    field drag, a picker drag or a text edit is running, then the edit that
    started on the old objects still commits to them (`0007` criterion 36), or is
    restored when it was text.
60. Given Escape is pressed with the focus in the panel or while the app is in a
    panel mode, then it acts on the first of these that applies: ends eyedropper
    picking; reverts a running drag; restores an edited field and returns focus to
    the canvas; otherwise returns focus to the canvas. It never clears the
    selection.

### Value field reset

(Numbered 61 so that the numbers 1 to 60, which `adrs.md` and the UX notes
cite, stay stable. It belongs with criteria 34 to 48.)

61. Given a value field (Stroke Opacity, Stroke Width, Fill Opacity; the Count
    field of `stroke-markers`), then a reset slot 24 px wide is reserved at its
    right end, inside the border (criterion 34). The reset icon in it is shown
    only when the value differs from the field's default or is Mixed, and is
    hidden (the slot stays empty, the value does not move) when the value equals
    the default. Defaults: Width 0.25 mm, Opacity 100 % (stroke and fill), Count
    1. A press on the icon, or Ctrl+Backspace (Cmd+Backspace on macOS) on the
    focused field when it is not being typed in, sets every selected object to the
    default in one commit (Mixed included), and writes nothing when the value
    already is the default. The icon is a button with the name "Reset stroke width
    to 0.25 mm" (and the equivalent for the other fields) and a tooltip that
    names the key; it is not a tab stop (`tabindex="-1"`) and sits after the field
    in the accessibility tree, not inside it. Backspace or Delete alone never
    reset and never reach the canvas. A reset of Stroke Width or Opacity is an
    edit in the sense of criterion 9. After a mouse press on the icon focus goes
    to the canvas (criterion 57).

## Out of scope

- Colour presets, palettes, thread palettes, colour history, CMYK, HSL, HSV,
  ICC, import and export of palettes, an "old and current" swatch pair:
  `color-management` (Draft, after the MVP).
- Markers (arrowheads and dots on a stroke): `stroke-markers`.
- Gradients in any form, ever, until the customer asks for them; then as a new
  design, not as a restoration.
- Picking colours from outside the app window, picking from a placed image (no
  image object exists), a keyboard path for the eyedropper.
- Value fields in the tool bars (Radius, Points, Ratio, entry chips). The panel
  gets the new field; the bars keep theirs. See the open question.
- Mouse-wheel stepping of a field (the wheel scrolls the panel).
- A visual dash editor; editing the miter limit; the fill rule (all as in `0007`).
- Undo and redo (`undo-redo`); every edit is still one commit.
- Document units other than mm.
- SVG export and import of any of this (`svg-import-export`).

## Open questions (customer; each has a default, nothing blocks)

1. **Files with gradient data (criterion 53).** *A (default):* opens with no
   fill for those objects, the data stays in the file until the fill is edited.
   *B:* on open, each such fill becomes Solid in the first stop's colour (the
   shape stays visible; the data is dropped when saved). *C:* refuse to open with a
   message. Recommendation: A, as you described it. Only test files can contain
   gradients, so the choice has little effect.
2. **Eyedropper source (criteria 24, 27).** *A (default):* the colour of the
   topmost object's painted stroke or fill under the pointer, exactly as stored,
   with its alpha. *B:* the colour of the rendered pixel, which also mixes
   overlapping transparent objects and shows antialiased edges. Recommendation:
   A. It is exact at any zoom and does not depend on antialiasing; B is the
   choice once placed images exist.
3. **Empty panel frame (criterion 1).** *A (default):* the empty 280 px frame and
   its tab stay, so the canvas never changes size on selecting. *B:* the panel
   closes by itself when nothing is selected and opens again on a selection (the
   canvas grows and shrinks). Recommendation: A.
4. **Modifier keys on the value field (criterion 38).** Default: Shift = ten
   times coarser, Ctrl (Cmd on macOS) = ten times finer, the same for keys and
   mouse, consistent with the Shift = 10 times rule that arrow keys already had.
   GIMP uses the opposite assignment for some of its sliders. Say if you prefer
   Shift = fine.
5. **Value fields in the bars.** Default: only the panel changes now. The bars'
   number fields (Radius, Points, Ratio, entry chips) keep their current look.
   Option: the same field there in a follow-up change, so the app has one kind of
   number field. Recommendation: follow up once you have tried the panel.
6. **Inline picker size.** *Decided by the lead, 2026-10-08:* the
   saturation/value area (96 px high) and the hue slider are always visible under
   each Solid colour, with no disclosure arrow; the panel scrolls on a short
   window. The option of one disclosure arrow per colour (still inline, no popup)
   is not built; the customer may ask for it after trying the panel.

Further design decisions by the lead, 2026-10-08 (the UX notes' questions
U1 to U4, section 12): section order Stroke above Fill as built; the reset icon
shows only when the value differs from the default (criterion 61); the
current/old swatch pair stays in `color-management`.

## UX notes

Decided 2026-10-08 (ux-engineer), against the criteria above. Tokens, sizes and
component rules are in `docs/design-system.md`, "Properties panel: Style
section" and the "Value field" row group; this part holds the decisions and the
behaviour. It rewrites the `0007` UX notes that this spec supersedes (sections 2
to 5, 7 and 8 there). Kept from `0007`: docked 280 px panel, collapse tab,
`Shift+Ctrl+F`, mixed-state placeholders, preview per frame and one commit on
release, focus return to the canvas after a mouse press, the casing rule.

Fixed by the customer and not reopened here: criteria 1 to 6, 11, 22, the panel
rule, 28 to 30, 34 to 44 and the logarithmic drag. Everything below is the
ux-engineer's.

### 1. Layout of the Style section

Panel padding 12 px, content 244 px, rows 28 px high, 8 px between rows,
16 px between Stroke and Fill (a 1 px `--toolbar-icon` 25% line in the middle).
Text 14 px; section titles 12 px semibold. One scrolling column, one scrollbar.

**Header (one row, 24 px):** "Style" (14 px semibold) at the left, the subject
line (12 px, `--panel-muted-fg`: "Rectangle", "3 rectangles", "4 objects",
"2 paths") right-aligned on the same row. The old "Nothing selected" and Pen
texts are gone (criterion 4).

**The section title and the Paint switch share one row.** "Stroke" (12 px
semibold) sits in the 60 px label column, the None / Solid group (2 x 44 px) in
the control column. With Paint None this row is the whole section (criteria 5,
6): the title never moves, so a mouse press on the switch stays under the
pointer when the rows below go.

Stroke, top to bottom, with a path selected and Paint Solid:

| # | Row | Control | Height |
|---|---|---|---|
| 1 | Stroke | Paint group None / Solid | 28 |
| 2 | Color | swatch 28, eyedropper 28, hex field (rest, about 180), 4 px gaps | 28 |
| 3 | (picker) | saturation/value area 244 x 96, 8 px gap, hue slider 244 x 12 | 116 |
| 4 | Opacity | value field, full width | 28 |
| 5 | Width | value field, full width | 28 |
| 6 | Dash | four preset buttons, 4 x 44 px | 28 |
| 7 | Pattern | text line (176 px) | 28 |
| 8 | Join | `ToggleGroup` 3 x 40 | 28 |
| 9 | Cap | `ToggleGroup` 3 x 40 | 28 |
| 10 | Markers | `stroke-markers` (that spec's UX notes); under Cap | 132, or 204 with a Middle shape |

Fill: Fill (Paint None / Solid), Color, picker, Opacity; identical to rows 1 to
4 above.

Rows 2, 3, 4 and 5 have no label in the label column: Color and the picker are
unlabelled (the swatch, the eyedropper and the hex field say what they are
under the section title), and the value fields carry their label inside
(section 3). Rows 1, 6 to 9 use the label column.

**Heights (computed, 280 px panel).** Stroke without markers: 404 px; Fill
Solid: 224 px; header, paddings and section gap: 72 px; whole panel with both
Solid: 700 px; with a path and the marker rows (Middle None): 832 px; with
Middle set (Place, Count): 904 px. At 800 x 600 the panel viewport is about 570
px: the header and the whole Stroke section (to the Cap row, bottom at 448 px)
show without scrolling, the Fill section starts at 464 px and its picker is
cut off; the panel scrolls as a whole. On a 1080p display everything but a
marker stack with Middle set shows at once. The scroll position is kept when
rows come and go (the browser clamps it when the content gets shorter).

**Order stays Stroke, then Fill** (decided by the lead, 2026-10-08, U2). A change
would alter an accepted look. Stroke is the taller and more often edited section for the
cutters this product targets; the shorter Fill sits low and is the one that
may need a scroll at 600 px height.

**Pickers always visible (open question 6; decided by the lead, 2026-10-08,
U1).** Keep them always visible in this slice. The area is 244 x 96, not the 128 px the old popover
had: 96 px is still 1 px per 1% of value, and every colour is reachable by hex,
eyedropper or arrow keys. I measured the cost above (the Fill picker is cut off
at 600 px); if the customer finds the scrolling annoying after trying it, the
fix is one disclosure arrow per colour, still inline (U1).

### 2. Empty panel, hidden rows, no layout jumps

- **Empty (criterion 1).** The panel frame (280 px, `--panel-bg`, the left edge
  line) and the collapse tab are drawn; nothing else is in the tree. No text,
  no icon, no hint. Switching between empty and filled is instant (no fade, no
  slide) and changes no size (criterion 2). A screen reader meets the `aside`
  named "Properties" with nothing in it; no live region announces the change
  (that would be text in the panel).
- **Hidden means removed (criteria 3, 5, 6).** Rows leave the tree instantly.
  There is no height animation and no reserved space; the old "reserved
  heights" are gone. The rule that keeps this calm is anchoring: the header and
  every section title row are the first things in their block, so the control
  that is pressed to hide rows (Paint) never moves; only blocks below it do.
- **Focus when a focused control disappears.** Keyboard: `Home` on the Width
  field drags to 0, the stroke turns off at key-up and the field leaves the tree:
  focus moves to that section's Paint group (not to `body`, not to the canvas).
  If the whole panel became empty with focus inside it, focus goes to the
  canvas. After a mouse action it goes to the canvas anyway (criterion 57).
- **Width 0 (criterion 8).** While the drag runs, the field stays; at 0 the bar
  is empty and the value reads "0 mm" (no extra warning). On release the stroke
  turns off and the rows go; Solid brings it back at the last non-zero width.
- **Mixed Paint (criterion 9).** Paint shows nothing pressed; all rows show, with
  "Mixed" where the stored values differ. An off stroke among on ones brings its
  stored colour, width, dash and so on along; editing Color, Opacity or Width
  switches it on, editing Dash, Join or Cap does not (criterion 9). For the
  fill no edit switches it on, only its Paint switch (criterion 15). No
  indicator is needed.
- **Scope (`0007` criterion 37)** unchanged: Node tool edits the paths of the
  selected nodes or of the object selection; Pen and nothing selected give the
  empty panel; a creation tool with a selection edits the selection.

### 3. The value field (Width, Opacity, Fill opacity; Count in `stroke-markers`)

One component for all of them (`ValueField`). Anatomy, left to right inside a
244 x 28 px box (`rounded-[5px]`, white ground, 1 px border):

1. **Fill bar.** Behind the content, from the left edge to the position of the
   value, `--value-fill` (`--accent` at 28% over white, `#C5D7FA`). Its right edge
   carries a 2 px `--accent` line (4.5:1 on white) so the position reads without
   relying on the pale tint (1.45:1 to the ground). At value 0 nothing is drawn;
   at the right end the line sits inside the border. No thumb, no handle, no
   +/- buttons, no stepper arrows (customer).
2. **Label**, 8 px from the left, 14 px `--toolbar-icon`: "Width", "Opacity",
   "Count".
3. **Value and unit**, right-aligned: value 14 px tabular `--toolbar-icon`, unit
   ("mm", "%") 14 px `--panel-muted-fg` after it, 4 px gap. Width shows up to 3
   decimals without trailing zeros, Opacity and Count are integers.
4. **Reset slot**, 24 px at the right end, 4 px inside the border, always
   reserved so the value does not shift when the icon appears. The icon (Lucide
   `RotateCcw`, 12 px, 1.5 px stroke, `--toolbar-icon` at 80%) shows when the
   value differs from the default or is Mixed, and is hidden otherwise. A
   press resets in one commit and returns focus to the canvas. See "Reset".

**Mixed.** The word "Mixed" in `--field-placeholder` replaces value and unit; no
fill bar; the reset icon shows. A drag, a typed value, `Home`/`End` or a reset
sets all objects (criterion 44).

**States (all instant; no transitions):**

| State | Look |
|---|---|
| Rest | 1 px border `--toolbar-icon` 60% (3.1:1 on the panel), white ground, fill bar, cursor `ew-resize` |
| Hover | border `--toolbar-icon` 100% (8.3:1); the fill does not change (a darker tint would drop the unit below 4.5:1) |
| Focus (keyboard) | 2 px `--editor-accent` ring, 1 px `--toolbar-bg` offset, the panel's focus ring; not shown for mouse focus |
| Drag | as Rest with the focus ring; the pointer is captured; cursor `ew-resize` everywhere in the window until release; with Shift or Ctrl down the muted 12 px word "coarse" or "fine" appears centred in the field (`aria-hidden`) |
| Typing | the fill bar is hidden, ground stays white, the border becomes 2 px `--editor-accent`, the unit stays at the right, the text is right-aligned at the same place as at rest, caret, cursor `text` |
| Invalid | typing state with a 2 px `--field-invalid` border, `aria-invalid`, message chip below (section 7); cleared on the next keystroke |
| Disabled | does not exist (criterion 3) |

**Pointer mechanics.** The field is a `div`, not an `input`, outside the typing
state, so there is nothing to select or drag (criterion 35): `user-select:
none`, `-webkit-user-drag: none`, `touch-action: pan-y`. The whole panel is
`user-select: none` except its text inputs (hex, pattern, a value field being
typed in). Press, 3 px threshold, relative change, capture, clamping,
`Escape` revert: criteria 36 to 40. The value is `clamp(p0 + dx / W)` computed
from the press, with no re-basing at the ends (criteria 36 and 37), so after
pushing past an end the pointer has to come back to the end before the value
moves again. That is deterministic, matches "relative to the press", and is the
behaviour a tester can compute.

**Scales (criterion 46, checked).** I kept the proposed bases; they are not
"slightly" logarithmic in ratio terms (the slope at the top is 100 times the one
at the bottom for Width) but they are the range a maker needs:

| Width v | Bar position at W = 244 | mm per px there |
|---|---|---|
| 0.10 mm | 21 px | 0.0057 |
| 0.25 mm | 43 px | 0.0085 |
| 1 mm | 95 px | 0.023 |
| 5 mm | 172 px | 0.098 |
| 20 mm | 244 px | 0.38 |

A plain drag reaches every 0.01 mm step up to about 0.33 mm (one px or more per
step); above that Ctrl (fine) or typing is the way to an exact value. Opacity
moves 0.2 to 0.76 % per pixel, always under one grid step per pixel, so a plain
drag reaches every whole percent. The inverse that draws the bar is
`p = ln(1 + v (b - 1) / R) / ln b` with `R` = 20 (Width, b = 100) or `R` = 100
(Opacity, b = 4, written `p = ln(1 + 3 v / 100) / ln 4`). The check values of
criterion 46 hold unchanged. **No tick marks** on the bar: ticks on a
logarithmic bar would invite reading it as a ruler, and the number is the
readout. Revisit if testers cannot predict where 1 mm sits.

**Count (`stroke-markers`)** is the same component with a linear scale
`p = (v - 1) / 49`, integer grid, no unit, drag range 1 to 50, typed 1 to 500
(5 px per step at W = 244).

**Modifier legend** (kept in one place, shown in the tooltip, section 8): drag to
change; Shift coarse (10x); Ctrl (Cmd on macOS) fine (1/10); click to type.
The same keys, same meaning, for arrow keys.

**Reset.** The default is Width 0.25 mm, Opacity 100 %, Count 1. It is a pointer
and a key action: the icon, and `Ctrl+Backspace` (`Cmd+Backspace` on macOS) on
the focused field (`aria-keyshortcuts`). `Backspace` and `Delete` alone do
nothing in the spinbutton state (no undo exists yet, so a bare key must not
destroy a value). The reset icon is `tabindex="-1"` (not a second Tab stop per
field); its name is "Reset stroke width to 0.25 mm" and its tooltip repeats it
with the key. One commit; when Mixed, it sets all. It does nothing at the
default. This is the customer's GIMP reference (a small reset icon); the rules
are criterion 61 (formerly C6).

**Keyboard and semantics (criterion 43).** The spinbutton is the `div` itself:
`role="spinbutton"`, `tabindex="0"`, `aria-label` "Stroke width" (contains the
visible "Width"), `aria-valuemin` 0, `aria-valuemax` the typed maximum (1000,
100, 500), `aria-valuenow`, `aria-valuetext` "0.25 millimetres" / "50 percent" /
"3 markers"; Mixed: no `aria-valuenow`, `aria-valuetext` "Mixed". `End` goes to
the scale end (20 mm), which is below `aria-valuemax`; the tooltip says so.
Value changes by keys are announced by the spinbutton role itself; drags are
pointer-only and announce nothing. The reset icon is a `button` in the tree,
placed after the spinbutton in DOM order, not inside it.

### 4. Colour block

**Color row:** 28 px swatch, 4 px, 28 px eyedropper button, 4 px, hex field
(fills the rest, about 180 px, left-aligned, 14 px tabular, upper case).

- **Swatch:** 28 x 28, `rounded-[5px]`, 1 px `--swatch-border`, 1 px white inner
  line, colour at its alpha over the checkerboard, hatch for mixed. Display
  only: `aria-hidden`, no hover state, no cursor change (criterion 16). The
  "stroke off" slash of `0007` is gone with the disabled state.
- **Hex field:** a text input with the `0007` field rules (Enter commits and
  returns to the canvas, Tab commits and moves on, Escape restores, Enter on
  untouched text writes nothing). Content `#2F6FEEFF`. A typed 3, 4, 6 or 8
  digit form is read as criterion 12 says and shown back in the canonical form
  on commit. Mixed: placeholder "Mixed". Refused: "Enter 3, 4, 6 or 8 hex
  digits". Name "Stroke color hex (RRGGBBAA)".
- **Where alpha is edited.** The hex field's last two digits and the Opacity
  field edit the same alpha; each rounds in its own grid (criterion 13). No
  alpha slider in the picker: the Opacity row is the one place for the drag.

**Inline picker (criteria 17 to 21):** area 244 x 96 with `crosshair` cursor,
8 px gap, hue slider 244 x 12 with a rainbow ramp (`hsl` 0 to 360 at 100% / 50%,
in sRGB). Thumbs: area 14 px circle, hue 16 px circle; both a 2 px white ring
inside a 1 px `#000` 40% casing (`--picker-thumb-ring`, `--picker-thumb-casing`)
so they read on every colour. The picker keeps its own HSV state and re-derives
it from the stored colour only when the colour changes from outside it, so the
hue does not jump through a grey (criterion 20). Mixed: no thumbs; the area keeps drawing the
last hue (red before any interaction). Keys (criterion 19):
arrows 1 %, Shift 10 %, preview on key-down, commit on key-up. Names:
"Saturation and value" with `aria-valuetext` "Saturation 50 %, value 40 %" and
"Hue" with `aria-valuetext` "Hue 215 degrees". A pointer press on the area also
moves the thumb to the press point (a click picks), then drags.

**Current and old swatch: not in this slice.** It needs an "editing session"
(when does "old" get captured?) and a place that does not exist yet: the sketch
of `color-management` puts the pair in its Picker tab. Doing half of it here would
be rebuilt there. The Color row is built so that the pair replaces the single
swatch without changing anything else: see the UX notes of `color-management`.

### 5. Eyedropper

- **Button:** 28 x 28, icon Lucide `Pipette` 16 px, 1.5 px stroke, between
  swatch and hex. Names "Pick stroke color from the drawing" / "Pick fill color
  from the drawing". `aria-pressed`: true while picking. Looks like a pressed
  `ToggleGroup` item while picking (`--toolbar-icon-active-bg`, white glyph),
  otherwise a bordered button with hover `--editor-accent-hover`. Tooltip: "Pick
  a color from the drawing (Esc cancels)".
- **Cursor** over the canvas while picking: an eyedropper image (24 px, black
  with a white casing, hotspot at the tip), fallback `crosshair`. Over the panel
  the normal arrow. No hover box, no hover highlight of objects while picking:
  the Select tool draws only what is selected.
- **Hover preview (criterion 23):** a text readout chip of the existing kind
  (the "Live transform readout" surface: `--toolbar-bg`, 8 px radius, 12 px up
  and to the right of the pointer, flips at the canvas edges via
  `readoutPlacement.ts`, `pointer-events: none`). Content: a 16 px swatch (alpha
  over checkerboard, `--swatch-border`) and the colour as `#RRGGBBAA` (12 px
  tabular). If `adrs.md` makes the source an object paint, a muted word after
  it names it ("stroke" or "fill"). Where nothing is painted the chip reads "No
  paint here" with no swatch, which tells the maker why a click would do nothing
  (criterion 25). Updated at most once per animation frame, using the Select
  tool's hit test, so large documents do not slow the pointer.
- **Click** (criterion 24) commits and ends picking; focus goes to the canvas.
  **Cancel:** Escape (first step of the cascade, criterion 60), a tool change, a
  rail press, any press in the panel other than the button, the button again,
  and a right-click on the canvas, which opens no menu while picking
  (criterion 26). **Pan, zoom and the wheel keep working** and do not end picking,
  since the maker may need to move to the colour they want.
- **The chip is not a popup.** It holds no control, takes no focus and has no
  pointer events; it is the readout pattern the design system already has. The
  panel rule is about controls (C3).
- **No keyboard path (criterion 27).** The button can be focused and activated
  by key (it starts picking, Esc ends it), but only the pointer picks. The
  accessible name does not promise more than that; the hex field is the keyboard
  way to a colour.
- **After picking,** the colour is in the hex field, the swatch and the picker
  thumbs, exactly like a typed colour. A polite status "Stroke color set to
  #2F6FEEFF" is announced once through the app's status region (reuse the
  existing one if there is one; do not add a panel-local live region).

### 6. Dash

- **Row "Dash":** four buttons in one `radiogroup` (one Tab stop, arrows move and
  select), 44 x 28 px each (176 px), 1 px `--toolbar-icon` 60% outline, items
  separated by 1 px, pressed look as the other `ToggleGroup`s. Each shows a line
  sample, 2 px thick, `--toolbar-icon`, centred, 32 to 34 px wide, at the stored
  ratios scaled to a whole number of repeats: Solid unbroken; Dash 8 on / 5 off
  (three dashes); Dot 2 / 6 (five dots); Dash-Dot 9 / 4 / 2 / 4 (two dashes, two
  dots). Names "Solid", "Dash", "Dot", "Dash-dot"; tooltips add the numbers
  ("Dash: 6 4"). Pressed items show their sample in `--toolbar-icon-active-fg`.
- **Row "Pattern":** label "Pattern" in the label column, text input 176 px, 14 px
  tabular, left-aligned, fixed suffix "x width" (12 px, `--panel-muted-fg`) at the
  right inside the field, so the unit of the numbers is visible without a
  caption. Placeholder when Solid: "Solid. Example: 6 4"; Mixed: "Mixed". Tooltip
  (the label of criterion 29): "Lengths in multiples of the stroke width: on,
  off, on, off. Example: 1 2 4 2". Accessible name "Stroke dash pattern,
  multiples of the stroke width, on then off".
- **Behaviour:** the shared text-field rules; commit on Enter or Tab; nothing
  previews while typing. A committed text is shown back as numbers with one
  space and a point as decimal separator (`1   2 4  2` becomes `1 2 4 2`; a comma
  is refused, criterion 30), and a
  preset that matches presses its button (criterion 31). A preset press writes
  its numbers into the line at once.
- **Error chip** (criterion 30): the same chip as the number fields (12 px text,
  `--field-invalid`, 12 px alert glyph, ring, right-aligned under the field,
  `role="alert"`, overlay so no row shifts, cleared on the next keystroke). One
  change for all chips: maximum width 244 px, not 168, so this message needs two
  lines and not four. Message: "Enter 1 to 16 numbers from 0 to 1000, for
  example 1 2 4 2".
- **Example:** `1 2 4 2` at width 0.5 mm draws 0.5 mm on, 1 mm off, 2 mm on,
  1 mm off, repeating; widening the stroke to 1 mm doubles all four.
  `0 3` with round caps draws dots (the Pattern tooltip does not explain this;
  it is a power-user case).
- The tooltip note of `0007` stays on the Dash group: "Patterns scale with the
  stroke width and draw solid when too small to see."

### 7. Preview, commit, focus, keys

As `0007` section 7 and criterion 57, with these changes: there are no
popovers, so "Escape closes a popover" is gone; the picker, the value fields
and the eyedropper are covered by criteria 18, 19, 39, 40, 43 and 60.

| Key | Where | Does |
|---|---|---|
| `Shift+Ctrl+F` (Cmd on macOS) | anywhere | Expands the panel if collapsed and focuses Stroke Paint; empty panel: the panel itself (`tabindex="-1"`). Never closes it |
| Tab / Shift+Tab | panel | Collapse tab, Stroke Paint, eyedropper, hex, area, hue, Opacity, Width, Dash group, Pattern, Join, Cap, marker groups and Count (`stroke-markers`), Fill Paint, eyedropper, hex, area, hue, Opacity. Reset icons and the swatch are not stops |
| Arrows (Shift = 10x, Ctrl = 1/10) | value field | Step on the grid, preview on key-down, commit on key-up |
| Arrows (Shift = 10 x) | area, hue | 1 %, preview on key-down, commit on key-up |
| Home, End | value field | Scale minimum, scale end (drag maximum) |
| Enter, F2, a digit, `.`, `,`, `-` | value field | Start typing (the key replaces the text) |
| Ctrl+Backspace (Cmd on macOS) | value field | Reset to default |
| Enter, Tab, Escape | any text input | Commit and return focus, commit and move on, restore and return |
| Escape | anywhere in the panel | The cascade of criterion 60; never clears the selection |
| Backspace, Delete, letters | any panel control | Never reach the canvas |

Validation chip for every text input and value field in the typing state: below
the field, right-aligned, 12 px, max width 244 px, `--field-invalid` text and
ring, `role="alert"`, it takes no pointer events and covers whatever lies below
it for as long as the error stands. Messages: criteria 12, 30, 42 and
`stroke-markers` 31. Width: "Enter a number from 0 to 1000".

### 8. Tooltips and names

**Tooltips are allowed in the panel** (the panel rule lists them): text only, 400
ms delay, `side="left"` so they open over the canvas and never over the next
row, no controls inside, never take focus, dismissed by any key, press or leave.
They exist on: icon-only controls (Paint items, Join, Cap, eyedropper, reset,
marker items, the collapse tab), the Dash and Pattern controls, and each value
field. The value-field text is the modifier legend: "Drag to change, click to
type. Shift: coarse. Ctrl: fine. Ctrl+Backspace: reset." (Cmd on macOS.) A
tooltip is suppressed while a drag runs.

**Accessible names:** panel `aside` "Properties"; section "Style"; groups "Stroke
paint" ("No stroke", "Solid stroke"), "Fill paint" ("No fill", "Solid fill");
"Stroke color hex (RRGGBBAA)", "Fill color hex (RRGGBBAA)"; "Pick stroke color
from the drawing", "Pick fill color from the drawing"; "Saturation and value",
"Hue" (each prefixed "Stroke" or "Fill" in the accessible name, since there are
two); "Stroke opacity", "Stroke width", "Fill opacity"; "Reset stroke width to
0.25 mm"; "Stroke dash" group ("Solid", "Dash", "Dot", "Dash-dot"); "Stroke dash
pattern, multiples of the stroke width, on then off"; "Stroke join" (Miter, Round,
Bevel join), "Stroke cap" (Butt, Round, Square cap); collapse tab "Hide
properties panel" / "Show properties panel".

### 9. Tokens and contrast

Full rows in `docs/design-system.md`. New: `--value-fill` (`--accent` at 28%
on white = `#C5D7FA`), `--value-edge` (`--accent`), `--picker-thumb-ring`
(`#FFFFFF`), `--picker-thumb-casing` (`#000` at 40%). Reused: `--toolbar-icon`,
`--panel-muted-fg`, `--field-placeholder`, `--field-invalid`,
`--editor-accent`, `--swatch-border`, `--checker-*`, `--mixed-hatch`. Retired
in the panel: `--field-disabled-bg`, `--field-disabled-fg` (no disabled
control), `--no-paint-slash` (no slashed swatch).

Measured (WCAG, panel ground `#DCDCE0`, field ground white):

| Pair | Ratio |
|---|---|
| Label and value (`--toolbar-icon`) on the fill bar | 7.8 |
| Unit (`--panel-muted-fg`) on the fill bar | 4.7 |
| "Mixed" (`--field-placeholder`) on white | 5.3 |
| Bar edge (`--accent`) on white | 4.5 |
| Bar tint on white | 1.45 (decoration; the number and the edge carry the value) |
| Field border at rest on the panel | 3.1 |
| Field border on hover | 8.3 |
| Reset glyph (80%) on white / on the bar | 6.2 / 4.3 |
| Validation text and border (`--field-invalid`) on the panel | 4.8 |
| Pressed `ToggleGroup` item glyph (white on `--toolbar-icon-active-bg`) | 4.5 |

Hit targets: value field 244 x 28; reset 24 x 24 inside it; eyedropper 28 x 28;
toggle items 40 or 44 x 28; area and hue are 244 wide. Nothing under 24 px.

### 10. Motion

None. State changes, show and hide of rows, the bar, hover and focus are
instant. Reason: the rows leave the tree (criterion 5), so there is nothing to
animate out; the bar follows a pointer and any easing would lag it; the
panel's work is repeated for hours and a tool should not make the maker wait
for it. The `prefers-reduced-motion` rule needs nothing because there is no
motion to reduce. The one moving thing is the pointer-driven bar.

### 11. Changes the criteria need (for the PO)

All applied by the PO on 2026-10-08: C1 in criteria 5 and 6, C2 in 34 and 61, C3
in 55, C4 in 26, C5 in 36 to 38, C6 as criterion 61, C7 in 43, C8 in 12, 30 and
42, C9 in 1, C10 in 22. The text below is the request as made.

- **C1, criteria 5 and 6:** read "its heading and the Paint row" as one row (the
  section title and the Paint switch share it).
- **C2, criterion 34:** add the 24 px reset slot inside the right end of the
  field and the label inside the field (the width is the content width).
- **C3, criterion 55:** the exceptions are text-only tooltips, validation
  messages, and "canvas readouts that hold no control, such as the existing
  transform readout and the eyedropper's colour chip".
- **C4, criterion 26:** add "a right-click on the canvas ends picking and opens
  no menu"; add that pan, zoom and the wheel work and do not end it.
- **C5, criteria 36 and 37:** state that the value is `clamp(p0 + dx / W)` from
  the press, with no re-basing at the ends.
- **C6, new criterion in "Value fields":** reset (the icon and `Ctrl+Backspace`,
  default Width 0.25 mm, Opacity 100 %, Count 1, one commit, sets all when
  Mixed, nothing at the default; bare Backspace and Delete never reset).
- **C7, criterion 43:** `aria-valuemax` is the typed maximum; `End` goes to the
  scale end; add `aria-keyshortcuts` and `Ctrl+Backspace`.
- **C8, criteria 12, 30:** the chip's maximum width is 244 px (wording only).
- **C9, criterion 1:** add "if a focused control leaves the tree, focus goes to
  its section's Paint group, or to the canvas when the panel became empty."
- **C10, criterion 22:** the swatch is `aria-hidden` and not a Tab stop (it says
  so in 16; repeat in 22 so the Tab order is testable).

### 12. Questions for the customer (each has a default)

All four decided by the lead on 2026-10-08, taking the defaults: U1 always
visible, U2 Stroke above Fill, U3 reset icon only when the value differs from the
default, U4 the current/old pair in `color-management`. The customer may still
reopen them after trying the panel.

- **U1. Picker height.** Always visible (default), or a disclosure arrow per
  colour. Decide after trying the panel at your usual window size.
- **U2. Order.** Stroke above Fill as built (default), or Fill above Stroke as
  Inkscape and Illustrator have it.
- **U3. Reset icon.** Shown only when the value differs from the default
  (default), or always.
- **U4. Current and old swatch.** In `color-management` with the tabs
  (default), or here as a split swatch in the Color row.

## Links
Requirements: R-EDIT-005, R-EDIT-006 (`docs/requirements.md`)
Supersedes in part: `specs/0007-stroke-and-fill-styling/`
Followed by: `specs/0018-stroke-markers/`, `specs/0022-color-management/`
PR: TBD
