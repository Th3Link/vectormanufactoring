# Properties panel tabs: Document, Style, History

Status: Ready (the criteria are complete and testable; `adrs.md` confirms that no new ADR is needed; the UX notes are in; one branch `story/properties-tabs`, one PR, can start from `main`, `CLAUDE.md` §4)
Priority: Should (it is the first thing `0020-undo-redo` builds on: the History tab needs it, so it is built first)
Origin: Customer (request of 2026-10-10, final: the right side needs microtabs like Blender's; it was already "rather sexy" that the right context, Document or Style, had to be chosen, and now there are Style and History). The strip's orientation, the auto-switch rules and the details are my proposals and are marked.

## User value

As a maker I want the right panel to show one context at a time with a small strip of icon tabs (Document, Style, History) so that I can see which context I am in, switch with one click or key, and reach the document settings while an object is selected, which today needs Escape first.

**Reference tools.** Blender's Properties editor has a column of icon-only tabs at the editor's left edge, with a tooltip each, and shows only the tabs that apply to the selected object; Ctrl+Wheel over the strip steps through them. Inkscape's docked dialogs sit in a notebook with icon-and-label tabs on top. Affinity's Studio panels are tabs with icons. LightBurn has dockable windows with no tabs. What we do: three tabs, icons with text tooltips, a strip that never changes size or order, and a rule for when the panel switches by itself and when it leaves the maker's choice alone.

**Design-system rule this replaces.** `docs/design-system.md` says "one scrolling panel of stacked sections, not tabs ... add tabs only once a second section makes a single scroll genuinely unwieldy". The customer has decided; the ux-engineer rewrites that paragraph.

## Words used below

- **Tab:** one of Document, Style, History; the panel shows the content of the **active tab**.
- **Strip:** the row of tab icons.
- **Selection state:** empty (no object selected) or not empty. A selection of any size is "not empty". Precisely, it is whether the active tool's style scope holds objects (`adrs.md` decision 2): in the Select tool the selected objects, in the Node tool the edited path; switching to the Pen with a selection is a change from not empty to empty.
- **Manual choice:** the maker pressed a tab (mouse, keyboard or a shortcut).

## Acceptance criteria

### The tabs

1. Given the Properties panel, then it has a strip with three tabs in this order: **Document**, **Style**, **History**. Each tab shows an icon (a page, a droplet, a clock with a return arrow; not a brush, which belongs to `0033`) and no visible text. The strip sits in the panel's header row, in place of the title word (criterion 20). Each tab has a hit area of 36 × 28 px, an accessible name ("Document", "Style", "History") and a text-only tooltip after 400 ms and on keyboard focus: "Document (Shift+Ctrl+D)", "Style (Shift+Ctrl+F)", "History (Shift+Ctrl+H)". Until `0020` exists the History tab is not in the tree (a tab with nothing behind it is not built, `CLAUDE.md` §5); the strip then has two tabs and the order is unchanged.
2. Given the active tab, then it is shown pressed by a shape (a bar or fill under or behind the icon), not by colour alone, and the other tabs are not. Exactly one tab is active at any time.
3. Given the Document tab active, then the panel body shows the Document section exactly as `0015`, `0030` and `0040` define it; with the Style tab the Style section as `0017` and `0018` define it, with its subject line and rows (the title word is replaced by the strip). History shows what `0020` defines. Nothing inside a section changes in this slice.
4. Given the Style tab active and **nothing selected**, then the tab is shown **dimmed** and pressing it does nothing and gives no notice (it is exempt from the 3:1 non-text rule while dimmed, like the dimmed toolbox buttons). It has `aria-disabled="true"` and is **not focusable** (`tabindex="-1"`), so it is announced as unavailable in the tablist but is never a keyboard stop. Its tooltip says "Style: select an object first"; the tooltip is for the pointer only. The Style body is never shown empty by this rule: the active tab is not Style while the selection is empty (criterion 7). (Question 3 for the alternative of hiding the tab.)

### Which tab is active, and when it switches by itself

5. Given a new session (New, Open, start), then the active tab is **Document** when the selection is empty and **Style** when it is not.
6. Given the selection going from empty to not empty while the active tab is **Document**, then the active tab becomes **Style**. Given it going from not empty to empty while the active tab is **Style**, then the active tab becomes **Document**. This is the behaviour of today's panel.
7. Given the selection going from not empty to empty while the active tab is **Style**, then Document becomes active (criterion 6); given a selection change that stays not empty (one object to another, to several), then the active tab does not change, whatever it is.
8. **Sticky choices.** Given the active tab is **History**, then the panel never switches by itself, whatever the selection does. Given the maker pressed **Document** while the selection is not empty, then Document stays active through selection changes between non-empty selections, and becomes Style when the selection goes empty and then non-empty again (criterion 6 applies at the boundary). Given the maker pressed **Style** while the selection is empty, nothing happens (criterion 4).
9. Given the selection changing while the maker holds a field with edited text, then the active tab changes only after that field's own press-elsewhere rule has run (`0015` criterion 15, `0017` text field rule), as a change of section does today.
10. Given the Pen with an unfinished path, then the strip stays as it is, the Document and Style bodies are empty as `0015` and `0017` say, and a press on a tab sets the active tab; the History body (once it exists) is shown normally, read-only. A press on a row of the History list during an unfinished Pen path shows the hint "Finish the path (Enter) or cancel it (Esc) first." (the text of `0020` criterion 6e). The selection rule of the Pen does not change. (Today the panel is empty; the strip now stays so the maker can see where the panel will come back.)
11. Given a manual choice, then it is session state only: not saved in the project, not in any file, reset by New and Open (criterion 5), kept when the panel is collapsed and expanded and when the window is resized.

### Keys, focus, shortcuts

12. Given the keyboard, then the strip is one Tab stop (the active tab, or the first when none is active) with roving focus: Left and Right move to the previous or next tab and **activate it** (the content is instant, so no separate Enter), Home and End jump to the first and last. The strip has `role="tablist"` named "Panel", the tabs `role="tab"` with `aria-selected` and `aria-controls`, the body `role="tabpanel"` with `aria-labelledby`. A dimmed Style tab is skipped by the arrows, has `aria-disabled="true"` and `tabindex="-1"`, and is never the Tab stop (criterion 4). Tab order inside the panel: the collapse tab, the strip, the body (`0017`).
13. Given a press on a tab with the mouse, then keyboard focus goes to the canvas, like every other panel mouse action (`0017` criterion 57); a press with the keyboard keeps the focus on the strip.
14. Given **Shift+Ctrl+F** (Shift+Cmd+F on macOS), then the panel expands if it is collapsed and the active tab becomes Style when the selection is not empty and Document when it is empty, and focus moves to the first control as `0015` and `0017` say. It does not switch away from History when History is active and the key is pressed with a selection; **Proposal:** in that case it activates Style. (Question 4.) Given **Shift+Ctrl+D** (Shift+Cmd+D on macOS), then the panel expands if it is collapsed, Document becomes active, and focus moves to its first control (Width when nothing else is open); it is Inkscape's key for Document Properties, and it is ignored during a canvas drag.
15. **Proposal (with `0020`):** Given **Shift+Ctrl+H** (Shift+Cmd+H), then the panel expands if collapsed, History becomes active and focus moves to its list. This is Inkscape's key for its Undo History. Ignored during a canvas drag.
16. Given a running eyedropper pick (`0017`, `0040`), then pressing a tab cancels the pick, like Escape, and then the tab is activated. Given a colour block, a hex field or the Pattern field with edited text, the press follows that field's own press-elsewhere rule.
17. Given an inline form is open in the Document tab (the Add format form of `0045`, or any other) and the tab changes, by the maker or because the selection changed, then its draft is kept for the session and nothing is written; the draft returns when the Document tab is shown again; New and Open drop it. (A maker who is typing a name and presses Ctrl+A on the canvas does not lose it.)

### Size and layout

18. Given the panel, then its width stays **280 px**, its content width **244 px**, and the canvas region does not change width when the strip is added, when tabs change, or when the active tab changes. At the minimum window 800 × 600 the canvas region is still 520 px wide. The strip is not part of the scrolling body: it stays in view while the body scrolls. The strip does not scroll; the History tab fills the panel height and its list is the one scroll area (`0020` criterion 43); every other tab scrolls as a whole.
19. Given any change of tab or selection, then no tab icon moves, appears or disappears, the strip's height is constant, and the body's scroll position resets to the top (as `0040` says for a changed section). Switching is instant: no animation, no fade.
20. **Strip height.** The strip is horizontal and sits in the header row, in place of the title word (Question 1, decided: option C, refined). The header row becomes 28 px high where it was 24 px, so the net cost is **4 px** of the panel's height; the subject line stays in the row, right-aligned, in the remaining 120 px. At 800 × 600 this moves the Document tab's Background block (`0040`) down by 4 px; the ux-engineer rewrites the statement of the design system "Background scroll" with the new numbers. Nothing becomes unreachable: the panel scrolls as a whole. The fallback, if the customer wants a visible word back, is the strip in a row of its own (32 to 36 px).
21. Hit target of a tab: at least 28 × 28 px; icon size 16 to 20 px; the icon against the panel at least 3:1; the pressed state against the unpressed at least 3:1; focus ring 2 px as the panel's.
22. Given the panel collapsed (0 px, `0017`), then the strip is not shown; the collapse tab alone is. Given it expanded again, then the previous active tab is active.

### Notes on what the tabs replace

23. Given the "empty panel" rules of `0017` criterion 1 and `0015`, then they hold as follows: the panel's width never changes; the Style body is empty only through the Pen rule of criterion 10; with nothing selected the panel shows the Document tab (criterion 5), never an empty Style body.
24. Given an object selected and the Document tab active (a manual choice), then the maker can edit the document size, the preset, the unit and the background while the selection stays; the canvas keeps the selection box and handles; Fit to content works as `0015` says. This is new: before the tabs, the Document section was reachable only with nothing selected.

25. Given a selection change caused by undo or redo (`0020` criterion 23), by Select all (`0044`) or by Escape, then the same rules apply (criteria 5 to 8), and the tab and the body change in the same frame as the selection, so the wrong body is never drawn for a frame. Test: undoing "Draw rectangle" empties the selection and takes Style to Document; with History active nothing switches.

## Out of scope

- More tabs: Layers (`0039`), Machine and Job, Align. A tab is added by the spec that needs it. The strip's code holds a list of tabs, not a plugin interface.
- Tab labels next to the icons, a tab overflow menu, reordering or hiding tabs, dragging a tab out into a window.
- Ctrl+Wheel or Ctrl+PageUp and Ctrl+PageDown to step through tabs (Blender's habit; Ctrl+PageUp and Ctrl+PageDown are browser tab keys). Proposal for later.
- Remembering the active tab between sessions.
- A vertical strip on the outer edge (Question 1) unless the customer chooses it.
- Changes to the content of Document or Style.

## Open questions (customer; each has a default)

1. **Where does the strip go?** *Decided (2026-10-10, ux-engineer's refinement, accepted as the default): C, merged into the header row,* at the top of the panel inside the 280 px: the strip replaces the title word, three tabs of 36 × 28 px, the subject line stays right-aligned in 120 px; net cost 4 px of height, no width (criterion 20). The other options: *A:* a strip row of its own above the header, 36 px of height. *B:* **vertical on the panel's edge, like Blender.** It looks like Blender, but it costs about 36 px of width: either the panel grows to 316 px and the canvas shrinks from 520 to 484 px at 800 × 600 (the Select bar wraps earlier), or the content narrows to 208 px, which breaks the 244 px grid of every value field, picker and strip. Not recommended. If the customer wants the visible word back, A is the answer.
2. **Tab order.** *A (default):* Document, Style, History. *B:* Style, Document, History (the most used first). A follows Blender's "scene before object" and the customer's words ("document settings and style, now style and history").
3. **Style tab with nothing selected (criterion 4).** *A (default):* the tab stays in place, dimmed, so the strip never changes. *B:* the tab is not in the tree (the panel rule "hidden, not disabled"), the strip is two tabs and Style appears when something is selected; History would move. Recommendation A: a fixed strip beats a pure rule here, and the toolbox buttons already dim.
4. **Shift+Ctrl+F while History is active (criterion 14).** *A (default):* it activates Style (or Document) and focuses the first control, as the key's name says. *B:* it leaves History alone and only focuses the panel. Recommendation A.
5. **Sticky Document with a selection (criterion 8).** *A (default):* a manual Document stays until the selection goes empty. *B:* a manual choice is forgotten at every selection change. Recommendation A: the maker who is editing the page size does not want it to flip when he clicks an object.

## UX notes

Written by the ux-engineer, 2026-10-10. The numbers and component rules are in `docs/design-system.md`, "Properties panel: tabs"; the History tab is in the same file, "Properties panel: History tab".

### Decisions

| Question | Decision | What it costs |
|---|---|---|
| 1. Strip position | **Horizontal, at the top, inside the header row** (option C, refined). The strip replaces the title word: three tabs of 36 × 28 px, 4 px apart (116 px), left-aligned at the content edge. The subject line stays right-aligned in the remaining 120 px (12 px muted, ellipsis, full text in its tooltip). | **+4 px of height** (a 28 px row and 8 px gap instead of 24 and 8). Option A, a strip row of its own above the header: +36 px, the Background block of the Document tab then scrolls about 106 px instead of 70 at 800 × 600. Option B, vertical like Blender: +36 px of width, the canvas goes from 520 to 484 px, the Select bar wraps earlier; or the content shrinks to 208 px and every 244 px component (value field, picker, preset strips) breaks. B is not recommended. A is the fallback: the same component in its own row, nothing else changes. |
| 2. Tab order | Document, Style, History (PO default). | None. |
| 3. Style with no selection | The tab stays in place, dimmed (PO default). Exception to "hidden, not disabled", recorded in the design system: a strip that moves is worse than a dimmed icon, and the toolbox buttons already dim. | None. |
| 4. Shift+Ctrl+F in History | Activates Style (or Document) and focuses its first control (PO default). | None. |
| 5. Sticky Document | A manual Document stays until the selection goes empty (PO default). | None. |

Why the title word can go: the pressed tab already names the context, every tab has an accessible name and a tooltip, and Blender's microtabs carry no text either. The subject line carries the part that changes ("A4, portrait", "3 rectangles", "24 steps").

### The strip

| Part | Rule |
|---|---|
| Tab | 36 × 28 px hit area (criterion 21), `rounded-[5px]`, 4 px apart. Glyph 18 px, Lucide, 1.5 px stroke: Document `File`, Style `Droplet`, History `History` (clock with a return arrow). Not a brush: the brush belongs to `0033` |
| Unpressed | Glyph `--toolbar-icon` (8.3:1), no border, no ground. Hover: `--editor-accent-hover` ground. Focus: the panel's ring, 2 px `--editor-accent`, 1 px `--toolbar-bg` offset |
| Pressed | `--toolbar-icon-active-bg` ground with `--toolbar-icon-active-fg` glyph: the look of the active tool on the rail, so "accent = this is active" keeps one meaning. Shape carries it (a filled square against bare icons); 3.3:1 against the panel |
| Dimmed Style | Glyph at 40 % opacity, no hover ground, cursor `default`. `aria-disabled="true"`, `tabindex="-1"`, so the arrows skip it and it is not a focus stop; the tooltip still shows on hover: "Style: select an object first". A press does nothing and gives no notice |
| Tooltips | Radix, 400 ms, `side="left"`, text only: "Document (Shift+Ctrl+D)", "Style (Shift+Ctrl+F)", "History (Shift+Ctrl+H)" |
| Strip and body | The panel is a flex column: the header row with the strip does not scroll; the body below it scrolls (one scrollbar, below the strip). The scroll position resets to the top on every tab change. Switching is instant, no animation |
| Subject line | Document tab: "A4, portrait" or "Custom" (`0030`). Style: "3 rectangles" (`0017`). History: "24 steps" in the Document scope, "Rectangle 3f9a1c" in the Object scope (kind may ellipsize, the 6-character id never) |

### Which tab is active

The PO rules (criteria 5 to 11) stand. Additions:

- The tab and the body change in the same frame as the selection, so the wrong body is never drawn for a frame.
- A selection change caused by undo or redo (`0020` criterion 23), by Select all (`0044`) or by Escape follows the same rules: undoing "Draw rectangle" empties the selection and takes Style to Document. History is sticky, so a maker who is working in the History tab sees no switch at all.
- An inline form in the Document tab (`0045`) keeps its draft when the tab changes, by the maker or by a selection. It is not written and not lost; New and Open drop it. This replaces criterion 17 (see the list below).

### How the Document tab holds its blocks

One scrolling column, no nested tabs: the format strips (Paper and any other group with favourites), Orientation, "All formats" (`0045`), Width, Height, Unit, Fit to content, Background. Nothing in the Document tab is a second tab strip; the cost of one more level is not paid until a tab holds more than a screen and a half. The order and the 12 / 16 px gaps are those of `0030`, `0040`, `0045`.

### Keyboard

| Key | Effect |
|---|---|
| Tab | Collapse tab -> the active tab of the strip (one stop) -> the first control of the body. In History the body stop is the list |
| Left, Right | Previous or next tab, activated at once (content is instant). A dimmed tab is skipped. No wrap |
| Home, End | First and last tab |
| Shift+Ctrl+F | As `0017`; with a selection Style, with none Document; History is left for Style (Question 4) |
| Shift+Ctrl+D | **New.** Expands the panel and activates Document, focus on its first control (Width when nothing else is open). Inkscape's key for Document Properties; `0015` had left it out only because nothing needed it. Ignored during a drag |
| Shift+Ctrl+H | As criterion 15 |
| Escape in the strip | Returns focus to the canvas; never clears the selection |

A tab pressed with the mouse returns focus to the canvas (criterion 13); with the keyboard focus stays on the strip so the arrows keep working.

### Height budget at 800 × 600

Panel about 570 px high (to be measured at build). Document tab with Paper only (Slides off): strip row 36, Paper 60, Orientation 60, All formats 36, rule 16, Width, Height, Unit, Fit 144: the Background title row starts at about 380 px. The block still needs the same ~70 px of panel scroll as before `0045`, because Slides (60 px, now off) leaves and "All formats" (36 px) and the +4 px of the strip come in. Style tab: every height of the Style section grows by 4 px ("Whole panel 700 px" becomes 704). History tab: list about 340 px high in the Object scope (seven rows and the Now row), about 410 px in the Document scope, no panel scroll.

### Criteria changes requested and applied

All changes the ux-engineer requested (criteria 1, 3, 4, 10, 12, 14, 17, 18, 20) were applied to the criteria above by the product owner on 2026-10-10. Criterion 17 now keeps the draft instead of cancelling; `0045` criterion 23 has the same rule.

### Design questions: decided (2026-10-10)

1. Merged strip row: decided (Question 1). If the customer wants the visible word "Style", "Document" or "History" back, the strip in its own row is the answer and costs 32 px.
2. Shift+Ctrl+D as a new key: decided yes (criterion 14).

## Links

Requirements: R-EDIT-025
Builds on: `specs/0015-document-size-and-rulers/`, `specs/0017-style-panel-rework/`, `specs/0030-document-size-presets/`, `specs/0040-document-background/`
Needed by: `specs/0020-undo-redo/` (History tab), `specs/0045-document-formats-library/`
ADRs: `adrs.md` confirms none are needed (rule in `curvyo-ui-core/src/panel_tabs.rs`, state in `Session`); build order with 0040 and 0045 is in `adrs.md` ("Collisions and order")
PR: -
