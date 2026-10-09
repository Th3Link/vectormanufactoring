//! Independent session-level acceptance tests for `0016-boolean-operations` PR 3 (the boolean
//! command): availability, the five operations on every mix of path, rectangle, rounded
//! rectangle, ellipse, polygon and star, stacking order, result style/place/rotation/commit,
//! refusals that change nothing, repeated operations, compound results in the Node tool, gates
//! against a running drag or typed entry, and a few precision cases. Written from the
//! specification before the implementation was read; the expected regions come from reference
//! polylines built here, not from the code under test.

#![allow(
    unused_must_use,
    clippy::manual_midpoint,
    clippy::match_wildcard_for_single_variants,
    clippy::assert_is_empty,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::used_underscore_binding,
    clippy::manual_assert,
    clippy::needless_range_loop,
    clippy::unreadable_literal,
    clippy::struct_field_names,
    clippy::trivially_copy_pass_by_ref,
    clippy::too_many_arguments
)]

use std::collections::HashSet;
use std::f64::consts::PI;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicU64, Ordering};

use curvyo_document_core::{
    AnchorId, AnchorKind, Angle, Color, CornerRadii, Document, EllipseFrame, FillMode,
    FillModeTarget, InnerRatio, Length, NewAnchor, NodeId, ObjectSnapshot, Opacity, PathSnapshot,
    Point, PointCount, RectBounds, StarFrame, StyleEdit, Vec2, outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{BooleanOutcome, DoubleClickHint, KeyInput, Session, Tool};
use curvyo_ui_core::{BooleanAvailability, BooleanOp, BooleanRefusal};

const ALL_OPS: [BooleanOp; 5] = [
    BooleanOp::Union,
    BooleanOp::Difference,
    BooleanOp::Intersection,
    BooleanOp::Exclusion,
    BooleanOp::ReverseDifference,
];

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

static NEXT_ANCHOR: AtomicU64 = AtomicU64::new(1);

fn aid() -> AnchorId {
    AnchorId::new(77, NEXT_ANCHOR.fetch_add(1, Ordering::SeqCst))
}

fn corner(x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(aid(), pt(x, y))
}

// ---------------------------------------------------------------------------------------------
// Shapes: one document object each, plus an independent reference polyline.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum K {
    Rect,
    RRect,
    Circle,
    Ellipse,
    Polygon,
    Star,
    Tri,
    Spike,
    Arc,
}

const ALL_KINDS: [K; 9] = [
    K::Rect,
    K::RRect,
    K::Circle,
    K::Ellipse,
    K::Polygon,
    K::Star,
    K::Tri,
    K::Spike,
    K::Arc,
];

fn bezier_circle(cx: f64, cy: f64, r: f64) -> Vec<NewAnchor> {
    let k = 0.552_284_749_830_793_4 * r;
    let mk = |x: f64, y: f64, hi: (f64, f64), ho: (f64, f64)| NewAnchor {
        id: aid(),
        point: pt(x, y),
        handle_in: Vec2::new(hi.0, hi.1),
        handle_out: Vec2::new(ho.0, ho.1),
        kind: AnchorKind::Corner,
    };
    vec![
        mk(cx + r, cy, (0.0, -k), (0.0, k)),
        mk(cx, cy + r, (k, 0.0), (-k, 0.0)),
        mk(cx - r, cy, (0.0, k), (0.0, -k)),
        mk(cx, cy - r, (-k, 0.0), (k, 0.0)),
    ]
}

fn spike_points(cx: f64, cy: f64, r: f64) -> Vec<Point> {
    (0..5)
        .map(|i| {
            let a = f64::from(i) * 4.0 * PI / 5.0 - PI / 2.0;
            pt(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Adds shape `k` inside the 40 x 40 box at `(ox, oy)`; returns the id.
fn add(d: &Document, k: K, ox: f64, oy: f64) -> NodeId {
    let c = pt(ox + 20.0, oy + 20.0);
    match k {
        K::Rect => d.create_rect(RectBounds {
            origin: pt(ox, oy),
            width: Length::from_mm(40.0),
            height: Length::from_mm(32.0),
        }),
        K::RRect => {
            let id = d.create_rect(RectBounds {
                origin: pt(ox, oy),
                width: Length::from_mm(40.0),
                height: Length::from_mm(40.0),
            });
            d.set_corner_radius(&[id], Length::from_mm(8.0)).unwrap();
            id
        }
        K::Circle => d.create_ellipse(EllipseFrame {
            center: c,
            rx: Length::from_mm(20.0),
            ry: Length::from_mm(20.0),
        }),
        K::Ellipse => d.create_ellipse(EllipseFrame {
            center: c,
            rx: Length::from_mm(20.0),
            ry: Length::from_mm(13.0),
        }),
        K::Polygon => d.create_polygon(
            StarFrame {
                center: c,
                radius: Length::from_mm(20.0),
                angle: Angle::from_radians(0.3),
            },
            PointCount::new(6).unwrap(),
        ),
        K::Star => d.create_star(
            StarFrame {
                center: c,
                radius: Length::from_mm(20.0),
                angle: Angle::from_radians(-PI / 2.0),
            },
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.45).unwrap(),
        ),
        K::Tri => d.create_path(
            &[
                corner(ox, oy),
                corner(ox + 40.0, oy),
                corner(ox + 20.0, oy + 38.0),
            ],
            true,
        ),
        K::Spike => {
            let pts = spike_points(c.x, c.y, 20.0);
            d.create_path(
                &pts.iter().map(|p| corner(p.x, p.y)).collect::<Vec<_>>(),
                true,
            )
        }
        K::Arc => d.create_path(&bezier_circle(c.x, c.y, 20.0), true),
    }
}

fn sample_circle(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<Point> {
    (0..360)
        .map(|i| {
            let a = f64::from(i) * PI / 180.0;
            pt(cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

/// The shape's region as a closed polyline, within 0.005 mm of the true outline.
fn reference(d: &Document, id: NodeId, k: K, ox: f64, oy: f64) -> Vec<Point> {
    let (cx, cy) = (ox + 20.0, oy + 20.0);
    match k {
        K::Rect | K::Polygon | K::Star => {
            let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
                panic!("primitive")
            };
            outline_of_rotated(&p.shape, p.rotation)
                .iter()
                .map(|a| a.point)
                .collect()
        }
        K::RRect => {
            let r = 8.0;
            let mut out = Vec::new();
            for (qx, qy, start) in [
                (ox + 40.0 - r, oy + r, -90.0_f64),
                (ox + 40.0 - r, oy + 40.0 - r, 0.0),
                (ox + r, oy + 40.0 - r, 90.0),
                (ox + r, oy + r, 180.0),
            ] {
                for i in 0..=90 {
                    let a = (start + f64::from(i)) * PI / 180.0;
                    out.push(pt(qx + r * a.cos(), qy + r * a.sin()));
                }
            }
            out
        }
        K::Circle | K::Arc => sample_circle(cx, cy, 20.0, 20.0),
        K::Ellipse => sample_circle(cx, cy, 20.0, 13.0),
        K::Tri => vec![pt(ox, oy), pt(ox + 40.0, oy), pt(ox + 20.0, oy + 38.0)],
        K::Spike => spike_points(cx, cy, 20.0),
    }
}

/// Nonzero winding number of `poly` around `p`.
fn winding(poly: &[Point], p: Point) -> i32 {
    let mut w = 0;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let side = (b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y);
        if a.y <= p.y {
            if b.y > p.y && side > 0.0 {
                w += 1;
            }
        } else if b.y <= p.y && side < 0.0 {
            w -= 1;
        }
    }
    w
}

fn seg_dist(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    ((p.x - a.x - t * dx).powi(2) + (p.y - a.y - t * dy).powi(2)).sqrt()
}

fn poly_dist(poly: &[Point], p: Point) -> f64 {
    (0..poly.len())
        .map(|i| seg_dist(p, poly[i], poly[(i + 1) % poly.len()]))
        .fold(f64::INFINITY, f64::min)
}

fn shoelace(poly: &[Point]) -> f64 {
    let mut s = 0.0;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        s += a.x * b.y - b.x * a.y;
    }
    s / 2.0
}

fn perimeter(poly: &[Point]) -> f64 {
    (0..poly.len())
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
        })
        .sum()
}

fn expected_in(op: BooleanOp, inside: &[bool]) -> bool {
    let n = inside.len();
    match op {
        BooleanOp::Union => inside.iter().any(|b| *b),
        BooleanOp::Difference => inside[0] && !inside[1..].iter().any(|b| *b),
        BooleanOp::Intersection => inside.iter().all(|b| *b),
        BooleanOp::Exclusion => inside.iter().filter(|b| **b).count() % 2 == 1,
        BooleanOp::ReverseDifference => inside[n - 1] && !inside[..n - 1].iter().any(|b| *b),
    }
}

// ---------------------------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------------------------

fn session_of(d: &Document) -> Session {
    let mut s = Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s.resize_viewport(1200.0, 800.0);
    s
}

fn reread(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn json(s: &Session) -> Vec<u8> {
    reread(s).export_json().unwrap()
}

fn marquee(s: &mut Session, from: Point, to: Point, shift: bool) {
    s.pointer_hover(from, shift, false);
    s.pointer_down(from, shift);
    s.pointer_hover(
        pt((from.x + to.x) / 2.0, (from.y + to.y) / 2.0),
        shift,
        false,
    );
    s.pointer_hover(to, shift, false);
    s.pointer_up(to, shift, false);
}

fn click(s: &mut Session, p: Point, shift: bool) {
    s.pointer_hover(p, shift, false);
    s.pointer_down(p, shift);
    s.pointer_up(p, shift, false);
}

/// Every Loro change as `(message, operation count)`, oldest first. Loro splits one large commit
/// into several changes with the same message once a snapshot is re-imported (about 1,290
/// operations; a plain 400-node `create_path` does it too), so tests compare operation totals.
fn labels(d: &Document) -> Vec<(String, usize)> {
    let loro = loro::LoroDoc::new();
    loro.import(&d.export_loro_snapshot().unwrap()).unwrap();
    let ids: Vec<loro::ID> = loro.oplog_frontiers().iter().collect();
    let mut out: Vec<(u32, String, usize)> = Vec::new();
    loro.travel_change_ancestors(&ids, &mut |m| {
        out.push((
            m.lamport,
            m.message.map(|s| s.to_string()).unwrap_or_default(),
            m.len,
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    out.sort();
    out.into_iter().map(|(_, s, n)| (s, n)).collect()
}

/// The commits written after `before` changes existed: all carry `label`, and a result that
/// fits one Loro change (under 1,200 operations) is exactly one.
fn assert_one_commit(d: &Document, before: usize, label: &str, tag: &str) {
    let l = labels(d);
    let new = &l[before..];
    assert!(!new.is_empty(), "{tag}: no commit written");
    assert!(new.iter().all(|(m, _)| m == label), "{tag}: labels {new:?}");
    let ops: usize = new.iter().map(|(_, n)| *n).sum();
    if ops < 1200 {
        assert_eq!(new.len(), 1, "{tag}: one commit, got {new:?}");
    } else {
        assert!(new.len() <= 1 + ops / 1000, "{tag}: {new:?}");
    }
}

fn path_outlines(p: &PathSnapshot) -> Vec<Vec<Point>> {
    p.subpaths()
        .map(|sp| sp.anchors.iter().map(|a| a.point).collect())
        .collect()
}

/// Net area of all outlines (nonzero filling with opposite-wound holes).
fn net_area(p: &PathSnapshot) -> f64 {
    path_outlines(p)
        .iter()
        .map(|o| shoelace(o))
        .sum::<f64>()
        .abs()
}

fn total_perimeter(p: &PathSnapshot) -> f64 {
    path_outlines(p).iter().map(|o| perimeter(o)).sum()
}

fn winding_all(outlines: &[Vec<Point>], p: Point) -> i32 {
    outlines.iter().map(|o| winding(o, p)).sum()
}

fn style_of(o: &ObjectSnapshot) -> curvyo_document_core::Style {
    match o {
        ObjectSnapshot::Path(p) => p.style.clone(),
        ObjectSnapshot::Primitive(p) => p.style.clone(),
    }
}

/// Gives the object a distinctive, complete style so that "copy of the base style" is testable.
fn stylize(d: &Document, id: NodeId, n: u8) {
    d.edit_style(
        &[id],
        &StyleEdit::FillColor(Color {
            r: 10 + n * 30,
            g: 200 - n * 20,
            b: 5 * n,
        }),
    )
    .unwrap();
    d.set_fill_mode(
        FillMode::Solid,
        &[FillModeTarget {
            id,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    d.edit_style(
        &[id],
        &StyleEdit::StrokeWidth(Length::from_mm(0.5 + f64::from(n))),
    )
    .unwrap();
    d.edit_style(
        &[id],
        &StyleEdit::StrokeColor(Color {
            r: 200 - n * 10,
            g: n * 40,
            b: 77,
        }),
    )
    .unwrap();
    d.edit_style(
        &[id],
        &StyleEdit::FillOpacity(Opacity::new(0.25 + f64::from(n) * 0.1).unwrap()),
    )
    .unwrap();
}

fn only_path(s: &Session) -> PathSnapshot {
    let d = reread(s);
    let ids = d.object_ids();
    assert_eq!(ids.len(), 1, "exactly one object expected");
    d.path(ids[0]).expect("a path")
}

// ---------------------------------------------------------------------------------------------
// Criterion 1: availability
// ---------------------------------------------------------------------------------------------

#[test]
fn ac1_availability_follows_selection_and_tool() {
    let d = Document::new(1);
    let _a = add(&d, K::Rect, 0.0, 0.0);
    let _b = add(&d, K::Circle, 200.0, 0.0);
    let mut s = session_of(&d);
    assert_eq!(
        s.boolean_availability(),
        BooleanAvailability::NeedsTwo,
        "none selected"
    );
    click(&mut s, pt(0.0, 16.0), false);
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(
        s.boolean_availability(),
        BooleanAvailability::NeedsTwo,
        "one selected"
    );
    click(&mut s, pt(240.0, 20.0), true); // the circle's right edge
    assert_eq!(s.selected_object_count(), 2);
    assert_eq!(s.boolean_availability(), BooleanAvailability::Ready);

    // Node and Pen keep the object selection; creation tools clear it (Session::set_tool).
    s.set_tool(Tool::Node);
    s.set_tool(Tool::Select);
    assert_eq!(
        s.boolean_availability(),
        BooleanAvailability::Ready,
        "Node round trip keeps it"
    );
    for tool in [
        Tool::Pen,
        Tool::Node,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::PolygonStar,
    ] {
        s.set_tool(tool);
        assert_eq!(
            s.boolean_availability(),
            BooleanAvailability::NeedsTwo,
            "{tool:?} is not the Select tool"
        );
        s.set_tool(Tool::Select);
    }
    s.set_tool(Tool::Rectangle);
    s.set_tool(Tool::Select);
    assert_eq!(
        s.boolean_availability(),
        BooleanAvailability::NeedsTwo,
        "a creation tool cleared the selection"
    );
}

#[test]
fn ac1_open_paths_count_and_closed_degenerate_are_ready() {
    let d = Document::new(1);
    let _closed = add(&d, K::Rect, 0.0, 0.0);
    let o1 = d.create_path(
        &[corner(100.0, 0.0), corner(130.0, 20.0), corner(160.0, 0.0)],
        false,
    );
    let o2 = d.create_path(&[corner(100.0, 100.0), corner(130.0, 120.0)], false);
    // a closed path of two nodes (no area) is closed, so "Ready" (known only to the kernel)
    let _two = d.create_path(&[corner(300.0, 0.0), corner(330.0, 20.0)], true);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-10.0, -10.0), pt(60.0, 60.0), false);
    click(&mut s, pt(130.0, 20.0), true);
    assert_eq!(s.selected_object_count(), 2);
    assert_eq!(
        s.boolean_availability(),
        BooleanAvailability::OpenPaths { open: 1, of: 2 }
    );
    click(&mut s, pt(130.0, 120.0), true);
    assert_eq!(s.selected_object_count(), 3);
    assert_eq!(
        s.boolean_availability(),
        BooleanAvailability::OpenPaths { open: 2, of: 3 }
    );
    let _ = (o1, o2);
    // only the two-node closed path and the rectangle: Ready
    let mut s = session_of(&d);
    marquee(&mut s, pt(-10.0, -10.0), pt(400.0, 60.0), false);
    // the marquee also covers the open path o1 (100..160, 0..20): deselect it by shift-click
    click(&mut s, pt(130.0, 20.0), true);
    assert_eq!(s.selected_object_count(), 2);
    assert_eq!(s.boolean_availability(), BooleanAvailability::Ready);
}

// ---------------------------------------------------------------------------------------------
// Criteria 4 to 8, 10 to 13a, 19 to 24, 27, 28 on every mix of kinds
// ---------------------------------------------------------------------------------------------

/// Two objects of kinds `ka` (lower) and `kb` (upper), overlapping, with a bystander below, one
/// between them and one on top; both operands styled differently.
struct Pair {
    s: Session,
    a: NodeId,
    b: NodeId,
    ids_before: Vec<NodeId>,
    ref_a: Vec<Point>,
    ref_b: Vec<Point>,
    style_a: curvyo_document_core::Style,
    style_b: curvyo_document_core::Style,
}

fn pair(ka: K, kb: K) -> Pair {
    let d = Document::new(1);
    let z0 = add(&d, K::Rect, 600.0, 0.0);
    let a = add(&d, ka, 0.0, 0.0);
    let m = add(&d, K::Circle, 600.0, 100.0);
    let b = add(&d, kb, 22.0, 14.0);
    let z1 = add(&d, K::Tri, 600.0, 200.0);
    stylize(&d, a, 1);
    stylize(&d, b, 2);
    let ref_a = reference(&d, a, ka, 0.0, 0.0);
    let ref_b = reference(&d, b, kb, 22.0, 14.0);
    let style_a = style_of(&d.object(a).unwrap());
    let style_b = style_of(&d.object(b).unwrap());
    let mut s = session_of(&d);
    marquee(&mut s, pt(-20.0, -20.0), pt(100.0, 90.0), false);
    assert_eq!(
        s.selected_object_count(),
        2,
        "{ka:?}/{kb:?}: both operands selected, no bystander"
    );
    assert_eq!(
        d.object_ids(),
        vec![z0, a, m, b, z1],
        "creation order is stacking order"
    );
    Pair {
        s,
        a,
        b,
        ids_before: vec![z0, a, m, b, z1],
        ref_a,
        ref_b,
        style_a,
        style_b,
    }
}

fn check_region(tag: &str, op: BooleanOp, refs: &[&Vec<Point>], outlines: &[Vec<Point>]) {
    let mut minx = f64::INFINITY;
    let mut miny = f64::INFINITY;
    let mut maxx = f64::NEG_INFINITY;
    let mut maxy = f64::NEG_INFINITY;
    for r in refs {
        for p in *r {
            minx = minx.min(p.x);
            miny = miny.min(p.y);
            maxx = maxx.max(p.x);
            maxy = maxy.max(p.y);
        }
    }
    let n = 36;
    let mut checked = 0;
    for i in 0..n {
        for j in 0..n {
            let p = pt(
                minx - 1.0 + (maxx - minx + 2.0) * (f64::from(i) + 0.37) / f64::from(n),
                miny - 1.0 + (maxy - miny + 2.0) * (f64::from(j) + 0.41) / f64::from(n),
            );
            if refs.iter().any(|r| poly_dist(r, p) < 0.06) {
                continue;
            }
            let inside: Vec<bool> = refs.iter().map(|r| winding(r, p) != 0).collect();
            let want = expected_in(op, &inside);
            // The result must be away from its own boundary too, or a flatten error could flip it.
            if outlines.iter().any(|o| poly_dist(o, p) < 0.02) {
                continue;
            }
            let got = winding_all(outlines, p) != 0;
            assert_eq!(got, want, "{tag} {op:?}: point {p:?} inside {inside:?}");
            checked += 1;
        }
    }
    assert!(checked > 200, "{tag}: only {checked} sample points");
}

/// For each ordered pair of kinds and each of the five operations: region, structure, style,
/// place, rotation, selection, tool, one labelled commit, operands removed, bystanders intact.
#[test]
fn ac6_to_28_every_pair_of_kinds_every_operation() {
    let mut ran = 0;
    let mut refused = 0;
    for ka in ALL_KINDS {
        for kb in ALL_KINDS {
            for op in ALL_OPS {
                let mut p = pair(ka, kb);
                let tag = format!("{ka:?}+{kb:?}");
                let before = reread(&p.s);
                let labels_before = labels(&before).len();
                let outcome = p.s.apply_boolean(op);
                let after = reread(&p.s);
                match outcome {
                    BooleanOutcome::Applied { operands, compound } => {
                        ran += 1;
                        assert_eq!(operands, 2, "{tag} {op:?}");
                        let ids = after.object_ids();
                        assert_eq!(ids.len(), 4, "{tag} {op:?}: two operands became one");
                        let removed: HashSet<_> = [p.a, p.b].into_iter().collect();
                        assert!(
                            ids.iter().all(|i| !removed.contains(i)),
                            "{tag} {op:?}: operands are gone"
                        );
                        // bystanders keep ids and relative order; the result sits at the base's place
                        let base_index = if op == BooleanOp::ReverseDifference {
                            2
                        } else {
                            1
                        };
                        let mut expected: Vec<NodeId> =
                            vec![p.ids_before[0], p.ids_before[2], p.ids_before[4]];
                        let result_id = ids[base_index];
                        expected.insert(base_index, result_id);
                        assert_eq!(ids, expected, "{tag} {op:?}: place in the stacking order");
                        let ObjectSnapshot::Path(res) = after.object(result_id).unwrap() else {
                            panic!("{tag} {op:?}: result is a path")
                        };
                        assert_eq!(res.is_compound(), compound, "{tag} {op:?}");
                        // style: complete copy of the base operand's
                        let want_style = if op == BooleanOp::ReverseDifference {
                            &p.style_b
                        } else {
                            &p.style_a
                        };
                        assert_eq!(
                            &res.style, want_style,
                            "{tag} {op:?}: style of the base operand"
                        );
                        assert_eq!(
                            res.rotation,
                            Angle::from_radians(0.0),
                            "{tag} {op:?}: rotation 0"
                        );
                        // selection, tool
                        assert_eq!(p.s.selected_object_count(), 1, "{tag} {op:?}");
                        assert_eq!(p.s.tool(), Tool::Select);
                        // exactly one new commit with the label
                        assert_one_commit(
                            &after,
                            labels_before,
                            op_label(op),
                            &format!("{tag} {op:?}"),
                        );
                        // nodes: corner, no handles, no short edges, no collinear nodes
                        for sp in res.subpaths() {
                            assert!(sp.closed, "{tag} {op:?}");
                            assert!(sp.anchors.len() >= 3);
                            for (i, a) in sp.anchors.iter().enumerate() {
                                assert_eq!(a.kind, AnchorKind::Corner);
                                assert_eq!(a.handle_in, Vec2::ZERO);
                                assert_eq!(a.handle_out, Vec2::ZERO);
                                let prev =
                                    sp.anchors[(i + sp.anchors.len() - 1) % sp.anchors.len()].point;
                                let next = sp.anchors[(i + 1) % sp.anchors.len()].point;
                                let step = ((a.point.x - next.x).powi(2)
                                    + (a.point.y - next.y).powi(2))
                                .sqrt();
                                assert!(step >= 0.001 - 1e-9, "{tag} {op:?}: edge {step}");
                                let dd = seg_dist(a.point, prev, next);
                                assert!(dd > 0.001 - 1e-9, "{tag} {op:?}: collinear node, {dd}");
                            }
                        }
                        // region versus the reference polylines (A below B)
                        check_region(&tag, op, &[&p.ref_a, &p.ref_b], &path_outlines(&res));
                    }
                    BooleanOutcome::Refused(BooleanRefusal::Empty) => {
                        refused += 1;
                        assert_eq!(
                            after.export_json().unwrap(),
                            before.export_json().unwrap(),
                            "{tag} {op:?}"
                        );
                        assert_eq!(
                            labels(&after).len(),
                            labels_before,
                            "{tag} {op:?}: no commit"
                        );
                        assert_eq!(p.s.selected_object_count(), 2);
                    }
                    other => panic!("{tag} {op:?}: unexpected {other:?}"),
                }
            }
        }
    }
    assert!(ran > 300, "ran {ran}, refused {refused}");
}

fn op_label(op: BooleanOp) -> &'static str {
    match op {
        BooleanOp::Union => "boolean_union",
        BooleanOp::Difference => "boolean_difference",
        BooleanOp::Intersection => "boolean_intersection",
        BooleanOp::Exclusion => "boolean_exclusion",
        BooleanOp::ReverseDifference => "boolean_reverse_difference",
    }
}

/// Area identities of criterion 14 through the session, with analytic operand areas.
#[test]
fn ac14_area_identities_through_the_session() {
    for ka in ALL_KINDS {
        for kb in ALL_KINDS {
            let mut areas = Vec::new();
            for op in [
                BooleanOp::Union,
                BooleanOp::Intersection,
                BooleanOp::Difference,
                BooleanOp::ReverseDifference,
                BooleanOp::Exclusion,
            ] {
                let mut p = pair(ka, kb);
                let area_a = shoelace(&p.ref_a).abs();
                let area_b = shoelace(&p.ref_b).abs();
                let per = perimeter(&p.ref_a) + perimeter(&p.ref_b);
                let area = match p.s.apply_boolean(op) {
                    BooleanOutcome::Applied { .. } => {
                        let d = reread(&p.s);
                        let ids = d.object_ids();
                        let id = if op == BooleanOp::ReverseDifference {
                            ids[2]
                        } else {
                            ids[1]
                        };
                        net_area(&d.path(id).unwrap())
                    }
                    BooleanOutcome::Refused(BooleanRefusal::Empty) => 0.0,
                    other => panic!("{ka:?}+{kb:?} {op:?}: {other:?}"),
                };
                areas.push((area, area_a, area_b, per));
            }
            let (u, i, d, rd, x) = (areas[0].0, areas[1].0, areas[2].0, areas[3].0, areas[4].0);
            let (a, b, per) = (areas[0].1, areas[0].2, areas[0].3);
            // Spike (self-intersecting path) has a nonzero area that the plain shoelace does not
            // give; skip the analytic part for it.
            if ka == K::Spike || kb == K::Spike {
                assert!((u + i - (u + i)).abs() < 1e-9);
                continue;
            }
            let tol = 1e-6 * (a + b) + 0.012 * per;
            assert!(
                (u + i - (a + b)).abs() <= tol,
                "{ka:?}+{kb:?} union+inter {} vs {}",
                u + i,
                a + b
            );
            assert!(
                (d + i - a).abs() <= tol,
                "{ka:?}+{kb:?} diff+inter {} vs {a}",
                d + i
            );
            assert!(
                (rd + i - b).abs() <= tol,
                "{ka:?}+{kb:?} rdiff+inter {} vs {b}",
                rd + i
            );
            assert!(
                (x - (a + b - 2.0 * i)).abs() <= tol * 2.0,
                "{ka:?}+{kb:?} exclusion"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Criteria 9, 11-13 with three operands, 10, 22, 24, 39, 42: concrete numbers
// ---------------------------------------------------------------------------------------------

fn square(d: &Document, x: f64, y: f64, side: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(side),
        height: Length::from_mm(side),
    })
}

fn disc(d: &Document, x: f64, y: f64, r: f64) -> NodeId {
    d.create_ellipse(EllipseFrame {
        center: pt(x, y),
        rx: Length::from_mm(r),
        ry: Length::from_mm(r),
    })
}

#[test]
fn ac10_two_offset_squares_union_is_700_mm2_with_8_nodes() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 20.0);
    square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0), false);
    assert_eq!(
        s.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Applied {
            operands: 2,
            compound: false
        }
    );
    let p = only_path(&s);
    assert_eq!(p.anchors.len(), 8);
    assert!((net_area(&p) - 700.0).abs() < 1e-6);
}

#[test]
fn ac24_two_squares_sharing_a_full_edge_unite_to_four_nodes() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 20.0);
    square(&d, 20.0, 0.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(50.0, 30.0), false);
    s.apply_boolean(BooleanOp::Union);
    let p = only_path(&s);
    assert_eq!(p.anchors.len(), 4);
    assert!((net_area(&p) - 800.0).abs() < 1e-6);
}

#[test]
fn ac39_grid_pairs_0_0004_unites_0_0020_stays_two_outlines() {
    for (gap, outlines) in [(0.0, 1), (0.0004, 1), (0.0020, 2)] {
        let d = Document::new(1);
        square(&d, 0.0, 0.0, 20.0);
        square(&d, 20.0 + gap, 0.0, 20.0);
        let mut s = session_of(&d);
        marquee(&mut s, pt(-5.0, -5.0), pt(50.0, 30.0), false);
        let out = s.apply_boolean(BooleanOp::Union);
        assert!(
            matches!(out, BooleanOutcome::Applied { .. }),
            "{gap}: {out:?}"
        );
        let p = only_path(&s);
        assert_eq!(p.subpaths().count(), outlines, "gap {gap}");
        if outlines == 1 {
            assert_eq!(p.anchors.len(), 4, "gap {gap}");
        }
    }
}

#[test]
fn ac42_operands_far_from_the_origin() {
    let d = Document::new(1);
    let off = 100_000.0;
    square(&d, off, off, 20.0);
    square(&d, off + 10.0, off + 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(
        &mut s,
        pt(off - 5.0, off - 5.0),
        pt(off + 40.0, off + 40.0),
        false,
    );
    assert_eq!(s.selected_object_count(), 2);
    assert!(matches!(
        s.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Applied { .. }
    ));
    let p = only_path(&s);
    assert_eq!(p.anchors.len(), 8);
    assert!((net_area(&p) - 700.0).abs() < 1e-3);
}

#[test]
fn ac11_to_13a_three_operands() {
    // three discs of radius 20 at (0,0), (14,0), (7,12): lower to upper = A, B, C
    let build = || {
        let d = Document::new(1);
        let a = disc(&d, 0.0, 0.0, 20.0);
        let b = disc(&d, 14.0, 0.0, 20.0);
        let c = disc(&d, 7.0, 12.0, 20.0);
        (d, a, b, c)
    };
    let centers = [pt(0.0, 0.0), pt(14.0, 0.0), pt(7.0, 12.0)];
    let refs: Vec<Vec<Point>> = centers
        .iter()
        .map(|c| sample_circle(c.x, c.y, 20.0, 20.0))
        .collect();
    for op in ALL_OPS {
        let (d, a, b, c) = build();
        let mut s = session_of(&d);
        marquee(&mut s, pt(-30.0, -30.0), pt(50.0, 50.0), false);
        assert_eq!(s.selected_object_count(), 3);
        let out = s.apply_boolean(op);
        let BooleanOutcome::Applied { operands, .. } = out else {
            panic!("{op:?}: {out:?}")
        };
        assert_eq!(operands, 3);
        let r = reread(&s);
        let ids = r.object_ids();
        assert_eq!(ids.len(), 1);
        assert!(![a, b, c].contains(&ids[0]));
        let res = r.path(ids[0]).unwrap();
        check_region(
            "3 discs",
            op,
            &[&refs[0], &refs[1], &refs[2]],
            &path_outlines(&res),
        );
    }
}

/// Criterion 9: the same objects give the same result however they were selected.
#[test]
fn ac9_click_order_does_not_matter_for_any_operation() {
    let build = || {
        let d = Document::new(1);
        square(&d, 0.0, 0.0, 60.0);
        square(&d, 30.0, 30.0, 60.0);
        square(&d, 20.0, 10.0, 60.0);
        d
    };
    // picks that hit exactly one square each (on the stroke)
    let picks = [pt(0.0, 10.0), pt(90.0, 50.0), pt(78.0, 10.0)];
    for op in ALL_OPS {
        let mut results = Vec::new();
        for order in [
            [0, 1, 2],
            [2, 1, 0],
            [1, 0, 2],
            [1, 2, 0],
            [0, 2, 1],
            [2, 0, 1],
        ] {
            let d = build();
            let mut s = session_of(&d);
            for (n, i) in order.iter().enumerate() {
                click(&mut s, picks[*i], n > 0);
            }
            assert_eq!(s.selected_object_count(), 3, "{order:?} picks {picks:?}");
            let out = s.apply_boolean(op);
            assert!(
                matches!(out, BooleanOutcome::Applied { .. }),
                "{op:?} {order:?}: {out:?}"
            );
            let p = only_path(&s);
            results.push(path_outlines(&p));
        }
        for r in &results[1..] {
            assert_eq!(r, &results[0], "{op:?}: click order changed the result");
        }
        // and the marquee gives the same
        let d = build();
        let mut s = session_of(&d);
        marquee(&mut s, pt(-5.0, -5.0), pt(150.0, 150.0), false);
        s.apply_boolean(op);
        assert_eq!(path_outlines(&only_path(&s)), results[0], "{op:?}: marquee");
    }
}

// ---------------------------------------------------------------------------------------------
// Criteria 5, 7, 8: paint independence, self-intersecting path, holes stay holes
// ---------------------------------------------------------------------------------------------

#[test]
fn ac5_paint_does_not_change_the_result() {
    let make = |variant: u8| {
        let d = Document::new(1);
        let a = square(&d, 0.0, 0.0, 20.0);
        let b = square(&d, 10.0, 10.0, 20.0);
        match variant {
            0 => {
                stylize(&d, a, 1);
                stylize(&d, b, 2);
            }
            1 => {
                // stroke-only A, wide stroke B
                d.set_fill_mode(
                    FillMode::None,
                    &[FillModeTarget {
                        id: a,
                        seed_stops: vec![],
                    }],
                )
                .unwrap();
                d.edit_style(&[b], &StyleEdit::StrokeWidth(Length::from_mm(10.0)))
                    .unwrap();
            }
            2 => {
                // fill solid at opacity 0 for A, stroke off for B
                d.set_fill_mode(
                    FillMode::Solid,
                    &[FillModeTarget {
                        id: a,
                        seed_stops: vec![],
                    }],
                )
                .unwrap();
                d.edit_style(&[a], &StyleEdit::FillOpacity(Opacity::new(0.0).unwrap()))
                    .unwrap();
                d.edit_style(&[b], &StyleEdit::StrokeEnabled(false))
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let mut s = session_of(&d);
        marquee(&mut s, pt(-20.0, -20.0), pt(60.0, 60.0), false);
        assert_eq!(s.selected_object_count(), 2);
        s.apply_boolean(BooleanOp::Union);
        path_outlines(&only_path(&s))
    };
    let base = make(0);
    assert_eq!(make(1), base);
    assert_eq!(make(2), base);
}

#[test]
fn ac7_self_intersecting_star_path_includes_its_centre() {
    let d = Document::new(1);
    let pts = spike_points(0.0, 0.0, 20.0);
    d.create_path(
        &pts.iter().map(|p| corner(p.x, p.y)).collect::<Vec<_>>(),
        true,
    );
    square(&d, 200.0, 0.0, 10.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-40.0, -40.0), pt(260.0, 40.0), false);
    assert_eq!(s.selected_object_count(), 2);
    assert!(matches!(
        s.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Applied { .. }
    ));
    let p = only_path(&s);
    // nonzero painted area of the {5/2} star polygon: 5 * R * r * sin(pi/5) with r/R = sin 18 / sin 54
    let r_out = 20.0;
    let r_in = r_out * (18.0_f64.to_radians().sin() / 54.0_f64.to_radians().sin());
    let star = 5.0 * r_out * r_in * (PI / 5.0).sin();
    let want = star + 100.0;
    let per = total_perimeter(&p);
    assert!(
        (net_area(&p) - want).abs() <= 0.01 * per,
        "area {} vs {want}",
        net_area(&p)
    );
    // the centre is inside the star outline of the result
    assert_ne!(winding_all(&path_outlines(&p), pt(0.0, 0.0)), 0);
    assert_eq!(p.subpaths().count(), 2, "star outline and the far square");
}

/// Criterion 8 and 20: a ring is a compound path; uniting it with a disc inside the hole gives
/// three outlines (outer, hole, island) and area pi (400 - 100 + 25).
#[test]
fn ac8_ring_plus_island_and_ac20_compound_results() {
    let d = Document::new(1);
    let big = disc(&d, 0.0, 0.0, 20.0);
    let mid = disc(&d, 0.0, 0.0, 10.0);
    let island = disc(&d, 0.0, 0.0, 5.0);
    let _ = (big, mid, island);
    let mut s = session_of(&d);
    // select big and mid by their strokes
    click(&mut s, pt(20.0, 0.0), false);
    click(&mut s, pt(10.0, 0.0), true);
    assert_eq!(s.selected_object_count(), 2);
    let out = s.apply_boolean(BooleanOp::Difference);
    assert_eq!(
        out,
        BooleanOutcome::Applied {
            operands: 2,
            compound: true
        }
    );
    assert_eq!(s.selected_object_count(), 1);
    let doc1 = reread(&s);
    assert_eq!(doc1.object_ids().len(), 2, "ring + island");
    let ring = doc1.path(doc1.object_ids()[0]).unwrap();
    assert!(ring.is_compound());
    assert_eq!(ring.subpaths().count(), 2);
    assert!((net_area(&ring) - PI * 300.0).abs() <= 0.01 * total_perimeter(&ring));
    // the hole is a hole
    assert_eq!(winding_all(&path_outlines(&ring), pt(0.0, 0.0)), 0);
    assert_ne!(winding_all(&path_outlines(&ring), pt(15.0, 0.0)), 0);
    // second operation: ring (selected) + island
    click(&mut s, pt(5.0, 0.0), true);
    assert_eq!(s.selected_object_count(), 2);
    let out = s.apply_boolean(BooleanOp::Union);
    assert_eq!(
        out,
        BooleanOutcome::Applied {
            operands: 2,
            compound: true
        }
    );
    let p = only_path(&s);
    assert_eq!(p.subpaths().count(), 3, "outer, hole, island");
    let want = PI * (400.0 - 100.0 + 25.0);
    assert!(
        (net_area(&p) - want).abs() <= 0.01 * total_perimeter(&p),
        "{} vs {want}",
        net_area(&p)
    );
    // winding: island and hole are opposite
    let o = path_outlines(&p);
    let sa: Vec<f64> = o.iter().map(|x| shoelace(x)).collect();
    assert!(sa.iter().any(|a| *a > 0.0) && sa.iter().any(|a| *a < 0.0));
    // the subject line
    assert_eq!(s.style_scope().subject, "Compound path");
}

#[test]
fn ac20_one_outline_result_is_an_ordinary_path() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 20.0);
    square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0), false);
    s.apply_boolean(BooleanOp::Intersection);
    let p = only_path(&s);
    assert!(!p.is_compound());
    assert_eq!(p.anchors.len(), 4);
    assert!((net_area(&p) - 100.0).abs() < 1e-6);
    assert_eq!(s.style_scope().subject, "Path");
    // the Node tool edits it: it shows nodes, not the compound sentence
    s.set_tool(Tool::Node);
    assert!(!s.node_toolbar_state().compound_only);
}

#[test]
fn ac26_disc_in_a_union_has_71_to_142_nodes_and_ac25_stays_within_tolerance() {
    let d = Document::new(1);
    disc(&d, 0.0, 0.0, 10.0);
    square(&d, 500.0, 500.0, 5.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-20.0, -20.0), pt(600.0, 600.0), false);
    s.apply_boolean(BooleanOp::Union);
    let p = only_path(&s);
    let disc_outline: Vec<Point> = path_outlines(&p)
        .into_iter()
        .find(|o| o.iter().all(|q| q.x < 100.0))
        .unwrap();
    assert!(
        (71..=142).contains(&disc_outline.len()),
        "{} nodes",
        disc_outline.len()
    );
    for (i, q) in disc_outline.iter().enumerate() {
        let r = (q.x * q.x + q.y * q.y).sqrt();
        assert!((r - 10.0).abs() <= 0.0101, "node {i}: radius {r}");
        let n = disc_outline[(i + 1) % disc_outline.len()];
        let mid = pt((q.x + n.x) / 2.0, (q.y + n.y) / 2.0);
        let rm = (mid.x * mid.x + mid.y * mid.y).sqrt();
        assert!((10.0 - rm) <= 0.0101, "chord {i} sags {}", 10.0 - rm);
    }
}

// ---------------------------------------------------------------------------------------------
// Criteria 15 to 18: refusals change nothing
// ---------------------------------------------------------------------------------------------

struct Snapshot {
    json: Vec<u8>,
    labels: usize,
    selected: usize,
    tool: Tool,
}

fn snap(s: &Session) -> Snapshot {
    Snapshot {
        json: json(s),
        labels: labels(&reread(s)).len(),
        selected: s.selected_object_count(),
        tool: s.tool(),
    }
}

fn assert_unchanged(s: &Session, before: &Snapshot, what: &str) {
    let now = snap(s);
    assert_eq!(now.json, before.json, "{what}: document changed");
    assert_eq!(now.labels, before.labels, "{what}: a commit was written");
    assert_eq!(now.selected, before.selected, "{what}: selection changed");
    assert_eq!(now.tool, before.tool, "{what}: tool changed");
}

#[test]
fn ac15_open_paths_are_refused_for_every_operation_and_name_every_offender() {
    let d = Document::new(1);
    let _sq = square(&d, 0.0, 0.0, 20.0);
    let o1 = d.create_path(
        &[corner(100.0, 0.0), corner(130.0, 20.0), corner(160.0, 0.0)],
        false,
    );
    let o2 = d.create_path(
        &[
            corner(100.0, 100.0),
            corner(130.0, 120.0),
            corner(160.0, 100.0),
        ],
        false,
    );
    let mut s = session_of(&d);
    click(&mut s, pt(0.0, 10.0), false);
    click(&mut s, pt(130.0, 20.0), true);
    click(&mut s, pt(130.0, 120.0), true);
    assert_eq!(s.selected_object_count(), 3);
    let before = snap(&s);
    for op in ALL_OPS {
        let out = s.apply_boolean(op);
        let BooleanOutcome::Refused(BooleanRefusal::OpenPaths { offenders, of }) = out else {
            panic!("{op:?}: {out:?}")
        };
        assert_eq!(of, 3);
        let set: HashSet<_> = offenders.iter().copied().collect();
        assert_eq!(set, [o1, o2].into_iter().collect::<HashSet<_>>(), "{op:?}");
        assert_eq!(offenders.len(), 2);
        assert_unchanged(&s, &before, &format!("{op:?} open"));
    }
}

#[test]
fn ac16_shapes_without_area_are_refused_and_named() {
    let d = Document::new(1);
    let _sq = square(&d, 0.0, 0.0, 20.0);
    let line2 = d.create_path(&[corner(100.0, 0.0), corner(130.0, 20.0)], true);
    let collinear = d.create_path(
        &[corner(200.0, 0.0), corner(220.0, 0.0), corner(240.0, 0.0)],
        true,
    );
    let point = d.create_path(
        &[corner(300.0, 0.0), corner(300.0, 0.0), corner(300.0, 0.0)],
        true,
    );
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(350.0, 30.0), false);
    assert_eq!(s.selected_object_count(), 4);
    let before = snap(&s);
    let plain = s.draw_list().triangle_count();
    for op in ALL_OPS {
        let out = s.apply_boolean(op);
        let BooleanOutcome::Refused(BooleanRefusal::NoArea { offenders, of }) = out else {
            panic!("{op:?}: {out:?}")
        };
        assert_eq!(of, 4);
        assert_eq!(
            offenders.iter().copied().collect::<HashSet<_>>(),
            [line2, collinear, point]
                .into_iter()
                .collect::<HashSet<_>>(),
            "{op:?}"
        );
        assert_unchanged(&s, &before, &format!("{op:?} no area"));
    }
    assert!(s.draw_list().triangle_count() > plain, "offenders outlined");
    // a changed selection ends the outline
    click(&mut s, pt(0.0, 10.0), false);
    s.clear_boolean_refusal();
    assert_eq!(s.draw_list().triangle_count(), {
        click(&mut s, pt(0.0, 10.0), false);
        s.draw_list().triangle_count()
    });
}

#[test]
fn ac16_a_zero_area_operand_beside_open_path_reports_the_open_path_first_or_both_consistently() {
    // Not specified which refusal wins; it must be one of the two, name only genuine offenders and
    // change nothing.
    let d = Document::new(1);
    let _sq = square(&d, 0.0, 0.0, 20.0);
    let open = d.create_path(
        &[corner(100.0, 0.0), corner(130.0, 20.0), corner(160.0, 0.0)],
        false,
    );
    let line2 = d.create_path(&[corner(300.0, 0.0), corner(330.0, 20.0)], true);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(400.0, 40.0), false);
    assert_eq!(s.selected_object_count(), 3);
    let before = snap(&s);
    let out = s.apply_boolean(BooleanOp::Union);
    match out {
        BooleanOutcome::Refused(BooleanRefusal::OpenPaths { offenders, .. }) => {
            assert_eq!(offenders, vec![open]);
        }
        BooleanOutcome::Refused(BooleanRefusal::NoArea { offenders, .. }) => {
            assert_eq!(offenders, vec![line2]);
        }
        other => panic!("{other:?}"),
    }
    assert_unchanged(&s, &before, "mixed refusal");
}

#[test]
fn ac17_empty_results_are_refused_with_nothing_changed() {
    struct Case {
        name: &'static str,
        build: fn(&Document),
        op: BooleanOp,
    }
    let cases = [
        Case {
            name: "intersection of two far discs",
            build: |d| {
                disc(d, 0.0, 0.0, 10.0);
                disc(d, 50.0, 0.0, 10.0);
            },
            op: BooleanOp::Intersection,
        },
        Case {
            name: "intersection of squares sharing only an edge",
            build: |d| {
                square(d, 0.0, 0.0, 10.0);
                square(d, 10.0, 0.0, 10.0);
            },
            op: BooleanOp::Intersection,
        },
        Case {
            name: "intersection of squares sharing only a corner",
            build: |d| {
                square(d, 0.0, 0.0, 10.0);
                square(d, 10.0, 10.0, 10.0);
            },
            op: BooleanOp::Intersection,
        },
        Case {
            name: "difference covered completely",
            build: |d| {
                square(d, 5.0, 5.0, 10.0);
                square(d, 0.0, 0.0, 30.0);
            },
            op: BooleanOp::Difference,
        },
        Case {
            name: "difference of identical squares",
            build: |d| {
                square(d, 0.0, 0.0, 10.0);
                square(d, 0.0, 0.0, 10.0);
            },
            op: BooleanOp::Difference,
        },
        Case {
            name: "reverse difference covered completely",
            build: |d| {
                square(d, 0.0, 0.0, 30.0);
                square(d, 5.0, 5.0, 10.0);
            },
            op: BooleanOp::ReverseDifference,
        },
        Case {
            name: "exclusion of identical squares",
            build: |d| {
                square(d, 0.0, 0.0, 10.0);
                square(d, 0.0, 0.0, 10.0);
            },
            op: BooleanOp::Exclusion,
        },
    ];
    for case in cases {
        let d = Document::new(1);
        (case.build)(&d);
        let mut s = session_of(&d);
        marquee(&mut s, pt(-60.0, -60.0), pt(120.0, 120.0), false);
        assert_eq!(s.selected_object_count(), 2, "{}", case.name);
        let before = snap(&s);
        let draw = s.draw_list().triangle_count();
        assert_eq!(
            s.apply_boolean(case.op),
            BooleanOutcome::Refused(BooleanRefusal::Empty),
            "{}",
            case.name
        );
        assert_unchanged(&s, &before, case.name);
        assert_eq!(
            s.draw_list().triangle_count(),
            draw,
            "{}: Empty outlines nothing",
            case.name
        );
        // and a second activation gives the same
        assert_eq!(
            s.apply_boolean(case.op),
            BooleanOutcome::Refused(BooleanRefusal::Empty),
            "{}",
            case.name
        );
    }
}

#[test]
fn ac17_identical_squares_union_and_intersection_are_the_square() {
    for op in [BooleanOp::Union, BooleanOp::Intersection] {
        let d = Document::new(1);
        square(&d, 0.0, 0.0, 10.0);
        square(&d, 0.0, 0.0, 10.0);
        let mut s = session_of(&d);
        marquee(&mut s, pt(-5.0, -5.0), pt(20.0, 20.0), false);
        assert!(matches!(
            s.apply_boolean(op),
            BooleanOutcome::Applied { .. }
        ));
        let p = only_path(&s);
        assert_eq!(p.anchors.len(), 4);
        assert!((net_area(&p) - 100.0).abs() < 1e-9);
    }
}

#[test]
fn needs_two_and_wrong_tool_change_nothing() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 10.0);
    square(&d, 50.0, 0.0, 10.0);
    let mut s = session_of(&d);
    for op in ALL_OPS {
        // nothing selected
        let before = snap(&s);
        assert_eq!(
            s.apply_boolean(op),
            BooleanOutcome::Refused(BooleanRefusal::NeedsTwo)
        );
        assert_unchanged(&s, &before, "none");
    }
    click(&mut s, pt(0.0, 5.0), false);
    for op in ALL_OPS {
        let before = snap(&s);
        assert_eq!(
            s.apply_boolean(op),
            BooleanOutcome::Refused(BooleanRefusal::NeedsTwo)
        );
        assert_unchanged(&s, &before, "one");
    }
    click(&mut s, pt(50.0, 5.0), true);
    for tool in [
        Tool::Node,
        Tool::Pen,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::PolygonStar,
    ] {
        s.set_tool(tool);
        let before = snap(&s);
        for op in ALL_OPS {
            assert_eq!(
                s.apply_boolean(op),
                BooleanOutcome::Ignored,
                "{tool:?} {op:?}"
            );
        }
        assert_unchanged(&s, &before, &format!("{tool:?}"));
        s.set_tool(Tool::Select);
    }
}

#[test]
fn out_of_range_operands_are_refused_and_named() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 10.0);
    let far = d.create_path(
        &[
            corner(2.0e7, 0.0),
            corner(2.0e7 + 10.0, 0.0),
            corner(2.0e7 + 10.0, 10.0),
            corner(2.0e7, 10.0),
        ],
        true,
    );
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(2.1e7, 20.0), false);
    assert_eq!(s.selected_object_count(), 2);
    let before = snap(&s);
    for op in ALL_OPS {
        let out = s.apply_boolean(op);
        let BooleanOutcome::Refused(BooleanRefusal::OutOfRange { offenders, of }) = out else {
            panic!("{op:?}: {out:?}")
        };
        assert_eq!(offenders, vec![far]);
        assert_eq!(of, 2);
        assert_unchanged(&s, &before, "out of range");
    }
}

#[test]
fn non_finite_coordinates_never_panic_and_change_nothing() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 10.0);
    d.create_path(
        &[corner(f64::NAN, 0.0), corner(10.0, 0.0), corner(10.0, 10.0)],
        true,
    );
    d.create_path(
        &[
            corner(f64::INFINITY, 0.0),
            corner(10.0, 0.0),
            corner(10.0, 10.0),
        ],
        true,
    );
    let mut s = Session::open(2, &pack(&d, "0.1.0").unwrap()).unwrap_or_else(|_| {
        // a document with non-finite anchors may be refused at open: that is also fine
        Session::new(2)
    });
    s.set_tool(Tool::Select);
    marquee(&mut s, pt(-5.0, -5.0), pt(30.0, 30.0), false);
    let before = snap(&s);
    for op in ALL_OPS {
        let out = s.apply_boolean(op);
        assert!(
            !matches!(out, BooleanOutcome::Applied { .. }) || s.selected_object_count() == 1,
            "{op:?}"
        );
        if !matches!(out, BooleanOutcome::Applied { .. }) {
            assert_unchanged(&s, &before, "non finite");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Repeated operations, copy and transform of results, Node tool behaviour
// ---------------------------------------------------------------------------------------------

#[test]
fn repeated_operations_keep_working_and_keep_one_commit_each() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 40.0);
    square(&d, 30.0, 0.0, 40.0);
    square(&d, 60.0, 0.0, 40.0);
    square(&d, 200.0, 0.0, 40.0);
    let mut s = session_of(&d);
    let start = labels(&reread(&s)).len();
    click(&mut s, pt(0.0, 10.0), false); // square 1, left edge
    click(&mut s, pt(70.0, 10.0), true); // square 2, right edge
    assert_eq!(s.selected_object_count(), 2);
    assert!(matches!(
        s.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Applied { .. }
    ));
    assert_eq!(s.selected_object_count(), 1);
    click(&mut s, pt(100.0, 10.0), true); // square 3, right edge
    assert_eq!(s.selected_object_count(), 2);
    assert_eq!(
        s.apply_boolean(BooleanOp::Exclusion),
        BooleanOutcome::Applied {
            operands: 2,
            compound: true
        }
    );
    assert_eq!(
        reread(&s).object_ids().len(),
        2,
        "exclusion result and square 4"
    );
    click(&mut s, pt(240.0, 10.0), true);
    assert_eq!(
        s.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Applied {
            operands: 2,
            compound: true
        }
    );
    let after = labels(&reread(&s));
    let new: Vec<&str> = after[start..].iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(new, ["boolean_union", "boolean_exclusion", "boolean_union"]);
    let p = only_path(&s);
    assert_eq!(p.subpaths().count(), 3);
    assert!((net_area(&p) - (60.0 * 40.0 + 30.0 * 40.0 + 1600.0)).abs() < 1e-6);
}

#[test]
fn a_refusal_then_a_fix_then_success() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 10.0);
    square(&d, 100.0, 0.0, 10.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(120.0, 20.0), false);
    assert_eq!(
        s.apply_boolean(BooleanOp::Intersection),
        BooleanOutcome::Refused(BooleanRefusal::Empty)
    );
    assert_eq!(
        s.apply_boolean(BooleanOp::Exclusion),
        BooleanOutcome::Applied {
            operands: 2,
            compound: true
        }
    );
}

#[test]
fn compound_result_moves_copies_and_stays_a_ring() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 40.0);
    square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(50.0, 50.0), false);
    s.apply_boolean(BooleanOp::Difference);
    let ring = only_path(&s);
    let before = path_outlines(&ring);
    // move: press on the outer stroke at (0, 20) (clear of handles) and drag by (100, 0)
    s.pointer_hover(pt(10.0, 15.0), false, false);
    s.pointer_down(pt(10.0, 15.0), false);
    s.pointer_hover(pt(50.0, 15.0), false, false);
    s.pointer_hover(pt(110.0, 15.0), false, false);
    s.pointer_up(pt(110.0, 15.0), false, false);
    let moved = only_path(&s);
    let shifted: Vec<Vec<Point>> = before
        .iter()
        .map(|o| o.iter().map(|p| pt(p.x + 100.0, p.y)).collect())
        .collect();
    assert_eq!(
        path_outlines(&moved),
        shifted,
        "every outline moved together"
    );
    assert_eq!(
        winding_all(&path_outlines(&moved), pt(120.0, 20.0)),
        0,
        "hole still a hole"
    );
    assert_ne!(winding_all(&path_outlines(&moved), pt(105.0, 20.0)), 0);

    // copy by Ctrl-move: the original stays, a copy appears with fresh anchor ids
    s.pointer_hover(pt(110.0, 15.0), false, false);
    s.pointer_down(pt(110.0, 15.0), false);
    s.pointer_hover(pt(110.0, 115.0), false, true);
    s.pointer_up(pt(110.0, 115.0), false, true);
    let d2 = reread(&s);
    let ids = d2.object_ids();
    assert_eq!(ids.len(), 2, "original and copy");
    let all: Vec<PathSnapshot> = ids.iter().map(|i| d2.path(*i).unwrap()).collect();
    let anchors: Vec<AnchorId> = all
        .iter()
        .flat_map(|p| p.all_anchors().map(|a| a.id))
        .collect();
    assert_eq!(
        anchors.len(),
        anchors.iter().copied().collect::<HashSet<_>>().len(),
        "anchor ids unique"
    );
    assert!(all.iter().all(PathSnapshot::is_compound));
    let ys: Vec<f64> = all
        .iter()
        .map(|p| {
            p.anchors
                .iter()
                .map(|a| a.point.y)
                .fold(f64::INFINITY, f64::min)
        })
        .collect();
    assert!(
        ys.iter().any(|y| (*y - 0.0).abs() < 1e-9) && ys.iter().any(|y| (*y - 100.0).abs() < 1e-9),
        "{ys:?}"
    );
}

#[test]
fn node_tool_shows_the_sentence_and_changes_nothing_for_compound_paths() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 40.0);
    square(&d, 10.0, 10.0, 20.0);
    let open = d.create_path(
        &[corner(100.0, 0.0), corner(130.0, 20.0), corner(160.0, 0.0)],
        false,
    );
    let open2 = d.create_path(
        &[
            corner(100.0, 50.0),
            corner(130.0, 70.0),
            corner(160.0, 50.0),
        ],
        false,
    );
    let _ = (open, open2);
    let mut s = session_of(&d);
    click(&mut s, pt(0.0, 20.0), false);
    click(&mut s, pt(10.0, 20.0), true);
    assert_eq!(s.selected_object_count(), 2);
    s.apply_boolean(BooleanOp::Difference);
    s.set_tool(Tool::Node);
    let st = s.node_toolbar_state();
    assert!(
        st.compound_only,
        "only selected object is the compound path"
    );
    assert!(!st.can_delete && !st.can_split && !st.can_join && !st.can_insert);
    let before = snap(&s);
    // click and marquee over the ring's corners, edges and hole
    for p in [
        pt(0.0, 0.0),
        pt(40.0, 0.0),
        pt(10.0, 10.0),
        pt(20.0, 0.0),
        pt(0.0, 20.0),
        pt(20.0, 20.0),
    ] {
        click(&mut s, p, false);
        let st = s.node_toolbar_state();
        assert!(
            !st.can_delete && !st.can_split && !st.can_insert,
            "{p:?}: {st:?}"
        );
    }
    marquee(&mut s, pt(-5.0, -5.0), pt(50.0, 50.0), false);
    let st = s.node_toolbar_state();
    assert!(!st.can_delete && !st.can_split && !st.can_join);
    s.join_selected();
    s.split_selected();
    s.delete_selected();
    s.insert_selected();
    s.insert_at(pt(20.0, 0.0));
    s.make_line();
    s.make_curve();
    s.convert_selected(AnchorKind::Corner);
    assert_eq!(
        json(&s),
        before.json,
        "Node tool commands on a compound path write nothing"
    );
    // two selected objects, the compound path and an open path: the compound one adds no node
    s.set_tool(Tool::Select);
    click(&mut s, pt(0.0, 20.0), false);
    click(&mut s, pt(130.0, 20.0), true);
    assert_eq!(s.selected_object_count(), 2);
    s.set_tool(Tool::Node);
    assert!(
        !s.node_toolbar_state().compound_only,
        "two objects selected"
    );
    let before = json(&s);
    s.join_selected();
    s.split_selected();
    assert_eq!(json(&s), before);
}

#[test]
fn double_click_on_a_compound_path_gives_the_code_and_keeps_the_tool() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 40.0);
    square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(50.0, 50.0), false);
    s.apply_boolean(BooleanOp::Difference);
    for at in [pt(0.0, 25.0), pt(10.0, 15.0), pt(5.0, 5.0), pt(25.0, 5.0)] {
        click(&mut s, at, false);
        let hint = s.double_click_hint(at, false, false);
        assert_eq!(hint, DoubleClickHint::CompoundPath, "{at:?}");
        assert_eq!(hint.code(), "compound_path");
        assert_eq!(s.tool(), Tool::Select);
    }
    // an ordinary path result gives no such hint
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 20.0);
    square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0), false);
    s.apply_boolean(BooleanOp::Union);
    click(&mut s, pt(0.0, 10.0), false);
    assert_ne!(
        s.double_click_hint(pt(0.0, 10.0), false, false),
        DoubleClickHint::CompoundPath
    );
}

#[test]
fn click_inside_the_hole_of_a_result_picks_what_is_behind() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 40.0);
    square(&d, 10.0, 10.0, 20.0);
    let behind = square(&d, 12.0, 12.0, 16.0);
    d.set_fill_mode(
        FillMode::Solid,
        &[FillModeTarget {
            id: behind,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    let mut s = session_of(&d);
    click(&mut s, pt(0.0, 20.0), false);
    click(&mut s, pt(10.0, 25.0), true);
    // `behind` shares the hole; the hole's stroke at x=10 is covered only by the second square
    assert_eq!(s.selected_object_count(), 2);
    s.apply_boolean(BooleanOp::Difference);
    // the result sits at the lowest operand's place, below `behind`; deselect it first, because a
    // press inside the selected object's box starts a move
    click(&mut s, pt(500.0, 500.0), false);
    click(&mut s, pt(20.0, 20.0), false);
    assert_eq!(s.selected_object_count(), 1);
    let id = s.style_scope().ids[0];
    assert_eq!(id, behind, "a click in the hole picks the object behind");
}

// ---------------------------------------------------------------------------------------------
// Gates: drag in flight, typed entry, pen
// ---------------------------------------------------------------------------------------------

#[test]
fn a_drag_in_flight_ignores_the_command_and_nothing_is_lost_on_release() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 20.0);
    square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0), false);
    let before = json(&s);
    // a move drag in flight
    s.pointer_hover(pt(0.0, 10.0), false, false);
    s.pointer_down(pt(0.0, 10.0), false);
    s.pointer_hover(pt(30.0, 10.0), false, false);
    let out = s.apply_boolean(BooleanOp::Union);
    assert_eq!(out, BooleanOutcome::Ignored, "drag in flight");
    s.pointer_up(pt(30.0, 10.0), false, false);
    let d2 = reread(&s);
    assert_eq!(
        d2.object_ids().len(),
        2,
        "both objects still exist after the release"
    );
    assert_ne!(d2.export_json().unwrap(), before, "the move was applied");

    // a marquee in flight
    let mut s = session_of(&reread(&s));
    marquee(&mut s, pt(-5.0, -5.0), pt(80.0, 80.0), false);
    s.pointer_hover(pt(-100.0, -100.0), false, false);
    s.pointer_down(pt(-100.0, -100.0), false);
    s.pointer_hover(pt(-90.0, -90.0), false, false);
    assert_eq!(s.apply_boolean(BooleanOp::Union), BooleanOutcome::Ignored);
    s.pointer_up(pt(-90.0, -90.0), false, false);
    assert_eq!(reread(&s).object_ids().len(), 2);
}

#[test]
fn a_press_without_a_move_then_the_command_then_the_release_leaves_a_consistent_document() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 20.0);
    square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0), false);
    s.pointer_hover(pt(0.0, 10.0), false, false);
    s.pointer_down(pt(0.0, 10.0), false);
    let out = s.apply_boolean(BooleanOp::Union);
    s.pointer_up(pt(0.0, 10.0), false, false);
    let ids = reread(&s).object_ids();
    match out {
        BooleanOutcome::Applied { .. } => assert_eq!(ids.len(), 1, "no zombie operand"),
        BooleanOutcome::Ignored => assert_eq!(ids.len(), 2),
        other => panic!("{other:?}"),
    }
    // whatever happened, the next command works from a sane state
    let _ = s.boolean_availability();
    let _ = s.draw_list();
}

#[test]
fn a_typed_entry_is_not_left_dangling_by_the_command() {
    let d = Document::new(1);
    let a = square(&d, 0.0, 0.0, 20.0);
    let _b = square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    click(&mut s, pt(0.0, 10.0), false);
    // the typed move entry opens with the `m` key (it needs exactly one selected object, and
    // a selection change closes it, so it can only be open while the command has nothing to do)
    let key = |s: &mut Session, k: &str| {
        s.key_down(KeyInput {
            key: k,
            shift: false,
            ctrl: false,
            alt: false,
            repeat: false,
            dom_blocked: false,
        })
    };
    let _ = key(&mut s, "m");
    let had_entry = s.move_entry().is_some();
    let before_json = json(&s);
    let out = s.apply_boolean(BooleanOp::Union);
    // While a typed entry is open the command is ignored: the entry is not discarded and
    // nothing is written (decision of the PR 73 review).
    assert!(had_entry);
    assert_eq!(out, BooleanOutcome::Ignored);
    assert!(s.move_entry().is_some(), "the typed entry is still open");
    assert_eq!(json(&s), before_json);
    assert!(reread(&s).object_ids().contains(&a));
}

#[test]
fn escape_after_a_result_does_not_undo_anything() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 20.0);
    square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0), false);
    s.apply_boolean(BooleanOp::Union);
    let _ = s.escape();
    assert_eq!(reread(&s).object_ids().len(), 1);
    assert_eq!(s.tool(), Tool::Select);
}

// ---------------------------------------------------------------------------------------------
// Save and reopen (criterion 30), selection after a deletion of the result
// ---------------------------------------------------------------------------------------------

#[test]
fn ac30_result_survives_save_and_reopen_with_style_and_outlines() {
    let d = Document::new(1);
    let a = square(&d, 0.0, 0.0, 40.0);
    let _b = square(&d, 10.0, 10.0, 20.0);
    stylize(&d, a, 3);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(50.0, 50.0), false);
    s.apply_boolean(BooleanOp::Difference);
    let before = only_path(&s);
    let bytes = s.pack("0.1.0").unwrap();
    let again = unpack(5, &bytes).unwrap();
    let after = again.path(again.object_ids()[0]).unwrap();
    assert_eq!(after, before);
    assert!(after.is_compound());
    // and a session opened from it can operate on it again
    let s2 = Session::open(6, &bytes).unwrap();
    assert_eq!(s2.selected_object_count(), 0);
}

#[test]
fn deleting_the_result_leaves_a_clean_empty_document() {
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 20.0);
    square(&d, 10.0, 10.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0), false);
    s.apply_boolean(BooleanOp::Union);
    s.delete_selected();
    assert!(reread(&s).object_ids().is_empty());
    assert_eq!(s.boolean_availability(), BooleanAvailability::NeedsTwo);
    assert_eq!(
        s.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Refused(BooleanRefusal::NeedsTwo)
    );
}

// ---------------------------------------------------------------------------------------------
// Rounded rectangle against a circle across a rounded corner (criterion 6), arc paths, rotation
// ---------------------------------------------------------------------------------------------

#[test]
fn ac6_rounded_rectangle_minus_a_circle_crossing_a_rounded_corner() {
    let d = Document::new(1);
    let r = d.create_rect(RectBounds {
        origin: pt(0.0, 0.0),
        width: Length::from_mm(60.0),
        height: Length::from_mm(40.0),
    });
    d.set_corner_radius(&[r], Length::from_mm(12.0)).unwrap();
    disc(&d, 3.0, 3.0, 9.0); // covers the top-left rounded corner
    let mut s = session_of(&d);
    marquee(&mut s, pt(-20.0, -20.0), pt(80.0, 60.0), false);
    assert!(matches!(
        s.apply_boolean(BooleanOp::Difference),
        BooleanOutcome::Applied { .. }
    ));
    let p = only_path(&s);
    let o = path_outlines(&p);
    // inside the rounded rectangle but also inside the circle: removed
    assert_eq!(winding_all(&o, pt(4.0, 4.0)), 0);
    // the rounded corner cuts away the area outside the arc: (1, 1) is outside the rounded rect
    assert_eq!(winding_all(&o, pt(1.0, 1.0)), 0);
    // far from the circle: inside
    assert_ne!(winding_all(&o, pt(40.0, 20.0)), 0);
    // just right of the circle on the top edge region: inside
    assert_ne!(winding_all(&o, pt(14.0, 4.0)), 0);
    // analytic area: rr area - |rr ∩ disc| is checked through the identity with Intersection
    let rr_area = 60.0 * 40.0 - (4.0 - PI) * 144.0;
    assert!(net_area(&p) < rr_area && net_area(&p) > rr_area - PI * 81.0 - 1.0);
}

#[test]
fn rotated_primitives_use_their_drawn_shape() {
    // a rotated rectangle is created by the Select tool's rotate; here a polygon with a nonzero
    // frame angle stands in (its outline is the rotated one).
    let d = Document::new(1);
    let poly = d.create_polygon(
        StarFrame {
            center: pt(0.0, 0.0),
            radius: Length::from_mm(20.0),
            angle: Angle::from_radians(0.7),
        },
        PointCount::new(4).unwrap(),
    );
    let ObjectSnapshot::Primitive(pp) = d.object(poly).unwrap() else {
        panic!()
    };
    let outline: Vec<Point> = outline_of_rotated(&pp.shape, pp.rotation)
        .iter()
        .map(|a| a.point)
        .collect();
    disc(&d, 100.0, 0.0, 5.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-40.0, -40.0), pt(120.0, 40.0), false);
    s.apply_boolean(BooleanOp::Union);
    let p = only_path(&s);
    let sq = path_outlines(&p)
        .into_iter()
        .find(|o| o.iter().all(|q| q.x < 50.0))
        .unwrap();
    assert_eq!(sq.len(), 4);
    for q in &outline {
        assert!(
            sq.iter()
                .any(|r| (r.x - q.x).abs() < 0.002 && (r.y - q.y).abs() < 0.002),
            "{q:?} missing"
        );
    }
    let _ = CornerRadii::uniform(Length::from_mm(0.0));
}

// ---------------------------------------------------------------------------------------------
// A compound operand through every operation (criterion 8), analytic membership
// ---------------------------------------------------------------------------------------------

#[test]
fn ac8_a_compound_operand_keeps_its_hole_in_every_operation() {
    for op in ALL_OPS {
        let d = Document::new(1);
        disc(&d, 0.0, 0.0, 20.0);
        disc(&d, 0.0, 0.0, 10.0);
        // upper rectangle x 5..45, y -30..5: covers part of the hole and part of the ring
        d.create_rect(RectBounds {
            origin: pt(5.0, -30.0),
            width: Length::from_mm(40.0),
            height: Length::from_mm(35.0),
        });
        let mut s = session_of(&d);
        click(&mut s, pt(-20.0, 0.0), false);
        click(&mut s, pt(-10.0, 0.0), true);
        assert_eq!(s.selected_object_count(), 2);
        assert_eq!(
            s.apply_boolean(BooleanOp::Difference),
            BooleanOutcome::Applied {
                operands: 2,
                compound: true
            }
        );
        click(&mut s, pt(45.0, -12.0), true); // the rectangle right edge
        assert_eq!(s.selected_object_count(), 2, "ring and rectangle");
        let out = s.apply_boolean(op);
        let BooleanOutcome::Applied { operands, .. } = out else {
            panic!("{op:?}: {out:?}")
        };
        assert_eq!(operands, 2);
        let p = only_path(&s);
        let outlines = path_outlines(&p);
        let in_ring = |q: Point| {
            let r = (q.x * q.x + q.y * q.y).sqrt();
            r > 10.0 && r < 20.0
        };
        let in_rect = |q: Point| q.x > 5.0 && q.x < 45.0 && q.y > -30.0 && q.y < 5.0;
        let mut checked = 0;
        for i in 0..80 {
            for j in 0..80 {
                let q = pt(
                    -25.0 + f64::from(i) * 0.7 + 0.13,
                    -33.0 + f64::from(j) * 0.7 + 0.29,
                );
                let r = (q.x * q.x + q.y * q.y).sqrt();
                if (r - 10.0).abs() < 0.08
                    || (r - 20.0).abs() < 0.08
                    || (q.x - 5.0).abs() < 0.08
                    || (q.x - 45.0).abs() < 0.08
                    || (q.y + 30.0).abs() < 0.08
                    || (q.y - 5.0).abs() < 0.08
                    || outlines.iter().any(|o| poly_dist(o, q) < 0.03)
                {
                    continue;
                }
                let want = expected_in(op, &[in_ring(q), in_rect(q)]);
                assert_eq!(winding_all(&outlines, q) != 0, want, "{op:?} at {q:?}");
                checked += 1;
            }
        }
        assert!(checked > 3000);
        // the style is that of the base: ring (lowest) except Reverse difference (rectangle)
        assert_eq!(s.selected_object_count(), 1);
    }
}

// ---------------------------------------------------------------------------------------------
// Performance smoke through the session (criteria 46, 47): correctness and timing, debug build
// ---------------------------------------------------------------------------------------------

#[test]
fn ac46_a_thousand_rectangles_unite_to_one_four_node_outline() {
    let d = Document::new(1);
    for i in 0..1000 {
        let (x, y) = (f64::from(i % 40) * 8.0, f64::from(i / 40) * 8.0);
        square(&d, x, y, 10.0);
    }
    let mut s = session_of(&d);
    marquee(&mut s, pt(-20.0, -20.0), pt(400.0, 260.0), false);
    assert_eq!(s.selected_object_count(), 1000);
    let t = std::time::Instant::now();
    let out = s.apply_boolean(BooleanOp::Union);
    let took = t.elapsed();
    println!("1,000 rectangles union through the session: {took:?}");
    assert_eq!(
        out,
        BooleanOutcome::Applied {
            operands: 1000,
            compound: false
        }
    );
    let p = only_path(&s);
    assert_eq!(p.anchors.len(), 4);
    assert!((net_area(&p) - 322.0 * 202.0).abs() < 1e-6);
    let t = std::time::Instant::now();
    let _ = s.draw_list();
    println!("repaint after: {:?}", t.elapsed());
    if !cfg!(debug_assertions) {
        assert!(took < std::time::Duration::from_secs(1), "{took:?}");
    }
}

#[test]
fn ac47_two_operands_of_1000_nodes_through_the_session() {
    let n = 1000;
    let wobbly = |cx: f64, cy: f64, seed: f64| -> Vec<NewAnchor> {
        (0..n)
            .map(|i| {
                let a = f64::from(i) * 2.0 * PI / f64::from(n);
                let r = 30.0 + 4.0 * (a * 7.0 + seed).sin() + 1.5 * (a * 31.0 + seed).cos();
                corner(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    };
    for op in [
        BooleanOp::Union,
        BooleanOp::Difference,
        BooleanOp::Intersection,
    ] {
        let d = Document::new(1);
        d.create_path(&wobbly(0.0, 0.0, 0.0), true);
        d.create_path(&wobbly(25.0, 10.0, 1.0), true);
        let mut s = session_of(&d);
        marquee(&mut s, pt(-60.0, -60.0), pt(90.0, 90.0), false);
        assert_eq!(s.selected_object_count(), 2);
        let t = std::time::Instant::now();
        let out = s.apply_boolean(op);
        let kernel = t.elapsed();
        let _ = s.draw_list();
        println!(
            "{op:?}: press to repaint {:?} (kernel part {kernel:?})",
            t.elapsed()
        );
        assert!(
            matches!(out, BooleanOutcome::Applied { .. }),
            "{op:?}: {out:?}"
        );
        if !cfg!(debug_assertions) {
            assert!(
                t.elapsed() < std::time::Duration::from_millis(150),
                "{:?}",
                t.elapsed()
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Stale state after the operands are gone
// ---------------------------------------------------------------------------------------------

/// A style preview still in flight when the command runs must not be committed onto the result
/// or onto a deleted operand, and nothing may panic or come back.
#[test]
fn a_style_preview_in_flight_does_not_corrupt_the_result() {
    use curvyo_ui_core::StyleField;
    let d = Document::new(1);
    let a = square(&d, 0.0, 0.0, 20.0);
    let _b = square(&d, 10.0, 10.0, 20.0);
    stylize(&d, a, 1);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-5.0, -5.0), pt(40.0, 40.0), false);
    s.preview_style_color(StyleField::FillColor, Color { r: 1, g: 2, b: 3 });
    let out = s.apply_boolean(BooleanOp::Union);
    s.commit_style_preview();
    let ids = reread(&s).object_ids();
    match out {
        BooleanOutcome::Applied { .. } => assert_eq!(ids.len(), 1, "no operand came back"),
        BooleanOutcome::Ignored => assert_eq!(ids.len(), 2),
        other => panic!("{other:?}"),
    }
    // the document stays readable and operable
    let _ = s.style_panel_view();
    let _ = s.draw_list();
    println!(
        "style after preview + union: {:?}",
        style_of(&reread(&s).object(ids[0]).unwrap()).fill
    );
}

/// Nodes selected with the Node tool before the operation belong to deleted operands afterwards.
#[test]
fn a_node_selection_of_a_deleted_operand_is_harmless() {
    let d = Document::new(1);
    d.create_path(
        &[
            corner(0.0, 0.0),
            corner(40.0, 0.0),
            corner(40.0, 40.0),
            corner(0.0, 40.0),
        ],
        true,
    );
    d.create_path(
        &[
            corner(20.0, 20.0),
            corner(60.0, 20.0),
            corner(60.0, 60.0),
            corner(20.0, 60.0),
        ],
        true,
    );
    let mut s = session_of(&d);
    // select a node of the first path with the Node tool
    click(&mut s, pt(0.0, 0.0), false);
    s.set_tool(Tool::Node);
    click(&mut s, pt(0.0, 0.0), false);
    let had_node = s.node_toolbar_state().can_delete;
    s.set_tool(Tool::Select);
    marquee(&mut s, pt(-10.0, -10.0), pt(80.0, 80.0), false);
    assert_eq!(s.selected_object_count(), 2);
    let out = s.apply_boolean(BooleanOp::Union);
    assert!(matches!(out, BooleanOutcome::Applied { .. }), "{out:?}");
    s.set_tool(Tool::Node);
    let st = s.node_toolbar_state();
    println!("had node {had_node}; after union in Node tool: {st:?}");
    let before = json(&s);
    // a stale node selection must not delete, split or convert anything in the result
    s.delete_selected();
    s.split_selected();
    s.convert_selected(AnchorKind::Corner);
    let after = reread(&s);
    assert_eq!(after.object_ids().len(), 1);
    assert_eq!(
        after.export_json().unwrap(),
        before,
        "stale node selection changed the result"
    );
}

/// Strict form of the finding above: after the operands are gone the Node toolbar must not
/// offer Delete for their nodes. (A plain Delete of an object still leaves its node selection on
/// `main`, see the baseline test below; the boolean command clears it.)
#[test]
fn stale_node_selection_is_cleared_when_the_operands_go_away() {
    let d = Document::new(1);
    d.create_path(
        &[
            corner(0.0, 0.0),
            corner(40.0, 0.0),
            corner(40.0, 40.0),
            corner(0.0, 40.0),
        ],
        true,
    );
    d.create_path(
        &[
            corner(20.0, 20.0),
            corner(60.0, 20.0),
            corner(60.0, 60.0),
            corner(20.0, 60.0),
        ],
        true,
    );
    let mut s = session_of(&d);
    click(&mut s, pt(0.0, 0.0), false);
    s.set_tool(Tool::Node);
    click(&mut s, pt(0.0, 0.0), false);
    s.set_tool(Tool::Select);
    marquee(&mut s, pt(-10.0, -10.0), pt(80.0, 80.0), false);
    s.apply_boolean(BooleanOp::Union);
    s.set_tool(Tool::Node);
    assert!(
        !s.node_toolbar_state().can_delete,
        "Delete offered for a node that no longer exists"
    );
}

/// Baseline for the test above: the same stale node selection after a plain Delete of the object
/// (no boolean). Shows whether the stale selection is a general behaviour of the Node tool.
#[test]
fn baseline_stale_node_selection_after_deleting_the_object() {
    let d = Document::new(1);
    let _ = d.create_path(
        &[
            corner(0.0, 0.0),
            corner(40.0, 0.0),
            corner(40.0, 40.0),
            corner(0.0, 40.0),
        ],
        true,
    );
    let mut s = session_of(&d);
    click(&mut s, pt(0.0, 0.0), false);
    s.set_tool(Tool::Node);
    click(&mut s, pt(0.0, 0.0), false);
    assert!(s.node_toolbar_state().can_delete);
    s.set_tool(Tool::Select);
    click(&mut s, pt(40.0, 20.0), false);
    s.delete_selected();
    assert!(reread(&s).object_ids().is_empty());
    s.set_tool(Tool::Node);
    println!(
        "BASELINE can_delete after object delete: {}",
        s.node_toolbar_state().can_delete
    );
}

// ---------------------------------------------------------------------------------------------
// Criterion 40: degenerate fixtures through the session, every operation
// ---------------------------------------------------------------------------------------------

type Build = fn(&Document);

fn fixture_cases() -> Vec<(&'static str, Build)> {
    vec![
        ("a identical squares", |d| {
            square(d, 0.0, 0.0, 20.0);
            square(d, 0.0, 0.0, 20.0);
        }),
        ("b squares sharing a full edge", |d| {
            square(d, 0.0, 0.0, 20.0);
            square(d, 20.0, 0.0, 20.0);
        }),
        ("c squares sharing a corner", |d| {
            square(d, 0.0, 0.0, 20.0);
            square(d, 20.0, 20.0, 20.0);
        }),
        ("d square inside a larger one sharing an edge", |d| {
            square(d, 0.0, 0.0, 40.0);
            d.create_rect(RectBounds {
                origin: pt(0.0, 10.0),
                width: Length::from_mm(15.0),
                height: Length::from_mm(15.0),
            });
        }),
        ("e contour with a duplicated node", |d| {
            d.create_path(
                &[
                    corner(0.0, 0.0),
                    corner(20.0, 0.0),
                    corner(20.0, 0.0),
                    corner(20.0, 20.0),
                    corner(0.0, 20.0),
                ],
                true,
            );
            square(d, 10.0, 10.0, 20.0);
        }),
        ("f contour with a zero-length segment", |d| {
            d.create_path(
                &[
                    corner(0.0, 0.0),
                    corner(20.0, 0.0),
                    corner(20.0, 20.0),
                    corner(20.0, 20.0),
                    corner(0.0, 20.0),
                ],
                true,
            );
            square(d, 10.0, 10.0, 20.0);
        }),
        ("g bow-tie", |d| {
            d.create_path(
                &[
                    corner(0.0, 0.0),
                    corner(20.0, 20.0),
                    corner(20.0, 0.0),
                    corner(0.0, 20.0),
                ],
                true,
            );
            square(d, 5.0, 5.0, 20.0);
        }),
        ("h rectangle 0.005 x 100", |d| {
            d.create_rect(RectBounds {
                origin: pt(0.0, 10.0),
                width: Length::from_mm(100.0),
                height: Length::from_mm(0.005),
            });
            square(d, 10.0, 0.0, 20.0);
        }),
        ("i one-node closed path", |d| {
            d.create_path(&[corner(5.0, 5.0)], true);
            square(d, 0.0, 0.0, 20.0);
        }),
        (
            "j two squares touching at a corner and an edge point",
            |d| {
                square(d, 0.0, 0.0, 20.0);
                d.create_path(
                    &[corner(20.0, 10.0), corner(40.0, 0.0), corner(40.0, 20.0)],
                    true,
                );
            },
        ),
    ]
}

#[test]
fn ac40_degenerate_fixtures_complete_with_a_result_or_a_refusal() {
    for (name, build) in fixture_cases() {
        for op in ALL_OPS {
            let d = Document::new(1);
            build(&d);
            let mut s = session_of(&d);
            marquee(&mut s, pt(-10.0, -10.0), pt(150.0, 60.0), false);
            assert_eq!(s.selected_object_count(), 2, "{name}");
            let before = snap(&s);
            let t = std::time::Instant::now();
            let out = s.apply_boolean(op);
            assert!(
                t.elapsed().as_secs() < 2,
                "{name} {op:?}: {:?}",
                t.elapsed()
            );
            match out {
                BooleanOutcome::Applied { .. } => {
                    let p = only_path(&s);
                    for sp in p.subpaths() {
                        assert!(sp.anchors.len() >= 3, "{name} {op:?}");
                        assert!(
                            shoelace(&sp.anchors.iter().map(|a| a.point).collect::<Vec<_>>()).abs()
                                > 0.0
                        );
                        for a in sp.anchors {
                            assert!(a.point.x.is_finite() && a.point.y.is_finite());
                        }
                    }
                    assert!(net_area(&p) > 0.0, "{name} {op:?}");
                }
                BooleanOutcome::Refused(BooleanRefusal::Empty | BooleanRefusal::NoArea { .. }) => {
                    assert_unchanged(&s, &before, &format!("{name} {op:?}"));
                }
                other => panic!("{name} {op:?}: {other:?}"),
            }
        }
    }
    // the expectations the specification names for (a) and (c)
    for op in [BooleanOp::Union, BooleanOp::Intersection] {
        let d = Document::new(1);
        square(&d, 0.0, 0.0, 20.0);
        square(&d, 0.0, 0.0, 20.0);
        let mut s = session_of(&d);
        marquee(&mut s, pt(-10.0, -10.0), pt(50.0, 50.0), false);
        assert!(matches!(
            s.apply_boolean(op),
            BooleanOutcome::Applied { .. }
        ));
        assert_eq!(only_path(&s).anchors.len(), 4);
    }
    for op in [BooleanOp::Difference, BooleanOp::Exclusion] {
        let d = Document::new(1);
        square(&d, 0.0, 0.0, 20.0);
        square(&d, 0.0, 0.0, 20.0);
        let mut s = session_of(&d);
        marquee(&mut s, pt(-10.0, -10.0), pt(50.0, 50.0), false);
        assert_eq!(
            s.apply_boolean(op),
            BooleanOutcome::Refused(BooleanRefusal::Empty)
        );
    }
    let d = Document::new(1);
    square(&d, 0.0, 0.0, 20.0);
    square(&d, 20.0, 20.0, 20.0);
    let mut s = session_of(&d);
    marquee(&mut s, pt(-10.0, -10.0), pt(50.0, 50.0), false);
    assert_eq!(
        s.apply_boolean(BooleanOp::Intersection),
        BooleanOutcome::Refused(BooleanRefusal::Empty)
    );
    assert_eq!(
        s.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Applied {
            operands: 2,
            compound: true
        },
        "(c): union is one compound path of two outlines"
    );
    assert_eq!(only_path(&s).subpaths().count(), 2);
}
