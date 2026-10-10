//! `Session`'s select all and arrow-key nudge: the two keys of `specs/0044-editing-quick-wins/`,
//! as the session runs them once the key table of `keys.rs` has said yes.

use curvyo_ui_core::{NudgeEvent, NudgeOutcome, NudgeRun, nudge, selection_centre};

use super::Session;
use super::keys::{KeyHint, KeyOutcome};
use super::move_indicators::move_readout_text;
use super::shapes::LiveReadout;

impl Session {
    /// Ctrl+A: every object of the document root becomes the selection, bottom to top. View
    /// state only: nothing is written. An empty document is still a handled key.
    pub(super) fn select_all(&mut self) -> KeyOutcome {
        let ids = self.document.object_ids();
        self.selection.set(&ids);
        KeyOutcome::SelectedAll
    }

    /// One arrow-key event: the selection moves by the event's distance in one
    /// `translate_objects` commit, or nothing happens and the maker is told why.
    pub(super) fn nudge_selection(&mut self, event: NudgeEvent) -> KeyOutcome {
        let objects = self.objects();
        let outcome = nudge(
            &self.document,
            &objects,
            self.selection.ids(),
            event,
            self.nudge_run.as_ref(),
            &mut self.minter,
        );
        match outcome {
            NudgeOutcome::Moved { run, new_run } => {
                self.nudge_run = Some(run);
                KeyOutcome::Nudged { new_run }
            }
            NudgeOutcome::TooFar => KeyOutcome::Hint(KeyHint::TooFar),
            NudgeOutcome::NothingToMove => KeyOutcome::Ignored,
        }
    }

    /// The move readout of the nudge step that is running ("Δ 3.0, 0.0 mm", always millimetres),
    /// anchored at the centre of the selection's bounds, or `None` before any nudge.
    #[must_use]
    pub fn nudge_readout(&self) -> Option<LiveReadout> {
        let run = self.nudge_run.as_ref()?;
        let anchor = selection_centre(&self.objects(), self.selection.ids())?;
        Some(LiveReadout {
            text: move_readout_text(run.total(), false),
            anchor,
        })
    }

    /// What a screen reader hears when the running nudge step ends ("Moved 11 mm right."), or an
    /// empty text before any nudge.
    #[must_use]
    pub fn nudge_announcement(&self) -> String {
        self.nudge_run
            .as_ref()
            .map_or_else(String::new, NudgeRun::announcement)
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{
        AnchorId, Length, NewAnchor, NodeId, ObjectSnapshot, Point, RectBounds, Shape, Vec2,
    };

    use super::super::keys::{KeyInput, KeyOutcome};
    use super::super::{Session, Tool};

    fn rect_at(session: &Session, x: f64, y: f64) -> NodeId {
        session.document.create_rect(RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        })
    }

    fn session_with_rects(count: u32) -> (Session, Vec<NodeId>) {
        let session = Session::new(1);
        let ids = (0..count)
            .map(|i| rect_at(&session, f64::from(i) * 20.0, 0.0))
            .collect();
        (session, ids)
    }

    fn origin_of(session: &Session, id: NodeId) -> Point {
        match session.document.object(id) {
            Some(ObjectSnapshot::Primitive(p)) => match p.shape {
                Shape::Rect { bounds, .. } => bounds.origin,
                other => panic!("a rectangle, got {other:?}"),
            },
            other => panic!("a primitive, got {other:?}"),
        }
    }

    fn press(session: &mut Session, key: &str, shift: bool, repeat: bool, at: f64) -> KeyOutcome {
        session.key_down(KeyInput {
            key,
            shift,
            repeat,
            time_ms: at,
            ..KeyInput::default()
        })
    }

    fn ctrl_a(session: &mut Session) -> KeyOutcome {
        session.key_down(KeyInput {
            key: "a",
            ctrl: true,
            ..KeyInput::default()
        })
    }

    #[test]
    fn ctrl_a_selects_every_object_in_stacking_order_and_writes_nothing() {
        let (mut session, ids) = session_with_rects(5);
        let version = session.document.version();
        assert_eq!(ctrl_a(&mut session), KeyOutcome::SelectedAll);
        assert_eq!(session.selection.ids(), ids.as_slice());
        assert_eq!(session.document.version(), version);
    }

    #[test]
    fn ctrl_a_replaces_the_selection_and_is_idempotent() {
        let (mut session, ids) = session_with_rects(3);
        session.selection.select_single(ids[1]);
        let _ = ctrl_a(&mut session);
        assert_eq!(session.selection.ids(), ids.as_slice());
        let _ = ctrl_a(&mut session);
        assert_eq!(session.selection.ids(), ids.as_slice());
    }

    #[test]
    fn ctrl_a_on_an_empty_document_is_handled_and_selects_nothing() {
        let mut session = Session::new(1);
        assert_eq!(ctrl_a(&mut session), KeyOutcome::SelectedAll);
        assert!(session.selection.is_empty());
    }

    #[test]
    fn ctrl_a_in_another_tool_is_ignored_and_changes_nothing() {
        let (mut session, ids) = session_with_rects(3);
        session.selection.select_single(ids[0]);
        session.set_tool(Tool::Pen);
        assert_eq!(ctrl_a(&mut session), KeyOutcome::Ignored);
        session.set_tool(Tool::Node);
        assert_eq!(ctrl_a(&mut session), KeyOutcome::Ignored);
        assert_eq!(session.selection.ids(), &ids[..1]);
    }

    #[test]
    fn an_arrow_moves_one_millimetre_and_shift_ten_in_document_space() {
        let (mut session, ids) = session_with_rects(1);
        session.selection.select_single(ids[0]);
        for (key, shift, dx, dy) in [
            ("ArrowRight", false, 1.0, 0.0),
            ("ArrowDown", false, 0.0, 1.0),
            ("ArrowLeft", true, -10.0, 0.0),
            ("ArrowUp", true, 0.0, -10.0),
        ] {
            let before = origin_of(&session, ids[0]);
            let version = session.document.version();
            let outcome = press(&mut session, key, shift, false, 0.0);
            assert_eq!(outcome, KeyOutcome::Nudged { new_run: true }, "{key}");
            let after = origin_of(&session, ids[0]);
            assert_eq!(
                (after.x - before.x, after.y - before.y),
                (dx, dy),
                "{key} shift {shift}"
            );
            assert_ne!(session.document.version(), version, "one commit per event");
        }
        assert_eq!(session.selection.ids(), &ids[..1], "the selection stays");
    }

    fn mixed_session() -> (Session, Vec<NodeId>) {
        let (session, mut ids) = session_with_rects(2);
        ids.push(session.document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 50.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 60.0)),
            ],
            false,
        ));
        (session, ids)
    }

    #[test]
    fn a_nudge_moves_the_selection_exactly_like_translate_objects() {
        let (mut nudged, ids) = mixed_session();
        let (typed, _) = mixed_session();
        let _ = ctrl_a(&mut nudged);
        let _ = press(&mut nudged, "ArrowRight", true, false, 0.0);
        let _ = typed.document.translate_objects(&ids, Vec2::new(10.0, 0.0));
        for id in &ids {
            assert_eq!(nudged.document.object(*id), typed.document.object(*id));
        }
        assert_eq!(nudged.selection.ids(), ids.as_slice());
        assert_eq!(origin_of(&nudged, ids[0]), Point::new(10.0, 0.0));
    }

    #[test]
    fn a_held_key_moves_once_per_event_and_reads_out_the_step() {
        let (mut session, ids) = session_with_rects(1);
        session.selection.select_single(ids[0]);
        assert_eq!(
            press(&mut session, "ArrowRight", false, false, 1000.0),
            KeyOutcome::Nudged { new_run: true }
        );
        let mut versions = vec![session.document.version()];
        for step in 1..=10_u32 {
            let at = 1000.0 + f64::from(step) * 33.0;
            assert_eq!(
                press(&mut session, "ArrowRight", false, true, at),
                KeyOutcome::Nudged { new_run: false }
            );
            versions.push(session.document.version());
        }
        assert_eq!(origin_of(&session, ids[0]).x, 11.0);
        assert!(versions.windows(2).all(|pair| pair[0] != pair[1]));
        let readout = session.nudge_readout().expect("a readout");
        assert_eq!(readout.text, "Δ 11.0, 0.0 mm");
        assert_eq!(readout.anchor, Point::new(16.0, 5.0));
        assert_eq!(session.nudge_announcement(), "Moved 11 mm right.");
    }

    #[test]
    fn separate_presses_are_separate_runs() {
        let (mut session, ids) = session_with_rects(1);
        session.selection.select_single(ids[0]);
        for at in [0.0, 100.0, 200.0] {
            assert_eq!(
                press(&mut session, "ArrowDown", false, false, at),
                KeyOutcome::Nudged { new_run: true }
            );
            assert_eq!(session.nudge_announcement(), "Moved 1 mm down.");
        }
        assert_eq!(origin_of(&session, ids[0]).y, 3.0);
    }

    #[test]
    fn a_gap_a_changed_key_or_a_change_in_between_opens_a_new_run() {
        let (mut session, ids) = session_with_rects(1);
        session.selection.select_single(ids[0]);
        let _ = press(&mut session, "ArrowRight", false, false, 0.0);
        let again = |session: &mut Session, key, shift, at| press(session, key, shift, true, at);
        assert_eq!(
            again(&mut session, "ArrowRight", false, 601.0),
            KeyOutcome::Nudged { new_run: true },
            "more than 600 ms"
        );
        assert_eq!(
            again(&mut session, "ArrowDown", false, 630.0),
            KeyOutcome::Nudged { new_run: true },
            "another arrow"
        );
        assert_eq!(
            again(&mut session, "ArrowDown", true, 660.0),
            KeyOutcome::Nudged { new_run: true },
            "Shift changed"
        );
        assert_eq!(
            again(&mut session, "ArrowDown", true, 690.0),
            KeyOutcome::Nudged { new_run: false }
        );
        // A write that is not the run's own (a merge from a peer looks the same to the version).
        let _ = session
            .document
            .translate_objects(&[ids[0]], Vec2::new(0.0, 2.0));
        assert_eq!(
            again(&mut session, "ArrowDown", true, 720.0),
            KeyOutcome::Nudged { new_run: true },
            "the document changed in between"
        );
    }

    #[test]
    fn a_nudge_beyond_the_coordinate_limit_writes_nothing_and_says_so() {
        let mut session = Session::new(1);
        let id = rect_at(&session, 9_999_985.0, 0.0);
        session.selection.select_single(id);
        let version = session.document.version();
        assert_eq!(
            press(&mut session, "ArrowRight", true, false, 0.0),
            KeyOutcome::Hint(super::super::keys::KeyHint::TooFar)
        );
        assert_eq!(session.document.version(), version);
        assert_eq!(origin_of(&session, id).x, 9_999_985.0);
        assert!(session.nudge_readout().is_none());
        assert_eq!(
            press(&mut session, "ArrowRight", false, false, 10.0),
            KeyOutcome::Nudged { new_run: true },
            "a shorter nudge still fits"
        );
    }

    #[test]
    fn escape_after_a_nudge_leaves_the_objects_where_they_are() {
        let (mut session, ids) = session_with_rects(1);
        session.selection.select_single(ids[0]);
        let _ = press(&mut session, "ArrowRight", true, false, 0.0);
        let _ = press(&mut session, "Escape", false, false, 5.0);
        assert_eq!(origin_of(&session, ids[0]).x, 10.0);
    }

    #[test]
    fn an_arrow_with_nothing_selected_or_in_another_tool_is_ignored() {
        let (mut session, ids) = session_with_rects(1);
        assert_eq!(
            press(&mut session, "ArrowRight", false, false, 0.0),
            KeyOutcome::Ignored
        );
        session.selection.select_single(ids[0]);
        session.set_tool(Tool::Node);
        assert_eq!(
            press(&mut session, "ArrowRight", false, false, 0.0),
            KeyOutcome::Ignored
        );
        assert_eq!(origin_of(&session, ids[0]).x, 0.0);
    }
}
