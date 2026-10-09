//! The Curvyo draw-list builder (ADR 0001 §4, ADR 0011 §1): document
//! snapshot + view transform + decoration input → flat draw list,
//! tessellated with `lyon` (`specs/0002-path-node-editing/adrs.md`).
//!
//! Pure and wasm-compatible (`CLAUDE.md` §6): no filesystem, network,
//! clock, threads, UI or GPU access — `curvyo-editor-wasm` owns the
//! `wgpu` device/surface and uploads this crate's output as a vertex
//! buffer, applying the same [`curvyo_document_core::ViewTransform`]
//! uniformly to every vertex. Depends on `curvyo-document-core` only
//! (ADR 0011 §3): it cannot read `curvyo-ui-core`'s selection directly,
//! which is why [`DecorationInput`] exists.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod artwork;
mod color;
mod dash;
mod decorations;
mod document_area;
mod fill;
mod glyphs;
mod live_preview;
mod marker_place;
mod markers;
mod marquee_overlay;
mod move_axes;
mod pen_preview;
mod refusal_outline;
mod select_box;
mod select_decoration;
mod shape_preview;
mod stroke;
mod theme;

pub use artwork::build_artwork;
pub use color::RgbaColor;
pub use decorations::{DecorationInput, Hovered};
pub use document_area::{background_at, build_document_area};
pub use glyphs::{DrawList, Vertex};
pub use live_preview::build_live_edit_preview;
pub use marquee_overlay::{MarqueeOverlay, build_marquee_overlay};
pub use move_axes::{LockedAxis, MoveAxes, build_move_axes};
pub use pen_preview::build_pen_preview;
pub use refusal_outline::build_refusal_outlines;
pub use select_box::{SelectDecorationInput, SelectionBox};
pub use select_decoration::{TransformDecorationInput, TransformGlyphKind, TransformHandleGlyph};
pub use shape_preview::build_shape_live_preview;
pub use theme::{CANVAS_BG, PASTEBOARD_BG};

use curvyo_document_core::{PathSnapshot, ViewTransform};

fn screen_px_to_mm(view: ViewTransform, px: f64) -> f64 {
    px / view.scale()
}

/// Builds the Node tool's decorations alone (nodes, handles, hover rings and
/// the selected-segment overlay) for `paths`, drawn over the artwork.
#[must_use]
pub fn build_decorations(
    paths: &[PathSnapshot],
    view: ViewTransform,
    input: &DecorationInput,
) -> DrawList {
    decorations::build(paths, view, input)
}

/// Builds the Select tool's own decoration geometry for this frame: a
/// plain bounding box per selected/hovered object, no shape handles, no
/// path nodes (acceptance criteria 14, 15, 20;
/// `specs/0004-canvas-navigation-and-selection/specification.md`'s "a new,
/// unified 'selected' indicator").
#[must_use]
pub fn build_select_draw_list(view: ViewTransform, input: &SelectDecorationInput) -> DrawList {
    select_box::build(view, input)
}

/// Builds the Select tool's own transform-handle overlay for this frame
/// (acceptance criteria 1, 14-17, 22 of `specs/0005-object-transform/
/// specification.md`): the 8 resize + 1 rotate handles of a single
/// selected object, plus the active pivot marker during a drag.
#[must_use]
pub fn build_transform_draw_list(
    view: ViewTransform,
    input: &TransformDecorationInput,
) -> DrawList {
    select_decoration::build_transform_handles(view, input)
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, Document, NewAnchor, ObjectSnapshot, Point};

    use super::*;

    #[test]
    fn build_artwork_draws_a_path_and_decorations_are_separate() {
        let document = Document::new(1);
        let path = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(20.0, 0.0)),
            ],
            false,
        );
        let snapshot = document.path(path).expect("exists");
        let artwork = build_artwork(
            &[ObjectSnapshot::Path(snapshot.clone())],
            ViewTransform::identity(),
        );
        assert_ne!(artwork.triangles.len(), 0);
        let decorations = build_decorations(
            &[snapshot],
            ViewTransform::identity(),
            &DecorationInput::default(),
        );
        assert_eq!(decorations.triangles.len(), 0, "no nodes unless asked for");
    }

    #[test]
    fn an_empty_document_produces_an_empty_draw_list() {
        let list = build_artwork(&[], ViewTransform::identity());
        assert_eq!(list.triangles.len(), 0);
    }
}
