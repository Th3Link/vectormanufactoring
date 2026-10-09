# ADRs for "Stroke markers"

Markers are part of the stroke style of an object node: five new registers,
one format bump, a placement and tessellation step in `render-core`, panel rows
on the `style-panel-rework` value field. **No new crate, no new dependency, no
trait, no generic, no ADR amendment.** Builds after `style-panel-rework` PR 2
(format) and PR 3 (value field); see decision 6.

## Depends on

- [ADR 0002 §5](../../docs/adr/0002-document-model-units-and-svg-round-trip.md):
  per-node resolved style; markers are style values, not objects.
- [ADR 0009 §3](../../docs/adr/0009-concurrent-editing-semantics.md): one LWW
  register per control, so concurrent edits of different slots both survive.
- [ADR 0004 §9](../../docs/adr/0004-persistence-and-cross-machine-sync.md):
  an older reader would ignore the new keys and draw no markers (a silent
  partial read), so the format version goes up.
- [ADR 0003 §7](../../docs/adr/0003-geometry-kernel-booleans-offsetting-vcarving.md):
  markers are display geometry, like dashes, so placement and tessellation are
  `render-core` with `lyon` at a display tolerance. If `manufacturing-roles`
  ever makes markers cut geometry (spec open question 5), placement moves to
  `geometry-core` at manufacturing tolerance (§2), as 0007 said for dashes.
- [ADR 0011 §3](../../docs/adr/0011-workspace-and-crate-layout.md):
  `render-core -> document-core` only; it cannot use the kernel, which is why
  the tangent rules below use the anchors and `lyon`'s sampler.
- [`0007` adrs.md](../0007-stroke-and-fill-styling/adrs.md) and
  [`style-panel-rework` adrs.md](../0017-style-panel-rework/adrs.md): register
  split, single coverage per layer (depth), `StyleEdit::apply_to` for preview
  and commit, value field scales in `ui-core::value_scale`.

## Feature-local decisions

- **2026-10-08: (1) data model and format.** `Stroke` gains
  `markers: Markers`, so paths and primitives share one `Style` and one codec.
  A primitive never gets marker keys written (decision 4) and never draws them.

  ```text
  object node, new keys (each its own LWW register):
    stroke_marker_start      : "none" | "arrow" | "dot"   absent = "none"
    stroke_marker_mid        : "none" | "arrow" | "dot"   absent = "none"
    stroke_marker_end        : "none" | "arrow" | "dot"   absent = "none"
    stroke_marker_mid_place  : "spaced" | "nodes"        absent = "spaced"
    stroke_marker_mid_count  : integer (I64) >= 1         absent = 1
  ```

  Types in `style_model.rs`: `MarkerShape { None, Arrow, Dot }`,
  `MarkerPlace { Spaced, AtNodes }`, newtype `MarkerCount(u32)` (`>= 1`),
  `Markers { start, mid, end, mid_place, mid_count }`. Setting a slot back to
  None writes `"none"` (no key deletes, as for join and cap). Five registers,
  not one grouped `markers` map value: the slots, Place and Count are separate
  controls (0007's rule), and Place and Count stay stored while Middle is None
  (criterion 2). New `StyleEdit` variants `MarkerStart`, `MarkerMid`,
  `MarkerEnd`, `MarkerPlace`, `MarkerCount`; `write_changes` covers them, so
  Split's `write_style` and the "unchanged writes nothing" rule hold with no
  extra code. `document.json` gets them inside `style.stroke.markers`.
  - **Bounds.** 1 to 500 typed and 1 to 50 dragged are `ui-core` edit-time
    limits (`ValueScale::MarkerCount`, linear, integer grid). Stored: any
    integer `>= 1`. Drawn: at most 500 per slot of one object.
  - **Open-file validation** (strict, as for join and cap): an unknown shape or
    place string, a count that is not an integer, or a count `< 1` is
    `OpenError::Damaged`. A count above 500 opens and is kept. The lenient read
    (merged documents, never validated) maps an unknown string to the default
    and a count `< 1` to 1, and nothing is rewritten by reading.
  - **Criterion 29, PO edit:** a file "written by a later build" carries a
    higher `format_version` and is refused as "saved by a newer version" before
    any key is read; a later build that adds a shape must bump. So "a shape
    name this build does not know reads as None" holds for merged documents
    only, and "a Count in a file below 1 reads as 1" becomes "is refused as
    damaged". Default: build it this way; the PO rewords 29.
  - **`format_version`:** `CURRENT_FORMAT_VERSION + 1` at merge, **next free at merge (after 0016's 8)**
    (`style-panel-rework` and `ellipse-arcs-and-shaping` take theirs the same
    way; whichever merges first gets the lower number, by the standing rule). Migration empty:
    absent keys read as None, Spaced, 1. Fixtures: `markers_vNEXT.curvyo` (named after the version it takes) (every
    key set, a count of 501, a closed path), and the existing
    `legacy_gradient_v7.curvyo` and `dash_vNEXT.curvyo` (0017's fixture) must open with every slot
    None.

- **2026-10-08: (2) placement, a pure function in `render-core`.** New module
  `marker_place.rs`: `marker_placements(anchors, closed, &Markers, length
  measure) -> Vec<MarkerPlacement { point, direction, shape, reversed }>`.
  New module `markers.rs`: shapes from placements (arrow triangle `4w x 3w`,
  dot circle `3w` across, centred on the anchor) and their `lyon` fill
  tessellation. Two modules, one job each.
  - **Ends and nodes** (criteria 7, 9, 10, 14, 16) come from the anchors: the
    outgoing tangent of a node is `handle_out`, or if its length is below
    `1e-9` mm the direction to the next control point or node that differs by
    more than that; incoming likewise backwards; a closed path's first node
    takes its incoming tangent from the closing segment. Inner nodes use the
    normalized sum of incoming and outgoing unit tangents; within 1 degree of a
    reversal, the outgoing tangent. These are control-polygon directions, not
    curve evaluation.
  - **Spaced** (criteria 8, 10) uses `lyon::algorithms::measure`
    (`PathMeasurements`, `SampleType::Distance`), the same measure as
    `dash.rs`; the sample gives the point and the tangent at it. Measuring
    tolerance is `min(display tolerance, 0.01 mm)`, so positions do not move
    with zoom and the tests can hold 0.05 mm on a line and 0.5 mm on a curve at
    any view. Closed: fractions `k / N`, k = 0 to N - 1, from the first node.
  - Fewer than 2 nodes or a length below 0.001 mm: no markers (criterion 11).
    Dashes and caps never enter: placement reads the undashed path (criterion
    12). Size uses the **drawn** stroke width (the document width, floored at
    the display minimum and capped like the stroke itself), so a marker stays
    in proportion to the line you see; at normal zoom that is exactly `w`.
  - Primitives draw no markers in `artwork.rs::draw_primitive`, even if keys
    exist. "Object to path" therefore never shows a marker that was not set on
    a path (criterion 23 holds: no build writes marker keys on a primitive).

- **2026-10-08: (3) painting: markers are the stroke's own layer (criteria 5,
  17).** `artwork.rs::draw_stroke` appends the marker triangles to the
  stroke's draw list, in the same colour (`paint(stroke.color,
  stroke.opacity)`), before `extend_artwork`. Stroke and markers are then one
  depth layer: each pixel is written once, so a 50 % black stroke and its arrow
  overlap with no darker area, and the next object still covers both. No new
  shader, no vertex change, no extra draw call. Rejected: a separate marker
  layer (double blending where they overlap) and an offscreen pass per object
  (one render pass each). Nothing is drawn when the stroke is not drawn (off,
  alpha 0), so criterion 3 holds by construction.

- **2026-10-08: (4) editing, hit-testing and the commands.**
  - The `ui-core::style_panel` view shows the Markers group when the stroke
    rows are shown and the scope holds at least one path; marker fields use
    `BarValue` for mixed over the paths only. A marker edit targets the path
    ids of the scope (criterion 22). `Document::edit_style` refuses a marker
    edit that names a primitive, as a whole (`StyleEditError::NotAPath`), so
    the rule is enforced in the core, not only by the caller.
  - Copy, Split, Join, closing and "Object to path" need no new code: copy is
    structural, Split writes the full `Style`, Join keeps the survivor's meta,
    the ends of a closed path draw nothing, and primitives hold no marker keys
    (criteria 23, 25 to 27). Each gets a test.
  - **Hit-testing, bounds, eyedropper:** nothing changes. `hit_test_object`,
    `object_bounds`, the marquee and `colour_pick` read the outline only, so
    markers are excluded by construction (criteria 18, 20). The Select tool's
    hollow preview draws no style, so no markers (criterion 19). The Node
    tool's live drag substitutes paths in the artwork pass, so markers follow
    the drag. Tests pin each.

- **2026-10-08: (5) performance budget.** Markers are rebuilt every frame with
  the rest of the artwork (no cache exists yet). Limits: at most 500 markers
  per slot of one object, and **50 000 markers per frame** (a `MarkerBudget`
  next to `DashBudget`); past it, further markers are skipped, not the stroke.
  An arrow is one triangle; a dot is a `lyon` circle at display tolerance.
  Spaced markers need one `PathMeasurements` per object (none for ends and
  nodes). PR 1 extends the `#[ignore]` benchmark `style_read_cost.rs` with 200
  paths carrying Start and End arrows and 10 spaced dots; the 25 ms per frame at
  rest line of 0007 stays the threshold for moving the draw-list cache forward.

- **2026-10-08: (6) PR split.**
  1. **Model, format and drawing** (`document-core`, `render-core`): types,
     codec, validation, `StyleEdit` variants, format bump, fixtures, the two
     modules, the layer rule, benchmark. Starts after `style-panel-rework` PR 2
     merges; may run in parallel with its PR 3 or 4 (different crates).
     Criteria 4 to 17, 21, 23, 25 to 29 (storage and drawing).
  2. **Panel** (`ui-core`, `editor-wasm`, frontend): the Markers group,
     Place, Count as a value field, mixed, visibility, `NotAPath` routing.
     Starts after `style-panel-rework` PR 3 (value field) and PR 4. Criteria 1
     to 3, 18 to 20, 22, 24, 30 to 32 (32: the closed-path muted line is a flag
     of the `ui-core::style_panel` view, Start or End set and every path in
     scope closed; the host owns the text). The customer demo follows this PR.

## Flagged to the lead

1. **PO edit:** criterion 29 as in decision 1 (refused as newer version; count
   below 1 is damaged). Everything else is buildable as written.
2. **No customer question** from the architecture; open questions 1 to 6 stay
   the PO's, and every default there is buildable (B of question 1 would only
   drop the `NotAPath` refusal and the `draw_primitive` skip).
3. **`docs/technical-debt.md`, in PR 1:** "marker placement lives in
   `render-core` at display tolerance; SVG export of Spaced markers (no SVG
   equivalent) or a manufacturing use moves it to a core crate both can reach".
