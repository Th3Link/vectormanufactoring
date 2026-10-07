//! Black-box acceptance tests for `specs/0003-primitive-shapes/
//! specification.md`'s 22 acceptance criteria, written against
//! `vecmanf-document-core`'s public API only (`Document`,
//! `RectBounds`/`EllipseFrame`/`StarFrame`, `PointCount`/`InnerRatio`,
//! the `primitive_outline` functions, `pack`/`unpack`), before reading
//! the implementation diff.
//!
//! This crate is pure byte-level/geometry code with no UI, so the parts
//! of criteria 1, 2, 7, 8, 11, 12 that describe "the maker drags the
//! mouse" are exercised at the `vecmanf-ui-core` level instead
//! (`vecmanf-ui-core/tests/acceptance_0003.rs`); here they are tested as
//! "given these two points, what does the stored shape look like" plus
//! this crate's own newtype/codec/outline guarantees.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use loro::LoroDoc;
use vecmanf_document_core::{
    AnchorKind, Document, EllipseFrame, InnerRatio, Length, ObjectSnapshot, OpenError, PointCount,
    RectBounds, Shape, ShapeEditError, ShapeParamError, StarFrame, outline_of, pack, unpack,
};

fn pt(x: f64, y: f64) -> vecmanf_document_core::Point {
    vecmanf_document_core::Point::new(x, y)
}

// ---------------------------------------------------------------------
// AC1-6: rectangle
// ---------------------------------------------------------------------

#[test]
fn ac1_rect_bounds_from_any_two_opposite_corners_normalizes_and_is_zero_radius() {
    let document = Document::new(1);
    let bounds = RectBounds::from_corners(pt(30.0, 10.0), pt(10.0, 40.0));
    let id = document.create_rect(bounds);
    let primitive = document.primitive(id).expect("rect exists");
    match primitive.shape {
        Shape::Rect {
            bounds,
            corner_radius,
        } => {
            assert_eq!(bounds.origin, pt(10.0, 10.0));
            assert!((bounds.width.as_mm() - 20.0).abs() < 1e-9);
            assert!((bounds.height.as_mm() - 30.0).abs() < 1e-9);
            assert!((corner_radius.as_mm() - 0.0).abs() < 1e-9);
        }
        _ => panic!("expected a rect"),
    }
}

#[test]
fn ac1_degenerate_drag_is_not_the_document_s_job_to_create_but_bounds_math_still_well_defined() {
    // AC1's "A = B creates nothing" is a tool-level (ui-core) rule; at
    // this layer we only confirm `RectBounds::is_degenerate` exists and
    // agrees, since `vecmanf-ui-core`'s tool relies on it.
    assert!(RectBounds::is_degenerate(pt(5.0, 5.0), pt(5.0, 5.0)));
}

#[test]
fn ac3_resizing_a_rect_keeps_corner_radius_untouched() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(40.0, 40.0)));
    document
        .set_corner_radius(&[id], Length::from_mm(5.0))
        .unwrap();
    document
        .set_rect_bounds(id, RectBounds::from_corners(pt(0.0, 0.0), pt(100.0, 100.0)))
        .unwrap();
    let Shape::Rect { corner_radius, .. } = document.primitive(id).unwrap().shape else {
        panic!("expected rect");
    };
    assert!((corner_radius.as_mm() - 5.0).abs() < 1e-9);
}

#[test]
fn ac4_and_ac18_rounding_produces_8_corner_tangent_points() {
    let bounds = RectBounds::from_corners(pt(0.0, 0.0), pt(40.0, 20.0));
    let outline = vecmanf_document_core::rect_outline(bounds, Length::from_mm(4.0));
    assert_eq!(outline.len(), 8);
    assert!(
        outline.iter().all(|a| a.kind == AnchorKind::Corner),
        "AC18: every rounded-rect anchor must be Corner, never Smooth"
    );
}

#[test]
fn ac5_radius_is_clamped_to_half_the_shorter_side() {
    // Shorter side is height = 20mm, so half is 10mm; asking for 999mm
    // must clamp to exactly 10mm, not overlap/self-intersect.
    let bounds = RectBounds::from_corners(pt(0.0, 0.0), pt(100.0, 20.0));
    let effective = vecmanf_document_core::effective_corner_radius(bounds, Length::from_mm(999.0));
    assert!((effective.as_mm() - 10.0).abs() < 1e-9);
}

#[test]
fn ac5_clamp_is_exact_at_the_boundary_not_past_it() {
    let bounds = RectBounds::from_corners(pt(0.0, 0.0), pt(10.0, 10.0)); // square, half-side = 5
    let exact = vecmanf_document_core::effective_corner_radius(bounds, Length::from_mm(5.0));
    let over = vecmanf_document_core::effective_corner_radius(bounds, Length::from_mm(5.0001));
    assert!((exact.as_mm() - 5.0).abs() < 1e-9);
    assert!((over.as_mm() - 5.0).abs() < 1e-9);
}

#[test]
fn ac6_removing_rounding_returns_to_exactly_zero() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(40.0, 40.0)));
    document
        .set_corner_radius(&[id], Length::from_mm(8.0))
        .unwrap();
    document
        .set_corner_radius(&[id], Length::from_mm(0.0))
        .unwrap();
    let Shape::Rect { corner_radius, .. } = document.primitive(id).unwrap().shape else {
        panic!("expected rect");
    };
    assert!(corner_radius.as_mm().abs() < 1e-9);
    let outline = outline_of(&document.primitive(id).unwrap().shape);
    assert_eq!(outline.len(), 4);
    assert!(outline.iter().all(|a| a.kind == AnchorKind::Corner));
}

/// Architect decision pinned in `adrs.md`: "the corner radius is stored
/// raw and clamped where it is evaluated" — shrinking a rounded
/// rectangle and growing it back restores the original radius.
#[test]
fn radius_is_stored_raw_shrink_then_regrow_restores_original_radius() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(100.0, 100.0)));
    document
        .set_corner_radius(&[id], Length::from_mm(40.0))
        .unwrap();

    // Shrink so the shorter side is 20mm (half = 10mm): effective radius
    // must clamp down to 10mm, but the stored register stays 40mm.
    document
        .set_rect_bounds(id, RectBounds::from_corners(pt(0.0, 0.0), pt(100.0, 20.0)))
        .unwrap();
    let shrunk = document.primitive(id).unwrap();
    let Shape::Rect {
        bounds: shrunk_bounds,
        corner_radius: raw_radius,
    } = shrunk.shape
    else {
        panic!("expected rect");
    };
    assert!(
        (raw_radius.as_mm() - 40.0).abs() < 1e-9,
        "raw value must not be clamped on write"
    );
    let effective_while_shrunk =
        vecmanf_document_core::effective_corner_radius(shrunk_bounds, raw_radius);
    assert!((effective_while_shrunk.as_mm() - 10.0).abs() < 1e-9);

    // Grow back to the original size: the original 40mm radius must
    // reappear (not get stuck at the clamped 10mm).
    document
        .set_rect_bounds(id, RectBounds::from_corners(pt(0.0, 0.0), pt(100.0, 100.0)))
        .unwrap();
    let regrown = document.primitive(id).unwrap();
    let Shape::Rect {
        bounds: regrown_bounds,
        corner_radius: regrown_radius,
    } = regrown.shape
    else {
        panic!("expected rect");
    };
    let effective_after_regrow =
        vecmanf_document_core::effective_corner_radius(regrown_bounds, regrown_radius);
    assert!((regrown_radius.as_mm() - 40.0).abs() < 1e-9);
    assert!((effective_after_regrow.as_mm() - 40.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------
// AC7-9: ellipse
// ---------------------------------------------------------------------

#[test]
fn ac7_ellipse_frame_from_corners_centers_and_halves() {
    let document = Document::new(1);
    let frame = EllipseFrame::from_corners(pt(0.0, 0.0), pt(20.0, 40.0));
    let id = document.create_ellipse(frame);
    let Shape::Ellipse { frame } = document.primitive(id).unwrap().shape else {
        panic!("expected ellipse");
    };
    assert_eq!(frame.center, pt(10.0, 20.0));
    assert!((frame.rx.as_mm() - 10.0).abs() < 1e-9);
    assert!((frame.ry.as_mm() - 20.0).abs() < 1e-9);
}

#[test]
fn ac9_circle_can_be_reshaped_into_a_non_circular_ellipse_after_creation() {
    let document = Document::new(1);
    // A "circle" created under AC8 is simply rx == ry.
    let id = document.create_ellipse(EllipseFrame {
        center: pt(0.0, 0.0),
        rx: Length::from_mm(10.0),
        ry: Length::from_mm(10.0),
    });
    document
        .set_ellipse_frame(
            id,
            EllipseFrame {
                center: pt(0.0, 0.0),
                rx: Length::from_mm(10.0),
                ry: Length::from_mm(25.0),
            },
        )
        .unwrap();
    let Shape::Ellipse { frame } = document.primitive(id).unwrap().shape else {
        panic!("expected ellipse");
    };
    assert!((frame.rx.as_mm() - 10.0).abs() < 1e-9);
    assert!((frame.ry.as_mm() - 25.0).abs() < 1e-9);
}

/// AC19: circle/ellipse-to-path deviates by at most 0.1% of the larger
/// radius from the true circle, using kappa's exact value.
#[test]
fn ac19_ellipse_outline_is_4_smooth_nodes_within_0_1_percent_of_a_true_circle() {
    let frame = EllipseFrame {
        center: pt(0.0, 0.0),
        rx: Length::from_mm(50.0),
        ry: Length::from_mm(50.0),
    };
    let outline = vecmanf_document_core::ellipse_outline(frame);
    assert_eq!(outline.len(), 4);
    assert!(outline.iter().all(|a| a.kind == AnchorKind::Symmetric));

    // Sample the cubic Bezier at t=0.5 between the East and South
    // anchors and confirm it lies within 0.1% of radius from center.
    let east = outline[0];
    let south = outline[1];
    let p0 = east.point;
    let p1 = east.point.translated(east.handle_out);
    let p2 = south.point.translated(south.handle_in);
    let p3 = south.point;
    let t = 0.5_f64;
    let mt = 1.0 - t;
    let x = mt.powi(3) * p0.x
        + 3.0 * mt.powi(2) * t * p1.x
        + 3.0 * mt * t.powi(2) * p2.x
        + t.powi(3) * p3.x;
    let y = mt.powi(3) * p0.y
        + 3.0 * mt.powi(2) * t * p1.y
        + 3.0 * mt * t.powi(2) * p2.y
        + t.powi(3) * p3.y;
    let dist = (x * x + y * y).sqrt();
    let deviation = (dist - 50.0).abs() / 50.0;
    assert!(
        deviation < 0.001,
        "deviation {deviation} must be < 0.1% of the larger radius"
    );
}

#[test]
fn ac19_ellipse_outline_handles_are_mirrored_and_equal_length_smooth_handles() {
    let frame = EllipseFrame {
        center: pt(3.0, -7.0),
        rx: Length::from_mm(12.0),
        ry: Length::from_mm(8.0),
    };
    for anchor in vecmanf_document_core::ellipse_outline(frame) {
        assert_eq!(anchor.kind, AnchorKind::Symmetric);
        assert!(
            (anchor.handle_in.x + anchor.handle_out.x).abs() < 1e-9
                && (anchor.handle_in.y + anchor.handle_out.y).abs() < 1e-9,
            "a Smooth node's handles must be mirrored (handle_in = -handle_out)"
        );
    }
}

// ---------------------------------------------------------------------
// AC10: point count newtype + crash-safety open refusal
// ---------------------------------------------------------------------

#[test]
fn ac10_point_count_accepts_only_3_to_1024_inclusive() {
    assert!(PointCount::new(3).is_ok());
    assert!(PointCount::new(1024).is_ok());
    assert_eq!(
        PointCount::new(2).unwrap_err(),
        ShapeParamError::PointCountOutOfRange
    );
    assert_eq!(
        PointCount::new(1025).unwrap_err(),
        ShapeParamError::PointCountOutOfRange
    );
    assert_eq!(
        PointCount::new(0).unwrap_err(),
        ShapeParamError::PointCountOutOfRange
    );
    assert_eq!(
        PointCount::new(u32::MAX).unwrap_err(),
        ShapeParamError::PointCountOutOfRange
    );
}

/// Builds a `.vmf`'s bytes carrying one polygon whose on-disk
/// `point_count` is `raw_point_count` — a value that cannot be reached
/// through this crate's own public API (`PointCount::new` would refuse
/// it), simulating a hand-crafted or foreign-writer damaged file.
fn craft_vmf_with_raw_point_count(raw_point_count: i64) -> Vec<u8> {
    let document = Document::new(1);
    let frame = StarFrame::from_center_and_vertex(pt(50.0, 50.0), pt(60.0, 50.0));
    let id = document.create_polygon(frame, PointCount::new(6).unwrap());
    let good_bytes = document.export_loro_snapshot().unwrap();

    // Re-import into a bare `loro::LoroDoc` and overwrite `point_count`
    // directly on the meta map, bypassing every validated newtype this
    // crate's own writers go through.
    let loro = LoroDoc::new();
    loro.import(&good_bytes).unwrap();
    let tree = loro.get_tree("paths"); // documented on-disk tree key (adrs.md)
    let nodes = tree.nodes();
    assert_eq!(nodes.len(), 1, "exactly one polygon node");
    let meta = tree.get_meta(nodes[0]).unwrap();
    meta.insert("point_count", raw_point_count).unwrap();
    loro.commit();
    let bad_loro_bytes = loro.export(loro::ExportMode::Snapshot).unwrap();

    // Re-use `document`'s `NodeId` only to satisfy the type checker for
    // `pack`'s signature is avoided entirely: `pack` always re-derives
    // `document.json` from the live `Document`, but since `unpack` never
    // reads `document.json` to decide whether to refuse, a stale one is
    // fine here.
    let _ = id;
    build_vmf_zip(&bad_loro_bytes)
}

fn build_vmf_zip(loro_bytes: &[u8]) -> Vec<u8> {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    let manifest = serde_json::json!({
        "format_version": 3,
        "loro_snapshot_version": 1,
        "app_version": "tester-crafted-fixture",
    });
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file("manifest.json", options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer.start_file("document.loro", options).unwrap();
    writer.write_all(loro_bytes).unwrap();
    writer.start_file("document.json", options).unwrap();
    writer.write_all(b"{}").unwrap();
    writer.finish().unwrap().into_inner()
}

#[test]
fn ac10_a_vmf_with_point_count_above_1024_is_refused_as_damaged_not_a_crash() {
    let bytes = craft_vmf_with_raw_point_count(2000);
    let result = unpack(2, &bytes);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "expected OpenError::Damaged"
    );
}

#[test]
fn ac10_a_vmf_with_point_count_below_3_is_refused_as_damaged() {
    let bytes = craft_vmf_with_raw_point_count(2);
    let result = unpack(2, &bytes);
    assert!(matches!(result, Err(OpenError::Damaged)));
}

#[test]
fn ac10_a_vmf_with_a_billion_point_count_is_refused_not_an_oom_abort() {
    // The crash-safety case `adrs.md`'s "Flagged to the lead" names
    // explicitly: a 10^9 point count must never reach allocation.
    let bytes = craft_vmf_with_raw_point_count(1_000_000_000);
    let result = unpack(2, &bytes);
    assert!(matches!(result, Err(OpenError::Damaged)));
}

#[test]
fn ac10_a_vmf_with_point_count_exactly_at_the_valid_boundaries_still_opens() {
    let bytes_min = craft_vmf_with_raw_point_count(3);
    let bytes_max = craft_vmf_with_raw_point_count(1024);
    assert!(unpack(2, &bytes_min).is_ok());
    assert!(unpack(2, &bytes_max).is_ok());
}

/// Re-verification (tester, item 5): a `point_count` stored as a decimal
/// (e.g. `5.7`, a plausible mistyped-parameter case, not merely an
/// out-of-range integer) must be refused as `Damaged`, not silently
/// truncated to `5` or coerced in any other way. Bypasses the normal
/// write API by overwriting the meta map's `point_count` key directly
/// with a Loro f64 value, the same technique
/// `craft_vmf_with_raw_point_count` uses for an out-of-range integer.
#[test]
fn ac10_a_vmf_with_a_decimal_point_count_is_refused_as_damaged_not_truncated() {
    let document = Document::new(1);
    let frame = StarFrame::from_center_and_vertex(pt(50.0, 50.0), pt(60.0, 50.0));
    let _id = document.create_polygon(frame, PointCount::new(6).unwrap());
    let good_bytes = document.export_loro_snapshot().unwrap();

    let loro = LoroDoc::new();
    loro.import(&good_bytes).unwrap();
    let tree = loro.get_tree("paths");
    let nodes = tree.nodes();
    assert_eq!(nodes.len(), 1, "exactly one polygon node");
    let meta = tree.get_meta(nodes[0]).unwrap();
    // A decimal, not an integer: if this were silently truncated to 5
    // rather than refused, the file would open with a 5-point polygon
    // instead of being flagged damaged.
    meta.insert("point_count", 5.7_f64).unwrap();
    loro.commit();
    let bad_loro_bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let bytes = build_vmf_zip(&bad_loro_bytes);

    let result = unpack(2, &bytes);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "a decimal point_count must refuse as Damaged"
    );
}

/// Re-verification (tester, item 5): a `shape` tag stored as something
/// other than a string (here, an integer) must be refused as `Damaged`
/// outright — not silently read as "absent" (which would misread the
/// primitive as an ordinary path and likely panic or desync on its
/// missing `anchors`/`closed` fields).
#[test]
fn ac_a_vmf_with_a_non_string_shape_tag_is_refused_as_damaged() {
    let document = Document::new(1);
    let bounds = RectBounds::from_corners(pt(0.0, 0.0), pt(10.0, 10.0));
    let _id = document.create_rect(bounds);
    let good_bytes = document.export_loro_snapshot().unwrap();

    let loro = LoroDoc::new();
    loro.import(&good_bytes).unwrap();
    let tree = loro.get_tree("paths");
    let nodes = tree.nodes();
    assert_eq!(nodes.len(), 1, "exactly one rect node");
    let meta = tree.get_meta(nodes[0]).unwrap();
    // Overwrite the `shape` key (normally the string "rect") with a
    // non-string value.
    meta.insert("shape", 42_i64).unwrap();
    loro.commit();
    let bad_loro_bytes = loro.export(loro::ExportMode::Snapshot).unwrap();
    let bytes = build_vmf_zip(&bad_loro_bytes);

    let result = unpack(2, &bytes);
    assert!(
        matches!(result, Err(OpenError::Damaged)),
        "a non-string shape tag must refuse as Damaged"
    );
}

// ---------------------------------------------------------------------
// AC11-15: polygon/star
// ---------------------------------------------------------------------

#[test]
fn ac11_polygon_center_and_one_vertex_at_b_remaining_evenly_spaced() {
    let document = Document::new(1);
    let frame = StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0));
    let id = document.create_polygon(frame, PointCount::new(4).unwrap());
    let outline = outline_of(&document.primitive(id).unwrap().shape);
    assert_eq!(outline.len(), 4);
    // One vertex must coincide with B = (10, 0).
    assert!(
        outline
            .iter()
            .any(|a| (a.point.x - 10.0).abs() < 1e-9 && a.point.y.abs() < 1e-9)
    );
    // All vertices equidistant from the center (regular polygon).
    for anchor in &outline {
        let r = (anchor.point.x.powi(2) + anchor.point.y.powi(2)).sqrt();
        assert!((r - 10.0).abs() < 1e-9);
    }
}

#[test]
fn ac12_star_outer_and_inner_vertices_alternate_at_the_given_ratio() {
    let document = Document::new(1);
    let frame = StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0));
    let id = document.create_star(
        frame,
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.4).unwrap(),
    );
    let outline = outline_of(&document.primitive(id).unwrap().shape);
    assert_eq!(outline.len(), 10, "2N for N=5");
    for (i, anchor) in outline.iter().enumerate() {
        let r = (anchor.point.x.powi(2) + anchor.point.y.powi(2)).sqrt();
        if i % 2 == 0 {
            assert!(
                (r - 10.0).abs() < 1e-6,
                "even index must be an outer vertex"
            );
        } else {
            assert!(
                (r - 4.0).abs() < 1e-6,
                "odd index must be an inner vertex at R*outer"
            );
        }
    }
}

#[test]
fn ac12_inner_ratio_rejects_the_open_interval_endpoints() {
    assert!(InnerRatio::new(0.01).is_ok());
    assert!(InnerRatio::new(0.99).is_ok());
    assert_eq!(
        InnerRatio::new(0.0).unwrap_err(),
        ShapeParamError::InnerRatioOutOfRange
    );
    assert_eq!(
        InnerRatio::new(1.0).unwrap_err(),
        ShapeParamError::InnerRatioOutOfRange
    );
}

#[test]
fn ac13_scaling_a_star_keeps_point_count_and_ratio_the_relationship() {
    let document = Document::new(1);
    let frame = StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0));
    let id = document.create_star(
        frame,
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let new_frame = StarFrame {
        center: frame.center,
        radius: Length::from_mm(20.0),
        angle: frame.angle,
    };
    document.set_star_frame(id, new_frame).unwrap();
    let Shape::Star {
        frame,
        point_count,
        inner_ratio,
    } = document.primitive(id).unwrap().shape
    else {
        panic!("expected star");
    };
    assert!((frame.radius.as_mm() - 20.0).abs() < 1e-9);
    assert_eq!(point_count.get(), 5);
    assert!((inner_ratio.get() - 0.5).abs() < 1e-9);
}

#[test]
fn ac14_changing_ratio_keeps_outer_radius_fixed_and_a_polygon_refuses_the_operation() {
    let document = Document::new(1);
    let frame = StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0));
    let star_id = document.create_star(
        frame,
        PointCount::new(6).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    document
        .set_inner_ratio(&[star_id], InnerRatio::new(0.8).unwrap())
        .unwrap();
    let Shape::Star {
        frame: after,
        inner_ratio,
        ..
    } = document.primitive(star_id).unwrap().shape
    else {
        panic!("expected star");
    };
    assert!((after.radius.as_mm() - 10.0).abs() < 1e-9);
    assert!((inner_ratio.get() - 0.8).abs() < 1e-9);

    let polygon_id = document.create_polygon(frame, PointCount::new(6).unwrap());
    let result = document.set_inner_ratio(&[polygon_id], InnerRatio::new(0.3).unwrap());
    assert_eq!(result, Err(ShapeEditError::WrongShape));
}

#[test]
fn ac15_point_count_change_keeps_size_ratio_and_orientation() {
    let document = Document::new(1);
    let frame = StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0));
    let id = document.create_star(
        frame,
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    document
        .set_point_count(&[id], PointCount::new(8).unwrap())
        .unwrap();
    let Shape::Star {
        frame: after,
        point_count,
        inner_ratio,
    } = document.primitive(id).unwrap().shape
    else {
        panic!("expected star");
    };
    assert_eq!(point_count.get(), 8);
    assert_eq!(after, frame);
    assert!((inner_ratio.get() - 0.5).abs() < 1e-9);
}

#[test]
fn ac20_polygon_and_star_outlines_are_all_corner_nodes_straight_segments() {
    let frame = StarFrame::from_center_and_vertex(pt(0.0, 0.0), pt(10.0, 0.0));
    let polygon = vecmanf_document_core::polygon_outline(frame, PointCount::new(7).unwrap());
    assert_eq!(polygon.len(), 7);
    assert!(polygon.iter().all(|a| a.kind == AnchorKind::Corner));
    assert!(
        polygon
            .iter()
            .all(|a| a.handle_in == vecmanf_document_core::Vec2::ZERO
                && a.handle_out == vecmanf_document_core::Vec2::ZERO)
    );

    let star = vecmanf_document_core::star_outline(
        frame,
        PointCount::new(7).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    assert_eq!(star.len(), 14);
    assert!(star.iter().all(|a| a.kind == AnchorKind::Corner));
}

// ---------------------------------------------------------------------
// AC17, AC21, AC22: object to path
// ---------------------------------------------------------------------

#[test]
fn ac17_converting_a_rect_replaces_its_parameters_with_only_path_anchors() {
    let document = Document::new(1);
    let bounds = RectBounds::from_corners(pt(0.0, 0.0), pt(20.0, 10.0));
    let id = document.create_rect(bounds);
    let outline = outline_of(&document.primitive(id).unwrap().shape);
    let anchors: Vec<_> = outline
        .into_iter()
        .enumerate()
        .map(|(i, a)| vecmanf_document_core::NewAnchor {
            id: vecmanf_document_core::AnchorId::new(999, i as u64),
            point: a.point,
            handle_in: a.handle_in,
            handle_out: a.handle_out,
            kind: a.kind,
        })
        .collect();
    document.convert_to_paths(&[(id, anchors)]).unwrap();

    assert!(
        document.primitive(id).is_none(),
        "no primitive parameters remain"
    );
    match document.object(id).unwrap() {
        ObjectSnapshot::Path(path) => {
            assert_eq!(path.anchors.len(), 4);
            assert!(path.closed);
        }
        ObjectSnapshot::Primitive(_) => panic!("must be a path after conversion"),
    }
}

#[test]
fn ac17_convert_to_paths_refuses_an_unknown_or_already_converted_id() {
    let document = Document::new(1);
    // `NodeId` has no public constructor (ADR 0004 §3 keeps `loro::TreeID`
    // out of this crate's public API entirely), so a never-minted id is
    // built the same way a non-Rust reader of `document.json` would see
    // one: through `NodeId`'s own public `Deserialize`.
    let bogus: vecmanf_document_core::NodeId =
        serde_json::from_str(r#"{"peer":"999999999","counter":123456}"#).unwrap();
    let result = document.convert_to_paths(&[(bogus, vec![])]);
    assert_eq!(result, Err(ShapeEditError::NoSuchObject));
}

#[test]
fn ac17_convert_to_paths_refuses_an_id_that_is_already_a_path() {
    let document = Document::new(1);
    let id = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(10.0, 10.0)));
    document.convert_to_paths(&[(id, vec![])]).unwrap();
    let result = document.convert_to_paths(&[(id, vec![])]);
    assert_eq!(result, Err(ShapeEditError::NotAPrimitive));
}

#[test]
fn ac22_each_selected_primitive_converts_independently_in_one_call() {
    let document = Document::new(1);
    let rect_id = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(20.0, 10.0)));
    let ellipse_id =
        document.create_ellipse(EllipseFrame::from_corners(pt(0.0, 0.0), pt(10.0, 10.0)));

    let mk_anchors = |id: vecmanf_document_core::NodeId| -> Vec<vecmanf_document_core::NewAnchor> {
        outline_of(&document.primitive(id).unwrap().shape)
            .into_iter()
            .enumerate()
            .map(|(i, a)| vecmanf_document_core::NewAnchor {
                id: vecmanf_document_core::AnchorId::new(777, i as u64),
                point: a.point,
                handle_in: a.handle_in,
                handle_out: a.handle_out,
                kind: a.kind,
            })
            .collect()
    };

    document
        .convert_to_paths(&[
            (rect_id, mk_anchors(rect_id)),
            (ellipse_id, mk_anchors(ellipse_id)),
        ])
        .unwrap();

    for id in [rect_id, ellipse_id] {
        match document.object(id).unwrap() {
            ObjectSnapshot::Path(_) => {}
            ObjectSnapshot::Primitive(_) => panic!("{id:?} must have converted to a path"),
        }
    }
}

// ---------------------------------------------------------------------
// AC16: placeholder rendering default
// ---------------------------------------------------------------------

#[test]
fn ac16_a_freshly_created_primitive_has_the_same_stroke_default_as_a_path() {
    let document = Document::new(1);
    let rect = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(5.0, 5.0)));
    let path = document.create_path(&[], false);
    let rect_snapshot = document.primitive(rect).unwrap();
    let path_snapshot = document.object(path).unwrap();
    let ObjectSnapshot::Path(path_snapshot) = path_snapshot else {
        panic!("expected path")
    };
    assert_eq!(rect_snapshot.stroke, path_snapshot.stroke);
    assert_eq!(rect_snapshot.stroke_width, path_snapshot.stroke_width);
    assert_eq!(rect_snapshot.fill, None);
    assert_eq!(path_snapshot.fill, None);
}

// ---------------------------------------------------------------------
// format_version bump and `objects` rename (ADR 0004 §9 / adrs.md)
// ---------------------------------------------------------------------

#[test]
fn format_version_3_round_trips_through_pack_unpack() {
    let document = Document::new(1);
    let _ = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(10.0, 10.0)));
    let bytes = pack(&document, "test").unwrap();
    let reopened = unpack(2, &bytes).expect("a format_version=3 file must open");
    assert_eq!(reopened.object_ids().len(), 1);
}

#[test]
fn document_json_uses_the_objects_key_not_paths() {
    let document = Document::new(1);
    let _ = document.create_rect(RectBounds::from_corners(pt(0.0, 0.0), pt(10.0, 10.0)));
    let json_bytes = document.export_json().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();
    assert!(value.get("objects").is_some(), "must use the 'objects' key");
    assert!(
        value.get("paths").is_none(),
        "must not keep the old 'paths' key"
    );
}
