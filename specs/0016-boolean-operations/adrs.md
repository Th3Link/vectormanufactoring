# ADRs for "Boolean operations"

This slice adds the first real kernel operation and the first document-model
change since `stroke-and-fill-styling`: a path object may hold more than one
outline. **No new crate, no new ADR, one new external dependency
(`clipper2-rust`, chosen by the 2026-10-04 spike), one `format_version`
bump.** The customer approved the compound-path decision and its file-format
change on 2026-10-09 (`CLAUDE.md` §3, document model; "Customer decisions"
below).

## Depends on

- [ADR 0001 §1, §3](../../docs/adr/0001-ui-framework-and-canvas-rendering.md):
  the rail buttons' enabled state and the command live in `curvyo-ui-core` as
  pure functions; `curvyo-editor-wasm` only binds; the frontend renders the
  state and forwards clicks.
- [ADR 0002 §6](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  "paths are sequences of cubic Bézier segments and lines, with explicit
  open/closed subpaths". The compound path below implements this accepted
  text; it does not amend it, so it needs no new ADR.
- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  sibling order is z-order. "Bottom-most" and "the base operand's place" are
  read from `Document::object_ids()` order, never from selection order (AC 9,
  22).
- [ADR 0002 §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one interaction, one commit. A boolean deletes the operands and creates the
  result in one commit; every id is resolved before the first write (AC 28).
- [ADR 0003 §1, §3, §8](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  the kernel lives in `curvyo-geometry-core`, takes an explicit `Tolerance`,
  runs on flattened polygons and returns polylines. The library is
  `clipper2-rust` (spike note under §3, 2026-10-04). No backend trait.
- [ADR 0003 §7](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  the 0.01 mm kernel tolerance is not the display tessellation tolerance.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  forces the `format_version` bump (an older reader would silently drop every
  outline but the first).
- [ADR 0009 §3](../../docs/adr/0009-concurrent-editing-semantics.md): every
  outline's anchors are a movable list of per-anchor maps, like today's.
  A peer's concurrent edit of an operand is lost when the operand is deleted
  by the boolean: the same delete floor as Delete.
- [ADR 0011 §3, §6](../../docs/adr/0011-workspace-and-crate-layout.md): crate
  edges unchanged; the new dependency must stay off the wasm32 ban list.
- [`specs/0006-path-merge-split-and-node-types/adrs.md`](../0006-path-merge-split-and-node-types/adrs.md):
  caller-minted `AnchorId`s, "a refusal writes nothing", the `format_version`
  merge rule (the merging PR takes `main`'s current version + 1).
- [`specs/0010-edit-interaction-polish/`](../0010-edit-interaction-polish/)
  criteria 54, 55: the key table and its gate; booleans add no entry.

## Feature-local decisions

### 2026-10-09: customer decisions (final)

1. **Compound path: option A**, one object with several outlines, encoded
   as `extra_subpaths` (next section). The document-model and file-format
   change is **approved by the customer**; the next section is accepted as
   written.
2. **Exclusion and Reverse difference are in:** five operations. No kernel
   change: the kernel keeps four operations (see "the kernel").
3. **The five operations are command buttons in their own group in the left
   tool rail**, under the existing tools, always visible, greyed out when
   fewer than two objects are selected. They are **no longer a Select bar
   group**. Consequences: "boolean commands in the tool rail" below.
4. **No keyboard shortcuts for now** (criterion 3 stands).

### 2026-10-09: compound path = option A, one object with several outlines (accepted by the customer 2026-10-09)

Answers the PO's open question 1. **Recommendation and default: A.**

- **(A) One path object holds one or more outlines. Chosen.** A ring is one
  object that fills as a ring, holes stay holes through transforms and later
  booleans, and it is what ADR 0002 §6 already promised. Cost: a format bump
  and an audit of every one-outline assumption (list below).
- **(B) Every result outline becomes its own path object.** No model change.
  Rejected: a filled plate minus a hole paints the hole over (two filled
  discs), the result of one boolean is several objects although AC 19 says
  one, and feeding those objects into the next boolean loses which outline
  was a hole. B is only correct for stroke-only work, and the format change A
  needs comes back with SVG import (multi-subpath `<path>`) anyway.

**Encoding (on disk and in the CRDT).** The existing keys stay the first
outline; one new optional key holds the others:

```text
meta map of a path (unchanged keys):
  closed, anchors, rotation, style keys      first outline, as today
  extra_subpaths : movable list, optional    NEW; absent = one-outline path
    each element a map:
      closed  : bool                         LWW register
      anchors : movable list of anchor maps  same schema as `anchors`
```

- Rejected: replacing `anchors` with a uniform `subpaths` list for every
  path. Old files are never rewritten on open (AC 37), so the reader and
  every writer would handle two shapes forever; `paths.rs`,
  `path_topology.rs` and `objects.rs` would all branch on it. The chosen
  shape leaves every existing path and every node-editing command untouched.
- Rejected: a compact packed-coordinate list for boolean outlines (fast,
  small). It would be a second path schema that the Node tool cannot edit,
  and criterion 20 makes a one-outline result an ordinary path. Revisit
  **only** if the PR 2 measurement below fails.
- Rejected: subpaths as child tree nodes. Children are reserved for groups
  (`layers-and-grouping`).
- Every anchor in every outline has a globally unique, caller-minted
  `AnchorId`. That lets a later node-editing slice (PO question 7, option B),
  Break Apart and Combine address nodes as `(NodeId, AnchorId)` with no
  outline index and **without another format change**.
- A writer never writes an empty `extra_subpaths`; a reader treats an empty
  list like an absent one. A boolean writes only closed outlines with at least
  3 anchors; the reader still accepts open ones (ADR 0002 §6, glyphs, SVG
  import later).
- **Fill rule:** nonzero over all outlines of the object together, fixed. No
  `fill_rule` register: no story needs even-odd. SVG import can turn an
  even-odd path into nonzero outlines with the kernel when it arrives.
- **Stroke:** every outline is stroked with the object's one style. Stroke
  markers (`stroke-markers`) need a rule for compound paths; flagged to that
  story, not decided here.
- **Open-file validation** (`validate_path_node`): a present
  `extra_subpaths` must be a movable list of maps, each with an `anchors`
  movable list whose elements pass `validate_anchor`. Anything else is
  `OpenError::Damaged`.
- **`format_version`:** `main`'s current value + 1 at merge (7 today; other
  drafts claim 8 and 9 provisionally; `document-size-and-rulers` takes none,
  its `adrs.md` decision 1). Migration from older versions: none, the key is
  simply absent. Golden fixtures: a compound path saved and reopened (AC 30,
  37) and a pre-bump file opened unchanged and not rewritten.

**Read model.** `PathSnapshot` keeps `closed` and `anchors` (the first
outline) and gains `extra_subpaths: Vec<SubpathSnapshot>` (`closed`,
`anchors`), empty for every one-outline path, plus `is_compound()` and a
`subpaths()` iterator over all outlines, first one included.
`document.json` skips the field when empty, so one-outline paths export as
before.

**The audit (the real cost of A).** Adding a field does not make the compiler
find the readers that ignore it. PR 2 changes each of these and adds one test
per line, mostly covered by AC 31 to 38:

| Where | What changes |
|---|---|
| `path_codec` (+ a new `subpath_codec` module; `path_codec` is at 474 lines) | read, write, validate `extra_subpaths` |
| `PathSnapshot::rotated`, `scaled`, `sheared` | map every outline |
| `objects.rs`: `translate_objects`, `duplicate_objects` (`renumber_anchors`, `check_anchor_ids`, `translate_path_meta`), `rotate_object`; `resize_path`; skew commit | write every outline; `CopySource::anchor_ids` counts all outlines' anchors in `subpaths()` order |
| `geometry-core::contains_point` | takes several outlines, one `BezPath`, winding summed (AC 33) |
| `ui-core`: `hit_test_object` (`outline_of`, `fills_point`, cheap reject), `object_bounds`, `oriented_box` | all outlines (AC 33, 34) |
| `render-core` artwork, stroke, dash | one lyon path with one subpath per outline; fill is already `FillRule::NonZero` (AC 31, 32) |
| Node tool (`Session::paths()`), `check_join`, `check_split` | skip or refuse compound paths; hint of AC 38 |

`docs/technical-debt.md` gets an entry at PR 2's merge: "one-outline
assumptions are found by audit, not by the compiler", with this table.

### 2026-10-09: the kernel

- **Library:** `clipper2-rust`, as the spike decided (ADR 0003 §3 note).
  Pure Rust, `#![forbid(unsafe_code)]`, BSL-1.0 (already allowed in
  `deny.toml`, added for `loro`'s `xxhash-rust`, so no `deny.toml` change), one dependency (`num-traits`, MIT/Apache). The
  spike built it for `wasm32-unknown-unknown`. Added as a workspace
  dependency, `default-features = false`, used only by
  `curvyo-geometry-core`. Take the newest 1.x at PR time and pin it exactly
  (`=1.x.y`): its provenance risk (AI-assisted port) means an update is a
  reviewed PR that reruns the fixtures, not a Renovate auto-bump.
- **Input and output.** The kernel takes operands as lists of outlines
  (`&[OutlineTriple]` plus `closed`, the triple `curvyo-ui-core` already
  builds for hit-testing) and returns `Vec<Vec<Point>>`. No `kurbo` or
  Clipper type crosses the crate API (as today). It does not take
  `PathSnapshot`, so PR 1 does not wait for PR 2.
- **Operations:** `Union`, `Difference` (first operand minus the rest),
  `Intersection`, `Exclusion`. Reverse difference is `Difference` with the
  caller putting the top-most operand first; no fifth kernel operation.
- **Pipeline**, in this order:
  1. Refuse an open outline: `BooleanError::OpenOperand { index }` (AC 15;
     the kernel enforces it, not only the UI).
  2. Flatten every segment with `kurbo` to `tolerance − 2·GRID` (0.008 mm
     for the fixed 0.01 mm), leaving room for grid rounding and step 6.
     Disc of radius 10 mm: about 79 nodes, inside AC 26's 71 to 142.
  3. Snap to an integer grid, `GRID` = 0.001 mm (a named constant inside the
     kernel, not a setting), into `Clipper64`. Non-finite coordinates or
     |coordinate| > 10⁷ mm: `BooleanError::OutOfRange`. 100 000 mm is 10⁸
     grid units, far inside Clipper's range (AC 42).
  4. **Normalize each operand on its own:** a nonzero self-union. This
     gives every operand positive orientation and its painted region
     (self-intersections, AC 7; holes of a compound operand, AC 8). Without
     it, two operands wound in opposite directions would cancel in their
     overlap under nonzero. An operand that comes out empty:
     `BooleanError::EmptyOperand { index }` (AC 16).
  5. The operation: Union = one nonzero union of all; Difference = first
     operand as subject, the rest as clip; Intersection and Exclusion are
     **folded pairwise** (Clipper's subject/clip form computes
     `A ∩ (B ∪ C)`, not `A ∩ B ∩ C`, and its XOR is not odd-parity for
     three or more operands).
  6. Clean up: Clipper's `SimplifyPaths` with ε = 1 grid unit (AC 24), then
     one more nonzero self-union to repair any crossing the simplification
     made (AC 41), then drop outlines with fewer than 3 points or zero area.
  7. Canonical form (AC 41, 43): outer outlines have positive shoelace area
     in document coordinates (clockwise on screen, Y down), holes negative;
     each outline starts at its smallest grid point (x, then y); outlines are
     sorted by that start point, ties by signed area, larger first. Then back
     to mm (integer × 0.001).
  8. Empty result: `BooleanError::EmptyResult` (AC 17).
- **Determinism (AC 43):** Clipper64 is integer arithmetic; flattening and
  rounding use IEEE basic operations and `sqrt`, which agree on x86-64,
  aarch64 and wasm32. PR 1 checks that `kurbo`'s flattening path calls no
  `powf`/trigonometric function; if it does, the kernel flattens with its own
  subdivision. No `HashMap` iteration order anywhere in the kernel. Output
  lies on the grid, so the golden files agree exactly, not just to 1e-6 mm.
- **Robustness:** `catch_unwind` and timeouts are unavailable in a core
  crate (no threads, wasm aborts on panic), so the guard is the test suite:
  the 9 fixtures of AC 40, the 200-pair seeded property test of AC 14
  (`proptest` is already a dev-dependency), and the spike's four degenerate
  fixtures. A Clipper error maps to a typed `BooleanError`, never a panic.
- **Spec issue, AC 39 (for the PO):** grid snapping cannot guarantee "less
  than 0.0004 mm apart is coincident": two values 0.0004 mm apart can round
  to neighbouring grid points. What the kernel guarantees: coordinates that
  round to the same 0.001 mm grid point coincide, and edges 0.002 mm or more
  apart stay separate. Proposed wording: test the 0.0004 mm gap at
  grid-aligned positions (20.0000 and 20.0004). Rejected: a morphological
  close (offset +δ, −δ) to merge near-gaps; it rounds concave corners and is
  a second geometric policy nobody asked for.
- **Performance (AC 44 to 47):** realistic for the kernel. The spike did a
  5,000-subpath intersection in 9 to 14 ms; 2 × 1,000 curved nodes flatten
  to a few thousand vertices (low milliseconds), 2 × 10,000 to at most
  about 200,000 (well under 2 s, four Clipper passes included). **The risk is
  the document write, not the kernel:** every result node is a Loro map with
  five registers and a 32-character id, about 100 bytes in the operation log.
  AC 46 (1,000 rectangles) and AC 47 (150 ms to repaint) are dominated by that
  write and by reading the snapshot back. PR 2 measures writing and reading a
  20,000-anchor compound path before it merges, because the encoding is the
  hard-to-change part. If it misses, the fallback is the packed encoding
  rejected above, as a new decision here and not a silent change.

### 2026-10-09: where the code lives

- **`curvyo-geometry-core`:** `boolean` (operations, normalization, fold),
  `boolean_grid` (grid conversion, cleanup, canonical form) and `flatten`;
  each under 500 lines. `contains_point` takes several outlines (PR 2).
- **`curvyo-document-core`:** the encoding above and one command,
  `Document::replace_with_path(operands: &[NodeId], base: NodeId,
  outlines: &[(Vec<NewAnchor>, bool)], label)`, in a new module (`objects.rs`
  is at 753 lines). It resolves every id first, then creates the result
  node directly after `base` (`mov_after`, as `duplicate_objects` does),
  writes `base`'s style with `write_path_style` (not a key-by-key copy, as
  Split decided), rotation 0 (AC 27), deletes the operands, and commits once
  (AC 19, 21, 22, 28). It does not compute geometry; `document-core` does not
  depend on the kernel.
- **Commit label:** `boolean_union`, `boolean_difference`,
  `boolean_intersection`, `boolean_exclusion`, `boolean_reverse_difference`,
  following the existing snake-case labels (`convert_to_paths`,
  `duplicate_objects`). The undo slice maps labels to display text. **For the
  PO:** AC 28 should say "a label that names the operation" instead of quoting
  "Union".
- **`curvyo-ui-core`:** a new `boolean` module: operands in z-order from the
  selection, base operand, outlines from each `ObjectSnapshot` (paths as
  stored, primitives via `outline_of_rotated`, through one shared helper that
  `hit_test_object` also uses), the kernel call at a named constant
  `BOOLEAN_TOLERANCE` = 0.01 mm, ids from `AnchorIdMinter`, the refusal types
  with the counts AC 15 to 17 need, the selection afterwards (AC 23), the
  `BooleanOp` enum and `boolean_availability` (next section). Split into
  `boolean` and `boolean_availability` if it passes 500 lines. The Select bar
  does not change (`select_bar.rs`, `SelectBarState` untouched). Nothing goes
  into `select_tool.rs` (2,502 lines) or `node_tool.rs` (1,809).
- **`curvyo-editor-wasm`:** a `session/boolean.rs` glue module (`Session` is
  already past the size limit, technical debt entry) and a `wasm_boolean.rs`
  binding.
- **No new crate.**

### 2026-10-09: boolean commands in the tool rail (customer decision 3)

**Commands are not tools.** The Rust `Tool` enum
(`curvyo-editor-wasm/src/session/mod.rs`) and the TS `Tool` type
(`frontend/src/hooks/useEditorSession.ts`) do **not** grow. A command button
never changes the active tool, is not `aria-pressed`, and never goes through
`onSelect(tool)` or `Session::set_tool`. Rejected: a `Tool::Boolean` mode with
its own bar. It is the Select bar again under another name, a mode the user
has to leave again, and it would claim a tool letter in the key table.

**Where the enabled state is computed.** One pure function in
`curvyo-ui-core::boolean`, never in the frontend and never in editor-wasm
(ADR 0001 §1 and §3):

```text
boolean_availability(objects: &[ObjectSnapshot], selection: &ObjectSelection)
  -> BooleanAvailability
BooleanAvailability = NeedsTwo                          greyed out
                    | OpenPaths { open: usize, of: usize }  enabled, refusal on activation (AC 15)
                    | Ready
```

- One state for all five buttons: their preconditions are identical.
- Counts only selected ids the document still holds (as `ids_of_kind`).
- AC 16 (operand without area) and AC 17 (empty result) are not part of the
  state: they need the kernel, so they are refused on activation.
- `plan_boolean` checks the same precondition function first, so a button
  state and the command cannot disagree.
- Whether `OpenPaths` looks different from `NeedsTwo`, and whether a greyed
  button is native `disabled` or `aria-disabled`, is the ux-engineer's call.
  The state keeps the two apart so the frontend never has to decide.
- The frontend reads it in `syncFromSession`, next to `selection_count()`,
  as a read-and-free plain object (the `NodeToolbarState` pattern), and only
  renders it.

**Which selection counts, per active tool.** `Session` passes its object
selection only while the Select tool is active, otherwise an empty one: the
rule `selected_for_keys` (`session/keys.rs`) already applies to the key
table. One private `Session` helper serves both.

| Active tool | Object selection today | Buttons |
|---|---|---|
| Select | drawn | `NeedsTwo`, `OpenPaths` or `Ready` |
| Node, Pen | kept by `set_tool`, not drawn; Node shows its node selection | `NeedsTwo` |
| Rectangle, Ellipse, Polygon/star | cleared by `set_tool` | `NeedsTwo` |

After a shape is drawn, the tool hands over to Select with that one shape
selected, so the buttons stay greyed until a second object is added.
Rejected: enabling the buttons in the Node tool on the kept object
selection. The command deletes objects and there is no undo yet; it must act
only on a selection the user can see, and in the Node tool the visible
selection is nodes, which can span other paths.

**Dispatch.** Button → `apply_boolean(op: &str) -> String` in
`wasm_boolean.rs` (an unknown `op` is a `JsValue` error, as
`convert_selected(kind)`) → `Session::apply_boolean(BooleanOp)` in
`session/boolean.rs` → `curvyo_ui_core::plan_boolean(objects, selection, op,
minter) -> Result<BooleanPlan, BooleanRefusal>` →
`Document::replace_with_path` → selection = the result (AC 23).

- `BooleanOp` lives in `ui-core` with five variants, the user's operations.
  It maps to the kernel's four operations plus operand order (Reverse
  difference = `Difference` with the top-most operand first) and to the
  commit label.
- `Session::apply_boolean` returns `"ignored"` unless the Select tool is
  active and no drag is in flight. It then flushes the Select bar and style
  previews and cancels an open entry (as `convert_selected_to_paths` does),
  plans, and writes once. Outcomes are `"applied"`, `"ignored"` or a refusal
  kind; refusal counts come from the read-and-free state above. The notice
  text lives in the frontend, keyed by refusal kind (as `KEY_HINT_TEXT`).
- The tool stays Select after the command.

**The rail.** `ToolRail.tsx` keeps its `ToolButton`s unchanged and renders,
under a divider, a command group from a new component (e.g.
`frontend/src/components/BooleanCommands.tsx`): plain buttons of the same
40 px size, props `availability`, `onCommand(op)`, `onReturnFocus`. New
tools are appended to the tool group above the divider, so the command group
moves down with them. Tab order, grouping and roving focus are the
ux-engineer's.

**Focus and key gating.** Nothing in the key gate changes.

- The rail is outside the canvas container that owns `onKeyDown`
  (`Canvas.tsx`), and `isFormControl` matches `button`. So while a rail
  button has focus, no canvas key fires and Space or Enter activates the
  button natively.
- After a mouse click, focus returns to the canvas (`onReturnFocus` when
  `event.detail > 0`, as `ToolButton` does), so Delete and the tool letters
  keep working. Keyboard activation keeps focus on the button.
- During a canvas drag the rail cannot be reached (pointer captured, focus
  in the canvas); the in-flight check in `apply_boolean` is a guard only.
- **No keyboard shortcuts:** `decide` in `session/keys.rs` gets no boolean
  action (AC 3).

**Height.** Six tools, a divider and five commands come to about 495 px
(11 × 40 px buttons, 4 px gaps, 4 px padding). With rulers (0015) the
viewport at an 800 × 600 window is about 546 px high, so the rail fits with
about 40 px to spare at its 12 px inset. No overflow rule now (no seventh
tool exists); flagged to the ux-engineer to measure.

## PR split and order

Updated 2026-10-09 for the customer decisions.

1. **Kernel.** `geometry-core` only, plus the workspace dependency. AC 7, 8,
   10 to 14, 17, 24 to 26, 39 to 45 at kernel level, fixtures in
   `tests/fixtures/`. Four kernel operations cover all five commands.
   Unchanged; in progress (`story/boolean-operations`).
2. **Compound path, format and document command.** Encoding, read model,
   the audit table, format bump (`main` + 1 at merge, 8 if nothing else
   bumps first), the `document-core` command `replace_with_path`, fixtures.
   AC 19 to 22, 27, 28, 30 to 38 and the write-cost measurement. No UI.
3. **Command and rail UI.** `ui-core`: `BooleanOp` (five), `plan_boolean`,
   refusals, `boolean_availability`, selection after; `editor-wasm`:
   `session/boolean.rs`, `wasm_boolean.rs`; frontend: the rail command group.
   AC 1 to 6, 9, 15 to 18, 23, 29, 46, 47 as the PO rewrites them for the
   rail. No Select bar change.
4. **Preview** (P1 to P3), only if question 6 is A. Kernel output drawn with
   the existing `--preview-new` live-preview path; no document write.

**Parallel plan with `document-size-and-rulers` (re-confirmed 2026-10-09).**
Rulers takes no `format_version`, so the version number no longer orders
the two features. `CLAUDE.md` §4 forbids parallel work on shared crates:

| Step | Booleans | Rulers | Shared crates |
|---|---|---|---|
| now | PR 1 (`geometry-core`) | PR 1 (`document-core`, `ui-core`) | none: in parallel |
| next | PR 2 | waits | `document-core` (`objects.rs` move helper), `ui-core` (`object_bounds.rs`), `render-core`, `editor-wasm` |
| then | waits | PR 2 (rulers, pasteboard, viewport) | `ui-core`, `render-core`, `editor-wasm`, `frontend` |
| then | PR 3 | waits | `ui-core`, `editor-wasm`, `frontend` (`App.tsx`) |
| last | | PR 3 (panel; after 0017 if 0017 goes first) | |

Booleans PR 2 starts only after rulers PR 1 merges, because both edit the
move helper and `object_bounds`. It goes before rulers PR 2 because it
carries the write-cost measurement that could still reopen the encoding.
Booleans PR 3 goes after rulers PR 2, because PR 2 moves the rail into the
ruler viewport (`App.tsx`) and the rail height above is measured there.
Swapping booleans PR 2 and rulers PR 2 costs nothing but that risk. Whichever
merges second adds the compound-path case to the rulers resize and fit tests
(rulers AC 17, 23).

## Flagged to the lead

1. ~~Customer, question 1~~: answered 2026-10-09, option A, format change
   approved.
2. ~~**PO:**~~ done 2026-10-09 (AC 1, 1a, 2, 3, 15, 22a, 23, 28, 39, 47a reworded): reword AC 39 (grid wording above) and AC 28 (label). Rewrite AC 1
   and 2 for the rail: always shown; greyed with fewer than two selected
   objects or with a tool other than Select active; the open-path state of
   AC 2 stays. AC 3 stands.
3. ~~**ux-engineer:**~~ done 2026-10-09 (UX notes and design system updated): the rail command group (divider, tab order, greyed vs.
   dimmed look, tooltip text for "select two or more objects with the Select
   tool"), and the rail height at an 800 × 600 window with rulers (about
   495 px of 546 px). Remove the "Boolean group" row from the Select bar
   layout in `docs/design-system.md`.
4. **`stroke-markers`:** needs a rule for markers on compound paths.
5. **Risk:** AC 46/47 depend on Loro write cost; measured in PR 2 before the
   encoding merges.
