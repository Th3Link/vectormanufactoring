//! Black-box / white-box edge cases for `Document::translate_objects` and
//! `Document::delete_objects` (`specs/0004-canvas-navigation-and-
//! selection/specification.md`, acceptance criteria 18-21), independent
//! of `vecmanf-document-core::objects`'s own `#[cfg(test)]` module —
//! duplicate ids in one batch, an empty batch, and a delete-then-
//! translate-the-same-id sequence.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vecmanf_document_core::{
    AnchorId, Document, Length, NewAnchor, Point, RectBounds, Shape, Vec2,
};

fn rect(document: &Document, x: f64) -> vecmanf_document_core::NodeId {
    document.create_rect(RectBounds {
        origin: Point::new(x, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    })
}

fn rect_origin(document: &Document, id: vecmanf_document_core::NodeId) -> Point {
    let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
        panic!("expected rect");
    };
    bounds.origin
}

#[test]
fn translate_objects_with_an_empty_id_list_is_a_no_op_that_still_succeeds() {
    let document = Document::new(1);
    let id = rect(&document, 0.0);
    let result = document.translate_objects(&[], Vec2::new(5.0, 5.0));
    assert!(result.is_ok());
    assert_eq!(rect_origin(&document, id), Point::new(0.0, 0.0));
}

#[test]
fn delete_objects_with_an_empty_id_list_is_a_no_op_that_still_succeeds() {
    let document = Document::new(1);
    let id = rect(&document, 0.0);
    let result = document.delete_objects(&[]);
    assert!(result.is_ok());
    assert!(
        document.primitive(id).is_some(),
        "nothing was named, nothing was deleted"
    );
}

/// The same id listed twice in one `translate_objects` call: the offset
/// must apply once per object, not once per occurrence in the list — a
/// naive "shift every meta the ids resolve to" implementation that does
/// not dedupe would double the offset for a repeated id.
#[test]
fn translate_objects_applies_the_offset_once_even_if_the_same_id_is_listed_twice() {
    let document = Document::new(1);
    let id = rect(&document, 0.0);
    document
        .translate_objects(&[id, id], Vec2::new(10.0, 0.0))
        .expect("translate");
    assert_eq!(
        rect_origin(&document, id),
        Point::new(10.0, 0.0),
        "a duplicate id in the batch must not double-apply the offset"
    );
}

/// A delete is terminal: translating an id that a *previous, already-
/// committed* delete removed is refused, same as the implementer's own
/// "stale id refuses the whole batch" tests, but chained the opposite
/// direction (delete first, as its own commit, then translate).
#[test]
fn translating_an_id_deleted_by_an_earlier_commit_is_refused() {
    let document = Document::new(1);
    let survivor = rect(&document, 0.0);
    let doomed = rect(&document, 50.0);
    document.delete_objects(&[doomed]).expect("delete");

    let result = document.translate_objects(&[survivor, doomed], Vec2::new(1.0, 1.0));
    assert!(result.is_err());
    assert_eq!(
        rect_origin(&document, survivor),
        Point::new(0.0, 0.0),
        "the still-live id in the same refused batch must not move either"
    );
}

/// A path with exactly one anchor (geometrically degenerate, but
/// `translate_objects` never reads anchor count, only iterates whatever
/// is there) still translates cleanly rather than panicking on an
/// out-of-bounds pair lookup.
#[test]
fn translate_objects_on_a_single_anchor_path_does_not_panic() {
    let document = Document::new(1);
    let path = document.create_path(
        &[NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0))],
        false,
    );
    document
        .translate_objects(&[path], Vec2::new(3.0, 4.0))
        .expect("translate a degenerate one-anchor path");
    let snapshot = document.path(path).expect("exists");
    assert_eq!(snapshot.anchors[0].point, Point::new(3.0, 4.0));
}

#[test]
fn delete_objects_with_the_same_id_listed_twice_does_not_panic() {
    let document = Document::new(1);
    let id = rect(&document, 0.0);
    // Both copies pass the initial existence check (taken before any
    // delete happens); the second `tree.delete` call in the loop then
    // targets an id already removed by the first.
    let result = document.delete_objects(&[id, id]);
    assert!(
        result.is_ok(),
        "a duplicate id must not panic the whole call"
    );
    assert!(document.primitive(id).is_none());
}
