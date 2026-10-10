//! `curvyo-ui-core` tests of `specs/0019-multi-object-transform/`: the press order
//! with a group box, the handles' hit rule, the gestures through the tool, the
//! all-or-nothing rule and the typed entries. Scale is 4 px/mm, so the 8 px
//! Select tolerance is 2 mm and the 16 px handle radius is 4 mm.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    missing_docs
)]
#![allow(clippy::too_many_lines)]

use curvyo_document_core::{
    Angle, Document, InnerRatio, Length, NodeId, ObjectSnapshot, Point, PointCount,
    PrimitiveSnapshot, RectBounds, Shape, StarFrame, Tolerance,
};
use curvyo_ui_core::{
    AnchorIdMinter, EditHandle, EntryOutcome, GroupEntry, InvalidReason, Modifiers,
    ObjectSelection, PressTarget, ResizeDirection, ScaleModes, SelectTool,
    TransformHandleTolerances, classify_press,
};

const SCALE: f64 = 4.0;
const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::new(true, false);
const CTRL: Modifiers = Modifiers::new(false, true);

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn tolerances() -> TransformHandleTolerances {
    TransformHandleTolerances::at_scale(SCALE)
}

fn tol() -> Tolerance {
    Tolerance::from_mm(2.0)
}

struct Rig {
    document: Document,
    selection: ObjectSelection,
    tool: SelectTool,
}

impl Rig {
    fn new() -> Self {
        Self {
            document: Document::new(1),
            selection: ObjectSelection::new(),
            tool: SelectTool::new(),
        }
    }

    fn rect(&self, x: f64, y: f64, w: f64, h: f64) -> NodeId {
        self.document.create_rect(RectBounds {
            origin: pt(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        })
    }

    fn objects(&self) -> Vec<ObjectSnapshot> {
        self.document
            .object_ids()
            .into_iter()
            .filter_map(|id| self.document.object(id))
            .collect()
    }

    fn select(&mut self, ids: &[NodeId]) {
        self.selection.set(ids);
    }

    fn target(&self, at: Point, modifiers: Modifiers) -> PressTarget {
        classify_press(
            &self.objects(),
            &self.selection,
            at,
            tol(),
            tolerances(),
            modifiers,
        )
    }

    fn drag(&mut self, from: Point, to: Point, modifiers: Modifiers) {
        let objects = self.objects();
        self.tool.pointer_down(
            &objects,
            &mut self.selection,
            from,
            tol(),
            tolerances(),
            modifiers,
        );
        self.tool.pointer_moved(
            pt(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y)),
            modifiers,
            &mut self.selection,
        );
        self.tool.pointer_moved(to, modifiers, &mut self.selection);
        self.tool.pointer_up(
            &self.document,
            &objects,
            &mut self.selection,
            to,
            modifiers,
            &mut AnchorIdMinter::new(9),
        );
    }

    fn shape(&self, id: NodeId) -> Shape {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot { shape, .. }) =
            self.document.object(id).unwrap()
        else {
            panic!("a primitive");
        };
        shape
    }
}

/// A 100 x 60 rectangle at (0, 0), another at (140, 0), a small one between them.
fn two_big_rects() -> (Rig, NodeId, NodeId, NodeId) {
    let rig = Rig::new();
    let a = rig.rect(0.0, 0.0, 100.0, 60.0);
    let b = rig.rect(140.0, 0.0, 100.0, 60.0);
    let c = rig.rect(110.0, 20.0, 20.0, 20.0);
    (rig, a, b, c)
}

/// Criterion 43: the order of a press on a multi-selection.
#[test]
fn the_press_order_with_a_group_box() {
    let (mut rig, a, b, c) = two_big_rects();
    rig.select(&[a, b]);
    // The group box is (0, 0) to (240, 60); its centre handle at (120, 30).
    assert_eq!(rig.target(pt(120.0, 30.0), NONE), PressTarget::CentreHandle);
    assert_eq!(
        rig.target(pt(120.0, 30.0), CTRL),
        PressTarget::CentreHandle,
        "Ctrl on the centre handle is still a move"
    );
    // The south-east corner handle.
    assert_eq!(
        rig.target(pt(240.0, 60.0), NONE),
        PressTarget::Handle(EditHandle::Resize(ResizeDirection::Se))
    );
    // A corner rotate handle stands 32 px (8 mm) out on the diagonal.
    let offset = 32.0 / SCALE / std::f64::consts::SQRT_2;
    assert_eq!(
        rig.target(pt(240.0 + offset, -offset), NONE),
        PressTarget::Handle(EditHandle::Rotate(ResizeDirection::Ne))
    );
    // A side rotate handle exists only with Shift held.
    let north = pt(120.0, -32.0 / SCALE);
    assert_eq!(rig.target(north, NONE), PressTarget::Empty);
    assert_eq!(
        rig.target(north, SHIFT),
        PressTarget::Handle(EditHandle::Rotate(ResizeDirection::N))
    );
    // A selected object's outline moves the selection; an unselected one is selected.
    assert_eq!(rig.target(pt(0.0, 15.0), NONE), PressTarget::Object(a));
    assert_eq!(rig.target(pt(110.0, 25.0), NONE), PressTarget::Object(c));
    // Empty canvas inside the group box: a marquee, not a move (criterion 45).
    assert_eq!(rig.target(pt(105.0, 55.0), NONE), PressTarget::Empty);
    // Alt first, on a handle too.
    assert_eq!(
        rig.target(pt(240.0, 60.0), NONE.with_alt(true)),
        PressTarget::Lasso
    );
}

/// Criterion 14: a handle that is not drawn has no hit area, an edge handle of a
/// small box is hit-testable although not drawn, and a uniform-only selection has
/// no edge handles at all (criterion 21).
#[test]
fn handles_that_are_not_drawn_have_no_hit_area() {
    let mut rig = Rig::new();
    // Two squares 6 mm apart: the box is 26 x 10 mm = 104 x 40 px: no centre handle.
    let a = rig.rect(0.0, 0.0, 10.0, 10.0);
    let b = rig.rect(16.0, 0.0, 10.0, 10.0);
    rig.select(&[a, b]);
    assert_ne!(rig.target(pt(13.0, 5.0), NONE), PressTarget::CentreHandle);
    // 40 px high: the edge handles are drawn; at 20 px they are not, but still hit.
    let narrow = rig.rect(0.0, 40.0, 10.0, 5.0);
    let other = rig.rect(16.0, 40.0, 10.0, 5.0);
    rig.select(&[narrow, other]);
    assert_eq!(
        rig.target(pt(13.0, 40.0), NONE),
        PressTarget::Handle(EditHandle::Resize(ResizeDirection::N)),
        "hit-testable under 24 px"
    );

    // A star holds the selection to proportional scaling: no east edge handle.
    let mut rig = Rig::new();
    let star = rig.document.create_star(
        StarFrame {
            center: pt(30.0, 30.0),
            radius: Length::from_mm(20.0),
            angle: Angle::from_radians(0.0),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let plain = rig.rect(60.0, 0.0, 40.0, 60.0);
    rig.select(&[star, plain]);
    let group = SelectTool::group_of(&rig.objects(), &rig.selection).unwrap();
    assert!(group.uniform_only());
    let b = group.bounds();
    let east = pt(b.max.x, f64::midpoint(b.min.y, b.max.y));
    assert_ne!(
        rig.target(east, NONE),
        PressTarget::Handle(EditHandle::Resize(ResizeDirection::E))
    );
    assert_eq!(
        rig.target(pt(b.max.x, b.max.y), NONE),
        PressTarget::Handle(EditHandle::Resize(ResizeDirection::Se))
    );
}

/// Criterion 21: a free corner drag and a Ctrl corner drag give a uniform-only
/// selection the same result: one factor for both axes.
#[test]
fn a_uniform_only_selection_ignores_ctrl_and_keeps_one_factor() {
    let build = || {
        let mut rig = Rig::new();
        let star = rig.document.create_star(
            StarFrame {
                center: pt(20.0, 20.0),
                radius: Length::from_mm(20.0),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let plain = rig.rect(60.0, 0.0, 40.0, 40.0);
        rig.select(&[star, plain]);
        (rig, plain)
    };
    let corner_of = |rig: &Rig| {
        let b = *SelectTool::group_of(&rig.objects(), &rig.selection)
            .unwrap()
            .bounds();
        pt(b.max.x, b.max.y)
    };
    let mut results = Vec::new();
    for modifiers in [NONE, CTRL] {
        let (mut rig, plain) = build();
        let corner = corner_of(&rig);
        rig.drag(corner, pt(corner.x + 30.0, corner.y + 2.0), modifiers);
        let Shape::Rect { bounds, .. } = rig.shape(plain) else {
            panic!("a rectangle");
        };
        assert!(
            (bounds.width.as_mm() / 40.0 - bounds.height.as_mm() / 40.0).abs() < 1e-9,
            "one factor"
        );
        assert!(bounds.width.as_mm() > 40.0);
        results.push(bounds.width.as_mm());
    }
    assert_eq!(results[0], results[1]);
}

/// Criterion 22: a result beyond the coordinate limit is no change, all or
/// nothing; the release writes nothing.
#[test]
fn a_scale_beyond_the_limit_changes_nothing() {
    let (mut rig, a, b, _) = two_big_rects();
    rig.select(&[a, b]);
    let before = rig.objects();
    rig.drag(pt(240.0, 60.0), pt(2.0e9, 60.0), NONE);
    assert_eq!(rig.objects(), before);
}

/// Criterion 22 and 18: a factor below zero stops at zero.
#[test]
fn a_negative_factor_clamps_to_zero() {
    let (mut rig, a, b, _) = two_big_rects();
    rig.select(&[a, b]);
    rig.drag(pt(240.0, 60.0), pt(-500.0, 60.0), NONE);
    for id in [a, b] {
        let Shape::Rect { bounds, .. } = rig.shape(id) else {
            panic!("a rectangle");
        };
        assert_eq!(bounds.width.as_mm(), 0.0);
        assert_eq!(bounds.height.as_mm(), 60.0);
    }
}

/// Criterion 30 and 31: a drag released inside the dead zone, or after Escape,
/// writes nothing.
#[test]
fn a_press_and_release_on_a_handle_writes_nothing() {
    let (mut rig, a, b, _) = two_big_rects();
    rig.select(&[a, b]);
    let before = rig.objects();
    rig.drag(pt(240.0, 60.0), pt(240.2, 60.0), NONE);
    assert_eq!(rig.objects(), before);
    let objects = rig.objects();
    rig.tool.pointer_down(
        &objects,
        &mut rig.selection,
        pt(240.0, 60.0),
        tol(),
        tolerances(),
        NONE,
    );
    rig.tool
        .pointer_moved(pt(300.0, 80.0), NONE, &mut rig.selection);
    assert!(
        rig.tool
            .live_edit(&objects, &rig.selection, pt(300.0, 80.0), false, false)
            .is_some()
    );
    rig.tool.escape();
    assert!(
        rig.tool
            .live_edit(&objects, &rig.selection, pt(300.0, 80.0), false, false)
            .is_none()
    );
    assert_eq!(rig.objects(), before);
}

/// Criterion 32: the preview is the same function as the release: the blue
/// geometry equals what the release writes.
#[test]
fn the_preview_is_what_the_release_commits() {
    let (mut rig, a, b, _) = two_big_rects();
    rig.select(&[a, b]);
    let objects = rig.objects();
    let to = pt(300.0, 100.0);
    rig.tool.pointer_down(
        &objects,
        &mut rig.selection,
        pt(240.0, 60.0),
        tol(),
        tolerances(),
        NONE,
    );
    rig.tool.pointer_moved(to, NONE, &mut rig.selection);
    let live = rig
        .tool
        .live_edit(&objects, &rig.selection, to, false, false)
        .unwrap();
    rig.tool.pointer_up(
        &rig.document,
        &objects,
        &mut rig.selection,
        to,
        NONE,
        &mut AnchorIdMinter::new(9),
    );
    for preview in &live.objects {
        assert_eq!(&rig.document.object(preview.id()).unwrap(), preview);
    }
}

fn group_entry_of(rig: &Rig, key: curvyo_ui_core::EntryKey) -> GroupEntry {
    let mut tool = SelectTool::new();
    tool.open_entry_for_key(&rig.objects(), &rig.selection, key)
        .unwrap();
    tool.group_entry().unwrap().clone()
}

/// Criteria 33 and 26: a typed angle turns by that angle about the group box
/// centre, and a typed negative angle turns it back.
#[test]
fn a_typed_angle_and_its_negative_restore_the_selection() {
    let (mut rig, a, b, _) = two_big_rects();
    rig.select(&[a, b]);
    let before = rig.objects();
    let entry = group_entry_of(&rig, curvyo_ui_core::EntryKey::Angle);
    assert_eq!(entry.fields()[0].prefill, "0");
    assert_eq!(
        entry.commit(&rig.document, ["37.5", ""], 0),
        EntryOutcome::Committed
    );
    assert_ne!(rig.objects(), before);
    let entry = group_entry_of(&rig, curvyo_ui_core::EntryKey::Angle);
    // A decimal comma, the real minus sign and the degree sign are accepted.
    assert_eq!(
        entry.commit(&rig.document, ["\u{2212}37,5\u{b0}", ""], 0),
        EntryOutcome::Committed
    );
    for (old, new) in before.iter().zip(rig.objects()) {
        let (Shape::Rect { bounds: x, .. }, Shape::Rect { bounds: y, .. }) = (
            match old {
                ObjectSnapshot::Primitive(p) => p.shape,
                ObjectSnapshot::Path(_) => panic!("a primitive"),
            },
            match &new {
                ObjectSnapshot::Primitive(p) => p.shape,
                ObjectSnapshot::Path(_) => panic!("a primitive"),
            },
        ) else {
            panic!("rectangles");
        };
        assert!((x.origin.x - y.origin.x).abs() < 1e-9 && (x.origin.y - y.origin.y).abs() < 1e-9);
        assert!(new.rotation().as_radians().abs() < 1e-9);
    }
}

/// Criterion 34: typed sizes are validated; zero is refused, a huge one is too
/// large, an unedited Enter writes nothing; a uniform-only selection links the
/// two fields at the box's aspect ratio.
#[test]
fn the_size_entry_validates_and_links_a_uniform_only_selection() {
    let (mut rig, a, b, _) = two_big_rects();
    rig.select(&[a, b]);
    let entry = group_entry_of(&rig, curvyo_ui_core::EntryKey::Size);
    assert_eq!(entry.fields()[0].prefill, "240.0");
    assert!(!entry.linked());
    assert_eq!(
        entry.commit(&rig.document, ["0", "60.0"], 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::NotPositive
        }
    );
    assert_eq!(
        entry.commit(&rig.document, ["240.0", "-5"], 1),
        EntryOutcome::Invalid {
            field: 1,
            reason: InvalidReason::NotPositive
        }
    );
    assert_eq!(
        entry.commit(&rig.document, ["240.0", "60.0"], 0),
        EntryOutcome::Unchanged
    );
    assert_eq!(
        entry.commit(&rig.document, ["5e9", "60.0"], 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::NotANumber
        }
    );

    let mut rig = Rig::new();
    let star = rig.document.create_star(
        StarFrame {
            center: pt(20.0, 20.0),
            radius: Length::from_mm(20.0),
            angle: Angle::from_radians(0.0),
        },
        PointCount::new(5).unwrap(),
        InnerRatio::new(0.5).unwrap(),
    );
    let plain = rig.rect(60.0, 0.0, 40.0, 40.0);
    rig.select(&[star, plain]);
    let entry = group_entry_of(&rig, curvyo_ui_core::EntryKey::Size);
    assert!(entry.linked(), "uniform-only: the fields are linked");
    let text = entry.linked_text(0, "200").expect("the other field");
    let box_ = *SelectTool::group_of(&rig.objects(), &rig.selection)
        .unwrap()
        .bounds();
    let expected = 200.0 * box_.height() / box_.width();
    assert_eq!(text, format!("{expected:.1}"));
    // The size is held: the group box becomes 200 wide.
    assert_eq!(
        entry.commit(&rig.document, ["200", &text], 0),
        EntryOutcome::Committed
    );
    let after = SelectTool::group_of(&rig.objects(), &rig.selection).unwrap();
    assert!((after.bounds().width() - 200.0).abs() < 1e-6);
}

/// Criteria 36 and 37: the skew entry exists for paths only and refuses 90 degrees.
#[test]
fn the_skew_entry_is_for_paths_and_refuses_a_right_angle() {
    use curvyo_document_core::{AnchorId, NewAnchor};
    let rig = Rig::new();
    let make = |n: u64, a: (f64, f64), b: (f64, f64)| {
        rig.document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, n), pt(a.0, a.1)),
                NewAnchor::corner(AnchorId::new(1, n + 1), pt(b.0, b.1)),
            ],
            false,
        )
    };
    let p = make(1, (0.0, 0.0), (40.0, 30.0));
    let q = make(3, (50.0, 0.0), (80.0, 30.0));
    let mut rig = rig;
    rig.select(&[p, q]);
    let entry = group_entry_of(&rig, curvyo_ui_core::EntryKey::SkewX);
    assert_eq!(entry.fields()[0].accessible_name, "Skew angle x");
    assert_eq!(
        entry.commit(&rig.document, ["90", ""], 0),
        EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::SkewRange
        }
    );
    assert_eq!(
        entry.commit(&rig.document, ["0", ""], 0),
        EntryOutcome::Unchanged
    );
    assert_eq!(
        entry.commit(&rig.document, ["-45", ""], 0),
        EntryOutcome::Committed
    );
    let r = rig.rect(200.0, 0.0, 10.0, 10.0);
    rig.select(&[p, r]);
    let mut tool = SelectTool::new();
    assert_eq!(
        tool.open_entry_for_key(
            &rig.objects(),
            &rig.selection,
            curvyo_ui_core::EntryKey::SkewX
        ),
        Err(curvyo_ui_core::KeyEntryRefusal::SkewNeedsPath)
    );
    let _ = ScaleModes::default();
}
