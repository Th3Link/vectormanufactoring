//! `Document::bend_segment` (`specs/0031-segment-drag-bending` criteria 6a, 12, 18, 19): the
//! node-kind rule on both ends, the directed closing segment of a two-node closed path, one
//! commit labelled `bend_segment`, and a refusal that writes nothing.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::ops::ControlFlow;

use curvyo_document_core::{
    AnchorId, AnchorKind, Document, NewAnchor, NodeId, PathEditError, Point, Vec2,
};
use loro::LoroDoc;

fn id(n: u64) -> AnchorId {
    AnchorId::new(1, n)
}

fn anchor(n: u64, x: f64, kind: AnchorKind) -> NewAnchor {
    NewAnchor {
        id: id(n),
        point: Point::new(x, 0.0),
        handle_in: Vec2::ZERO,
        handle_out: Vec2::ZERO,
        kind,
    }
}

fn v(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

/// Open path A (0), B (90), C (180) with the given kinds, all handles zero.
fn open_path(kinds: [AnchorKind; 3]) -> (Document, NodeId) {
    let document = Document::new(1);
    let path = document.create_path(
        &[
            anchor(1, 0.0, kinds[0]),
            anchor(2, 90.0, kinds[1]),
            anchor(3, 180.0, kinds[2]),
        ],
        false,
    );
    (document, path)
}

fn handles(document: &Document, path: NodeId, n: u64) -> (Vec2, Vec2) {
    let snapshot = document.path(path).unwrap();
    let anchor = snapshot.anchors.iter().find(|a| a.id == id(n)).unwrap();
    (anchor.handle_in, anchor.handle_out)
}

/// The commit messages of the document, oldest first.
fn labels(document: &Document) -> Vec<String> {
    let loro = LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    let frontiers: Vec<loro::ID> = loro.oplog_frontiers().iter().collect();
    let mut changes: Vec<(u32, String)> = Vec::new();
    loro.travel_change_ancestors(&frontiers, &mut |change| {
        changes.push((
            change.lamport,
            change.message.map(|m| m.to_string()).unwrap_or_default(),
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    changes.sort();
    changes.into_iter().map(|(_, label)| label).collect()
}

/// Criterion 12: B Symmetric: its outgoing handle mirrors the bent incoming one and the next
/// segment becomes a curve; with B a Corner it stays retracted and BC stays a line.
#[test]
fn the_far_handle_of_a_node_follows_its_kind() {
    let (document, path) = open_path([
        AnchorKind::Corner,
        AnchorKind::Symmetric,
        AnchorKind::Corner,
    ]);
    document
        .bend_segment(path, id(1), id(2), v(30.0, -40.0), v(-30.0, -40.0))
        .unwrap();
    assert_eq!(handles(&document, path, 1).1, v(30.0, -40.0));
    let (b_in, b_out) = handles(&document, path, 2);
    assert_eq!(b_in, v(-30.0, -40.0));
    assert_eq!(b_out, v(30.0, 40.0), "mirrored: BC is a curve now");
    assert_eq!(handles(&document, path, 3), (Vec2::ZERO, Vec2::ZERO));

    let (document, path) = open_path([AnchorKind::Corner; 3]);
    document
        .bend_segment(path, id(1), id(2), v(30.0, -40.0), v(-30.0, -40.0))
        .unwrap();
    assert_eq!(
        handles(&document, path, 2),
        (v(-30.0, -40.0), Vec2::ZERO),
        "BC stays a line"
    );
}

/// Criterion 12: an Asymmetric node keeps the length of its other handle and turns it opposite.
#[test]
fn an_asymmetric_node_keeps_the_length_of_its_other_handle() {
    let document = Document::new(1);
    let mut b = anchor(2, 90.0, AnchorKind::Asymmetric);
    b.handle_out = v(0.0, 20.0);
    let path = document.create_path(
        &[
            anchor(1, 0.0, AnchorKind::Corner),
            b,
            anchor(3, 180.0, AnchorKind::Corner),
        ],
        false,
    );
    document
        .bend_segment(path, id(1), id(2), v(30.0, -40.0), v(-30.0, -40.0))
        .unwrap();
    let (b_in, b_out) = handles(&document, path, 2);
    assert_eq!(b_in, v(-30.0, -40.0));
    assert!(
        close(b_out, v(12.0, 16.0)),
        "{b_out:?}: length 20 along (30, 40)"
    );
}

/// The hidden handle of an open path's end node is written by the same rule as a hand drag of
/// the shown handle.
#[test]
fn the_hidden_handle_of_an_end_node_is_written_by_the_kind_rule() {
    let (document, path) = open_path([
        AnchorKind::Symmetric,
        AnchorKind::Corner,
        AnchorKind::Corner,
    ]);
    document
        .bend_segment(path, id(1), id(2), v(30.0, -40.0), v(-30.0, -40.0))
        .unwrap();
    assert_eq!(
        handles(&document, path, 1),
        (v(-30.0, 40.0), v(30.0, -40.0))
    );
}

/// Criterion 18: one commit, labelled `bend_segment`; nothing but the handles changes.
#[test]
fn a_bend_is_one_commit_and_moves_no_node() {
    let (document, path) = open_path([AnchorKind::Corner; 3]);
    let before = labels(&document);
    let points: Vec<Point> = document
        .path(path)
        .unwrap()
        .anchors
        .iter()
        .map(|a| a.point)
        .collect();
    document
        .bend_segment(path, id(2), id(3), v(10.0, 5.0), v(-10.0, 5.0))
        .unwrap();
    let after = labels(&document);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after.last().map(String::as_str), Some("bend_segment"));
    let snapshot = document.path(path).unwrap();
    assert_eq!(
        snapshot.anchors.iter().map(|a| a.point).collect::<Vec<_>>(),
        points
    );
    assert_eq!(handles(&document, path, 1), (Vec2::ZERO, Vec2::ZERO));
}

/// A refusal writes nothing: unknown path or anchor, and two anchors that are not adjacent in
/// that direction (A and C of an open path; the closing direction of an open path).
#[test]
fn a_refusal_writes_nothing() {
    let (document, path) = open_path([AnchorKind::Corner; 3]);
    assert_eq!(
        document.bend_segment(path, id(1), id(3), v(1.0, 1.0), v(1.0, 1.0)),
        Err(PathEditError::NotAnAdjacentSegment)
    );
    assert_eq!(
        document.bend_segment(path, id(1), id(99), v(1.0, 1.0), v(1.0, 1.0)),
        Err(PathEditError::NoSuchAnchor)
    );
    let missing = document.create_path(
        &[
            anchor(8, 0.0, AnchorKind::Corner),
            anchor(9, 5.0, AnchorKind::Corner),
        ],
        false,
    );
    document.delete_objects(&[missing]).unwrap();
    let before = labels(&document);
    assert_eq!(
        document.bend_segment(missing, id(1), id(2), v(1.0, 1.0), v(1.0, 1.0)),
        Err(PathEditError::NoSuchPath)
    );
    assert_eq!(labels(&document), before);
    assert_eq!(handles(&document, path, 2), (Vec2::ZERO, Vec2::ZERO));
}

/// Criterion 6a: a closed path of two nodes has two segments; the direction picks the one.
#[test]
fn the_closing_segment_of_a_two_node_closed_path_can_be_bent() {
    let document = Document::new(1);
    let path = document.create_path(
        &[
            anchor(1, 0.0, AnchorKind::Corner),
            anchor(2, 90.0, AnchorKind::Corner),
        ],
        true,
    );
    // A to B: A's outgoing and B's incoming handle.
    document
        .bend_segment(path, id(1), id(2), v(1.0, 2.0), v(3.0, 4.0))
        .unwrap();
    assert_eq!(handles(&document, path, 1), (Vec2::ZERO, v(1.0, 2.0)));
    assert_eq!(handles(&document, path, 2), (v(3.0, 4.0), Vec2::ZERO));
    // B to A, the closing segment: B's outgoing and A's incoming handle.
    document
        .bend_segment(path, id(2), id(1), v(5.0, 6.0), v(7.0, 8.0))
        .unwrap();
    assert_eq!(handles(&document, path, 1), (v(7.0, 8.0), v(1.0, 2.0)));
    assert_eq!(handles(&document, path, 2), (v(3.0, 4.0), v(5.0, 6.0)));
}

/// The same fix makes "Make line" and "Make curve" work on the closing segment (criterion 6a).
#[test]
fn make_line_and_curve_reach_the_closing_segment() {
    let document = Document::new(1);
    let path = document.create_path(
        &[
            anchor(1, 0.0, AnchorKind::Corner),
            anchor(2, 90.0, AnchorKind::Corner),
        ],
        true,
    );
    document.set_segment_curve(path, id(2), id(1)).unwrap();
    assert_ne!(
        handles(&document, path, 2).1,
        Vec2::ZERO,
        "B's outgoing handle"
    );
    assert_ne!(
        handles(&document, path, 1).0,
        Vec2::ZERO,
        "A's incoming handle"
    );
    assert_eq!(
        handles(&document, path, 1).1,
        Vec2::ZERO,
        "the first segment is untouched"
    );
    document.set_segment_line(path, id(2), id(1)).unwrap();
    assert_eq!(handles(&document, path, 2), (Vec2::ZERO, Vec2::ZERO));
    assert_eq!(handles(&document, path, 1), (Vec2::ZERO, Vec2::ZERO));
}
