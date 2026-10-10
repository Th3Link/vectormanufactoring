//! Deterministic event fuzz over the path-tools slice (`0031`, `0034`, `0035`): random sequences
//! of pointer, key and command events across the Select, Node and Pen tools, with the invariants
//! of the document checked after every sequence: no panic, every anchor id unique, every number
//! finite, no empty path, no closed path of fewer than two nodes, the file still round-trips.

#![allow(
    unused_must_use,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::manual_midpoint
)]

use std::collections::HashSet;

use curvyo_document_core::{AnchorId, AnchorKind, Document, NewAnchor, Point, Vec2, pack, unpack};
use curvyo_editor_wasm::{Session, Tool};
use curvyo_ui_core::JoinType;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn anchor(id: u64, x: f64, y: f64, kind: AnchorKind, hin: Vec2, hout: Vec2) -> NewAnchor {
    NewAnchor {
        id: AnchorId::new(4242, id),
        point: Point::new(x, y),
        handle_in: hin,
        handle_out: hout,
        kind,
    }
}

fn seed_document() -> (Document, Vec<Point>) {
    let d = Document::new(1);
    let mut n = 1;
    let mut next = || {
        n += 1;
        n
    };
    let mut pts = Vec::new();
    let c = |x: f64, y: f64, id| anchor(id, x, y, AnchorKind::Corner, Vec2::ZERO, Vec2::ZERO);
    for (pts_list, closed) in [
        (
            vec![(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)],
            false,
        ),
        (vec![(100.0, 0.0), (140.0, 0.0), (140.0, 40.0)], true),
        (vec![(200.0, 0.0), (240.0, 20.0)], false),
        (vec![(0.0, 100.0), (30.0, 100.0), (60.0, 100.0)], false),
        (
            vec![
                (100.0, 100.0),
                (140.0, 100.0),
                (140.0, 140.0),
                (100.0, 140.0),
            ],
            true,
        ),
        (
            vec![
                (110.0, 110.0),
                (130.0, 110.0),
                (130.0, 130.0),
                (110.0, 130.0),
            ],
            true,
        ),
    ] {
        let a: Vec<NewAnchor> = pts_list
            .iter()
            .map(|&(x, y)| {
                pts.push(Point::new(x, y));
                c(x, y, next())
            })
            .collect();
        d.create_path(&a, closed);
    }
    let k = 0.5523 * 20.0;
    let circle = vec![
        anchor(
            next(),
            220.0,
            100.0,
            AnchorKind::Symmetric,
            Vec2::new(0.0, -k),
            Vec2::new(0.0, k),
        ),
        anchor(
            next(),
            200.0,
            120.0,
            AnchorKind::Symmetric,
            Vec2::new(k, 0.0),
            Vec2::new(-k, 0.0),
        ),
        anchor(
            next(),
            180.0,
            100.0,
            AnchorKind::Symmetric,
            Vec2::new(0.0, k),
            Vec2::new(0.0, -k),
        ),
        anchor(
            next(),
            200.0,
            80.0,
            AnchorKind::Symmetric,
            Vec2::new(-k, 0.0),
            Vec2::new(k, 0.0),
        ),
    ];
    d.create_path(&circle, true);
    (d, pts)
}

fn point_near(rng: &mut Rng, pts: &[Point]) -> Point {
    match rng.below(5) {
        0 => Point::new(rng.unit() * 280.0 - 20.0, rng.unit() * 180.0 - 20.0),
        1 => {
            let p = pts[rng.below(pts.len() as u64) as usize];
            Point::new(p.x + rng.unit() * 2.0 - 1.0, p.y + rng.unit() * 2.0 - 1.0)
        }
        2 => {
            let a = pts[rng.below(pts.len() as u64) as usize];
            let b = pts[rng.below(pts.len() as u64) as usize];
            let t = rng.unit();
            Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
        }
        3 => pts[rng.below(pts.len() as u64) as usize],
        _ => {
            let p = pts[rng.below(pts.len() as u64) as usize];
            Point::new(p.x + rng.unit() * 8.0 - 4.0, p.y + rng.unit() * 8.0 - 4.0)
        }
    }
}

fn check_invariants(s: &Session, seed: u64, step: usize) {
    let d = unpack(99, &s.pack("0.1.0").expect("pack")).expect("the file still opens");
    let mut seen: HashSet<AnchorId> = HashSet::new();
    for id in d.object_ids() {
        let Some(p) = d.path(id) else { continue };
        assert!(!p.anchors.is_empty(), "seed {seed} step {step}: empty path");
        if p.closed {
            assert!(
                p.anchors.len() >= 2,
                "seed {seed} step {step}: closed path of {} node(s)",
                p.anchors.len()
            );
        }
        for sp in p.subpaths() {
            for a in sp.anchors {
                assert!(
                    seen.insert(a.id),
                    "seed {seed} step {step}: duplicate anchor id {:?}",
                    a.id
                );
                for v in [
                    a.point.x,
                    a.point.y,
                    a.handle_in.x,
                    a.handle_in.y,
                    a.handle_out.x,
                    a.handle_out.y,
                ] {
                    assert!(v.is_finite(), "seed {seed} step {step}: non-finite {a:?}");
                }
            }
        }
    }
}

#[test]
fn random_event_sequences_keep_the_document_valid() {
    for seed in 1..=50u64 {
        let (d, pts) = seed_document();
        let mut s = Session::open(2, &pack(&d, "0.1.0").unwrap()).unwrap();
        s.resize_viewport(1600.0, 1000.0);
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut down = false;
        let mut shift = false;
        for step in 0..70 {
            match rng.below(16) {
                0 => s.set_tool([Tool::Select, Tool::Node, Tool::Pen][rng.below(3) as usize]),
                1..=3 => {
                    let p = point_near(&mut rng, &pts);
                    s.pointer_hover(p, shift, false);
                    if !down {
                        s.pointer_down(p, shift);
                        down = true;
                    }
                }
                4..=6 => {
                    let p = point_near(&mut rng, &pts);
                    s.pointer_hover(p, shift, false);
                }
                7 | 8 => {
                    let p = point_near(&mut rng, &pts);
                    s.pointer_up(p, shift, false);
                    down = false;
                }
                9 => {
                    s.escape();
                    down = false;
                }
                10 => {
                    shift = !shift;
                    s.modifiers_changed(shift, false, false);
                }
                11 => {
                    let p = point_near(&mut rng, &pts);
                    s.double_click(p, shift, false);
                }
                12 => {
                    s.finish_pen();
                }
                13 => match rng.below(5) {
                    0 => {
                        s.apply_combine();
                    }
                    1 => {
                        s.apply_break_apart();
                    }
                    2 => {
                        s.close_paths(JoinType::Sharp);
                    }
                    3 => {
                        s.close_paths(JoinType::Smooth);
                    }
                    _ => {
                        s.make_line();
                        s.make_curve();
                    }
                },
                14 => {
                    s.pointer_cancelled();
                    down = false;
                }
                _ => {
                    s.pointer_leave();
                }
            }
            if step % 10 == 9 {
                check_invariants(&s, seed, step);
            }
        }
        s.pointer_cancelled();
        s.finish_pen();
        check_invariants(&s, seed, 999);
    }
}

#[test]
fn non_finite_and_huge_coordinates_never_panic_in_any_tool() {
    let (d, _) = seed_document();
    for tool in [Tool::Select, Tool::Node, Tool::Pen] {
        let mut s = Session::open(2, &pack(&d, "0.1.0").unwrap()).unwrap();
        s.resize_viewport(1600.0, 1000.0);
        s.set_tool(tool);
        for p in [
            Point::new(f64::NAN, 5.0),
            Point::new(5.0, f64::INFINITY),
            Point::new(f64::NEG_INFINITY, f64::NAN),
            Point::new(1e300, -1e300),
            Point::new(-0.0, 0.0),
        ] {
            s.pointer_hover(p, false, false);
            s.pointer_down(p, true);
            s.pointer_hover(Point::new(p.x, p.y), true, false);
            s.pointer_up(p, false, false);
            s.double_click(p, false, false);
            s.escape();
        }
        // A bend with a real press and a hostile release.
        s.set_tool(Tool::Node);
        s.pointer_hover(Point::new(20.0, 0.0), false, false);
        s.pointer_down(Point::new(20.0, 0.0), false);
        s.pointer_hover(Point::new(20.0, -50.0), false, false);
        s.pointer_up(Point::new(f64::NAN, f64::NAN), false, false);
        check_invariants(&s, 0, 0);
    }
}
