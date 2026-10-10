# ADRs for "Properties panel tabs"

The tabs are view state and a small rule set on top of `panel_content`. **No
new ADR, no new crate, no new dependency, no format bump, no trait, no
generic.** Reference state: `main` at `52d4101`.

## Depends on

- [ADR 0001 §1, §5](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  which tabs exist, which is active, enabled and why, and every tooltip text
  are decided in Rust; the frontend renders the view (strings and scalars
  across the wasm boundary).
- [ADR 0009 §2](../../docs/adr/0009-concurrent-editing-semantics.md): the
  active tab is session view state, never written (criterion 11).
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md): the rule
  goes in `curvyo-ui-core`, the state in `curvyo-editor-wasm`'s `Session`.
- [`0017` adrs.md](../0017-style-panel-rework/adrs.md) decision 6: the
  `panel/` folder's lint rule (no popover, select or menu). Tabs are not a
  popup; Radix `Tabs` is allowed there.
- `docs/technical-debt.md`, "`panel_content` needs the active tool": the
  panel decision stays in one place, `ui-core`; no second decision in
  TypeScript.

## Feature-local decisions (2026-10-10)

1. **The rule: a new module `curvyo-ui-core/src/panel_tabs.rs`, next to
   `panel_content.rs`.** One job: which tab is active after an event.
   `enum PanelTab { Document, Style }` (0020 adds `History` with its rule
   and tests; no variant without a body, `CLAUDE.md` §5). `struct PanelTabs
   { active, scope_was_non_empty }` with pure methods:
   `reset(non_empty)` (criterion 5), `observe(non_empty)` (criteria 6 to 8),
   `press(tab, non_empty) -> bool` (criteria 4, 8), `shortcut_style(non_empty)`
   (criterion 14). `observe` acts on edges only: empty to non-empty with
   Document active gives Style; non-empty to empty with Style active gives
   Document; anything else keeps the tab. **No "manual" flag is needed:** a
   pressed Document survives non-empty-to-non-empty changes because there is
   no edge, and falls back to Style at the next empty-to-non-empty edge, which
   is criterion 8. History (0020) is untouched by both edges, so it is sticky
   for free. One table test lists criteria 5 to 8 and 14 row by row.
2. **"Selection state" is the active tool's style scope** (`style_has_objects`,
   the input `panel_content` already takes), not the object selection. In the
   Select tool they are the same. In the Node tool the scope holds the edited
   path, which is what the Style body shows today. A switch to the Pen with
   a selection is a non-empty-to-empty edge, so Style hands over to Document,
   as the panel does today.
3. **`panel_content` becomes the body rule per tab.** Signature
   `panel_body(active, pen_path_unfinished, style_has_objects) -> PanelContent`:
   Document shows the Document section unless the Pen has an unfinished path
   (criterion 10, the existing reason); Style shows Style when the scope holds
   objects, otherwise Empty (reachable only through the Pen rule, criterion 23).
   The old "Empty for a selection the tool cannot style" case disappears,
   because Document is now reachable with a selection (criterion 24). The
   existing `panel_content` tests are rewritten as rows of the new table.
4. **State in `Session`, updated when the panel view is read.** `Session`
   holds one `PanelTabs`. `Session::panel_view(&mut self) -> PanelView`
   (replacing `panel_content()`) calls `observe` with the current scope, then
   returns the tabs (id, accessible name, tooltip, `enabled`, `selected`)
   and the body. The frontend already reads the panel after every pointer
   release, tool, selection and panel change, so every edge is seen in the
   frame it happens. **Accepted risk:** two edges between two reads (empty,
   non-empty, empty) collapse into none; nothing in the UI produces that
   without a read in between. `new_document` and `open` call `reset`.
   `press_panel_tab(name) -> bool` and the Shift+Ctrl+F target go through
   `wasm_document.rs` (135 lines, room for three calls).
5. **Frontend: display only.** A new `panel/PanelTabs.tsx` over a shadcn
   wrapper `ui/tabs.tsx` of Radix `Tabs` from the existing `radix-ui` package
   (no new dependency). Radix gives `role="tablist"`/`tab`/`tabpanel`,
   `aria-selected`, `aria-controls`, one Tab stop, roving focus, Home and End
   (criterion 12). `activationMode="automatic"`, `value` controlled by the
   Rust view, `onValueChange` calls `press_panel_tab` and re-reads the view.
   The dimmed Style tab uses Radix `disabled` (skipped by the arrows) plus an
   explicit `aria-disabled="true"`; its tooltip sits on a wrapper element,
   as for the dimmed rail buttons, because a disabled button gets no pointer
   events. The strip lives in `PropertiesPanel.tsx` outside the scrolling
   body (criterion 18); the body's scroll resets on a tab change by keying
   the scroll container on the active tab (criterion 19). The mouse-press
   focus rule (criterion 13) and the eyedropper cancel (criterion 16) reuse
   the panel's existing handlers. No tab logic in `useDocumentPanel.ts`
   beyond passing the view through.
6. **Keys.** The strip's tabs are buttons, so the key gate's `dom_blocked`
   already keeps Left and Right away from the session (and from 0044's
   nudge) while the strip has focus. Shift+Ctrl+F stays where it is
   (`PropertiesPanel.tsx`) and asks Rust for the tab; Shift+Ctrl+H comes with
   0020.

## Collisions and order

- **0040 and 0045** edit the same files: `PropertiesPanel.tsx`,
  `DocumentSection.tsx`, `useDocumentPanel.ts`, `session/document.rs`,
  `wasm_document.rs`. They also share `ui-core` and `editor-wasm` with this
  slice, so they do not run in parallel with it (`CLAUDE.md` §4). Order:
  0043, then 0040, then 0045. 0043 changes the panel frame and the body
  selector only; the Document section's content is untouched, so 0040's
  `adrs.md` stays valid.
- **#80** touches `session/mod.rs` and `useEditorSession.ts`: rebase-level.
  0043 can start from `main` now.
- **0020** adds the History variant, its body, the Shift+Ctrl+H key and the
  sticky test rows.

## Delivery: one branch `story/properties-tabs`, one PR

1. `ui-core`: `panel_tabs.rs`, the body rule, table tests (criteria 4 to 8,
   10, 14, 23).
2. `editor-wasm`: `PanelTabs` in `Session`, `panel_view`, `press_panel_tab`,
   reset on New and Open; session tests for criteria 5 to 11 and 24 (edit
   the size with an object selected, the selection stays).
3. Frontend: the strip, layout, keyboard, tooltips (criteria 1, 2, 9, 12,
   13, 16 to 22). Then the UX review and the tester pass.

## Flagged to the PO (defaults taken)

1. Criteria 4 and 12 conflict slightly: a tab that the arrows skip and that
   is never the Tab stop cannot be focused by keyboard. Default: skipped and
   not focusable, announced as unavailable inside the tablist (Radix
   `disabled` + `aria-disabled`).
2. Criterion 6 says "selection"; the rule uses the tool's style scope
   (decision 2). Same result in the Select tool.
