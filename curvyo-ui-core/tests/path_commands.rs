//! The pure part of Combine and Break apart (`specs/0035-combine-and-break-apart` criteria 1, 3 to
//! 6, 8 to 11, 13 to 16): availability, plans and refusals.

#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::cast_precision_loss)]

use curvyo_document_core::{
    AnchorId, Document, EllipseFrame, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds,
    StyleEdit,
};
use curvyo_ui_core::{
    AnchorIdMinter, BooleanAvailability, BreakApartRefusal, CombineRefusal, ObjectSelection,
    path_availability, plan_break_apart, plan_combine,
};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(document: &Document, x: f64, y: f64, side: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(side),
        height: Length::from_mm(side),
    })
}

fn disc(document: &Document, x: f64, y: f64, radius: f64) -> NodeId {
    document.create_ellipse(EllipseFrame {
        center: pt(x, y),
        rx: Length::from_mm(radius),
        ry: Length::from_mm(radius),
    })
}

/// A path through `points` with ids from `first`.
fn poly(document: &Document, first: u64, points: &[(f64, f64)], closed: bool) -> NodeId {
    let anchors: Vec<NewAnchor> = points
        .iter()
        .enumerate()
        .map(|(k, &(x, y))| NewAnchor::corner(AnchorId::new(1, first + k as u64), pt(x, y)))
        .collect();
    document.create_path(&anchors, closed)
}

fn objects(document: &Document) -> Vec<ObjectSnapshot> {
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .collect()
}

fn select(ids: &[NodeId]) -> ObjectSelection {
    let mut selection = ObjectSelection::new();
    for &id in ids {
        selection.add(id);
    }
    selection
}

fn minter() -> AnchorIdMinter {
    AnchorIdMinter::new(77)
}

/// A minter of its own peer, for tests that write several plans into one document.
fn minter_of(peer: u64) -> AnchorIdMinter {
    AnchorIdMinter::new(peer)
}

/// The signed shoelace area of a straight-edged outline.
fn area(anchors: &[NewAnchor]) -> f64 {
    let n = anchors.len();
    (0..n)
        .map(|i| {
            let (a, b) = (anchors[i].point, anchors[(i + 1) % n].point);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

/// Two outlines placed as a ring: a plate with a square hole, written through Combine.
fn ring(document: &Document) -> NodeId {
    let plate = rect(document, 0.0, 0.0, 40.0);
    let hole = rect(document, 10.0, 10.0, 20.0);
    let all = objects(document);
    let plan = plan_combine(&all, &select(&[plate, hole]), &mut minter_of(70)).unwrap();
    document
        .replace_with_path(&plan.operands, plan.base, &plan.outlines, "combine_paths")
        .unwrap()
}

#[test]
fn availability_follows_the_selection() {
    let document = Document::new(1);
    let a = rect(&document, 0.0, 0.0, 10.0);
    let b = rect(&document, 20.0, 0.0, 10.0);
    let compound = ring(&document);
    let all = objects(&document);

    let none = path_availability(&all, &ObjectSelection::new());
    assert_eq!(none.combine, BooleanAvailability::NeedsTwo);
    assert!(!none.break_apart);

    let two = path_availability(&all, &select(&[a, b]));
    assert_eq!(two.combine, BooleanAvailability::Ready);
    assert!(!two.break_apart, "no compound path selected");

    let one = path_availability(&all, &select(&[compound]));
    assert_eq!(one.combine, BooleanAvailability::NeedsTwo);
    assert!(one.break_apart, "one compound path is enough");

    let mixed = path_availability(&all, &select(&[a, compound]));
    assert_eq!(mixed.combine, BooleanAvailability::Ready);
    assert!(mixed.break_apart);

    let open = poly(&document, 900, &[(0.0, 0.0), (5.0, 5.0), (9.0, 0.0)], false);
    let all = objects(&document);
    let with_open = path_availability(&all, &select(&[a, open]));
    assert_eq!(
        with_open.combine,
        BooleanAvailability::OpenPaths { open: 1, of: 2 },
        "an open path leaves Combine enabled"
    );
}

/// Criteria 3, 4, 6: the winding follows the depth, whatever direction the shapes were drawn in,
/// and the result takes the bottom-most style and place.
#[test]
fn nested_shapes_become_holes_and_the_bottom_style_wins() {
    for reversed in [false, true] {
        let document = Document::new(1);
        let mut inner = vec![(10.0, 10.0), (30.0, 10.0), (30.0, 30.0), (10.0, 30.0)];
        if reversed {
            inner.reverse();
        }
        let plate = rect(&document, 0.0, 0.0, 40.0);
        let hole = poly(&document, 10, &inner, true);
        let island = rect(&document, 15.0, 15.0, 10.0);
        document
            .edit_style(&[plate], &StyleEdit::StrokeWidth(Length::from_mm(3.0)))
            .unwrap();
        let all = objects(&document);
        let plan = plan_combine(&all, &select(&[island, hole, plate]), &mut minter()).unwrap();

        assert_eq!(plan.operands, vec![plate, hole, island], "stacking order");
        assert_eq!(plan.base, plate);
        assert_eq!(plan.holes, 1);
        assert!(plan.styles_differ);
        let signs: Vec<bool> = plan.outlines.iter().map(|(a, _)| area(a) > 0.0).collect();
        assert_eq!(signs, vec![true, false, true], "shape, hole, island");
    }
}

/// Criterion 5: nothing is flattened. Two circles keep their eight nodes, kinds and handles.
#[test]
fn circles_keep_their_nodes() {
    let document = Document::new(1);
    let a = disc(&document, 0.0, 0.0, 5.0);
    let b = disc(&document, 30.0, 0.0, 5.0);
    let all = objects(&document);
    let plan = plan_combine(&all, &select(&[a, b]), &mut minter()).unwrap();
    assert_eq!(plan.holes, 0);
    assert!(!plan.styles_differ);
    let nodes: usize = plan.outlines.iter().map(|(anchors, _)| anchors.len()).sum();
    assert_eq!(nodes, 8);
    let ObjectSnapshot::Primitive(first) = &all[0] else {
        panic!("a primitive");
    };
    let expected = curvyo_document_core::outline_of_rotated(&first.shape, first.rotation);
    for (anchor, wanted) in plan.outlines[0].0.iter().zip(&expected) {
        assert!(
            anchor.point == wanted.point
                || anchor.handle_in == wanted.handle_out
                || anchor.handle_out == wanted.handle_in
        );
        assert_eq!(anchor.kind, wanted.kind);
    }
}

/// Criterion 8: the same objects give the same plan however they were selected; anchor ids are
/// fresh and different.
#[test]
fn the_selection_order_does_not_matter_and_ids_are_fresh() {
    let document = Document::new(1);
    let a = rect(&document, 0.0, 0.0, 10.0);
    let b = rect(&document, 30.0, 0.0, 10.0);
    let all = objects(&document);
    let one = plan_combine(&all, &select(&[a, b]), &mut minter()).unwrap();
    let two = plan_combine(&all, &select(&[b, a]), &mut minter()).unwrap();
    assert_eq!(one, two);
    let mut ids: Vec<AnchorId> = one
        .outlines
        .iter()
        .flat_map(|(anchors, _)| anchors.iter().map(|a| a.id))
        .collect();
    let total = ids.len();
    ids.sort_by_key(|id| format!("{id:?}"));
    ids.dedup();
    assert_eq!(ids.len(), total);
}

/// Criteria 9, 10, 10a, 11: each refusal lists exactly its offenders.
#[test]
fn refusals_name_their_offenders() {
    let document = Document::new(1);
    let a = rect(&document, 0.0, 0.0, 10.0);
    let overlapping = rect(&document, 5.0, 5.0, 10.0);
    let sharing = rect(&document, 10.0, 0.0, 10.0);
    let apart = rect(&document, 40.0, 0.0, 10.0);
    let near = rect(&document, 50.01, 0.0, 10.0);
    let open = poly(&document, 900, &[(0.0, 0.0), (5.0, 5.0), (9.0, 0.0)], false);
    let eight = poly(
        &document,
        920,
        &[(100.0, 0.0), (110.0, 10.0), (110.0, 0.0), (100.0, 10.0)],
        true,
    );
    let flat = poly(
        &document,
        940,
        &[(0.0, 100.0), (5.0, 100.0), (9.0, 100.0)],
        true,
    );
    let far = poly(
        &document,
        960,
        &[(1.0e8, 0.0), (1.0e8 + 1.0, 0.0), (1.0e8, 1.0)],
        true,
    );
    let all = objects(&document);
    let plan = |ids: &[NodeId]| plan_combine(&all, &select(ids), &mut minter());

    assert_eq!(plan(&[a]), Err(CombineRefusal::NeedsTwo));
    assert_eq!(
        plan(&[a, open, apart]),
        Err(CombineRefusal::OpenPaths {
            offenders: vec![open],
            of: 3
        })
    );
    assert_eq!(
        plan(&[a, far]),
        Err(CombineRefusal::OutOfRange {
            offenders: vec![far],
            of: 2
        })
    );
    assert_eq!(
        plan(&[a, flat]),
        Err(CombineRefusal::NoArea {
            offenders: vec![flat],
            of: 2
        })
    );
    assert_eq!(
        plan(&[a, overlapping, apart]),
        Err(CombineRefusal::Touching {
            offenders: vec![a, overlapping],
            of: 3
        })
    );
    assert_eq!(
        plan(&[a, sharing]),
        Err(CombineRefusal::Touching {
            offenders: vec![a, sharing],
            of: 2
        }),
        "a shared edge touches"
    );
    assert!(plan(&[apart, near]).is_ok(), "0.01 mm apart combine");
    assert_eq!(
        plan(&[a, eight, apart]),
        Err(CombineRefusal::SelfTouching {
            offenders: vec![eight],
            of: 3
        })
    );
    assert_eq!(
        plan(&[a, overlapping, eight]),
        Err(CombineRefusal::Touching {
            offenders: vec![a, overlapping],
            of: 3
        }),
        "the touching sentence comes first"
    );
}

/// Criteria 13 to 16.
#[test]
fn break_apart_keeps_holes_with_their_piece() {
    let document = Document::new(1);
    let outer = rect(&document, 0.0, 0.0, 40.0);
    let hole = rect(&document, 10.0, 10.0, 20.0);
    let island = rect(&document, 15.0, 15.0, 10.0);
    let all = objects(&document);
    let combined = plan_combine(&all, &select(&[outer, hole, island]), &mut minter_of(71)).unwrap();
    let compound = document
        .replace_with_path(
            &combined.operands,
            combined.base,
            &combined.outlines,
            "combine_paths",
        )
        .unwrap();
    let plain = rect(&document, 100.0, 0.0, 10.0);
    let all = objects(&document);

    let broken = plan_break_apart(&all, &select(&[plain, compound]), &mut minter_of(72)).unwrap();
    assert_eq!(broken.parts.len(), 1);
    let (id, pieces) = &broken.parts[0];
    assert_eq!(*id, compound);
    assert_eq!(pieces.len(), 2, "the ring and the island");
    assert_eq!(pieces[0].len(), 2, "the ring has its hole");
    assert_eq!(pieces[1].len(), 1);
    assert_eq!(broken.pieces, 2);
    assert!(broken.with_holes);
    assert_eq!(broken.kept, vec![plain]);
    assert_eq!(broken.left_alone, 0);
    // Nodes and windings are exactly as they were; only the ids are new.
    let ObjectSnapshot::Path(path) = all.iter().find(|o| o.id() == compound).unwrap() else {
        panic!("the compound path");
    };
    let original: Vec<_> = path.subpaths().collect();
    assert_eq!(pieces[0][0].0.len(), original[0].anchors.len());
    for (new, old) in pieces[1][0].0.iter().zip(original[2].anchors) {
        assert_eq!(
            (new.point, new.handle_in, new.handle_out),
            (old.point, old.handle_in, old.handle_out)
        );
        assert_ne!(new.id, old.id);
    }
}

#[test]
fn break_apart_refusals() {
    let document = Document::new(1);
    let ring_id = ring(&document);
    let plain = rect(&document, 100.0, 0.0, 10.0);
    let all = objects(&document);
    assert_eq!(
        plan_break_apart(&all, &select(&[plain]), &mut minter()),
        Err(BreakApartRefusal::NoCompound)
    );
    assert_eq!(
        plan_break_apart(&all, &ObjectSelection::new(), &mut minter()),
        Err(BreakApartRefusal::NoCompound)
    );
    assert_eq!(
        plan_break_apart(&all, &select(&[ring_id, plain]), &mut minter()),
        Err(BreakApartRefusal::OnePiece { compounds: 1 })
    );
}

/// A ring left alone beside a compound that does break apart: counted, kept, selected after.
#[test]
fn one_region_compounds_are_left_as_they_are() {
    let document = Document::new(1);
    let ring_id = ring(&document);
    let a = rect(&document, 100.0, 0.0, 10.0);
    let b = rect(&document, 130.0, 0.0, 10.0);
    let all = objects(&document);
    let pair = plan_combine(&all, &select(&[a, b]), &mut minter_of(73)).unwrap();
    let two = document
        .replace_with_path(&pair.operands, pair.base, &pair.outlines, "combine_paths")
        .unwrap();
    let all = objects(&document);
    let plan = plan_break_apart(&all, &select(&[ring_id, two]), &mut minter()).unwrap();
    assert_eq!(plan.parts.len(), 1);
    assert_eq!(plan.pieces, 2);
    assert!(!plan.with_holes);
    assert_eq!(plan.kept, vec![ring_id]);
    assert_eq!(plan.left_alone, 1);
}

/// Criterion 4: concentric discs, drawn in either stacking order, combine into a ring that the
/// canvas paints (nonzero fill) at radius 15 mm and not at the centre, of the area
/// pi (400 - 100) within 1 mm squared; a third disc inside the hole is painted again.
#[test]
fn concentric_discs_paint_as_a_ring_with_an_island() {
    use curvyo_geometry_core::{Outline, contains_point_in_outlines, outline_area_mm2};

    for inner_first in [false, true] {
        let document = Document::new(1);
        let (outer, inner) = if inner_first {
            let inner = disc(&document, 50.0, 50.0, 10.0);
            (disc(&document, 50.0, 50.0, 20.0), inner)
        } else {
            let outer = disc(&document, 50.0, 50.0, 20.0);
            (outer, disc(&document, 50.0, 50.0, 10.0))
        };
        let island = disc(&document, 50.0, 50.0, 5.0);
        let all = objects(&document);
        let plan = plan_combine(&all, &select(&[outer, inner, island]), &mut minter()).unwrap();
        let triples: Vec<Vec<_>> = plan
            .outlines
            .iter()
            .map(|(anchors, _)| {
                anchors
                    .iter()
                    .map(|a| (a.point, a.handle_in, a.handle_out))
                    .collect()
            })
            .collect();
        let views: Vec<Outline<'_>> = triples.iter().map(|t| Outline::new(t, true)).collect();

        assert!(
            contains_point_in_outlines(&views, pt(65.0, 50.0)),
            "radius 15"
        );
        assert!(
            !contains_point_in_outlines(&views, pt(50.0, 50.0 - 7.5)),
            "radius 7.5, the hole"
        );
        assert!(
            contains_point_in_outlines(&views, pt(50.0, 50.0 - 2.0)),
            "the island"
        );
        let ring: f64 = views[..2].iter().map(outline_area_mm2).sum();
        let expected = std::f64::consts::PI * 300.0;
        assert!(
            (ring - expected).abs() < 1.0,
            "ring area {ring} against {expected}"
        );
    }
}
