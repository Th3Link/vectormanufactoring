//! The cost of writing and reading a big boolean result
//! (`specs/0016-boolean-operations/adrs.md`: "PR 2 measures writing and
//! reading a 20,000-anchor compound path before it merges, because the
//! encoding is the hard-to-change part"; criteria 46 and 47).
//!
//! Budgets are asserted in release builds only; a debug build runs a tenth of
//! the anchors (the unoptimised Loro takes seconds) and prints the numbers. Run: `cargo test --release -p curvyo-document-core --test
//! compound_write_cost -- --nocapture`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::cast_precision_loss)]

use std::time::{Duration, Instant};

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, NodeId, Point, RectBounds, pack, unpack,
};

/// `count` outlines of `per_outline` anchors each: concentric polygons on a
/// circle of growing radius, alternately wound.
fn outlines(count: usize, per_outline: usize) -> Vec<(Vec<NewAnchor>, bool)> {
    (0..count)
        .map(|o| {
            let radius = 10.0 + 5.0 * o as f64;
            let anchors = (0..per_outline)
                .map(|k| {
                    let angle = std::f64::consts::TAU * k as f64 / per_outline as f64;
                    let angle = if o % 2 == 1 { -angle } else { angle };
                    NewAnchor::corner(
                        AnchorId::new(9, (o * per_outline + k) as u64),
                        Point::new(radius * angle.cos(), radius * angle.sin()),
                    )
                })
                .collect();
            (anchors, true)
        })
        .collect()
}

fn path_object(document: &Document, node: NodeId) -> curvyo_document_core::ObjectSnapshot {
    document.object(node).unwrap()
}

fn operands(document: &Document, count: usize) -> Vec<NodeId> {
    (0..count)
        .map(|i| {
            document.create_rect(RectBounds {
                origin: Point::new((i % 40) as f64 * 5.0, (i / 40) as f64 * 5.0),
                width: Length::from_mm(8.0),
                height: Length::from_mm(8.0),
            })
        })
        .collect()
}

fn check(label: &str, took: Duration, budget: Duration) {
    println!("{label}: {took:?}");
    if !cfg!(debug_assertions) {
        assert!(took < budget, "{label} took {took:?}, budget {budget:?}");
    }
}

/// Anchors per outline: 5,000 in release (four outlines make 20,000), a tenth
/// of that in debug.
const PER_OUTLINE: usize = if cfg!(debug_assertions) { 500 } else { 5_000 };

/// Writing a compound path of 20,000 anchors (four outlines of 5,000), and
/// reading it back, saving and reopening it.
#[test]
fn a_compound_path_of_twenty_thousand_anchors() {
    let document = Document::new(1);
    let base = operands(&document, 2);
    let result = outlines(4, PER_OUTLINE);

    let started = Instant::now();
    let node = document
        .replace_with_path(&base, base[0], &result, "boolean_union")
        .unwrap();
    check(
        "write 20,000 anchors",
        started.elapsed(),
        Duration::from_secs(1),
    );

    let started = Instant::now();
    let path = document.path(node).unwrap();
    check(
        "read the snapshot",
        started.elapsed(),
        Duration::from_millis(150),
    );
    assert_eq!(path.all_anchors().count(), 4 * PER_OUTLINE);

    // Edits of the result: rotate and resize write every anchor, once. They
    // look each anchor up by id, which must not cost a scan per anchor.
    let rotated = path_object(&document, node).rotated(
        Point::new(0.0, 0.0),
        curvyo_document_core::Angle::from_radians(0.4),
    );
    let started = Instant::now();
    document.rotate_object(&rotated).unwrap();
    check(
        "rotate the result",
        started.elapsed(),
        Duration::from_millis(300),
    );

    let scaled = document
        .path(node)
        .unwrap()
        .scaled(Point::new(0.0, 0.0), 1.5, 0.5);
    let anchors: Vec<_> = scaled
        .all_anchors()
        .map(|a| (a.id, a.point, a.handle_in, a.handle_out))
        .collect();
    let started = Instant::now();
    document.resize_path(node, &anchors, None).unwrap();
    check(
        "resize the result",
        started.elapsed(),
        Duration::from_millis(300),
    );
    assert_eq!(
        document.path(node).unwrap().all_anchors().count(),
        4 * PER_OUTLINE
    );

    let started = Instant::now();
    let bytes = pack(&document, "0.1.0").unwrap();
    check("save", started.elapsed(), Duration::from_secs(2));
    println!("file size: {} bytes", bytes.len());

    let started = Instant::now();
    let reopened = unpack(2, &bytes).unwrap();
    check("reopen", started.elapsed(), Duration::from_secs(2));
    assert_eq!(
        reopened.path(node).unwrap(),
        document.path(node).unwrap(),
        "the edits survive a save"
    );
}

/// Criterion 46: 1,000 operands replaced by one result in one commit; the
/// document half of the budget (the kernel is measured in
/// `curvyo-geometry-core`).
#[test]
fn replacing_a_thousand_operands() {
    let document = Document::new(1);
    let ids = operands(&document, 1_000);
    let result = outlines(1, 8 * PER_OUTLINE / 10);

    let started = Instant::now();
    document
        .replace_with_path(&ids, ids[0], &result, "boolean_union")
        .unwrap();
    check(
        &format!(
            "replace 1,000 objects by one of {} anchors",
            8 * PER_OUTLINE / 10
        ),
        started.elapsed(),
        Duration::from_millis(500),
    );
    assert_eq!(document.object_ids().len(), 1);
}
