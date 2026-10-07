//! The typed numeric entry for an angle, a size or a radius
//! (`specs/object-transform-refinements/specification.md`, acceptance
//! criteria 18-32): its state, the number parser, the linked width/height
//! rule, and the resolution of a typed target into the same inputs a drag
//! produces, so a typed value and a dragged value never disagree
//! (`adrs.md`, "one resolving function per gesture"). The DOM chip only
//! holds the text, the caret and the focus; every rule lives here.

use vecmanf_document_core::{Angle, Document, ObjectSnapshot, Point, Shape};

use crate::ResizeDirection;
use crate::oriented_box::OrientedBox;
use crate::transform_commit::{MAX_COORDINATE_MM, commit_gesture};
use crate::transform_drag::{
    ResizeOptions, ScaleModes, pivot_for, resize_by_local_delta, rotate_by,
};
use crate::transform_handle_layout::{EditHandle, is_corner};
use crate::transform_math::{
    ANGLE_EQUAL_EPSILON_RAD, is_polygon_or_star, local_delta_for_radius, local_delta_for_size,
};
use vecmanf_document_core::PrimitiveSnapshot;

/// A typed size within this (millimetres) of the current one is "equal":
/// nothing is written (criteria 19, 31). The prefill is the readout's
/// rounded value, so this and the untouched-text rule are both needed.
const SIZE_EQUAL_EPSILON_MM: f64 = 1e-9;

impl EntryField {
    /// The single field of a parameter or skew entry.
    pub(crate) fn for_param(
        label: &'static str,
        accessible_name: &'static str,
        prefill: String,
    ) -> Self {
        Self {
            label,
            accessible_name,
            prefill,
            editable: true,
            axis: FieldAxis::Radius,
        }
    }
}

/// Which kind of value an entry edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// One field: the absolute rotation in degrees (criteria 18, 19).
    Angle,
    /// One or two fields: the width and/or height along the object's own
    /// axes, in millimetres (criterion 25).
    Size,
    /// One field: a polygon or star's outer radius, in millimetres
    /// (criterion 26).
    OuterRadius,
    /// One field: a rectangle's corner radius, in millimetres
    /// (`specs/unified-object-editing/`, criterion 18).
    CornerRadius,
    /// One field: a star's inner ratio (criterion 19).
    InnerRatio,
    /// One field: a path's skew angle in degrees.
    Skew,
}

/// What a field edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldAxis {
    Angle,
    Width,
    Height,
    Radius,
}

/// One field of the entry chip, as the DOM renders it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryField {
    /// The visible label ("W", "H", "r"; empty for the angle, whose "°" is
    /// a fixed suffix).
    pub label: &'static str,
    /// The accessible name ("Width", "Height", "Outer radius", "Angle").
    pub accessible_name: &'static str,
    /// The text the field opens with: the live readout's rounded value.
    pub prefill: String,
    /// Whether the field can be edited; `false` for the zero-extent axis of
    /// a path, which a drag cannot scale either.
    pub editable: bool,
    axis: FieldAxis,
}

/// Why a field was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidReason {
    /// Empty, letters, more than one separator, not finite, or out of
    /// range ("Enter a number").
    NotANumber,
    /// A size of zero or less ("Must be above 0").
    NotPositive,
    /// A corner radius below zero ("Must be 0 or more").
    Negative,
    /// An inner ratio outside 0.01 to 0.99 ("Must be 0.01 to 0.99").
    RatioRange,
    /// A skew angle of 90° or more in size ("Must be between -90 and 90").
    SkewRange,
    /// A skew that would pass the coordinate limit ("Too large").
    TooLarge,
}

/// What [`TransformEntry::commit`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryOutcome {
    /// One commit was made.
    Committed,
    /// Nothing was written: untouched text, an equal value, or a stale
    /// object. The entry closes.
    Unchanged,
    /// A field was refused: nothing was written and the entry stays open.
    Invalid {
        /// Index of the first offending field.
        field: usize,
        /// Why.
        reason: InvalidReason,
    },
}

/// An open numeric entry: the object and box as they were when it opened
/// and the rule fixed then (pivot or fixed point, Ctrl link, the stroke
/// switch's value), so later Shift, Ctrl or switch changes change nothing
/// (criteria 22, 28, 31).
#[derive(Debug, Clone)]
pub struct TransformEntry {
    kind: EntryKind,
    start: ObjectSnapshot,
    start_box: OrientedBox,
    handle: EditHandle,
    /// Shift at the second press: the fixed point and the rotate pivot.
    shift: bool,
    /// Width and height are linked (Ctrl at the second press on a corner
    /// of a rectangle, ellipse or path).
    linked: bool,
    modes: ScaleModes,
    side_rotate_revealed: bool,
    pivot: Point,
    fields: Vec<EntryField>,
}

/// `37.4°`, or `45°` when the value is whole (exact under Ctrl's snap):
/// one decimal at most, never `-0.0°`. The one formatter of the rotate
/// readout and the angle entry's prefill, so the two agree.
#[must_use]
pub fn format_degrees(degrees: f64) -> String {
    if degrees.abs() < 0.05 {
        return "0°".to_string();
    }
    if (degrees - degrees.round()).abs() < 1e-6 {
        format!("{:.0}°", degrees.round() + 0.0)
    } else {
        format!("{degrees:.1}°")
    }
}

/// The one decimal a size readout and a size field prefill show.
pub(crate) fn format_mm(value: f64) -> String {
    format!("{value:.1}")
}

/// Parses the text of an entry field (criteria 19, 21, 30): trimmed, an
/// optional sign ("+", "-" or the real minus U+2212), digits with at most one separator ("." or ","), and on
/// the angle field one optional trailing "°". Anything else (empty,
/// letters, "1,2,3", exponents, unit suffixes) is `None`, as is a value
/// that is not finite.
#[must_use]
pub fn parse_entry_number(text: &str, allow_degree: bool) -> Option<f64> {
    // The move readout prints a real minus sign (U+2212): a value copied from
    // it parses (`edit-interaction-polish`, flag 6).
    let normalized = text.replace('\u{2212}', "-");
    let mut rest = normalized.trim();
    if allow_degree {
        rest = rest.strip_suffix('°').unwrap_or(rest).trim_end();
    }
    let unsigned = rest.strip_prefix(['+', '-']).map_or(rest, str::trim_start);
    let negative = rest.starts_with('-');
    let separators = unsigned.chars().filter(|c| matches!(c, '.' | ',')).count();
    let digits = unsigned.chars().filter(char::is_ascii_digit).count();
    if separators > 1
        || digits == 0
        || unsigned.chars().count() != digits + separators
        || !unsigned
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ','))
    {
        return None;
    }
    let value: f64 = unsigned.replace(',', ".").parse().ok()?;
    let value = if negative { -value } else { value };
    value.is_finite().then_some(value)
}

impl TransformEntry {
    /// An angle entry for rotate handle `direction` (criteria 18, 22): the
    /// pivot is the opposite corner or side midpoint when `shift` was held
    /// at the second press, else the box center.
    #[must_use]
    pub fn for_rotate(
        object: &ObjectSnapshot,
        box_: &OrientedBox,
        direction: ResizeDirection,
        shift: bool,
    ) -> Self {
        let handle = EditHandle::Rotate(direction);
        Self {
            kind: EntryKind::Angle,
            start: object.clone(),
            start_box: *box_,
            handle,
            shift,
            linked: false,
            modes: ScaleModes::default(),
            side_rotate_revealed: !is_corner(direction),
            pivot: pivot_for(handle, object, box_, shift)
                .unwrap_or_else(|| box_.to_document(box_.local_center())),
            fields: vec![EntryField {
                label: "",
                accessible_name: "Angle",
                prefill: format_degrees(object.orientation().as_radians().to_degrees())
                    .trim_end_matches('°')
                    .to_string(),
                editable: true,
                axis: FieldAxis::Angle,
            }],
        }
    }

    /// A size entry for resize handle `direction` (criteria 25-29):
    /// `modifiers` are Shift (fixed point = box center) and Ctrl (linked
    /// width/height on a corner of a rectangle, ellipse or path) at the
    /// second press; `modes` are the two switches' values now.
    #[must_use]
    pub fn for_resize(
        object: &ObjectSnapshot,
        box_: &OrientedBox,
        direction: ResizeDirection,
        modifiers: (bool, bool),
        modes: ScaleModes,
    ) -> Self {
        let (shift, ctrl) = modifiers;
        let handle = EditHandle::Resize(direction);
        let radius_entry = is_polygon_or_star(object);
        let editable = |extent: f64| extent > SIZE_EQUAL_EPSILON_MM;
        let (kind, fields) = if let Some(radius) = outer_radius(object).filter(|_| radius_entry) {
            (
                EntryKind::OuterRadius,
                vec![EntryField {
                    label: "r",
                    accessible_name: "Outer radius",
                    prefill: format_mm(radius),
                    editable: editable(radius),
                    axis: FieldAxis::Radius,
                }],
            )
        } else {
            let width = EntryField {
                label: "W",
                accessible_name: "Width",
                prefill: format_mm(box_.width()),
                editable: editable(box_.width()),
                axis: FieldAxis::Width,
            };
            let height = EntryField {
                label: "H",
                accessible_name: "Height",
                prefill: format_mm(box_.height()),
                editable: editable(box_.height()),
                axis: FieldAxis::Height,
            };
            let fields = match direction {
                ResizeDirection::E | ResizeDirection::W => vec![width],
                ResizeDirection::N | ResizeDirection::S => vec![height],
                _ => vec![width, height],
            };
            (EntryKind::Size, fields)
        };
        let linked = ctrl
            && kind == EntryKind::Size
            && fields.len() == 2
            && fields.iter().all(|f| f.editable);
        Self {
            kind,
            start: object.clone(),
            start_box: *box_,
            handle,
            shift,
            linked,
            modes,
            side_rotate_revealed: false,
            pivot: pivot_for(handle, object, box_, shift)
                .unwrap_or_else(|| box_.to_document(box_.local_center())),
            fields,
        }
    }

    /// What this entry edits.
    #[must_use]
    pub const fn kind(&self) -> EntryKind {
        self.kind
    }

    /// The handle the entry belongs to (it keeps its dragging look while
    /// the chip is open).
    #[must_use]
    pub const fn handle(&self) -> EditHandle {
        self.handle
    }

    /// The object being edited.
    #[must_use]
    pub fn object(&self) -> &ObjectSnapshot {
        &self.start
    }

    /// The box the object had when the entry opened.
    #[must_use]
    pub const fn start_box(&self) -> &OrientedBox {
        &self.start_box
    }

    /// The fixed point shown by the pivot marker for as long as the entry is
    /// open (criteria 22, 28).
    #[must_use]
    pub const fn pivot(&self) -> Point {
        self.pivot
    }

    /// Whether the side rotate handles were showing when it opened (the
    /// entry's own side handle must stay on screen).
    #[must_use]
    pub const fn side_rotate_revealed(&self) -> bool {
        self.side_rotate_revealed
    }

    /// The chip's fields: one or two.
    #[must_use]
    pub fn fields(&self) -> &[EntryField] {
        &self.fields
    }

    /// Whether the two fields are linked (criterion 29).
    #[must_use]
    pub const fn linked(&self) -> bool {
        self.linked
    }

    /// The text for the *other* field after field `edited` changed to
    /// `text`, when the fields are linked (criterion 29): the aspect ratio
    /// the box had when the entry opened is kept. `None` if not linked or
    /// `text` is not a positive number yet.
    #[must_use]
    pub fn linked_text(&self, edited: usize, text: &str) -> Option<String> {
        if !self.linked {
            return None;
        }
        let value = parse_entry_number(text, false).filter(|v| *v > 0.0)?;
        let (width, height) = (self.start_box.width(), self.start_box.height());
        let other = match self.fields.get(edited)?.axis {
            FieldAxis::Width => value * height / width,
            FieldAxis::Height => value * width / height,
            FieldAxis::Angle | FieldAxis::Radius => return None,
        };
        Some(format_mm(other))
    }

    /// The start value a field edits, in the unit of its text (degrees or
    /// millimetres).
    fn start_value(&self, axis: FieldAxis) -> f64 {
        match axis {
            FieldAxis::Angle => self.start.orientation().as_radians().to_degrees(),
            FieldAxis::Width => self.start_box.width(),
            FieldAxis::Height => self.start_box.height(),
            FieldAxis::Radius => outer_radius(&self.start).unwrap_or(0.0),
        }
    }

    /// The typed target of field `index`: `Ok(None)` when the text was not
    /// edited or equals the current value (it writes nothing), `Ok(Some)`
    /// for a changed valid value, `Err` for a refused one.
    fn target_of(&self, index: usize, text: &str) -> Result<Option<f64>, InvalidReason> {
        let field = &self.fields[index];
        if !field.editable || text == field.prefill {
            return Ok(None);
        }
        let is_angle = field.axis == FieldAxis::Angle;
        let value = parse_entry_number(text, is_angle).ok_or(InvalidReason::NotANumber)?;
        if is_angle {
            let change = (value - self.start_value(field.axis)).to_radians();
            // A full turn is the same rotation.
            let wrapped = Angle::from_radians(change).normalized().as_radians();
            return Ok((wrapped.abs() > ANGLE_EQUAL_EPSILON_RAD).then_some(value));
        }
        if value <= 0.0 {
            return Err(InvalidReason::NotPositive);
        }
        if value > MAX_COORDINATE_MM {
            return Err(InvalidReason::NotANumber);
        }
        let equal = (value - self.start_value(field.axis)).abs() <= SIZE_EQUAL_EPSILON_MM;
        Ok((!equal).then_some(value))
    }

    /// The object after applying the typed values, `Ok(None)` if nothing
    /// would change (criteria 19, 31), or the first refused field
    /// (criteria 21, 30). Pure: the same snapshot a drag to the same value
    /// with the same handle and modifiers would produce (criteria 16, 27).
    ///
    /// # Errors
    /// The index and reason of the first field whose text was refused.
    pub fn resolve(
        &self,
        texts: [&str; 2],
        last_edited: usize,
    ) -> Result<Option<ObjectSnapshot>, (usize, InvalidReason)> {
        // Linked fields: only the field edited last carries a target; the
        // other one is derived from it and not read.
        let considered: Vec<usize> = if self.linked {
            vec![last_edited.min(self.fields.len() - 1)]
        } else {
            (0..self.fields.len()).collect()
        };
        let mut targets: Vec<(FieldAxis, f64)> = Vec::new();
        for index in considered {
            match self.target_of(index, texts[index]) {
                Ok(Some(value)) => targets.push((self.fields[index].axis, value)),
                Ok(None) => {}
                Err(reason) => return Err((index, reason)),
            }
        }
        if targets.is_empty() {
            return Ok(None);
        }
        let result = self.apply(&targets);
        Ok((result != self.start).then_some(result))
    }

    /// The typed `targets` turned into the same inputs a drag produces.
    fn apply(&self, targets: &[(FieldAxis, f64)]) -> ObjectSnapshot {
        let target = |wanted: FieldAxis| {
            targets
                .iter()
                .find(|(axis, _)| *axis == wanted)
                .map(|(_, value)| *value)
        };
        match self.handle {
            EditHandle::Rotate(_) => {
                let Some(degrees) = target(FieldAxis::Angle) else {
                    return self.start.clone();
                };
                let change = degrees.to_radians() - self.start.orientation().as_radians();
                rotate_by(
                    &self.start,
                    self.pivot,
                    Angle::from_radians(change).normalized(),
                )
            }
            EditHandle::Resize(direction) => {
                let delta = if let Some(radius) = target(FieldAxis::Radius) {
                    local_delta_for_radius(self.start_value(FieldAxis::Radius), direction, radius)
                } else {
                    local_delta_for_size(
                        &self.start_box,
                        direction,
                        target(FieldAxis::Width),
                        target(FieldAxis::Height),
                        self.shift,
                    )
                };
                resize_by_local_delta(
                    &self.start,
                    &self.start_box,
                    direction,
                    delta,
                    ResizeOptions {
                        shift: self.shift,
                        ctrl: self.linked,
                        modes: self.modes,
                    },
                )
            }
            EditHandle::Skew(_) | EditHandle::Move | EditHandle::Param(_) => self.start.clone(),
        }
    }

    /// Validates the typed values and, if valid and changed, writes them as
    /// one commit (criteria 19, 21, 27, 30, 31). An entry whose object
    /// changed or vanished since it opened (a collaborator's edit, an undo)
    /// writes nothing.
    #[must_use]
    pub fn commit(
        &self,
        document: &Document,
        texts: [&str; 2],
        last_edited: usize,
    ) -> EntryOutcome {
        if document.object(self.start.id()).as_ref() != Some(&self.start) {
            return EntryOutcome::Unchanged;
        }
        match self.resolve(texts, last_edited) {
            Err((field, reason)) => EntryOutcome::Invalid { field, reason },
            Ok(None) => EntryOutcome::Unchanged,
            Ok(Some(result)) => {
                commit_gesture(document, self.handle, &result, self.modes);
                EntryOutcome::Committed
            }
        }
    }
}

/// A polygon or star's outer radius, in millimetres.
fn outer_radius(object: &ObjectSnapshot) -> Option<f64> {
    match object {
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Polygon { frame, .. } | Shape::Star { frame, .. },
            ..
        }) => Some(frame.radius.as_mm()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    /// A value copied from the move readout (real minus U+2212) parses like
    /// the ASCII sign; exponent notation and a minus inside the number stay
    /// rejected.
    #[test]
    fn the_real_minus_sign_parses_and_exponents_do_not() {
        assert_eq!(parse_entry_number("\u{2212}3.5", false), Some(-3.5));
        assert_eq!(parse_entry_number(" \u{2212} 3,5 ", false), Some(-3.5));
        assert_eq!(parse_entry_number("\u{2212}12\u{b0}", true), Some(-12.0));
        assert_eq!(parse_entry_number("-3.5", false), Some(-3.5));
        for bad in [
            "1e3",
            "1E-3",
            "3\u{2212}5",
            "\u{2212}\u{2212}3",
            "\u{2212}",
            "3-",
        ] {
            assert_eq!(parse_entry_number(bad, false), None, "{bad:?}");
        }
    }

    use vecmanf_document_core::{InnerRatio, Length, PointCount, StarFrame};

    use super::*;
    use crate::oriented_box::oriented_bounds;

    fn created(frame_angle_deg: f64, rotation_deg: f64, star: bool) -> (Document, ObjectSnapshot) {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(100.0, 50.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(frame_angle_deg.to_radians()),
        };
        let count = PointCount::new(5).expect("count");
        let id = if star {
            document.create_star(frame, count, InnerRatio::new(0.5).expect("ratio"))
        } else {
            document.create_polygon(frame, count)
        };
        if rotation_deg != 0.0 {
            let turned = document
                .object(id)
                .expect("exists")
                .rotated(frame.center, Angle::from_radians(rotation_deg.to_radians()));
            document.rotate_object(&turned).expect("rotates");
        }
        let object = document.object(id).expect("exists");
        (document, object)
    }

    fn angle_entry(object: &ObjectSnapshot) -> TransformEntry {
        TransformEntry::for_rotate(object, &oriented_bounds(object), ResizeDirection::Ne, false)
    }

    /// The first vertex of the drawn outline, in document space.
    fn first_vertex(object: &ObjectSnapshot) -> Point {
        let ObjectSnapshot::Primitive(primitive) = object else {
            panic!("a primitive");
        };
        vecmanf_document_core::outline_of_rotated(&primitive.shape, primitive.rotation)[0].point
    }

    /// Criteria 1 and 7: the angle entry opens on the shape's real
    /// orientation (frame angle plus rotation), not on the register alone.
    #[test]
    fn the_angle_prefill_of_a_polygon_or_star_is_its_orientation() {
        for star in [false, true] {
            let (_, object) = created(78.7, 0.0, star);
            assert_eq!(angle_entry(&object).fields()[0].prefill, "78.7");
            let (_, object) = created(10.0, 30.0, star);
            assert_eq!(angle_entry(&object).fields()[0].prefill, "40");
            let (_, object) = created(-15.0, 0.0, star);
            assert_eq!(angle_entry(&object).fields()[0].prefill, "-15");
        }
    }

    /// Criterion 7: typing an angle A makes the shown angle A, and 0 puts the
    /// first vertex straight right of the centre, from any starting angle.
    #[test]
    fn a_typed_angle_becomes_the_shown_angle_and_zero_points_the_first_vertex_right() {
        for star in [false, true] {
            for (frame_angle, rotation) in [(78.7, 0.0), (10.0, 30.0), (-170.0, 25.0)] {
                for (typed, expected) in
                    [("0", 0.0), ("45", 45.0), ("-120", -120.0), ("180", 180.0)]
                {
                    let (document, object) = created(frame_angle, rotation, star);
                    let entry = angle_entry(&object);
                    assert_eq!(
                        entry.commit(&document, [typed, ""], 0),
                        EntryOutcome::Committed,
                        "{frame_angle}/{rotation} typed {typed}"
                    );
                    let after = document.object(object.id()).expect("exists");
                    let shown = after.orientation().as_radians().to_degrees();
                    assert!(
                        (shown - expected).abs() < 1e-9,
                        "{frame_angle}/{rotation} typed {typed}: shown {shown}"
                    );
                    let vertex = first_vertex(&after);
                    let want = Point::new(
                        100.0 + 10.0 * expected.to_radians().cos(),
                        50.0 + 10.0 * expected.to_radians().sin(),
                    );
                    assert!(
                        (vertex.x - want.x).abs() < 1e-9 && (vertex.y - want.y).abs() < 1e-9,
                        "{frame_angle}/{rotation} typed {typed}: vertex {vertex:?}, want {want:?}"
                    );
                }
            }
        }
    }

    /// Typing the angle already shown writes nothing (criterion 19 of the
    /// refinements, now measured on the shown angle).
    #[test]
    fn typing_the_shown_angle_writes_nothing() {
        let (document, object) = created(78.7, 0.0, false);
        let entry = angle_entry(&object);
        assert_eq!(
            entry.commit(&document, ["78.7", ""], 0),
            EntryOutcome::Unchanged
        );
        let (document, object) = created(10.0, 30.0, false);
        let entry = angle_entry(&object);
        assert_eq!(
            entry.commit(&document, ["40", ""], 0),
            EntryOutcome::Unchanged
        );
        assert_eq!(document.object(object.id()), Some(object));
    }

    /// Rectangles, ellipses and paths still show their register.
    #[test]
    fn a_rectangle_still_shows_its_rotation_register() {
        use vecmanf_document_core::RectBounds;
        let document = Document::new(1);
        let id = document.create_rect(RectBounds::from_corners(
            Point::new(0.0, 0.0),
            Point::new(20.0, 10.0),
        ));
        let turned = document.object(id).expect("exists").rotated(
            Point::new(10.0, 5.0),
            Angle::from_radians(30.0_f64.to_radians()),
        );
        document.rotate_object(&turned).expect("rotates");
        let object = document.object(id).expect("exists");
        assert_eq!(angle_entry(&object).fields()[0].prefill, "30");
    }
}
