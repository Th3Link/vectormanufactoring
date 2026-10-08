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
mod fill;
mod glyphs;
mod live_preview;
mod move_axes;
mod pen_preview;
mod select_box;
mod select_decoration;
mod shape_preview;
mod stroke;
mod theme;

pub use artwork::build_artwork;
pub use color::RgbaColor;
pub use decorations::{DecorationInput, Hovered};
pub use glyphs::{DrawList, Vertex};
pub use live_preview::build_live_edit_preview;
pub use move_axes::{LockedAxis, MoveAxes, build_move_axes};
pub use pen_preview::build_pen_preview;
pub use select_box::{SelectDecorationInput, SelectionBox};
pub use select_decoration::{TransformDecorationInput, TransformGlyphKind, TransformHandleGlyph};
pub use shape_preview::{build_primitive_strokes, build_shape_live_preview};

use curvyo_document_core::{PathSnapshot, ViewTransform};

fn screen_px_to_mm(view: ViewTransform, px: f64) -> f64 {
    px / view.scale()
}

/// Builds the draw list of `paths` as the Node tool shows them: every path's
/// artwork (fill, then stroke, in the style it carries), plus node, handle and
/// segment decorations (acceptance criteria 7, 9, 10, 14 of `path-node-
/// editing`) from `input`. The whole document is drawn with [`build_artwork`]
/// and [`build_decorations`] separately; this is the two combined for paths.
#[must_use]
pub fn build_draw_list(
    paths: &[PathSnapshot],
    view: ViewTransform,
    input: &DecorationInput,
) -> DrawList {
    let mut list = artwork::paths_artwork(paths, view);
    list.extend(decorations::build(paths, view, input));
    list
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
    use curvyo_document_core::{AnchorId, Document, NewAnchor, Point};

    use super::*;

    #[test]
    fn build_draw_list_includes_both_stroke_and_decorations() {
        let document = Document::new(1);
        let path = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let list = build_draw_list(
            &paths,
            ViewTransform::identity(),
            &DecorationInput::default(),
        );
        assert_ne!(list.triangles.len(), 0);
    }

    #[test]
    fn an_empty_document_produces_an_empty_draw_list() {
        let list = build_draw_list(&[], ViewTransform::identity(), &DecorationInput::default());
        assert_eq!(list.triangles.len(), 0);
    }
}
