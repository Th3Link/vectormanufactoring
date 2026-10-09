# Color management: palettes, thread palettes, colour history, other colour models

Status: Draft
Priority: Could
Origin: Customer

A sketch for the customer to react to, not a story to build yet. After the MVP
(the laser sequence in `specs/README.md`), and after `style-panel-rework`, whose
inline colour controls this extends. Numbers and names below are proposals until
the customer has read them. No `adrs.md` content yet.

## User value

As a maker I want a colour palette that I can fill, rename, reorder, import and
export, with thread palettes for embroidery, and the colours I used last at hand,
so that I pick a colour once, reuse it everywhere, and (for embroidery) pick
colours that are real threads from a thread maker's chart.

The customer's reasons: colours matter, for embroidery most of all, so the
palette has to be customizable and extendable; thread palettes come later on top
of it. It is one advanced feature, "colour management", and it is explicitly not
a wide bar like Inkscape's.

## What exists today (after `style-panel-rework`)

Per colour: a swatch, an eyedropper, an 8-digit RGBA hex field, an inline
saturation/value area and hue slider (always visible, no disclosure), and an
Opacity value field with a reset icon. No popup. That is enough to pick any sRGB
colour and nothing more. No palette, no history, no other colour model, no
current/old swatch pair.

## Reference tools

- **Inkscape:** a palette bar along the bottom of the window plus a Swatches
  dialog; palettes are GIMP `.gpl` files copied into the user's palettes
  folder. Wide, always visible, and the bar takes canvas height.
- **Ink/Stitch (Inkscape extension):** ships palettes of thread makers' colour
  charts that are installed as `.gpl` files, and can install a maker's own
  `.gpl` of the threads they own. The chosen palette also drives the thread names
  shown in print preview, and the colours go into the embroidery file when the
  format carries colour. This is the bar for the embroidery use.
- **LightBurn:** a fixed row of layer colours; colour is the layer, not a style.
  Not the model here, because ours is a style per object with manufacturing roles
  assigned separately (`R-MFG-002`).
- **GIMP:** its colour dialog has everything (several colour models, numeric
  entry with +/- steppers, 0..100 and 0..255 toggles, current and old swatches,
  history in two rows with a large + button). The customer: powerful, far too
  much, restless. Kept from it: the current/old swatch pair.

## Guardrails from the customer (final)

- Inline, no popups, no popovers (`style-panel-rework` panel rule).
- No fat bar. Whatever we add lives in the properties panel, in the colour block.
- Tabs, not stacked rows: colour history goes into a tab.
- No +/- buttons. No 0..100 / 0..255 toggles.
- A current and an old swatch side by side in the picker.
- Palettes are customizable and extendable; thread palettes come later.

## Rough acceptance criteria (sketch; each gets numbers and tests before Ready)

1. **Tabs in the colour block.** Under each Solid colour (stroke, fill) a tab strip
   with Picker, Palette and History. Picker is the inline area and hue slider of
   `style-panel-rework`. The tabs are inline controls, not a popup. The last tab
   used is remembered per session.
2. **Current and old.** In Picker a pair of swatches, side by side: "old" is the
   colour when the maker started editing (selection change or tab entry),
   "current" is the live colour. Clicking "old" restores it in one commit.
3. **History.** Tab with the colours the maker applied recently, newest first,
   duplicates merged, a fixed number (proposal: 24), in a single compact grid. A
   click applies the colour to the selection. No "+" button; a colour enters the
   history when it is committed. Opacity is part of the entry (RGBA).
4. **Palette.** Tab with the swatches of the active palette in a grid. A click
   applies the colour (RGBA, opacity 100% unless the entry has its own). The
   active palette is chosen from an inline list or a segmented control, not a
   dropdown. Hover or focus shows the entry's name and value as a tooltip.
5. **Edit palettes.** The maker can create, rename and delete a palette; add the
   current colour to it; remove, rename and reorder entries (drag). Built-in
   palettes cannot be changed, only copied.
6. **Import and export.** Read and write GIMP `.gpl` (name and RGB per entry; the
   format Inkscape and Ink/Stitch use). Later: Adobe `.ase` read. Import shows
   what was skipped (names too long, unknown model).
7. **Thread palettes.** Palettes whose entries have a maker, a thread number and a
   name (for example "Madeira Polyneon 1147 Cardinal"). Choosing one stores the
   RGB the object shows; whether it also stores the thread identity is open
   question 3. Which makers ship with the app depends on licensing (open
   question 4).
8. **Other colour models.** Optional numeric entry for HSL, HSV and CMYK, shown as
   value fields in the same GIMP style as Width and Opacity (label left, value
   right, drag or type, no steppers), in the Picker tab behind a single choice of
   model, not shown together. Values convert to and from sRGB; CMYK is a plain
   conversion unless open question 5 says otherwise.
9. **Eyedropper variants.** Pick with the eyedropper into the palette ("add to
   palette") as well as into the colour. Pixel sampling and outside the app window
   only if open question 7 asks for them.
10. **Offline and local.** Everything works with no network (`R-SYS-002`). Palettes
    sync like the other libraries once the sync design says how (open question 2).

## Open questions (customer)

1. **Palette format inside the app.** *A (proposal, recommended):* our own small
   record per palette (name, entries with RGBA and optional maker, code, name),
   with `.gpl` only for import and export, because `.gpl` has no place for a
   thread maker or an opacity. *B:* `.gpl` itself as the stored format. Default A.
2. **Where palettes live.** *A (recommended):* user palettes are app-level library
   records, one small file each, synced like the material database and asset
   libraries (ADR 0004 §6 and §11); a project may carry a copy of the palettes it
   uses so that it opens the same on another machine. *B:* palettes only inside
   each project (a "document palette"). *C:* both, with the project palette
   built from the colours used. Default: A, plus the project copy. The architect
   decides the storage in an ADR.
3. **Thread identity.** For embroidery the job needs the thread, not just the RGB.
   *A (recommended):* an object stores RGBA only; the embroidery story matches the
   nearest thread in the chosen thread palette at job time. *B:* a palette entry
   applied to an object keeps a link (maker, code), so the exact thread survives
   edits and is listed in the job. *B* is a document-model change and belongs in
   the embroidery story. Default A.
4. **Which thread palettes, and the rights.** Thread makers' colour charts may be
   protected or have trademarked names. We need to check what Ink/Stitch and
   others ship and under what licence before bundling any. Default: ship none;
   offer import of the maker's `.gpl`, and add bundled charts one by one after the
   rights are clear.
5. **CMYK and ICC.** *A (default):* CMYK entry is the plain device-independent
   formula, labelled as such, no colour profile. *B:* ICC profiles for print and
   cutting-plotter print-then-cut. Laser, plotter and embroidery work in sRGB;
   B is only worth it for print-then-cut (`R-MFG-CUT-002`). Recommendation: A.
6. **Colour history scope.** Per project, per app, or both. Default: per app,
   shared across projects, not synced, not stored in the project.
7. **Eyedropper variants.** The screen-wide eyedropper (outside the app window)
   needs an operating system service on each of Linux, Windows and macOS, and does
   not exist in a browser. Default: only the drawing, as in `style-panel-rework`.
8. **Spot colours / named colours for laser layers.** LightBurn ties cut settings
   to a layer colour. Should a palette entry be able to carry a manufacturing role
   (for example "cut" red)? Default: no, roles stay with `manufacturing-roles`.
9. **When.** Default: not scheduled; after the laser MVP. The customer says if
   embroidery moves up, since thread palettes come with it.

## Out of scope

- Gradients, patterns and swatch fills (removed or not planned; see
  `style-panel-rework`).
- Colour-managed display, soft proofing, ICC unless open question 5 says so.
- Automatic palette extraction from an image, and "reduce to N colours". A tracing
  story (`raster-trace`, R-VEC-001) may want it; not here.
- Matching to a thread chart by nearest colour at job time: the embroidery story.
- Shared palettes between collaborators during live editing (`R-COLLAB`).

## UX notes

Sketch only (2026-10-08, ux-engineer); the numbers are proposals until this
story is Ready. It extends the Color row and the inline picker that
`style-panel-rework` builds (its "Colour block" UX notes), and obeys the panel
rule: no popups, no popovers, no dropdown lists.

- **Where.** The tab strip goes between the Color row and the picker, per
  colour (Stroke, Fill). Tabs "Picker", "Palette", "History": a segmented
  `tablist` (one Tab stop, arrows move and select, `role="tab"` /
  `tabpanel`), 28 px high, text 14 px, pressed look as the other segmented
  groups. The tab content area has the picker block's height (116 px) in every
  tab, so switching tabs does not move anything below. The last tab is kept per
  session; the panel keeps the instant-show, no-animation rule.
- **Current and old.** The pair replaces the single 28 px swatch of the Color
  row with one 56 x 28 px element, side by side: left "old" (the colour when
  the maker started editing: the selection changed, or the tab was entered),
  right "current" (live). Before any change both halves are equal. The old half
  is a button (Tab stop, tooltip "Restore #2F6FEEFF"); a press restores it in
  one commit. The current half stays display-only. The hex field, which fills
  the rest of the row, gets 28 px narrower; nothing else in the Color row moves.
  One pair per colour, not per tab, so it is visible from every tab. Decided by
  the lead (2026-10-08): the pair belongs to this story and is not built in
  `style-panel-rework`. There the swatch is display only and not a Tab stop
  (criteria 16, 22); the "old" half is the one place this story adds a Tab stop,
  so criterion 22 gets an amendment when this story is built.
- **History tab.** A grid of the last 24 committed colours, newest first,
  duplicates merged, 10 cells per row (cell hit area 24 x 24 px, swatch 20 px, 244
  px wide), so 3 rows at 72 px, inside the 116 px block. A click applies the
  RGBA in one commit; the eyedropper keeps working with the tab open. No "+"
  button. Hover or focus shows the hex as a tooltip. The grid is one Tab stop (roving
  tabindex); the arrow keys move in two dimensions. An empty history shows a
  muted 12 px line, "Colors you apply appear here."
- **Palette tab.** The same grid for the active palette. The palette is chosen
  by an inline segmented control above the grid (never a dropdown). More than
  three palettes need a design decision (a taller block, or an inner scroll
  area, which the panel avoids); open. Editing a
  palette (add the current colour, rename, remove, reorder) uses inline controls
  in the same area, with drag to reorder and `Alt+arrows` as the key route.
  Entry names and values are tooltips. Built-in palettes show a lock glyph and
  a "Copy" action in place of editing.
- **Other colour models (criterion 8).** HSL, HSV or CMYK values are value
  fields as in the Style section (label left, value right, drag or type, no
  steppers, reset to the neutral value), shown in the Picker tab below the hue
  slider, one model at a time chosen by a segmented control. They add height to the
  Picker tab only; the tab content area then grows for all tabs so nothing
  jumps between tabs (the picker block becomes the height of the tallest tab).
- **Thread palettes** need a code and a name per entry: the tooltip becomes two
  lines ("Madeira Polyneon 1147" / "Cardinal"); the grid does not change.
- **Not decided here:** palette import and export. The sketch assumes the
  operating system's own file dialog, which is not a layer of the panel and
  does not break the panel rule; confirm when this story is written up.

## Links
Requirements: R-EDIT-017 (`docs/requirements.md`); related R-MFG-EMB-001 to
R-MFG-EMB-003, R-SYS-004
Depends on: `specs/0017-style-panel-rework/`
PR: TBD
