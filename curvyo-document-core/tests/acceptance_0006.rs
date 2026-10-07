//! Black-box acceptance tests for `specs/0006-path-merge-split-and-node-
//! types/specification.md`, written against `curvyo-document-core`'s
//! public API only (`Document`, `AnchorKind`, `HandleSlot`, `Vec2`, ...),
//! before reading the implementation diff.
//!
//! In-scope criteria per `specs/0006-.../plan.md`'s own "Deferred" note:
//! 1-5, 8, 10-16 (same-path and the reachable-via-split two-object Join).
//! Criteria 6, 7, and AC9's "two pre-existing different objects" case are
//! explicitly deferred (need `canvas-navigation-and-selection`'s
//! `ObjectSelection`, which does not exist on this branch) and are not
//! tested here as working end-to-end UI flows; the underlying
//! `Document::join_endpoints` two-object code path, however, is in scope
//! and is covered below directly at the document-core API (criterion 9's
//! document-model half).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    AnchorId, AnchorKind, Document, HandleSlot, NewAnchor, PathEditError, Point, Vec2,
};

const EPS: f64 = 1e-9;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn vec_approx_eq(a: Vec2, b: Vec2) -> bool {
    approx_eq(a.x, b.x) && approx_eq(a.y, b.y)
}

fn point_approx_eq(a: Point, b: Point) -> bool {
    vec_approx_eq(a.vector_to(b), Vec2::ZERO)
}

// ---------------------------------------------------------------------
// AC 2: Corner -> Asymmetric
// ---------------------------------------------------------------------

#[test]
fn ac2_corner_to_asymmetric_keeps_each_sides_own_nonzero_length_and_sets_default_for_zero() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    // Straight horizontal 3-node path; b is Corner with a long handle_out
    // and a zero handle_in, so the tangent direction is unambiguous and
    // we can check: handle_out keeps its own (non-zero) length, handle_in
    // gets the default length because it started at zero.
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor {
                id: b,
                point: Point::new(10.0, 0.0),
                handle_in: Vec2::ZERO,
                handle_out: Vec2::new(5.0, 0.0),
                kind: AnchorKind::Corner,
            },
            NewAnchor::corner(c, Point::new(20.0, 0.0)),
        ],
        false,
    );

    document
        .convert_anchor_kind(path, &[b], AnchorKind::Asymmetric)
        .expect("convert");

    let snapshot = document.path(path).expect("path exists");
    let anchor = snapshot.anchors[1];
    assert_eq!(anchor.kind, AnchorKind::Asymmetric);
    // Collinear through the node (tangent rule), each side pointing the
    // opposite direction of the other.
    assert!(vec_approx_eq(
        anchor.handle_out.normalized_to(1.0),
        anchor.handle_in.negated().normalized_to(1.0)
    ));
    // The side that already had a non-zero handle (handle_out, length 5)
    // keeps its own length exactly.
    assert!(approx_eq(anchor.handle_out.length(), 5.0));
    // The side that started at zero (handle_in) got the slice's default
    // length, not zero and not mirrored from the other side's length.
    assert!(anchor.handle_in.length() > EPS);
    assert_ne!(anchor.handle_in.length(), anchor.handle_out.length());
}

#[test]
fn ac2_corner_to_asymmetric_the_two_resulting_lengths_may_differ() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor {
                id: b,
                point: Point::new(10.0, 0.0),
                handle_in: Vec2::new(-3.0, 0.0),
                handle_out: Vec2::new(7.0, 0.0),
                kind: AnchorKind::Corner,
            },
            NewAnchor::corner(c, Point::new(20.0, 0.0)),
        ],
        false,
    );
    document
        .convert_anchor_kind(path, &[b], AnchorKind::Asymmetric)
        .expect("convert");
    let anchor = document.path(path).expect("exists").anchors[1];
    assert!(approx_eq(anchor.handle_in.length(), 3.0));
    assert!(approx_eq(anchor.handle_out.length(), 7.0));
}

// ---------------------------------------------------------------------
// AC 3: dragging one handle of an Asymmetric node — the exact
// distinguishing behaviour vs. Symmetric and Corner. This is the
// behaviour the implementer says they found and fixed a real bug in
// (`Document::set_handle` only mirroring for Symmetric).
// ---------------------------------------------------------------------

#[test]
fn ac3_asymmetric_drag_mirrors_angle_but_not_length() {
    let document = Document::new(1);
    let anchor_id = AnchorId::new(1, 1);
    let path = document.create_path(
        &[NewAnchor {
            id: anchor_id,
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::new(-4.0, 0.0),
            handle_out: Vec2::new(4.0, 0.0),
            kind: AnchorKind::Asymmetric,
        }],
        false,
    );

    // Drag handle_out to a new angle and a new (different) length.
    document
        .set_handle(path, anchor_id, HandleSlot::Out, Vec2::new(0.0, 9.0))
        .expect("set_handle");

    let anchor = document.path(path).expect("exists").anchors[0];
    assert!(vec_approx_eq(anchor.handle_out, Vec2::new(0.0, 9.0)));

    // The opposite handle (handle_in) must rotate to stay exactly
    // opposite (collinear through the node) ...
    let in_dir = anchor.handle_in.normalized_to(1.0);
    assert!(vec_approx_eq(in_dir, Vec2::new(0.0, -1.0)));
    // ... but its OWN length must be unchanged at 4.0, NOT mirrored to
    // the dragged handle's new length of 9.0. This is precisely what
    // distinguishes Asymmetric from Symmetric.
    assert!(
        approx_eq(anchor.handle_in.length(), 4.0),
        "expected handle_in length to stay 4.0 (independent), got {}",
        anchor.handle_in.length()
    );
}

#[test]
fn ac3_contrast_symmetric_drag_mirrors_both_angle_and_length() {
    let document = Document::new(1);
    let anchor_id = AnchorId::new(1, 1);
    let path = document.create_path(
        &[NewAnchor {
            id: anchor_id,
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::new(-4.0, 0.0),
            handle_out: Vec2::new(4.0, 0.0),
            kind: AnchorKind::Symmetric,
        }],
        false,
    );
    document
        .set_handle(path, anchor_id, HandleSlot::Out, Vec2::new(0.0, 9.0))
        .expect("set_handle");
    let anchor = document.path(path).expect("exists").anchors[0];
    // On Symmetric, the opposite handle's length DOES follow: exactly
    // the negation of the dragged value.
    assert!(vec_approx_eq(anchor.handle_in, Vec2::new(0.0, -9.0)));
}

#[test]
fn ac3_contrast_corner_drag_leaves_opposite_handle_untouched() {
    let document = Document::new(1);
    let anchor_id = AnchorId::new(1, 1);
    let path = document.create_path(
        &[NewAnchor {
            id: anchor_id,
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::new(-4.0, 0.0),
            handle_out: Vec2::new(4.0, 0.0),
            kind: AnchorKind::Corner,
        }],
        false,
    );
    document
        .set_handle(path, anchor_id, HandleSlot::Out, Vec2::new(0.0, 9.0))
        .expect("set_handle");
    let anchor = document.path(path).expect("exists").anchors[0];
    assert!(vec_approx_eq(anchor.handle_in, Vec2::new(-4.0, 0.0)));
}

#[test]
fn ac3_asymmetric_drag_with_zero_length_opposite_handle_stays_zero() {
    // adrs.md's own stated edge case: "an opposite handle of length zero
    // stays zero."
    let document = Document::new(1);
    let anchor_id = AnchorId::new(1, 1);
    let path = document.create_path(
        &[NewAnchor {
            id: anchor_id,
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(4.0, 0.0),
            kind: AnchorKind::Asymmetric,
        }],
        false,
    );
    document
        .set_handle(path, anchor_id, HandleSlot::Out, Vec2::new(0.0, 9.0))
        .expect("set_handle");
    let anchor = document.path(path).expect("exists").anchors[0];
    assert!(vec_approx_eq(anchor.handle_in, Vec2::ZERO));
}

#[test]
fn ac3_asymmetric_drag_to_zero_leaves_opposite_handle_unchanged() {
    // adrs.md: "`v` = zero leaves the opposite handle unchanged (no
    // direction to follow)."
    let document = Document::new(1);
    let anchor_id = AnchorId::new(1, 1);
    let path = document.create_path(
        &[NewAnchor {
            id: anchor_id,
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::new(-4.0, 0.0),
            handle_out: Vec2::new(6.0, 0.0),
            kind: AnchorKind::Asymmetric,
        }],
        false,
    );
    document
        .set_handle(path, anchor_id, HandleSlot::Out, Vec2::ZERO)
        .expect("set_handle");
    let anchor = document.path(path).expect("exists").anchors[0];
    assert!(vec_approx_eq(anchor.handle_out, Vec2::ZERO));
    assert!(vec_approx_eq(anchor.handle_in, Vec2::new(-4.0, 0.0)));
}

// ---------------------------------------------------------------------
// AC 4: shape-preserving conversions (Symmetric->Asymmetric,
// Asymmetric->Corner)
// ---------------------------------------------------------------------

#[test]
fn ac4_symmetric_to_asymmetric_preserves_handles_exactly() {
    let document = Document::new(1);
    let anchor_id = AnchorId::new(1, 1);
    let path = document.create_path(
        &[NewAnchor {
            id: anchor_id,
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::new(-4.0, 1.0),
            handle_out: Vec2::new(4.0, -1.0),
            kind: AnchorKind::Symmetric,
        }],
        false,
    );
    document
        .convert_anchor_kind(path, &[anchor_id], AnchorKind::Asymmetric)
        .expect("convert");
    let anchor = document.path(path).expect("exists").anchors[0];
    assert_eq!(anchor.kind, AnchorKind::Asymmetric);
    assert!(vec_approx_eq(anchor.handle_in, Vec2::new(-4.0, 1.0)));
    assert!(vec_approx_eq(anchor.handle_out, Vec2::new(4.0, -1.0)));
}

#[test]
fn ac4_asymmetric_to_corner_preserves_handles_exactly() {
    let document = Document::new(1);
    let anchor_id = AnchorId::new(1, 1);
    let path = document.create_path(
        &[NewAnchor {
            id: anchor_id,
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::new(-2.0, 0.5),
            handle_out: Vec2::new(9.0, -3.0),
            kind: AnchorKind::Asymmetric,
        }],
        false,
    );
    document
        .convert_anchor_kind(path, &[anchor_id], AnchorKind::Corner)
        .expect("convert");
    let anchor = document.path(path).expect("exists").anchors[0];
    assert_eq!(anchor.kind, AnchorKind::Corner);
    assert!(vec_approx_eq(anchor.handle_in, Vec2::new(-2.0, 0.5)));
    assert!(vec_approx_eq(anchor.handle_out, Vec2::new(9.0, -3.0)));
}

// ---------------------------------------------------------------------
// AC 5: Asymmetric -> Symmetric resets to default length, collinear,
// discarding (not averaging) the independent lengths.
// ---------------------------------------------------------------------

#[test]
fn ac5_asymmetric_to_symmetric_resets_to_equal_default_length_discarding_old_lengths() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor {
                id: b,
                point: Point::new(10.0, 0.0),
                handle_in: Vec2::new(-2.0, 0.0),
                handle_out: Vec2::new(50.0, 0.0),
                kind: AnchorKind::Asymmetric,
            },
            NewAnchor::corner(c, Point::new(20.0, 0.0)),
        ],
        false,
    );
    document
        .convert_anchor_kind(path, &[b], AnchorKind::Symmetric)
        .expect("convert");
    let anchor = document.path(path).expect("exists").anchors[1];
    assert_eq!(anchor.kind, AnchorKind::Symmetric);
    // Equal lengths, neither 2.0 nor 50.0 nor their average (26.0):
    // discarded entirely in favour of the slice's one default length.
    assert!(approx_eq(
        anchor.handle_in.length(),
        anchor.handle_out.length()
    ));
    let len = anchor.handle_in.length();
    assert!(len > EPS);
    assert_ne!(len, 2.0);
    assert_ne!(len, 50.0);
    assert_ne!(len, 26.0);
    assert!(vec_approx_eq(anchor.handle_in, anchor.handle_out.negated()));
}

// ---------------------------------------------------------------------
// AC 16: converting a node to the kind it already has is a true no-op:
// no handle/kind/position change AND no document commit.
// ---------------------------------------------------------------------

#[test]
fn ac16_reconverting_to_the_same_kind_changes_nothing() {
    let document = Document::new(1);
    let anchor_id = AnchorId::new(1, 1);
    let a = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor {
                id: anchor_id,
                point: Point::new(10.0, 0.0),
                handle_in: Vec2::new(-4.0, 0.0),
                handle_out: Vec2::new(7.0, 0.0),
                kind: AnchorKind::Asymmetric,
            },
            NewAnchor::corner(c, Point::new(20.0, 0.0)),
        ],
        false,
    );

    let before = document.path(path).expect("exists");

    document
        .convert_anchor_kind(path, &[anchor_id], AnchorKind::Asymmetric)
        .expect("no-op convert must still return Ok");

    let after = document.path(path).expect("exists");
    assert_eq!(
        before, after,
        "AC16: re-applying the current kind must change nothing"
    );
}

#[test]
fn ac16_reconverting_to_the_same_kind_makes_no_commit() {
    // Black-box proxy for "no document commit is made": with no write at
    // all (the implementation's own stated contract), the exported CRDT
    // snapshot must be byte-for-byte identical before and after, since
    // Loro's snapshot bytes are a deterministic function of the recorded
    // op history. Any commit — even one that rewrites the same value —
    // would append an op and change these bytes.
    let document = Document::new(1);
    let anchor_id = AnchorId::new(1, 1);
    let path = document.create_path(
        &[NewAnchor {
            id: anchor_id,
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::ZERO,
            kind: AnchorKind::Corner,
        }],
        false,
    );
    let before_bytes = document.export_loro_snapshot().expect("export");

    document
        .convert_anchor_kind(path, &[anchor_id], AnchorKind::Corner)
        .expect("no-op convert");

    let after_bytes = document.export_loro_snapshot().expect("export");
    assert_eq!(
        before_bytes, after_bytes,
        "AC16: same-kind conversion must make no commit at all"
    );
}

#[test]
fn ac16_mixed_selection_converts_only_the_differing_anchors_in_one_commit() {
    let document = Document::new(1);
    let already_symmetric = AnchorId::new(1, 1);
    let needs_conversion = AnchorId::new(1, 2);
    let path = document.create_path(
        &[
            NewAnchor {
                id: already_symmetric,
                point: Point::new(0.0, 0.0),
                handle_in: Vec2::new(-3.0, 0.0),
                handle_out: Vec2::new(3.0, 0.0),
                kind: AnchorKind::Symmetric,
            },
            NewAnchor::corner(needs_conversion, Point::new(10.0, 0.0)),
        ],
        false,
    );
    document
        .convert_anchor_kind(
            path,
            &[already_symmetric, needs_conversion],
            AnchorKind::Symmetric,
        )
        .expect("convert");
    let snapshot = document.path(path).expect("exists");
    // The already-symmetric anchor's handles must be untouched (not
    // reset), while the corner one is genuinely converted.
    assert!(vec_approx_eq(
        snapshot.anchors[0].handle_in,
        Vec2::new(-3.0, 0.0)
    ));
    assert_eq!(snapshot.anchors[1].kind, AnchorKind::Symmetric);
}

// ---------------------------------------------------------------------
// AC 8: Join enablement / refusal conditions.
// ---------------------------------------------------------------------

#[test]
fn ac8_join_refuses_interior_node() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(10.0, 0.0)),
            NewAnchor::corner(c, Point::new(20.0, 0.0)),
        ],
        false,
    );
    let result = document.join_endpoints(path, a, path, b);
    assert_eq!(result, Err(PathEditError::NotJoinable));
}

#[test]
fn ac8_join_refuses_a_closed_paths_node() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(10.0, 0.0)),
            NewAnchor::corner(c, Point::new(0.0, 10.0)),
        ],
        true,
    );
    assert_eq!(
        document.join_endpoints(path, a, path, c),
        Err(PathEditError::NotJoinable)
    );
}

#[test]
fn ac8_join_refuses_the_two_ends_of_a_two_anchor_open_path() {
    // The documented gap: joining would collapse to a 1-anchor closed
    // path nothing can draw.
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(10.0, 0.0)),
        ],
        false,
    );
    assert_eq!(
        document.join_endpoints(path, a, path, b),
        Err(PathEditError::NotJoinable)
    );
}

#[test]
fn ac8_join_refuses_the_same_anchor_selected_twice() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(10.0, 0.0)),
            NewAnchor::corner(c, Point::new(20.0, 0.0)),
        ],
        false,
    );
    assert_eq!(
        document.join_endpoints(path, a, path, a),
        Err(PathEditError::NotJoinable)
    );
}

// ---------------------------------------------------------------------
// AC 10, 11: same-path Join (closing an open path).
// ---------------------------------------------------------------------

#[test]
fn ac10_11_same_path_join_closes_the_path_with_midpoint_corner_and_preserved_handles() {
    let document = Document::new(1);
    let first = AnchorId::new(1, 1);
    let mid = AnchorId::new(1, 2);
    let last = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor {
                id: first,
                point: Point::new(0.0, 0.0),
                handle_in: Vec2::new(-1.0, -1.0), // dangling, discarded
                handle_out: Vec2::new(1.0, 1.0),  // kept (interior-facing)
                kind: AnchorKind::Symmetric,
            },
            NewAnchor::corner(mid, Point::new(10.0, 5.0)),
            NewAnchor {
                id: last,
                point: Point::new(20.0, 0.0),
                handle_in: Vec2::new(-2.0, 2.0), // kept (interior-facing)
                handle_out: Vec2::new(2.0, -2.0), // dangling, discarded
                kind: AnchorKind::Symmetric,
            },
        ],
        false,
    );

    let (result_path, merged_anchor) = document
        .join_endpoints(path, first, path, last)
        .expect("join");
    assert_eq!(result_path, path);
    assert_eq!(merged_anchor, first, "selection-order rule: first survives");

    let snapshot = document.path(path).expect("exists");
    assert!(snapshot.closed, "AC10: becomes closed");
    assert_eq!(snapshot.anchors.len(), 2, "one fewer node than before");

    let merged = snapshot.anchors[0];
    assert_eq!(merged.id, first);
    assert_eq!(merged.kind, AnchorKind::Corner, "AC9/10: always Corner");
    assert!(
        point_approx_eq(merged.point, Point::new(10.0, 0.0)),
        "midpoint"
    );
    // Each side keeps its own original *interior-facing* handle.
    assert!(vec_approx_eq(merged.handle_out, Vec2::new(1.0, 1.0)));
    assert!(vec_approx_eq(merged.handle_in, Vec2::new(-2.0, 2.0)));
}

#[test]
fn ac11_join_selects_only_the_merged_node() {
    // At the document-core level, "selection" is a ui-core concept; what
    // document-core promises is the merged (path, anchor) pair the UI
    // selects afterward, with the losing endpoint's id discarded.
    let document = Document::new(1);
    let first = AnchorId::new(1, 1);
    let mid = AnchorId::new(1, 2);
    let last = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(first, Point::new(0.0, 0.0)),
            NewAnchor::corner(mid, Point::new(10.0, 5.0)),
            NewAnchor::corner(last, Point::new(20.0, 0.0)),
        ],
        false,
    );
    let (_, merged_anchor) = document
        .join_endpoints(path, first, path, last)
        .expect("join");
    assert_eq!(merged_anchor, first);
    let snapshot = document.path(path).expect("exists");
    assert!(snapshot.anchors.iter().all(|a| a.id != last));
}

#[test]
fn ac9_join_has_no_distance_limit() {
    let document = Document::new(1);
    let first = AnchorId::new(1, 1);
    let mid = AnchorId::new(1, 2);
    let last = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(first, Point::new(0.0, 0.0)),
            NewAnchor::corner(mid, Point::new(5.0, 0.0)),
            NewAnchor::corner(last, Point::new(100_000.0, 100_000.0)),
        ],
        false,
    );
    let (_, merged) = document
        .join_endpoints(path, first, path, last)
        .expect("join");
    let snapshot = document.path(path).expect("exists");
    let anchor = snapshot.anchors.iter().find(|a| a.id == merged).unwrap();
    assert!(point_approx_eq(
        anchor.point,
        Point::new(50_000.0, 50_000.0)
    ));
}

// ---------------------------------------------------------------------
// AC 9 (document-model half): the cross-object join_endpoints code path,
// exercised directly without any Select-tool UI. All four endpoint-
// combination reversals from adrs.md's table.
// ---------------------------------------------------------------------

#[test]
fn ac9_join_two_different_objects_a_last_b_first_appends_in_order() {
    let document = Document::new(1);
    let a1 = AnchorId::new(1, 1);
    let a2 = AnchorId::new(1, 2);
    let b1 = AnchorId::new(1, 3);
    let b2 = AnchorId::new(1, 4);
    let path_a = document.create_path(
        &[
            NewAnchor::corner(a1, Point::new(0.0, 0.0)),
            NewAnchor::corner(a2, Point::new(10.0, 0.0)),
        ],
        false,
    );
    let path_b = document.create_path(
        &[
            NewAnchor::corner(b1, Point::new(12.0, 0.0)),
            NewAnchor::corner(b2, Point::new(20.0, 0.0)),
        ],
        false,
    );
    // a's last (a2) joined to b's first (b1): straightforward append.
    let (result_path, merged) = document
        .join_endpoints(path_a, a2, path_b, b1)
        .expect("join");
    assert_eq!(result_path, path_a, "a's object survives");
    assert_eq!(merged, a2);

    let snapshot = document.path(path_a).expect("exists");
    assert_eq!(snapshot.anchors.len(), 3);
    assert_eq!(snapshot.anchors[0].id, a1);
    assert_eq!(snapshot.anchors[1].id, a2);
    assert_eq!(snapshot.anchors[2].id, b2);
    assert!(point_approx_eq(
        snapshot.anchors[1].point,
        Point::new(11.0, 0.0)
    ));
    assert_eq!(snapshot.anchors[1].kind, AnchorKind::Corner);
    assert!(document.path(path_b).is_none(), "b's object is deleted");
}

#[test]
fn ac9_join_two_different_objects_a_last_b_last_reverses_b() {
    let document = Document::new(1);
    let a1 = AnchorId::new(1, 1);
    let a2 = AnchorId::new(1, 2);
    let b1 = AnchorId::new(1, 3);
    let b2 = AnchorId::new(1, 4);
    let path_a = document.create_path(
        &[
            NewAnchor::corner(a1, Point::new(0.0, 0.0)),
            NewAnchor::corner(a2, Point::new(10.0, 0.0)),
        ],
        false,
    );
    let path_b = document.create_path(
        &[
            NewAnchor::corner(b1, Point::new(20.0, 0.0)),
            NewAnchor::corner(b2, Point::new(12.0, 0.0)), // b2 is b's "last"
        ],
        false,
    );
    let (result_path, merged) = document
        .join_endpoints(path_a, a2, path_b, b2)
        .expect("join");
    assert_eq!(result_path, path_a);
    assert_eq!(merged, a2);
    let snapshot = document.path(path_a).expect("exists");
    assert_eq!(snapshot.anchors.len(), 3);
    assert_eq!(snapshot.anchors[0].id, a1);
    assert_eq!(snapshot.anchors[1].id, a2);
    // b must be reversed: b1 (not deleted, was the OTHER end) follows.
    assert_eq!(snapshot.anchors[2].id, b1);
}

#[test]
fn ac9_join_two_different_objects_a_first_b_first_reverses_b() {
    let document = Document::new(1);
    let a1 = AnchorId::new(1, 1);
    let a2 = AnchorId::new(1, 2);
    let b1 = AnchorId::new(1, 3);
    let b2 = AnchorId::new(1, 4);
    let path_a = document.create_path(
        &[
            NewAnchor::corner(a1, Point::new(0.0, 0.0)),
            NewAnchor::corner(a2, Point::new(10.0, 0.0)),
        ],
        false,
    );
    let path_b = document.create_path(
        &[
            NewAnchor::corner(b1, Point::new(-2.0, 0.0)),
            NewAnchor::corner(b2, Point::new(-20.0, 0.0)),
        ],
        false,
    );
    let (result_path, merged) = document
        .join_endpoints(path_a, a1, path_b, b1)
        .expect("join");
    assert_eq!(result_path, path_a);
    assert_eq!(merged, a1, "first-selected path keeps its id and order");
    let snapshot = document.path(path_a).expect("exists");
    assert_eq!(snapshot.anchors.len(), 3);
    // b reversed (b2 before a1=merged), then a1, then a2 — a's order
    // never reversed.
    assert_eq!(snapshot.anchors[0].id, b2);
    assert_eq!(snapshot.anchors[1].id, a1);
    assert_eq!(snapshot.anchors[2].id, a2);
}

#[test]
fn ac9_join_two_different_objects_a_first_b_last_prepends_unchanged() {
    let document = Document::new(1);
    let a1 = AnchorId::new(1, 1);
    let a2 = AnchorId::new(1, 2);
    let b1 = AnchorId::new(1, 3);
    let b2 = AnchorId::new(1, 4);
    let path_a = document.create_path(
        &[
            NewAnchor::corner(a1, Point::new(0.0, 0.0)),
            NewAnchor::corner(a2, Point::new(10.0, 0.0)),
        ],
        false,
    );
    let path_b = document.create_path(
        &[
            NewAnchor::corner(b1, Point::new(-20.0, 0.0)),
            NewAnchor::corner(b2, Point::new(-2.0, 0.0)),
        ],
        false,
    );
    let (result_path, merged) = document
        .join_endpoints(path_a, a1, path_b, b2)
        .expect("join");
    assert_eq!(result_path, path_a);
    assert_eq!(merged, a1);
    let snapshot = document.path(path_a).expect("exists");
    assert_eq!(snapshot.anchors.len(), 3);
    // b unchanged order: b1, b2(=merged=a1), a2.
    assert_eq!(snapshot.anchors[0].id, b1);
    assert_eq!(snapshot.anchors[1].id, a1);
    assert_eq!(snapshot.anchors[2].id, a2);
}

#[test]
fn ac9_join_discards_the_two_dangling_outward_handles() {
    let document = Document::new(1);
    let a1 = AnchorId::new(1, 1);
    let a2 = AnchorId::new(1, 2);
    let b1 = AnchorId::new(1, 3);
    let b2 = AnchorId::new(1, 4);
    let path_a = document.create_path(
        &[
            NewAnchor::corner(a1, Point::new(0.0, 0.0)),
            NewAnchor {
                id: a2,
                point: Point::new(10.0, 0.0),
                handle_in: Vec2::new(-3.0, 0.0), // interior-facing: kept
                handle_out: Vec2::new(99.0, 99.0), // dangling: discarded
                kind: AnchorKind::Corner,
            },
        ],
        false,
    );
    let path_b = document.create_path(
        &[
            NewAnchor {
                id: b1,
                point: Point::new(12.0, 0.0),
                handle_in: Vec2::new(-77.0, -77.0), // dangling: discarded
                handle_out: Vec2::new(4.0, 0.0),    // interior-facing: kept
                kind: AnchorKind::Corner,
            },
            NewAnchor::corner(b2, Point::new(20.0, 0.0)),
        ],
        false,
    );
    document
        .join_endpoints(path_a, a2, path_b, b1)
        .expect("join");
    let snapshot = document.path(path_a).expect("exists");
    let merged = snapshot.anchors.iter().find(|a| a.id == a2).unwrap();
    assert!(vec_approx_eq(merged.handle_in, Vec2::new(-3.0, 0.0)));
    assert!(vec_approx_eq(merged.handle_out, Vec2::new(4.0, 0.0)));
}

// ---------------------------------------------------------------------
// AC 12: Split enablement / refusal.
// ---------------------------------------------------------------------

#[test]
fn ac12_split_refuses_an_open_paths_first_anchor() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(10.0, 0.0)),
        ],
        false,
    );
    let new_id = AnchorId::new(9, 1);
    assert_eq!(
        document.split_at_anchor(path, a, new_id),
        Err(PathEditError::NotSplittable)
    );
}

#[test]
fn ac12_split_refuses_an_open_paths_last_anchor() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(b, Point::new(10.0, 0.0)),
        ],
        false,
    );
    let new_id = AnchorId::new(9, 1);
    assert_eq!(
        document.split_at_anchor(path, b, new_id),
        Err(PathEditError::NotSplittable)
    );
}

// ---------------------------------------------------------------------
// AC 13, 15: open-path Split -> two objects, handle/kind rule, both
// coincident copies present (selection itself is a ui-core concern, but
// both copies existing with the right ids is document-core's job).
// ---------------------------------------------------------------------

#[test]
fn ac13_split_an_open_path_interior_node_produces_two_objects_with_the_right_handle_rule() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let mid = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor {
                id: mid,
                point: Point::new(10.0, 0.0),
                handle_in: Vec2::new(-3.0, 1.0),
                handle_out: Vec2::new(3.0, -1.0),
                kind: AnchorKind::Symmetric,
            },
            NewAnchor::corner(c, Point::new(20.0, 0.0)),
        ],
        false,
    );
    let new_id = AnchorId::new(9, 1);
    let ((first_path, first_anchor), (second_path, second_anchor)) =
        document.split_at_anchor(path, mid, new_id).expect("split");

    assert_eq!(first_path, path, "first copy stays on the original object");
    assert_eq!(first_anchor, mid, "first copy keeps the original id");
    assert_ne!(second_path, first_path, "AC13: two SEPARATE objects");
    assert_eq!(second_anchor, new_id);

    let original = document.path(path).expect("exists");
    assert_eq!(original.anchors.len(), 2, "a, mid(copy)");
    let first_copy = original.anchors.iter().find(|x| x.id == mid).unwrap();
    assert!(point_approx_eq(first_copy.point, Point::new(10.0, 0.0)));
    assert!(
        vec_approx_eq(first_copy.handle_in, Vec2::new(-3.0, 1.0)),
        "keeps incoming handle"
    );
    assert!(
        vec_approx_eq(first_copy.handle_out, Vec2::ZERO),
        "outgoing retracted"
    );
    assert_eq!(first_copy.kind, AnchorKind::Corner);

    let new_object = document.path(second_path).expect("new object exists");
    assert!(!new_object.closed);
    assert_eq!(new_object.anchors.len(), 2, "mid(copy2), c");
    let second_copy = &new_object.anchors[0];
    assert_eq!(second_copy.id, new_id);
    assert!(point_approx_eq(second_copy.point, Point::new(10.0, 0.0)));
    assert!(
        vec_approx_eq(second_copy.handle_out, Vec2::new(3.0, -1.0)),
        "keeps outgoing handle"
    );
    assert!(
        vec_approx_eq(second_copy.handle_in, Vec2::ZERO),
        "incoming retracted"
    );
    assert_eq!(second_copy.kind, AnchorKind::Corner);
    assert_eq!(new_object.anchors[1].id, c);
}

#[test]
fn ac13_split_new_object_copies_the_original_paths_style() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let mid = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor::corner(mid, Point::new(10.0, 0.0)),
            NewAnchor::corner(c, Point::new(20.0, 0.0)),
        ],
        false,
    );
    let original_style = document.path(path).expect("exists");
    let new_id = AnchorId::new(9, 1);
    let (_, (second_path, _)) = document.split_at_anchor(path, mid, new_id).expect("split");
    let new_object = document.path(second_path).expect("exists");
    assert_eq!(new_object.stroke_width, original_style.stroke_width);
    assert_eq!(new_object.stroke, original_style.stroke);
}

// ---------------------------------------------------------------------
// AC 14: closed-path Split -> one more-open path.
// ---------------------------------------------------------------------

#[test]
fn ac14_split_a_closed_path_opens_it_with_one_more_anchor_and_right_handle_rule() {
    let document = Document::new(1);
    let a = AnchorId::new(1, 1);
    let b = AnchorId::new(1, 2);
    let c = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(a, Point::new(0.0, 0.0)),
            NewAnchor {
                id: b,
                point: Point::new(10.0, 0.0),
                handle_in: Vec2::new(-2.0, 1.0),
                handle_out: Vec2::new(2.0, -1.0),
                kind: AnchorKind::Symmetric,
            },
            NewAnchor::corner(c, Point::new(0.0, 10.0)),
        ],
        true,
    );
    let new_id = AnchorId::new(9, 1);
    let ((first_path, first_anchor), (second_path, second_anchor)) =
        document.split_at_anchor(path, b, new_id).expect("split");

    assert_eq!(first_path, path);
    assert_eq!(second_path, path, "same object, not two");
    assert_eq!(
        first_anchor, new_id,
        "outgoing-handle copy is the new first node"
    );
    assert_eq!(second_anchor, b, "incoming-handle copy is the last node");

    let snapshot = document.path(path).expect("exists");
    assert!(!snapshot.closed, "AC14: becomes open");
    assert_eq!(snapshot.anchors.len(), 4, "one more anchor than before");

    assert_eq!(
        snapshot.anchors[0].id, new_id,
        "first node: outgoing-handle copy"
    );
    assert!(point_approx_eq(
        snapshot.anchors[0].point,
        Point::new(10.0, 0.0)
    ));
    assert!(vec_approx_eq(
        snapshot.anchors[0].handle_out,
        Vec2::new(2.0, -1.0)
    ));
    assert!(vec_approx_eq(snapshot.anchors[0].handle_in, Vec2::ZERO));
    assert_eq!(snapshot.anchors[0].kind, AnchorKind::Corner);

    // Traversal continues through every other original anchor in cyclic
    // order: c, a, then the last node (the incoming-handle copy, b).
    assert_eq!(snapshot.anchors[1].id, c);
    assert_eq!(snapshot.anchors[2].id, a);
    assert_eq!(snapshot.anchors[3].id, b, "last node: incoming-handle copy");
    assert!(point_approx_eq(
        snapshot.anchors[3].point,
        Point::new(10.0, 0.0)
    ));
    assert!(vec_approx_eq(
        snapshot.anchors[3].handle_in,
        Vec2::new(-2.0, 1.0)
    ));
    assert!(vec_approx_eq(snapshot.anchors[3].handle_out, Vec2::ZERO));
    assert_eq!(snapshot.anchors[3].kind, AnchorKind::Corner);
}

// ---------------------------------------------------------------------
// Join/Split as exact inverses on the same-path case (spec's own framing).
// ---------------------------------------------------------------------

#[test]
fn join_then_split_at_the_junction_restores_the_original_anchor_count() {
    let document = Document::new(1);
    let first = AnchorId::new(1, 1);
    let mid = AnchorId::new(1, 2);
    let last = AnchorId::new(1, 3);
    let path = document.create_path(
        &[
            NewAnchor::corner(first, Point::new(0.0, 0.0)),
            NewAnchor::corner(mid, Point::new(10.0, 5.0)),
            NewAnchor::corner(last, Point::new(20.0, 0.0)),
        ],
        false,
    );
    let (_, merged) = document
        .join_endpoints(path, first, path, last)
        .expect("join");
    assert_eq!(document.path(path).unwrap().anchors.len(), 2);

    let new_id = AnchorId::new(9, 1);
    document
        .split_at_anchor(path, merged, new_id)
        .expect("split back open");
    let snapshot = document.path(path).expect("exists");
    assert!(!snapshot.closed, "split of a closed path reopens it");
    assert_eq!(
        snapshot.anchors.len(),
        3,
        "back to the original anchor count"
    );
}
