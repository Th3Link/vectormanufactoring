//! The vecmanf draw-list builder (ADR 0001 §4, ADR 0011 §1): document
//! snapshot + view transform + decoration input → flat draw list,
//! tessellated with `lyon` (`specs/0002-path-node-editing/adrs.md`).
//!
//! Pure and wasm-compatible (`CLAUDE.md` §6): no filesystem, network,
//! clock, threads, UI or GPU access — `vecmanf-editor-wasm` owns the
//! `wgpu` device/surface and uploads this crate's output as a vertex
//! buffer, applying the same [`vecmanf_document_core::ViewTransform`]
//! uniformly to every vertex. Depends on `vecmanf-document-core` only
//! (ADR 0011 §3): it cannot read `vecmanf-ui-core`'s selection directly,
//! which is why [`DecorationInput`] exists.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod color;
mod decorations;
mod glyphs;
mod pen_preview;
mod shape_preview;
mod stroke;
mod theme;

pub use color::RgbaColor;
pub use decorations::{DecorationInput, Hovered};
pub use glyphs::{DrawList, Vertex};
pub use pen_preview::build_pen_preview;
pub use shape_preview::{
    RenderShapeHandle, ShapeDecorationInput, ShapeHandleKind, build_shape_live_preview,
};

use vecmanf_document_core::{PathSnapshot, PrimitiveSnapshot, ViewTransform};

/// Builds the full draw list for one frame: every path's stroke
/// (acceptance criterion 6), plus node/handle/segment decorations
/// (acceptance criteria 7, 9, 10, 14) from `input`.
#[must_use]
pub fn build_draw_list(
    paths: &[PathSnapshot],
    view: ViewTransform,
    input: &DecorationInput,
) -> DrawList {
    let mut list = DrawList::default();
    for snapshot in paths {
        list.extend(stroke::path_stroke(
            &snapshot.anchors,
            snapshot.closed,
            snapshot.stroke_width.as_mm(),
            snapshot.stroke.into(),
        ));
    }
    list.extend(decorations::build(paths, view, input));
    list
}

/// Builds one frame's primitive-shape geometry: every primitive's own
/// stroke (acceptance criterion 16), plus bounding-box selection/hover
/// and shape-handle decorations, from `input`
/// (`specs/0003-primitive-shapes/specification.md`).
#[must_use]
pub fn build_shape_draw_list(
    primitives: &[PrimitiveSnapshot],
    view: ViewTransform,
    input: &ShapeDecorationInput,
) -> DrawList {
    shape_preview::build(primitives, view, input)
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::{AnchorId, Document, NewAnchor, Point};

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
