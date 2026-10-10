//! `Session`'s Pen-tool glue (`specs/0002-path-node-editing`): finishing
//! the in-progress path, the pen's close-target cue and the pen's drag
//! threshold. Split out of `session/mod.rs` (`docs/technical-debt.md`,
//! "`Session` is one module past the size limit").

use std::cell::RefCell;

use curvyo_document_core::{DocumentVersion, Length, Point};
use curvyo_render_core::{ClosingCue, PenCue};
use curvyo_ui_core::{EndNodeIndex, JoinType, PenTarget, PressAction, continuation_of, pen_target};

use super::{Session, Tool};

/// What the Pen caches and remembers between events (`specs/0034-pen-path-extension`): the index
/// of the end nodes of open paths, rebuilt only when the document changes (criterion 24), and the
/// target the latest press was decided on, which the cue keeps until the release (a drag from a
/// target is ignored and the cursor stays what it was).
#[derive(Debug, Default)]
pub(super) struct PenCueState {
    ends: RefCell<Option<(DocumentVersion, EndNodeIndex)>>,
    press: Option<PenTarget>,
}

impl PenCueState {
    /// Forgets the target of a press that ended.
    pub(super) fn release(&mut self) {
        self.press = None;
    }
}

impl Session {
    pub(super) fn drag_threshold(&self) -> Length {
        Length::from_mm(self.transform_handle_tolerances().drag_threshold_mm)
    }

    /// Acceptance criterion 3 / the dedicated "finish path" action
    /// (Enter, or a double-click the host has already recognized).
    pub fn finish_pen(&mut self) {
        self.pen.finish(&self.document);
    }

    /// The in-progress pen path's placed nodes, for the host's
    /// rubber-band/live-curve preview — `None` when idle or the node
    /// tool is active.
    #[must_use]
    pub fn pen_in_progress(&self) -> Option<&[curvyo_document_core::NewAnchor]> {
        if self.tool == Tool::Pen {
            self.pen.in_progress_nodes()
        } else {
            None
        }
    }

    /// Runs `f` on the index of the end nodes of open ordinary paths, rebuilding it first if the
    /// document changed since it was built.
    pub(super) fn with_end_index<R>(&self, f: impl FnOnce(&EndNodeIndex) -> R) -> R {
        let version = self.document.version();
        let mut cache = self.pen_cue.ends.borrow_mut();
        if cache.as_ref().is_none_or(|(held, _)| *held != version) {
            *cache = Some((version, EndNodeIndex::from_paths(&self.paths())));
        }
        // invariant: the cache was filled just above.
        #[allow(clippy::unwrap_used)]
        f(&cache.as_ref().unwrap().1)
    }

    /// What a Pen press at the pointer would do now: while a press is down, what it was decided
    /// to do; otherwise the live target. `None` outside the Pen tool, off the canvas and while the
    /// eyedropper is picking (the pointer is picking a colour, not aiming a press).
    #[must_use]
    pub fn pen_target(&self) -> Option<PenTarget> {
        if self.tool != Tool::Pen || self.colour_pick_target().is_some() {
            return None;
        }
        if self.button_down {
            return self.pen_cue.press.clone();
        }
        let point = self.pointer_position?;
        let radius = self.point_tolerance_as_length();
        Some(
            self.with_end_index(|index| {
                pen_target(&self.pen, index, point, radius, self.held.shift)
            }),
        )
    }

    /// A Pen press at `point`: decide the action (`pen_target`), remember it for the cue, and begin
    /// the gesture.
    pub(super) fn pen_pointer_down(&mut self, point: Point, shift: bool) {
        let radius = self.point_tolerance_as_length();
        let target =
            self.with_end_index(|index| pen_target(&self.pen, index, point, radius, shift));
        let action = match &target {
            PenTarget::Continue(end) => self
                .paths()
                .iter()
                .find(|path| path.id == end.path)
                .map_or(PressAction::Place, |path| {
                    PressAction::Continue(continuation_of(end, path))
                }),
            PenTarget::Join(end) => PressAction::Join(end.clone()),
            PenTarget::Close { join, .. } => PressAction::Close(*join),
            PenTarget::Place | PenTarget::NewPathAt | PenTarget::PlaceOverEnd => PressAction::Place,
        };
        self.pen_cue.press = Some(target);
        self.pen.pointer_down_action(point, action);
    }

    /// The Pen's cue as render data: the end node a press would continue or join, and the closing
    /// segment of a close target (`0034` criteria 1, 7, 17).
    pub(super) fn pen_cue_data(&self) -> PenCue {
        let mut cue = PenCue::default();
        match self.pen_target() {
            Some(PenTarget::Continue(end) | PenTarget::Join(end)) => {
                cue.target = Some((end.anchor.point, end.anchor.kind));
            }
            Some(PenTarget::Close { join, .. }) => {
                cue.closing = self.pen.closing_preview(join).map(|preview| ClosingCue {
                    from: preview.from,
                    to: preview.to,
                    node: preview.closing_node,
                });
            }
            _ => {}
        }
        cue
    }

    /// The direction, as a unit vector in document space (y down, as on the screen), from the
    /// path in progress toward the side away from it at the target of a close or a join: where a
    /// hint chip can sit without covering the closing segment or the handles of the nodes it is
    /// about. `(0, 0)` when there is nothing to be away from.
    #[must_use]
    pub fn pen_cue_away(&self) -> (f64, f64) {
        let target = match self.pen_target() {
            Some(PenTarget::Join(end)) => end.anchor.point,
            Some(PenTarget::Close { .. }) => match self.pen_in_progress().and_then(<[_]>::first) {
                Some(first) => first.point,
                None => return (0.0, 0.0),
            },
            _ => return (0.0, 0.0),
        };
        let Some(nodes) = self.pen_in_progress() else {
            return (0.0, 0.0);
        };
        let others: Vec<_> = nodes.iter().filter(|node| node.point != target).collect();
        if others.is_empty() {
            return (0.0, 0.0);
        }
        #[allow(clippy::cast_precision_loss)]
        let count = others.len() as f64;
        let (sum_x, sum_y) = others.iter().fold((0.0, 0.0), |(x, y), node| {
            (x + node.point.x, y + node.point.y)
        });
        let (dx, dy) = (target.x - sum_x / count, target.y - sum_y / count);
        let length = dx.hypot(dy);
        if length < 1e-9 {
            (0.0, 0.0)
        } else {
            (dx / length, dy / length)
        }
    }

    /// The point the rubber band runs to: the target's node centre over a join target, nothing
    /// over a close target (the closing segment replaces it), else the pointer.
    pub(super) fn pen_rubber_band_end(&self) -> Option<Point> {
        match self.pen_target() {
            Some(PenTarget::Join(end)) => Some(end.anchor.point),
            Some(PenTarget::Close { .. }) => None,
            _ => self.pointer_position,
        }
    }

    /// Whether the continued path and the path of a join target differ in style (`0034` criterion
    /// 7's third chip line). `false` unless the Pen continues a path and its press target is a join.
    #[must_use]
    pub fn pen_join_style_differs(&self) -> bool {
        let Some(PenTarget::Join(end)) = self.pen_target() else {
            return false;
        };
        let Some(continuation) = self.pen.continuation() else {
            return false;
        };
        let (Some(own), Some(other)) = (
            self.document.path(continuation.path),
            self.document.path(end.path),
        ) else {
            return false;
        };
        own.style != other.style
    }

    /// The join a close target applies and the one "as drawn", for the chip.
    #[must_use]
    pub fn pen_close_joins(&self) -> Option<(JoinType, JoinType, bool)> {
        match self.pen_target()? {
            PenTarget::Close {
                join,
                as_drawn,
                shift,
            } => Some((join, as_drawn, shift)),
            _ => None,
        }
    }
}
