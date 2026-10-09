# Architect review: PR #72 (0015 PR 2 of 3, rulers and pasteboard)

Head `75d6fae`, base `c61a5fb`. Static review against `specification.md`,
`adrs.md` (decisions 4, 5, 8 to 12, 14 to 16), the UX notes,
`docs/design-system.md` and CLAUDE.md §5 and §6. No build or test was run
(other agents share the machine); the implementer's gate run is assumed.

## Verdict: CHANGES REQUESTED

One blocking finding (AC 2 on the resize paths). Everything else is
non-blocking: record it or fix it in this PR or PR 3.

The structure holds. No new crate, no new external dependency, no format
change. `render-core` still sees only `ViewTransform` and document types.
`curvyo-app` drops `curvyo-document-core`, which brings it in line with
ADR 0011 §3 (`app → storage-io`). Nothing in `curvyo-app`, `frontend/src`,
`.github` or `docs` still uses `project-state`, `get_project_state`,
`ProjectStatePayload` or `size_mm`. `Cargo.lock` loses one internal edge, so
`cargo deny` is not affected.

## Blocking

1. **`frontend/src/components/Rulers.tsx`: the rulers are one frame late on
   a window resize and on a Properties panel toggle (AC 2 names both).**
   Within one frame, the browser runs `requestAnimationFrame` callbacks,
   then layout, then `ResizeObserver` callbacks, then paints. So the order
   on a resize is:
   - the rulers' rAF loop paints from the old origin and the old strip
     size;
   - `useEditorSession`'s ResizeObserver then calls `session.resize` and
     renders the GPU canvas at once.

   The canvas shows the new view and the rulers show the old one for that
   frame. A panel toggle also changes the horizontal strip's CSS width
   before its backing store is resized, so the strip is drawn stretched for
   one frame (about 35 % at 800 px).

   A second ResizeObserver inside `Rulers` does not fix this. Child effects
   run first, so its callback would run before the session is resized.

   Fix: give `useEditorSession` a small "view resized" listener. Its
   ResizeObserver calls the listener right after `session.resize`, and
   `Rulers` registers the same `repaintIfChanged` its rAF loop already uses.
   That is a hook point of a few lines, not drawing code, so decision 8's
   "do not add the drawing to `useEditorSession.ts`" still holds. The rAF
   loop stays for pan, zoom, pointer and unit changes. If the PO prefers to
   accept the lag instead, write it as an as-built note on AC 2. Do not
   leave it unrecorded.

## Non-blocking

2. **Default view and the `inset_view` flag (`curvyo-ui-core/src/viewport.rs`,
   AC 11a).** This is a bounded piece of state, not a clean design. It is
   acceptable with conditions.
   - One hidden flag changes what `resize` means. Until the first
     navigation, a window resize keeps the top-left corner. After it, the
     resize keeps the centre.
   - The behaviour of accepted `0004` criterion 10 now depends on history.
     For example, a maker who draws without panning and then resizes the
     window gets the top-left behaviour. That is a change to accepted
     behaviour, and it is not in the spec.
   - Every future mutator of `Viewport` has to decide whether it clears the
     flag. That is the trap. Examples are PR 3's
     `pan_by_document_offset` and any later zoom preset.

   Conditions:
   - (a) Add an as-built note to the spec under 11a: "a window resize keeps
     the document's top-left until the first pan or zoom; this amends 0004
     criterion 10 for an untouched view". Add a dated note under decision 7
     in `adrs.md`.
   - (b) In PR 3, state in a test whether `pan_by_document_offset` clears
     the flag. Recommendation: it does not.
   - (c) Check in the browser whether the drift really comes from early size
     reports. `attach_canvas` already records the laid-out size, and the
     ResizeObserver's first callback reports the same size, which shifts
     nothing. If the drift does not come from there, the flag only serves
     window resizes. Then the simpler design is to delete the flag and keep
     0004's centre rule.
   - The 72 px default is applied in `WasmSession::new` and `open`, not in
     `Session::new` and `Session::open`. That is acceptable, because
     headless tests keep the 0004 view. Say so in the doc comment of
     `show_default_view`.
3. **`Session::frame_draw_list` instead of `draw_list` (deviation from
   decision 11).** The deviation is justified.
   - About 300 call sites in `editor-wasm` tests assert on `draw_list`
     layers and triangle counts. PR 71's tester file also uses `draw_list`.
     Prepending the area there would have changed all of them for no gain.
   - Both production render paths (`render`, `resize`) use
     `frame_draw_list`. `DrawList::extend` keeps the layering right with and
     without artwork, and `frame.rs` tests both cases.
   - The clear colour and the vertex colours go through the same `/255`
     path on the WebGPU and WebGL2 backends, so the document area matches
     the old clear colour. The canvas element's CSS background is the
     pasteboard colour, which is right before the first frame.
   - Record it: add a dated note to decision 11 in `adrs.md`: "the area is
     prepended in `frame_draw_list`; `draw_list` stays the artwork and
     overlay for headless tests; a future raster or thumbnail path must use
     `frame_draw_list`".
4. **Label thinning band (`ruler.rs`, `STEP_LABEL_GAP_PX` = 4 against
   `LABEL_GAP_PX` = 8).** The step follows AC 5 (label plus 4 px). The
   labels thin when the spacing is below label plus 8 px, which is the UX
   notes' number. In the band between the two, labels go on every second
   tick although they are not "wider than the major spacing" (AC 7
   wording). The result is correct, and it keeps the end of a label off the
   next full-height major tick. The wording of AC 7 and the code disagree,
   though.
   - Add a dated note to decision 9: "step per AC 5 (+4 px); labels thin
     below +8 px (UX notes) so the end of a label never touches the next
     major tick".
   - Ask the PO to change AC 7 to "when a label plus 8 px is wider than the
     major spacing".
   - Keep the two constants. `labels_thin_out_to_every_second_tick...`
     covers the band.
5. **Units as types (`ruler.rs` lines 132 to 149).** `value * mm_per_unit`
   and `/ mm_per_unit` bypass `Length::from_unit` and `Length::in_unit`,
   which decision 3 names as the only conversions. Route `to_px` and
   `from_px` through them. `px_per_unit` as a scale factor is fine.
   - `origin_px` uses an unnamed 1 px tolerance (`-1.0..=length_px + 1.0`).
     Make it a named constant.
   - The label building is correct. `label_text` works on i128 integers
     with the decimal point placed by the exponent, and its tests cover 0.3,
     negatives, trailing zeros and 1e6.
6. **`digit_px` is measured as the width of "0" only
   (`Rulers.tsx` `measureDigit`).** The doc comment of `ruler_layout`
   promises that the width per character is "never narrower than the real
   text". That holds only if U+2212 and every digit are no wider than "0".
   In proportional `system-ui` fonts (Canvas2D cannot ask for tabular
   figures) that is not guaranteed. Measure the maximum advance over
   `"0123456789.−"`. This is one line and makes AC 7 hold whatever the
   font.
7. **Module size and responsibility.**
   - `ruler.rs` is 541 lines, about 270 of them production code, and
     `viewport.rs` grew from 481 to 545. Either give a reason in the PR or
     move the tests to `ruler/tests.rs`.
   - `session/ruler.rs`'s doc sentence needs "and" three times: rulers,
     status texts, default view. Move `show_default_view` to
     `session/navigation.rs`. Move `display_unit`, `size_text` and
     `cursor_text` to the `session/document.rs` that PR 3 creates anyway
     (decision 13). `wasm_ruler.rs` has the same issue with the status texts.
8. **`frontend/src/hooks/useSessionStatus.ts` polls the session every frame
   forever.** Decision 12 says to read after every sync. The poll costs
   little next to the GPU render loop. It is still a third perpetual rAF
   loop, and `INITIAL.cursorText` copies Rust's formatting into TS.
   - In PR 3, move the read into `syncFromSession`. PR 3 adds the resize,
     fit and unit commands, which go through the sync anyway.
   - Until then, add a `docs/technical-debt.md` entry. This PR adds none.
9. **Exports with no user outside their crate.** `MIN_MAJOR_PX` and
   `LABEL_OFFSET_PX` (ui-core), and `background_at` and `CANVAS_BG`
   (render-core), are `pub use`d but used only inside their own crate.
   Narrow them to `pub(crate)` (CLAUDE.md §5, dead code).
   - The TS constant `LABEL_OFFSET = 4` copies `LABEL_OFFSET_PX`. Have
     `RulerView` return label start positions instead of tick positions,
     so the offset lives only in Rust.
10. **`build_pen_preview(…, document_size)` signature change.** The change
    is fine. The function now has six parameters, and `DocumentSize` is the
    simplest input. A callback or a generic would break §5. The knockout
    colour is chosen at the knockout's centre, as decision 11 says. A
    knockout that straddles the edge is half wrong for at most a few
    pixels, which is accepted. No other knockout uses `CANVAS_BG`.

## Checked and fine

- `ruler.rs` is pure and builds for wasm32: no I/O, i128 integers,
  `repeat_n`. It covers AC 5 examples, AC 2 projection within 0.5 px over
  20 steps, AC 6, AC 7 to ±1e6 for all units and lengths, and AC 3 and 4.
- The pointer marker comes from the same canvas-relative CSS position the
  status bar converts, so the two are equal by construction (decision 10).
  It disappears on `pointerleave`.
- The rulers repaint only when the view, unit, size, device pixel ratio or
  pointer changes. Exact equality is right there, because a change is a
  change. Ticks snap to whole device pixels.
- Rulers and the corner are `aria-hidden`, cannot be focused, use the
  `default` cursor and swallow presses (AC 10). The wheel is forwarded with
  the point clamped into the canvas, and the canvas handler reads `clientX`,
  so synthetic events work (AC 10a).
- There is a test runner: `npm test` (`node --test`) runs in CI, and
  `rulerDraw.ts` is pure and tested there. Everything numeric is tested in
  Rust. The remaining untested glue is the event wiring in `Rulers.tsx`,
  which is the tester's Browser pane check.
- The document area is snapped to device pixels and covered by tests at
  DPR 1, 1.5 and 2. `PASTEBOARD_BG` is one constant in render-core's
  `theme.rs`, and the duplicate in `gpu.rs` is gone.

## Overlap with PR #71 and merge order

- PR 71 (`story/boolean-compound-path`, local head `4395d11`) shares only
  `curvyo-editor-wasm/src/session/mod.rs` with this PR. PR 71 changes
  `paths()`. This PR adds `mod frame;` and `mod ruler;`. Neither has a
  textual conflict.
- PR 71 does not touch `session/draw.rs` or `pen_preview.rs` at its current
  head.
- Semantically the two do not interact: compound artwork layers follow the
  area layer, and PR 71's tests use `draw_list`, which this PR leaves alone.
- **Recommendation: merge PR 71 first.** It carries the format v8 bump and
  is next in the 0016 sequence. Then rebase this PR and run the
  `editor-wasm` and `render-core` tests once. Either order works without a
  version change.

## Technical debt to add (this PR)

- If finding 8 is deferred: "Status bar texts are polled every animation
  frame (`useSessionStatus.ts`); move to `syncFromSession` with PR 3."
- If finding 1 is accepted rather than fixed: "Rulers lag one frame behind
  the canvas on window resize and panel toggle."
