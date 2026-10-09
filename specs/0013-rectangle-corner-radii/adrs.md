# ADRs for "Rectangle corner radii: a separate radius for each corner"

This feature changes one stored field of the rectangle (one radius becomes four)
and one rule of the handle layout. Everything else is the machinery
`unified-object-editing` already built (`ParamHandle::CornerRadius(Corner)`,
`apply_param`, `ScaleModes`, `DragOrigin::shift_at_press`, the Select bar's
Radius field). **No new crate, no new dependency, no new ADR.** The document
change is a feature-local note in the style of `object-transform` (rotation)
and `0007` (style): it extends ADR 0002 §5 and ADR 0009 §3 by more registers of
the same kind and uses ADR 0004 §9 as written, so it is not `needs-customer`
(`CLAUDE.md` §3 asks for ADRs that introduce a document-model concept; this
adds none). `format_version` increases once.

Reference state: `main` at `e285c2d` (Curvyo rename), `CURRENT_FORMAT_VERSION`
5, unified-object-editing PR 1 and PR 2 merged. Checked against
`curvyo-document-core` (`primitive_model.rs`, `shape_codec.rs`, `shapes.rs`,
`primitive_outline.rs`, `container.rs`), `curvyo-ui-core` (`param_handles.rs`,
`param_edit.rs`, `param_entry.rs`, `select_bar.rs`, `select_tool/bar.rs`,
`transform_primitive.rs`, `transform_commit.rs`, `transform_drag.rs`) and
`curvyo-editor-wasm` (`session/select*.rs`, `wasm_select_bar.rs`).

## Depends on

- [ADR 0002 §3](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  radii are `Length` end to end, in a `CornerRadii` newtype; every comparison
  below names its tolerance.
- [ADR 0002 §5, §9](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  one drag, one entry, one bar edit is one commit; the four registers live in
  the primitive's existing meta map.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  a newer file is refused, never partly read. This is why `format_version`
  increases (decision 3).
- [ADR 0009 §3](../../docs/adr/0009-concurrent-editing-semantics.md): every
  parameter is a per-field last-writer-wins register; a rewrite of an
  unchanged value is a new operation that can beat a concurrent edit, so an
  unchanged register is never written (decision 4).
- [`specs/0003-primitive-shapes/adrs.md`](../0003-primitive-shapes/adrs.md):
  the radius is stored raw and clamped where it is evaluated; "Object to path"
  keeps the `NodeId` and strips every primitive key; line-to-arc joins are
  `Corner` nodes.
- [`specs/0009-unified-object-editing/adrs.md`](../0009-unified-object-editing/adrs.md):
  one registry, one hit rule, one resolving function `apply_param` for drag,
  entry and bar; "room for the two follow-up specs" is taken up here as
  planned. Its design-system claim that an unlinked corner "cannot break" the
  handle clearance is wrong for diagonal corners (decision 9, flag 1).
- [`specs/0007-stroke-and-fill-styling/adrs.md`](../0007-stroke-and-fill-styling/adrs.md)
  and [`specs/0021-ellipse-arcs-and-shaping/specification.md`](../0021-ellipse-arcs-and-shaping/specification.md):
  the other two pending `format_version` increases (decision 3).

## Feature-local decisions

- **1. 2026-10-07: stored representation: four flat registers, written per
  corner.** Options: (A) four registers `corner_radius_tl`, `corner_radius_tr`,
  `corner_radius_br`, `corner_radius_bl` in the primitive's meta map, each one
  millimetre `f64`, stored raw; chosen. (B) One list register
  `corner_radii: [tl, tr, br, bl]`. Rejected: LWW is per register, so two peers
  editing different corners would lose one edit (the PO's recommendation, and
  the reason `rect_bounds` being one register is acceptable: a frame is edited
  as a unit, a corner is not). (C) A nested `LoroMap` of four keys. Rejected: it
  gains nothing over flat keys (same per-key LWW), and a nested container is a
  new thing to create, strip on "Object to path" and validate; every other
  primitive field is a flat key. Order and names follow the outline: TL, TR,
  BR, BL (`Corner::ALL`). A rectangle with four equal radii is stored like any
  other (no "uniform" encoding).
  - **Model.** `Shape::Rect { bounds, corner_radii: CornerRadii }` replaces
    `corner_radius: Length`. `CornerRadii { tl, tr, br, bl: Length }` is `Copy`,
    with `CornerRadii::uniform(Length)`, `get(Corner)`, `with(Corner, Length)`.
    `Corner { Tl, Tr, Br, Bl }` moves from `curvyo-ui-core::param_handles` to
    `curvyo-document-core` (the Document API needs it; ui-core re-exports it).
    `Corner::inward_diagonal` is pure `Vec2` maths and moves with it;
    `Corner::local_position(&OrientedBox)` stays in ui-core as a free function
    (it needs `OrientedBox`; a trait for it would have one implementation, which
    `CLAUDE.md` §5 forbids). New module `corner_radii.rs` in `document-core`:
    "A rectangle's four corner radii and their evaluation", pure, no Loro.
    `document.json` (non-authoritative view) serialises it as
    `corner_radii: {tl, tr, br, bl}` in millimetres.
  - **Codec.** New module `corner_radii_codec.rs` ("the Loro keys of a
    rectangle's corner radii"): `read_corner_radii`, `write_corner_radius_if_changed`
    (per register, decision 4), validation. `shape_codec.rs` is 425 lines and
    `shapes.rs` 515 non-test lines (already over the limit), so neither takes
    the growth. The `impl Document` radius commands (`set_corner_radius`,
    `set_corner_radii`, and the radius part of `resize_rect`) move out of
    `shapes.rs` into `shape_radii.rs` ("Document commands that write a
    rectangle's corner radii") as a pure-move first commit.

- **2. 2026-10-07: migration of the single `corner_radius` key.** The legacy key
  stays readable forever and is never written again.
  - **Read, per corner:** the corner's own key if present, else the legacy
    `corner_radius` if present, else the rectangle is `Damaged`. Fallback per
    corner (not "per-corner set absent as a whole") is what makes partial
    states valid: a node with only the legacy key, a node with all four, and a
    node with some of the four plus the legacy key (a version-5 file after one
    corner edit, or a merge of the two) all read to one defined value per
    corner. A node with both a legacy key and an own key reads the own key.
  - **Write:** never the legacy key. `create_rect` writes the four keys at 0.
    An edit writes only the registers whose value changes (decision 4); the
    legacy key stays in the node as dead data, which costs a few bytes and
    avoids a delete operation that could race an edit. "Object to path" strips
    all five keys (`ALL_PRIMITIVE_KEYS` gains the four; the legacy name stays in
    the list). `duplicate_objects` copies the whole meta map and needs no change.
  - **Old files are not rewritten on open** (criterion 18): open validates and
    reads only. A version-5 file opens as four equal radii and renders and
    converts as before; the first save writes the new manifest version but
    leaves the node's keys alone until a radius is edited.
  - **Dual write rejected** ("write `corner_radius` too, for old readers"): an
    old reader cannot reach the objects at all (decision 3), and two sources of
    truth would need a rule for which wins after concurrent edits; an old reader
    that did get past the manifest would show wrong corners silently, the exact
    case ADR 0004 §9 forbids.

- **3. 2026-10-07: `format_version`: one increase, and the numbering plan.**
  - **Why a bump even though a uniform rectangle is still a rectangle.** The new
    writer no longer writes `corner_radius`. A version-5 reader would refuse
    every new rectangle as `Damaged` (missing parameter), the wrong message, and
    with per-corner values it must not open the file at all. The bump makes it
    say "saved by a newer version" (`OpenError::FormatTooNew`). As with every
    earlier bump the manifest declares the container's version, so a file saved
    by the new build is refused by the old one whether or not it holds a
    per-corner radius; criterion 19 is met and slightly exceeded, and the PO
    need not change it.
  - **Migration from version 5 is empty** (decision 2): no key is rewritten.
  - **The number is provisional**, by the standing rule in the
    `CURRENT_FORMAT_VERSION` doc comment: the PR that merges takes `main`'s
    `CURRENT_FORMAT_VERSION + 1` and renumbers its fixtures and notes. Planned
    order, with the number each gets if the order holds:
    1. **this feature, PR 1: 6.** `unified-object-editing` already ranked it
       before 0007 ("the first stored-field change after this feature"), it has
       the smallest model change, and its fixtures are ready to write.
    2. `0007-stroke-and-fill-styling`: 7 (its own note says 6; it renumbers when
       it rebases, and its architect updates that note; this file does not
       touch it).
    3. `advanced-selection`: no bump (its `adrs.md`).
    4. `ellipse-arcs-and-shaping`: 8, one bump for Part A (arc) and Part B
       (curve) if they ship in one PR, otherwise 8 and 9.
    If the customer reorders, only the numbers move; nothing in the designs
    depends on them. **Shared habit for the later registers** (they should copy
    it): flat per-field keys, absent means the default so an untouched shape is
    byte-for-byte as before, the unchanged-value guard in the codec, the
    legacy-or-default read in one function, validation by `Damaged`, a
    generated golden fixture per new version.
  - `CURRENT_FORMAT_VERSION`'s doc comment gets a paragraph for this bump, like
    the earlier ones. `future_format_version.curvyo` must still be newer than
    the new current (a test, not a regeneration).

- **4. 2026-10-07: writing, unchanged values, merging, and the Linked write.**
  - **Per register, compare then write.** `write_corner_radius_if_changed`
    reads the corner with the fallback of decision 2 and writes only when the
    value differs by more than 0 (exact `f64` compare of what the codec read;
    the callers pass already-snapped values). It does not rely on Loro skipping
    equal inserts: the existing code guards by hand (`resize_rect`,
    `set_corner_radii`) and this keeps the property testable. A command that
    changes no register commits nothing (as `set_corner_radii` does now).
  - **Different corners merge.** Two peers editing TL and BR each write one
    register; both survive. Same corner: last writer wins, as for every
    register. A legacy-only node edited concurrently at two corners merges the
    same way, the untouched corners falling back to the legacy value on both
    sides.
  - **Linked drag, entry or bar edit** is one commit that writes the registers
    whose value changes, up to four. A corner that already holds the value is
    not rewritten, so a Linked edit of a rectangle whose TR already has the
    value cannot beat a concurrent TR edit with its own copy.
  - **A merge can break a limit a peer respected**: peer A sets TL 40, peer B
    sets TR 40 on a 60 mm wide rectangle; each wrote a value within its own
    limit. The sum is 80, and evaluation shrinks both (decision 5). Nothing
    repairs the stored values; the same stance as the polygon curve below
    `k_min` and the raw radius of `0003`.

- **5. 2026-10-07: clamping: CSS factor, at evaluation only, one function.**
  `effective_corner_radii(bounds, CornerRadii) -> CornerRadii` in
  `corner_radii.rs` is the one evaluation function. It floors negatives to 0
  (defence; open already refuses them), computes
  `f = min(1, W/(tl+tr), W/(bl+br), H/(tl+bl), H/(tr+br))` skipping a term
  whose denominator is at most 0, and returns the stored radii **unchanged when
  `f` is 1** (no multiply, so a file or fixture with uniform radii within the
  limit is bit-for-bit what it was) and `f · r_i` otherwise. With four equal
  radii `f · r` equals today's `min(r, W/2, H/2)` within 1e-9 mm (the existing
  clamp tests are restated with that tolerance). It replaces
  `effective_corner_radius`, which is deleted (no caller keeps a single-radius
  reading). Every consumer goes through it, as before: `rect_outline`,
  `param_handles`, `value_from_pointer`, `apply_param`, `ParamEntry`,
  `select_bar_state`, `select_view`'s readout. Hit test, rendering and "Object
  to path" read the outline, so they need no change; the selection box and
  `oriented_bounds` are the stored bounds, which a rounded outline always
  touches on all four sides, so criterion 21's "selection box" clause is
  satisfied by construction and needs no code. **The clamp is never written
  back** (criterion 10), with one exception that is a consequence of
  criterion 3 (decision 8, flag 2).
  - **Sharp corner tolerance** (criterion 11): an effective radius of at most
    `SHARP_CORNER_EPSILON_MM = 1e-9` is sharp. The constant is public in
    `corner_radii.rs` and is the same one `PARAM_EQUAL_EPSILON` already is;
    `remove_rounding`'s "has a radius" test and the bar's enabled state use it.

- **6. 2026-10-07: the outline and "Object to path".** `rect_outline(bounds,
  CornerRadii)` walks the corners clockwise from the end of TL's arc: top edge
  (TL end, TR start), right edge (TR end, BR start), bottom edge (BR end, BL
  start), left edge (BL end, TL start). A sharp corner contributes one node, at
  the corner point, in place of its two tangent nodes, so the list holds 4 to 8
  nodes; a sharp TL puts its single node first, so "starts at the TL corner"
  holds (criterion 16). Every node is `AnchorKind::Corner`: a line-to-arc join
  cannot be Symmetric (the `0003` rule), and an arc-to-arc join of two adjacent
  rounded corners is not smooth either (their tangents differ by 90 degrees).
  Arcs use `KAPPA * r_i` handles per corner (0.027 % deviation, inside the 0.1 %
  of criterion 16). Two arcs meeting leave a zero-length straight segment and
  two coincident nodes, as today. With four equal radii the output equals
  today's, 4 nodes at radius 0 and 8 otherwise, anchor for anchor (an exact
  equality test against the old function's results, kept as a regression test).
  "Object to path" is unchanged in code: `convert_to_paths` receives the
  anchors the ui-core conversion built from `outline_of_rotated`.

- **7. 2026-10-07: `resize_rect` and the Scale corner radius switch.**
  `Document::resize_rect(id, bounds, CornerRadii, stroke_width)` writes the
  bounds and each corner register only if its value changed (decision 4). The
  resolved snapshot decides:
  - `CornerRadiusScaling::Keep` (the default): `resize_primitive` hands the
    stored `CornerRadii` back untouched, so no register is written (the property
    the merge test of `unified-object-editing` pins, now per register).
  - `Proportional`: **all four stored radii are multiplied by the one factor
    `√(sx·sy)`** (criterion 12), floored at 0 through the existing
    `scaled_and_floored`, from the stored (raw) values. Per-corner independent
    scaling is not possible with circular radii (a corner has no separate
    horizontal and vertical radius), and one shared factor keeps the ratios
    between corners. A corner at 0 stays 0 and is not rewritten.
  The stroke factor is the same number, computed once, as now; the two switches
  stay independent (criterion 13). The switch is read at the press or when the
  entry opens, exactly as the unified ADR has it; no change to `ScaleModes`.

- **8. 2026-10-07: the Link switch and Shift, drag, entry and bar.**
  - **State.** The Link corners switch is session UI state like the two scale
    switches, never stored (criterion 2). Not part of `ScaleModes` (it is not a
    scale mode). `SelectTool` gets a field `corner_linking: CornerLinking`
    (`Linked` default, `Unlinked`; an enum for the reason `StrokeScaling` is
    one), an accessor pair `corner_linking()` / `set_corner_linking()`, and
    wasm `link_corners()` / `set_link_corners(bool)` in `wasm_select_bar.rs`. A
    click on the switch closes an open entry without writing, as the other two
    switches do (one rule for every bar switch; the entry's own link state was
    fixed when it opened, so this only keeps the rule uniform).
  - **The drag is unlinked** when the switch and Shift differ: `(corner_linking
    == Unlinked) != shift_at_press`, an exclusive or (criterion 3: Shift inverts
    the switch for that one drag; the first draft of this decision said OR and
    the `ux-engineer` overruled it on 2026-10-07, flag 3). `begin_handle_drag`
    computes
    `unlinked: bool` once from the two and stores it in `TransformDrag` next to
    `param_gain`, so preview and release share it and a mid-drag switch click
    changes nothing.
  - **Values.** `ParamValue` gains `CornerRadius(Corner, Length)` (one corner);
    the existing `Radius(Length)` stays and means "all four" (linked drag,
    bar, Remove rounding, entry when linked). Both have a user now.
    `value_from_pointer(start, handle, delta, gain, unlinked)` starts from the
    dragged corner's **effective** radius in both cases and returns
    `CornerRadius(corner, r)` when unlinked, `Radius(r)` when linked.
  - **`apply_param` for a corner** takes the start's effective radii, replaces
    the corner with the value limited to `min(W - r_h, H - r_v)` (criterion 4,
    from the effective neighbours) and never below 0, and returns the start
    unchanged when the resulting effective radii equal the start's within 1e-9
    mm (criterion 12 of the unified spec, kept). **For all four** the value is
    limited to half the shorter side and written to every corner. Because the
    result starts from the *effective* radii, an unlinked edit of a rectangle
    that is shrunk (`f < 1`) also writes the three neighbours at their effective
    values: otherwise the new stored sum would still exceed the side and the
    neighbours would change under the maker's hand, which criterion 3 forbids.
    Where `f = 1`, only the dragged register differs and only it is written.
    This is the one place a clamp is materialised (flag 2).
  - **Commit.** `commit_param` for `CornerRadius(_)` calls `set_corner_radii`
    with the snapshot's `CornerRadii`; `commit_param_batch` for `Radius` keeps
    `set_corner_radius(&ids, r)` (all four, per register guard). `set_corner_radii`
    is generalised from `(NodeId, Length)` to `(NodeId, CornerRadii)`; the bar's
    Radius field still limits each rectangle to half of its own shorter side
    (`select_tool/bar.rs`, as built; the unified ADR's "largest of the batch"
    sentence was superseded in code) and passes `CornerRadii::uniform`.
  - **Typed entry.** `ParamEntry::for_handle(object, box, handle, unlinked)`
    captures `unlinked` when the field opens, from the switch and the Shift state
    of the second press (so `SelectTool` passes the modifiers it recorded at
    that press); the field opens with the dragged corner's effective radius.
    Enter goes through the same `apply_param`: a corner value limited by
    criterion 4 or all four limited by half the shorter side; the limited value
    is written (criterion 6). Accessible name, readouts and the "r 12.0 mm max"
    chip are `ux-engineer` matters on top of that.
  - **Remove rounding** writes 0 to every corner of every selected rectangle that
    has any stored radius above the sharp tolerance, in one commit, each register
    only if it changes. Its enabled state is "some selected rectangle has some
    stored corner above the tolerance".
  - **The bar's Radius field** shows `Uniform(r)` when every effective radius of
    every selected rectangle (four per rectangle) is equal within 1e-9 mm and
    `Mixed` otherwise, so a single rectangle with unequal corners shows Mixed
    (flag 4). The "limited" tag is kept: for a Uniform value, the stored value of
    a corner that exceeds its effective one. Typing sets all four of each
    selected rectangle. No new `Session` state.

- **9. 2026-10-07: handle layout when radii differ, and the diagonal collision.**
  `param_handles` places each corner's knob by its own effective radius with
  the unified map unchanged: `p_i = 15 + ρ_i · L(s)` along its diagonal, `ρ_i =
  e_i / (s/2)`, `s` the shorter side, so `ρ_i` runs from 0 to 2 (a single corner
  may reach the shorter side `s`, the CSS limit with three zeros). **Checked
  by hand and by the second-pass formulas of the unified ADR:**
  - Two corners that share a side (TL/TR, TL/BL, ...) stay at least 14 px apart:
    their radii on that side sum to at most that side, the map is linear and
    `ρ_1 + ρ_2 ≤ 2` on the shorter side (their x or y gap is `W - (o_1 + o_2)`
    with `o_i = (15 + ρ_i L)/√2`, equal to 14 at `ρ_1 + ρ_2 = 2`, larger on a
    longer side because `d/dW` of the gap is positive). The edge resize glyphs
    stay at least 27 px from any knob at `s` 72 and `ρ` 2; the box centre can be
    within 20 px of a knob, so the centre glyph yields as already specified.
  - **Two diagonal corners (TL/BR, TR/BL) can collide.** The CSS rule does not
    link them, so TL = BR = 0.55 s on a square is valid; their knobs then sit on
    the same diagonal at `o_1 + o_2 = (30 + (ρ_1+ρ_2)L)/√2`, which passes the
    side length at `ρ_1 + ρ_2` between about 2.2 and 2.8 (more than 2). The
    knobs overlap (criterion 8 of the unified spec), and the hit rule's nearest
    centre ties. The statement in the unified spec and design system that an
    unlinked corner "cannot break" the clearance is true for adjacent corners
    only. **Default taken (layout only, no restriction on the shapes a maker
    can build):** the knob is drawn at `ρ'_i = min(ρ_i, max(1, Σ − ρ_j))` with `j`
    the diagonal partner and `Σ = 2 + √2·(S − s)/L(s)` (`s` and `S` the shorter
    and longer side on screen, `L` the travel). **Derivation:** the offset of a
    knob from its corner along each axis is `o_i = (15 + ρ'_i·L)/√2`, so two
    diagonal knobs are `S − (o_1 + o_2)` apart along the longer side; 14 px
    centre to centre is the 10 px glyph plus the 4 px gap, so
    `ρ'_1 + ρ'_2 ≤ (√2·(S − 14) − 30)/L = 2 + √2·(S − s)/L = Σ`, using
    `√2·(s − 14) = 2L + 30`. On a square `Σ = 2`, the flat rule this decision
    first stated (`max(1, 2 − ρ_j)`); on a wide box the longer side buys room, so
    a flat 2 would pull knobs back that are 80 px apart. The cap gives
    `ρ'_i + ρ'_j ≤ Σ` for every pair, so at least 14 px between diagonal knobs;
    a corner alone can still reach `ρ' = 2`; where both partners exceed 1 on a
    square both knobs rest at the `ρ = 1` position. The drag arithmetic is
    unchanged (delta from the effective radius with the frozen gain); in that
    capped state the knob lags the pointer, which is the same visible rule as a
    handle that has reached a limit. One pure function
    `knob_rho(effective_radii, shorter_side, longer_side, corner, tolerances) -> f64` next to
    `radius_travel`; `param_handles` and the drag's guide line both call it.
  - **Test.** The clearance property of the unified ADR is re-run over this
    domain: `s ≥ 72`, aspect 1 to 8, four independent effective radii obeying
    the CSS rule (generated as random stored radii pushed through
    `effective_corner_radii`, with the cases TL = BR large and one corner at the
    shorter side included), rotation, zoom: every pair of drawn glyphs (all
    transform handles and four knobs, centre glyph excluded) is at least 4 px
    apart (circumscribed radii as there). **What is true about reaching the
    body (2026-10-07, correcting the claim "a point halfway between the centre
    and each edge midpoint is a move", which is false: on a wide box a knob can
    sit 7.7 px from such a point, also with one uniform radius):** a press is a
    handle if a drawn handle's centre is within its radius (parameter handles 12
    px with no inner band; resize handles only inside a 6 px band); the nearest
    centre wins and ties go to the parameter handle, then TL, TR, BR, BL; every
    other point inside the box is a move. The dead zones are the four 12 px discs
    and the 6 px bands. Over the scanned configurations at least 35 to 41 % of the
    interior stays a move; the centre of a near-square at `ρ` about 1 is a knob
    press in about 9 % of random states, an accepted consequence of
    `unified-object-editing`. A fixed case at TL = BR = 0.6 s, TR = BL = 0 on a square at
    `s` 72 pins the diagonal rule.

- **10. 2026-10-07: validation on open.** In `validate_rect`: each of the four keys,
  where present, is a number that is finite and at least 0, else `Damaged`; a
  corner without its own key needs a valid legacy key, else `Damaged`; bounds
  as today. A stored sum above the side is **not** damaged (decision 5: opening
  must not clamp or refuse what a merge can produce). `OpenError::Damaged` is the
  "named error" of criterion 20; no new error variant (the `0003` rule, and
  YAGNI).

- **11. 2026-10-07: tests and fixtures** (`CLAUDE.md` §5 golden files).
  1. `tests/fixtures/corner_radii_v6.curvyo` (generated by an `#[ignore]` test in
     the style of `container.rs`, committed): a rectangle with four different
     radii, one with a sharp corner at TL, one whose stored sum exceeds the side,
     one rotated, and one node with some own keys plus the legacy key. Open reads
     the exact values; save and reopen is exact (criterion 17); open writes
     nothing.
  2. `tests/fixtures/legacy_corner_radius_v5.curvyo` (built with raw Loro inserts
     of the old key, as the version-3 fixture is): a rounded and a sharp rectangle.
     It opens as four equal radii, renders and converts as before, and is not
     rewritten on open (the Loro version vector is unchanged).
  3. `tests/fixtures/rect_outline_mixed_radii.json` (anchors): the two examples of
     criterion 9 on 100 x 40, TL = BR large, a single rounded corner, all sharp,
     four equal radii equal to the old function's output exactly, and a clamped
     case checked at 1e-9 mm.
  4. Merge tests: peers on different corners both survive; same corner
     last-writer-wins; legacy node edited at two corners by two peers; resize
     with Keep against a corner edit; a Linked write that changes no register
     commits nothing and one that changes two writes two (operation count).
  5. Validation: negative, NaN and mistyped per-corner keys, a mistyped legacy
     key used as fallback, a missing corner with no legacy key: each `Damaged`.
  6. ui-core: unlinked drag leaves the other three effective radii unchanged at
     every step, also from an `f < 1` start; Shift XOR switch; the limit
     `min(W - r_h, H - r_v)`; linked drag overwrites; drag, entry and bar agree
     per value; typed entry captures link state at open; the clearance property
     above; `Remove rounding` per register.
  The existing tests that build `Shape::Rect { corner_radius }` change
  mechanically to `corner_radii` (about 50 sites over 20 files); each keeps its
  assertion.

- **12. 2026-10-07: the two PRs and the size impact.** Both are
  `story/rectangle-corner-radii`-derived and need customer acceptance
  (`CLAUDE.md` §9). Neither is dangerous alone: after PR 1 the maker sees no
  difference, so unlike `unified-object-editing` the PRs need no shared release
  window.

  **PR 1: the model and the format. Criteria 9 to 11, 16 to 21; criteria 12 to 15
  unchanged for uniform radii.** No visible change.
  1. Pure-move prelude: radius commands from `shapes.rs` to `shape_radii.rs`.
  2. `corner_radii.rs` (`Corner` moved, `CornerRadii`, `effective_corner_radii`,
     the sharp tolerance) test first, replacing `effective_corner_radius`.
  3. `corner_radii_codec.rs`, `validate_rect`, `create_rect`,
     `ALL_PRIMITIVE_KEYS`, version 6 and its doc paragraph, the two fixtures
     (17 to 20).
  4. `rect_outline` and the outline fixture (11, 16, 21).
  5. `set_corner_radius`, `set_corner_radii(CornerRadii)`, `resize_rect` with the
     per-register guard; merge tests.
  6. The compile-driven ui-core and wasm adaptation: every consumer reads
     `corner_radii` through `effective_corner_radii`; the UI still writes all four
     (a drag on any handle sets `CornerRadii::uniform`), so behaviour is as
     today. Gate, `docs/requirements.md` R-EDIT-002 line unchanged.

  **PR 2: the editing. Criteria 1 to 8, 12 to 15.** `corner_linking`, the Link
  switch and the bar toggle (the `ux-engineer` look), Shift, `ParamValue::
  CornerRadius`, `value_from_pointer` / `apply_param` / `ParamEntry` per corner,
  `knob_rho` and the clearance property, the Radius field's Mixed, Remove rounding
  per register, readouts and hint, golden screenshots by the `ux-engineer`, demo.

  **Size.** `shapes.rs` shrinks (about 515 to 440 non-test lines); `shape_radii.rs`
  about 150; `corner_radii.rs` about 130; `corner_radii_codec.rs` about 90;
  `shape_codec.rs` down by the radius functions it hands over; `primitive_outline.rs`
  295 to about 330 non-test; `param_edit.rs` 531 total (about 265 non-test) gains
  about 90, `param_handles.rs` 638 total (about 250 non-test) about 40,
  `param_entry.rs` about 20, `select_bar.rs` about 20: all stay under 500
  non-test lines. `select_tool.rs` gets one field and an accessor pair. No new
  `Session` state; two wasm methods in `wasm_select_bar.rs`.

- **13. 2026-10-07: purity and wasm32.** All new document code is in
  `curvyo-document-core` (no filesystem, clock, thread or UI; `#![forbid(unsafe_code)]`,
  builds for `wasm32-unknown-unknown` in the gate). No float is compared without
  a named tolerance (`SHARP_CORNER_EPSILON_MM`, `PARAM_EQUAL_EPSILON`, 1e-9 mm,
  and the exact `f64` compare of the codec on values it just read).

## Flagged to the lead

1. **PO and UX: the claim that unlinked corners cannot break the handle clearance
   is false for diagonal corners** (TL and BR, TR and BL both above about 0.55
   of the shorter side, a valid CSS shape). Default taken, layout only: the
   knob's displayed `ρ` is capped so that two diagonal knobs sum to at most 2
   (decision 9). Visible consequence: with both diagonal corners above half the
   shorter side, their knobs rest together at the `ρ = 1` position and lag the
   pointer. Alternative if the customer minds: forbid diagonal sums above the
   shorter side (rejects a leaf shape with two big opposite corners and does
   not cover a file or a merge). `docs/design-system.md`, "Parameter handle
   layout", needs its sentence "an unlinked corner cannot break it either"
   corrected by the `ux-engineer`, and criterion 1's last sentence by the PO.
2. **PO: criterion 3 and criterion 10 need one sentence.** An unlinked drag or
   entry on a rectangle that has been shrunk below its radii (`f < 1`) writes the
   three other corners at their effective values, because criterion 3 ("the
   other three effective radii do not change at any moment") cannot hold
   otherwise. After such an edit, enlarging the rectangle no longer brings the
   neighbours' old stored radii back. Only an `f < 1` start does this.
3. **PO: criterion 3 contradicts itself.** "Shift inverts the switch" (XOR) against
   "the switch is off, or Shift is held" and "Shift: this corner only" (OR). First
   default: OR. **Resolved 2026-10-07: XOR** (the `ux-engineer`'s judgement,
   adopted in criterion 3): Shift inverts the switch for one drag, the one rule
   the maker can state; with OR Shift would be a dead key in the unlinked state.
   The hint then names what Shift does now ("Shift: all four corners" when the
   switch is off).
4. **UX: states the spec does not draw.** (a) The bar's Radius field shows Mixed for
   one rectangle with unequal corners; typing sets all four. (b) When the
   "Link corners" toggle is visible: default, with the Radius field when the
   selection holds a rectangle (the reserved slot), operable then. (c) The
   toggle closes an open entry like the other switches. (d) The look of the
   toggle and the follower rule (unlinked: others idle; linked: others hover) are
   already noted as open.
5. **PO: wording.** Criterion 20's "named error" is `OpenError::Damaged`, as in
   `0003`; criterion 21's selection-box clause needs no code (a rounded outline
   always touches the four sides of its bounds); criterion 19 holds for every
   file the new build saves, not only those with per-corner radii (as with every
   earlier bump). Criterion 12's "radii keep their ratios" holds only from
   stored values, which is what `Proportional` scales.
6. **Lead: the 0007 note and the ellipse-arcs numbering are not updated here.** The 0007 architect should change "stays at 6" to "7 if
   rectangle-corner-radii merges first"; the arcs architect inherits 8. The
   order is the lead's call with the customer; the rule "merge takes `main`'s
   current + 1" already makes it safe.
7. **Readiness.** Specification and `adrs.md` exist; the spec can become Ready once
   the PO has taken flags 1 to 3 and 5 (wording, no scope change beyond flag 1's
   default) **and the `ux-engineer` has filled the "UX notes" section** (the toggle
   look and its slot, follower and limit rules during an unlinked drag, the Mixed
   display, the design-system row correction of flag 1). No customer question
   remains for the architect; the two customer questions in the specification
   (how to change one corner, elliptical corners) are the PO's.
