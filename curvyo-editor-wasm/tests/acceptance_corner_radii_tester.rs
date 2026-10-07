//! Independent black-box tests for `specs/rectangle-corner-radii/` PART 1 at
//! the `Session` level (criteria 12 to 15 for the model-visible parts, 16 to 22
//! data layer): resize with "Scale corner radius" off and on, rotation and
//! flips, op-log writes, legacy files untouched by open/select/hover, the Select
//! bar's data for unequal corners, "Remove rounding", "Object to path".
//! Written from the specification and `adrs.md` before reading the implementation.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss,
    clippy::needless_range_loop,
    clippy::type_complexity,
    clippy::doc_markdown,
    missing_docs
)]

use std::io::Read;

use curvyo_document_core::{
    AnchorKind, Angle, CornerRadii, Document, Length, NodeId, ObjectSnapshot, Point, RectBounds,
    Shape, outline_of_rotated, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::BarValue;

const ROT: f64 = 0.5;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn mm(v: f64) -> Length {
    Length::from_mm(v)
}

fn rb(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
    RectBounds {
        origin: pt(x, y),
        width: mm(w),
        height: mm(h),
    }
}

fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
    CornerRadii {
        tl: mm(tl),
        tr: mm(tr),
        br: mm(br),
        bl: mm(bl),
    }
}

fn arr(r: CornerRadii) -> [f64; 4] {
    [r.tl.as_mm(), r.tr.as_mm(), r.br.as_mm(), r.bl.as_mm()]
}

fn rot(p: Point, c: Point, a: f64) -> Point {
    let (s, co) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

/// A 40 x 20 rectangle at the origin with `r`, optionally rotated by `ROT`
/// about its centre.
fn build(r: [f64; 4], rotated: bool) -> (Document, NodeId) {
    let d = Document::new(1);
    let id = d.create_rect(rb(0.0, 0.0, 40.0, 20.0));
    d.set_corner_radii(&[(id, radii(r[0], r[1], r[2], r[3]))])
        .unwrap();
    if rotated {
        let o = d
            .object(id)
            .unwrap()
            .rotated(pt(20.0, 10.0), Angle::from_radians(ROT));
        d.rotate_object(&o).unwrap();
    }
    (d, id)
}

fn open_bytes(bytes: &[u8]) -> Session {
    let mut s = Session::open(2, bytes).expect("opens");
    s.set_tool(Tool::Select);
    s
}

fn open(d: &Document) -> Session {
    open_bytes(&pack(d, "0.1.0").unwrap())
}

fn doc_of(s: &Session) -> Document {
    unpack(99, &s.pack("0.1.0").unwrap()).unwrap()
}

fn rect_of(s: &Session) -> (RectBounds, CornerRadii, f64) {
    let d = doc_of(s);
    let id = d.object_ids()[0];
    let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
        panic!()
    };
    let Shape::Rect {
        bounds,
        corner_radii,
    } = p.shape
    else {
        panic!()
    };
    (bounds, corner_radii, p.rotation.as_radians())
}

fn click(s: &mut Session, p: Point) {
    s.pointer_hover(p, false, false);
    s.pointer_down(p, false);
    s.pointer_up(p, false, false);
}

fn drag(s: &mut Session, from: Point, to: Point, shift: bool, ctrl: bool) {
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(to, shift, ctrl);
    s.pointer_up(to, shift, ctrl);
}

fn selected(r: [f64; 4], rotated: bool) -> Session {
    let (d, _) = build(r, rotated);
    let mut s = open(&d);
    let p = if rotated {
        rot(pt(20.0, 0.0), pt(20.0, 10.0), ROT)
    } else {
        pt(20.0, 0.0)
    };
    click(&mut s, p);
    s
}

/// Local-frame handle positions with the opposite anchor and which axes act.
fn handles() -> Vec<(&'static str, Point, Point, bool, bool)> {
    let (x0, y0, x1, y1) = (0.0, 0.0, 40.0, 20.0);
    let (xm, ym) = (20.0, 10.0);
    vec![
        ("se", pt(x1, y1), pt(x0, y0), true, true),
        ("nw", pt(x0, y0), pt(x1, y1), true, true),
        ("ne", pt(x1, y0), pt(x0, y1), true, true),
        ("sw", pt(x0, y1), pt(x1, y0), true, true),
        ("n", pt(xm, y0), pt(xm, y1), false, true),
        ("s", pt(xm, y1), pt(xm, y0), false, true),
        ("w", pt(x0, ym), pt(x1, ym), true, false),
        ("e", pt(x1, ym), pt(x0, ym), true, false),
    ]
}

fn gesture(
    h: (&str, Point, Point, bool, bool),
    f: (f64, f64),
    shift: bool,
    rotated: bool,
) -> (Point, Point) {
    let c = pt(20.0, 10.0);
    let (_, handle, anchor, hx, vy) = h;
    let o = if shift { c } else { anchor };
    let to = pt(
        if hx {
            o.x + (handle.x - o.x) * f.0
        } else {
            handle.x
        },
        if vy {
            o.y + (handle.y - o.y) * f.1
        } else {
            handle.y
        },
    );
    let a = if rotated { ROT } else { 0.0 };
    (rot(handle, c, a), rot(to, c, a))
}

// ---- loro helpers ----------------------------------------------------

fn loro_of(s: &Session) -> loro::LoroDoc {
    let l = loro::LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l
}

fn vv(s: &Session) -> loro::VersionVector {
    loro_of(s).oplog_vv()
}

fn ops_since(s: &Session, from: &loro::VersionVector) -> String {
    let l = loro_of(s);
    format!("{:?}", l.export_json_updates(from, &l.oplog_vv()))
}

fn corner_writes(ops: &str) -> [usize; 4] {
    ["tl", "tr", "br", "bl"].map(|k| ops.matches(&format!("\"corner_radius_{k}\"")).count())
}

fn member(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut f = z.by_name(name).unwrap();
    let mut v = Vec::new();
    f.read_to_end(&mut v).unwrap();
    v
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../curvyo-document-core/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-7
}

// ---------------------------------------------------------------------
// Criteria 12-15 (the model-visible part): Keep and Proportional
// ---------------------------------------------------------------------

const STORED: [f64; 4] = [2.0, 4.0, 6.0, 8.0];

const GESTURES: [((f64, f64), bool, bool); 12] = [
    ((1.5, 2.5), false, false),
    ((0.4, 0.6), false, false),
    ((1.5, 2.5), true, false),
    ((2.0, 2.0), false, true),
    ((3.0, 3.0), true, true),
    ((2.0, 1.5), false, false),
    ((1.2, 0.5), false, false),
    ((0.1, 0.1), false, false),
    // Flips: dragged past the opposite edge.
    ((-0.5, 1.5), false, false),
    ((1.5, -0.5), false, false),
    ((-1.0, -1.0), false, false),
    ((-2.0, -2.0), true, false),
];

#[test]
fn ac15_default_is_off_and_keep_never_writes_a_radius_register() {
    for rotated in [false, true] {
        for (hi, h) in handles().into_iter().enumerate() {
            for (gi, (f, shift, ctrl)) in GESTURES.into_iter().enumerate() {
                let mut s = selected(STORED, rotated);
                assert!(!s.scale_corner_radius(), "default off");
                let before = vv(&s);
                let (b0, _, _) = rect_of(&s);
                let (press, release) = gesture(h, f, shift, rotated);
                drag(&mut s, press, release, shift, ctrl);
                let (b1, r1, _) = rect_of(&s);
                let ctx = format!("rot {rotated} handle {hi} gesture {gi}");
                assert!(
                    (b1.width.as_mm() - b0.width.as_mm()).abs() > 1e-6
                        || (b1.height.as_mm() - b0.height.as_mm()).abs() > 1e-6,
                    "the drag must resize ({ctx})"
                );
                assert_eq!(arr(r1), STORED, "stored radii exactly as before ({ctx})");
                let ops = ops_since(&s, &before);
                assert_eq!(
                    corner_writes(&ops),
                    [0; 4],
                    "no register written ({ctx}): {ops}"
                );
                assert!(!ops.contains("\"corner_radius\""), "{ctx}");
            }
        }
    }
}

#[test]
fn ac12_proportional_multiplies_all_four_stored_radii_by_the_one_factor() {
    for rotated in [false, true] {
        for (hi, h) in handles().into_iter().enumerate() {
            for (gi, (f, shift, ctrl)) in GESTURES.into_iter().enumerate() {
                let mut s = selected(STORED, rotated);
                s.set_scale_corner_radius(true);
                let (b0, _, _) = rect_of(&s);
                let (press, release) = gesture(h, f, shift, rotated);
                drag(&mut s, press, release, shift, ctrl);
                let (b1, r1, _) = rect_of(&s);
                let sx = b1.width.as_mm() / b0.width.as_mm();
                let sy = b1.height.as_mm() / b0.height.as_mm();
                let k = (sx * sy).sqrt();
                let want = STORED.map(|r| r * k);
                let ctx = format!("rot {rotated} handle {hi} gesture {gi} sx {sx} sy {sy}");
                for i in 0..4 {
                    assert!(
                        close(arr(r1)[i], want[i]),
                        "{ctx}: {:?} want {want:?}",
                        arr(r1)
                    );
                }
                // Ratios between corners stay.
                if k > 1e-9 {
                    assert!(close(arr(r1)[1] / arr(r1)[0], 2.0), "{ctx}");
                } else {
                    assert_eq!(
                        arr(r1),
                        [0.0; 4],
                        "collapsed box: radii at the floor ({ctx})"
                    );
                }
            }
        }
    }
}

#[test]
fn ac15_spec_examples() {
    // 100 x 40 with four radii of 5, sx = sy = 2: off keeps 5, on gives 10; sx 2, sy 1: 5*sqrt 2.
    let d = Document::new(1);
    let id = d.create_rect(rb(0.0, 0.0, 100.0, 40.0));
    d.set_corner_radius(&[id], mm(5.0)).unwrap();
    let bytes = pack(&d, "t").unwrap();
    for (on, f, want) in [
        (false, (2.0, 2.0), 5.0),
        (true, (2.0, 2.0), 10.0),
        (true, (2.0, 1.0), 5.0 * 2.0_f64.sqrt()),
        (false, (2.0, 1.0), 5.0),
    ] {
        let mut s = open_bytes(&bytes);
        click(&mut s, pt(50.0, 0.0));
        s.set_scale_corner_radius(on);
        drag(
            &mut s,
            pt(100.0, 40.0),
            pt(100.0 * f.0, 40.0 * f.1),
            false,
            false,
        );
        let (b, r, _) = rect_of(&s);
        assert!(close(b.width.as_mm(), 100.0 * f.0) && close(b.height.as_mm(), 40.0 * f.1));
        for v in arr(r) {
            assert!(close(v, want), "on {on} f {f:?}: {v} want {want}");
        }
    }
}

#[test]
fn ac12_a_radius_driven_to_zero_is_a_valid_sharp_corner_and_zero_stays_zero() {
    let mut s = selected([0.0, 4.0, 0.0, 8.0], false);
    s.set_scale_corner_radius(true);
    let before = vv(&s);
    drag(&mut s, pt(40.0, 20.0), pt(80.0, 40.0), false, false);
    let (_, r, _) = rect_of(&s);
    assert_eq!(arr(r)[0], 0.0);
    assert_eq!(arr(r)[2], 0.0);
    assert!(close(arr(r)[1], 8.0) && close(arr(r)[3], 16.0));
    let ops = ops_since(&s, &before);
    assert_eq!(
        corner_writes(&ops),
        [0, 1, 0, 1],
        "zero corners are not rewritten: {ops}"
    );
}

#[test]
fn ac13_the_stroke_switch_has_no_effect_on_the_radii() {
    for stroke_on in [false, true] {
        for radius_on in [false, true] {
            let mut s = selected(STORED, false);
            s.set_scale_stroke_width(stroke_on);
            s.set_scale_corner_radius(radius_on);
            drag(&mut s, pt(40.0, 20.0), pt(80.0, 60.0), false, false);
            let (_, r, _) = rect_of(&s);
            let k = (2.0_f64 * 3.0).sqrt();
            for i in 0..4 {
                let want = if radius_on { STORED[i] * k } else { STORED[i] };
                assert!(
                    close(arr(r)[i], want),
                    "stroke {stroke_on} radius {radius_on}"
                );
            }
        }
    }
}

#[test]
fn ac14_a_move_does_not_change_or_rewrite_the_radii() {
    let mut s = selected(STORED, false);
    s.set_scale_corner_radius(true);
    // Move by dragging the body.
    let before = vv(&s);
    drag(&mut s, pt(20.0, 10.0), pt(35.0, 28.0), false, false);
    let (b, r, _) = rect_of(&s);
    assert!(close(b.origin.x, 15.0) && close(b.origin.y, 18.0), "{b:?}");
    assert_eq!(arr(r), STORED);
    assert_eq!(corner_writes(&ops_since(&s, &before)), [0; 4]);
}

#[test]
fn ac15_switch_flip_mid_drag_applies_from_the_next_drag_only() {
    // Read at the press (criterion 15): flipping the switch between press and release changes nothing.
    let mut s = selected(STORED, false);
    let from = pt(40.0, 20.0);
    s.pointer_hover(from, false, false);
    s.pointer_down(from, false);
    s.pointer_hover(pt(60.0, 30.0), false, false);
    s.set_scale_corner_radius(true);
    s.pointer_hover(pt(80.0, 40.0), false, false);
    s.pointer_up(pt(80.0, 40.0), false, false);
    let (_, r, _) = rect_of(&s);
    assert_eq!(arr(r), STORED, "the drag started with the switch off");
}

// ---------------------------------------------------------------------
// Criterion 18: legacy files are not touched by open / select / hover
// ---------------------------------------------------------------------

fn nodes_of(bytes: &[u8]) -> Vec<serde_json::Value> {
    let l = loro::LoroDoc::new();
    l.import(&member(bytes, "document.loro")).unwrap();
    let v = serde_json::to_value(l.get_deep_value()).unwrap();
    v["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["meta"].clone())
        .collect()
}

#[test]
fn ac18_open_hover_select_and_handle_hover_write_nothing_to_a_legacy_file() {
    let bytes = fixture("legacy_corner_radius_v5.curvyo");
    let l0 = loro::LoroDoc::new();
    l0.import(&member(&bytes, "document.loro")).unwrap();
    let vv0 = l0.oplog_vv();
    let nodes0 = nodes_of(&bytes);

    let mut s = open_bytes(&bytes);
    assert_eq!(vv(&s), vv0, "open");
    // Hover and click every object at many points, hover every handle of the selection.
    let d = doc_of(&s);
    for id in d.object_ids() {
        let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
            panic!()
        };
        let Shape::Rect { bounds, .. } = p.shape else {
            continue;
        };
        let (x, y) = (bounds.origin.x, bounds.origin.y);
        let (w, h) = (bounds.width.as_mm(), bounds.height.as_mm());
        for (px, py) in [
            (x + w / 2.0, y),
            (x, y + h / 2.0),
            (x + w, y + h),
            (x, y),
            (x + w / 2.0, y + h / 2.0),
        ] {
            s.pointer_hover(pt(px, py), false, false);
        }
        click(&mut s, pt(x + w / 2.0, y));
        for (px, py) in [
            (x, y),
            (x + w, y),
            (x + w, y + h),
            (x, y + h),
            (x + w / 2.0, y),
            (x + w / 2.0, y + h),
        ] {
            s.pointer_hover(pt(px, py), false, false);
        }
        let _ = s.select_bar_state();
    }
    s.pointer_leave();
    assert_eq!(vv(&s), vv0, "no operation after open + hover + select");
    let saved = s.pack("0.1.0").unwrap();
    assert_eq!(
        nodes_of(&saved),
        nodes0,
        "node keys unchanged in the saved file"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&member(&saved, "manifest.json")).unwrap();
    assert_eq!(manifest["format_version"], 6);
}

#[test]
fn ac18_legacy_rectangle_resizes_like_before_with_both_switch_states() {
    let bytes = fixture("legacy_corner_radius_v5.curvyo");
    let d = unpack(2, &bytes).unwrap();
    let id = d
        .object_ids()
        .into_iter()
        .find(|id| {
            matches!(d.object(*id).unwrap(), ObjectSnapshot::Primitive(p)
                if matches!(p.shape, Shape::Rect { corner_radii, .. } if corner_radii.tl.as_mm() > 0.0))
        })
        .unwrap();
    let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
        panic!()
    };
    let Shape::Rect {
        bounds,
        corner_radii,
    } = p.shape
    else {
        panic!()
    };
    let legacy = corner_radii.tl.as_mm();
    let (x, y) = (bounds.origin.x, bounds.origin.y);
    let (w, h) = (bounds.width.as_mm(), bounds.height.as_mm());
    for on in [false, true] {
        let mut s = open_bytes(&bytes);
        let before = vv(&s);
        click(&mut s, pt(x + w / 2.0, y));
        s.set_scale_corner_radius(on);
        drag(
            &mut s,
            pt(x + w, y + h),
            pt(x + 2.0 * w, y + 2.0 * h),
            false,
            false,
        );
        let nd = doc_of(&s);
        let r = nd
            .object_ids()
            .into_iter()
            .find_map(|i| match nd.object(i).unwrap() {
                ObjectSnapshot::Primitive(p) if i == id => match p.shape {
                    Shape::Rect { corner_radii, .. } => Some(corner_radii),
                    _ => None,
                },
                _ => None,
            })
            .unwrap();
        let want = if on { legacy * 2.0 } else { legacy };
        assert_eq!(arr(r), [want; 4], "switch {on}");
        let ops = ops_since(&s, &before);
        assert!(
            !ops.contains("\"corner_radius\""),
            "legacy key never written: {ops}"
        );
        if on {
            assert_eq!(corner_writes(&ops), [1; 4], "{ops}");
        } else {
            assert_eq!(
                corner_writes(&ops),
                [0; 4],
                "Keep on a legacy node writes nothing: {ops}"
            );
        }
    }
}

// ---------------------------------------------------------------------
// Criterion 22 (data layer): the Select bar field
// ---------------------------------------------------------------------

#[test]
fn ac22_unequal_corners_show_mixed_equal_corners_show_the_value() {
    let s = selected([12.0, 0.0, 12.0, 0.0], false);
    let st = s.select_bar_state();
    assert_eq!(st.radius, Some(BarValue::Mixed));
    assert!(st.remove_rounding_shown && st.remove_rounding_enabled);
    assert_eq!(st.radius_limited, None, "Mixed shows no 'limited' tag");

    let s = selected([5.0, 5.0, 5.0, 5.0], false);
    let st = s.select_bar_state();
    assert_eq!(st.radius, Some(BarValue::Uniform(mm(5.0))));
    assert_eq!(st.radius_limited, None);

    // Equal within the tolerance (1e-9 mm) counts as equal.
    let s = selected([5.0, 5.0 + 1e-12, 5.0, 5.0 - 1e-12], false);
    assert!(matches!(
        s.select_bar_state().radius,
        Some(BarValue::Uniform(_))
    ));
    // Differing by more is Mixed.
    let s = selected([5.0, 5.0 + 1e-6, 5.0, 5.0], false);
    assert_eq!(s.select_bar_state().radius, Some(BarValue::Mixed));

    // A rounded corner next to sharp ones with equal effective radii?  0 vs 0 are equal.
    let s = selected([0.0; 4], false);
    assert_eq!(
        s.select_bar_state().radius,
        Some(BarValue::Uniform(mm(0.0)))
    );
    assert!(!s.select_bar_state().remove_rounding_enabled);
}

#[test]
fn ac22_mixed_is_judged_on_effective_radii_and_uniform_may_be_limited() {
    // Stored 30 on a 40 x 20 box: effective 10 each (half the shorter side): Uniform(10), limited 30.
    let s = selected([30.0; 4], false);
    let st = s.select_bar_state();
    match st.radius {
        Some(BarValue::Uniform(v)) => assert!(close(v.as_mm(), 10.0), "{v:?}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(st.radius_limited.map(Length::as_mm), Some(30.0));
    // Unequal stored radii that become equal after the clamp are Uniform (effective values decide):
    // tl = 30, tr = 30 (sum 60 > 40, but H/(tl+bl)... bl=br=0: f = min(40/60, ...) = 0.666: 20, 20 > ...
    let s = selected([30.0, 30.0, 0.0, 0.0], false);
    let st = s.select_bar_state();
    assert_eq!(
        st.radius,
        Some(BarValue::Mixed),
        "TL, TR rounded and BR, BL sharp"
    );
}

#[test]
fn ac22_typing_a_radius_sets_all_four_limited_to_half_the_shorter_side_in_one_commit() {
    let mut s = selected([12.0, 0.0, 3.0, 0.0], false);
    let before = vv(&s);
    let out = s.set_selected_radius_text("4");
    assert!(
        format!("{out:?}").to_lowercase().contains("commit"),
        "{out:?}"
    );
    let (_, r, _) = rect_of(&s);
    assert_eq!(arr(r), [4.0; 4]);
    let ops = ops_since(&s, &before);
    assert_eq!(corner_writes(&ops), [1, 1, 1, 1], "all four changed: {ops}");

    // Larger than half the shorter side (10): limited to 10.
    let mut s = selected([12.0, 0.0, 3.0, 0.0], false);
    s.set_selected_radius_text("99");
    let (_, r, _) = rect_of(&s);
    assert_eq!(arr(r), [10.0; 4]);

    // Typing the value one corner already has writes only the other three.
    let mut s = selected([4.0, 0.0, 0.0, 0.0], false);
    let before = vv(&s);
    s.set_selected_radius_text("4");
    let ops = ops_since(&s, &before);
    assert_eq!(corner_writes(&ops), [0, 1, 1, 1], "{ops}");
}

#[test]
fn ac8_remove_rounding_zeroes_every_register_that_is_not_zero_in_one_commit() {
    let mut s = selected([12.0, 0.0, 3.0, 0.0], false);
    let before = vv(&s);
    s.remove_corner_rounding();
    let (_, r, _) = rect_of(&s);
    assert_eq!(arr(r), [0.0; 4]);
    let ops = ops_since(&s, &before);
    assert_eq!(corner_writes(&ops), [1, 0, 1, 0], "{ops}");
    assert!(!s.select_bar_state().remove_rounding_enabled);
    // Again: nothing to do, nothing written.
    let before = vv(&s);
    s.remove_corner_rounding();
    assert_eq!(vv(&s), before);
}

// ---------------------------------------------------------------------
// Criteria 16, 11, 21: Object to path
// ---------------------------------------------------------------------

fn dist_seg(p: Point, a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.0) * dx + (p.y - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.x - (a.0 + t * dx)).hypot(p.y - (a.1 + t * dy))
}

fn dist_boundary(p: Point, w: f64, h: f64, e: [f64; 4]) -> f64 {
    let mut d = f64::MAX;
    d = d.min(dist_seg(p, (e[0], 0.0), (w - e[1], 0.0)));
    d = d.min(dist_seg(p, (w, e[1]), (w, h - e[2])));
    d = d.min(dist_seg(p, (w - e[2], h), (e[3], h)));
    d = d.min(dist_seg(p, (0.0, h - e[3]), (0.0, e[0])));
    for (cx, cy, sx, sy, r) in [
        (e[0], e[0], -1.0, -1.0, e[0]),
        (w - e[1], e[1], 1.0, -1.0, e[1]),
        (w - e[2], h - e[2], 1.0, 1.0, e[2]),
        (e[3], h - e[3], -1.0, 1.0, e[3]),
    ] {
        if r <= 0.0 {
            d = d.min((p.x - cx).hypot(p.y - cy));
            continue;
        }
        let (vx, vy) = (p.x - cx, p.y - cy);
        if vx * sx >= 0.0 && vy * sy >= 0.0 {
            d = d.min((vx.hypot(vy) - r).abs());
        }
        d = d
            .min((p.x - (cx + sx * r)).hypot(p.y - cy))
            .min((p.x - cx).hypot(p.y - (cy + sy * r)));
    }
    d
}

#[test]
fn ac16_object_to_path_gives_four_to_eight_corner_nodes_on_the_effective_outline() {
    let cases: [([f64; 4], usize); 6] = [
        ([0.0; 4], 4),
        ([5.0; 4], 8),
        ([5.0, 0.0, 0.0, 0.0], 5),
        ([0.0, 6.0, 0.0, 3.0], 6),
        ([12.0, 0.0, 12.0, 0.0], 6),
        ([30.0, 30.0, 0.0, 30.0], 7), // f < 1 on 40 x 20: effective 10-ish
    ];
    for rotated in [false, true] {
        for (r, nodes) in cases {
            let mut s = selected(r, rotated);
            let (b, rr, _) = rect_of(&s);
            let e = arr(curvyo_document_core::effective_corner_radii(b, rr));
            s.convert_selected_to_paths();
            let d = doc_of(&s);
            let id = d.object_ids()[0];
            let path = d.path(id).expect("converted to a path");
            assert!(path.closed, "{r:?}");
            assert_eq!(
                path.anchors.len(),
                nodes,
                "rotated {rotated} {r:?} eff {e:?}"
            );
            assert!(
                path.anchors.iter().all(|a| a.kind == AnchorKind::Corner),
                "{r:?}"
            );
            // First anchor: end of TL's arc (or the TL corner), in document space.
            let c = pt(20.0, 10.0);
            let a = if rotated { ROT } else { 0.0 };
            let tl_end = rot(pt(e[0], 0.0), c, a);
            let first = path.anchors[0].point;
            assert!(
                close(first.x, tl_end.x) && close(first.y, tl_end.y),
                "{first:?} vs {tl_end:?}"
            );
            // Every anchor lies on the effective outline (local frame), clockwise order.
            let back = |p: Point| rot(p, c, -a);
            for an in &path.anchors {
                let l = back(an.point);
                assert!(dist_boundary(l, 40.0, 20.0, e) < 1e-6, "{l:?} {e:?}");
            }
            // Winding: positive area (clockwise on screen).
            let mut area = 0.0;
            for i in 0..path.anchors.len() {
                let p = path.anchors[i].point;
                let q = path.anchors[(i + 1) % path.anchors.len()].point;
                area += p.x * q.y - q.x * p.y;
            }
            assert!(area > 0.0, "clockwise {r:?}");
            // Same as the outline function.
            let d2 = open(&build(r, rotated).0);
            let _ = d2;
            let (b0, r0, rot0) = (
                rb(0.0, 0.0, 40.0, 20.0),
                radii(r[0], r[1], r[2], r[3]),
                if rotated { ROT } else { 0.0 },
            );
            let want = outline_of_rotated(
                &Shape::Rect {
                    bounds: b0,
                    corner_radii: r0,
                },
                Angle::from_radians(rot0),
            );
            assert_eq!(want.len(), path.anchors.len());
            for (w, g) in want.iter().zip(&path.anchors) {
                assert!(close(w.point.x, g.point.x) && close(w.point.y, g.point.y));
                assert!(close(w.handle_in.x, g.handle_in.x) && close(w.handle_in.y, g.handle_in.y));
                assert!(
                    close(w.handle_out.x, g.handle_out.x) && close(w.handle_out.y, g.handle_out.y)
                );
            }
        }
    }
}

#[test]
fn ac16_object_to_path_of_a_legacy_rectangle_is_as_before() {
    let bytes = fixture("legacy_corner_radius_v5.curvyo");
    let d = unpack(2, &bytes).unwrap();
    let mut counts = Vec::new();
    for id in d.object_ids() {
        let ObjectSnapshot::Primitive(p) = d.object(id).unwrap() else {
            panic!()
        };
        let Shape::Rect { corner_radii, .. } = p.shape else {
            continue;
        };
        counts.push((id, if corner_radii.tl.as_mm() > 0.0 { 8 } else { 4 }));
    }
    let mut s = open_bytes(&bytes);
    // Select all rectangles one by one by clicking their outline.
    let doc = doc_of(&s);
    for (id, n) in counts {
        let ObjectSnapshot::Primitive(p) = doc.object(id).unwrap() else {
            panic!()
        };
        let Shape::Rect { bounds, .. } = p.shape else {
            panic!()
        };
        click(
            &mut s,
            pt(
                bounds.origin.x + bounds.width.as_mm() / 2.0,
                bounds.origin.y,
            ),
        );
        s.convert_selected_to_paths();
        let after = doc_of(&s);
        assert_eq!(after.path(id).expect("a path now").anchors.len(), n);
        s.set_tool(Tool::Select);
    }
}

// ---------------------------------------------------------------------
// Criterion 14/15: a typed size follows the same rules, the switch read when the entry opens
// ---------------------------------------------------------------------

fn dbl(s: &mut Session, at: Point) {
    s.pointer_hover(at, false, false);
    s.pointer_down(at, false);
    s.pointer_up(at, false, false);
    s.double_click(at, false, false);
}

#[test]
fn ac14_a_typed_size_keeps_or_scales_all_four_radii_by_the_switch() {
    for rotated in [false, true] {
        for on in [false, true] {
            let mut s = selected(STORED, rotated);
            s.set_scale_corner_radius(on);
            let before = vv(&s);
            let se = rot(
                pt(40.0, 20.0),
                pt(20.0, 10.0),
                if rotated { ROT } else { 0.0 },
            );
            dbl(&mut s, se);
            assert_eq!(s.transform_entry().expect("size entry").kind, "size");
            let out = s.commit_transform_entry("80", "60", 0);
            let (b1, r1, _) = rect_of(&s);
            let ctx = format!("rotated {rotated} switch {on}: {out:?}");
            assert!(
                close(b1.width.as_mm(), 80.0) && close(b1.height.as_mm(), 60.0),
                "{ctx}: {b1:?}"
            );
            let k = (2.0_f64 * 3.0).sqrt();
            for i in 0..4 {
                let want = if on { STORED[i] * k } else { STORED[i] };
                assert!(close(arr(r1)[i], want), "{ctx}: {:?} want {want}", arr(r1));
            }
            let ops = ops_since(&s, &before);
            if on {
                assert_eq!(corner_writes(&ops), [1; 4], "{ctx}");
            } else {
                assert_eq!(corner_writes(&ops), [0; 4], "{ctx}");
            }
        }
    }
}

#[test]
fn ac14_a_click_on_the_switch_closes_an_open_size_entry_without_writing() {
    let mut s = selected(STORED, false);
    let before = vv(&s);
    dbl(&mut s, pt(40.0, 20.0));
    assert!(s.transform_entry().is_some());
    s.set_scale_corner_radius(true);
    assert!(s.transform_entry().is_none(), "entry closed by the switch");
    assert_eq!(vv(&s), before);
}
