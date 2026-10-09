//! Independent tester tests for PR 3 (panel) of
//! `specs/0015-document-size-and-rulers` through `Session`: the panel's
//! content for every tool, selection and Pen state (14a), Fit to content
//! (22 to 27a), the unit change (33, 34, 36), the view following a fit so
//! nothing moves on screen (20), the status texts (21) and the save, close
//! and open round trip (36, 39).
//!
//! `DocumentSide`, `SizeOutcome` and `FitOutcome` are not re-exported on the
//! host (`session/mod.rs` gates them on `wasm32`), so the typed Width and
//! Height path (`Session::set_document_side`, `side_text`) cannot be called
//! from an integration test. `FitOutcome` values are compared by their
//! `Debug` text.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss)]
#![allow(clippy::many_single_char_names)]

use std::ops::ControlFlow;

use curvyo_document_core::{
    AnchorId, Angle, DisplayUnit, Document, EllipseFrame, Length, NewAnchor, NodeId,
    ObjectSnapshot, Point, PointCount, RectBounds, StarFrame, ViewTransform, outline_of_rotated,
    pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::PanelContent;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn rect(d: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    })
}

fn line(d: &Document, a: (f64, f64), b: (f64, f64)) -> NodeId {
    d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(a.0, a.1)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(b.0, b.1)),
        ],
        false,
    )
}

fn rotate(d: &Document, id: NodeId, about: Point, rad: f64) {
    let o = d.object(id).unwrap();
    d.rotate_object(&o.rotated(about, Angle::from_radians(rad)))
        .unwrap();
}

fn session_of(d: &Document) -> Session {
    Session::open(2, &pack(d, "0.1.0").unwrap()).unwrap()
}

fn reopened(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

/// A mix of kinds, rotated and not, some off the page.
fn populated() -> Document {
    let d = Document::new(1);
    let r = rect(&d, 10.0, 10.0, 50.0, 50.0);
    let _far = rect(&d, -80.0, 500.0, 20.0, 5.0);
    let e = d.create_ellipse(EllipseFrame {
        center: pt(100.0, 120.0),
        rx: mm(30.0),
        ry: mm(12.0),
    });
    let _pg = d.create_polygon(
        StarFrame {
            center: pt(150.0, 40.0),
            radius: mm(25.0),
            angle: Angle::from_radians(0.3),
        },
        PointCount::new(5).unwrap(),
    );
    let _l = line(&d, (5.0, 5.0), (190.0, 77.0));
    rotate(&d, r, pt(20.0, 20.0), 0.7);
    rotate(&d, e, pt(100.0, 120.0), 1.1);
    d
}

/// All outline points of all objects, in document millimetres.
fn points(d: &Document) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for id in d.object_ids() {
        match d.object(id).unwrap() {
            ObjectSnapshot::Path(p) => {
                out.extend(p.anchors.iter().map(|a| (a.point.x, a.point.y)));
            }
            ObjectSnapshot::Primitive(p) => {
                out.extend(
                    outline_of_rotated(&p.shape, p.rotation)
                        .iter()
                        .map(|a| (a.point.x, a.point.y)),
                );
            }
        }
    }
    out
}

fn on_screen(view: ViewTransform, pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    pts.iter()
        .map(|&(x, y)| view.document_to_screen(pt(x, y)))
        .collect()
}

fn assert_same_screen(a: &[(f64, f64)], b: &[(f64, f64)], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}");
    for (i, (p, q)) in a.iter().zip(b).enumerate() {
        assert!(
            (p.0 - q.0).abs() <= 0.5 && (p.1 - q.1).abs() <= 0.5,
            "{what}: point {i} moved on screen {p:?} -> {q:?}"
        );
        // The spec allows 0.5 px; the arithmetic is exact to far less.
        assert!(
            (p.0 - q.0).abs() <= 1e-6 && (p.1 - q.1).abs() <= 1e-6,
            "{what}: point {i} drifted {p:?} -> {q:?}"
        );
    }
}

fn bbox(pts: &[(f64, f64)]) -> ((f64, f64), (f64, f64)) {
    let mut lo = (f64::INFINITY, f64::INFINITY);
    let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &(x, y) in pts {
        lo = (lo.0.min(x), lo.1.min(y));
        hi = (hi.0.max(x), hi.1.max(y));
    }
    (lo, hi)
}

fn loro_of(d: &Document) -> loro::LoroDoc {
    let l = loro::LoroDoc::new();
    l.import(&d.export_loro_snapshot().unwrap()).unwrap();
    l
}

fn change_count(s: &Session) -> usize {
    loro_of(&reopened(s)).len_changes()
}

/// Commit labels, oldest first.
fn labels(s: &Session) -> Vec<String> {
    let l = loro_of(&reopened(s));
    let ids: Vec<loro::ID> = l.oplog_frontiers().iter().collect();
    let mut out: Vec<(u32, String)> = Vec::new();
    l.travel_change_ancestors(&ids, &mut |m| {
        out.push((
            m.lamport,
            m.message.map(|x| x.to_string()).unwrap_or_default(),
        ));
        ControlFlow::Continue(())
    })
    .unwrap();
    out.sort();
    out.into_iter().map(|(_, m)| m).collect()
}

/// The stored bytes of the document, for "nothing was written".
fn snapshot(s: &Session) -> Vec<u8> {
    reopened(s).export_loro_snapshot().unwrap()
}

fn size_mm(s: &Session) -> (f64, f64) {
    let size = reopened(s).size();
    (size.width.as_mm(), size.height.as_mm())
}

fn fit(s: &mut Session) -> String {
    format!("{:?}", s.fit_document())
}

fn click(s: &mut Session, x: f64, y: f64) {
    s.pointer_hover(pt(x, y), false, false);
    s.pointer_down(pt(x, y), false);
    s.pointer_up(pt(x, y), false, false);
}

const ALL_TOOLS: [Tool; 6] = [
    Tool::Select,
    Tool::Pen,
    Tool::Node,
    Tool::Rectangle,
    Tool::Ellipse,
    Tool::PolygonStar,
];

// ---- criterion 14a: panel content ----

#[test]
fn ac14a_nothing_selected_shows_the_document_section_for_every_tool() {
    for tool in ALL_TOOLS {
        let mut s = session_of(&populated());
        s.set_tool(tool);
        assert_eq!(s.selected_object_count(), 0);
        assert_eq!(s.panel_content(), PanelContent::Document, "{tool:?}");
    }
    assert_eq!(Session::new(1).panel_content(), PanelContent::Document);
}

#[test]
fn ac14a_a_selection_shows_style_and_escape_reveals_the_document_section() {
    let d = Document::new(1);
    rect(&d, 10.0, 10.0, 50.0, 50.0);
    let mut s = session_of(&d);
    s.set_tool(Tool::Select);
    click(&mut s, 10.0, 30.0);
    assert_eq!(s.selected_object_count(), 1);
    assert_eq!(s.panel_content(), PanelContent::Style);
    // Switching to a creation tool keeps the selection and the Style area.
    for tool in [
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::PolygonStar,
        Tool::Node,
    ] {
        s.set_tool(tool);
        let want = if s.selected_object_count() > 0 {
            PanelContent::Style
        } else {
            PanelContent::Document
        };
        assert_eq!(s.panel_content(), want, "{tool:?}");
        s.set_tool(Tool::Select);
        click(&mut s, 10.0, 30.0);
    }
    s.set_tool(Tool::Select);
    s.escape();
    assert_eq!(s.selected_object_count(), 0);
    assert_eq!(s.panel_content(), PanelContent::Document);
    // Clicking the empty pasteboard clears a selection the same way.
    click(&mut s, 10.0, 30.0);
    assert_eq!(s.panel_content(), PanelContent::Style);
    click(&mut s, -300.0, -300.0);
    assert_eq!(s.panel_content(), PanelContent::Document);
}

#[test]
fn ac14a_an_unfinished_pen_path_empties_the_panel_until_it_is_gone() {
    let mut s = session_of(&populated());
    s.set_tool(Tool::Pen);
    assert_eq!(s.panel_content(), PanelContent::Document);
    click(&mut s, 300.0, 300.0);
    assert!(s.pen_in_progress().is_some());
    assert_eq!(s.panel_content(), PanelContent::Empty);
    click(&mut s, 330.0, 340.0);
    assert_eq!(s.panel_content(), PanelContent::Empty);
    s.escape();
    assert!(s.pen_in_progress().is_none());
    assert_eq!(s.panel_content(), PanelContent::Document);
}

/// Switching tools while the Pen path is unfinished must not leave a path
/// that a resize would then move the objects around. Observed: what the Pen
/// does with its path on a tool switch, and that the panel and the Pen agree.
#[test]
fn ac14a_leaving_the_pen_with_an_unfinished_path_leaves_no_hidden_path() {
    let mut bad = Vec::new();
    for other in [Tool::Select, Tool::Node, Tool::Rectangle] {
        let mut s = session_of(&populated());
        s.set_tool(Tool::Pen);
        click(&mut s, 300.0, 300.0);
        click(&mut s, 330.0, 340.0);
        assert_eq!(s.panel_content(), PanelContent::Empty);
        s.set_tool(other);
        let shown = s.panel_content();
        s.set_tool(Tool::Pen);
        if shown == PanelContent::Document && s.pen_in_progress().is_some() {
            bad.push(other);
        }
    }
    assert!(
        bad.is_empty(),
        "with {bad:?} the panel offers the Document section (a resize) while the Pen \
         still holds an unfinished path that returns when the Pen is picked again"
    );
}

// ---- criterion 22, 26, 27a, 23-25, 27: Fit to content ----

#[test]
fn ac22_fit_on_an_empty_document_does_nothing() {
    let mut s = Session::new(1);
    assert!(!s.has_objects());
    let before = snapshot(&s);
    let n = change_count(&s);
    assert_eq!(fit(&mut s), "Empty");
    assert_eq!(snapshot(&s), before);
    assert_eq!(change_count(&s), n);
    assert_eq!(s.size_text(), "210.0 \u{d7} 297.0 mm");
}

#[test]
fn ac22_the_button_follows_the_object_count() {
    let d = Document::new(1);
    let id = rect(&d, 1.0, 1.0, 5.0, 5.0);
    let _ = id;
    let mut s = session_of(&d);
    assert!(s.has_objects());
    s.set_tool(Tool::Select);
    click(&mut s, 1.0, 3.0);
    assert_eq!(s.selected_object_count(), 1);
    s.delete_selected();
    assert!(!s.has_objects());
    assert_eq!(
        fit(&mut s),
        "Empty",
        "Empty, never AlreadyFits, with no object"
    );
}

#[test]
fn ac23_24_fit_sizes_the_document_to_the_outline_box_and_moves_it_to_0_0() {
    // Straight outlines only (rotated rectangles, a polygon, a line), so the
    // expected box is computed from corners here.
    let d = Document::new(1);
    let r = rect(&d, 10.0, 10.0, 50.0, 30.0);
    rotate(&d, r, pt(20.0, 20.0), 0.7);
    rect(&d, -80.0, 500.0, 20.0, 5.0);
    let _ = d.create_polygon(
        StarFrame {
            center: pt(150.0, 40.0),
            radius: mm(25.0),
            angle: Angle::from_radians(0.3),
        },
        PointCount::new(5).unwrap(),
    );
    line(&d, (5.0, 5.0), (190.0, 77.0));
    let before = points(&d);
    let (lo, hi) = bbox(&before);
    let mut s = session_of(&d);
    assert_eq!(fit(&mut s), "Fitted");
    let after = points(&reopened(&s));
    let (alo, ahi) = bbox(&after);
    assert!(alo.0.abs() < 1e-9 && alo.1.abs() < 1e-9, "top-left {alo:?}");
    let (w, h) = size_mm(&s);
    assert!((w - (hi.0 - lo.0)).abs() < 1e-9, "{w} vs {}", hi.0 - lo.0);
    assert!((h - (hi.1 - lo.1)).abs() < 1e-9, "{h} vs {}", hi.1 - lo.1);
    assert!((ahi.0 - w).abs() < 1e-9 && (ahi.1 - h).abs() < 1e-9);
    for (p, q) in before.iter().zip(&after) {
        assert!((p.0 - lo.0 - q.0).abs() < 1e-9 && (p.1 - lo.1 - q.1).abs() < 1e-9);
    }
}

#[test]
fn ac23_fit_uses_true_curve_extremes_for_an_ellipse() {
    // Unrotated: the box is the radii exactly. Rotated by t: half extents
    // sqrt(rx^2 cos^2 + ry^2 sin^2) and sqrt(rx^2 sin^2 + ry^2 cos^2), with
    // the four-arc approximation's error well below 0.02 mm at these radii.
    for (rot, tol) in [
        (0.0, 1e-9),
        (0.6, 0.02),
        (std::f64::consts::FRAC_PI_2, 1e-6),
    ] {
        let d = Document::new(1);
        let e = d.create_ellipse(EllipseFrame {
            center: pt(100.0, 120.0),
            rx: mm(30.0),
            ry: mm(12.0),
        });
        if rot != 0.0 {
            rotate(&d, e, pt(100.0, 120.0), rot);
        }
        let mut s = session_of(&d);
        assert_eq!(fit(&mut s), "Fitted");
        let (w, h) = size_mm(&s);
        let (c, sn): (f64, f64) = (f64::cos(rot), f64::sin(rot));
        let want_w = 2.0 * (30.0f64.powi(2) * c * c + 12.0f64.powi(2) * sn * sn).sqrt();
        let want_h = 2.0 * (30.0f64.powi(2) * sn * sn + 12.0f64.powi(2) * c * c).sqrt();
        assert!((w - want_w).abs() < tol, "rot {rot}: w {w} vs {want_w}");
        assert!((h - want_h).abs() < tol, "rot {rot}: h {h} vs {want_h}");
    }
}

#[test]
fn ac24_fit_with_everything_far_away_in_both_directions() {
    for (x, y) in [(-5000.0, -5000.0), (80_000.0, 90_000.0), (-1.0e4, 5.0e4)] {
        let d = Document::new(1);
        rect(&d, x, y, 40.0, 20.0);
        let mut s = session_of(&d);
        assert_eq!(fit(&mut s), "Fitted");
        assert_eq!(size_mm(&s), (40.0, 20.0));
        let (lo, _) = bbox(&points(&reopened(&s)));
        assert!(lo.0.abs() < 1e-9 && lo.1.abs() < 1e-9, "{lo:?}");
    }
}

#[test]
fn ac25_a_flat_line_fits_to_one_mm_on_that_axis_and_is_centred() {
    let d = Document::new(1);
    line(&d, (0.0, 0.0), (100.0, 0.0));
    let mut s = session_of(&d);
    assert_eq!(fit(&mut s), "Fitted");
    assert_eq!(size_mm(&s), (100.0, 1.0));
    assert_eq!(s.size_text(), "100.0 \u{d7} 1.0 mm");
    let pts = points(&reopened(&s));
    assert!(
        (pts[0].1 - 0.5).abs() < 1e-9 && (pts[1].1 - 0.5).abs() < 1e-9,
        "{pts:?}"
    );
    assert!(pts[0].0.abs() < 1e-9 && (pts[1].0 - 100.0).abs() < 1e-9);
    // And a vertical one, and a single point.
    let d = Document::new(1);
    line(&d, (30.0, 40.0), (30.0, 90.0));
    let mut s = session_of(&d);
    assert_eq!(fit(&mut s), "Fitted");
    assert_eq!(size_mm(&s), (1.0, 50.0));
    let pts = points(&reopened(&s));
    assert!(
        (pts[0].0 - 0.5).abs() < 1e-9 && pts[0].1.abs() < 1e-9,
        "{pts:?}"
    );
    let d = Document::new(1);
    line(&d, (30.0, 40.0), (30.0, 40.0));
    let mut s = session_of(&d);
    assert_eq!(fit(&mut s), "Fitted");
    assert_eq!(size_mm(&s), (1.0, 1.0));
    let pts = points(&reopened(&s));
    assert!(
        (pts[0].0 - 0.5).abs() < 1e-9 && (pts[0].1 - 0.5).abs() < 1e-9,
        "{pts:?}"
    );
}

#[test]
fn ac26_a_second_fit_is_idempotent_and_writes_nothing() {
    for d in [populated(), {
        let d = Document::new(1);
        let r = rect(&d, 0.1, 0.2, 33.3, 17.7);
        rotate(&d, r, pt(10.0, 10.0), 0.123_456_789);
        d
    }] {
        let mut s = session_of(&d);
        assert_eq!(fit(&mut s), "Fitted");
        let stored = snapshot(&s);
        let n = change_count(&s);
        let size = size_mm(&s);
        let pts = points(&reopened(&s));
        for round in 0..3 {
            assert_eq!(fit(&mut s), "AlreadyFits", "round {round}");
            assert_eq!(snapshot(&s), stored, "round {round} wrote");
            assert_eq!(change_count(&s), n);
            assert_eq!(size_mm(&s), size);
            assert_eq!(points(&reopened(&s)), pts);
        }
    }
}

#[test]
fn ac26_fit_after_a_unit_change_is_still_idempotent() {
    let mut s = session_of(&populated());
    assert_eq!(fit(&mut s), "Fitted");
    assert!(s.set_display_unit(DisplayUnit::In));
    assert_eq!(fit(&mut s), "AlreadyFits");
}

#[test]
fn ac27_fit_is_one_commit_labelled_fit_document_to_content() {
    let mut s = session_of(&populated());
    let n = change_count(&s);
    assert_eq!(fit(&mut s), "Fitted");
    assert_eq!(change_count(&s), n + 1);
    assert_eq!(labels(&s).last().unwrap(), "fit_document_to_content");
}

#[test]
fn ac27a_content_larger_than_the_limit_is_refused_with_the_message() {
    for (unit, needle) in [
        (DisplayUnit::Mm, "100000 mm"),
        (DisplayUnit::Cm, "10000 cm"),
        (DisplayUnit::In, "3937 in"),
    ] {
        let d = Document::new(1);
        rect(&d, -60_000.0, 5.0, 100_001.0, 10.0);
        rect(&d, 5.0, 5.0, 10.0, 10.0);
        let mut s = session_of(&d);
        s.set_display_unit(unit);
        let before = snapshot(&s);
        let n = change_count(&s);
        let size = size_mm(&s);
        let out = fit(&mut s);
        assert!(out.starts_with("TooLarge("), "{out}");
        assert!(out.contains(needle), "{unit:?}: {out}");
        assert!(out.contains("Nothing was changed"), "{out}");
        assert_eq!(snapshot(&s), before, "{unit:?} wrote");
        assert_eq!(change_count(&s), n);
        assert_eq!(size_mm(&s), size);
    }
}

#[test]
fn ac27a_too_large_on_the_height_axis_and_exactly_at_the_limit() {
    let d = Document::new(1);
    rect(&d, 0.0, -10.0, 10.0, 100_000.5);
    let mut s = session_of(&d);
    assert!(fit(&mut s).starts_with("TooLarge("));
    let d = Document::new(1);
    rect(&d, 7.0, -10.0, 10.0, 100_000.0);
    let mut s = session_of(&d);
    assert_eq!(fit(&mut s), "Fitted");
    assert_eq!(size_mm(&s), (10.0, 100_000.0));
    assert_eq!(s.size_text(), "10.0 \u{d7} 100000.0 mm");
}

// ---- criterion 20: the view follows a fit ----

#[test]
fn ac20_a_fit_moves_no_object_on_screen_at_any_zoom_or_pan() {
    type Prepare = fn(&mut Session);
    let prepare: [(&str, Prepare); 6] = [
        ("default view (72 px inset)", |s| {
            s.show_default_view();
            s.resize_viewport(1000.0, 700.0);
        }),
        ("no default view call", |s| s.resize_viewport(900.0, 600.0)),
        ("zoomed in", |s| {
            s.show_default_view();
            s.resize_viewport(1000.0, 700.0);
            s.wheel(0.0, -240.0, 300.0, 200.0, false, true);
        }),
        ("zoomed out", |s| {
            s.show_default_view();
            s.resize_viewport(1000.0, 700.0);
            for _ in 0..6 {
                s.wheel(0.0, 200.0, 500.0, 350.0, false, true);
            }
        }),
        ("panned", |s| {
            s.show_default_view();
            s.resize_viewport(1000.0, 700.0);
            s.wheel(120.0, -75.0, 0.0, 0.0, false, false);
        }),
        ("panned then window resized", |s| {
            s.show_default_view();
            s.resize_viewport(1000.0, 700.0);
            s.wheel(-60.0, 33.0, 0.0, 0.0, false, false);
            s.resize_viewport(640.0, 480.0);
        }),
    ];
    for (name, prep) in prepare {
        let mut s = session_of(&populated());
        prep(&mut s);
        let before = on_screen(s.view(), &points(&reopened(&s)));
        assert_eq!(fit(&mut s), "Fitted");
        let after = on_screen(s.view(), &points(&reopened(&s)));
        assert_same_screen(&before, &after, name);
        // A window resize of a view that only followed a fit keeps them put
        // when the view was still untouched.
        s.resize_viewport(1200.0, 800.0);
        let _ = on_screen(s.view(), &points(&reopened(&s)));
    }
}

#[test]
fn ac20_a_window_resize_after_a_fit_in_the_untouched_view_keeps_objects_in_place() {
    let mut s = session_of(&populated());
    s.show_default_view();
    s.resize_viewport(1000.0, 700.0);
    let before = on_screen(s.view(), &points(&reopened(&s)));
    assert_eq!(fit(&mut s), "Fitted");
    s.resize_viewport(700.0, 500.0);
    s.resize_viewport(1300.0, 900.0);
    let after = on_screen(s.view(), &points(&reopened(&s)));
    assert_same_screen(&before, &after, "window resize after fit");
}

#[test]
fn ac20_a_fit_that_changes_nothing_does_not_move_the_view() {
    let mut s = session_of(&populated());
    s.show_default_view();
    s.resize_viewport(1000.0, 700.0);
    assert_eq!(fit(&mut s), "Fitted");
    let v = s.view();
    assert_eq!(fit(&mut s), "AlreadyFits");
    assert_eq!(s.view(), v);
    let mut empty = Session::new(1);
    empty.show_default_view();
    empty.resize_viewport(800.0, 600.0);
    let v = empty.view();
    assert_eq!(fit(&mut empty), "Empty");
    assert_eq!(empty.view(), v);
}

#[test]
fn ac20_a_refused_fit_does_not_move_the_view() {
    let d = Document::new(1);
    rect(&d, 0.0, 0.0, 100_001.0, 10.0);
    let mut s = session_of(&d);
    s.show_default_view();
    s.resize_viewport(1000.0, 700.0);
    let v = s.view();
    assert!(fit(&mut s).starts_with("TooLarge("));
    assert_eq!(s.view(), v);
}

// ---- criteria 21, 34, 36, 38: unit ----

#[test]
fn ac34_a_unit_change_changes_display_only_and_is_one_commit() {
    let mut s = session_of(&populated());
    s.show_default_view();
    s.resize_viewport(1000.0, 700.0);
    let pts = points(&reopened(&s));
    let size = size_mm(&s);
    let view = s.view();
    let n = change_count(&s);
    assert!(s.set_display_unit(DisplayUnit::In));
    assert_eq!(s.display_unit(), DisplayUnit::In);
    assert_eq!(change_count(&s), n + 1);
    assert_eq!(labels(&s).last().unwrap(), "set_display_unit");
    assert_eq!(points(&reopened(&s)), pts, "an object moved");
    assert_eq!(size_mm(&s), size, "the stored size changed");
    assert_eq!(s.view(), view, "the view moved");
    assert_eq!(s.size_text(), "8.268 \u{d7} 11.693 in");
    assert_eq!(s.cursor_text(25.4, 50.8), "x: 1.000  y: 2.000 in");
    assert!(s.set_display_unit(DisplayUnit::Cm));
    assert_eq!(s.size_text(), "21.00 \u{d7} 29.70 cm");
    assert_eq!(s.cursor_text(12.3, 45.6), "x: 1.23  y: 4.56 cm");
    assert!(s.set_display_unit(DisplayUnit::Mm));
    assert_eq!(s.size_text(), "210.0 \u{d7} 297.0 mm");
    assert_eq!(size_mm(&s), size);
}

#[test]
fn ac34_the_same_unit_again_writes_nothing() {
    let mut s = Session::new(1);
    let before = snapshot(&s);
    let n = change_count(&s);
    assert!(!s.set_display_unit(DisplayUnit::Mm), "mm is the default");
    assert_eq!(snapshot(&s), before);
    assert_eq!(change_count(&s), n);
    assert!(s.set_display_unit(DisplayUnit::In));
    let after = snapshot(&s);
    let n = change_count(&s);
    for _ in 0..3 {
        assert!(!s.set_display_unit(DisplayUnit::In));
    }
    assert_eq!(snapshot(&s), after);
    assert_eq!(change_count(&s), n);
}

#[test]
fn ac36_the_unit_is_saved_with_the_document() {
    for unit in [DisplayUnit::Cm, DisplayUnit::In, DisplayUnit::Mm] {
        let mut s = session_of(&populated());
        s.set_display_unit(DisplayUnit::In);
        s.set_display_unit(unit);
        let reopened_session = Session::open(5, &s.pack("0.1.0").unwrap()).unwrap();
        assert_eq!(reopened_session.display_unit(), unit);
        assert_eq!(reopened_session.size_text(), s.size_text());
    }
    // A new project and a file saved before the setting open in mm.
    assert_eq!(Session::new(1).display_unit(), DisplayUnit::Mm);
}

#[test]
fn ac36_a_new_project_after_inches_is_mm_again() {
    let mut s = Session::new(1);
    s.set_display_unit(DisplayUnit::In);
    let fresh = Session::new(2);
    assert_eq!(fresh.display_unit(), DisplayUnit::Mm);
}

// ---- criterion 39: save, close, open ----

#[test]
fn ac39_a_fit_survives_save_close_and_open() {
    let mut s = session_of(&populated());
    s.set_display_unit(DisplayUnit::Cm);
    assert_eq!(fit(&mut s), "Fitted");
    let size = size_mm(&s);
    let pts = points(&reopened(&s));
    let bytes = s.pack("0.1.0").unwrap();
    drop(s);
    let again = Session::open(3, &bytes).unwrap();
    assert_eq!(size_mm(&again), size);
    assert_eq!(points(&reopened(&again)), pts);
    assert_eq!(again.display_unit(), DisplayUnit::Cm);
    let mut again = again;
    assert_eq!(fit(&mut again), "AlreadyFits");
}

#[test]
fn ac12_21_the_size_readout_follows_a_fit_at_once() {
    let d = Document::new(1);
    rect(&d, 100.0, 100.0, 80.0, 40.0);
    let mut s = session_of(&d);
    assert_eq!(s.size_text(), "210.0 \u{d7} 297.0 mm");
    assert_eq!(fit(&mut s), "Fitted");
    assert_eq!(s.size_text(), "80.0 \u{d7} 40.0 mm");
    s.set_display_unit(DisplayUnit::In);
    assert_eq!(s.size_text(), "3.150 \u{d7} 1.575 in");
}

// ---- Fit with every tool state: gestures in flight are not disturbed ----

#[test]
fn fit_after_creating_an_object_with_a_tool_includes_it() {
    let mut s = Session::new(1);
    s.set_tool(Tool::Rectangle);
    s.pointer_hover(pt(-100.0, -50.0), false, false);
    s.pointer_down(pt(-100.0, -50.0), false);
    s.pointer_hover(pt(400.0, 600.0), false, false);
    s.pointer_up(pt(400.0, 600.0), false, false);
    assert!(s.has_objects());
    assert_eq!(fit(&mut s), "Fitted");
    assert_eq!(size_mm(&s), (500.0, 650.0));
}
