//! The typed numeric entry of a multi-selection: an angle, a size or a skew
//! (`specs/0019-multi-object-transform/` criteria 33, 34 and 36). Its state, the
//! rules of its fields and the resolution of a typed value into the same
//! [`GroupMap`] a drag of the same handle produces, so a typed value and a dragged
//! one cannot disagree (`adrs.md` decision 4). The DOM chip only holds the text,
//! the caret and the focus. The single object's counterparts are
//! [`crate::TransformEntry`] and [`crate::SkewEntry`].

use curvyo_document_core::{Angle, Document, NodeId, ObjectSnapshot, PathSnapshot, Point};

use crate::ResizeDirection;
use crate::conversion::{ConversionCounts, converted_counts, converted_paths};
use crate::group_box::GroupSelection;
use crate::group_transform::{GroupMap, commit_group, group_pivot, map_all_checked};
use crate::oriented_box::OrientedBox;
use crate::skew_math::{MIN_SKEW_LEVER_MM, skew_factor, skew_frame};
use crate::transform_commit::{MAX_COORDINATE_MM, same_within_tolerance};
use crate::transform_drag::ScaleModes;
use crate::transform_entry::{
    EntryField, EntryKind, EntryOutcome, InvalidReason, format_mm, parse_entry_number,
};
use crate::transform_handle_layout::{EditHandle, Side, is_corner};
use crate::transform_math::ANGLE_EQUAL_EPSILON_RAD;

/// A typed size within this (millimetres) of the current one is "equal": nothing
/// is written.
const SIZE_EQUAL_EPSILON_MM: f64 = 1e-9;

/// What a field of the chip edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Angle,
    Width,
    Height,
    Skew,
}

/// An open group entry: the selected objects and the group box as they were when
/// it opened and the fixed point fixed then, so later Shift, Ctrl or switch
/// changes change nothing.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // one flag per fact the entry fixes when it opens
pub struct GroupEntry {
    kind: EntryKind,
    starts: Vec<ObjectSnapshot>,
    ids: Vec<NodeId>,
    start_box: OrientedBox,
    handle: EditHandle,
    shift: bool,
    linked: bool,
    modes: ScaleModes,
    side_rotate_revealed: bool,
    centre_chip: bool,
    pivot: Point,
    fields: Vec<EntryField>,
    axes: Vec<Axis>,
    /// For a size entry, beside each start, the path it becomes if the typed size
    /// is a stretch (criterion 53); built when the entry opens.
    converted: Vec<Option<PathSnapshot>>,
}

fn field(label: &'static str, name: &'static str, prefill: String, editable: bool) -> EntryField {
    let mut field = EntryField::for_param(label, name, prefill);
    field.editable = editable;
    field
}

impl GroupEntry {
    /// An angle entry for rotate handle `direction` (criterion 33): one field "Δ",
    /// prefilled "0"; the pivot is the group box centre, or the opposite corner
    /// or side with `shift` at the second press.
    #[must_use]
    pub fn for_rotate(
        members: &[ObjectSnapshot],
        group: &GroupSelection,
        direction: ResizeDirection,
        shift: bool,
    ) -> Self {
        let handle = EditHandle::Rotate(direction);
        Self::new(
            EntryKind::Angle,
            members,
            group,
            (handle, shift),
            vec![(
                Axis::Angle,
                field("Δ", "Rotate selection by", "0".to_string(), true),
            )],
        )
        .with_side_rotate(!is_corner(direction))
    }

    /// A size entry for resize handle `direction` (criterion 34): "W" and "H"
    /// prefilled with the group box size, one of them for an edge handle.
    /// `modifiers` are Shift (the centre is the fixed point) and Ctrl (linked
    /// fields, one factor) at the second press; the two fields are otherwise
    /// independent, whatever the selection holds (criterion 34).
    #[must_use]
    pub fn for_resize(
        members: &[ObjectSnapshot],
        group: &GroupSelection,
        direction: ResizeDirection,
        modifiers: (bool, bool),
        modes: ScaleModes,
    ) -> Self {
        let (shift, ctrl) = modifiers;
        let box_ = group.bounds();
        let width = (
            Axis::Width,
            field(
                "W",
                "Width",
                format_mm(box_.width()),
                box_.width() > SIZE_EQUAL_EPSILON_MM,
            ),
        );
        let height = (
            Axis::Height,
            field(
                "H",
                "Height",
                format_mm(box_.height()),
                box_.height() > SIZE_EQUAL_EPSILON_MM,
            ),
        );
        let fields = match direction {
            ResizeDirection::E | ResizeDirection::W => vec![width],
            ResizeDirection::N | ResizeDirection::S => vec![height],
            _ => vec![width, height],
        };
        let linked = fields.len() == 2 && fields.iter().all(|(_, f)| f.editable) && ctrl;
        let mut entry = Self::new(
            EntryKind::Size,
            members,
            group,
            (EditHandle::Resize(direction), shift),
            fields,
        );
        entry.linked = linked;
        entry.modes = modes;
        entry.converted = converted_paths(&entry.starts);
        entry
    }

    /// A skew entry for skew handle `side` (criterion 36); `None` unless every
    /// selected object is a path.
    #[must_use]
    pub fn for_skew(
        members: &[ObjectSnapshot],
        group: &GroupSelection,
        side: Side,
        shift: bool,
    ) -> Option<Self> {
        if !group.all_paths() {
            return None;
        }
        let editable = skew_frame(group.bounds(), side, shift).lever.abs() >= MIN_SKEW_LEVER_MM;
        let name = if side.skews_along_u() {
            "Skew angle x"
        } else {
            "Skew angle y"
        };
        Some(Self::new(
            EntryKind::Skew,
            members,
            group,
            (EditHandle::Skew(side), shift),
            vec![(Axis::Skew, field("", name, "0".to_string(), editable))],
        ))
    }

    fn new(
        kind: EntryKind,
        members: &[ObjectSnapshot],
        group: &GroupSelection,
        (handle, shift): (EditHandle, bool),
        fields: Vec<(Axis, EntryField)>,
    ) -> Self {
        let start_box = *group.bounds();
        let (axes, fields) = fields.into_iter().unzip();
        Self {
            kind,
            ids: members.iter().map(ObjectSnapshot::id).collect(),
            starts: members.to_vec(),
            start_box,
            handle,
            shift,
            linked: false,
            modes: ScaleModes::default(),
            side_rotate_revealed: false,
            centre_chip: false,
            pivot: group_pivot(&start_box, handle, shift)
                .unwrap_or_else(|| start_box.to_document(start_box.local_center())),
            fields,
            axes,
            converted: Vec::new(),
        }
    }

    const fn with_side_rotate(mut self, revealed: bool) -> Self {
        self.side_rotate_revealed = revealed;
        self
    }

    /// Marks the entry as opened by the key S: the chip goes by the box centre,
    /// the fixed point is the centre and no handle is highlighted.
    #[must_use]
    pub(crate) fn with_centre_chip(mut self) -> Self {
        self.centre_chip = true;
        self.pivot = self.start_box.to_document(self.start_box.local_center());
        self.shift = true;
        self
    }

    /// What this entry edits: an angle, a size or a skew.
    #[must_use]
    pub const fn kind(&self) -> EntryKind {
        self.kind
    }

    /// The handle the entry belongs to.
    #[must_use]
    pub const fn handle(&self) -> EditHandle {
        self.handle
    }

    /// The ids of the selected objects, in selection order.
    #[must_use]
    pub fn ids(&self) -> &[NodeId] {
        &self.ids
    }

    /// The group box when the entry opened.
    #[must_use]
    pub const fn start_box(&self) -> &OrientedBox {
        &self.start_box
    }

    /// The fixed point shown by the pivot marker for as long as the entry is open.
    #[must_use]
    pub const fn pivot(&self) -> Point {
        self.pivot
    }

    /// Whether the side rotate handles were showing when it opened.
    #[must_use]
    pub const fn side_rotate_revealed(&self) -> bool {
        self.side_rotate_revealed
    }

    /// Whether the chip sits by the box centre (the key S).
    #[must_use]
    pub const fn centre_chip(&self) -> bool {
        self.centre_chip
    }

    /// The chip's fields: one or two.
    #[must_use]
    pub fn fields(&self) -> &[EntryField] {
        &self.fields
    }

    /// Whether the two fields are linked (the aspect ratio is kept).
    #[must_use]
    pub const fn linked(&self) -> bool {
        self.linked
    }

    /// The text for the other field after field `edited` changed to `text`, when
    /// the fields are linked: the aspect ratio of the box when the entry opened
    /// is kept. `None` if not linked or `text` is not a positive number yet.
    #[must_use]
    pub fn linked_text(&self, edited: usize, text: &str) -> Option<String> {
        if !self.linked {
            return None;
        }
        let value = parse_entry_number(text, false).filter(|v| *v > 0.0)?;
        let (width, height) = (self.start_box.width(), self.start_box.height());
        let other = match self.axes.get(edited)? {
            Axis::Width => value * height / width,
            Axis::Height => value * width / height,
            Axis::Angle | Axis::Skew => return None,
        };
        Some(format_mm(other))
    }

    /// The typed target of field `index`: `Ok(None)` when the text was not edited
    /// or equals the current value, `Ok(Some)` for a changed valid value.
    fn target_of(&self, index: usize, text: &str) -> Result<Option<f64>, InvalidReason> {
        let field = &self.fields[index];
        if !field.editable || text == field.prefill {
            return Ok(None);
        }
        let axis = self.axes[index];
        let value = parse_entry_number(text, axis != Axis::Width && axis != Axis::Height)
            .ok_or(InvalidReason::NotANumber)?;
        match axis {
            Axis::Angle | Axis::Skew => Ok(Some(value)),
            Axis::Width | Axis::Height => {
                if value <= 0.0 {
                    return Err(InvalidReason::NotPositive);
                }
                if value > MAX_COORDINATE_MM {
                    return Err(InvalidReason::NotANumber);
                }
                let start = if axis == Axis::Width {
                    self.start_box.width()
                } else {
                    self.start_box.height()
                };
                Ok(((value - start).abs() > SIZE_EQUAL_EPSILON_MM).then_some(value))
            }
        }
    }

    /// The map the typed `texts` mean, `Ok(None)` if nothing would change.
    fn map_of(
        &self,
        texts: [&str; 2],
        last_edited: usize,
    ) -> Result<Option<GroupMap>, (usize, InvalidReason)> {
        let considered: Vec<usize> = if self.linked {
            vec![last_edited.min(self.fields.len() - 1)]
        } else {
            (0..self.fields.len()).collect()
        };
        let mut targets: Vec<(Axis, f64)> = Vec::new();
        for index in considered {
            match self.target_of(index, texts[index]) {
                Ok(Some(value)) => targets.push((self.axes[index], value)),
                Ok(None) => {}
                Err(reason) => return Err((index, reason)),
            }
        }
        let Some(&(axis, value)) = targets.first() else {
            return Ok(None);
        };
        match self.kind {
            EntryKind::Angle => {
                let delta = Angle::from_radians(value.to_radians()).normalized();
                // A full turn is the same rotation.
                Ok(
                    (delta.as_radians().abs() > ANGLE_EQUAL_EPSILON_RAD).then_some(
                        GroupMap::Rotate {
                            pivot: self.pivot,
                            delta,
                        },
                    ),
                )
            }
            EntryKind::Size => Ok(Some(self.scale_map(&targets, axis, value))),
            EntryKind::Skew => {
                if value.abs() >= 90.0 {
                    return Err((0, InvalidReason::SkewRange));
                }
                let EditHandle::Skew(side) = self.handle else {
                    return Ok(None);
                };
                let angle = Angle::from_radians(value.to_radians());
                if angle.as_radians().abs() <= ANGLE_EQUAL_EPSILON_RAD {
                    return Ok(None);
                }
                let frame = skew_frame(&self.start_box, side, self.shift);
                let k = skew_factor(&frame, angle);
                let (ku, kv) = if frame.along_u { (k, 0.0) } else { (0.0, k) };
                Ok(Some(GroupMap::Shear {
                    fixed: frame.fixed_point,
                    ku,
                    kv,
                }))
            }
            EntryKind::CornerRadius | EntryKind::InnerRatio => Ok(None),
        }
    }

    /// The scale a typed size means: about the fixed point, linked fields by the
    /// one factor of the field edited last.
    fn scale_map(&self, targets: &[(Axis, f64)], axis: Axis, value: f64) -> GroupMap {
        let (width, height) = (self.start_box.width(), self.start_box.height());
        let factor_of = |wanted: Axis| {
            targets
                .iter()
                .find(|(a, _)| *a == wanted)
                .map(|(_, v)| *v / if wanted == Axis::Width { width } else { height })
        };
        let (sx, sy) = if self.linked {
            let factor = value / if axis == Axis::Width { width } else { height };
            (factor, factor)
        } else {
            (
                factor_of(Axis::Width).unwrap_or(1.0),
                factor_of(Axis::Height).unwrap_or(1.0),
            )
        };
        GroupMap::Scale {
            pivot: self.pivot,
            sx,
            sy,
        }
    }

    /// The paths this entry would convert shapes into (`None` per object that keeps
    /// its kind), for the session to give real anchor ids.
    pub(crate) fn converted_mut(&mut self) -> &mut [Option<PathSnapshot>] {
        &mut self.converted
    }

    /// The shapes the typed values would turn into paths, counted by kind: empty
    /// unless the typed size is a stretch (criterion 55's note under the fields,
    /// "Turns 2 shapes into paths.") or a text is refused.
    #[must_use]
    pub fn conversion_counts(&self, texts: [&str; 2], last_edited: usize) -> ConversionCounts {
        match self.resolve(texts, last_edited) {
            Ok(Some(results)) => converted_counts(&self.starts, &results),
            _ => ConversionCounts::default(),
        }
    }

    /// The objects after applying the typed values, `Ok(None)` if nothing would
    /// change, or the first refused field. Pure: the snapshots a drag to the same
    /// value with the same handle and modifiers would produce.
    ///
    /// # Errors
    /// The index and reason of the first field whose text was refused, or
    /// `TooLarge` when a result would pass the coordinate limit.
    pub fn resolve(
        &self,
        texts: [&str; 2],
        last_edited: usize,
    ) -> Result<Option<Vec<ObjectSnapshot>>, (usize, InvalidReason)> {
        let Some(map) = self.map_of(texts, last_edited)? else {
            return Ok(None);
        };
        let results = map_all_checked(&self.starts, &self.converted, &map, self.modes)
            .ok_or((0, InvalidReason::TooLarge))?;
        let unchanged = results
            .iter()
            .zip(&self.starts)
            .all(|(new, old)| same_within_tolerance(new, old));
        Ok((!unchanged).then_some(results))
    }

    /// Validates the typed values and, if valid and changed, writes them as one
    /// commit, the one a drag writes. An entry whose objects changed or vanished
    /// since it opened writes nothing.
    #[must_use]
    pub fn commit(
        &self,
        document: &Document,
        texts: [&str; 2],
        last_edited: usize,
    ) -> EntryOutcome {
        self.commit_counting(document, texts, last_edited).0
    }

    /// [`GroupEntry::commit`], and the shapes the commit turned into paths (empty
    /// unless it was committed and converted something).
    #[must_use]
    pub fn commit_counting(
        &self,
        document: &Document,
        texts: [&str; 2],
        last_edited: usize,
    ) -> (EntryOutcome, ConversionCounts) {
        let current = self
            .starts
            .iter()
            .all(|object| document.object(object.id()).as_ref() == Some(object));
        if !current {
            return (EntryOutcome::Unchanged, ConversionCounts::default());
        }
        match self.resolve(texts, last_edited) {
            Err((field, reason)) => (
                EntryOutcome::Invalid { field, reason },
                ConversionCounts::default(),
            ),
            Ok(None) => (EntryOutcome::Unchanged, ConversionCounts::default()),
            Ok(Some(results)) => {
                if commit_group(document, &results, self.modes.stroke) {
                    (
                        EntryOutcome::Committed,
                        converted_counts(&self.starts, &results),
                    )
                } else {
                    (EntryOutcome::Unchanged, ConversionCounts::default())
                }
            }
        }
    }
}
