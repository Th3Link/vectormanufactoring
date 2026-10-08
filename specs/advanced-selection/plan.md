# Plan for Advanced selection

Branch `story/advanced-selection`, worktree `/home/marc/workbench/vecmanf-claude/adv-selection`,
from `main` at `744dfad` (`CURRENT_FORMAT_VERSION` 7). No stored field, no document write: no
format bump is expected (checked at the end by `git diff` on `curvyo-document-core`: no change).
One PR.

The spec and ADR predate `0007-stroke-and-fill-styling` (filled interiors are hit-tested, the
Properties panel exists) and `edit-interaction-polish` (Esc cascade, key gate, copy badge, dashed
selection box). Where they disagree, the code on `main` and the customer-accepted behaviour win and
the point is listed under "Reconciliation" for the lead.

## Affected crates/modules

| Crate | New or changed |
|---|---|
| `curvyo-ui-core` | `modifiers.rs` gains `alt`. `object_selection.rs`: `SelectionCombine`, `ObjectSelection::apply`. New `marquee.rs` (`MarqueeMode`, `objects_in_marquee`). `hit_test_object.rs`: `hit_test_objects` (candidate list), `hit_test_objects_along` (lasso). `select_tool.rs`: `SelectDrag::Marquee`/`Lasso`, `cycle` field, `pointer_down` takes `Modifiers`; new children `select_tool/cycle.rs`, `select_tool/gesture.rs` (marquee and lasso state, live read, release). |
| `curvyo-render-core` | New `marquee_overlay.rs` (box with 12 % fill and 1.5 px border, dashed lasso line) and the two colour tokens in `theme.rs`. |
| `curvyo-editor-wasm` | `session/tolerances.rs`: `OBJECT_TOLERANCE_PX = 8`, `object_tolerance()` for the Select tool's press, hover, cursor, badge and double-click. `modifiers_changed` gains `alt`. New `session/select_gesture.rs` (overlay, legend text, cursor kind). `move_indicators.rs`: minus badge. `wasm_api.rs` split by tool (pure move) before anything else; the pointer calls carry `ctrl` and `alt`. |
| `frontend/` | `useEditorSession.ts`: Ctrl and Alt on the pointer calls, `applyModifiers(shift, ctrl, alt)`, Alt default suppression, no double-click while Alt is down; `MoveBadges.tsx`: minus badge; `cursors.ts`: `crosshair` and `lasso`. |
| `docs/` | `design-system.md` (8 px row, minus badge), `technical-debt.md` (`wasm_api.rs` entry closed), this folder's `specification.md` status. |

## Tasks

- [x] 1. Pure move: split `curvyo-editor-wasm/src/wasm_api.rs` by tool into one
  `#[wasm_bindgen] impl WasmSession` per file (`wasm_navigation.rs`, `wasm_edit.rs`,
  `wasm_shape_tools.rs`, `wasm_transform_entry.rs`, `wasm_render.rs` plus the existing
  `wasm_*.rs`); `wasm_api.rs` keeps the struct, constructors and the shared view types. No
  behaviour change (technical-debt entry).
- [x] 2. `Modifiers` gets `alt` (`new(shift, ctrl)` stays, `with_alt`); `SelectionCombine` with
  `from_modifiers` (Ctrl wins over Shift) and `ObjectSelection::apply` (Replace, Add, strict
  Remove) (AC 12, 13, 19, 20). Tests first.
- [x] 3. `marquee.rs`: `MarqueeMode::for_drag(start, current, alt)` (rightward = Contain, equal x
  = Touch, Alt inverts) and `objects_in_marquee` over `oriented_bounds(..).document_corners()`
  (Contain: all four corners inside; Touch: the corners' axis-aligned box overlaps), explicit
  `Tolerance` (AC 9, 10, 11, 14). Tests first.
- [x] 4. `hit_test_objects` (every candidate, nearest first, a tie to the topmost; filled
  interiors reconciled, see below) and `hit_test_objects_along` (lasso: resampled polyline
  against `distance_to_object`, bounds prefilter) (AC 3, 4, 5, 7, 18). Tests first, including
  "the first candidate is `hit_test_object`'s answer".
- [x] 5. `OBJECT_TOLERANCE_PX = 8` for the Select tool; Node tool and shape tools stay at 4
  (AC 1, 2). Tests first (a point 7 px away hits, 9 px misses; Node segment at 5 px misses).
- [x] 6. `select_tool/cycle.rs`: `ClickCycle` (captured candidate list, current id, start point);
  recorded by a plain press that acts on an object, dropped by Shift, a handle, a miss, a
  marquee, a lasso, Escape and a tool switch; Alt-click without movement steps it (AC 3 to 7).
  Tests first.
- [x] 7. `select_tool/gesture.rs`, marquee: an empty-canvas press without Alt arms
  `SelectDrag::Marquee`; the 3 px dead zone (`DragOrigin`) separates click from drag; release
  resolves with the modifiers and the mode of that moment; no object moves; Shift and Ctrl on an
  empty press leave the selection alone, a plain press clears it as before (AC 8 to 15). Tests
  first.
- [x] 8. Same file, lasso: an Alt press anywhere arms `SelectDrag::Lasso` (locked at press); past
  the dead zone points are collected; release resolves outline proximity with the combine of that
  moment; a plain Alt-drag that touches nothing clears (AC 16 to 20). Tests first.
- [x] 9. `render-core/marquee_overlay.rs`: green or red box (12 % fill, solid 1.5 px border), the
  dashed 4/3 green lasso line (AC 9, 10, 14, 16; UX notes).
- [x] 10. Session glue: `modifiers_changed(shift, ctrl, alt)`, alt cached; press, hover,
  double-click, cursor and badges use `object_tolerance()`; draw the overlay; the legend through
  `live_readout()`; `cursor_hint()` returns `crosshair` for a marquee and `lasso` for a lasso or
  Alt held while idle; the minus badge where a Ctrl press arms the marquee (AC 1, 11, 14, 16;
  UX notes).
- [x] 11. Facade and frontend: `pointer_down/hover/up` carry `ctrl` and `alt`, `applyModifiers`
  sends Alt, the hover is re-sent when Alt, Shift or Ctrl changes during a drag, a lone Alt
  key event is not left to the browser, Alt presses never count as a double-click, cursors and
  the minus badge (AC 11, 14, 16, 5).
- [x] 12. Docs: design-system rows, technical-debt entry, spec status; browser check in the
  Browser pane; full CI gate.

## Reconciliation (spec versus `main`)

1. **Candidate set with fills.** The spec says the cycle runs over the outline-distance
   candidates because no fill hit test exists. `0007` added one (criteria 23, 27). The plain click
   keeps `hit_test_object`'s answer unchanged; `hit_test_objects` returns that answer first, then
   the other outline candidates nearest first, then objects covered by a filled object above them
   (outline in tolerance or filled interior), so an Alt-click can reach what a fill hides.
2. **Ctrl inside the sole selected box.** First built as a copy-move (read from the code on
   `main`), corrected after review: `edit-interaction-polish` criterion 37, `unified-object-editing`
   criterion 35 and this spec's UX note agree that Ctrl, like Shift, bypasses the inside-box move
   and arms the marquee (Ctrl removes, Shift adds). `classify_press` takes `Modifiers` for it.
   Ctrl-copy stays on an outline and on the centre handle (criterion 38).
3. **Clearing on an empty press.** The clear of a plain empty click is at the **release**, as the
   ADR describes (`PendingEmpty`), because criteria 12 and 13 read Shift and Ctrl at the release.
   Nine existing tests that asserted an empty selection after the press now release first.
4. **Alt over a handle.** Not a deviation: `unified-object-editing` criterion 35 already decides
   that Alt at the press arms the lasso wherever it lands, a handle included. The Alt rule lives
   in `classify_press` (`PressTarget::Lasso`, `PressTarget::arms_lasso`), read by the press, the
   hover, the cursor and the badges.
5. **Lasso and fills.** Outline proximity only (AC 18); a line inside a filled shape that never
   crosses its outline selects nothing, as AC 20 words it.

## Validation

- Core logic test-first in `curvyo-ui-core` (unit tests beside each module, integration tests in
  `curvyo-ui-core/tests/advanced_selection.rs` and `curvyo-editor-wasm/tests/advanced_selection.rs`
  for the Session level: modifiers, tolerance, cursor, legend, badge, Escape).
- `render-core` tests count triangles and colours of the overlay.
- Full CI gate from `.github/workflows/ci.yml` on the head sha, then a pass in the Browser pane:
  marquee both directions, Alt inversion mid-drag, lasso, Alt-click cycling, minus badge, cursors,
  8 px hit area.
