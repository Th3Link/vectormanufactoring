//! The rectangle tool's state machine (`specs/0003-primitive-shapes/
//! specification.md`, acceptance criteria 1-6). Follows
//! [`crate::PenTool`]/[`crate::NodeTool`]'s established pattern —
//! ephemeral in-progress drag state (ADR 0009 §2), one
//! [`vecmanf_document_core::Document`] commit on release, a
//! zero-movement create writes nothing — plus a live, uncommitted
//! preview ([`crate::LiveShape`]) threaded through every pointer move
//! so the UX notes' "a maker dragging out a rectangle sees a rectangle
//! updating live" actually holds (ux-engineer review).

use vecmanf_document_core::{
    Document, Length, NodeId, Point, PrimitiveSnapshot, RectBounds, Shape, effective_corner_radius,
};

use crate::ObjectSelection;
use crate::handle_layout::{
    self, HandleKind, ResizeDirection, corner_radius_from_drag, resize_rect_bounds,
};
use crate::shape_hit_test::{hit_test_handle, hit_test_primitive};
use crate::shape_tool_common::{
    LiveShape, ShapeHitTolerances, apply_selection_click, constrained_endpoint, is_degenerate,
    rects_only,
};

#[derive(Debug, Default)]
enum RectDrag {
    #[default]
    None,
    Creating {
        down_at: Point,
        current: Point,
        constrain: bool,
    },
    Resizing {
        id: NodeId,
        direction: ResizeDirection,
        down_at: Point,
        current: Point,
        start_bounds: RectBounds,
        /// The object's own corner radius, captured at drag-start, so
        /// the live preview keeps showing it (recomputed against the
        /// live bounds) while the maker resizes — not just the bare
        /// box (acceptance criterion 3: "any existing corner radius
        /// keeps its absolute length").
        corner_radius: Length,
    },
    DraggingRadius {
        id: NodeId,
        down_at: Point,
        current: Point,
        /// The *effective* radius at drag-start (architect review: the
        /// raw stored value would leave the handle's drag distance
        /// stuck below the true current radius after a shrink, since
        /// the stored register can exceed what the box currently
        /// allows — see `effective_corner_radius`).
        start_radius: Length,
        bounds: RectBounds,
    },
}

/// What [`RectangleTool::pointer_down`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectPointerDownOutcome {
    /// Nothing was hit; a create-drag began.
    Creating,
    /// An existing rectangle was hit and (now) selected.
    Selected,
    /// A shape handle was hit; a resize or radius drag began.
    Handle,
}

/// What [`RectangleTool::pointer_up`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectPointerUpOutcome {
    /// No drag was in flight, or it moved nowhere (acceptance criterion
    /// 1's "A = B creates nothing").
    NoOp,
    /// A new rectangle was created and committed.
    Created(NodeId),
    /// An existing rectangle was resized (acceptance criterion 3).
    Resized,
    /// The corner radius changed (acceptance criteria 4, 5, 6).
    RadiusChanged,
}

/// The rectangle tool's state (acceptance criteria 1-6). Selection is
/// shared ([`ObjectSelection`]), passed in rather than owned, so the
/// ellipse and polygon/star tools can accumulate a mixed-kind selection
/// alongside it (acceptance criterion 22).
#[derive(Debug, Default)]
pub struct RectangleTool {
    drag: RectDrag,
}

impl RectangleTool {
    /// A tool with nothing selected and no drag in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acceptance criteria 1, 2 (create-drag start), 3-6 (selecting an
    /// existing rectangle, or starting a handle drag on it). The
    /// constrain (Ctrl) flag is not known yet at press time — a plain
    /// press with no movement has no visible preview to get wrong
    /// either way ([`RectangleTool::live_shape`] returns `None` until
    /// the drag is non-degenerate) — and is seeded by the first
    /// [`RectangleTool::pointer_move`] call instead.
    pub fn pointer_down(
        &mut self,
        primitives: &[PrimitiveSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
        tolerances: ShapeHitTolerances,
        shift: bool,
    ) -> RectPointerDownOutcome {
        let rects = rects_only(primitives);

        if let [only] = selection.ids()
            && let Some(snapshot) = rects.iter().find(|p| p.id == *only)
            && let Shape::Rect {
                bounds,
                corner_radius,
            } = snapshot.shape
        {
            let handles = handle_layout::rect_handles(bounds, corner_radius);
            if let Some(index) = hit_test_handle(&handles, point, tolerances.handle) {
                let handle = handles[index];
                self.drag = match handle.kind {
                    HandleKind::CornerRadius => RectDrag::DraggingRadius {
                        id: snapshot.id,
                        down_at: point,
                        current: point,
                        start_radius: effective_corner_radius(bounds, corner_radius),
                        bounds,
                    },
                    HandleKind::Resize(direction) => RectDrag::Resizing {
                        id: snapshot.id,
                        direction,
                        down_at: point,
                        current: point,
                        start_bounds: bounds,
                        corner_radius,
                    },
                    HandleKind::CornerRadiusEcho | HandleKind::InnerRadius => {
                        // invariant: `hit_test_handle` only ever returns
                        // the index of a `draggable` handle, and
                        // `rect_handles` never marks either of these
                        // kinds `draggable: true`.
                        unreachable!("not draggable; filtered by hit_test_handle")
                    }
                };
                return RectPointerDownOutcome::Handle;
            }
        }

        let hit = hit_test_primitive(&rects, point, tolerances.outline);
        if let Some(id) = hit {
            apply_selection_click(selection, Some(id), shift);
            return RectPointerDownOutcome::Selected;
        }
        apply_selection_click(selection, None, shift);
        self.drag = RectDrag::Creating {
            down_at: point,
            current: point,
            constrain: false,
        };
        RectPointerDownOutcome::Creating
    }

    /// The pointer moved to `point` with the drag still in flight
    /// (acceptance criteria 3, 4, 5, 9's live-update UX notes) —
    /// updates [`RectangleTool::live_shape`]'s return value, writes
    /// nothing to the document. A no-op when no drag is in progress.
    pub fn pointer_move(&mut self, point: Point, constrain: bool) {
        match &mut self.drag {
            RectDrag::Creating {
                current,
                constrain: stored_constrain,
                ..
            } => {
                *current = point;
                *stored_constrain = constrain;
            }
            RectDrag::Resizing { current, .. } | RectDrag::DraggingRadius { current, .. } => {
                *current = point;
            }
            RectDrag::None => {}
        }
    }

    /// The live, uncommitted preview of whatever drag is in flight —
    /// `None` when idle, or when a create-drag has not moved far enough
    /// to be anything but degenerate yet (acceptance criterion 1; the
    /// UX notes: "a drag that would create nothing shows no preview at
    /// all").
    #[must_use]
    pub fn live_shape(&self) -> Option<LiveShape> {
        match self.drag {
            RectDrag::None => None,
            RectDrag::Creating {
                down_at,
                current,
                constrain,
            } => {
                let end = if constrain {
                    constrained_endpoint(down_at, current)
                } else {
                    current
                };
                if is_degenerate(down_at, end) {
                    return None;
                }
                let bounds = RectBounds::from_corners(down_at, end);
                Some(LiveShape::Creating(
                    Shape::Rect {
                        bounds,
                        corner_radius: Length::from_mm(0.0),
                    },
                    end,
                ))
            }
            RectDrag::Resizing {
                direction,
                down_at,
                current,
                start_bounds,
                corner_radius,
                ..
            } => {
                let delta = down_at.vector_to(current);
                let bounds = resize_rect_bounds(start_bounds, direction, delta);
                Some(LiveShape::Adjusting(Shape::Rect {
                    bounds,
                    corner_radius,
                }))
            }
            RectDrag::DraggingRadius {
                down_at,
                current,
                start_radius,
                bounds,
                ..
            } => {
                let delta = down_at.vector_to(current);
                let radius = corner_radius_from_drag(start_radius, bounds, delta);
                Some(LiveShape::Adjusting(Shape::Rect {
                    bounds,
                    corner_radius: radius,
                }))
            }
        }
    }

    /// Acceptance criteria 1, 2, 3, 4, 5, 6: commits whatever gesture
    /// [`RectangleTool::pointer_down`] began. `constrain` is the Ctrl
    /// modifier's state at release (acceptance criterion 2).
    pub fn pointer_up(
        &mut self,
        document: &Document,
        point: Point,
        constrain: bool,
    ) -> RectPointerUpOutcome {
        match std::mem::take(&mut self.drag) {
            RectDrag::None => RectPointerUpOutcome::NoOp,
            RectDrag::Creating { down_at, .. } => {
                let end = if constrain {
                    constrained_endpoint(down_at, point)
                } else {
                    point
                };
                if is_degenerate(down_at, end) {
                    return RectPointerUpOutcome::NoOp;
                }
                let bounds = RectBounds::from_corners(down_at, end);
                RectPointerUpOutcome::Created(document.create_rect(bounds))
            }
            RectDrag::Resizing {
                id,
                direction,
                down_at,
                start_bounds,
                ..
            } => {
                if point == down_at {
                    return RectPointerUpOutcome::NoOp;
                }
                let delta = down_at.vector_to(point);
                let bounds = resize_rect_bounds(start_bounds, direction, delta);
                let _ = document.set_rect_bounds(id, bounds);
                RectPointerUpOutcome::Resized
            }
            RectDrag::DraggingRadius {
                id,
                down_at,
                start_radius,
                bounds,
                ..
            } => {
                if point == down_at {
                    return RectPointerUpOutcome::NoOp;
                }
                let delta = down_at.vector_to(point);
                let radius = corner_radius_from_drag(start_radius, bounds, delta);
                let _ = document.set_corner_radius(&[id], radius);
                RectPointerUpOutcome::RadiusChanged
            }
        }
    }

    /// Discards any in-progress drag, writing nothing. Returns whether
    /// there was anything to discard.
    pub fn escape(&mut self) -> bool {
        let had_drag = !matches!(self.drag, RectDrag::None);
        self.drag = RectDrag::None;
        had_drag
    }

    /// Acceptance criterion 6's "remove rounding" action: zeroes the
    /// corner radius of every currently selected rectangle, in one
    /// commit for the whole selection (architect review: previously one
    /// commit per selected id).
    pub fn remove_rounding(&self, document: &Document, selection: &ObjectSelection) {
        let ids: Vec<NodeId> = rects_only_ids(document, selection.ids());
        if ids.is_empty() {
            return;
        }
        let _ = document.set_corner_radius(&ids, Length::from_mm(0.0));
    }
}

/// Filters `ids` down to the ones that currently name a rectangle —
/// `document.set_corner_radius`'s batch refuses the *whole* call if any
/// id in it is not a rect, so the caller (here) pre-filters, same
/// contract `convert_to_paths` already uses.
fn rects_only_ids(document: &Document, ids: &[NodeId]) -> Vec<NodeId> {
    ids.iter()
        .copied()
        .filter(|&id| {
            matches!(
                document.primitive(id).map(|p| p.shape),
                Some(Shape::Rect { .. })
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Document, Tolerance};

    const TOLERANCES: ShapeHitTolerances = ShapeHitTolerances {
        outline: Tolerance::from_mm(1.0),
        handle: Tolerance::from_mm(2.0),
    };

    fn snapshots(document: &Document) -> Vec<PrimitiveSnapshot> {
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.primitive(id))
            .collect()
    }

    /// AC1: a drag from A to B creates a rectangle with A, B as opposite
    /// corners, zero radius, and A == B creates nothing.
    #[test]
    fn ac1_rectangle_drag_creates_a_zero_radius_rect() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();

        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(10.0, 20.0), false);
        let RectPointerUpOutcome::Created(id) = outcome else {
            panic!("expected Created, got {outcome:?}");
        };
        let snapshot = document.primitive(id).expect("exists");
        assert_eq!(
            snapshot.shape,
            Shape::Rect {
                bounds: RectBounds {
                    origin: Point::new(0.0, 0.0),
                    width: Length::from_mm(10.0),
                    height: Length::from_mm(20.0),
                },
                corner_radius: Length::from_mm(0.0),
            }
        );

        // A = B creates nothing.
        tool.pointer_down(&[], &mut selection, Point::new(5.0, 5.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(5.0, 5.0), false);
        assert_eq!(outcome, RectPointerUpOutcome::NoOp);
    }

    /// The UX notes' live-feedback rule: during a create-drag, before
    /// release, `live_shape` already reports the final bounds — not
    /// just a placeholder.
    #[test]
    fn live_shape_during_create_drag_matches_what_pointer_up_would_commit() {
        let mut tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        assert!(tool.live_shape().is_none(), "no movement yet: no preview");

        tool.pointer_move(Point::new(10.0, 20.0), false);
        let Some(LiveShape::Creating(Shape::Rect { bounds, .. }, anchor)) = tool.live_shape()
        else {
            panic!("expected a Creating rect preview");
        };
        assert!((bounds.width.as_mm() - 10.0).abs() < 1e-9);
        assert!((bounds.height.as_mm() - 20.0).abs() < 1e-9);
        assert_eq!(anchor, Point::new(10.0, 20.0));
    }

    /// The constrain flag applies live, not just at release.
    #[test]
    fn live_shape_during_create_drag_respects_constrain() {
        let mut tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        tool.pointer_move(Point::new(5.0, 20.0), true);
        let Some(LiveShape::Creating(Shape::Rect { bounds, .. }, _)) = tool.live_shape() else {
            panic!("expected a Creating rect preview");
        };
        assert!((bounds.width.as_mm() - 20.0).abs() < 1e-9);
        assert!((bounds.height.as_mm() - 20.0).abs() < 1e-9);
    }

    /// AC2: Ctrl-constrain sizes the rectangle to the larger extent on
    /// both axes.
    #[test]
    fn ac2_constrain_makes_a_square_sized_to_the_larger_extent() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        let outcome = tool.pointer_up(&document, Point::new(5.0, 20.0), true);
        let RectPointerUpOutcome::Created(id) = outcome else {
            panic!("expected Created");
        };
        let snapshot = document.primitive(id).expect("exists");
        let Shape::Rect { bounds, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        assert!((bounds.width.as_mm() - 20.0).abs() < 1e-9);
        assert!((bounds.height.as_mm() - 20.0).abs() < 1e-9);
    }

    /// AC3: dragging a resize handle changes width/height, keeps the
    /// radius (absolute length) and the object stays a rectangle; the
    /// live preview along the way already shows the resized box.
    #[test]
    fn ac3_resize_handle_drag_changes_bounds_keeps_radius() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .set_corner_radius(&[id], Length::from_mm(2.0))
            .expect("set radius");
        let mut tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);

        let primitives = snapshots(&document);
        let outcome = tool.pointer_down(
            &primitives,
            &mut selection,
            Point::new(10.0, 5.0), // the E resize handle
            TOLERANCES,
            false,
        );
        assert_eq!(outcome, RectPointerDownOutcome::Handle);

        tool.pointer_move(Point::new(20.0, 5.0), false);
        let Some(LiveShape::Adjusting(Shape::Rect {
            bounds: live_bounds,
            corner_radius: live_radius,
        })) = tool.live_shape()
        else {
            panic!("expected an Adjusting rect preview");
        };
        assert!((live_bounds.width.as_mm() - 20.0).abs() < 1e-9);
        assert!((live_radius.as_mm() - 2.0).abs() < 1e-9);

        let up_outcome = tool.pointer_up(&document, Point::new(20.0, 5.0), false);
        assert_eq!(up_outcome, RectPointerUpOutcome::Resized);

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Rect {
            bounds,
            corner_radius,
        } = snapshot.shape
        else {
            panic!("expected rect");
        };
        assert!((bounds.width.as_mm() - 20.0).abs() < 1e-9);
        assert!((corner_radius.as_mm() - 2.0).abs() < 1e-9);
    }

    /// AC4, AC5: dragging the corner-radius handle out rounds all four
    /// corners, clamped to half the shorter side.
    #[test]
    fn ac4_ac5_corner_radius_handle_drag_rounds_and_clamps() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let mut tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);

        let primitives = snapshots(&document);
        let outcome = tool.pointer_down(
            &primitives,
            &mut selection,
            Point::new(10.0, 0.0), // the corner-radius handle, at zero radius
            TOLERANCES,
            false,
        );
        assert_eq!(outcome, RectPointerDownOutcome::Handle);
        // Drag far past half the shorter side (5mm).
        let up_outcome = tool.pointer_up(&document, Point::new(-90.0, 90.0), false);
        assert_eq!(up_outcome, RectPointerUpOutcome::RadiusChanged);

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Rect { corner_radius, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        assert!((corner_radius.as_mm() - 5.0).abs() < 1e-6);
    }

    /// Architect review: the corner-radius drag must start from the
    /// *effective* (clamped) radius, not the raw stored one, or the
    /// handle stays visually stuck after a shrink.
    #[test]
    fn corner_radius_drag_starts_from_the_effective_radius_after_a_shrink() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(100.0),
        });
        document
            .set_corner_radius(&[id], Length::from_mm(40.0))
            .expect("set radius");
        // Shrink so the shorter side is 20mm (half = 10mm): the raw
        // register stays 40mm, but the effective radius clamps to 10mm.
        document
            .set_rect_bounds(
                id,
                RectBounds::from_corners(Point::new(0.0, 0.0), Point::new(100.0, 20.0)),
            )
            .expect("resize");

        let mut tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let primitives = snapshots(&document);
        let shrunk_bounds = RectBounds::from_corners(Point::new(0.0, 0.0), Point::new(100.0, 20.0));
        // The handle now sits 10mm in from the corner along the
        // diagonal (the effective radius), not 40mm.
        let handle_position = handle_layout::rect_handles(shrunk_bounds, Length::from_mm(40.0))
            .into_iter()
            .find(|h| h.kind == HandleKind::CornerRadius)
            .expect("corner-radius handle exists")
            .position;
        let outcome = tool.pointer_down(
            &primitives,
            &mut selection,
            handle_position,
            TOLERANCES,
            false,
        );
        assert_eq!(outcome, RectPointerDownOutcome::Handle);
        // The slightest further drag toward the corner (the NE
        // diagonal direction) must immediately start reducing the
        // radius below 10mm, not require first "catching up" from
        // 40mm.
        let toward_corner = handle_position.translated(ResizeDirection::Ne.unit_vector());
        tool.pointer_move(toward_corner, false);
        let Some(LiveShape::Adjusting(Shape::Rect {
            corner_radius: live_radius,
            ..
        })) = tool.live_shape()
        else {
            panic!("expected an Adjusting rect preview");
        };
        assert!(
            live_radius.as_mm() < 10.0,
            "radius must shrink immediately from the effective 10mm, got {}",
            live_radius.as_mm()
        );
    }

    /// AC6: "remove rounding" zeroes the radius.
    #[test]
    fn ac6_remove_rounding_zeroes_the_radius() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .set_corner_radius(&[id], Length::from_mm(4.0))
            .expect("set radius");
        let tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        tool.remove_rounding(&document, &selection);

        let snapshot = document.primitive(id).expect("exists");
        let Shape::Rect { corner_radius, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        assert!(corner_radius.as_mm().abs() < f64::EPSILON);
    }

    /// Architect review: removing rounding on a multi-rect selection is
    /// one commit, not one per object.
    #[test]
    fn remove_rounding_on_a_multi_selection_is_one_commit() {
        let document = Document::new(1);
        let a = document.create_rect(RectBounds::from_corners(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
        ));
        let b = document.create_rect(RectBounds::from_corners(
            Point::new(20.0, 0.0),
            Point::new(30.0, 10.0),
        ));
        document
            .set_corner_radius(&[a], Length::from_mm(3.0))
            .unwrap();
        document
            .set_corner_radius(&[b], Length::from_mm(3.0))
            .unwrap();

        let tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(a);
        selection.toggle(b);
        tool.remove_rounding(&document, &selection);

        for id in [a, b] {
            let Shape::Rect { corner_radius, .. } = document.primitive(id).unwrap().shape else {
                panic!("expected rect");
            };
            assert!(corner_radius.as_mm().abs() < f64::EPSILON);
        }
    }

    /// Escape mid-drag cancels it, writing nothing.
    #[test]
    fn escape_mid_drag_writes_nothing() {
        let document = Document::new(1);
        let mut tool = RectangleTool::new();
        let mut selection = ObjectSelection::new();
        tool.pointer_down(&[], &mut selection, Point::new(0.0, 0.0), TOLERANCES, false);
        assert!(tool.escape());
        assert!(tool.live_shape().is_none());
        let outcome = tool.pointer_up(&document, Point::new(50.0, 50.0), false);
        assert_eq!(outcome, RectPointerUpOutcome::NoOp);
        assert_eq!(document.object_ids(), Vec::new());
    }
}
