//! `Document::extend_path`, `connect_paths` and `close_paths` (`specs/0034-pen-path-extension`
//! criteria 4, 5, 8 to 11, 14, 19 to 21): the order of the nodes, what survives, one commit with
//! its label, and refusals that write nothing.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::ops::ControlFlow;

use curvyo_document_core::{
    AnchorId, AnchorKind, AnchorSnapshot, Color, Document, NewAnchor, NodeId, PathEditError,
    PathEnd, PathGrowth, Point, StyleEdit, Vec2,
};
use loro::LoroDoc;

fn id(n: u64) -> AnchorId {
    AnchorId::new(1, n)
}

fn node(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(id(n), Point::new(x, y))
}

fn path_of(document: &Document, nodes: &[NewAnchor]) -> NodeId {
    document.create_path(nodes, false)
}

fn order(document: &Document, path: NodeId) -> Vec<(u64, f64, f64)> {
    document
        .path(path)
        .unwrap()
        .anchors
        .iter()
        .map(|a| {
            (
                u64::try_from(a.id.as_u128() & u128::from(u64::MAX)).unwrap(),
                a.point.x,
                a.point.y,
            )
        })
        .collect()
}

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

fn growth(path: NodeId, end: PathEnd, added: Vec<NewAnchor>) -> PathGrowth {
    PathGrowth {
        path,
        end,
        added,
        absorb: None,
        replace: Vec::new(),
        drop: Vec::new(),
    }
}

/// Criterion 4: continuing from the last node appends; continuing from the first node places the
/// new nodes before it, the node drawn last first.
#[test]
fn extending_keeps_the_existing_order_at_either_end() {
    let document = Document::new(1);
    let path = path_of(
        &document,
        &[node(1, 0.0, 0.0), node(2, 10.0, 0.0), node(3, 20.0, 0.0)],
    );
    document
        .extend_path(&growth(
            path,
            PathEnd::Last,
            vec![node(4, 30.0, 0.0), node(5, 40.0, 0.0)],
        ))
        .unwrap();
    assert_eq!(
        order(&document, path),
        vec![
            (1, 0.0, 0.0),
            (2, 10.0, 0.0),
            (3, 20.0, 0.0),
            (4, 30.0, 0.0),
            (5, 40.0, 0.0)
        ]
    );

    let other = path_of(
        &document,
        &[node(11, 0.0, 0.0), node(12, 10.0, 0.0), node(13, 20.0, 0.0)],
    );
    // Drawn from (0, 0): first (0, 10) then (0, 20); stored order puts the node drawn last first.
    document
        .extend_path(&growth(
            other,
            PathEnd::First,
            vec![node(15, 0.0, 20.0), node(14, 0.0, 10.0)],
        ))
        .unwrap();
    assert_eq!(
        order(&document, other),
        vec![
            (15, 0.0, 20.0),
            (14, 0.0, 10.0),
            (11, 0.0, 0.0),
            (12, 10.0, 0.0),
            (13, 20.0, 0.0)
        ]
    );
}

/// Criterion 5: the survivor keeps its id, style and the ids of its nodes; one commit labelled
/// `extend_path`; nothing is written for an empty growth that is refused later.
#[test]
fn extending_is_one_labelled_commit_and_keeps_style_and_ids() {
    let document = Document::new(1);
    let path = path_of(&document, &[node(1, 0.0, 0.0), node(2, 10.0, 0.0)]);
    document
        .edit_style(
            &[path],
            &StyleEdit::StrokeColor(Color { r: 200, g: 0, b: 0 }),
        )
        .unwrap();
    let style = document.path(path).unwrap().style;
    let before = labels(&document);
    document
        .extend_path(&growth(path, PathEnd::Last, vec![node(3, 20.0, 5.0)]))
        .unwrap();
    let after = labels(&document);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after.last().map(String::as_str), Some("extend_path"));
    let snapshot = document.path(path).unwrap();
    assert_eq!(snapshot.style, style);
    assert_eq!(snapshot.anchors[0].id, id(1));
    assert!(!snapshot.closed);
}

/// Criterion 8: P (0,0),(10,0) continued from its last node and joined to Q (30,0),(40,0) at
/// Q's first node: the nodes follow in order; at Q's last node: reversed. Q is gone.
#[test]
fn connecting_orders_the_absorbed_nodes_so_the_ends_meet() {
    for (qend, expected) in [
        (
            PathEnd::First,
            vec![
                (1, 0.0, 0.0),
                (2, 10.0, 0.0),
                (3, 30.0, 0.0),
                (4, 40.0, 0.0),
            ],
        ),
        (
            PathEnd::Last,
            vec![
                (1, 0.0, 0.0),
                (2, 10.0, 0.0),
                (4, 40.0, 0.0),
                (3, 30.0, 0.0),
            ],
        ),
    ] {
        let document = Document::new(1);
        let p = path_of(&document, &[node(1, 0.0, 0.0), node(2, 10.0, 0.0)]);
        let q = path_of(&document, &[node(3, 30.0, 0.0), node(4, 40.0, 0.0)]);
        let mut g = growth(p, PathEnd::Last, Vec::new());
        g.absorb = Some((q, qend));
        document.connect_paths(&g).unwrap();
        assert_eq!(order(&document, p), expected);
        assert!(document.path(q).is_none(), "Q is removed");
        assert_eq!(
            labels(&document).last().map(String::as_str),
            Some("connect_paths")
        );
    }
}

/// Criterion 8, first end: the absorbed path comes first, its joined end adjacent to P's first node.
#[test]
fn connecting_at_the_first_end_puts_the_absorbed_nodes_before_the_new_ones() {
    let document = Document::new(1);
    let p = path_of(&document, &[node(1, 100.0, 0.0), node(2, 110.0, 0.0)]);
    let q = path_of(&document, &[node(3, 60.0, 0.0), node(4, 70.0, 0.0)]);
    // Drawn from P's first node (100, 0) through (80, 0), then onto Q's last node (70, 0).
    let mut g = growth(p, PathEnd::First, vec![node(5, 80.0, 0.0)]);
    g.absorb = Some((q, PathEnd::Last));
    document.connect_paths(&g).unwrap();
    assert_eq!(
        order(&document, p),
        vec![
            (3, 60.0, 0.0),
            (4, 70.0, 0.0),
            (5, 80.0, 0.0),
            (1, 100.0, 0.0),
            (2, 110.0, 0.0)
        ]
    );
}

/// Criterion 8, reversing: the handles of a reversed absorbed node are swapped.
#[test]
fn a_reversed_absorbed_path_swaps_each_nodes_handles() {
    let document = Document::new(1);
    let p = path_of(&document, &[node(1, 0.0, 0.0), node(2, 10.0, 0.0)]);
    let mut q3 = node(3, 30.0, 0.0);
    q3.handle_in = Vec2::new(-1.0, 0.0);
    q3.handle_out = Vec2::new(2.0, 0.0);
    let q = path_of(&document, &[q3, node(4, 40.0, 0.0)]);
    let mut g = growth(p, PathEnd::Last, Vec::new());
    g.absorb = Some((q, PathEnd::Last));
    document.connect_paths(&g).unwrap();
    let snapshot = document.path(p).unwrap();
    let third = snapshot.anchors.iter().find(|a| a.id == id(3)).unwrap();
    assert_eq!(third.handle_in, Vec2::new(2.0, 0.0));
    assert_eq!(third.handle_out, Vec2::new(-1.0, 0.0));
}

/// Criterion 9: coincident ends are merged by the caller's `replace` and `drop`: one node, no
/// zero-length segment.
#[test]
fn coincident_ends_are_merged_through_replace_and_drop() {
    let document = Document::new(1);
    let p = path_of(&document, &[node(1, 0.0, 0.0), node(2, 10.0, 0.0)]);
    let q = path_of(&document, &[node(3, 10.0, 0.0), node(4, 20.0, 0.0)]);
    let merged = AnchorSnapshot {
        kind: AnchorKind::Corner,
        ..node(2, 10.0, 0.0)
    };
    let mut g = growth(p, PathEnd::Last, Vec::new());
    g.absorb = Some((q, PathEnd::First));
    g.replace = vec![merged];
    g.drop = vec![id(3)];
    document.connect_paths(&g).unwrap();
    assert_eq!(
        order(&document, p),
        vec![(1, 0.0, 0.0), (2, 10.0, 0.0), (4, 20.0, 0.0)]
    );
}

/// Criterion 10: the survivor's style stays, the other object's is discarded.
#[test]
fn the_surviving_path_keeps_its_style_on_connect() {
    let document = Document::new(1);
    let p = path_of(&document, &[node(1, 0.0, 0.0), node(2, 10.0, 0.0)]);
    let q = path_of(&document, &[node(3, 30.0, 0.0), node(4, 40.0, 0.0)]);
    document
        .edit_style(&[p], &StyleEdit::StrokeColor(Color { r: 200, g: 0, b: 0 }))
        .unwrap();
    document
        .edit_style(&[q], &StyleEdit::StrokeColor(Color { r: 0, g: 0, b: 200 }))
        .unwrap();
    let red = document.path(p).unwrap().style;
    let mut g = growth(p, PathEnd::Last, Vec::new());
    g.absorb = Some((q, PathEnd::First));
    document.connect_paths(&g).unwrap();
    assert_eq!(document.path(p).unwrap().style, red);
}

/// Criteria 14, 19: closing one or several paths is one commit labelled `close_path`; a growth
/// that adds nodes is closed in the same commit; the replaced closing node is written.
#[test]
fn closing_is_one_commit_for_every_path_with_the_join_applied() {
    let document = Document::new(1);
    let a = path_of(
        &document,
        &[node(1, 0.0, 0.0), node(2, 20.0, 0.0), node(3, 20.0, 20.0)],
    );
    let b = path_of(&document, &[node(11, 100.0, 0.0), node(12, 120.0, 0.0)]);
    let before = labels(&document);
    let smooth = AnchorSnapshot {
        handle_in: Vec2::new(-5.0, 0.0),
        handle_out: Vec2::new(5.0, 0.0),
        kind: AnchorKind::Asymmetric,
        ..node(1, 0.0, 0.0)
    };
    let mut first = growth(a, PathEnd::Last, Vec::new());
    first.replace = vec![smooth];
    // b gets a third node and closes in the same commit.
    let second = growth(b, PathEnd::Last, vec![node(13, 120.0, 20.0)]);
    document.close_paths(&[first, second]).unwrap();
    let after = labels(&document);
    assert_eq!(after.len(), before.len() + 1, "one commit for both");
    assert_eq!(after.last().map(String::as_str), Some("close_path"));
    let closed = document.path(a).unwrap();
    assert!(closed.closed);
    assert_eq!(closed.anchors[0].kind, AnchorKind::Asymmetric);
    assert_eq!(closed.anchors[0].handle_in, Vec2::new(-5.0, 0.0));
    assert!(document.path(b).unwrap().closed);
}

/// Refusals write nothing: a closed result of two nodes, a closed or unknown survivor, a
/// duplicate added id; and one bad growth stops the others of the same call.
#[test]
fn a_refusal_writes_nothing() {
    let document = Document::new(1);
    let a = path_of(
        &document,
        &[node(1, 0.0, 0.0), node(2, 10.0, 0.0), node(3, 20.0, 0.0)],
    );
    let two = path_of(&document, &[node(11, 100.0, 0.0), node(12, 110.0, 0.0)]);
    let before = labels(&document);
    let version = document.version();
    assert_eq!(
        document.close_paths(&[
            growth(a, PathEnd::Last, Vec::new()),
            growth(two, PathEnd::Last, Vec::new())
        ]),
        Err(PathEditError::TooFewToClose)
    );
    assert!(
        !document.path(a).unwrap().closed,
        "the valid growth was not applied either"
    );
    assert_eq!(
        document.extend_path(&growth(a, PathEnd::Last, vec![node(2, 30.0, 0.0)])),
        Err(PathEditError::DuplicateAnchorId)
    );
    let mut absorbs_itself = growth(a, PathEnd::Last, Vec::new());
    absorbs_itself.absorb = Some((a, PathEnd::First));
    assert_eq!(
        document.connect_paths(&absorbs_itself),
        Err(PathEditError::NotExtendable)
    );
    let mut extends_with_absorb = growth(a, PathEnd::Last, Vec::new());
    extends_with_absorb.absorb = Some((two, PathEnd::First));
    assert_eq!(
        document.extend_path(&extends_with_absorb),
        Err(PathEditError::NotExtendable)
    );
    assert_eq!(labels(&document), before);
    assert_eq!(document.version(), version, "nothing changed");
    document
        .close_paths(&[growth(a, PathEnd::Last, Vec::new())])
        .unwrap();
    assert_ne!(document.version(), version, "a commit changes the version");
    assert_eq!(
        document.extend_path(&growth(a, PathEnd::Last, vec![node(21, 0.0, 5.0)])),
        Err(PathEditError::NotExtendable),
        "a closed path cannot be extended"
    );
}
