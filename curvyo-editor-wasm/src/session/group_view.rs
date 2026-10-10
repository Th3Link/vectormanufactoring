//! What `Session` shows for a multi-selection's group box
//! (`specs/0019-multi-object-transform/`): the box and the lighter member boxes,
//! the group handles with their pivot marker, the cursor and the hint chip of
//! the handle under the pointer. The single object's counterparts are in
//! `select_view.rs` and `select_cursor.rs`; this is a child module of `session`.

use std::collections::HashSet;

use curvyo_document_core::{NodeId, ObjectSnapshot, Point};
use curvyo_render_core::{
    GroupBoxKind, GroupDecorationInput, TransformDecorationInput, TransformHandleGlyph,
};
use curvyo_ui_core::{
    EditHandle, GroupBoxShape, GroupSelection, OrientedBox, SelectTool, format_degrees,
    group_handles, is_corner, is_drawn_group_handle, oriented_bounds,
};

use super::select_readout::skew_readout;
use super::select_view::glyph_kind;
use super::shapes::LiveReadout;
use super::{Session, Tool};

/// A release that moves or transforms at least this many selected objects may take
/// long enough for the host to show a `wait` cursor first (criteria 28, 46, 49).
const SLOW_RELEASE_OBJECTS: usize = 100;

/// More selected objects than this and no member box is drawn (criterion 5): a
/// count of the selection, so panning and zooming never change it.
const PER_OBJECT_BOX_LIMIT: usize = 500;

impl Session {
    /// The group of the selection over `objects`, in the Select tool only.
    fn group_in(&self, objects: &[ObjectSnapshot]) -> Option<GroupSelection> {
        if self.tool != Tool::Select {
            return None;
        }
        SelectTool::group_of(objects, &self.selection)
    }

    /// The group box as drawn this frame: the box at the press turned by the live
    /// angle about the live pivot during a rotate drag (criterion 28), else the
    /// group's own axis-aligned bounds, which a gesture's preview and a commit
    /// both leave axis-aligned (criterion 7).
    fn group_box_shown(&self, group: &GroupSelection) -> OrientedBox {
        self.pointer_position
            .and_then(|pointer| {
                self.select
                    .group_turned_box(pointer, self.held.shift, self.held.ctrl)
            })
            .unwrap_or(*group.bounds())
    }

    /// The group box and member boxes to draw this frame, from the live
    /// objects the caller built, so they follow the geometry a release would
    /// commit (criterion 29). `None` for fewer than two selected objects.
    pub(super) fn group_decoration_input_in(
        &self,
        objects: &[ObjectSnapshot],
    ) -> Option<GroupDecorationInput> {
        let group = self.group_in(objects)?;
        let box_ = self.group_box_shown(&group);
        let members = if self.selection.ids().len() > PER_OBJECT_BOX_LIMIT {
            Vec::new()
        } else {
            let selected: HashSet<NodeId> = self.selection.ids().iter().copied().collect();
            objects
                .iter()
                .filter(|object| selected.contains(&object.id()))
                .map(|object| oriented_bounds(object).document_corners())
                .collect()
        };
        Some(GroupDecorationInput {
            corners: box_.document_corners(),
            kind: match group.shape() {
                GroupBoxShape::Box => GroupBoxKind::Box,
                GroupBoxShape::Line => GroupBoxKind::Line,
                GroupBoxShape::Point => GroupBoxKind::Point,
            },
            members,
            canvas_px: self.viewport.canvas_size(),
            device_pixel_ratio: self.device_pixel_ratio,
            skew_guide: self.skew_guide_now(),
        })
    }

    /// The group handle under the pointer, if any, with the group: the one hit
    /// rule a press would use, plus the centre handle for hover. Only while no
    /// drag runs and no entry is open.
    pub(super) fn group_hovered_handle(
        &self,
        objects: &[ObjectSnapshot],
    ) -> Option<(GroupSelection, EditHandle)> {
        if self.tool != Tool::Select || self.select.drag_in_flight() || self.select.has_entry() {
            return None;
        }
        let point = self.pointer_position?;
        SelectTool::group_handle_at(
            objects,
            &self.selection,
            point,
            self.transform_handle_tolerances(),
            self.held.shift,
            true,
        )
    }

    /// The handles of the group box, the pivot marker and the skew guide for
    /// this frame, over the live objects (criteria 9, 10, 28, 29). The centre
    /// handle hides while a resize, rotate or skew drag runs or an entry is
    /// open: the pivot marker may live there.
    pub(super) fn group_transform_decoration_input_in(
        &self,
        objects: &[ObjectSnapshot],
    ) -> TransformDecorationInput {
        let Some(group) = self.group_in(objects) else {
            return TransformDecorationInput::default();
        };
        let tolerances = self.transform_handle_tolerances();
        let box_ = self.group_box_shown(&group);
        let highlighted = self
            .select
            .dragging_handle()
            .or_else(|| self.select.entry_handle());
        let hover = self.group_hovered_handle(objects);
        let hovered = hover.as_ref().map(|(_, handle)| *handle);
        let side_rotate = self.select.side_rotate_revealed(self.held.shift);
        let hide_center = self.select.centre_chip_open()
            || highlighted.is_some_and(|handle| handle != EditHandle::Move);
        let handles = group_handles(&group, &box_, &tolerances, side_rotate)
            .into_iter()
            .filter(|(handle, _)| {
                is_drawn_group_handle(*handle, &group, &tolerances)
                    && !(hide_center && *handle == EditHandle::Move)
            })
            .map(|(handle, position)| TransformHandleGlyph {
                position,
                kind: glyph_kind(handle, &box_),
                dragging: highlighted == Some(handle),
                hovered: hovered == Some(handle),
            })
            .collect();
        // Shift held, nothing running, the pointer on a handle: the marker
        // previews the point that handle would use.
        let preview = if self.held.shift && highlighted.is_none() {
            hover
                .as_ref()
                .and_then(|(group, handle)| SelectTool::group_hover_pivot(group, *handle, true))
        } else {
            None
        };
        TransformDecorationInput {
            handles,
            pivot_marker: self.select.live_pivot(self.held.shift).or(preview),
            pivot_marker_full: self.select.centre_chip_open(),
            skew_guide: self.skew_guide_now(),
            param_guides: Vec::new(),
            device_pixel_ratio: self.device_pixel_ratio,
        }
    }

    /// The lines of the hint chip of the group handle under the pointer
    /// (criterion 38), empty when there is none. The first line is the title.
    pub(super) fn group_hint_lines(&self, objects: &[ObjectSnapshot]) -> Vec<String> {
        let Some((group, handle)) = self.group_hovered_handle(objects) else {
            return Vec::new();
        };
        let lines: &[&str] = match handle {
            EditHandle::Move => &[
                "Move selection",
                "Shift: keep one axis",
                "Ctrl: copy",
                "Double-click or M: type an offset",
            ],
            EditHandle::Resize(direction) if !is_corner(direction) => &[
                "Resize selection",
                "Shift: from the center",
                "Double-click or S: type a size",
            ],
            // A corner of a selection that holds a shape a stretch converts is
            // always proportional (criterion 19); the line points to the edge
            // handle, which only exists to point to while it is drawn.
            EditHandle::Resize(_) if group.proportional_corners() => {
                let tolerances = self.transform_handle_tolerances();
                let edge_drawn = group_handles(&group, group.bounds(), &tolerances, false)
                    .into_iter()
                    .any(|(handle, _)| {
                        matches!(handle, EditHandle::Resize(d) if !is_corner(d))
                            && is_drawn_group_handle(handle, &group, &tolerances)
                    });
                let mut lines = vec!["Resize selection, proportional".to_string()];
                if edge_drawn {
                    lines.push("Stretch with an edge handle".to_string());
                }
                lines.extend(
                    ["Shift: from the center", "Double-click or S: type a size"].map(String::from),
                );
                return lines;
            }
            EditHandle::Resize(_) => &[
                "Resize selection",
                "Shift: from the center",
                "Ctrl: keep proportions",
                "Double-click or S: type a size",
            ],
            EditHandle::Rotate(direction) if is_corner(direction) => &[
                "Rotate selection",
                "Shift: pivot at opposite corner",
                "Ctrl: snap",
                "Double-click or R: type an angle",
            ],
            EditHandle::Rotate(_) => &[
                "Rotate selection",
                "Pivot: opposite side",
                "Ctrl: snap",
                "Double-click or R: type an angle",
            ],
            EditHandle::Skew(side) => {
                let key = if side.skews_along_u() { "K" } else { "Shift+K" };
                return vec![
                    "Skew selection".to_string(),
                    "Shift: from the center line".to_string(),
                    "Ctrl: snap".to_string(),
                    format!("Double-click or {key}: type an angle"),
                ];
            }
            EditHandle::Param(_) => return Vec::new(),
        };
        lines.iter().map(ToString::to_string).collect()
    }

    /// Whether the release of the drag in flight is expected to be slow: a move
    /// or a group resize, rotate or skew of many objects. The host then shows the
    /// `wait` cursor, lets it paint, and only then releases, with the preview
    /// still on screen (`specs/0019-multi-object-transform/` criteria 28, 46, 49).
    #[must_use]
    pub fn release_is_slow(&self) -> bool {
        self.tool == Tool::Select
            && (self.select.group_drag_in_flight() || self.select.move_in_flight())
            && self.selection.ids().len() >= SLOW_RELEASE_OBJECTS
    }

    /// The sentence a screen reader gets when a multi-selection changes: the
    /// count and the group box size ("4 objects selected, 46.2 by 18.7 mm"),
    /// empty for fewer than two selected objects
    /// (`specs/0019-multi-object-transform/` UX notes, section 13).
    #[must_use]
    pub fn selection_announcement(&self) -> String {
        // Before the document read: this runs on every sync.
        if self.tool != Tool::Select || self.selection.ids().len() < 2 {
            return String::new();
        }
        let objects = self.objects();
        let Some(group) = self.group_in(&objects) else {
            return String::new();
        };
        format!(
            "{} objects selected, {:.1} by {:.1} mm",
            group.count(),
            group.bounds().width(),
            group.bounds().height()
        )
    }

    /// The centre of the group box, where the origin axes of an axis-locked
    /// move run (criterion 17): the position of the centre handle, drawn or
    /// not. `None` for fewer than two selected objects.
    pub(super) fn group_centre(&self, objects: &[ObjectSnapshot]) -> Option<Point> {
        let group = self.group_in(objects)?;
        let box_ = group.bounds();
        Some(box_.to_document(box_.local_center()))
    }

    /// The live readout of a group resize, rotate or skew drag past the dead
    /// zone, at the pointer (criteria 23, 28, 42): the preview group box size
    /// ("123.4 mm × 67.8 mm"), the turn since the press ("Δ 37.4°", clockwise
    /// positive) or the skew ("Skew x +12.5°").
    pub(super) fn group_live_readout(&self) -> Option<LiveReadout> {
        let handle = self.select.dragging_handle()?;
        let anchor = self.pointer_position?;
        let (shift, ctrl) = (self.held.shift, self.held.ctrl);
        let text = match handle {
            EditHandle::Resize(_) => {
                if !self.select.group_drag_active_at(anchor) {
                    return None;
                }
                let objects = self.objects();
                let live = self.select_live_edit_in(&objects);
                let live_objects = Self::live_objects_in(&objects, live.as_ref());
                let group = SelectTool::group_of(&live_objects, &self.selection)?;
                format!(
                    "{:.1} mm \u{d7} {:.1} mm",
                    group.bounds().width(),
                    group.bounds().height()
                )
            }
            EditHandle::Rotate(_) => {
                let delta = self.select.live_group_rotation(anchor, shift, ctrl)?;
                format!(
                    "\u{394} {}",
                    signed_degrees(delta.as_radians().to_degrees())
                )
            }
            EditHandle::Skew(side) => {
                let angle = self.select.live_skew_angle(anchor, shift, ctrl)?;
                skew_readout(side, angle.as_radians().to_degrees())
            }
            EditHandle::Move | EditHandle::Param(_) => return None,
        };
        Some(LiveReadout { text, anchor })
    }
}

/// `37.4°` or `\u{2212}37.4°`: one decimal at most, a real minus sign (U+2212),
/// no sign for a value that rounds to zero.
fn signed_degrees(degrees: f64) -> String {
    let magnitude = format_degrees(degrees.abs());
    if degrees < 0.0 && magnitude != "0°" {
        format!("\u{2212}{magnitude}")
    } else {
        magnitude
    }
}
