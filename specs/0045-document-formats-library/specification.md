# Document formats library: formats, groups and favourites the maker can change

Status: Ready for the rectangular part (criteria 1 to 34; `adrs.md` and the UX notes exist). Where the maker's formats are stored is decided: a per-user file `document-formats.toml` in the data directory (customer, 2026-10-10, Question 1). Shapes are `0046-document-shapes`. Build after `0040` and `0043` merge (milestone 1, the loader in `document-core`, may start earlier, `adrs.md`).
Priority: Should
Origin: Customer (request of 2026-10-10, final: the format list must be customisable, built-in defaults with A0 to A6 on, groups, favourites with a star, a full list inline, a way to add custom formats, an inline add and save, import for sharing is the product owner's addition). The place where user formats are stored was my proposal and is decided (Question 1). Follow-up of the merged `0030-document-size-presets`. Non-rectangular formats (circle, outline from a file) are `0046-document-shapes`.

## User value

As a maker who lasers key rings, breakfast boards and coasters, or embroiders in a hoop, I want to keep my own document formats next to A0 to A6, group them ("Laser", "Embroidery"), star the few I use every day so they sit one click away, and share the list with a colleague, so that a new document takes one click instead of typing the same millimetres again, and the product does not need a release for every material or hoop.

**What the reference tools do, and what we do differently.** Inkscape's Document Properties has a fixed page-size list in a drop-down plus a custom size; its templates are separate files. Illustrator and Affinity have a New Document dialog with preset groups and "save preset". LightBurn takes the work area from the machine, not from a document format. Figma keeps frame presets in a menu. What we do:

- The list is data, not code. Built-in formats ship in the existing TOML file; the maker's formats are a second file of the same shape, laid over it.
- **Favourites decide what is in the quick selection.** The strips of `0030` become the favourites: one click, always visible. The rest is one disclosure away.
- **No popup.** The customer asked for a drop-down. The rule "no popups in the properties panel" (customer, final, 2026-10-08) wins, and the customer accepted the inline version ("just activate it inside the sidebar"): the full list is an inline list that expands inside the panel, and adding a format is an inline form with an Add or Save button. The OS file dialog for import and export is the only layer, as for Open and Save.
- Groups can be switched off: A0 to A6 are on by default, Slides are present and off. A maker who never makes slides never sees them.

## Words used below

- **Format:** a named document size (called a preset in `0030` and in the code). Later also a shape (`0046`).
- **Group:** a heading with formats and a default orientation. Built-in groups: Paper, Slides. User groups: made by the maker.
- **Favourite:** a format the maker starred. The quick selection shows the favourites of the groups that are on.
- **Quick selection:** the strips of `0030`, now filled from the favourites.
- **Built-in file:** `curvyo-document-core/data/document-presets.toml`, compiled in.
- **User file:** `document-formats.toml`, the maker's formats and choices (criterion 29 for where).

## Acceptance criteria

Sizes in millimetres unless a criterion names a unit. The rules of `0030` (orientation, selected state, resize around the centre, tolerance 0.01 mm) are unchanged and are named, not repeated.

### Part A: the data

1. Given the built-in file, then its schema `format` is **2**: a group may carry `enabled` (true or false, default true) and a preset may carry `favourite` (default true), and a size `unit` may be `mm`, `cm`, `in` or `px` (cm is `value * 10`). The shipped file has Paper `enabled = true`, Slides `enabled = false`, every preset a favourite. A reader refuses any other `format` value. Test: the shipped file loads; Paper is on with 7 favourites, Slides off with 3. (A data schema, not the project file: the project's `format_version` does not change, criterion 28.)
2. Given a user file, then it is TOML with the same blocks as the built-in file plus two top-level keys:

   ```toml
   format = 1
   favourites = ["a4", "a3", "u-coaster-90"]   # ids; absent = the defaults of the built-in file
   enabled = { slides = true }                  # group id -> on or off; absent = the built-in value

   [[group]]
   id = "u-laser"                               # a new group: id, name, default_orientation
   name = "Laser"
   default_orientation = "landscape"

     [[group.preset]]
     id = "u-keyring"
     name = "Key ring"
     short_side = 30
     long_side = 50
     unit = "mm"
   ```

3. Given the library loader, then it takes the built-in text and the optional user text and returns the validated library or an error naming the id and the reason. It opens no file, reads no clock, starts no thread, and builds for `wasm32-unknown-unknown` (`CLAUDE.md` §6). Reading and writing the user file is a platform crate's job (criterion 29).
4. **Overlay rules.** (a) A user `[[group]]` whose `id` is a built-in group's appends its presets after that group's built-in presets; a `name` or `default_orientation` given there must equal the built-in's, otherwise the loader returns an error. (b) A user group with a new id comes after all built-in groups, in file order. (c) A user preset id must differ from every other id in the library, built-in or user. (d) The built-in groups and presets cannot be changed or removed by the user file. (e) `favourites` and `enabled` may name ids that do not exist (a group the maker deleted, a file from a colleague); those entries are ignored without error. Structure errors are strict, state entries lenient.
5. **Validation.** Every rule of `0030` criterion 6 applies to user groups and presets: valid TOML with no unknown key; ids of lower-case letters, digits and `-`; names 1 to 24 characters after trimming; notes at most 60; both sides finite, positive, short side not above long side, converted sides from 1 to 100 000 mm; no two formats with the same size (each side within 0.01 mm) anywhere in the library. Added limits: at most 200 user formats, at most 20 user groups, a user file of at most 256 KiB. Each rule has a failing fixture and a test. (Same-size duplicates are refused so that the selected state of `0030` criterion 10 stays unique; Question 4.)
6. Given a user file that fails to load, then the app starts with the built-in formats only, the Document section shows the line "Your formats file could not be read: <reason>. Built-in formats are shown." with a button "Set file aside", and the file is **not changed or deleted**. The line and the button are a block at the top of the format area; it is persistent (not a live region) and the button is a normal Tab stop. While the file is broken, Add, Edit, Delete, Import, Export, the stars and the "Show" switches (they are writes too) are not in the tree, so nothing can overwrite it; every edit is also refused in the core. The list shows the built-in formats read-only. "Set file aside" renames it to `document-formats.toml.broken` (replacing an older one), the block leaves, the notice "Moved to document-formats.toml.broken." shows for 3 seconds, and the library then runs on the built-ins.
7. Given the shipped built-in file, then a test in the default `cargo nextest run` loads it and requires success (as `0030` criterion 7).

### Part B: the quick selection (favourites)

8. Given the default settings, then the Document section shows exactly what `0030` showed for Paper: the heading "Paper" and the strip A0 to A6, and no Slides strip. Test: view built from the library and no user file equals `0030`'s Paper block.
9. Given a library, then the quick selection shows, for each group that is on and has at least one favourite, the group's heading and a strip of its favourite formats in list order. The strip rules of `0030` (cell minimum 32 px, wrap to further rows, never truncated, pressed look, tooltip) are unchanged. A group that is off, or has no favourite, draws nothing. With no favourite at all only the Orientation group and the "All formats" row remain.
10. Given a press on a quick-selection cell, then the document is resized exactly as `0030` criteria 11 to 13 say (centre fixed, one `resize_document` commit, orientation by the group rule, nothing written when the size is already the same).
11. Given any document size, then the **subject line** of the header (`0030` criterion 10a) names the first format, in list order, of any group that is on, that has the document's size in either orientation, and "Custom" when none has. A cell is shown pressed only when that format is in the quick selection. Test: Slides off, document 508 × 285.75: "Custom". A3 not a favourite, document 297 × 420: the subject line says "A3, portrait" and no cell is pressed.

### Part C: the full list, inline

12. Given the Document tab (`0043`), then a row "All formats" is a disclosure (`aria-expanded`, `aria-controls`) directly under the Orientation group (not between the quick-selection strips and Orientation, so the strips and Orientation stay one compact block), collapsed at every start of the app. Pressing it expands the list in the panel's own column and scrolls the panel body so that the row is at its top. The fold state of the groups is kept for the session. No element with role `dialog`, `listbox` or `menu` exists, nothing opens over the canvas or other panel content (`0017` criterion 55). The panel scrolls as a whole; the list has no inner scroll area.
13. Given the expanded list, then it shows every group in library order, each with a header row: a disclosure to fold its rows (its accessible name includes the group name), the group name, a count of its formats (for a group that is off, the word "Off" instead), for a user group an edit and a delete button, and at the right a switch with the visible label "Show" and the accessible name "Show group <name>" that turns the group on or off. A group that is on starts unfolded; one that is off shows its header only. Each format row shows the name, the size in the display unit as `0030` criterion 17 writes it ("210 × 297 mm"), and a star toggle (`aria-pressed`). User formats show an edit and a delete button as well; built-in formats do not.
14. Given a press on a format's name or size in the list, then it is applied exactly like a quick-selection cell (criterion 10), in the orientation rule of its group, and the pressed state follows criterion 11.
15. Given a press on a star, then the format is added to or removed from the favourites; the quick selection changes in the same frame; the user file is written (criterion 29). The star on a format of a group that is off is not in the tree.
16. Given a press on a group's "Show" switch, then the group is turned on or off, the quick selection and the subject line change in the same frame, and the user file is written. A group that is turned on again has the same favourites as before. All groups may be turned off.
17. Given the keyboard, then the disclosure, each group header, each row's apply control, each star, edit and delete button, and the switch are reachable; arrow keys move between rows (roving focus, one Tab stop for the list body, the `ToggleGroup` rule of the design system); Left and Right move between the controls of a row (a format row: star, apply, edit, delete; a header: fold, edit, delete, Show). Space or Enter presses; on the switch only Space toggles. Add format, Import and Export are separate Tab stops after the list. Accessible names: "Apply A4, 210 × 297 mm"; the star has the **constant** name "Quick selection: A4" and the state in `aria-pressed` (a name that flips between "Add" and "Remove" together with `aria-pressed` is announced twice), with the tooltip "Show in the quick selection" or "Remove from the quick selection"; "Show group Slides", "Edit Key ring", "Delete Key ring".

### Part D: add, edit and delete, inline

18. Given the expanded list, then its last row is a button "Add format". Pressing it opens an inline form in place of the button (the same column, no popup) with: **Name** (text), **Width** and **Height** (text fields), **Unit** (a segmented group mm, cm, in, px), **Group** (a wrapped strip of radio cells with the names of the user groups and the built-in groups and a last cell "New group…", one Tab stop, in the look and wrapping rule of the preset strips; a radio list would be 28 px per group), a switch "Add to quick selection" (on by default), and the buttons **Add** and **Cancel**. The form is one block with a 1 px border. **Add** (and **Save** in criterion 21) use the primary button look, filled with the accent colour and a white label; Cancel is an outline button. The form opens prefilled with the document's current size in the display unit (the document unit as the field's unit; px is never prefilled), an empty name, the group last used (else the first user group, else "New group…"), and keyboard focus on Name.
19. Given "New group…" chosen, then two more fields appear: **Group name** (1 to 24 characters, unique ignoring case) and **Opens as** (Portrait or Landscape, the group's `default_orientation`). A format has no orientation of its own; the group decides (`0030` criterion 13).
20. Given a press on Add (or Enter in a field), then the form is validated: an empty or longer-than-24 name "Enter a name of 1 to 24 characters"; a size that is not a number, or whose converted side is below 1 mm or above 100 000 mm "Enter a number from 1 to 100000 mm" (the limit written in the chosen unit); a size already in the library "Same size as <name>"; a name already used in the group "This group already has <name>"; a limit of criterion 5 "The list is full (200 formats)". An invalid field gets the 2 px invalid border and `aria-invalid`; only the chip of the first invalid field shows, and focus goes to that field; the other fields show their chip when they get focus. Nothing is written. A valid form creates the format (sides stored short and long with the unit as typed), writes the file, closes the form, and the notice says "Added <name>." The new format is in the list; it is in the quick selection when its star is on. The document is not changed.
21. Given a user format, when the maker presses its edit button, then the form opens in the row's place with the format's values and the buttons **Save** and **Cancel**. Save validates as criterion 20 (the format's own old size is not a duplicate) and keeps the id and the favourite state. Editing a format never changes the document, even if the document has that size.
22. Given a user format, when the maker presses its delete button, then the row is replaced in place by the line "Delete <name>?" with the buttons **Cancel** (left) and **Delete** (right); keyboard focus goes to Cancel, Delete ignores presses for the first 400 ms, and Escape cancels. Delete removes it, removes its id from the favourites, writes the file. The document is not changed (it stores millimetres only, `0030` criteria 19 to 21). A user group has the same buttons on its header: "Delete group <name> and its N formats?". A built-in group or format has neither: it can be turned off (criterion 16) or unstarred (criterion 15).
23. Given an open form, when Escape is pressed in it or Cancel is pressed, then the form closes and nothing is written. Given the Document tab is hidden while a form is open (an object is selected, the panel is collapsed, another tab is shown, `0043`), then the draft is kept, nothing is written, and the draft returns with the tab; New and Open drop it (the same rule as `0043` criterion 17).
24. Given a user group, then its header has an edit button that renames it (1 to 24 characters, unique) and sets "Opens as", with Save and Cancel as in criterion 21.

### Part E: share the list

25. Given the expanded list, then below the Add format button come two full-width text buttons, one under the other, **Import formats** and **Export my formats** (the two labels do not fit side by side in 244 px). Export is not in the tree while there is no user format. Both use the OS file dialog, which is the only layer (as Open and Save). The notices of criteria 20 and 27 ("Added <name>.", "Imported 5 formats in 2 groups. Skipped 2 that were already there.", and the refusal "Could not import <file>: <reason>. Nothing was changed.") appear 4 px under the pressed button; the long one lasts 5 seconds, the refusal 8 seconds.
26. Given Export, then the file written is TOML in the user-file schema holding the user groups, their formats, and a `favourites` list limited to the exported ids. It holds no `enabled` table and nothing built-in. Default file name `curvyo-formats.toml`.
27. Given Import of a file, then it is validated as a user file. A file that fails is refused whole with the named error and nothing changes. A valid file is merged: a format with an id already in the library and the same content is skipped; the same id with other content, or a clash with a built-in id, is added with a new generated id; a format whose size already exists is skipped; a group with the id or the name of an existing user group joins that group; the imported ids in `favourites` become favourites. The notice says "Imported 5 formats in 2 groups. Skipped 2 that were already there." Built-in groups and the `enabled` choices are not changed by an import.

### Part F: where it lives

28. Given the user file, then it is **not in the project file**. A project stores only its size in millimetres, `format_version` does not change, a project does not know which format it came from, and `0030` criteria 19 to 21 hold with a library of any content. Test: save a project after picking a user format, delete the format, reopen: the same size, no error, "Custom" in the subject line.
29. Given the app, then the user file is read at start by the platform layer (`curvyo-storage-io` does the file access, `curvyo-app` has commands that move text only, `adrs.md` decision 5) and handed to the library loader as text, and written with an atomic write (a temporary file in the same folder, then a rename; no lock files, as ADR 0004 §7). A missing file means the built-in defaults; it is created at the first change. The file is `document-formats.toml` in the data directory of ADR 0004 §7 (until that directory is configurable: the app data directory of the platform); Question 1. The file is read before the panel first renders, so the quick selection never shows the defaults for a frame when the user file changes them.
30. Given a change by another program while the app runs (a sync tool, a text editor), then it is read at the next start. A write by the app replaces the whole file with the in-memory library; the last writer wins. Comments a maker typed into the file by hand are therefore lost on the next change; the built-in file keeps its comments. Given a write that fails, then a notice says so and the in-memory library stays as it is.
31. Given the browser build (the public demo), then nothing in the core crates changes: the platform layer keeps the file text in browser local storage under `curvyo.document-formats` (every access guarded; a failure means the built-in formats only), Import is a file picker and Export a download, as Open and Save are today. (Built here, about ten lines in `frontend/src/platform/host.ts`, `adrs.md` decision 5.)

### Part F, continued: ids, the file text and the favourites rule

32. Given a built-in format or group, then its id never starts with `u-` (a test on the shipped file). Given a format or group the maker adds or an import brings in, then its id is `u-` plus a slug of its name, with `-2`, `-3` on a collision; the id is derived from the name and the ids already in the library only, with no clock and no random number, so the same input gives the same id.
33. Given the user file written by the app, then it is in a canonical order (`favourites`, `enabled`, then groups in creation order), so reading and writing it again gives the same text. A file whose `format` is not 1 is refused with "made by a newer version" and never partially read. Golden fixtures in `tests/fixtures/` cover one valid file, one file per failing rule, the round trip, the export and the import merge (`CLAUDE.md` §5).
34. Given a favourite state, then a user file without a `favourites` key uses the `favourite` flags of the built-in file; once the key exists it is the whole list of favourites (criterion 2).

### Part G: what changes in `0030`

| `0030` | What happens |
|---|---|
| Criterion 1 and the data file | Schema 2 (criterion 1). Slides stay in the file, off by default |
| 6c "a group has no preset" | Applies to built-in groups only; a user group may become empty |
| 6h "no two presets alike" | Now across built-in and user formats (criterion 5) |
| 9, 17 (the strips) | The strips show the favourites (criterion 9); Paper looks as before by default |
| 10, 10a (selected state, subject line) | Over all formats of groups that are on (criterion 11) |
| 11 to 16, 18 | Unchanged |
| Data file text "User-defined presets later" | Done here |

## Out of scope

- Shapes (circle, rounded rectangle, an outline from SVG or DXF): `0046-document-shapes`. A format here is a rectangle.
- A search or filter field for a long list. A Proposal for later; with 200 formats the groups can be folded.
- Reordering formats or groups; user groups are in creation order, formats in creation order within a group.
- Hiding a single built-in format; renaming or editing built-ins.
- Formats per project, formats inside the project file, a project remembering its format.
- Cloud or account sync of the user file. Folder sync works because the file sits in the data directory (ADR 0004 §7); detection of conflict copies is ADR 0004's.
- Linking a format to a machine's work area or a material record (`machine-profile`, R-MAT); thumbnails; margins, bleed.
- Importing the page-size lists or templates of other tools.
- A New Document dialog; New still creates A4 portrait (`0001`).
- Translated names.
- Undo of a library change: the library is the maker's settings, not the document; criterion 22 asks first instead. (`0020` covers document steps only.)

## Open questions (customer; each has a default)

1. **Where do the maker's formats live? Decided by the customer on 2026-10-10: A, a per-user file.**
   *A (default, recommended):* in a per-user file `document-formats.toml` in the **data directory** of ADR 0004 (the folder that also holds the machine and material records, configurable, folder-synced, cloud-synced later). One list for every project on this computer; a collaborator does not need it, because a project stores only its size. Core parses and validates; a platform crate reads and writes. It is a settings file: the browser build keeps it in browser storage later.
   *B:* inside the project file. The list travels with the project and a team sees the same formats, but every project has its own list and its own stars, the document model and the CRDT carry settings that are not part of the drawing, and a new project starts with an empty list.
   *C:* both (a user file plus a copy in every project). Rejected as speculative.
   Recommendation A. Decided: A. Caveat (customer, 2026-10-10): this holds for now and may change when collaboration arrives. A list shared by a team, or one that travels with a shared project, would point to B or to a team-level list. The loader in `document-core` takes the user text as an argument, so another home changes the platform layer only.
2. **Which first-party formats ship beyond A0 to A6 and the three slides?** The customer named key rings, breakfast boards, cork and slate coasters, leather labels and an embroidery hoop. *Default:* none; the maker adds his own, and no size is invented here. *B:* the customer sends his real list (for example hoop diameters, coaster sizes) and each becomes one block in the built-in file, in new built-in groups ("Laser", "Embroidery"); circle shapes wait for `0046`.
3. **Slides.** *A (default):* the group is present and off, its three formats are favourites, so turning it on brings back the `0030` strip. *B:* Slides on by default as in `0030`. The customer said A0 to A6 stay on by default; the slides were left open.
4. **Same size twice (criterion 5).** *A (default):* refused with "Same size as <name>", so the selected state stays unique. *B:* allowed; the first in list order is shown as selected. Recommendation A; revisit when shapes exist (a circle and a square of one bounding box are different formats, `0046`).
5. **Format of the shared file.** *A (default):* TOML, the same as the built-in file (comments, hand editing). *B:* also JSON. The customer wrote "JSON/TOML"; one format is less to test. Recommendation A.
6. **A new format is a favourite (criterion 18).** *A (default):* the star is on in the form. *B:* off.
7. **A broken user file (criterion 6).** *A (default):* built-ins plus a notice and "Set file aside". *B:* the app refuses to start the Document section. Recommendation A.

## UX notes

Written by the ux-engineer, 2026-10-10. Numbers and component rules: `docs/design-system.md`, "Document formats library", "List row", "Inline confirm", "Properties panel: Document section" (row "Presets" rewritten). The list is in the **Document tab** (`0043`); there is no popup and no `dialog`, `listbox` or `menu` role.

### Order of the Document tab (top to bottom)

1. The broken-file block (only while the user file cannot be read).
2. Quick selection: per group that is on and has a favourite, heading plus strip (`0030`, unchanged: Paper A0 to A6 by default).
3. Orientation.
4. **"All formats"** disclosure row (28 px, 8 px gap above) and, expanded, the list.
5. Rule, Width, Height, Unit, Fit to content (and "Save as format", `0046`), Background.

The disclosure sits under Orientation, not between the strips and Orientation: the quick block (strips plus Orientation) stays one compact unit, and the list is the "more" below it. The subject line in the fixed strip row ("A3, portrait") shows what a press in the list did even when the quick selection is scrolled away. The default tab is as tall as `0030`'s: Slides (60 px) are off and the disclosure row (36 px) comes in.

### The disclosure and the list

| Part | Rule |
|---|---|
| Disclosure row | 244 × 28, chevron 12 px, label "All formats" 14 px, muted count "14 formats" at the right (formats in groups that are on), `aria-expanded`, `aria-controls`. Collapsed at every start. Expanding scrolls the panel body so the row is at the top (with the body's 12 px scroll padding); the list is in the flow of the panel scroll, no inner scroll area (a long library is folded by group) |
| Group header row | 28 px: fold chevron button (28 × 28, the group name is part of its accessible name), name 14 px semibold with ellipsis (tooltip shows the full name), muted count ("7"; for a group that is off the word "Off" instead), then for user groups an edit and a delete icon button (24 px, `Pencil`, `Trash2`), then at the right the **Show** switch (the `Switch` of the design system: label "Show" 14 px at the left of a 32 × 18 track, accessible name "Show group Slides"). Built-in groups have no edit or delete. A group that is off shows its header only; a group that is on starts unfolded, the fold state is kept for the session |
| Format row | 28 px: star toggle (28 × 28 hit, `Star` 16 px, outline when off, filled `--toolbar-icon` when on: shape, not colour), then the apply button (name 14 px with ellipsis, size 12 px tabular `--panel-muted-fg` right-aligned, "210 × 297 mm" in the display unit as `0030`), then for user formats an edit and a delete icon button (24 px, reserved width 52 px so nothing shifts). Hover `--editor-accent-hover`; the row whose format matches the document uses the **selected row look** (`--value-fill` ground, 3 px `--accent` bar), also when the format is not a favourite, so the list shows the current state; focus ring 2 px inside the row |
| Star | `aria-pressed`, constant accessible name "Quick selection: A4" (a name that flips between "Add" and "Remove" together with `aria-pressed` is announced twice); tooltip "Show in the quick selection" / "Remove from the quick selection". Not in the tree for a format of a group that is off |
| Order and spacing | Library order, 4 px between groups. Between the last row and the buttons below, 8 px |
| Rows below the list | **Add format** (244 × 28 outline button). Under it, stacked at full width, **Import formats** and **Export my formats** (28 px each, 8 px apart; the two labels together are 20 px wider than 244 px at 14 px, so they do not share a row). Export is not in the tree while there is no user format |

### Add and Edit, inline

Pressing **Add format** replaces the button by the form in the same place; Edit replaces the format's row by the same form. The form is a block with a 1 px `--toolbar-icon` at 25 % border, `rounded-[5px]`, 8 px padding (226 px inside), the Style grid (label 60 px, 8 px, control 158 px), 8 px between rows.

| Row | Control |
|---|---|
| Name | Text field, 158 px |
| Width, Height | Two text fields (one row each, so the labels stay visible while typing), 158 px, right-aligned tabular text, the unit as the fixed muted suffix inside the right edge, `inputmode="decimal"` |
| Unit | `ToggleGroup` with text items "mm", "cm", "in", "px", 36 px each |
| Group | A 16 px heading "Group", then a **wrapped radio strip** of the group names plus the last cell "New group…", the look and wrapping rule of the preset strip (cells grow, minimum 32 px, never truncated, one Tab stop). A radio list would be 28 px per group, 560 px for 20 groups |
| New group | Appears only for "New group…": "Group name" (text field) and "Opens as" (`ToggleGroup`, Portrait and Landscape with the page glyphs of Orientation) |
| Quick selection | The `Switch` row, label "Add to quick selection", on by default |
| Buttons | **Add** (or **Save**) and **Cancel**, 109 px each, 8 px apart. Add and Save use the **primary button** look (`--toolbar-icon-active-bg` ground, white label); Cancel is an outline button. Enter in a field presses Add |

Opening: prefilled as criterion 18, keyboard focus in Name, the form scrolled into view (its top at the top of the body; the form is about 290 px high, 360 px with a new group). Validation: the **validation chip** (244 px, below the field, right-aligned, no row moves) with the texts of criterion 20. Only the chip of the first invalid field shows, and focus goes to that field; the others get the 2 px `--field-invalid` border and `aria-invalid`, and show their chip when focused. Success: the form closes, focus returns to the "Add format" button (Edit: to the row's apply button), the notice "Added Key ring." 4 px under the button.

Escape or Cancel closes the form and drops the draft. A **change of tab** (by the maker, or because the selection became non-empty) keeps the draft: it is not written, and it returns when the Document tab is shown again; New and Open drop it. A user who is typing a name and presses Ctrl+A on the canvas should not lose it.

### Delete and group edit, inline

- **Delete a format:** the row is replaced by a block: the line "Delete Key ring?" (14 px) and **Cancel** and **Delete**, 109 px each (the "Inline confirm" pattern of `0020`: focus on Cancel, the confirm ignores presses for 400 ms, Escape cancels). A user group: "Delete group Laser and its 2 formats?".
- **Edit a group:** the header is replaced by a small form: Group name, Opens as, **Save**, **Cancel**.
- Nothing here changes the document (criterion 22).

### The broken-file block

At the top of the format area, only while the user file cannot be read: 8 px padding, 1 px `--field-invalid` border, a 12 px alert glyph, 12 px `--field-invalid` text "Your formats file could not be read: <reason>. Built-in formats are shown." (up to five lines), then a 28 px outline button **Set file aside**. Persistent, not a live region; the button is a normal Tab stop. While it is shown the stars, edit, delete, Add, Import and Export are not in the tree; the list shows the built-ins read-only (the Show switches are writes too, so they are hidden as well). After the button, the block leaves and the notice "Moved to document-formats.toml.broken." shows for 3 s.

### Keyboard

The list body is **one Tab stop** with roving focus over the disclosure's content: Up and Down move between group headers and format rows, Home and End jump, Left and Right move between the controls of a row (header: fold, edit, delete, Show; format: star, apply, edit, delete). Enter or Space presses (on the switch Space toggles, Enter does not). Add format, Import and Export are separate Tab stops after the list. Accessible names: "Apply A4, 210 × 297 mm", "Quick selection: A4", "Show group Slides", "Edit Key ring", "Delete Key ring". A mouse press on a row returns focus to the canvas; keyboard use does not.

### What the maker sees for production work

The library knows no machines. A group is the maker's own label: "Laser" with "Key ring 30 × 50 mm", "Coaster 90 × 90 mm"; "Embroidery" with hoops. A group with favourites shows as its own strip in the quick selection, with its name as heading, so a laser maker opens a document in one click. Round hoops come with `0046` (circle glyph, "⌀ 130 mm"). Linking a format to a machine bed or a hoop safe margin is a later spec and will add a small tag to the row; no space is reserved for it now.

### Criteria changes requested and applied

All changes the ux-engineer requested (criteria 6, 12, 13, 17, 18, 20, 22, 23, 25) were applied to the criteria above by the product owner on 2026-10-10.

### Design questions: decided (2026-10-10)

1. "All formats" directly under Orientation: decided (criterion 12).
2. The primary button look (accent ground) for Add and Save in these inline forms: decided yes (criterion 18). It is new to the panel and appears only here; an outline Add would make the one button that writes look like Cancel.
3. The draft is kept across tab changes instead of cancelled: decided yes (criterion 23).

## Links

Requirements: R-EDIT-027
Builds on: `specs/0030-document-size-presets/`, `specs/0015-document-size-and-rulers/`, `specs/0017-style-panel-rework/` (panel rule), ADR 0004 §6, §7 (data directory, atomic writes, folder sync)
Follow-up: `specs/0046-document-shapes/`
Panel placement: `specs/0043-properties-tabs/` (the Document tab)
ADRs: `adrs.md` (architect): no new ADR; one new written file format (the user file, `format = 1`) and the `toml` writer feature; where the file lives is decided (Question 1, A)
PR: -
