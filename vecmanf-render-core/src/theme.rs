//! The decoration tokens this slice seeds (`docs/design-system.md`,
//! "Color tokens" and "Spacing and sizing"). This crate is the single
//! place that turns those token *values* into actual draw-list geometry;
//! nothing else in the workspace hardcodes a decoration color or size.

use crate::color::RgbaColor;

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

/// Handle endpoint circle diameter, screen-space pixels.
pub const HANDLE_DIAMETER_PX: f64 = 6.0;

/// Handle line weight, screen-space pixels.
pub const HANDLE_LINE_WIDTH_PX: f64 = 1.0;

/// How much wider than the path's own stroke the selected-segment overlay
/// is drawn, screen-space pixels.
pub const SEGMENT_OVERLAY_EXTRA_PX: f64 = 2.0;

/// The hover ring's diameter, screen-space pixels ("a 10px circle",
/// `docs/design-system.md`'s UX notes are in `specification.md`; the
/// token table itself does not repeat this one, so it is named here).
pub const HOVER_RING_DIAMETER_PX: f64 = 10.0;

/// How many straight segments approximate one handle/hover circle.
/// Coarse on purpose: these are small, flat-colored UI glyphs, not
/// document geometry, so no [`vecmanf_document_core::Tolerance`] applies.
pub const CIRCLE_SEGMENTS: usize = 16;
