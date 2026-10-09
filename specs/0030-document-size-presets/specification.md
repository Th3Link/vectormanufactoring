# Document size presets: paper sizes A0 to A6 and slide formats

Status: Ready (`adrs.md` and the UX notes exist, both 2026-10-10)
Priority: Should
Origin: Customer (the request, the paper and slide groups, the data file, inline choice, portrait/landscape, round trip). The slide list, the pixel rule and the orientation rule are my proposals, marked in "Decided by the product owner" and in the open questions.

## User value

As a maker I want to set my document to a standard sheet (A0 to A6) or a slide format (16:9, 16:10, 4:3) with one click, and turn it between portrait and landscape, so that I do not look up or type millimetres for the sizes I use every day. As someone who extends the product I want the list of formats to live in one small data file, so that adding a format is adding a few lines and not changing code.

**What we match, and what we do differently.** Inkscape's Document Properties and Illustrator's New Document dialog both offer a list of page formats with an orientation switch, in a dropdown or a dialog. Figma lists frame sizes (including presentation 1920 x 1080) in a menu. LightBurn has no document size: the work area comes from the machine. Differences here:

- The formats are buttons in the Properties panel, all visible at once. No dropdown, no dialog (`0017-style-panel-rework`, panel rule; `0015-document-size-and-rulers` Question 1, option A).
- Choosing a format resizes the document **around its centre**, exactly like typing a size (`0015`, criterion 17). Old objects stay where they are relative to the centre; Inkscape resizes from a corner. Nothing is scaled and nothing is rotated.
- The selected format is **derived from the size**, never stored. Type 297 x 210 and A4 shows as selected; type 211 and nothing does. There is no state to get out of sync.
- The list comes from a data file that is checked by a test, not from code.

**Slide sizes.** Slides are defined in pixels, because that is how makers know them (1920 x 1080), and stored in millimetres like every other size. The rule is the CSS and SVG rule, 96 pixels per inch: 1 px = 25.4 / 96 mm. So 1920 x 1080 px is 508 x 285.75 mm. This is the same rule Inkscape and SVG use for "px" and the one `docs/adr/0002-document-model-units-and-svg-round-trip.md` uses at the import and export boundary. No pixel unit is added to the Document section: the fields keep showing mm, cm or in (`0015`, Part C).

## Acceptance criteria

Sizes are in millimetres unless a criterion names another unit. "Same size" means the document's width and height, each within 0.01 mm. A document is "in portrait" when its width is smaller than its height, "in landscape" when it is larger, and square when the two are within 0.01 mm.

### Part A: the preset set and its data file

1. Given a build of the app, then the presets are these, in this order, in two groups. Group **Paper** (default orientation portrait): A0 841 x 1189, A1 594 x 841, A2 420 x 594, A3 297 x 420, A4 210 x 297, A5 148 x 210, A6 105 x 148 mm (ISO 216). Group **Slides** (default orientation landscape): **16:9** 1920 x 1080 px = 508 x 285.75 mm, **16:10** 1920 x 1200 px = 508 x 317.5 mm, **4:3** 1024 x 768 px = 270.9333... x 203.2 mm. Test: the list read through the public function equals this table, every side within 1e-9 mm.
2. Given the source tree, then no preset name, size or group is written in any Rust or TypeScript source file outside tests. They are in one TOML data file at `curvyo-document-core/data/document-presets.toml`, embedded with `include_str!` (the format is shown under "Data file format"; location and parser per `adrs.md`). Check: a search for `841` and `1189` in non-test sources finds nothing.
3. Given the data file, when a maker or developer adds one `[[group.preset]]` block to a group, then the new preset appears in the Document section at that place in that group, and nothing else needs changing (no Rust, no TypeScript, no CSS). Test: the loader (criterion 4) is called with the shipped text plus one extra block; the returned list has one more entry at the expected index; and the panel view built from a list of 9 paper presets has 9 paper entries (a Rust test in `curvyo-ui-core`). The frontend has no test runner, so the last step, that the rendered panel shows 9 paper buttons, is a browser check by the tester on the finished slice, not an automated frontend test.
4. Given the loader, then it takes the file text as input and returns the validated list or an error; it opens no file, reads no clock and starts no thread, and the crate that holds it builds for `wasm32-unknown-unknown` (`CLAUDE.md` §6). The error type names the problem (criterion 6).
5. Given a size in the file, then its `unit` is `mm`, `in` or `px`. Conversion to mm: `mm` as is; `in` as `value * 254 / 10` (as `0015` as-built notes, so 8.5 in is the double nearest 215.9 mm); `px` as `value * 254 / 960`. Test: 1920 px gives exactly 508.0 mm, 1080 px gives exactly 285.75 mm, 1200 px gives exactly 317.5 mm, 768 px gives 203.2 mm (within 1e-9). The document stores millimetres only.
6. Given a file that breaks one of these rules, then the loader returns an error that names the preset id (or group id) and the reason, and no list. Each rule has its own failing fixture and test:
   a. the text is not valid TOML, or has a key the format does not define (typos fail);
   b. `format` is missing or is not 1;
   c. a group is missing `id`, `name` or `default_orientation`; `default_orientation` is not `portrait` or `landscape`; a group has no preset; two groups share an id;
   d. a preset is missing `id`, `name`, `short_side`, `long_side` or `unit`; two presets share an id (across all groups);
   e. `unit` is not `mm`, `in` or `px`;
   f. a side is not a finite number, or is 0 or negative, or `short_side` is larger than `long_side`, or a converted side is below 1 mm or above 100 000 mm (the limits of `0015` criterion 16, so a pick can never be refused);
   g. an `id` has characters other than lower-case letters, digits and `-`; a `name` is empty or longer than 24 characters after trimming; a `note` is longer than 60 characters;
   h. two presets in the whole file have the same size (each side within 0.01 mm), because the selected state (criterion 10) could not tell them apart.
7. Given the shipped data file, then a test in the default `cargo nextest run` loads it and requires success, and a second test asserts criterion 1. A bad edit to the file therefore fails the quality gate and CI, and no build can contain it. There is no build script and no run-time check on the maker's machine.
8. Given the list, then groups appear in file order and presets within a group in file order. Nothing is sorted by the program.

### Part B: choosing a preset in the Document section

9. Given the Document section is shown (`0015` criterion 14a), then it also shows the presets: for each group a small heading with the group's `name`, and under it one button per preset with the preset's `name`, in file order. All buttons are in the panel's own column and visible at once; nothing opens (no element with role `dialog`, `listbox` or `menu`, `0017` criterion 55). When the buttons of a group do not fit the 244 px content width they wrap to a further row. The section keeps the Width and Height fields, the Unit control and Fit to content of `0015` as they are. Exact rows and sizes: UX notes.
10. Given any document size, then a preset is shown as selected (pressed) if and only if the document has the same size as the preset in either orientation, that is, the document's shorter and longer side equal the preset's `short_side` and `long_side` after conversion. At most one preset is selected (criterion 6h). Test: a new project (210 x 297) shows A4 selected; Width 297 and Height 210 shows A4 selected; 210.004 x 297 shows A4 selected; 211 x 297 shows none; 508 x 285.75 shows 16:9 selected; 285.75 x 508 shows 16:9 selected. The selected state follows every change of the size (typing, a pick, Fit to content, file open) in the same frame as the Width and Height fields.
10a. Given the Document section, then its header row shows, right-aligned, a subject line with the name and orientation of the matching preset ("A4, portrait"; "16:9, landscape"; a square that matches a preset shows the name only) or "Custom" when no preset matches. It is derived from the size on every change, in the same frame as the fields, and never stored. It replaces the "no subject line" of `0015`. Test: a new project shows "A4, portrait"; Width 297 and Height 210 shows "A4, landscape"; 211 x 297 shows "Custom"; 508 x 285.75 shows "16:9, landscape".
11. Given a document of size (w0, h0) and a press on a preset button, then the new size (w1, h1) is the preset's size in the orientation of criterion 13, and it is applied exactly as `0015` criteria 17 to 19 define a resize: every object is moved by ((w1 - w0) / 2, (h1 - h0) / 2); nothing is scaled, cropped, restyled or rotated; one commit stored as `resize_document`. Test with the rectangle of `0015` (x = 10, y = 10, 50 x 50) in the 210 x 297 document: pick A3 and the document is 297 x 420, the rectangle at x = 53.5, y = 71.5; from there pick A5 and the document is 148 x 210 with the rectangle at x = -21, y = -33.5 (still 50 x 50). Pick 16:9 from A4 portrait: the document is 508 x 285.75 and the rectangle is at x = 159, y = 4.375.
12. Given a press on the preset whose size the document already has to within 1e-9 mm on each side in the orientation of criterion 13, then nothing is written and no commit is made (`0015` criterion 19). Given the document differs from it by more than 1e-9 mm but is shown selected (criterion 10), then the press sets the exact preset size.
13. Given a press on a preset P, then the orientation of the result is: if the document's current size matches a preset of the same group as P (criterion 10), the document's current orientation (portrait or landscape; a square counts as portrait); otherwise the default orientation of P's group. In words: staying inside a group keeps the orientation, entering a group starts with its default. Test: A4 landscape (297 x 210), pick A3 gives 420 x 297. A4 portrait, pick 16:9 gives 508 x 285.75 (landscape). A custom 300 x 400, pick A4 gives 210 x 297. A custom 400 x 300, pick A4 gives 210 x 297. 16:9 turned to portrait (285.75 x 508), pick 16:10 gives 317.5 x 508.
14. Given the Document section, then it has an orientation choice with the two items **Portrait** and **Landscape** (an inline group like the Unit control, no dropdown); each item shows a page glyph and its name, so a test can find the names. The pressed item is the document's current orientation (the definitions above Part A): Portrait when width < height, Landscape when width > height, none when square. A press on the item that is not pressed swaps width and height, as one resize that keeps the centre (`0015` criteria 17 to 19, `resize_document`). Test: A4 portrait with the rectangle of criterion 11, press Landscape: the document is 297 x 210 and the rectangle is at x = 53.5, y = -33.5, not rotated. A press on the pressed item, or on either item of a square document, writes nothing. The group is shown for every size, not only for sizes that match a preset (a custom 300 x 400 turns into 400 x 300).
15. Given a Width or Height field with edited text that is not yet committed, when the maker presses a preset button or an orientation item, then the typed text is committed first by the rules of `0015` criterion 15 (a refused value is refused as there), and then the press applies to the resulting size. The unit control already behaves this way (`0015` criterion 34).
16. Given keyboard use, then each group (Paper, Slides, Orientation) is one Tab stop; the arrow keys move within the group and press the item they land on, which is one commit per step; each button's accessible name contains the group name and the preset name (for example "Paper A4"); and after a mouse press, keyboard focus is on the canvas (`0017` criterion 57). The preset buttons never show disabled (`0017` criterion 3).
17. Given a preset button under the pointer or with keyboard focus, then a text-only tooltip names its size **as a press would set it**, that is in the orientation of criterion 13 and not as the preset's short and long side, in the display unit of the document with the number rules of `0015` criterion 35, written with the multiplication sign U+00D7 (the status bar's sign), for example "210 × 297 mm", in a landscape document "297 × 210 mm", or, in inches, "8.2677 × 11.6929 in". For a preset whose `unit` is `px` it names the pixels first, in the same orientation: "1920 × 1080 px = 508 × 285.75 mm", in inches "1920 × 1080 px = 20 × 11.25 in". If the preset has a `note`, it follows on a second line ("Full HD"). The tooltip holds no control and takes no focus.
18. Given the Document section is not shown (an object is selected, or the Pen has an unfinished path), then no preset button and no orientation item is in the tree. This slice changes when the section is shown in no case.

### Part C: round trip and persistence

19. Given a pick followed by Save, Close and Open, then the size is the same and the same preset is shown selected, with no preset id in the file. The project file stores no new value and `format_version` does not change (`0015` criterion 38). Test: pick 16:9, the fields show "508" and "285.75" in mm; with unit in (`0015` Part C) "20" and "11.25"; after the round trip A4 is not selected and 16:9 is.
20. Given a project whose size matches no preset (any file from before this slice with another size), then it opens unchanged, no preset is selected, and nothing is written by opening it.
21. Given a later edit of the data file (a size corrected, a preset removed), then no stored document changes; only which preset is shown as selected is recomputed on the next open.

## Data file format

Confirmed in `adrs.md` (2026-10-10). TOML is chosen for comments and hand editing. The file is `curvyo-document-core/data/document-presets.toml`, parsed by the `toml` crate (default features off; `std`, `parse` and `serde` only, no writer), which is already in `Cargo.lock` through the Tauri build and builds for `wasm32-unknown-unknown`. The loader is `PresetList::parse(text)` in `curvyo-document-core/src/document_presets.rs`.

```toml
format = 1                      # schema version of this file; a reader refuses another

[[group]]
id = "paper"
name = "Paper"                  # heading in the panel
default_orientation = "portrait"

  [[group.preset]]
  id = "a4"                     # stable, lower case; never shown
  name = "A4"                   # label on the button, 24 characters at most
  note = "ISO 216"              # optional, shown in the tooltip
  short_side = 210
  long_side = 297
  unit = "mm"                   # mm | in | px (px = 1/96 in)

[[group]]
id = "slides"
name = "Slides"
default_orientation = "landscape"

  [[group.preset]]
  id = "slide-16-9"
  name = "16:9"
  note = "Full HD"
  short_side = 1080
  long_side = 1920
  unit = "px"
```

Why `short_side` and `long_side` and not `width` and `height`: a preset has no orientation of its own, the group has a default and the document has a current one (criterion 13). Two numbers that cannot disagree with an orientation field are easier to validate (criterion 6f) than a width, a height and a flag.

**User-defined presets later (not built here).** The format does not block them: a second file of the same format, handed to the same loader as text by a platform crate that reads it from the maker's folder, could be appended after the shipped groups. Ids would need a prefix or a collision rule, which is that later story's decision. This slice adds no trait, no registry and no second loader.

## Out of scope

- **User-defined presets**: reading a file from the maker's folder, a "save current size as preset" action, editing or hiding presets. The format and the pure loader are ready for it (see above).
- **Other formats**: US Letter, Legal, Tabloid, B and C series, A7 to A10, business cards, social media sizes, laser bed sizes. Each is one block in the file once the customer wants it (Question 2). A bed size could also come from `machine-profile` (R-MFG-001) later; document size and machine work area stay independent until then.
- **Scaling the content to the new size**, a "scale objects" switch, anchoring to a corner. The centre-fixed resize of `0015` is the only behaviour.
- **Rotating the content** when the orientation changes. The objects keep their orientation and their place relative to the centre.
- **A pixel unit or a dpi setting** in the Document section or the project file. Pixels exist only as an authoring unit in the data file (96 per inch).
- **A New Document dialog** with presets. New still creates A4 portrait (`0001`).
- **Margins, bleed, safe areas**, several pages (dropped 2026-10-05).
- **Translated names** of groups or presets. The names are UI copy in English like all UI strings.
- **Export, print or job behaviour** for a preset size (`0015` criterion 32).

## Open questions

Each has a default; nothing blocks.

1. **Slide list (criterion 1).** *A (default, recommended):* three slides, one per ratio: 1920 x 1080, 1920 x 1200, 1024 x 768 px. *B:* also the office-suite sizes, 1280 x 720 px (PowerPoint widescreen, 13.333 x 7.5 in), 1280 x 800 and 960 x 720 px (PowerPoint standard, 10 x 7.5 in), six buttons. They are three more blocks in the file at any time. A keeps the group to one row.
2. **More paper formats.** Default: only A0 to A6, as asked. Say if US Letter, Legal and Tabloid (in inches in the file) should come with it; that is three blocks and would be a third group "US" or part of Paper.
3. **Data format (see above).** Resolved by the architect: TOML with the `toml` crate (MIT or Apache-2.0, builds for wasm32), justified in `adrs.md`. JSON was rejected: no comments in a file meant to be edited by hand.
4. **Orientation after a pick (criterion 13).** *A (default):* staying in a group keeps the orientation, entering a group starts with its default. *B:* a pick always keeps the current orientation (simpler; picking 16:9 in a new project then gives a portrait slide until Landscape is pressed). *C:* a pick always uses the group default (A3 landscape then A4 gives portrait). Recommendation: A.
5. **96 dpi (see User value).** Default: 96 pixels per inch. Some slide tools use 72 points per inch, which would make 1920 x 1080 a 677 x 381 mm document; that only matters if slides are printed. Say if you want 72.

Decided by the product owner (change if you disagree): the slide set of Question 1 A with the names "16:9", "16:10", "4:3"; Priority Should; a preset press is a resize and nothing else; the orientation group is shown for every size; presets keep working when the Pen is idle only (inherited from `0015`); the tolerance for "same size" is 0.01 mm, which is below the smallest difference between two presets (A4 and US Letter would differ by 5.9 mm).

## UX notes

By `ux-engineer`, 2026-10-10. Numbers and component rows are in `docs/design-system.md`, "Properties panel: Document section" (rows "Presets", "Preset strip", "Orientation", "Subject line"). Reference tools: Inkscape and Illustrator put formats in a dropdown or a dialog; here every format is visible and one press away, which is the whole point of the slice, and the panel rule forbids the dropdown anyway.

### Layout: where the list goes

The presets come **first**, above the fields, in the order Paper, Slides, Orientation, then a 16px gap with a 1px rule (the Stroke / Fill rule), then Width, Height, Unit, Fit to content as shipped. Reasons: the maker chooses a format, then reads or adjusts the exact size right under it; Orientation sits directly above the two numbers it swaps; Fit to content stays the last row, so its removal on an empty document still moves nothing. Width and Height only move when the block above them changes height, which it does not: the block has a fixed height for a given preset file, whatever is selected.

- **Header row:** "Document" at the left, and at the right a 12px `--panel-muted-fg` **subject line** (the Style header's pattern): the name and orientation of the matching preset, "A4, portrait", "16:9, landscape", or "Custom" when no preset matches. It is the only place that says "Custom" and the only place the selected state is also written as text, so it is not colour alone. Derived like the pressed state, never stored, not a live region. This replaces `0015`'s "no subject line".
- **Group block**, three times (Paper, Slides, Orientation): a 16px heading line (12px semibold, `--toolbar-icon`; "Paper", "Slides", "Orientation"), 4px, then the strip (28px). 12px between blocks. Whole presets block 168px, section about 352px with the fields; the 800 x 600 viewport is about 546px, so **nothing scrolls** and Fit to content is visible. If the data file grows, the panel scrolls as a whole (one scrollbar, no inner scroll area, no second column of chips).
- **Strip:** the panel's `ToggleGroup` look (one 1px `--toolbar-icon` at 60% border, items separated by 1px, 28px high, `rounded-[5px]` outside). Cells `flex: 1 1 auto`, **minimum 32px**, 6px side padding, 14px text, never truncated: a cell is as wide as its name needs. The 244px strip then gives Paper seven cells of about 34.9px (7 x 32 + 6 dividers = 230px, 14px slack for font differences, so Paper never wraps by accident), Slides three cells of about 81px, Orientation two of about 122px. A group whose cells do not fit one row wraps to a further strip row of the same look, rows spanning the full width, cells of the last row growing to fill it (nine paper presets give 5 + 4; accepted for a rare case). A name of 24 characters (the data limit) takes its own row.
- **Pressed**, hover, focus: as the `ToggleGroup` item of the Style section (`--toolbar-icon-active-bg` ground and `--toolbar-icon-active-fg` text, 4.5:1; hover `--editor-accent-hover`; 2px `--editor-accent` focus ring inside the strip). At most one preset cell is pressed across Paper and Slides (criterion 6h). None is pressed for a custom size, and the subject line says "Custom"; there is no "Custom" cell (as the dash presets have none).
- **Orientation** has two cells with a 16px page glyph (1.5px stroke, `currentColor`) and the name: Portrait a 9 x 12 rounded rectangle, Landscape the same turned, 12 x 9, then "Portrait" / "Landscape" 6px right of it. Pressed follows the document's orientation; **a square document presses neither**. Names are always visible; the glyph alone would not tell the two apart for a screen magnifier user.
- **Empty and error states:** if a group has no presets it draws nothing; if the whole list is empty (the loader's error branch, unreachable by the tests) only Orientation and the fields show. Nothing is dimmed or disabled.

### Behaviour

- **Press (mouse):** one `resize_document` commit. In the same frame the fields, the subject line, the rulers, the status bar and the pressed cell change; the artwork does not move on screen, the document edges do (`0015` Feedback). No animation, no confirmation. Focus goes to the canvas after a mouse press (criterion 16).
- **What the fields show:** the new size in the display unit by `0015`'s number rules (A3 from A4: "297" and "420"; 16:9 in inches: "20" and "11.25"). The fields keep their text and unit; a field that had focus loses it to the canvas with the press.
- **Edited text in a field:** committed first (criterion 15). If it is refused, the validation chip shows under that field for its usual time; the press still applies to the unchanged size.
- **Keyboard:** each group is one Tab stop with roving focus (the `ToggleGroup` behaviour of the Style section): Tab lands on the pressed cell, or on the first cell when none is pressed; Left, Right, Up, Down, Home and End move **and press** (one commit per step, criterion 16); focus stays in the group after a key press. Tab order: Paper, Slides, Orientation, Width, Height, Unit, Fit to content. `Shift+Ctrl+F` still focuses Width (`0015`); the presets are one `Shift+Tab` above it. The arrows press because that is how every group of the panel behaves and because a size change is exact to undo by stepping back (the centre-fixed resize returns the objects to the same positions); open design question 1 has the alternative.
- **Tooltips** (Radix `Tooltip`, 400ms, `side="left"`, text only, also on keyboard focus). Line 1: the size **as a press would set it** (orientation by criterion 13) in the display unit, with the multiplication sign U+00D7 as the status bar uses: "210 × 297 mm"; document in landscape "297 × 210 mm"; unit in "8.2677 × 11.6929 in". A px preset names the pixels first: "1920 × 1080 px = 508 × 285.75 mm", in inches "1920 × 1080 px = 20 × 11.25 in". Line 2, muted, only if the preset has a `note`: "ISO 216", "Full HD". Orientation: Portrait "Taller than wide. Swaps width and height." / Landscape "Wider than tall. Swaps width and height." with the muted second line "Objects keep their place relative to the centre; nothing rotates." Tooltips carry no control and take no focus.
- **Accessible names:** groups `role="radiogroup"` named by their heading ("Paper", "Slides", "Orientation"; `aria-labelledby`); cells `role="radio"`, `aria-checked`, names "Paper A4", "Slides 16:9", "Orientation Portrait"; the tooltip text is the description. The subject line is plain text in the header. Non-text contrast: pressed fill 3.3:1 to the panel and glyph/text 4.5:1 (as the existing groups); cell borders 3.1:1. Hit target 32 x 28px at the smallest, above the 24px minimum.
- **Not shown / no writes:** with an object selected or an unfinished Pen path none of this is in the tree (criterion 18). The section never shows a disabled control.

### Open design questions (defaults taken)

1. **Arrow keys press.** *Default:* yes, as the other groups (a resize is exactly reversible). *Alternative:* arrows move focus, Space or Enter presses (Radix `ToggleGroup` instead of `RadioGroup` for these three groups only): safer once undo and many presets exist, and a second behaviour in one panel. Revisit with `undo-redo`.
2. **Presets above the fields (default) or below.** Above reads as "choose, then adjust"; below leaves the shipped fields where they are. Cost of above: the Width field moves 184px down, which only matters to someone with muscle memory from `0015`.
3. **Subject line "A4, portrait" / "Custom".** *Default:* yes. Remove it if the pressed cell is felt to be enough; then "Custom" is not written anywhere.
4. **Slides cells stretch to 81px.** *Default:* yes, one rule for all strips. A fixed 44px cell would leave 112px of empty strip.

### Criteria changes requested (by number)

Applied to the criteria above on 2026-10-10: new 10a (subject line, open design question 3), 14 (glyph and name on the items), 17 (the "×" sign, the size as a press would set it). Criteria 9 and 16 are unchanged ("roving, arrows press" is built as written).

## Delivery

Built as the last milestone of the `story/style-panel-rework` branch of `0017-style-panel-rework`, and delivered in that one PR (customer decision: the "bigger PR"). It builds on 0017's final panel and does not run in parallel with it. Milestones are in `adrs.md`: (1) data file and loader in `curvyo-document-core`, criteria 1 to 8; (2) `ui-core` view, session glue and bindings, criteria 10, 10a, 11 to 14, 18 to 21; (3) frontend, criteria 9, 15 to 17. The UX review and the tester pass run once on the whole 0017 slice.

## Links

Requirements: R-EDIT-019 (`docs/requirements.md`)
Builds on: `specs/0015-document-size-and-rulers/` (Document section, resize around the centre, `resize_document`, display unit, limits), `specs/0017-style-panel-rework/` (panel rule: no popups; criteria 3, 55, 57)
Related: `docs/adr/0002-document-model-units-and-svg-round-trip.md` (mm, 96 dpi at the SVG boundary)
ADRs: `adrs.md` (architect, 2026-10-10)
PR: TBD (part of the 0017 PR)
