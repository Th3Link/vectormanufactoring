//! Where the eyedropper (`0017`) meets the path tools (`0031`, `0034`, `0035`): each test pins one
//! rule that neither slice can test alone.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use curvyo_document_core::{
    AnchorId, Document, Length, NewAnchor, NodeId, Point, RectBounds, pack,
};
use curvyo_editor_wasm::{BooleanOutcome, KeyInput, Session, Tool};
use curvyo_ui_core::{BooleanOp, JoinType, PaintTarget, PanelContent, PenTarget};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(document: &Document, x: f64, y: f64, side: f64) -> NodeId {
    document.create_rect(RectBounds {
        origin: pt(x, y),
        width: Length::from_mm(side),
        height: Length::from_mm(side),
    })
}

fn node(n: u64, x: f64, y: f64) -> NewAnchor {
    NewAnchor::corner(AnchorId::new(1, n), pt(x, y))
}

fn session_of(document: &Document, tool: Tool) -> Session {
    let mut session = Session::open(2, &pack(document, "0.1.0").unwrap()).unwrap();
    session.resize_viewport(1200.0, 800.0);
    session.set_tool(tool);
    session
}

fn click(session: &mut Session, at: Point, shift: bool) {
    session.modifiers_changed(shift, false, false);
    session.pointer_hover(at, shift, false);
    session.pointer_down(at, shift);
    session.pointer_up(at, shift, false);
    session.modifiers_changed(false, false, false);
    session.pointer_hover(at, false, false);
}

fn select_all(session: &mut Session, to: Point) {
    let from = pt(-200.0, -200.0);
    session.pointer_hover(from, false, false);
    session.pointer_down(from, false);
    session.pointer_hover(to, false, false);
    session.pointer_up(to, false, false);
}

fn escape(session: &mut Session) {
    session.key_down(KeyInput {
        key: "Escape",
        ..KeyInput::default()
    });
}

/// Two squares and a third one overlapping the first, for Boolean and Combine.
fn squares() -> Document {
    let document = Document::new(1);
    let _ = rect(&document, 0.0, 0.0, 40.0);
    let _ = rect(&document, 20.0, 20.0, 40.0);
    let _ = rect(&document, 120.0, 0.0, 40.0);
    document
}

/// `0017` criterion 26 with the rail commands of `0016`, `0035`: a rail command ends
/// picking, so the Pick button clears and a later press does not paint the result.
#[test]
fn rail_commands_end_picking() {
    let mut session = session_of(&squares(), Tool::Select);
    select_all(&mut session, pt(300.0, 100.0));
    session.begin_colour_pick(PaintTarget::Fill);
    assert!(matches!(
        session.apply_boolean(BooleanOp::Union),
        BooleanOutcome::Applied { .. }
    ));
    assert_eq!(session.colour_pick_target(), None, "Boolean");

    let mut session = session_of(&squares(), Tool::Select);
    select_all(&mut session, pt(300.0, 100.0));
    session.begin_colour_pick(PaintTarget::Fill);
    let _ = session.apply_combine();
    assert_eq!(session.colour_pick_target(), None, "Combine");

    session.begin_colour_pick(PaintTarget::Stroke);
    let _ = session.apply_break_apart();
    assert_eq!(session.colour_pick_target(), None, "Break apart");
}

/// The Node bar's Close path ends picking as well.
#[test]
fn close_path_ends_picking() {
    let document = Document::new(1);
    let _ = document.create_path(
        &[
            node(1, 0.0, 0.0),
            node(2, 100.0, 0.0),
            node(3, 100.0, 100.0),
        ],
        false,
    );
    let mut session = session_of(&document, Tool::Select);
    select_all(&mut session, pt(300.0, 300.0));
    session.set_tool(Tool::Node);
    session.begin_colour_pick(PaintTarget::Stroke);
    let outcome = session.close_paths(JoinType::Sharp);
    assert_eq!(outcome.closed, 1);
    assert_eq!(session.colour_pick_target(), None);
}

/// While picking, the Pen has no target (no cue, no chip) and the Node tool's segment hover
/// band does not show.
#[test]
fn picking_hides_the_pen_cue_and_the_segment_band() {
    let document = Document::new(1);
    let _ = document.create_path(&[node(1, 0.0, 0.0), node(2, 100.0, 0.0)], false);
    let mut session = session_of(&document, Tool::Pen);
    session.pointer_hover(pt(100.0, 0.0), false, false);
    assert!(matches!(session.pen_target(), Some(PenTarget::Continue(_))));
    session.begin_colour_pick(PaintTarget::Stroke);
    session.pointer_hover(pt(100.0, 0.0), false, false);
    assert_eq!(session.pen_target(), None);
    assert_eq!(session.pen_cue_away(), (0.0, 0.0));
    assert!(!session.pen_join_style_differs());
    assert_eq!(session.pen_close_joins(), None);

    // The band: the same pointer position draws less ink while picking.
    let mut session = session_of(&document, Tool::Node);
    select_all(&mut session, pt(300.0, 300.0));
    session.pointer_hover(pt(50.0, 0.0), false, false);
    let with_band = session.draw_list().triangle_count();
    session.begin_colour_pick(PaintTarget::Stroke);
    session.pointer_hover(pt(50.0, 0.0), false, false);
    assert!(session.draw_list().triangle_count() < with_band);
}

/// `0034` criterion 25 with `0017` criterion 60: with picking on and a Pen continuation in
/// progress, the first Escape ends picking only; the second discards the continuation.
#[test]
fn escape_ends_picking_before_it_discards_a_continuation() {
    let document = Document::new(1);
    let _ = document.create_path(&[node(1, 0.0, 0.0), node(2, 100.0, 0.0)], false);
    let mut session = session_of(&document, Tool::Pen);
    click(&mut session, pt(100.0, 0.0), false);
    click(&mut session, pt(200.0, 50.0), false);
    assert_eq!(session.pen_in_progress().unwrap().len(), 2);
    session.begin_colour_pick(PaintTarget::Stroke);
    escape(&mut session);
    assert_eq!(session.colour_pick_target(), None);
    assert_eq!(session.pen_in_progress().unwrap().len(), 2, "kept");
    escape(&mut session);
    assert!(session.pen_in_progress().is_none(), "discarded");
}

/// The eyedropper cursor wins over the Pen's.
#[test]
fn the_eyedropper_cursor_wins_over_the_pen() {
    let mut session = session_of(&squares(), Tool::Select);
    click(&mut session, pt(0.0, 20.0), false);
    session.begin_colour_pick(PaintTarget::Fill);
    session.pointer_hover(pt(80.0, 20.0), false, false);
    assert_eq!(session.cursor_hint(), "eyedropper");

    session.set_tool(Tool::Pen);
    session.begin_colour_pick(PaintTarget::Fill);
    assert_eq!(session.cursor_hint(), "eyedropper");
}

/// `0034` criterion 2: a Pen continuation counts as an unfinished path, so the panel is empty.
#[test]
fn a_pen_continuation_empties_the_panel() {
    let document = Document::new(1);
    let _ = document.create_path(&[node(1, 0.0, 0.0), node(2, 100.0, 0.0)], false);
    let mut session = session_of(&document, Tool::Pen);
    assert_eq!(session.panel_content(), PanelContent::Document);
    click(&mut session, pt(100.0, 0.0), false);
    assert!(session.pen_in_progress().is_some());
    assert_eq!(session.panel_content(), PanelContent::Empty);
    // Also with a selection behind the Pen.
    session.finish_pen();
    session.set_tool(Tool::Select);
    click(&mut session, pt(0.0, 0.0), false);
    assert_eq!(session.selected_object_count(), 1);
    session.set_tool(Tool::Pen);
    click(&mut session, pt(100.0, 0.0), false);
    assert!(session.pen_in_progress().is_some());
    assert_eq!(session.panel_content(), PanelContent::Empty);
}
