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

/// `--node-stroke`: a corner/smooth node glyph's outline, both states.
pub const NODE_STROKE: RgbaColor = RgbaColor::opaque(0x3A, 0x3A, 0x3F);

/// Node glyph size, screen-space pixels (7×7, `docs/design-system.md`).
pub const NODE_SIZE_PX: f64 = 7.0;

/// Node glyph outline thickness, screen-space pixels. Not a separate
/// design-system token (the table gives only the glyph's overall size);
/// 1px is this crate's own reasonable minimum for a visible outline.
pub const NODE_OUTLINE_PX: f64 = 1.0;

/// Handle endpoint circle diameter, screen-space pixels (`docs/design-
/// system.md`; 2026-10-05: doubled from 6px — customer feedback called
/// the handles "hard to hit... and very delicate/thin". Node glyphs
/// ([`NODE_SIZE_PX`]) are deliberately untouched — the customer called
/// out handles specifically, not nodes. The hit-test radius around a
/// handle doubles alongside this in `vecmanf-ui-core::hit_test` — a
/// visual-only change here would look right but still feel exactly as
/// hard to hit.
pub const HANDLE_DIAMETER_PX: f64 = 12.0;

/// Handle line weight, screen-space pixels.
pub const HANDLE_LINE_WIDTH_PX: f64 = 1.0;

/// How much wider than the path's own stroke the selected-segment overlay
/// is drawn, screen-space pixels.
pub const SEGMENT_OVERLAY_EXTRA_PX: f64 = 2.0;

/// The hover ring's diameter, screen-space pixels ("a 10px circle",
/// `docs/design-system.md`'s UX notes are in `specification.md`; the
/// token table itself does not repeat this one, so it is named here).
/// Used for a node's own hover ring (bigger than [`NODE_SIZE_PX`]'s 7px,
/// so it still rings the glyph) and the pen tool's close-target/
/// most-recently-placed-node rings — every ring drawn around a *node*,
/// never a handle (see [`HANDLE_HOVER_RING_DIAMETER_PX`]).
pub const HOVER_RING_DIAMETER_PX: f64 = 10.0;

/// The handle hover ring's own diameter, screen-space pixels — a
/// separate token from [`HOVER_RING_DIAMETER_PX`] (2026-10-05): once
/// [`HANDLE_DIAMETER_PX`] doubled to 12px it exceeded the shared 10px
/// ring, so a selected handle's hover ring drew fully behind (and so
/// completely hidden by) the handle's own opaque fill — no hover cue at
/// all, breaking `specs/0002-path-node-editing/specification.md`'s hover
/// feedback. Sized relative to the handle glyph it rings, the same
/// margin `HOVER_RING_DIAMETER_PX` already keeps over `NODE_SIZE_PX`
/// (a few px larger, never computed from it) — `vecmanf-render-core`'s
/// `decorations::build` draws every handle hover ring after the handle's
/// own glyph, with this diameter, so it reads as a ring around it either
/// way; keeping both independently correct (size *and* draw order) is
/// deliberate, not redundant — either alone would have been enough to
/// fix this, but a future, unrelated glyph-size change should not be
/// able to silently reopen this exact bug.
pub const HANDLE_HOVER_RING_DIAMETER_PX: f64 = HANDLE_DIAMETER_PX + 4.0;

/// How many straight segments approximate one handle/hover circle.
/// Coarse on purpose: these are small, flat-colored UI glyphs, not
/// document geometry, so no [`vecmanf_document_core::Tolerance`] applies.
pub const CIRCLE_SEGMENTS: usize = 16;

/// `--shape-handle-fill`/`--shape-handle-stroke` idle state
/// (`docs/design-system.md`, `primitive-shapes`): white fill, accent
/// outline — same two-tone construction as a node glyph's idle state.
pub const SHAPE_HANDLE_STROKE: RgbaColor = ACCENT;

/// Shape handle size, screen-space pixels (8×8, `docs/design-system.md`
/// — deliberately larger than [`NODE_SIZE_PX`] so the two glyph
/// vocabularies never read as the same control).
pub const SHAPE_HANDLE_SIZE_PX: f64 = 8.0;

/// Shape handle outline thickness, screen-space pixels — same
/// reasoning as [`NODE_OUTLINE_PX`].
pub const SHAPE_HANDLE_OUTLINE_PX: f64 = 1.0;

/// The primitive bounding-box selection/hover outline's weight,
/// screen-space pixels (`docs/design-system.md`'s "Bounding-box
/// selection outline").
pub const BOUNDING_BOX_OUTLINE_PX: f64 = 1.0;

/// One dash's length, screen-space pixels, for the corner-radius
/// connecting guide (`docs/design-system.md`: "dashed `--accent-hover`
/// line").
pub const GUIDE_DASH_PX: f64 = 4.0;

/// The gap between two dashes, screen-space pixels.
pub const GUIDE_GAP_PX: f64 = 3.0;

/// A shape tool's live, uncommitted preview outline weight,
/// screen-space pixels (`specs/0003-primitive-shapes/specification.md`'s
/// "Live creation feedback": "screen-space-constant stroke weight") —
/// distinct from the committed placeholder stroke's document-mm
/// weight (acceptance criterion 16), since nothing has committed yet.
pub const LIVE_PREVIEW_STROKE_PX: f64 = 1.5;
