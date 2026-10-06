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
    ResizeOptions, StrokeScaling, is_polygon_or_star, pivot_for, resize_by_local_delta, rotate_by,
};
use crate::transform_handle_layout::{TransformHandle, is_corner};
use crate::transform_math::{local_delta_for_radius, local_delta_for_size};
use vecmanf_document_core::PrimitiveSnapshot;

/// A typed size within this (millimetres) of the current one is "equal":
/// nothing is written (criteria 19, 31). The prefill is the readout's
/// rounded value, so this and the untouched-text rule are both needed.
const SIZE_EQUAL_EPSILON_MM: f64 = 1e-9;

/// A typed angle within this (radians) of the current rotation is equal.
const ANGLE_EQUAL_EPSILON_RAD: f64 = 1e-12;

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
    Radius,
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
    /// The accessible name ("Width", "Height", "Radius", "Angle").
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
    handle: TransformHandle,
    /// Shift at the second press: the fixed point and the rotate pivot.
    shift: bool,
    /// Width and height are linked (Ctrl at the second press on a corner
    /// of a rectangle, ellipse or path).
    linked: bool,
    stroke_scaling: StrokeScaling,
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
fn format_mm(value: f64) -> String {
    format!("{value:.1}")
}

/// Parses the text of an entry field (criteria 19, 21, 30): trimmed, an
/// optional sign, digits with at most one separator ("." or ","), and on
/// the angle field one optional trailing "°". Anything else (empty,
/// letters, "1,2,3", exponents, unit suffixes) is `None`, as is a value
/// that is not finite.
#[must_use]
pub fn parse_entry_number(text: &str, allow_degree: bool) -> Option<f64> {
    let mut rest = text.trim();
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
        let handle = TransformHandle::Rotate(direction);
        Self {
            kind: EntryKind::Angle,
            start: object.clone(),
            start_box: *box_,
            handle,
            shift,
            linked: false,
            stroke_scaling: StrokeScaling::Keep,
            side_rotate_revealed: !is_corner(direction),
            pivot: pivot_for(handle, object, box_, shift)
                .unwrap_or_else(|| box_.to_document(box_.local_center())),
            fields: vec![EntryField {
                label: "",
                accessible_name: "Angle",
                prefill: format_degrees(object.rotation().as_radians().to_degrees())
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
    /// second press; `stroke_scaling` is the switch's value now.
    #[must_use]
    pub fn for_resize(
        object: &ObjectSnapshot,
        box_: &OrientedBox,
        direction: ResizeDirection,
        modifiers: (bool, bool),
        stroke_scaling: StrokeScaling,
    ) -> Self {
        let (shift, ctrl) = modifiers;
        let handle = TransformHandle::Resize(direction);
        let radius_entry = is_polygon_or_star(object);
        let editable = |extent: f64| extent > SIZE_EQUAL_EPSILON_MM;
        let (kind, fields) = if let Some(radius) = outer_radius(object).filter(|_| radius_entry) {
            (
                EntryKind::Radius,
                vec![EntryField {
                    label: "r",
                    accessible_name: "Radius",
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
            stroke_scaling,
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
    pub const fn handle(&self) -> TransformHandle {
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
            FieldAxis::Angle => self.start.rotation().as_radians().to_degrees(),
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
            TransformHandle::Rotate(_) => {
                let Some(degrees) = target(FieldAxis::Angle) else {
                    return self.start.clone();
                };
                let change = degrees.to_radians() - self.start.rotation().as_radians();
                rotate_by(
                    &self.start,
                    self.pivot,
                    Angle::from_radians(change).normalized(),
                )
            }
            TransformHandle::Resize(direction) => {
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
                        stroke_scaling: self.stroke_scaling,
                    },
                )
            }
            TransformHandle::Skew(_) | TransformHandle::Move => self.start.clone(),
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
                commit_gesture(document, self.handle, &result, self.stroke_scaling);
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
