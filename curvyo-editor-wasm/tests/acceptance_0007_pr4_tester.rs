//! Independent tester cases for `0007-stroke-and-fill-styling`, PR 4 at the
//! `Session` level: fill-type switching keeps stops (13, 17), the stop editor's
//! commands (18 to 20, 34, 35), multi-selection by rank, mixed fill types,
//! live drags of a stop, hit-testing of gradient fills (23), the gradient box
//! through copy, split, join and "Object to path" (30 to 33, 21, 22), and the
//! file round trip of the draw list. Written from `specification.md` before the
//! PR 4 implementation was read. Everything shader-side is wasm32-only and not
//! covered here.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(clippy::too_many_lines, clippy::cast_precision_loss, missing_docs)]
#![allow(clippy::many_single_char_names, clippy::assert_is_empty)]
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use curvyo_document_core::{
    AnchorId, Angle, Color, CopySource, Document, EllipseFrame, FillKind, FillMode, FillModeTarget,
    GradientStop, Length, NewAnchor, NodeId, ObjectSnapshot, Opacity, Point, RectBounds, StopId,
    StopPosition, Style, StyleEdit, Vec2, pack, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::GradientFill;
use curvyo_ui_core::StopField;
use loro::LoroDoc;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

fn stop(n: u64, p: f64, c: Color, o: f64) -> GradientStop {
    GradientStop {
        id: StopId::new(9, n),
        position: StopPosition::new(p).unwrap(),
        color: c,
        opacity: Opacity::new(o).unwrap(),
    }
}

fn rect(d: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    d.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(w),
        height: Length::from_mm(h),
    })
}

fn open(d: &Document) -> Session {
    let mut s = Session::open(5, &pack(d, "0.1.0").unwrap()).unwrap();
    s.set_tool(Tool::Select);
    s
}

fn click(s: &mut Session, p: Point, shift: bool) {
    s.pointer_hover(p, shift, false);
    s.pointer_down(p, shift);
    s.pointer_up(p, shift, false);
}

fn pick(s: &mut Session, p: Point) {
    click(s, pt(900.0, 900.0), false);
    click(s, p, false);
}

/// Selects the objects whose rectangles start at the given x (each 10 mm, y 0
/// to 10) by clicking their interiors; the objects must be filled.
fn select_rects(s: &mut Session, xs: &[f64]) {
    click(s, pt(900.0, 900.0), false);
    for (k, x) in xs.iter().enumerate() {
        click(s, pt(x + 5.0, 5.0), k > 0);
    }
}

fn doc_of(s: &Session) -> Document {
    unpack(9, &s.pack("0.1.0").unwrap()).unwrap()
}

fn style_of(d: &Document, id: NodeId) -> Style {
    match d.object(id).unwrap() {
        ObjectSnapshot::Path(p) => p.style,
        ObjectSnapshot::Primitive(p) => p.style,
    }
}

fn ids(d: &Document) -> Vec<NodeId> {
    d.object_ids()
}

fn vv(s: &Session) -> loro::VersionVector {
    let l = LoroDoc::new();
    l.import(&doc_of(s).export_loro_snapshot().unwrap())
        .unwrap();
    l.oplog_vv()
}

fn solid(d: &Document, id: NodeId, c: Color) {
    d.edit_style(&[id], &StyleEdit::FillColor(c)).unwrap();
    d.set_fill_mode(
        FillMode::Solid,
        &[FillModeTarget {
            id,
            seed_stops: vec![],
        }],
    )
    .unwrap();
}

fn gradient(d: &Document, id: NodeId, mode: FillMode, stops: Vec<GradientStop>) {
    d.set_fill_mode(
        mode,
        &[FillModeTarget {
            id,
            seed_stops: stops,
        }],
    )
    .unwrap();
}

fn fills(s: &Session) -> Vec<GradientFill> {
    s.draw_list().gradients().to_vec()
}

// ------------------------------------------- AC 13, 17: switching fill type

#[test]
fn ac17_solid_to_linear_gives_the_stored_colour_to_white_and_every_switch_keeps_the_stops() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 10.0, 10.0);
    solid(&d, r, rgb(255, 0, 0));
    let mut s = open(&d);
    click(&mut s, pt(5.0, 5.0), false);
    s.set_fill_mode(FillMode::Linear);
    let st = style_of(&doc_of(&s), r);
    assert_eq!(st.fill.kind, FillKind::Linear);
    assert_eq!(st.fill.stops.len(), 2);
    assert_eq!(
        (
            st.fill.stops[0].position.get(),
            st.fill.stops[0].color,
            st.fill.stops[0].opacity.get()
        ),
        (0.0, rgb(255, 0, 0), 1.0)
    );
    assert_eq!(
        (
            st.fill.stops[1].position.get(),
            st.fill.stops[1].color,
            st.fill.stops[1].opacity.get()
        ),
        (1.0, rgb(255, 255, 255), 1.0)
    );
    let seeded = st.fill.stops.clone();
    // Edit a stop, then walk through every fill type: stops and the stored
    // solid colour survive each switch.
    s.set_stop_text(1, StopField::Color, "#0F0").unwrap();
    let edited = style_of(&doc_of(&s), r).fill.stops;
    assert_ne!(edited, seeded);
    for mode in [
        FillMode::Radial,
        FillMode::Solid,
        FillMode::None,
        FillMode::Linear,
        FillMode::None,
        FillMode::Radial,
    ] {
        s.set_fill_mode(mode);
        let st = style_of(&doc_of(&s), r);
        assert_eq!(st.fill.stops, edited, "{mode:?} kept the stops");
        assert_eq!(st.fill.color, rgb(255, 0, 0), "{mode:?} kept the colour");
        assert_eq!(st.fill.enabled, mode != FillMode::None);
    }
}

#[test]
fn ac17_white_pairs_with_black_and_an_unset_colour_is_black_to_white() {
    let d = Document::new(1);
    let w = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let u = rect(&d, 30.0, 0.0, 10.0, 10.0);
    solid(&d, w, rgb(255, 255, 255));
    solid(&d, u, Color::BLACK);
    d.edit_style(&[u], &StyleEdit::FillColor(Color::BLACK))
        .unwrap();
    let mut s = open(&d);
    for x in [0.0, 30.0] {
        pick(&mut s, pt(x + 5.0, 5.0));
        s.set_fill_mode(FillMode::Radial);
    }
    let after = doc_of(&s);
    let ids_ = ids(&after);
    let by = |n: usize| style_of(&after, ids_[n]).fill.stops;
    assert_eq!(
        (by(0)[0].color, by(0)[1].color),
        (rgb(255, 255, 255), rgb(0, 0, 0))
    );
    assert_eq!(
        (by(1)[0].color, by(1)[1].color),
        (rgb(0, 0, 0), rgb(255, 255, 255))
    );
}

// ------------------------------------------------------- stop commands

fn linear_rect(stops: Vec<GradientStop>) -> (Session, NodeId) {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 10.0, 10.0);
    gradient(&d, r, FillMode::Linear, stops);
    let mut s = open(&d);
    click(&mut s, pt(5.0, 5.0), false);
    (s, r)
}

fn three() -> Vec<GradientStop> {
    vec![
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 0.5, rgb(0, 255, 0), 1.0),
        stop(3, 1.0, rgb(0, 0, 255), 1.0),
    ]
}

#[test]
fn ac20_editing_one_value_changes_only_that_value_and_the_selected_stop_follows_a_re_sort() {
    let (mut s, r) = linear_rect(three());
    let before = style_of(&doc_of(&s), r).fill.stops;
    assert_eq!(s.set_stop_text(0, StopField::Position, "80"), Ok(true));
    let after = style_of(&doc_of(&s), r).fill.stops;
    assert_eq!(after[0].position.get(), 0.8);
    assert_eq!(
        (after[0].color, after[0].opacity),
        (before[0].color, before[0].opacity)
    );
    assert_eq!(&after[1..], &before[1..], "other stops unchanged");
    assert_eq!(
        s.style_panel_view().selected_stop,
        1,
        "between the 50 % and the 100 % stop now"
    );
    // Colour and opacity of the stop that moved, addressed by its new rank.
    assert_eq!(s.set_stop_text(1, StopField::Color, "#abc"), Ok(true));
    assert_eq!(s.set_stop_text(1, StopField::Opacity, "40"), Ok(true));
    let end = style_of(&doc_of(&s), r).fill.stops;
    assert_eq!(end[0].color, rgb(0xaa, 0xbb, 0xcc));
    assert_eq!(end[0].opacity.get(), 0.4);
    assert_eq!(end[0].position.get(), 0.8);
    assert_eq!(&end[1..], &before[1..]);
}

#[test]
fn ac20_bad_text_and_a_missing_rank_write_nothing() {
    let (mut s, _) = linear_rect(three());
    let v0 = vv(&s);
    for (field, text) in [
        (StopField::Position, "101"),
        (StopField::Position, "-1"),
        (StopField::Position, "abc"),
        (StopField::Position, ""),
        (StopField::Position, "NaN"),
        (StopField::Position, "inf"),
        (StopField::Color, "#12"),
        (StopField::Color, "#GGG"),
        (StopField::Color, "12345678"),
        (StopField::Opacity, "101"),
        (StopField::Opacity, "-5"),
        (StopField::Opacity, "1e400"),
    ] {
        assert!(
            s.set_stop_text(1, field, text).is_err(),
            "{field:?} {text:?}"
        );
    }
    assert_eq!(s.set_stop_text(7, StopField::Color, "#fff"), Ok(false));
    assert_eq!(vv(&s), v0, "nothing written");
}

#[test]
fn ac20_one_decimal_position_is_stored_as_typed() {
    let (mut s, r) = linear_rect(three());
    assert_eq!(s.set_stop_text(1, StopField::Position, "12.5"), Ok(true));
    let st = style_of(&doc_of(&s), r).fill.stops;
    assert_eq!(st[1].position.get(), 0.125);
    assert_eq!(s.set_stop_text(1, StopField::Position, "12.5%"), Ok(true));
}

#[test]
fn ac18_ac19_limits_add_refused_at_16_remove_refused_at_2() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 10.0, 10.0);
    solid(&d, r, rgb(255, 0, 0));
    let mut s = open(&d);
    click(&mut s, pt(5.0, 5.0), false);
    s.set_fill_mode(FillMode::Linear);
    for k in 2..16 {
        assert!(s.add_stop(None), "add #{k}");
    }
    assert_eq!(style_of(&doc_of(&s), r).fill.stops.len(), 16);
    let v = s.style_panel_view();
    assert!(!v.stops_can_add && v.stops_can_remove);
    let before = vv(&s);
    assert!(!s.add_stop(None), "17th refused");
    assert!(!s.add_stop(Some(0.3)), "bar click refused at 16");
    assert_eq!(vv(&s), before, "refusal writes nothing");
    for k in (3..=16).rev() {
        assert!(s.remove_stop(0), "remove at {k}");
    }
    assert_eq!(style_of(&doc_of(&s), r).fill.stops.len(), 2);
    let v = s.style_panel_view();
    assert!(v.stops_can_add && !v.stops_can_remove);
    let before = vv(&s);
    assert!(!s.remove_stop(0), "a gradient keeps 2");
    assert_eq!(vv(&s), before);
    assert!(!s.remove_stop(5), "no such rank");
}

#[test]
fn ac18_a_bar_click_takes_the_clicked_position_and_the_ramp_colour_so_nothing_changes() {
    let (mut s, r) = linear_rect(vec![
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 1.0, rgb(0, 0, 255), 0.5),
    ]);
    let ramp_before = fills(&s)[0].ramp.clone();
    assert!(s.add_stop(Some(0.3)));
    let st = style_of(&doc_of(&s), r).fill.stops;
    assert_eq!(st.len(), 3);
    let n = st
        .iter()
        .find(|x| x.id != StopId::new(9, 1) && x.id != StopId::new(9, 2))
        .unwrap();
    assert!((n.position.get() - 0.3).abs() < 1e-9);
    assert!((f64::from(n.color.r) - 178.5).abs() <= 1.5, "{:?}", n.color);
    assert!((n.opacity.get() - 0.85).abs() < 0.005);
    let ramp_after = &fills(&s)[0].ramp;
    for (a, b) in ramp_before.0.iter().zip(ramp_after.0.iter()) {
        for ch in 0..4 {
            assert!(
                (i32::from(a[ch]) - i32::from(b[ch])).abs() <= 1,
                "{a:?} vs {b:?}"
            );
        }
    }
    assert_eq!(
        s.style_panel_view().selected_stop,
        1,
        "the new stop is selected"
    );
    // A click past the ends clamps and, at the edge, coincides.
    assert!(s.add_stop(Some(-4.0)));
    assert!(s.add_stop(Some(9.0)));
    let st = style_of(&doc_of(&s), r).fill.stops;
    assert_eq!(st.iter().filter(|x| x.position.get() == 0.0).count(), 2);
    assert_eq!(st.iter().filter(|x| x.position.get() == 1.0).count(), 2);
}

#[test]
fn ac35_add_on_a_gradient_with_no_stop_list_creates_one_stop_at_fifty_percent() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 10.0, 10.0);
    d.edit_style(&[r], &StyleEdit::FillColor(rgb(9, 8, 7)))
        .unwrap();
    gradient(&d, r, FillMode::Radial, vec![]);
    let mut s = open(&d);
    click(&mut s, pt(0.0, 5.0), false); // the outline; there is no fill to click
    let v = s.style_panel_view();
    assert_eq!(v.stops_state, "editor");
    assert!(v.stops_can_add && !v.stops_can_remove);
    assert!(s.add_stop(None));
    let st = style_of(&doc_of(&s), r);
    assert_eq!(st.fill.kind, FillKind::Radial, "the mode stays");
    assert_eq!(st.fill.stops.len(), 1);
    assert_eq!(st.fill.stops[0].position.get(), 0.5);
    assert_eq!(st.fill.stops[0].color, rgb(9, 8, 7));
    assert_eq!(st.fill.stops[0].opacity.get(), 1.0);
    assert!(s.add_stop(None), "a second");
    assert_eq!(style_of(&doc_of(&s), r).fill.stops.len(), 2);
}

// --------------------------------------------- multi-selection by rank

fn three_rects() -> (Session, Vec<NodeId>) {
    let d = Document::new(1);
    let colours = [rgb(255, 0, 0), rgb(0, 255, 0), rgb(0, 0, 255)];
    let mut v = Vec::new();
    for (k, c) in colours.iter().enumerate() {
        let r = rect(&d, k as f64 * 30.0, 0.0, 10.0, 10.0);
        solid(&d, r, *c);
        v.push(r);
    }
    let mut s = open(&d);
    select_rects(&mut s, &[0.0, 30.0, 60.0]);
    assert_eq!(s.selected_object_count(), 3);
    (s, v)
}

#[test]
fn ac17_ac34_a_multi_selection_seeds_each_object_from_its_own_colour_then_edits_by_rank() {
    let (mut s, v) = three_rects();
    s.set_fill_mode(FillMode::Linear);
    let d = doc_of(&s);
    let own = [rgb(255, 0, 0), rgb(0, 255, 0), rgb(0, 0, 255)];
    for (k, id) in v.iter().enumerate() {
        let st = style_of(&d, *id).fill.stops;
        assert_eq!(st.len(), 2);
        assert_eq!(st[0].color, own[k]);
    }
    let pv = s.style_panel_view();
    assert_eq!(pv.stops_state, "editor");
    assert_eq!(pv.stops_objects, 3);
    assert!(!pv.stops_can_add && !pv.stops_can_remove);
    assert!(!pv.stops_bar_shown, "colours differ: hatched track");
    // Rank 1 colour for all three.
    assert_eq!(s.set_stop_text(1, StopField::Color, "#102030"), Ok(true));
    let d = doc_of(&s);
    for (k, id) in v.iter().enumerate() {
        let st = style_of(&d, *id).fill.stops;
        assert_eq!(st[1].color, rgb(0x10, 0x20, 0x30));
        assert_eq!(st[0].color, own[k], "rank 0 untouched");
        assert_eq!(st[1].position.get(), 1.0);
    }
    let pv = s.style_panel_view();
    assert_eq!(pv.stop_rows[9], 0.0, "row 1 colours now agree");
    assert_eq!(pv.stop_rows[3], 1.0, "row 0 colours still differ");
    // Add and Remove are refused for several objects.
    let before = vv(&s);
    assert!(!s.add_stop(None));
    assert!(!s.remove_stop(0));
    assert_eq!(vv(&s), before);
}

#[test]
fn ac34_different_counts_show_the_message_and_stop_edits_write_nothing() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&d, 30.0, 0.0, 10.0, 10.0);
    gradient(
        &d,
        a,
        FillMode::Linear,
        vec![
            stop(1, 0.0, rgb(1, 1, 1), 1.0),
            stop(2, 1.0, rgb(2, 2, 2), 1.0),
        ],
    );
    gradient(&d, b, FillMode::Linear, three());
    let mut s = open(&d);
    select_rects(&mut s, &[0.0, 30.0]);
    assert_eq!(s.style_panel_view().stops_state, "different-counts");
    let before = vv(&s);
    let r = s.set_stop_text(0, StopField::Color, "#fff");
    assert_eq!(
        vv(&s),
        before,
        "no edit while the editor is not shown (got {r:?})"
    );
}

#[test]
fn ac34_a_mix_of_modes_hides_the_editor_and_a_mode_pick_keeps_each_objects_stops() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0); // solid
    let b = rect(&d, 30.0, 0.0, 10.0, 10.0); // linear, edited stops
    let c = rect(&d, 60.0, 0.0, 10.0, 10.0); // solid, then none
    solid(&d, a, rgb(5, 6, 7));
    gradient(&d, b, FillMode::Linear, three());
    solid(&d, c, rgb(8, 9, 10));
    let mut s = open(&d);
    select_rects(&mut s, &[0.0, 30.0, 60.0]);
    let pv = s.style_panel_view();
    assert_eq!(pv.stops_state, "hidden");
    assert_eq!(pv.fill_mode, "mixed", "no mode shown as pressed");
    s.set_fill_mode(FillMode::Radial);
    let after = doc_of(&s);
    for id in [a, b, c] {
        let f = style_of(&after, id).fill;
        assert_eq!(f.kind, FillKind::Radial);
        assert!(f.enabled);
    }
    assert_eq!(
        style_of(&after, b).fill.stops,
        three(),
        "b keeps its three stops"
    );
    let sa = style_of(&after, a).fill.stops;
    assert_eq!((sa.len(), sa[0].color), (2, rgb(5, 6, 7)));
    let sc = style_of(&after, c).fill.stops;
    assert_eq!((sc.len(), sc[0].color), (2, rgb(8, 9, 10)));
    assert_eq!(s.style_panel_view().stops_state, "different-counts");
}

// ---------------------------------------------- live drag of one stop

#[test]
fn ac20_a_thumb_dragged_past_its_neighbour_keeps_editing_the_stop_it_started_on() {
    let (mut s, r) = linear_rect(three());
    let before = style_of(&doc_of(&s), r).fill.stops;
    let v0 = vv(&s);
    let ramp0 = fills(&s)[0].ramp.clone();
    s.preview_stop(0, StopField::Position, 30.0);
    s.preview_stop(0, StopField::Position, 60.0); // past the stop at 50 %
    s.preview_stop(0, StopField::Position, 80.0);
    assert_eq!(vv(&s), v0, "a drag writes nothing until release");
    assert_ne!(fills(&s)[0].ramp, ramp0, "but the canvas shows it");
    s.commit_style_preview();
    let after = style_of(&doc_of(&s), r).fill.stops;
    assert_eq!(after[0].position.get(), 0.8);
    assert_eq!(after[0].id, before[0].id);
    assert_eq!(&after[1..], &before[1..]);
    assert_eq!(s.style_panel_view().selected_stop, 1);
}

#[test]
fn ac36_escape_drops_a_stop_drag_and_the_release_writes_nothing() {
    let (mut s, r) = linear_rect(three());
    let before = style_of(&doc_of(&s), r).fill.stops;
    let v0 = vv(&s);
    let ramp0 = fills(&s)[0].ramp.clone();
    s.preview_stop(1, StopField::Color, f64::from(0x00_FF_FF_FF_u32));
    s.preview_stop(1, StopField::Opacity, 10.0);
    s.cancel_style_preview();
    s.commit_style_preview();
    assert_eq!(vv(&s), v0);
    assert_eq!(style_of(&doc_of(&s), r).fill.stops, before);
    assert_eq!(fills(&s)[0].ramp, ramp0, "the canvas is back");
}

#[test]
fn ac20_a_colour_drag_commits_once_and_only_that_value() {
    let (mut s, r) = linear_rect(three());
    let before = style_of(&doc_of(&s), r).fill.stops;
    for c in [0x0011_2233_u32, 0x0044_5566, 0x0077_8899] {
        s.preview_stop(1, StopField::Color, f64::from(c));
    }
    s.commit_style_preview();
    let after = style_of(&doc_of(&s), r).fill.stops;
    assert_eq!(after[1].color, rgb(0x77, 0x88, 0x99));
    assert_eq!(
        (after[1].position, after[1].opacity),
        (before[1].position, before[1].opacity)
    );
    assert_eq!(after[0], before[0]);
    assert_eq!(after[2], before[2]);
}

// ------------------------------------------------------- hit-testing 23

#[test]
fn ac23_ac35_gradient_interiors_select_by_stops_not_by_opacity() {
    let d = Document::new(1);
    let transparent = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let one = rect(&d, 30.0, 0.0, 10.0, 10.0);
    let zero = rect(&d, 60.0, 0.0, 10.0, 10.0);
    let radial = rect(&d, 90.0, 0.0, 10.0, 10.0);
    gradient(
        &d,
        transparent,
        FillMode::Linear,
        vec![
            stop(1, 0.0, rgb(1, 1, 1), 0.0),
            stop(2, 1.0, rgb(1, 1, 1), 0.0),
        ],
    );
    gradient(
        &d,
        one,
        FillMode::Linear,
        vec![stop(3, 0.4, rgb(1, 1, 1), 1.0)],
    );
    gradient(&d, zero, FillMode::Linear, vec![]);
    gradient(&d, radial, FillMode::Radial, three());
    let mut s = open(&d);
    for (x, want) in [(0.0, 1), (30.0, 1), (60.0, 0), (90.0, 1)] {
        pick(&mut s, pt(x + 5.0, 5.0));
        assert_eq!(s.selected_object_count(), want, "interior at x {x}");
    }
    // Hover follows the press rule: the same objects light up.
    s.pointer_hover(pt(900.0, 900.0), false, false);
    s.pointer_hover(pt(95.0, 5.0), false, false);
}

// --------------------------- copy, split, join, Object to path (30 to 33)

fn zigzag(d: &Document) -> (NodeId, Vec<AnchorId>) {
    let pts = [
        (0.0, 0.0),
        (10.0, 5.0),
        (20.0, 0.0),
        (30.0, 8.0),
        (40.0, 0.0),
    ];
    let anchors: Vec<NewAnchor> = pts
        .iter()
        .enumerate()
        .map(|(i, (x, y))| NewAnchor::corner(AnchorId::new(1, i as u64 + 1), pt(*x, *y)))
        .collect();
    let id = d.create_path(&anchors, false);
    (id, anchors.iter().map(|a| a.id).collect())
}

fn coincident3() -> Vec<GradientStop> {
    vec![
        stop(1, 0.0, rgb(255, 0, 0), 1.0),
        stop(2, 0.5, rgb(0, 255, 0), 0.5),
        stop(3, 0.5, rgb(0, 0, 255), 0.25),
    ]
}

#[test]
fn ac31_split_gives_both_halves_the_stops_and_each_its_own_box() {
    let d = Document::new(1);
    let (p, a) = zigzag(&d);
    gradient(&d, p, FillMode::Linear, coincident3());
    let before = style_of(&d, p);
    d.split_at_anchor(p, a[2], AnchorId::new(2, 1)).unwrap();
    let s = open(&d);
    let all = ids(&d);
    assert_eq!(all.len(), 2);
    for id in &all {
        assert_eq!(style_of(&d, *id), before_with(&before, *id, &d));
    }
    let f = fills(&s);
    assert_eq!(f.len(), 2, "both halves paint a gradient");
    let mut boxes: Vec<(f64, f64)> = f.iter().map(|g| (g.frame.min.x, g.frame.max.x)).collect();
    boxes.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    assert!(
        (boxes[0].0 - 0.0).abs() < 1e-9 && (boxes[0].1 - 20.0).abs() < 1e-9,
        "{boxes:?}"
    );
    assert!(
        (boxes[1].0 - 20.0).abs() < 1e-9 && (boxes[1].1 - 40.0).abs() < 1e-9,
        "{boxes:?}"
    );
    assert_eq!(
        f[0].ramp, f[1].ramp,
        "same stops, same ramp, restarted per half"
    );
}

fn before_with(before: &Style, _id: NodeId, _d: &Document) -> Style {
    before.clone()
}

#[test]
fn ac31_splitting_a_closed_path_changes_nothing_in_its_style() {
    let d = Document::new(1);
    let anchors: Vec<NewAnchor> = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]
        .iter()
        .enumerate()
        .map(|(i, (x, y))| NewAnchor::corner(AnchorId::new(1, i as u64 + 1), pt(*x, *y)))
        .collect();
    let p = d.create_path(&anchors, true);
    gradient(&d, p, FillMode::Radial, coincident3());
    let before = style_of(&d, p);
    d.split_at_anchor(p, anchors[1].id, AnchorId::new(2, 1))
        .unwrap();
    assert_eq!(ids(&d).len(), 1);
    assert_eq!(style_of(&d, p), before);
}

#[test]
fn ac32_join_keeps_the_survivors_style_whichever_order_it_was_asked_in() {
    for swap in [false, true] {
        let d = Document::new(1);
        let a = d.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), pt(10.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 3), pt(10.0, 10.0)),
            ],
            false,
        );
        let b = d.create_path(
            &[
                NewAnchor::corner(AnchorId::new(2, 1), pt(10.0, 10.0)),
                NewAnchor::corner(AnchorId::new(2, 2), pt(0.0, 10.0)),
                NewAnchor::corner(AnchorId::new(2, 3), pt(0.0, 5.0)),
            ],
            false,
        );
        gradient(&d, a, FillMode::Linear, coincident3());
        solid(&d, b, rgb(9, 9, 9));
        let (sa, sb) = (style_of(&d, a), style_of(&d, b));
        let (first, second) = if swap {
            ((b, AnchorId::new(2, 1)), (a, AnchorId::new(1, 3)))
        } else {
            ((a, AnchorId::new(1, 3)), (b, AnchorId::new(2, 1)))
        };
        let (survivor, _) = d
            .join_endpoints(first.0, first.1, second.0, second.1)
            .unwrap();
        assert_eq!(ids(&d).len(), 1);
        let expected = if survivor == a { sa } else { sb };
        assert_eq!(style_of(&d, survivor), expected, "swap {swap}");
        // The ramp restarts on the joined path's own box.
        let s = open(&d);
        let f = fills(&s);
        if survivor == a {
            assert_eq!(f.len(), 1);
            assert!(
                (f[0].frame.min.x - 0.0).abs() < 1e-9 && (f[0].frame.max.x - 10.0).abs() < 1e-9
            );
        } else {
            assert!(f.is_empty(), "the survivor is the solid one");
        }
    }
}

#[test]
fn ac30_a_copy_paints_its_own_gradient_in_its_own_box() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 10.0, 10.0);
    gradient(&d, r, FillMode::Linear, coincident3());
    d.edit_style(&[r], &StyleEdit::StrokeEnabled(false))
        .unwrap();
    let copy = d
        .duplicate_objects(
            &[CopySource {
                id: r,
                anchor_ids: vec![],
            }],
            Vec2::new(30.0, 0.0),
        )
        .unwrap()[0];
    assert_eq!(style_of(&d, copy), style_of(&d, r));
    let s = open(&d);
    let f = fills(&s);
    assert_eq!(f.len(), 2);
    let mut xs: Vec<f64> = f.iter().map(|g| g.frame.min.x).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!((xs[1] - xs[0] - 30.0).abs() < 1e-9, "{xs:?}");
    assert_eq!(f[0].ramp, f[1].ramp);
}

/// Ramp coordinates of a few document points, for comparing before and after.
fn coords(g: &GradientFill, points: &[Point]) -> Vec<[f32; 2]> {
    points.iter().map(|p| g.coordinate(*p)).collect()
}

fn same(a: &[[f32; 2]], b: &[[f32; 2]]) {
    for (x, y) in a.iter().zip(b) {
        assert!(
            (x[0] - y[0]).abs() < 1e-3 && (x[1] - y[1]).abs() < 1e-3,
            "{a:?} vs {b:?}"
        );
    }
}

#[test]
fn ac33_object_to_path_does_not_move_the_ramp_of_a_rectangle_or_ellipse_even_rotated() {
    for (mode, turn) in [
        (FillMode::Linear, 0.0_f64),
        (FillMode::Linear, 30.0),
        (FillMode::Radial, 30.0),
        (FillMode::Linear, 90.0),
        (FillMode::Linear, -45.0),
    ] {
        for ellipse in [false, true] {
            let d = Document::new(1);
            let id = if ellipse {
                d.create_ellipse(EllipseFrame {
                    center: pt(50.0, 40.0),
                    rx: Length::from_mm(30.0),
                    ry: Length::from_mm(12.0),
                })
            } else {
                rect(&d, 20.0, 28.0, 60.0, 24.0)
            };
            gradient(
                &d,
                id,
                mode,
                vec![
                    stop(1, 0.0, rgb(255, 0, 0), 1.0),
                    stop(2, 1.0, rgb(0, 0, 255), 1.0),
                ],
            );
            if turn != 0.0 {
                let rotated = d
                    .object(id)
                    .unwrap()
                    .rotated(pt(50.0, 40.0), Angle::from_radians(turn.to_radians()));
                d.rotate_object(&rotated).unwrap();
            }
            let mut s = open(&d);
            click(&mut s, pt(50.0, 40.0), false);
            assert_eq!(s.selected_object_count(), 1);
            let samples = [
                pt(50.0, 40.0),
                pt(30.0, 40.0),
                pt(70.0, 40.0),
                pt(50.0, 33.0),
                pt(50.0, 47.0),
                pt(40.0, 36.0),
                pt(61.0, 44.0),
            ];
            let before = coords(&fills(&s)[0], &samples);
            s.convert_selected_to_paths();
            let f = fills(&s);
            assert_eq!(f.len(), 1);
            let after = coords(&f[0], &samples);
            let doc = doc_of(&s);
            assert!(
                matches!(doc.object(id).unwrap(), ObjectSnapshot::Path(_)),
                "converted"
            );
            same(&before, &after);
            let _ = (mode, ellipse);
        }
    }
}

#[test]
fn ac33_the_converted_path_keeps_its_stops() {
    let d = Document::new(1);
    let id = rect(&d, 0.0, 0.0, 10.0, 10.0);
    gradient(&d, id, FillMode::Linear, coincident3());
    let mut s = open(&d);
    click(&mut s, pt(5.0, 5.0), false);
    s.convert_selected_to_paths();
    assert_eq!(style_of(&doc_of(&s), id).fill.stops, coincident3());
}

// ------------------------------------------------ file round trip, draw list

#[test]
fn a_saved_and_reopened_document_paints_the_same_gradients() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0);
    let b = d.create_ellipse(EllipseFrame {
        center: pt(50.0, 5.0),
        rx: Length::from_mm(8.0),
        ry: Length::from_mm(4.0),
    });
    gradient(&d, a, FillMode::Linear, coincident3());
    gradient(&d, b, FillMode::Radial, three());
    let rotated = d
        .object(b)
        .unwrap()
        .rotated(pt(50.0, 5.0), Angle::from_radians(0.7));
    d.rotate_object(&rotated).unwrap();
    let s1 = open(&d);
    let s2 = {
        let bytes = s1.pack("0.1.0").unwrap();
        let mut s = Session::open(6, &bytes).unwrap();
        s.set_tool(Tool::Select);
        s
    };
    let (f1, f2) = (fills(&s1), fills(&s2));
    assert_eq!(f1.len(), 2);
    assert_eq!(f1, f2);
    assert_eq!(
        s1.draw_list().triangles.len(),
        s2.draw_list().triangles.len()
    );
}

#[test]
fn no_gradient_is_recorded_for_objects_without_a_gradient_fill() {
    let d = Document::new(1);
    let a = rect(&d, 0.0, 0.0, 10.0, 10.0);
    solid(&d, a, rgb(1, 2, 3));
    let b = rect(&d, 30.0, 0.0, 10.0, 10.0);
    gradient(&d, b, FillMode::Linear, three());
    d.set_fill_mode(
        FillMode::None,
        &[FillModeTarget {
            id: b,
            seed_stops: vec![],
        }],
    )
    .unwrap();
    let s = open(&d);
    assert!(fills(&s).is_empty());
}

#[test]
fn a_gradient_survives_a_drag_move_of_its_object_with_the_box_following() {
    let d = Document::new(1);
    let r = rect(&d, 0.0, 0.0, 10.0, 10.0);
    gradient(&d, r, FillMode::Linear, three());
    let mut s = open(&d);
    click(&mut s, pt(5.0, 5.0), false);
    s.pointer_hover(pt(5.0, 5.0), false, false);
    s.pointer_down(pt(5.0, 5.0), false);
    s.pointer_hover(pt(45.0, 25.0), false, false);
    // Mid-drag the preview paints the gradient at the moved place.
    let mid = fills(&s);
    assert_eq!(mid.len(), 1, "the preview draws the fill once");
    s.pointer_up(pt(45.0, 25.0), false, false);
    let f = fills(&s);
    assert_eq!(f.len(), 1);
    assert!(
        (f[0].frame.min.x - 40.0).abs() < 1e-6 && (f[0].frame.min.y - 20.0).abs() < 1e-6,
        "{:?}",
        f[0].frame
    );
    assert!((f[0].coordinate(pt(40.0, 25.0))[0]).abs() < 1e-4);
    assert!((f[0].coordinate(pt(50.0, 25.0))[0] - 1.0).abs() < 1e-4);
}

// ------------------------------------------------ more reachable edges

/// The same list-order insertion defect as in the ui-core tests, reached from
/// the panel: drag stops until two share a position (this leaves the stored
/// list unsorted), then click the bar on that position.
#[test]
fn ac18_defect_bar_click_on_a_hard_edge_after_dragging_changes_the_ramp() {
    let (mut s, _r) = linear_rect(three());
    // red 0 -> 100, then blue 100 -> 50: list [red@1, green@.5, blue@.5],
    // a hard edge at 50 % behind a stop at 100 % in list order.
    assert_eq!(s.set_stop_text(0, StopField::Position, "100"), Ok(true));
    assert_eq!(s.set_stop_text(2, StopField::Position, "50"), Ok(true));
    let before = fills(&s)[0].ramp.clone();
    assert!(s.add_stop(Some(0.5)));
    let after = &fills(&s)[0].ramp;
    for (a, b) in before.0.iter().zip(after.0.iter()) {
        for ch in 0..4 {
            assert!(
                (i32::from(a[ch]) - i32::from(b[ch])).abs() <= 1,
                "adding a stop changed the ramp: {a:?} -> {b:?}"
            );
        }
    }
}

#[test]
fn a_path_rotated_a_quarter_turn_has_a_top_to_bottom_ramp_and_the_stops_are_kept() {
    let d = Document::new(1);
    let anchors: Vec<NewAnchor> = [(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (0.0, 10.0)]
        .iter()
        .enumerate()
        .map(|(i, (x, y))| NewAnchor::corner(AnchorId::new(1, i as u64 + 1), pt(*x, *y)))
        .collect();
    let p = d.create_path(&anchors, true);
    gradient(&d, p, FillMode::Linear, three());
    let rotated = d.object(p).unwrap().rotated(
        pt(10.0, 5.0),
        Angle::from_radians(std::f64::consts::FRAC_PI_2),
    );
    d.rotate_object(&rotated).unwrap();
    let s = open(&d);
    let f = fills(&s);
    assert_eq!(f.len(), 1);
    let at = |x, y| f[0].coordinate(pt(x, y))[0];
    // The turned path spans x 5..15, y -5..15; one end of its long side is at
    // the top, the other at the bottom, and the ramp is constant across.
    let (top, bottom) = (at(10.0, -5.0), at(10.0, 15.0));
    assert!(
        (top - 0.0).abs() < 1e-3 && (bottom - 1.0).abs() < 1e-3,
        "{top} {bottom}"
    );
    assert!(
        (at(5.0, 5.0) - at(15.0, 5.0)).abs() < 1e-4,
        "constant across"
    );
}

#[test]
fn a_flat_open_path_with_a_gradient_neither_panics_nor_produces_non_finite_coordinates() {
    let d = Document::new(1);
    let p = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(1, 1), pt(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), pt(30.0, 0.0)),
        ],
        false,
    );
    gradient(&d, p, FillMode::Radial, three());
    let s = open(&d);
    let list = s.draw_list();
    for a in list.gradient_attributes(4) {
        assert!(a.iter().all(|v| v.is_finite()), "{a:?}");
    }
    let q = d.create_path(
        &[
            NewAnchor::corner(AnchorId::new(2, 1), pt(5.0, 5.0)),
            NewAnchor::corner(AnchorId::new(2, 2), pt(5.0, 5.0)),
        ],
        false,
    );
    gradient(&d, q, FillMode::Linear, three());
    let s = open(&d);
    for a in s.draw_list().gradient_attributes(4) {
        assert!(a.iter().all(|v| v.is_finite()), "{a:?}");
    }
}
