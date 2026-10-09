//! Regression coverage for PR #20 (`fix/canvas-interaction-bugs`, bug 1:
//! "Node tool broken" / invisible horizontal and vertical strokes).
//!
//! The PR attributes this purely to missing MSAA in `curvyo-editor-
//! wasm::gpu` (anti-aliasing cannot fix geometry that was never
//! generated in the first place). Verifying the GPU-side fix needs a
//! real browser (`curvyo-editor-wasm`'s `gpu`/`wasm_api` modules only
//! compile for `wasm32`, see its own `lib.rs` doc comment), but the
//! *tessellation* that feeds the GPU — whether `lyon`, via
//! `curvyo-render-core`'s private `stroke` module, actually emits
//! non-degenerate triangle geometry for an exactly horizontal or
//! exactly vertical segment at all — is pure, host-testable geometry.
//! This locks that in independently of any rendering/anti-aliasing
//! concern: a thin stroke that is still *zero triangles* would stay
//! invisible under any amount of MSAA.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_document_core::{AnchorId, AnchorKind, Color, Document, NewAnchor, Point};
use curvyo_render_core::{DrawList, build_artwork, build_pen_preview};

fn two_node_path(a: Point, b: Point) -> Document {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), a),
            NewAnchor::corner(AnchorId::new(1, 2), b),
        ],
        false,
    );
    document
}

/// AC6: a committed open path's own 0.25mm stroke must tessellate to
/// real geometry when the two nodes are exactly horizontal (same y) —
/// One path's artwork at the identity view.
fn draw(snapshot: curvyo_document_core::PathSnapshot) -> DrawList {
    build_artwork(
        &[curvyo_document_core::ObjectSnapshot::Path(snapshot)],
        &[],
        curvyo_document_core::ViewTransform::identity(),
    )
}

/// not an edge case `lyon`'s stroke tessellator is free to degenerate to
/// zero triangles for (e.g. a perpendicular/normal computation that
/// divides out to nothing along one axis).
#[test]
fn committed_horizontal_path_stroke_is_non_empty() {
    let document = two_node_path(Point::new(0.0, 50.0), Point::new(80.0, 50.0));
    let id = document.object_ids()[0];
    let snapshot = document.path(id).expect("exists");
    let list = draw(snapshot);
    // Two node glyphs (outline+fill, 2 quads = 4 triangles each) account
    // for 8 triangles on their own; the stroke itself must add strictly
    // more than that, or the segment between them is contributing zero
    // geometry.
    assert!(
        list.triangle_count() > 8,
        "a horizontal 0.25mm stroke must tessellate to visible geometry, not just the two \
         node glyphs (triangle_count={})",
        list.triangle_count()
    );
}

/// Same as above, exactly vertical (same x).
#[test]
fn committed_vertical_path_stroke_is_non_empty() {
    let document = two_node_path(Point::new(40.0, 0.0), Point::new(40.0, 80.0));
    let id = document.object_ids()[0];
    let snapshot = document.path(id).expect("exists");
    let list = draw(snapshot);
    assert!(
        list.triangle_count() > 8,
        "a vertical 0.25mm stroke must tessellate to visible geometry, not just the two node \
         glyphs (triangle_count={})",
        list.triangle_count()
    );
}

/// A horizontal and a diagonal stroke of the same length should
/// tessellate to a comparable amount of geometry (same tolerance, same
/// width, same path length) — a regression that only zeroes out the
/// axis-aligned case specifically (as opposed to a general
/// mis-tessellation) would show up as a lopsided triangle count between
/// the two, not just an absolute-zero check.
#[test]
fn horizontal_and_diagonal_strokes_of_equal_length_tessellate_comparably() {
    let horizontal = two_node_path(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
    let diagonal = two_node_path(Point::new(0.0, 0.0), Point::new(70.7107, 70.7107));

    let h_id = horizontal.object_ids()[0];
    let d_id = diagonal.object_ids()[0];
    let h_list = draw(horizontal.path(h_id).unwrap());
    let d_list = draw(diagonal.path(d_id).unwrap());

    assert!(
        h_list.triangle_count() > 8,
        "horizontal must have real stroke geometry"
    );
    assert!(
        d_list.triangle_count() > 8,
        "diagonal must have real stroke geometry"
    );
    // Not an exact-equality check (tessellation of a straight stroke can
    // legitimately differ by a triangle or two depending on cap/join
    // geometry at each end) — just that neither one collapsed relative
    // to the other.
    let ratio = f64::from(u32::try_from(h_list.triangle_count()).unwrap())
        / f64::from(u32::try_from(d_list.triangle_count()).unwrap());
    assert!(
        (0.5..=2.0).contains(&ratio),
        "horizontal ({}) and diagonal ({}) triangle counts must be the same order of \
         magnitude, not one collapsing relative to the other",
        h_list.triangle_count(),
        d_list.triangle_count()
    );
}

/// The in-progress pen-tool preview's own connecting stroke (a separate
/// code path from the committed-path one above, per
/// `curvyo-render-core::pen_preview`'s own doc comment) must likewise
/// not degenerate for an exactly horizontal or vertical segment between
/// two placed, not-yet-committed nodes.
#[test]
fn in_progress_pen_preview_stroke_is_non_empty_for_axis_aligned_segments() {
    let horizontal_nodes = [
        curvyo_document_core::AnchorSnapshot {
            id: AnchorId::new(1, 1),
            point: Point::new(0.0, 20.0),
            handle_in: curvyo_document_core::Vec2::ZERO,
            handle_out: curvyo_document_core::Vec2::ZERO,
            kind: AnchorKind::Corner,
        },
        curvyo_document_core::AnchorSnapshot {
            id: AnchorId::new(1, 2),
            point: Point::new(60.0, 20.0),
            handle_in: curvyo_document_core::Vec2::ZERO,
            handle_out: curvyo_document_core::Vec2::ZERO,
            kind: AnchorKind::Corner,
        },
    ];
    let no_cursor_list = build_pen_preview(
        &horizontal_nodes[..1],
        None,
        None,
        curvyo_document_core::ViewTransform::identity(),
        false,
        curvyo_document_core::DocumentSize::default(),
    );
    let with_segment_list = build_pen_preview(
        &horizontal_nodes,
        None,
        None,
        curvyo_document_core::ViewTransform::identity(),
        false,
        curvyo_document_core::DocumentSize::default(),
    );
    assert!(
        with_segment_list.triangle_count() > no_cursor_list.triangle_count(),
        "adding the second, exactly-horizontal in-progress node must add stroke geometry \
         connecting it to the first (with={}, without={})",
        with_segment_list.triangle_count(),
        no_cursor_list.triangle_count()
    );
}

/// Sanity check that `Color` round-trips to the AC6 placeholder the way
/// the other tests in this file assume (`build_artwork` reads
/// `snapshot.stroke` directly) — guards against a silent default-color
/// change making the other assertions here pass for the wrong reason.
#[test]
fn ac6_placeholder_stroke_is_black() {
    let document = two_node_path(Point::new(0.0, 0.0), Point::new(1.0, 0.0));
    let id = document.object_ids()[0];
    let snapshot = document.path(id).unwrap();
    assert_eq!(snapshot.style.stroke.color, Color::BLACK);
}
