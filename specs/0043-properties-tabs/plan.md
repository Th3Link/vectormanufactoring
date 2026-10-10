# Plan for Properties panel tabs: Document, Style (History follows with 0020)

Built together with `0045-document-formats-library` as one PR on
`story/properties-tabs-and-formats` (milestones are commits). Spec and ADR
notes: `specification.md` and `adrs.md` in this folder (from the docs PR).

## Affected crates/modules
- `curvyo-ui-core`: new `panel_tabs.rs` (tab rule, strip entries); `panel_content.rs`
  becomes `panel_body` (the body rule per tab).
- `curvyo-editor-wasm`: `Session.panel_tabs`, new `session/panel_tabs.rs`
  (`panel_view`, `press_panel_tab`, shortcuts), `wasm_document.rs` pass-throughs.
- `frontend`: `ui/tabs.tsx`, `panel/PanelTabStrip.tsx`, `PropertiesPanel.tsx`
  (header row, scrolling body, shortcuts), `useDocumentPanel.ts`, `index.css`.
- Docs: `docs/design-system.md` already has the tab sections (docs PR); no change.

## Tasks
- [x] 1. `panel_tabs.rs`: `PanelTabs` rule with a table test for criteria 5 to 8 and 14; strip entries and tooltips (AC 1, 4, 5, 6, 7, 8, 14).
- [x] 2. `panel_body(active, pen_unfinished, scope)` replaces `panel_content`; table test (AC 3, 10, 23).
- [x] 3. `Session`: tab state, `panel_view`, `press_panel_tab`, shortcuts; session tests for edges, sticky Document, Pen, size edit with a selection, new session (AC 5 to 8, 10, 11, 14, 24, 25).
- [x] 4. Frontend: strip with Radix Tabs, header row with subject line, scrolling body keyed on the tab, Shift+Ctrl+F and Shift+Ctrl+D, focus rules (AC 1 to 4, 12, 13, 14, 16, 18 to 22).
- [ ] 5. Browser check at 800 x 600: strip, tooltips, canvas width 520 px, no scroll jump (AC 18, 19, 20).

## Validation
Table tests in Rust for the rule; session tests for the glue; the frontend is
display only and is checked in the browser. Existing tests that asserted the old
"empty body for a selection the tool cannot style" (Node tool with a rectangle
selected) now see the Document tab, as `adrs.md` decision 3 says.
