//! The decoration tokens this slice seeds (`docs/design-system.md`,
//! "Color tokens" and "Spacing and sizing"). This crate is the single
//! place that turns those token *values* into actual draw-list geometry;
//! nothing else in the workspace hardcodes a decoration color or size.

use crate::color::RgbaColor;

/// `--canvas-bg` (`docs/design-system.md`): used as the pen tool's
/// in-progress node glyphs' inner cutout, so they read as hollow/
/// outline-only rather than filled (`specification.md`'s UX notes) —
/// the committed-path idle glyph uses white instead (`decorations.rs`),
/// since this is the one place that distinction matters.
pub const CANVAS_BG: RgbaColor = RgbaColor::opaque(0xE8, 0xE8, 0xEB);

/// `--accent` (`docs/design-system.md`): the one selection/active color.
pub const ACCENT: RgbaColor = RgbaColor::opaque(0x2F, 0x6F, 0xEE);

/// `--accent-hover`: `--accent` at 20% opacity.
pub const ACCENT_HOVER: RgbaColor = ACCENT.with_alpha(51); // 20% of 255, rounded

/// `--axis-guide`: `--accent` at 50%, the origin axis line an axis-locked move
/// runs along (`edit-interaction-polish` criterion 27). Faint on purpose.
pub const AXIS_GUIDE: RgbaColor = ACCENT.with_alpha(128);

/// `--axis-guide-idle`: `--accent-hover`, the origin axis line the locked move
/// is not along.
pub const AXIS_GUIDE_IDLE: RgbaColor = ACCENT_HOVER;

/// `--node-stroke`: a corner/smooth node glyph's outline, both states.
pub const NODE_STROKE: RgbaColor = RgbaColor::opaque(0x3A, 0x3A, 0x3F);

/// Node glyph size, screen-space pixels (14×14, `docs/design-system.md`;
/// 2026-10-05: doubled from 7px — customer feedback: "you can click on
/// the nodes too — the node squares and diamonds need to be bigger too,"
/// the same complaint and the same fix already applied to
/// [`HANDLE_DIAMETER_PX`] one round earlier, now extended to nodes. The
/// node hit-test radius doubles alongside this in
/// `curvyo-editor-wasm::session::POINT_TOLERANCE_PX`, same reasoning as
/// the handle doc comment above: a visual-only change would look right
/// but still feel exactly as hard to hit.
pub const NODE_SIZE_PX: f64 = 14.0;

/// Node glyph outline thickness, screen-space pixels. Not a separate
/// design-system token (the table gives only the glyph's overall size);
/// 1px is this crate's own reasonable minimum for a visible outline.
pub const NODE_OUTLINE_PX: f64 = 1.0;

/// Handle endpoint circle diameter, screen-space pixels (`docs/design-
/// system.md`; 2026-10-05: doubled from 6px — customer feedback called
/// the handles "hard to hit... and very delicate/thin". Node glyphs
/// ([`NODE_SIZE_PX`]) were deliberately left untouched in *this* round —
/// the customer called out handles specifically, not nodes — but a later
/// 2026-10-05 follow-up request did the same for nodes too, so that
/// distinction no longer holds; see `NODE_SIZE_PX`'s own doc comment. The
/// hit-test radius around a handle doubles alongside this in
/// `curvyo-ui-core::hit_test` — a visual-only change here would look
/// right but still feel exactly as hard to hit.
pub const HANDLE_DIAMETER_PX: f64 = 12.0;

/// Handle line weight, screen-space pixels.
pub const HANDLE_LINE_WIDTH_PX: f64 = 1.0;

/// How much wider than the path's own stroke the selected-segment overlay
/// is drawn, screen-space pixels.
pub const SEGMENT_OVERLAY_EXTRA_PX: f64 = 2.0;

/// The hover ring's diameter, screen-space pixels (originally "a 10px
/// circle" per `docs/design-system.md`'s UX notes in `specification.md`;
/// now computed from [`NODE_SIZE_PX`] — 2026-10-05: once `NODE_SIZE_PX`
/// doubled to 14px it exceeded the old fixed 10px value, which would
/// have reopened exactly the "ring hidden behind the glyph's own opaque
/// fill" bug [`HANDLE_HOVER_RING_DIAMETER_PX`]'s doc comment describes
/// for handles, one round earlier, for nodes this time. Same margin
/// (+4px) that constant keeps over [`HANDLE_DIAMETER_PX`], so a future
/// `NODE_SIZE_PX` change cannot silently reopen this again. Used for a
/// node's own hover ring and the pen tool's close-target/most-recently-
/// placed-node rings — every ring drawn around a *node*, never a handle
/// (see [`HANDLE_HOVER_RING_DIAMETER_PX`]).
pub const HOVER_RING_DIAMETER_PX: f64 = NODE_SIZE_PX + 4.0;

/// The handle hover ring's own diameter, screen-space pixels — a
/// separate token from [`HOVER_RING_DIAMETER_PX`] (2026-10-05): once
/// [`HANDLE_DIAMETER_PX`] doubled to 12px it exceeded the shared 10px
/// ring of the time, so a selected handle's hover ring drew fully behind
/// (and so completely hidden by) the handle's own opaque fill — no hover
/// cue at all, breaking `specs/0002-path-node-editing/specification.md`'s
/// hover feedback. A dedicated token rather than deriving this one from
/// `HANDLE_DIAMETER_PX` the same way `HOVER_RING_DIAMETER_PX` now derives
/// from `NODE_SIZE_PX` (both followed the same "+4px margin" reasoning
/// when sized, just not the same mechanism) — `curvyo-render-core`'s
/// `decorations::build` draws every handle hover ring after the handle's
/// own glyph, with this diameter, so it reads as a ring around it either
/// way; keeping both independently correct (size *and* draw order) is
/// deliberate, not redundant — either alone would have been enough to
/// fix this, but a future, unrelated glyph-size change should not be
/// able to silently reopen this exact bug.
pub const HANDLE_HOVER_RING_DIAMETER_PX: f64 = HANDLE_DIAMETER_PX + 4.0;

/// How many straight segments approximate one handle/hover circle.
/// Coarse on purpose: these are small, flat-colored UI glyphs, not
/// document geometry, so no [`curvyo_document_core::Tolerance`] applies.
pub const CIRCLE_SEGMENTS: usize = 16;

/// `--shape-handle-fill`/`--shape-handle-stroke` idle state
/// (`docs/design-system.md`, `primitive-shapes`): white fill, accent
/// outline — same two-tone construction as a node glyph's idle state.
pub const SHAPE_HANDLE_STROKE: RgbaColor = ACCENT;

/// Shape handle size, screen-space pixels (8×8, `docs/design-system.md`
/// — originally deliberately larger than [`NODE_SIZE_PX`] so the two
/// glyph vocabularies would never read as the same control; no longer
/// true since [`HANDLE_DIAMETER_PX`]'s 2026-10-05 doubling to 12px, and
/// now even less true since `NODE_SIZE_PX`'s own 2026-10-05 doubling to
/// 14px made it the *larger* of the two — this token itself was not
/// revisited either time (`docs/design-system.md`'s own token-table row
/// notes this). The two vocabularies still read as distinct by shape
/// (square/diamond vs. hollow square) and color convention, not by
/// relative size.
pub const SHAPE_HANDLE_SIZE_PX: f64 = 8.0;

/// The primitive bounding-box selection/hover outline's weight,
/// screen-space pixels (`docs/design-system.md`'s "Bounding-box
/// selection outline").
pub const BOUNDING_BOX_OUTLINE_PX: f64 = 1.0;

/// The Select tool's own transform resize handle (`object-transform`),
/// screen-space pixels (`docs/design-system.md`'s "Transform resize
/// handle": "8×8px screen-space... Same footprint as the
/// `primitive-shapes` shape handle on purpose").
pub const TRANSFORM_RESIZE_HANDLE_SIZE_PX: f64 = SHAPE_HANDLE_SIZE_PX;

/// The transform resize handle's own outline thickness — same
/// reasoning as the node glyph's outline.
pub const TRANSFORM_RESIZE_HANDLE_OUTLINE_PX: f64 = 1.0;

/// The transform resize handle's corner radius, screen-space pixels
/// (`docs/design-system.md`: "8×8px screen-space, 2px corner radius").
pub const TRANSFORM_RESIZE_HANDLE_CORNER_RADIUS_PX: f64 = 2.0;

/// The rotate handle's arc stroke thickness, screen-space pixels — this
/// crate's own choice (the design-system row gives only the 12×12px
/// footprint and "circular-arrow icon"), thick enough to read at 12px.
pub const TRANSFORM_ROTATE_HANDLE_STROKE_PX: f64 = 2.0;

/// The rotate handle's own glyph size, screen-space pixels (`docs/
/// design-system.md`'s "Transform rotate handle": "12×12px... circular-
/// arrow icon glyph").
///
/// The resize/rotate handles' own hit-test radii and the rotate
/// handle's 20px screen offset (`docs/design-system.md`'s own rows) are
/// not constants in this crate: this crate never hit-tests or lays out
/// handles (ADR 0011 §3) — `curvyo-ui-core::transform_handle_layout`
/// and `curvyo-editor-wasm`'s own wiring own those values; this crate
/// only draws a glyph at whatever position it is handed.
pub const TRANSFORM_ROTATE_HANDLE_SIZE_PX: f64 = 12.0;

/// The centre move handle's footprint, screen-space pixels (`docs/
/// design-system.md`'s "Transform center move handle": "16×16px rounded
/// square (3px radius), white fill, 1px `--accent` outline").
pub const TRANSFORM_MOVE_HANDLE_SIZE_PX: f64 = 16.0;

/// The centre move handle's corner radius, screen-space pixels.
pub const TRANSFORM_MOVE_HANDLE_CORNER_RADIUS_PX: f64 = 3.0;

/// The centre move handle's outline thickness, screen-space pixels.
pub const TRANSFORM_MOVE_HANDLE_OUTLINE_PX: f64 = 1.0;

/// The four-way arrow inside the centre move handle: its width,
/// screen-space pixels ("a four-way arrow 10px wide inside (1.5px stroke)").
pub const TRANSFORM_MOVE_ARROW_SIZE_PX: f64 = 10.0;

/// Stroke of the arrows inside the move and skew handle glyphs, screen-space
/// pixels (1.5 px in `docs/design-system.md`).
pub const TRANSFORM_ARROW_STROKE_PX: f64 = 1.5;

/// Arrowhead length of the move and skew handle glyphs, screen-space
/// pixels (3 px in `docs/design-system.md`).
pub const TRANSFORM_ARROW_HEAD_PX: f64 = 3.0;

/// The skew handle's footprint along its side, screen-space pixels
/// (`docs/design-system.md`'s "Transform skew handle": "18×12px").
pub const TRANSFORM_SKEW_HANDLE_LENGTH_PX: f64 = 18.0;

/// The skew handle's footprint across its side, screen-space pixels.
pub const TRANSFORM_SKEW_HANDLE_WIDTH_PX: f64 = 12.0;

/// The skew handle's hover and dragging ground corner radius, screen-space
/// pixels (3 px in `docs/design-system.md`).
pub const TRANSFORM_SKEW_HANDLE_CORNER_RADIUS_PX: f64 = 3.0;

/// The skew fixed-line guide's color: `--transform-guide`, `--accent` at
/// full opacity (`--accent-hover` at 20% was invisible, about 1.1:1 on the
/// canvas, `docs/design-system.md`).
pub const TRANSFORM_SKEW_GUIDE_COLOR: RgbaColor = ACCENT;

// The skew fixed-line guide's weight is `BOUNDING_BOX_OUTLINE_PX`: the guide
// takes the box's own line and snap (`edit-interaction-polish` criterion 68).

/// The pivot marker's diameter, screen-space pixels (`docs/design-
/// system.md`'s "Transform pivot marker").
pub const TRANSFORM_PIVOT_MARKER_SIZE_PX: f64 = 6.0;

/// The pivot marker's own color — `--accent` at 60% opacity (`docs/
/// design-system.md`), a third opacity tier alongside [`ACCENT`]'s own
/// full-opacity and [`ACCENT_HOVER`]'s 20%.
pub const TRANSFORM_PIVOT_MARKER_COLOR: RgbaColor = ACCENT.with_alpha(153); // 60% of 255, rounded

/// One dash's length, screen-space pixels, for the corner-radius
/// connecting guide (`docs/design-system.md`: "dashed `--accent-hover`
/// line").
pub const GUIDE_DASH_PX: f64 = 4.0;

/// The gap between two dashes, screen-space pixels.
pub const GUIDE_GAP_PX: f64 = 3.0;

/// One dash's length of the skew fixed-line guide, screen-space pixels
/// (`edit-interaction-polish` criterion 68: 2 on / 2 off, so the guide does
/// not look like the 4 / 3 selection box it runs along).
pub const SKEW_GUIDE_DASH_PX: f64 = 2.0;

/// The gap between two dashes of the skew fixed-line guide, screen-space
/// pixels.
pub const SKEW_GUIDE_GAP_PX: f64 = 2.0;

/// The selection box's nominal dash length, screen-space pixels
/// (`edit-interaction-polish` criterion 63, the customer's V1). A dash is
/// never longer than this.
pub const SELECTION_BOX_DASH_PX: f64 = 4.0;

/// The selection box's nominal gap, screen-space pixels. The fit picks the
/// gap nearest this within [`SELECTION_BOX_GAP_MIN_PX`] and
/// [`SELECTION_BOX_GAP_MAX_PX`].
pub const SELECTION_BOX_GAP_PX: f64 = 3.0;

/// The smallest gap of a fitted selection-box edge, screen-space pixels.
/// Where no exact fit exists the gap is this and the dash flexes
/// (`edit-interaction-polish` criterion 64).
pub const SELECTION_BOX_GAP_MIN_PX: f64 = 2.0;

/// The largest gap of a fitted selection-box edge, screen-space pixels.
pub const SELECTION_BOX_GAP_MAX_PX: f64 = 4.0;

/// An edge shorter than this has room for no two dashes and a gap and is
/// drawn solid, screen-space pixels (`edit-interaction-polish` criterion 64).
pub const SELECTION_BOX_MIN_DASHED_EDGE_PX: f64 = 10.0;

/// How far from the skew guide's line both ends of a selection-box edge may
/// lie for the edge to count as on it, screen pixels (the snap moves a box
/// edge by at most half a device pixel).
pub const SELECTION_BOX_GUIDE_TOLERANCE_PX: f64 = 1.0;

/// An edge longer than this is drawn solid, screen-space pixels: it bounds
/// the draw list at an absurd zoom, where the edge is far off screen anyway.
pub const SELECTION_BOX_MAX_DASHED_EDGE_PX: f64 = 50_000.0;

/// `--marquee-touch`: the marquee box in touch mode and the lasso line
/// (`advanced-selection`, `docs/design-system.md`).
pub const MARQUEE_TOUCH: RgbaColor = RgbaColor::opaque(0x2F, 0xAE, 0x57);

/// `--marquee-contain`: the marquee box in contain mode.
pub const MARQUEE_CONTAIN: RgbaColor = RgbaColor::opaque(0xE5, 0x48, 0x4D);

/// The alpha of `--marquee-touch-fill` and `--marquee-contain-fill`: 12 % of
/// 255, rounded.
pub const MARQUEE_FILL_ALPHA: u8 = 31;

/// The marquee border's and the lasso line's weight, screen-space pixels.
pub const MARQUEE_STROKE_PX: f64 = 1.5;

/// The lasso line's dash length, screen-space pixels (4 on, 3 off).
pub const LASSO_DASH_PX: f64 = 4.0;

/// The lasso line's gap, screen-space pixels.
pub const LASSO_GAP_PX: f64 = 3.0;

/// The curve-approximation display tolerance for stroking, in screen
/// pixels (`specs/0004-canvas-navigation-and-selection/adrs.md`: "Display
/// tolerance becomes screen-space... Use 0.25 px / scale. ADR 0003 §7's
/// 'separate, coarser than the kernel' still holds"). A fixed millimetre
/// tolerance (`stroke.rs`'s old `DISPLAY_TOLERANCE_MM = 0.05`) was a 15px
/// chord error at 8000% zoom — visible facets exactly where acceptance
/// criterion 7 promises precise node placement; this screen-space value
/// keeps curves visually smooth at every zoom level instead.
pub const DISPLAY_TOLERANCE_PX: f64 = 0.25;

/// The minimum width, screen-space pixels, any committed stroke ever
/// renders at (`adrs.md`'s flag 2, default (a)): "draw every document
/// stroke at least 1 screen px wide, for display only". Below this, the
/// default 0.25mm stroke's sub-pixel triangles can drop out entirely at
/// low zoom (2%: 0.019px wide), turning acceptance criterion 7's "overview
/// a full sheet" into an empty-looking canvas. Display only — the stored
/// width is never touched.
pub const MIN_DISPLAY_STROKE_WIDTH_PX: f64 = 1.0;

/// A shape tool's live, uncommitted preview outline weight,
/// screen-space pixels (`specs/0003-primitive-shapes/specification.md`'s
/// "Live creation feedback": "screen-space-constant stroke weight") —
/// distinct from the committed placeholder stroke's document-mm
/// weight (acceptance criterion 16), since nothing has committed yet.
pub const LIVE_PREVIEW_STROKE_PX: f64 = 1.5;

/// `--preview-new` (`docs/design-system.md`): the "new" half of blue-new,
/// black-old, the hollow outline of the geometry a release would commit. An
/// alias of [`ACCENT`], named so the preview can be re-coloured without
/// touching the selection colour. The "old" half has no token: it is the
/// committed object in its own style.
pub const PREVIEW_NEW: RgbaColor = ACCENT;

/// A parameter handle's knob, screen-space pixels: a 10 px circle
/// (`docs/design-system.md`, "Parameter handle"). `curvyo-ui-core`'s own
/// `KNOB_DIAMETER_PX` is the same number, which its 4 px clearance property
/// is derived from; a test here guards that no glyph grows past it.
pub const PARAM_KNOB_DIAMETER_PX: f64 = 10.0;

/// The knob's `--shape-handle-stroke` ring, screen-space pixels.
pub const PARAM_KNOB_RING_PX: f64 = 1.5;

/// The knob's centre dot, screen-space pixels.
pub const PARAM_KNOB_DOT_PX: f64 = 4.0;

/// `--shape-handle-guide` (`docs/design-system.md`): the dashed guide from a
/// rectangle corner to its hovered or dragged radius handle, `--accent` at
/// 60%. `--accent-hover` at 20% measured about 1.1:1 on the canvas and was
/// invisible.
pub const SHAPE_HANDLE_GUIDE: RgbaColor = ACCENT.with_alpha(153); // 60% of 255, rounded

/// `--selection-casing` (`docs/design-system.md`, "Casing over artwork"): the
/// white drawn under every `--accent` editor line that has no white ground of
/// its own, so it stays visible over a fill of any colour.
pub const SELECTION_CASING: RgbaColor = RgbaColor::WHITE;

/// A casing is the line's width plus one line width on each side: three times
/// the line width (`docs/design-system.md`, "Casing over artwork").
pub const CASING_WIDTH_FACTOR: f64 = 3.0;

/// `--hover-box` (`0007` criterion 41): the Select tool's hover box,
/// `--accent` at 65%. Raised from `--accent-hover`'s 20%, which measured 1.0
/// to 1.3:1 on every fill. `--accent-hover` stays for rings, buttons and rows.
pub const HOVER_BOX: RgbaColor = ACCENT.with_alpha(166); // 65% of 255, rounded

/// The hover box's casing: `--selection-casing` at the same 65%. Both must be
/// 65%: with the line at 65% over a 60% casing, red stays at 1.96:1.
pub const HOVER_BOX_CASING: RgbaColor = SELECTION_CASING.with_alpha(166);

/// One side of the pivot marker's casing, screen-space pixels: the marker is
/// a dot, not a line, so its casing is a fixed ring one pixel wide.
pub const PIVOT_MARKER_CASING_PX: f64 = 1.0;
