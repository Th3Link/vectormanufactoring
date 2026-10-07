//! `Session`'s shape-creation glue (`specs/0003-primitive-shapes/
//! specification.md`, acceptance criteria 1, 2, 7, 8, 10-12; `specs/unified-
//! object-editing/`, criteria 25 to 30): dispatching pointer events to
//! whichever of the three creation tools is active, the polygon/star settings
//! for the next shape, and the live preview and readout of a create-drag. The
//! shape tools only create: a press never selects, moves or handle-drags an
//! existing object, and a committed create-drag hands over to the Select tool
//! with the new shape selected. A child module of `session`, so it shares
//! `Session`'s privacy boundary and its methods join the same type's
//! `impl Session`.

use vecmanf_document_core::{InnerRatio, Point, PointCount, Shape};
use vecmanf_ui_core::{CreateOutcome, CreatePreview, Modifiers, PolyStarMode, format_degrees};

use super::{Session, Tool};

/// The live, uncommitted numeric readout for a create-drag or a Select-tool
/// edit (`specification.md`'s "Live creation feedback" UX notes) — the text
/// to show, and the document-space point it is anchored near.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveReadout {
    /// The formatted text (e.g. `"20.0 × 10.0 mm"`).
    pub text: String,
    /// Where to anchor it, in document space — the live pointer position.
    pub anchor: Point,
}

impl Session {
    /// Acceptance criteria 1, 2, 7, 8, 11, 12 and 25: a press in a creation
    /// tool always starts a new shape, on empty canvas, on an existing outline,
    /// inside a selected box or where a handle would be. It never selects,
    /// toggles or moves anything and leaves the selection as it is (26).
    pub(super) fn shape_pointer_down(&mut self, point: Point) {
        match self.tool {
            Tool::Rectangle => self.rectangle.pointer_down(point),
            Tool::Ellipse => self.ellipse.pointer_down(point),
            Tool::PolygonStar => self.poly_star.pointer_down(point),
            Tool::Select | Tool::Pen | Tool::Node => {}
        }
    }

    /// Feeds whatever create-drag is in flight for the live preview; writes
    /// nothing to the document.
    pub(super) fn shape_pointer_move(&mut self, point: Point, modifiers: Modifiers) {
        match self.tool {
            Tool::Rectangle => self.rectangle.pointer_move(point, modifiers),
            Tool::Ellipse => self.ellipse.pointer_move(point, modifiers),
            Tool::PolygonStar => self.poly_star.pointer_move(point, modifiers),
            Tool::Select | Tool::Pen | Tool::Node => {}
        }
    }

    /// Commits whatever create-drag `shape_pointer_down` began. A committed
    /// create-drag makes the Select tool active in the same step with the new
    /// shape as the only selected object, so its full handle set shows
    /// (criterion 28; `0003` criterion 1, "it becomes the selected object").
    /// Every other outcome, a press without movement included, leaves the tool
    /// and the selection alone (criterion 26).
    pub(super) fn shape_pointer_up(&mut self, point: Point, modifiers: Modifiers) {
        let outcome = match self.tool {
            Tool::Rectangle => self.rectangle.pointer_up(&self.document, point, modifiers),
            Tool::Ellipse => self.ellipse.pointer_up(&self.document, point, modifiers),
            Tool::PolygonStar => self.poly_star.pointer_up(&self.document, point, modifiers),
            Tool::Select | Tool::Pen | Tool::Node => CreateOutcome::NoOp,
        };
        if let CreateOutcome::Created(id) = outcome {
            self.selection.select_single(id);
            self.set_tool(Tool::Select);
        }
    }

    /// Cancels whichever creation tool's in-progress drag, writing nothing.
    /// Returns whether there was one.
    pub(super) fn shape_escape(&mut self) -> bool {
        match self.tool {
            Tool::Rectangle => self.rectangle.escape(),
            Tool::Ellipse => self.ellipse.escape(),
            Tool::PolygonStar => self.poly_star.escape(),
            Tool::Select | Tool::Pen | Tool::Node => false,
        }
    }

    /// The polygon/star bar's mode for the next shape.
    #[must_use]
    pub fn poly_star_mode(&self) -> PolyStarMode {
        self.poly_star.mode()
    }

    /// The polygon/star bar's point count for the next shape.
    #[must_use]
    pub fn poly_star_point_count(&self) -> PointCount {
        self.poly_star.point_count()
    }

    /// The polygon/star bar's ratio for the next star.
    #[must_use]
    pub fn poly_star_ratio(&self) -> InnerRatio {
        self.poly_star.ratio()
    }

    /// The mode toggle (acceptance criteria 11 vs. 12): only affects shapes
    /// drawn after the switch.
    pub fn set_poly_star_mode(&mut self, mode: PolyStarMode) {
        self.poly_star.set_mode(mode);
    }

    /// The point-count setting for the next shape (acceptance criterion 10):
    /// it persists across shapes and never changes a selected shape; that is
    /// the Select bar's "Points" (`unified-object-editing` criteria 21, 29).
    pub fn set_poly_star_point_count(&mut self, count: PointCount) {
        self.poly_star.set_point_count(count);
    }

    /// The ratio setting for the next star; it never changes a selected star.
    pub fn set_poly_star_ratio(&mut self, ratio: InnerRatio) {
        self.poly_star.set_ratio(ratio);
    }

    /// The active creation tool's live, uncommitted preview — `None` outside a
    /// creation tool, or when it has nothing to show (idle, or a
    /// still-degenerate create-drag).
    pub(super) fn live_preview(&self) -> Option<CreatePreview> {
        match self.tool {
            Tool::Rectangle => self.rectangle.live_shape(),
            Tool::Ellipse => self.ellipse.live_shape(),
            Tool::PolygonStar => self.poly_star.live_shape(),
            Tool::Select | Tool::Pen | Tool::Node => None,
        }
    }

    /// The numeric readout for an in-progress create-drag
    /// (`specification.md`'s "Live creation feedback": "W × H" for a
    /// rectangle, "rx × ry" for an ellipse, the outer radius — and, for a
    /// star, its fixed ratio — for a polygon/star), or a Select-tool edit's
    /// own. `None` otherwise.
    #[must_use]
    pub fn live_readout(&self) -> Option<LiveReadout> {
        if self.tool == Tool::Select {
            return self.select_live_readout();
        }
        let CreatePreview { shape, anchor, .. } = self.live_preview()?;
        let text = match shape {
            Shape::Rect { bounds, .. } => {
                format!(
                    "{:.1} × {:.1} mm",
                    bounds.width.as_mm(),
                    bounds.height.as_mm()
                )
            }
            Shape::Ellipse { frame } => {
                format!("{:.1} × {:.1} mm", frame.rx.as_mm(), frame.ry.as_mm())
            }
            Shape::Polygon { frame, .. } => format!(
                "r {:.1} mm, {}",
                frame.radius.as_mm(),
                format_degrees(frame.angle.normalized().as_radians().to_degrees())
            ),
            Shape::Star {
                frame, inner_ratio, ..
            } => format!(
                "r {:.1} mm, ratio {:.2}, {}",
                frame.radius.as_mm(),
                inner_ratio.get(),
                format_degrees(frame.angle.normalized().as_radians().to_degrees())
            ),
        };
        Some(LiveReadout { text, anchor })
    }
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::{Point, Shape};

    use super::super::{Session, Tool};

    /// The first primitive of the document.
    fn first_shape(session: &Session) -> Shape {
        let id = session.document.object_ids()[0];
        session.document.primitive(id).expect("a primitive").shape
    }

    /// AC1: drawing a rectangle with the rectangle tool, from an empty
    /// canvas, with no UI setup beyond switching tools.
    #[test]
    fn ac1_rectangle_tool_creates_a_rectangle() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false, false);
        assert_eq!(session.document.object_ids().len(), 1);
        assert!(matches!(first_shape(&session), Shape::Rect { .. }));
    }

    /// AC7: same for the ellipse tool.
    #[test]
    fn ac7_ellipse_tool_creates_an_ellipse() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Ellipse);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false, false);
        assert!(matches!(first_shape(&session), Shape::Ellipse { .. }));
    }

    /// AC11: same for the polygon/star tool, in polygon mode by default.
    #[test]
    fn ac11_polygon_star_tool_creates_a_polygon_by_default() {
        let mut session = Session::new(1);
        session.set_tool(Tool::PolygonStar);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        assert!(matches!(first_shape(&session), Shape::Polygon { .. }));
    }

    /// Criterion 28: a committed create-drag hands over to the Select tool
    /// with the new shape as the only selected object.
    #[test]
    fn a_create_drag_makes_the_select_tool_active_with_the_new_shape_selected() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(40.0, 30.0), false, false);
        assert_eq!(session.tool(), Tool::Select);
        assert_eq!(session.selection.ids(), &[session.document.object_ids()[0]]);
    }

    /// Criterion 26: a press and release without moving creates nothing,
    /// keeps the tool, and leaves the selection as it was (empty: choosing a
    /// creation tool clears it), even on another object's outline.
    #[test]
    fn a_press_without_movement_creates_nothing_and_keeps_the_selection() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(40.0, 30.0), false, false);
        session.set_tool(Tool::Ellipse);
        assert_eq!(session.selection.ids(), &[]);
        for at in [Point::new(200.0, 200.0), Point::new(0.0, 15.0)] {
            session.pointer_down(at, false);
            session.pointer_up(at, false, false);
        }
        assert_eq!(session.document.object_ids().len(), 1);
        assert_eq!(session.tool(), Tool::Ellipse);
        assert_eq!(session.selection.ids(), &[]);
    }

    /// ux-engineer review item 1: during a create-drag, before release,
    /// `draw_list` already includes the live preview outline — not just
    /// a placeholder that snaps to shape on release.
    #[test]
    fn draw_list_includes_the_live_preview_during_a_create_drag() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        let empty = session.draw_list().triangle_count();

        session.pointer_down(Point::new(0.0, 0.0), false);
        let still_degenerate = session.draw_list().triangle_count();
        assert_eq!(still_degenerate, empty, "no movement yet: no preview");

        session.pointer_hover(Point::new(20.0, 10.0), false, false);
        let with_preview = session.draw_list().triangle_count();
        assert!(
            with_preview > empty,
            "the live rectangle preview must draw before release"
        );

        session.pointer_up(Point::new(20.0, 10.0), false, false);
        assert_eq!(
            session.document.object_ids().len(),
            1,
            "and it still commits once"
        );
    }

    /// ux-engineer review item 2: the numeric readout is present during a
    /// create-drag and anchored at the live pointer position, and absent
    /// otherwise (idle, or a Select-tool press that moves nothing).
    #[test]
    fn live_readout_applies_to_a_create_drag_or_a_select_edit_only() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        assert!(
            session.live_readout().is_none(),
            "idle: nothing to read out"
        );

        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_hover(Point::new(20.0, 10.0), false, false);
        let readout = session.live_readout().expect("a create-drag is in flight");
        assert_eq!(readout.anchor, Point::new(20.0, 10.0));
        assert!(readout.text.contains("20.0"));
        assert!(readout.text.contains("10.0"));

        session.pointer_up(Point::new(20.0, 10.0), false, false);
        // The Select tool is active now; a body press that has not moved
        // shows no readout.
        session.pointer_down(Point::new(10.0, 5.0), false);
        session.pointer_hover(Point::new(10.0, 5.0), false, false);
        assert!(
            session.live_readout().is_none(),
            "no readout for a move or a press that has not moved"
        );
    }

    /// The polygon/star settings never change a selected shape.
    #[test]
    fn the_polygon_star_settings_never_change_a_selected_shape() {
        let mut session = Session::new(1);
        session.set_tool(Tool::PolygonStar);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);
        let before = first_shape(&session);
        session.set_poly_star_point_count(vecmanf_document_core::PointCount::new(11).unwrap());
        session.set_poly_star_ratio(vecmanf_document_core::InnerRatio::new(0.2).unwrap());
        assert_eq!(first_shape(&session), before);
        assert_eq!(session.poly_star_point_count().get(), 11);
    }
}
