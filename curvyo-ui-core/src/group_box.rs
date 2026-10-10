//! The group box of a multi-selection and the handles on it
//! (`specs/0019-multi-object-transform/`, criteria 1 to 15): the axis-aligned
//! tight bounds of the drawn outlines of the selected objects, what kinds the
//! selection holds (which decides the handles), and the handle layout over the
//! box, which is the single-object layout of [`crate::transform_handle_layout`]
//! on an angle-0 [`OrientedBox`].

use std::collections::HashSet;

use curvyo_document_core::{Angle, NodeId, ObjectSnapshot, Point, PrimitiveSnapshot, Shape};

use crate::ResizeDirection;
use crate::object_bounds::object_outline_bounds;
use crate::oriented_box::OrientedBox;
use crate::transform_handle_layout::{
    ALL_EIGHT, CORNERS_FOUR, EditHandle, HandleSpec, TransformHandleTolerances, at_least,
    hit_transform_handle_for_side, transform_handles,
};

/// Two lengths closer than this (millimetres) are equal, and an extent below it
/// is "zero": the document's geometric tolerance of the group box rules.
pub(crate) const GEOMETRIC_TOLERANCE_MM: f64 = 1e-6;

/// A rotation within this (radians) of a multiple of 90 degrees is one.
const QUARTER_TURN_TOLERANCE_RAD: f64 = 1e-9;

/// The shape of a group box on screen: a box, one line when one axis has no
/// extent, or a single point (criteria 8 and 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBoxShape {
    /// Both axes have an extent.
    Box,
    /// One axis has none: the box is a line, drawn once.
    Line,
    /// Neither axis has an extent: all selected objects are one point.
    Point,
}

/// A kind of primitive that cannot be stretched, for the hint of criterion 21.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    Polygon,
    Star,
    RotatedRectangle,
    RotatedEllipse,
}

/// The causes in the order the hint names them, with their words.
const CAUSES: [(Cause, &str); 4] = [
    (Cause::Polygon, "a polygon"),
    (Cause::Star, "a star"),
    (Cause::RotatedRectangle, "a rotated rectangle"),
    (Cause::RotatedEllipse, "a rotated ellipse"),
];

/// Which of the [`CAUSES`] the selection holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct UniformCause {
    present: [bool; 4],
}

/// What a multi-selection is, as far as its group box and handles go.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupSelection {
    box_: OrientedBox,
    count: usize,
    all_paths: bool,
    uniform_only: bool,
    cause: UniformCause,
}

/// Whether `rotation` is a multiple of 90 degrees (criterion "aligned primitive").
fn is_quarter_turn(rotation: Angle) -> bool {
    let quarters = rotation.as_radians() / std::f64::consts::FRAC_PI_2;
    (quarters - quarters.round()).abs() * std::f64::consts::FRAC_PI_2 <= QUARTER_TURN_TOLERANCE_RAD
}

/// Whether `object` is a rectangle or ellipse whose outline can be stretched
/// along the document axes without becoming something else: a rotation that is a
/// multiple of 90 degrees, or an ellipse whose radii are equal (a circle, whatever
/// its rotation). A polygon or star never is (criterion "Aligned primitive").
#[must_use]
pub fn is_aligned_primitive(object: &ObjectSnapshot) -> bool {
    match object {
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { .. },
            rotation,
            ..
        }) => is_quarter_turn(*rotation),
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Ellipse { frame },
            rotation,
            ..
        }) => {
            is_quarter_turn(*rotation)
                || (frame.rx.as_mm() - frame.ry.as_mm()).abs() < GEOMETRIC_TOLERANCE_MM
        }
        _ => false,
    }
}

/// Whether `object` is a primitive that can only be scaled proportionally.
fn is_uniform_only_member(object: &ObjectSnapshot) -> bool {
    matches!(object, ObjectSnapshot::Primitive(_)) && !is_aligned_primitive(object)
}

impl UniformCause {
    fn note(&mut self, object: &ObjectSnapshot) {
        if let ObjectSnapshot::Primitive(primitive) = object
            && !is_aligned_primitive(object)
        {
            let cause = match primitive.shape {
                Shape::Polygon { .. } => Cause::Polygon,
                Shape::Star { .. } => Cause::Star,
                Shape::Rect { .. } => Cause::RotatedRectangle,
                Shape::Ellipse { .. } => Cause::RotatedEllipse,
            };
            if let Some(index) = CAUSES.iter().position(|(c, _)| *c == cause) {
                self.present[index] = true;
            }
        }
    }

    /// "Holds a star and a rotated rectangle": only the kinds present, in the
    /// order polygon, star, rotated rectangle, rotated ellipse, each with "a",
    /// joined by commas and "and" (criterion 21).
    fn line(self) -> Option<String> {
        let names: Vec<&str> = CAUSES
            .iter()
            .zip(self.present)
            .filter_map(|((_, name), present)| present.then_some(*name))
            .collect();
        let (last, rest) = names.split_last()?;
        Some(if rest.is_empty() {
            format!("Holds {last}")
        } else {
            format!("Holds {} and {last}", rest.join(", "))
        })
    }
}

impl GroupSelection {
    /// The group of the objects named by `ids` that exist in `objects`; `None`
    /// unless there are at least two (a single object keeps its own box,
    /// criterion 6).
    #[must_use]
    pub fn from_objects(objects: &[ObjectSnapshot], ids: &[NodeId]) -> Option<Self> {
        let wanted: HashSet<NodeId> = ids.iter().copied().collect();
        let mut members = objects.iter().filter(|o| wanted.contains(&o.id()));
        let first = members.next()?;
        let (mut min, mut max) = object_outline_bounds(first);
        let mut count = 1;
        let mut all_paths = matches!(first, ObjectSnapshot::Path(_));
        let mut uniform_only = is_uniform_only_member(first);
        let mut cause = UniformCause::default();
        cause.note(first);
        for object in members {
            let (low, high) = object_outline_bounds(object);
            min = Point::new(min.x.min(low.x), min.y.min(low.y));
            max = Point::new(max.x.max(high.x), max.y.max(high.y));
            count += 1;
            all_paths &= matches!(object, ObjectSnapshot::Path(_));
            uniform_only |= is_uniform_only_member(object);
            cause.note(object);
        }
        (count >= 2).then(|| Self {
            box_: axis_aligned_box(min, max),
            count,
            all_paths,
            uniform_only,
            cause,
        })
    }

    /// The group box: axis-aligned, the tight bounds of the drawn outlines,
    /// without stroke width (criteria 1 to 3).
    #[must_use]
    pub const fn bounds(&self) -> &OrientedBox {
        &self.box_
    }

    /// The number of selected objects the group holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Whether every selected object is a path (the skew handles exist only
    /// then, criterion 40).
    #[must_use]
    pub const fn all_paths(&self) -> bool {
        self.all_paths
    }

    /// Whether the selection can only be scaled proportionally: it holds a
    /// polygon, a star, or a rectangle or ellipse that is not an aligned
    /// primitive (criterion 21).
    #[must_use]
    pub const fn uniform_only(&self) -> bool {
        self.uniform_only
    }

    /// The cause line of the hint of a corner handle of a uniform-only
    /// selection ("Holds a star and a rotated rectangle"); `None` for any other.
    #[must_use]
    pub fn uniform_cause_line(&self) -> Option<String> {
        if self.uniform_only {
            self.cause.line()
        } else {
            None
        }
    }

    /// Whether the box has no extent in its x axis (width under the tolerance).
    fn flat_x(&self) -> bool {
        self.box_.width() < GEOMETRIC_TOLERANCE_MM
    }

    /// Whether the box has no extent in its y axis.
    fn flat_y(&self) -> bool {
        self.box_.height() < GEOMETRIC_TOLERANCE_MM
    }

    /// Whether the box is a box, a line or a point (criteria 8 and 13).
    #[must_use]
    pub fn shape(&self) -> GroupBoxShape {
        match (self.flat_x(), self.flat_y()) {
            (true, true) => GroupBoxShape::Point,
            (false, false) => GroupBoxShape::Box,
            _ => GroupBoxShape::Line,
        }
    }

    /// `s`, the "shorter side" the size tiers and hit radii use: the shorter
    /// side of a box, the one extent of a line, 0 for a point (criteria 10, 12, 13).
    #[must_use]
    pub fn side_mm(&self) -> f64 {
        match self.shape() {
            GroupBoxShape::Box => self.box_.width().min(self.box_.height()),
            GroupBoxShape::Line => self.box_.width().max(self.box_.height()),
            GroupBoxShape::Point => 0.0,
        }
    }
}

/// The angle-0 box with corners `min` and `max`. Its pivot is its centre, as
/// the box of a primitive has.
fn axis_aligned_box(min: Point, max: Point) -> OrientedBox {
    OrientedBox {
        min,
        max,
        angle: Angle::from_radians(0.0),
        pivot: Point::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y)),
    }
}

/// The edge resize handles of a box that is flat in y: only the two on its x
/// extent change something (criterion 12).
const HORIZONTAL_EDGES: [ResizeDirection; 2] = [ResizeDirection::E, ResizeDirection::W];

/// The edge resize handles of a box that is flat in x.
const VERTICAL_EDGES: [ResizeDirection; 2] = [ResizeDirection::N, ResizeDirection::S];

impl GroupSelection {
    /// The resize handles the group offers (criteria 12 and 21): the four corners
    /// and four edges; only the corners for a uniform-only selection, whose edge
    /// handles are not drawn and have no hit area; and, for a box flat in one
    /// axis, only what changes the other axis (nothing for a uniform-only
    /// selection, whose corners change both).
    fn resize_directions(&self) -> &'static [ResizeDirection] {
        let (flat_x, flat_y) = (self.flat_x(), self.flat_y());
        if flat_x && flat_y {
            return &[];
        }
        if self.uniform_only {
            // The corners change both axes; a flat box has no axis to stretch.
            return if flat_x || flat_y { &[] } else { &CORNERS_FOUR };
        }
        match (flat_x, flat_y) {
            (false, true) => &HORIZONTAL_EDGES,
            (true, false) => &VERTICAL_EDGES,
            _ => &ALL_EIGHT,
        }
    }
}

/// Every handle of the group the hit rule sees, in document space, on `box_`
/// (the group box, or, in a rotate drag, the box at the press turned by the
/// live angle). The edge resize handles of a box under 24 px are in the set
/// although not drawn ([`is_drawn_group_handle`]); a handle that is not drawn
/// and has no hit area is not in it (criterion 14). A single point has none.
#[must_use]
pub fn group_handles(
    group: &GroupSelection,
    box_: &OrientedBox,
    tolerances: &TransformHandleTolerances,
    side_rotate: bool,
) -> Vec<(EditHandle, Point)> {
    if group.shape() == GroupBoxShape::Point {
        return Vec::new();
    }
    let spec = HandleSpec {
        resize_directions: group.resize_directions(),
        skew: group.all_paths(),
        side_rotate,
    };
    // The layout of a single object's box, on the group box. Its centre handle
    // uses the shorter side, which is zero for a flat box, so the centre is
    // decided here with the group's own `s` (criterion 12).
    let mut handles: Vec<(EditHandle, Point)> = transform_handles(box_, spec, tolerances)
        .into_iter()
        .filter(|(handle, _)| *handle != EditHandle::Move)
        .collect();
    if at_least(group.side_mm(), tolerances.center_min_side_mm) {
        handles.push((EditHandle::Move, box_.to_document(box_.local_center())));
    }
    handles
}

/// Whether `handle` of `group` is drawn (criterion 10): every handle of
/// [`group_handles`] is, except an edge resize handle on a box under 24 px.
#[must_use]
pub fn is_drawn_group_handle(
    handle: EditHandle,
    group: &GroupSelection,
    tolerances: &TransformHandleTolerances,
) -> bool {
    match handle {
        EditHandle::Resize(direction) => {
            crate::transform_handle_layout::is_corner(direction)
                || at_least(group.side_mm(), tolerances.edge_handle_min_side_mm)
        }
        _ => true,
    }
}

/// The group handle under `point`: the one nearest-centre test over the drawn
/// group handles (criterion 11), with the centre handle tested last and only
/// inside its hover region when `include_move`.
#[must_use]
pub fn hit_group_handle(
    group: &GroupSelection,
    handles: &[(EditHandle, Point)],
    box_: &OrientedBox,
    point: Point,
    tolerances: &TransformHandleTolerances,
    include_move: bool,
) -> Option<EditHandle> {
    hit_transform_handle_for_side(
        handles,
        box_,
        point,
        tolerances,
        include_move,
        group.side_mm(),
    )
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{
        AnchorId, Document, Length, NewAnchor, PointCount, RectBounds, StarFrame,
    };

    use super::*;
    use crate::transform_handle_layout::Side;

    fn rect(document: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        })
    }

    fn objects_of(document: &Document) -> Vec<ObjectSnapshot> {
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect()
    }

    fn group_of(document: &Document, ids: &[NodeId]) -> Option<GroupSelection> {
        GroupSelection::from_objects(&objects_of(document), ids)
    }

    fn line_path(document: &Document, n: u64, a: Point, b: Point) -> NodeId {
        document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, n), a),
                NewAnchor::corner(AnchorId::new(1, n + 1), b),
            ],
            false,
        )
    }

    /// Criterion 1, worked example: a 20 x 10 rectangle at (10, 5) turned by 30
    /// degrees and a circle of radius 5 at (40, 5) have the box x -1.16 to 45.00
    /// and y -4.33 to 14.33.
    #[test]
    fn the_group_box_is_the_tight_bounds_of_the_drawn_outlines() {
        let document = Document::new(1);
        let turned = rect(&document, 0.0, 0.0, 20.0, 10.0);
        let circle = document.create_ellipse(curvyo_document_core::EllipseFrame {
            center: Point::new(40.0, 5.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(5.0),
        });
        let object = document.object(turned).expect("exists").rotated(
            Point::new(10.0, 5.0),
            Angle::from_radians(30.0_f64.to_radians()),
        );
        document.rotate_object(&object).expect("rotates");
        let group = group_of(&document, &[turned, circle]).expect("two objects");
        let b = group.bounds();
        assert!((b.min.x + 1.160_254).abs() < 1e-5, "{b:?}");
        assert!((b.max.x - 45.0).abs() < 1e-9);
        assert!((b.min.y + 4.330_127).abs() < 1e-5);
        assert!((b.max.y - 14.330_127).abs() < 1e-5);
        assert_eq!(b.angle.as_radians(), 0.0);
        assert_eq!(group.count(), 2);
    }

    /// Criterion 6: one object has no group; ids that do not exist are ignored.
    #[test]
    fn a_single_object_has_no_group_box() {
        let document = Document::new(1);
        let a = rect(&document, 0.0, 0.0, 10.0, 10.0);
        let b = rect(&document, 20.0, 0.0, 10.0, 10.0);
        assert!(group_of(&document, &[a]).is_none());
        assert!(group_of(&document, &[]).is_none());
        assert!(group_of(&document, &[a, b]).is_some());
        document.delete_objects(&[b]).expect("deletes");
        assert!(group_of(&document, &[a, b]).is_none(), "b is gone");
    }

    /// Criterion 2: the stroke width is not part of the box.
    #[test]
    fn the_stroke_width_does_not_change_the_box() {
        let document = Document::new(1);
        let a = rect(&document, 0.0, 0.0, 10.0, 10.0);
        let b = rect(&document, 20.0, 0.0, 10.0, 10.0);
        let before = group_of(&document, &[a, b]).expect("group");
        let mut wide = objects_of(&document);
        for object in &mut wide {
            object.style_mut().stroke.width = Length::from_mm(8.0);
        }
        let after = GroupSelection::from_objects(&wide, &[a, b]).expect("group");
        assert_eq!(before.bounds(), after.bounds());
    }

    /// Criteria 8 and 13: a flat box is a line, a single point is a point.
    #[test]
    fn a_flat_box_is_a_line_and_one_point_is_a_point() {
        let document = Document::new(1);
        let a = line_path(&document, 1, Point::new(0.0, 5.0), Point::new(10.0, 5.0));
        let b = line_path(&document, 3, Point::new(20.0, 5.0), Point::new(30.0, 5.0));
        let line = group_of(&document, &[a, b]).expect("group");
        assert_eq!(line.shape(), GroupBoxShape::Line);
        assert!(
            (line.side_mm() - 30.0).abs() < 1e-9,
            "s is the extent that exists"
        );
        let c = line_path(&document, 5, Point::new(7.0, 7.0), Point::new(7.0, 7.0));
        let d = line_path(&document, 7, Point::new(7.0, 7.0), Point::new(7.0, 7.0));
        let point = group_of(&document, &[c, d]).expect("group");
        assert_eq!(point.shape(), GroupBoxShape::Point);
        assert_eq!(point.side_mm(), 0.0);
    }

    /// Criterion 21 and D3: polygons, stars and misaligned rectangles and
    /// ellipses hold the selection to proportional scaling; a circle never does;
    /// the cause line lists only what is present, in order.
    #[test]
    fn uniform_only_follows_the_aligned_primitive_rule() {
        let document = Document::new(1);
        let plain = rect(&document, 0.0, 0.0, 10.0, 10.0);
        let path = line_path(&document, 1, Point::new(0.0, 0.0), Point::new(5.0, 5.0));
        let frame = StarFrame {
            center: Point::new(50.0, 0.0),
            radius: Length::from_mm(5.0),
            angle: Angle::from_radians(0.0),
        };
        let star = document.create_star(
            frame,
            PointCount::new(5).expect("count"),
            curvyo_document_core::InnerRatio::new(0.5).expect("ratio"),
        );
        let polygon = document.create_polygon(frame, PointCount::new(5).expect("count"));
        let turned = rect(&document, 30.0, 0.0, 10.0, 5.0);
        let object = document
            .object(turned)
            .expect("exists")
            .rotated(Point::new(35.0, 2.5), Angle::from_radians(0.3));
        document.rotate_object(&object).expect("rotates");
        let circle = document.create_ellipse(curvyo_document_core::EllipseFrame {
            center: Point::new(70.0, 0.0),
            rx: Length::from_mm(3.0),
            ry: Length::from_mm(3.0),
        });
        let turned_circle = document
            .object(circle)
            .expect("exists")
            .rotated(Point::new(70.0, 0.0), Angle::from_radians(0.4));
        document.rotate_object(&turned_circle).expect("rotates");

        assert!(
            !group_of(&document, &[plain, path])
                .expect("g")
                .uniform_only()
        );
        assert!(
            !group_of(&document, &[plain, circle])
                .expect("g")
                .uniform_only()
        );
        let g = group_of(&document, &[plain, star, turned]).expect("g");
        assert!(g.uniform_only());
        assert_eq!(
            g.uniform_cause_line().as_deref(),
            Some("Holds a star and a rotated rectangle")
        );
        let g = group_of(&document, &[polygon, star, turned]).expect("g");
        assert_eq!(
            g.uniform_cause_line().as_deref(),
            Some("Holds a polygon, a star and a rotated rectangle")
        );
        let g = group_of(&document, &[path, star]).expect("g");
        assert_eq!(g.uniform_cause_line().as_deref(), Some("Holds a star"));
        assert!(
            group_of(&document, &[plain, path])
                .expect("g")
                .uniform_cause_line()
                .is_none()
        );
    }

    /// Criterion 10: the centre handle from 48 px, not before; corner resize and
    /// corner rotate always; the edge resize handles from 24 px are drawn, below
    /// that they are hit-testable but not drawn.
    #[test]
    fn the_size_tiers_follow_the_shorter_side() {
        let document = Document::new(1);
        let a = rect(&document, 0.0, 0.0, 10.0, 10.0);
        let b = rect(&document, 20.0, 0.0, 10.0, 10.0);
        let group = group_of(&document, &[a, b]).expect("group");
        // 30 x 10 mm: the shorter side is 10 mm.
        let drawn = |scale: f64| -> Vec<EditHandle> {
            let tolerances = TransformHandleTolerances::at_scale(scale);
            group_handles(&group, group.bounds(), &tolerances, false)
                .into_iter()
                .map(|(handle, _)| handle)
                .filter(|handle| is_drawn_group_handle(*handle, &group, &tolerances))
                .collect()
        };
        let has = |handles: &[EditHandle], wanted: EditHandle| handles.contains(&wanted);
        let centre = |scale: f64| {
            let tolerances = TransformHandleTolerances::at_scale(scale);
            group_handles(&group, group.bounds(), &tolerances, false)
                .into_iter()
                .find(|(handle, _)| *handle == EditHandle::Move)
                .map(|(_, at)| at)
        };
        // 50 px: everything; the centre is at the middle of the box.
        let at_50 = drawn(5.0);
        assert!(has(&at_50, EditHandle::Move));
        assert!(has(&at_50, EditHandle::Resize(ResizeDirection::N)));
        let middle = centre(5.0).expect("centre handle");
        assert!((middle.x - 15.0).abs() < 1e-9 && (middle.y - 5.0).abs() < 1e-9);
        // 40 px: no centre handle; the edge handles stay.
        let at_40 = drawn(4.0);
        assert!(!has(&at_40, EditHandle::Move));
        assert!(has(&at_40, EditHandle::Resize(ResizeDirection::N)));
        // 20 px: corners only are drawn, the edge handles are still in the set.
        let at_20 = drawn(2.0);
        assert!(!has(&at_20, EditHandle::Resize(ResizeDirection::N)));
        assert!(has(&at_20, EditHandle::Resize(ResizeDirection::Ne)));
        assert!(has(&at_20, EditHandle::Rotate(ResizeDirection::Ne)));
        let tolerances = TransformHandleTolerances::at_scale(2.0);
        assert!(
            group_handles(&group, group.bounds(), &tolerances, false)
                .iter()
                .any(|(handle, _)| *handle == EditHandle::Resize(ResizeDirection::N)),
            "an edge handle under 24 px stays hit-testable"
        );
    }

    /// Criterion 12: a box flat in y keeps the two edge handles on its x extent
    /// and the centre handle; the corners and the other edges are not offered.
    #[test]
    fn a_flat_box_keeps_only_the_resize_handles_that_change_something() {
        let document = Document::new(1);
        let a = line_path(&document, 1, Point::new(0.0, 5.0), Point::new(10.0, 5.0));
        let b = line_path(&document, 3, Point::new(70.0, 5.0), Point::new(80.0, 5.0));
        let group = group_of(&document, &[a, b]).expect("group");
        let tolerances = TransformHandleTolerances::at_scale(2.0); // 160 px wide
        let handles: Vec<EditHandle> = group_handles(&group, group.bounds(), &tolerances, false)
            .into_iter()
            .map(|(handle, _)| handle)
            .collect();
        let resize: Vec<_> = handles
            .iter()
            .filter(|h| matches!(h, EditHandle::Resize(_)))
            .collect();
        assert_eq!(
            resize,
            [
                &EditHandle::Resize(ResizeDirection::E),
                &EditHandle::Resize(ResizeDirection::W)
            ]
        );
        assert!(handles.contains(&EditHandle::Move));
        assert_eq!(
            handles
                .iter()
                .filter(|h| matches!(h, EditHandle::Rotate(_)))
                .count(),
            4,
            "the corner rotate handles stand at the notional square's corners"
        );
        assert!(
            !handles
                .iter()
                .any(|h| matches!(h, EditHandle::Skew(Side::Top | Side::Bottom))),
            "zero height removes the top and bottom skew handles"
        );
    }

    /// Criterion 13: one point offers no handle at all.
    #[test]
    fn a_single_point_has_no_handle() {
        let document = Document::new(1);
        let c = line_path(&document, 5, Point::new(7.0, 7.0), Point::new(7.0, 7.0));
        let d = line_path(&document, 7, Point::new(7.0, 7.0), Point::new(7.0, 7.0));
        let group = group_of(&document, &[c, d]).expect("group");
        let tolerances = TransformHandleTolerances::at_scale(5.0);
        assert_eq!(
            group_handles(&group, group.bounds(), &tolerances, true),
            Vec::new()
        );
    }

    /// Criterion 21: a uniform-only selection offers the corner handles only.
    #[test]
    fn a_uniform_only_selection_has_no_edge_handle() {
        let document = Document::new(1);
        let star = document.create_star(
            StarFrame {
                center: Point::new(50.0, 50.0),
                radius: Length::from_mm(20.0),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(5).expect("count"),
            curvyo_document_core::InnerRatio::new(0.5).expect("ratio"),
        );
        let plain = rect(&document, 0.0, 0.0, 10.0, 10.0);
        let group = group_of(&document, &[star, plain]).expect("group");
        let tolerances = TransformHandleTolerances::at_scale(5.0);
        let handles = group_handles(&group, group.bounds(), &tolerances, false);
        let edges = handles.iter().filter(|(h, _)| {
            matches!(h, EditHandle::Resize(d) if !crate::transform_handle_layout::is_corner(*d))
        });
        assert_eq!(edges.count(), 0);
        assert_eq!(
            handles
                .iter()
                .filter(|(h, _)| matches!(h, EditHandle::Resize(_)))
                .count(),
            4
        );
    }

    /// Criteria 9 and 40: skew handles for a selection of paths only.
    #[test]
    fn only_a_selection_of_paths_has_skew_handles() {
        let document = Document::new(1);
        let a = line_path(&document, 1, Point::new(0.0, 0.0), Point::new(60.0, 40.0));
        let b = line_path(&document, 3, Point::new(70.0, 0.0), Point::new(90.0, 40.0));
        let plain = rect(&document, 0.0, 100.0, 10.0, 10.0);
        let tolerances = TransformHandleTolerances::at_scale(3.0);
        let skews = |ids: &[NodeId]| {
            let group = group_of(&document, ids).expect("group");
            group_handles(&group, group.bounds(), &tolerances, false)
                .iter()
                .filter(|(h, _)| matches!(h, EditHandle::Skew(_)))
                .count()
        };
        assert_eq!(skews(&[a, b]), 4);
        assert_eq!(skews(&[a, plain]), 0, "hidden, not inert");
    }
}
