//! The arrow-key nudge of the selection: how far an event moves it and which events form one run
//! (`specs/0044-editing-quick-wins/` criteria 7 and 9, `adrs.md` decisions 2 to 4).

use std::collections::HashSet;

use curvyo_document_core::{
    Document, DocumentVersion, Length, NodeId, ObjectSnapshot, Point, Vec2,
};

use crate::anchor_id_minter::AnchorIdMinter;
use crate::object_bounds::object_outline_bounds;
use crate::transform_commit::{commit_move, offset_within_limit};

/// One plain arrow key moves the selection this far, in document millimetres, whatever the zoom
/// and the display unit.
pub(crate) const NUDGE: Length = Length::from_mm(1.0);

/// One Shift+arrow key moves the selection this far.
pub(crate) const NUDGE_LARGE: Length = Length::from_mm(10.0);

/// A repeat event joins the run of the previous nudge only when it arrives within this many
/// milliseconds of it.
pub const CONTINUATION_WINDOW_MS: f64 = 600.0;

/// The four arrow keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    /// Towards smaller x.
    Left,
    /// Towards larger x.
    Right,
    /// Towards smaller y (document Y grows downward).
    Up,
    /// Towards larger y.
    Down,
}

impl Arrow {
    /// The arrow named by a DOM `KeyboardEvent.key`, or `None` for any other key.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "ArrowLeft" => Some(Self::Left),
            "ArrowRight" => Some(Self::Right),
            "ArrowUp" => Some(Self::Up),
            "ArrowDown" => Some(Self::Down),
            _ => None,
        }
    }
}

/// One nudge key event, as far as the run rule cares.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NudgeEvent {
    /// The arrow key.
    pub arrow: Arrow,
    /// Shift is down: the large distance.
    pub shift: bool,
    /// The event is an auto-repeat of a held key.
    pub repeat: bool,
    /// `KeyboardEvent.timeStamp`, milliseconds on the DOM's own clock.
    pub time_ms: f64,
}

impl NudgeEvent {
    /// How far this event moves the selection.
    #[must_use]
    pub fn offset(self) -> Vec2 {
        let distance = if self.shift { NUDGE_LARGE } else { NUDGE }.as_mm();
        match self.arrow {
            Arrow::Left => Vec2::new(-distance, 0.0),
            Arrow::Right => Vec2::new(distance, 0.0),
            Arrow::Up => Vec2::new(0.0, -distance),
            Arrow::Down => Vec2::new(0.0, distance),
        }
    }
}

impl NudgeEvent {
    /// Whether moving objects with the tight bounds `bounds` (`(min, max)` corners) by this event
    /// keeps every edge within the document's coordinate limit, the rule of a typed move.
    #[must_use]
    pub(crate) fn fits(self, bounds: (Point, Point)) -> bool {
        offset_within_limit(bounds, self.offset())
    }
}

/// The nudges of one held key press so far: what the readout and the announcement say, and what
/// the next event is compared with.
#[derive(Debug, Clone, PartialEq)]
pub struct NudgeRun {
    last: NudgeEvent,
    /// The document version right after the run's last commit.
    version: DocumentVersion,
    total: Vec2,
}

impl NudgeRun {
    /// Whether `event` continues `previous`: it is a repeat of the same arrow with the same Shift
    /// state, at most [`CONTINUATION_WINDOW_MS`] after it, and nothing was committed or merged
    /// since (`version` is the document's version now). Anything else opens a new run.
    #[must_use]
    pub fn continues(
        previous: Option<&Self>,
        event: NudgeEvent,
        version: &DocumentVersion,
    ) -> bool {
        let Some(run) = previous else {
            return false;
        };
        let gap = event.time_ms - run.last.time_ms;
        event.repeat
            && event.arrow == run.last.arrow
            && event.shift == run.last.shift
            && (0.0..=CONTINUATION_WINDOW_MS).contains(&gap)
            && *version == run.version
    }

    /// The run after `event` was written: `previous` extended when `continues` (the answer of
    /// [`NudgeRun::continues`]), else a new run. `version` is the document's version after the
    /// write.
    #[must_use]
    pub fn after(
        previous: Option<&Self>,
        continues: bool,
        event: NudgeEvent,
        version: DocumentVersion,
    ) -> Self {
        let offset = event.offset();
        let carried = previous
            .filter(|_| continues)
            .map_or(Vec2::ZERO, |run| run.total);
        Self {
            last: event,
            version,
            total: Vec2::new(carried.x + offset.x, carried.y + offset.y),
        }
    }

    /// The distance the run has moved the selection so far, X right, Y down.
    #[must_use]
    pub const fn total(&self) -> Vec2 {
        self.total
    }

    /// What a screen reader hears when the run ends: "Moved 11 mm right." or "Moved 3 mm right,
    /// 1 mm down.".
    #[must_use]
    pub fn announcement(&self) -> String {
        let mut parts = Vec::new();
        for (value, positive, negative) in [
            (self.total.x, "right", "left"),
            (self.total.y, "down", "up"),
        ] {
            let mm = value.abs().round();
            if mm >= 1.0 {
                let side = if value > 0.0 { positive } else { negative };
                parts.push(format!("{mm:.0} mm {side}"));
            }
        }
        format!("Moved {}.", parts.join(", "))
    }
}

/// What [`nudge`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum NudgeOutcome {
    /// None of the selected ids names an object any more, or the document refused the write:
    /// nothing moved.
    NothingToMove,
    /// The move would leave the document's coordinate limit: nothing was written.
    TooFar,
    /// The objects moved, in one commit.
    Moved {
        /// The run after this event.
        run: NudgeRun,
        /// This event opened a new run (a new step, once `0020` groups commits).
        new_run: bool,
    },
}

/// Moves the selected objects by `event` with the typed move's own write
/// (`commit_move`, the commit `translate_objects`), one commit per event, and returns the run
/// the event belongs to. `objects` is the document's current read; ids in `selected` that are
/// not in it are skipped, so one stale id does not refuse the batch. A write the document
/// refuses (an object in `objects` that is gone from the document) leaves the version as it was
/// and reports [`NudgeOutcome::NothingToMove`], not a move.
///
/// Future work (`0020`, `specs/0044-editing-quick-wins/adrs.md` decision 4): when `continues` is
/// true, this is where `Document::continue_step()` will be called before the write, so the
/// commits of a held key form one step. It does not exist yet and nothing calls it.
pub fn nudge(
    document: &Document,
    objects: &[ObjectSnapshot],
    selected: &[NodeId],
    event: NudgeEvent,
    previous: Option<&NudgeRun>,
    minter: &mut AnchorIdMinter,
) -> NudgeOutcome {
    let live = selected_objects(objects, selected);
    let Some(bounds) = union_bounds(&live) else {
        return NudgeOutcome::NothingToMove;
    };
    if !event.fits(bounds) {
        return NudgeOutcome::TooFar;
    }
    let before = document.version();
    let continues = NudgeRun::continues(previous, event, &before);
    let ids: Vec<NodeId> = live.iter().map(|object| object.id()).collect();
    // 0020: `Document::continue_step()` goes directly above this write when `continues`.
    let _ = commit_move(document, &ids, event.offset(), false, minter);
    let after = document.version();
    if after == before {
        return NudgeOutcome::NothingToMove;
    }
    NudgeOutcome::Moved {
        run: NudgeRun::after(previous, continues, event, after),
        new_run: !continues,
    }
}

/// The centre of the tight outline bounds of the objects of `ids` in `objects`, or `None` when
/// none of them is there: where the move readout of a nudge sits.
#[must_use]
pub fn selection_centre(objects: &[ObjectSnapshot], ids: &[NodeId]) -> Option<Point> {
    let (min, max) = union_bounds(&selected_objects(objects, ids))?;
    Some(Point::new(
        f64::midpoint(min.x, max.x),
        f64::midpoint(min.y, max.y),
    ))
}

/// The objects of `objects` named in `ids`, in `objects` order; a stale id names nothing.
fn selected_objects<'a>(objects: &'a [ObjectSnapshot], ids: &[NodeId]) -> Vec<&'a ObjectSnapshot> {
    let wanted: HashSet<NodeId> = ids.iter().copied().collect();
    objects
        .iter()
        .filter(|object| wanted.contains(&object.id()))
        .collect()
}

/// The box around the tight outlines of `objects` (`(min, max)` corners).
fn union_bounds(objects: &[&ObjectSnapshot]) -> Option<(Point, Point)> {
    objects
        .iter()
        .map(|object| object_outline_bounds(object))
        .reduce(|(min, max), (other_min, other_max)| {
            (
                Point::new(min.x.min(other_min.x), min.y.min(other_min.y)),
                Point::new(max.x.max(other_max.x), max.y.max(other_max.y)),
            )
        })
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, Document, NewAnchor};

    use super::*;

    fn event(arrow: Arrow, shift: bool, repeat: bool, time_ms: f64) -> NudgeEvent {
        NudgeEvent {
            arrow,
            shift,
            repeat,
            time_ms,
        }
    }

    fn version() -> DocumentVersion {
        Document::new(1).version()
    }

    #[test]
    fn an_event_moves_one_or_ten_millimetres_towards_its_arrow() {
        let offset = |arrow, shift| event(arrow, shift, false, 0.0).offset();
        assert_eq!(offset(Arrow::Right, false), Vec2::new(1.0, 0.0));
        assert_eq!(offset(Arrow::Left, false), Vec2::new(-1.0, 0.0));
        assert_eq!(offset(Arrow::Down, false), Vec2::new(0.0, 1.0));
        assert_eq!(offset(Arrow::Up, true), Vec2::new(0.0, -10.0));
        assert_eq!(offset(Arrow::Right, true), Vec2::new(10.0, 0.0));
    }

    #[test]
    fn an_event_that_would_leave_the_coordinate_limit_does_not_fit() {
        let limit = crate::transform_commit::MAX_COORDINATE_MM;
        let near = |x: f64| (Point::new(x - 5.0, 0.0), Point::new(x, 5.0));
        let right = event(Arrow::Right, false, false, 0.0);
        assert!(right.fits(near(limit - 1.0)), "exactly on the limit fits");
        assert!(!right.fits(near(limit - 0.5)));
        assert!(!event(Arrow::Right, true, false, 0.0).fits(near(limit - 5.0)));
        let left = event(Arrow::Left, false, false, 0.0);
        assert!(left.fits(near(limit - 0.5)), "the other way is free");
        assert!(!left.fits((Point::new(-limit, 0.0), Point::new(0.0, 1.0))));
    }

    #[test]
    fn only_the_four_arrow_keys_are_arrows() {
        assert_eq!(Arrow::from_key("ArrowLeft"), Some(Arrow::Left));
        assert_eq!(Arrow::from_key("ArrowDown"), Some(Arrow::Down));
        assert_eq!(Arrow::from_key("a"), None);
        assert_eq!(Arrow::from_key("Home"), None);
    }

    fn run_of(first: NudgeEvent, v: &DocumentVersion) -> NudgeRun {
        NudgeRun::after(None, false, first, v.clone())
    }

    #[test]
    fn a_repeat_of_the_same_key_within_the_window_continues() {
        let v = version();
        let run = run_of(event(Arrow::Right, false, false, 1000.0), &v);
        let next = event(Arrow::Right, false, true, 1030.0);
        assert!(NudgeRun::continues(Some(&run), next, &v));
        let edge = event(Arrow::Right, false, true, 1600.0);
        assert!(NudgeRun::continues(Some(&run), edge, &v));
    }

    #[test]
    fn every_other_event_opens_a_new_run() {
        let v = version();
        let run = run_of(event(Arrow::Right, false, false, 1000.0), &v);
        let continues = |e| NudgeRun::continues(Some(&run), e, &v);
        assert!(!NudgeRun::continues(
            None,
            event(Arrow::Right, false, true, 1030.0),
            &v
        ));
        assert!(
            !continues(event(Arrow::Right, false, false, 1030.0)),
            "a new press"
        );
        assert!(
            !continues(event(Arrow::Left, false, true, 1030.0)),
            "another arrow"
        );
        assert!(
            !continues(event(Arrow::Right, true, true, 1030.0)),
            "Shift changed"
        );
        assert!(
            !continues(event(Arrow::Right, false, true, 1601.0)),
            "after 600 ms"
        );
        assert!(
            !continues(event(Arrow::Right, false, true, 900.0)),
            "a clock that ran back"
        );
        assert!(!continues(event(Arrow::Right, false, true, f64::NAN)));
    }

    #[test]
    fn a_commit_or_merge_between_two_events_opens_a_new_run() {
        let document = Document::new(1);
        let run = run_of(event(Arrow::Right, false, false, 0.0), &document.version());
        let _ = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(1.0, 0.0)),
            ],
            false,
        );
        let next = event(Arrow::Right, false, true, 30.0);
        assert!(!NudgeRun::continues(Some(&run), next, &document.version()));
    }

    #[test]
    fn a_continued_run_adds_up_and_a_new_run_starts_again() {
        let v = version();
        let first = run_of(event(Arrow::Right, false, false, 0.0), &v);
        let repeat = event(Arrow::Right, false, true, 30.0);
        let second = NudgeRun::after(Some(&first), true, repeat, v.clone());
        assert_eq!(second.total(), Vec2::new(2.0, 0.0));
        let fresh = NudgeRun::after(
            Some(&second),
            false,
            event(Arrow::Down, true, false, 90.0),
            v,
        );
        assert_eq!(fresh.total(), Vec2::new(0.0, 10.0));
    }

    #[test]
    fn the_announcement_names_distance_and_direction() {
        let v = version();
        let mut run = run_of(event(Arrow::Right, true, false, 0.0), &v);
        run = NudgeRun::after(
            Some(&run),
            true,
            event(Arrow::Right, true, true, 30.0),
            v.clone(),
        );
        assert_eq!(run.announcement(), "Moved 20 mm right.");
        let up = run_of(event(Arrow::Up, false, false, 0.0), &v);
        assert_eq!(up.announcement(), "Moved 1 mm up.");
        let both = NudgeRun {
            total: Vec2::new(3.0, 1.0),
            ..up
        };
        assert_eq!(both.announcement(), "Moved 3 mm right, 1 mm down.");
    }

    fn two_paths(document: &Document) -> (NodeId, NodeId, Vec<ObjectSnapshot>) {
        let make = |x: f64, n: u64| {
            document.create_path(
                &[
                    NewAnchor::corner(AnchorId::new(1, n), Point::new(x, 0.0)),
                    NewAnchor::corner(AnchorId::new(1, n + 1), Point::new(x + 10.0, 5.0)),
                ],
                false,
            )
        };
        let (a, b) = (make(0.0, 1), make(50.0, 3));
        let objects = [a, b]
            .iter()
            .filter_map(|id| document.object(*id))
            .collect();
        (a, b, objects)
    }

    fn first_x(document: &Document, id: NodeId) -> f64 {
        let Some(ObjectSnapshot::Path(path)) = document.object(id) else {
            panic!("a path");
        };
        path.anchors[0].point.x
    }

    #[test]
    fn a_nudge_moves_every_selected_object_in_one_translate_objects_commit() {
        let document = Document::new(1);
        let (a, b, objects) = two_paths(&document);
        let mut minter = AnchorIdMinter::new(1);
        let outcome = nudge(
            &document,
            &objects,
            &[a, b],
            event(Arrow::Right, true, false, 0.0),
            None,
            &mut minter,
        );
        let NudgeOutcome::Moved { run, new_run } = outcome else {
            panic!("moved, got {outcome:?}");
        };
        assert!(new_run);
        assert_eq!(run.total(), Vec2::new(10.0, 0.0));
        assert_eq!((first_x(&document, a), first_x(&document, b)), (10.0, 60.0));
    }

    #[test]
    fn a_repeat_extends_the_run_and_a_press_opens_a_new_one() {
        let document = Document::new(1);
        let (a, _, objects) = two_paths(&document);
        let mut minter = AnchorIdMinter::new(1);
        let mut run = None;
        for (repeat, time_ms) in [(false, 100.0), (true, 130.0), (true, 160.0)] {
            let objects: Vec<_> = document.object(a).into_iter().collect();
            let e = event(Arrow::Down, false, repeat, time_ms);
            let NudgeOutcome::Moved { run: next, new_run } =
                nudge(&document, &objects, &[a], e, run.as_ref(), &mut minter)
            else {
                panic!("moved");
            };
            assert_eq!(new_run, !repeat);
            run = Some(next);
        }
        assert_eq!(run.as_ref().map(NudgeRun::total), Some(Vec2::new(0.0, 3.0)));
        let again = event(Arrow::Down, false, false, 400.0);
        let NudgeOutcome::Moved { run, new_run } =
            nudge(&document, &objects, &[a], again, run.as_ref(), &mut minter)
        else {
            panic!("moved");
        };
        assert!(new_run);
        assert_eq!(run.total(), Vec2::new(0.0, 1.0));
    }

    #[test]
    fn a_nudge_past_the_limit_writes_nothing() {
        let document = Document::new(1);
        let (a, b, _) = two_paths(&document);
        let edge = crate::transform_commit::MAX_COORDINATE_MM;
        let _ = document.translate_objects(&[a], Vec2::new(edge - 10.0, 0.0));
        let objects: Vec<_> = [a, b]
            .iter()
            .filter_map(|id| document.object(*id))
            .collect();
        let version = document.version();
        let mut minter = AnchorIdMinter::new(1);
        let outcome = nudge(
            &document,
            &objects,
            &[a, b],
            event(Arrow::Right, false, false, 0.0),
            None,
            &mut minter,
        );
        assert_eq!(outcome, NudgeOutcome::TooFar);
        assert_eq!(document.version(), version);
    }

    #[test]
    fn a_stale_id_is_skipped_and_an_all_stale_selection_moves_nothing() {
        let document = Document::new(1);
        let (a, _, objects) = two_paths(&document);
        let gone = Document::new(2).create_rect(curvyo_document_core::RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(1.0),
            height: Length::from_mm(1.0),
        });
        let mut minter = AnchorIdMinter::new(1);
        let e = event(Arrow::Left, false, false, 0.0);
        let moved = nudge(&document, &objects, &[gone, a], e, None, &mut minter);
        assert!(matches!(moved, NudgeOutcome::Moved { .. }));
        assert_eq!(first_x(&document, a), -1.0);
        let version = document.version();
        let none = nudge(&document, &objects, &[gone], e, None, &mut minter);
        assert_eq!(none, NudgeOutcome::NothingToMove);
        assert_eq!(document.version(), version);
    }

    #[test]
    fn a_write_the_document_refuses_is_not_reported_as_a_move() {
        let document = Document::new(1);
        let (a, _, objects) = two_paths(&document);
        document.delete_objects(&[a]).expect("deleted");
        let version = document.version();
        let mut minter = AnchorIdMinter::new(1);
        let outcome = nudge(
            &document,
            &objects,
            &[a],
            event(Arrow::Right, false, false, 0.0),
            None,
            &mut minter,
        );
        assert_eq!(outcome, NudgeOutcome::NothingToMove);
        assert_eq!(document.version(), version);
    }

    #[test]
    fn the_centre_of_a_selection_is_the_centre_of_its_objects_only() {
        let document = Document::new(1);
        let (a, b, objects) = two_paths(&document);
        assert_eq!(
            selection_centre(&objects, &[a, b]),
            Some(Point::new(30.0, 2.5))
        );
        assert_eq!(
            selection_centre(&objects, &[b]),
            Some(Point::new(55.0, 2.5))
        );
        assert_eq!(selection_centre(&objects, &[]), None);
    }
}
