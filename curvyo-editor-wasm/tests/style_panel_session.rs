//! `Session`-level checks of the Style panel (`specs/0007-stroke-and-fill-styling`
//! PR 3, criteria 2, 5, 13, 24, 36, 37): the panel edits what the tool's scope
//! says, a drag previews without writing and commits once on release, and the
//! preview is drawn in place of the stored style.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    Color, Document, FillMode, Length, LineCap, ObjectSnapshot, Opacity, Point, Style, unpack,
};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_render_core::DrawList;
use curvyo_ui_core::{BarValue, DashChoice, StopField, StopsPanel, StyleEntryError, StyleField};

/// A session with `n` 10 mm rectangles in a row, the last one selected.
fn session_with_rectangles(n: u32) -> Session {
    let mut session = Session::new(1);
    for k in 0..n {
        session.set_tool(Tool::Rectangle);
        let x = f64::from(k) * 30.0;
        session.pointer_down(Point::new(x, 0.0), false);
        session.pointer_up(Point::new(x + 10.0, 10.0), false, false);
    }
    session
}

/// Selects the first `n` rectangles of `session_with_rectangles`: a click on
/// the edge of the first, Shift-clicks on the others.
fn select_first(session: &mut Session, n: u32) {
    session.set_tool(Tool::Select);
    for k in 0..n {
        let edge = Point::new(f64::from(k) * 30.0, 5.0);
        session.pointer_hover(edge, k > 0, false);
        session.pointer_down(edge, k > 0);
        session.pointer_up(edge, k > 0, false);
    }
}

/// The styles of every object as the document holds them now.
fn stored_styles(session: &Session) -> Vec<Style> {
    let bytes = session.pack("0.1.0").unwrap();
    let document: Document = unpack(9, &bytes).unwrap();
    document
        .object_ids()
        .into_iter()
        .filter_map(|id| document.object(id))
        .map(|object| match object {
            ObjectSnapshot::Path(p) => p.style,
            ObjectSnapshot::Primitive(p) => p.style,
        })
        .collect()
}

fn loro_of(session: &Session) -> loro::LoroDoc {
    let document = unpack(9, &session.pack("0.1.0").unwrap()).unwrap();
    let loro = loro::LoroDoc::new();
    loro.import(&document.export_loro_snapshot().unwrap())
        .unwrap();
    loro
}

/// How many commits the session's document holds, counted from its Loro
/// snapshot (the way `acceptance_unified_editing.rs` counts one interaction).
/// Consecutive commits of one peer with the same label can merge into one
/// change, so a count of one is only proof where the last change before the
/// gesture has another label; `op_count` is the check that cannot merge away.
fn change_count(session: &Session) -> usize {
    loro_of(session).len_changes()
}

/// How many operations the document has recorded: a gesture that wrote once per
/// tick would record more than one that wrote once on release.
fn op_count(session: &Session) -> i32 {
    loro_of(session).oplog_vv().values().copied().sum()
}

fn has_alpha(list: &DrawList, alpha: u8) -> bool {
    list.triangles.iter().any(|v| v.color.a == alpha)
}

fn percent(n: u32) -> Opacity {
    Opacity::new(f64::from(n) / 100.0).unwrap()
}

#[test]
fn a_drawn_rectangle_is_selected_and_editable() {
    let session = session_with_rectangles(1);
    let state = session.style_panel_state();
    assert!(state.enabled);
    assert_eq!(state.subject, "Rectangle");
}

#[test]
fn the_pen_and_an_empty_selection_disable_the_panel() {
    let mut session = session_with_rectangles(1);
    session.set_tool(Tool::Pen);
    let state = session.style_panel_state();
    assert!(!state.enabled);
    assert_eq!(state.subject, "Pen: finish the path to style it");
    assert_eq!(
        session.set_style_text(StyleField::StrokeWidth, "2"),
        Ok(false)
    );
    assert_eq!(
        stored_styles(&session)[0].stroke.width,
        Length::from_mm(0.25)
    );
}

#[test]
fn a_typed_value_is_one_commit_for_the_whole_selection() {
    let mut session = session_with_rectangles(3);
    select_first(&mut session, 3);
    assert_eq!(session.style_panel_state().subject, "3 rectangles");
    assert_eq!(
        session.set_style_text(StyleField::StrokeWidth, "2,5"),
        Ok(true)
    );
    for style in stored_styles(&session) {
        assert_eq!(style.stroke.width, Length::from_mm(2.5));
    }
    assert_eq!(
        session.style_panel_state().stroke.width,
        BarValue::Uniform(Length::from_mm(2.5))
    );
}

#[test]
fn an_invalid_value_writes_nothing_and_says_why() {
    let mut session = session_with_rectangles(1);
    assert_eq!(
        session.set_style_text(StyleField::StrokeColor, "#12345678"),
        Err(StyleEntryError::HexEightDigits)
    );
    assert_eq!(
        session.set_style_text(StyleField::FillOpacity, "101"),
        Err(StyleEntryError::Percent)
    );
    assert_eq!(stored_styles(&session)[0], Style::default());
}

#[test]
fn a_style_change_keeps_the_primitive_a_primitive() {
    let mut session = session_with_rectangles(1);
    session.set_stroke_cap(LineCap::Round);
    session.set_fill_mode(FillMode::Solid);
    let bytes = session.pack("0.1.0").unwrap();
    let document = unpack(9, &bytes).unwrap();
    let id = document.object_ids()[0];
    assert!(document.primitive(id).is_some(), "criterion 2");
}

#[test]
fn the_stroke_switch_dash_join_and_fill_row_commit_discretely() {
    let mut session = session_with_rectangles(1);
    session.set_stroke_paint(false);
    assert!(session.style_panel_state().stroke.all_off);
    assert!(!stored_styles(&session)[0].stroke.enabled);
    session.set_stroke_dash(DashChoice::Dash);
    session.set_stroke_dash(DashChoice::Custom);
    assert_eq!(
        stored_styles(&session)[0].stroke.dash.as_slice(),
        &[6.0, 4.0],
        "Custom is not a choice and writes nothing"
    );
    session.set_fill_mode(FillMode::Solid);
    assert_eq!(
        session.style_panel_state().fill.mode,
        BarValue::Uniform(FillMode::Solid)
    );
    session.set_fill_mode(FillMode::None);
    assert_eq!(
        session.style_panel_state().fill.mode,
        BarValue::Uniform(FillMode::None)
    );
}

#[test]
fn a_drag_previews_in_the_draw_list_and_writes_once_on_release() {
    let mut session = session_with_rectangles(1);
    let before = session.draw_list();
    assert!(!has_alpha(&before, 102));

    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    session.preview_style_opacity(StyleField::StrokeOpacity, 60.0);
    assert!(has_alpha(&session.draw_list(), 153), "drawn at 60%");
    assert_eq!(
        stored_styles(&session)[0].stroke.opacity,
        Opacity::OPAQUE,
        "nothing stored while dragging"
    );
    assert_eq!(
        session.style_panel_state().stroke.opacity,
        BarValue::Uniform(percent(60)),
        "the field follows the drag"
    );

    session.commit_style_preview();
    assert_eq!(stored_styles(&session)[0].stroke.opacity, percent(60));
    assert!(has_alpha(&session.draw_list(), 153));
}

#[test]
fn escape_during_a_drag_reverts_and_the_release_writes_nothing() {
    let mut session = session_with_rectangles(1);
    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    session.cancel_style_preview();
    session.preview_style_opacity(StyleField::StrokeOpacity, 30.0);
    session.commit_style_preview();
    assert_eq!(stored_styles(&session)[0], Style::default());
    assert!(!has_alpha(&session.draw_list(), 77));
}

#[test]
fn a_pending_drag_commits_to_the_objects_it_started_on() {
    let mut session = session_with_rectangles(2);
    // The second rectangle is selected; start a drag on it, then the tool
    // changes before the release.
    session.preview_style_opacity(StyleField::StrokeOpacity, 50.0);
    session.set_tool(Tool::Select);
    let styles = stored_styles(&session);
    assert_eq!(styles[0].stroke.opacity, Opacity::OPAQUE);
    assert_eq!(styles[1].stroke.opacity, percent(50));
}

#[test]
fn the_node_tool_edits_the_selected_path() {
    let mut session = Session::new(1);
    session.set_tool(Tool::Pen);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false, false);
    session.pointer_down(Point::new(50.0, 0.0), false);
    session.pointer_up(Point::new(50.0, 0.0), false, false);
    session.finish_pen();
    session.set_tool(Tool::Node);
    session.pointer_down(Point::new(0.0, 0.0), false);
    session.pointer_up(Point::new(0.0, 0.0), false, false);
    assert_eq!(session.style_panel_state().subject, "Path");
    assert_eq!(
        session.set_style_text(StyleField::StrokeColor, "#F80"),
        Ok(true)
    );
    let style = &stored_styles(&session)[0];
    assert_eq!((style.stroke.color.r, style.stroke.color.g), (0xFF, 0x88));
}

#[test]
fn a_panel_toggle_does_not_move_the_document_on_screen() {
    let mut session = session_with_rectangles(1);
    session.resize_viewport(1000.0, 600.0);
    let corner = session.view().screen_to_document(0.0, 0.0);
    session.keep_view_origin_for_width_change(-280.0);
    session.resize_viewport(720.0, 600.0);
    assert_eq!(session.view().screen_to_document(0.0, 0.0), corner);
    session.keep_view_origin_for_width_change(280.0);
    session.resize_viewport(1000.0, 600.0);
    assert_eq!(session.view().screen_to_document(0.0, 0.0), corner);
}

#[test]
fn a_drag_of_many_ticks_records_what_one_typed_value_records() {
    let mut dragged = session_with_rectangles(1);
    let mut typed = session_with_rectangles(1);
    let (n_dragged, n_typed) = (op_count(&dragged), op_count(&typed));
    let changes = change_count(&dragged);
    for percent in [90.0, 80.0, 70.0, 60.0, 50.0] {
        dragged.preview_style_opacity(StyleField::StrokeOpacity, percent);
    }
    assert_eq!(op_count(&dragged), n_dragged, "ticks write nothing");
    dragged.commit_style_preview();
    typed
        .set_style_text(StyleField::StrokeOpacity, "50")
        .unwrap();
    let written = op_count(&dragged) - n_dragged;
    assert!(written > 0);
    assert_eq!(
        written,
        op_count(&typed) - n_typed,
        "one write of one value"
    );
    assert_eq!(change_count(&dragged), changes + 1);
    dragged.commit_style_preview();
    assert_eq!(
        op_count(&dragged) - n_dragged,
        written,
        "a second release is a no-op"
    );
}

#[test]
fn escape_then_the_release_records_nothing() {
    let mut session = session_with_rectangles(1);
    let before = op_count(&session);
    session.preview_style_opacity(StyleField::StrokeOpacity, 40.0);
    session.cancel_style_preview();
    session.preview_style_opacity(StyleField::StrokeOpacity, 30.0);
    session.commit_style_preview();
    assert_eq!(op_count(&session), before);
}

#[test]
fn an_edit_over_three_objects_is_one_commit_of_three_writes() {
    let written = |n: u32, drag: bool| {
        let mut session = session_with_rectangles(n);
        select_first(&mut session, n);
        let (ops, changes) = (op_count(&session), change_count(&session));
        if drag {
            session.preview_style_color(StyleField::StrokeColor, Color { r: 1, g: 2, b: 3 });
            session.preview_style_color(StyleField::StrokeColor, Color { r: 4, g: 5, b: 6 });
            session.commit_style_preview();
        } else {
            session
                .set_style_text(StyleField::StrokeColor, "#040506")
                .unwrap();
        }
        assert_eq!(change_count(&session), changes + 1, "one commit");
        for style in stored_styles(&session) {
            assert_eq!(style.stroke.color, Color { r: 4, g: 5, b: 6 });
        }
        op_count(&session) - ops
    };
    let one = written(1, false);
    assert!(one > 0);
    assert_eq!(written(3, false), 3 * one, "typed");
    assert_eq!(written(3, true), 3 * one, "dragged");
}

// ---- gradients (criteria 16 to 22, 34, 35) -------------------------------------

fn sorted_first(stops: &[curvyo_document_core::GradientStop]) -> curvyo_document_core::StopId {
    curvyo_document_core::sorted_stops(stops)[0].id
}

fn stops_of(session: &Session) -> Vec<curvyo_document_core::GradientStop> {
    stored_styles(session)[0].fill.stops.clone()
}

fn editor_rows(session: &Session) -> usize {
    match session.style_panel_state().fill.stops {
        StopsPanel::Editor(view) => view.rows.len(),
        _ => usize::MAX,
    }
}

/// Criterion 17: Linear on a solid red fill makes red to white, with fresh ids,
/// in one commit, and keeps the solid colour.
#[test]
fn switching_to_linear_seeds_red_to_white_in_one_commit() {
    let mut session = session_with_rectangles(1);
    session
        .set_style_text(StyleField::FillColor, "#F00")
        .unwrap();
    session.set_fill_mode(FillMode::Solid);
    // A commit of another label first: same-label commits can merge into one.
    session.set_stroke_cap(LineCap::Round);
    let (changes, ops) = (change_count(&session), op_count(&session));
    session.set_fill_mode(FillMode::Linear);
    assert_eq!(change_count(&session), changes + 1);
    assert!(op_count(&session) > ops);
    let style = &stored_styles(&session)[0];
    assert_eq!(style.fill.kind, curvyo_document_core::FillKind::Linear);
    let stops = stops_of(&session);
    assert_eq!(stops.len(), 2);
    assert_eq!(
        (stops[0].color.r, stops[0].color.g, stops[0].position.get()),
        (255, 0, 0.0)
    );
    assert_eq!((stops[1].color.g, stops[1].position.get()), (255, 1.0));
    assert_ne!(stops[0].id, stops[1].id);
    assert_eq!(style.fill.color.r, 255, "the solid colour is kept");
    // Back to Solid and to Radial: neither loses the stops (criterion 13).
    session.set_fill_mode(FillMode::Solid);
    session.set_fill_mode(FillMode::Radial);
    assert_eq!(stops_of(&session), stops);
    assert_eq!(
        session.style_panel_state().fill.mode,
        BarValue::Uniform(FillMode::Radial)
    );
}

/// The drawn gradient: one fill with a ramp and the object's own box.
#[test]
fn a_gradient_fill_reaches_the_draw_list_with_its_box() {
    let mut session = session_with_rectangles(1);
    assert_eq!(session.draw_list().gradients().len(), 0);
    session.set_fill_mode(FillMode::Linear);
    let list = session.draw_list();
    assert_eq!(list.gradients().len(), 1);
    let fill = &list.gradients()[0];
    assert!(!fill.radial);
    assert!((fill.frame.max.x - fill.frame.min.x - 10.0).abs() < 1e-9);
    session.set_fill_mode(FillMode::Radial);
    assert!(session.draw_list().gradients()[0].radial);
    session.set_fill_mode(FillMode::None);
    assert_eq!(session.draw_list().gradients().len(), 0);
}

/// Criterion 17 again, across a selection: each object takes its own colour.
#[test]
fn each_object_of_a_selection_seeds_from_its_own_colour() {
    let mut session = session_with_rectangles(2);
    select_first(&mut session, 1);
    session
        .set_style_text(StyleField::FillColor, "#0000FF")
        .unwrap();
    select_first(&mut session, 2);
    session.set_fill_mode(FillMode::Linear);
    let styles = stored_styles(&session);
    let firsts: Vec<_> = styles
        .iter()
        .map(|s| {
            let sorted = curvyo_document_core::sorted_stops(&s.fill.stops);
            (sorted[0].color.r, sorted[0].color.b)
        })
        .collect();
    assert!(
        firsts.contains(&(0, 255)) && firsts.contains(&(0, 0)),
        "{firsts:?}"
    );
}

#[test]
fn stop_text_edits_one_value_of_one_stop_and_the_selection_follows_the_stop() {
    let mut session = session_with_rectangles(1);
    session.set_fill_mode(FillMode::Linear);
    assert!(session.add_stop(None));
    let before = stops_of(&session);
    session.select_stop(0);
    let changes = change_count(&session);
    // Move the first stop past the middle one: the list is re-sorted, and the
    // stop stays selected.
    assert_eq!(
        session.set_stop_text(0, StopField::Position, "90"),
        Ok(true)
    );
    assert_eq!(change_count(&session), changes + 1);
    let after = stops_of(&session);
    let find = |stops: &[curvyo_document_core::GradientStop], id| {
        *stops.iter().find(|s| s.id == id).unwrap()
    };
    let moved = find(&before, sorted_first(&before));
    assert!((find(&after, moved.id).position.get() - 0.9).abs() < 1e-12);
    for stop in before.iter().filter(|s| s.id != moved.id) {
        assert_eq!(find(&after, stop.id), *stop, "every other stop unchanged");
    }
    assert_eq!(session.style_panel_view().selected_stop, 1);
    // A refused value writes nothing.
    assert_eq!(
        session.set_stop_text(0, StopField::Color, "#12345678"),
        Err(StyleEntryError::HexEightDigits)
    );
    assert_eq!(stops_of(&session), after);
}

/// Criterion 18, 19: add inserts in position order with the ramp's colour, up
/// to 16; remove stops at two.
#[test]
fn add_and_remove_follow_the_stop_rules() {
    let mut session = session_with_rectangles(1);
    session.set_fill_mode(FillMode::Linear);
    assert!(session.add_stop(None));
    let stops = stops_of(&session);
    assert_eq!(stops.len(), 3);
    let added = stops.iter().find(|s| s.position.get() == 0.5).unwrap();
    assert_eq!(
        (added.color.r, added.color.g),
        (128, 128),
        "black to white, so grey at 0.5"
    );
    assert!(session.add_stop(Some(0.25)));
    assert_eq!(editor_rows(&session), 4);
    for _ in 4..16 {
        assert!(session.add_stop(None));
    }
    assert_eq!(editor_rows(&session), 16);
    assert!(!session.add_stop(None), "the 17th is refused");
    assert_eq!(stops_of(&session).len(), 16);
    for _ in 2..16 {
        assert!(session.remove_stop(0));
    }
    assert_eq!(editor_rows(&session), 2);
    assert!(!session.remove_stop(0), "a gradient keeps two");
    assert_eq!(stops_of(&session).len(), 2);
}

#[test]
fn a_stop_drag_previews_in_the_draw_list_and_commits_once() {
    let mut session = session_with_rectangles(1);
    session.set_fill_mode(FillMode::Linear);
    let ops = op_count(&session);
    session.preview_stop(0, StopField::Position, 30.0);
    session.preview_stop(1, StopField::Position, 40.0); // the rank is the first tick's
    session.preview_stop(0, StopField::Position, 50.0);
    assert_eq!(op_count(&session), ops, "nothing written while dragging");
    assert_eq!(stops_of(&session)[0].position.get(), 0.0);
    match session.style_panel_state().fill.stops {
        StopsPanel::Editor(view) => assert_eq!(
            view.rows[0].position,
            BarValue::Uniform(curvyo_document_core::StopPosition::new(0.5).unwrap()),
            "the row follows the drag"
        ),
        other => panic!("{other:?}"),
    }
    session.commit_style_preview();
    assert_eq!(stops_of(&session)[0].position.get(), 0.5);
    assert_eq!(stops_of(&session)[1].position.get(), 1.0);
    // Escape reverts.
    session.preview_stop(0, StopField::Position, 90.0);
    session.cancel_style_preview();
    session.commit_style_preview();
    assert_eq!(stops_of(&session)[0].position.get(), 0.5);
}

/// Criterion 34: several selected gradients edit by rank in one commit.
#[test]
fn a_stop_edit_over_a_selection_goes_to_the_stop_of_each_rank() {
    let mut session = session_with_rectangles(2);
    select_first(&mut session, 2);
    session.set_fill_mode(FillMode::Linear);
    let (changes, ops) = (change_count(&session), op_count(&session));
    assert_eq!(
        session.set_stop_text(1, StopField::Color, "#00FF00"),
        Ok(true)
    );
    assert_eq!(change_count(&session), changes + 1);
    assert!(op_count(&session) > ops);
    for style in stored_styles(&session) {
        let sorted = curvyo_document_core::sorted_stops(&style.fill.stops);
        assert_eq!((sorted[1].color.r, sorted[1].color.g), (0, 255));
        assert_eq!(sorted[0].color.g, 0, "rank 0 untouched");
    }
    // Add and Remove are not offered for several objects.
    assert!(!session.add_stop(None));
    assert!(!session.remove_stop(0));
}

/// Criterion 35, through the session: a document whose gradient holds no stops
/// (as a merge can leave it) opens, shows the empty editor, paints nothing, is
/// not clickable, and Add creates the first stop at 50 %.
#[test]
fn a_gradient_without_stops_is_a_state_and_add_creates_the_first_stop() {
    let document = Document::new(1);
    let id = document.create_rect(curvyo_document_core::RectBounds {
        origin: Point::new(0.0, 0.0),
        width: Length::from_mm(10.0),
        height: Length::from_mm(10.0),
    });
    document
        .set_fill_mode(
            FillMode::Linear,
            &[curvyo_document_core::FillModeTarget {
                id,
                seed_stops: Vec::new(),
            }],
        )
        .unwrap();
    let bytes = curvyo_document_core::pack(&document, "0.1.0").unwrap();
    let mut session = Session::open(3, &bytes).unwrap();
    assert_eq!(session.draw_list().gradients().len(), 0);
    session.set_tool(Tool::Select);
    session.pointer_down(Point::new(0.0, 5.0), false);
    session.pointer_up(Point::new(0.0, 5.0), false, false);
    assert_eq!(editor_rows(&session), 0);
    assert!(session.add_stop(None));
    let stops = stops_of(&session);
    assert_eq!(stops.len(), 1);
    assert_eq!(stops[0].position.get(), 0.5);
}
