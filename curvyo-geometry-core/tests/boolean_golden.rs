//! Golden-file tests of the boolean kernel (`specs/0016-boolean-operations` criteria 40 and 43).
//!
//! Each file in `tests/fixtures/boolean/` holds the operands of one case and the expected result
//! of all four operations, as integers on the 0.001 mm grid, so the comparison is exact and
//! agrees on every platform. The inputs are synthetic: the project has no customer files with
//! degenerate outlines yet; when it has, they join the directory as further `.fixture` files.
//!
//! To regenerate the expected results after an intended change, run the tests with
//! `CURVYO_UPDATE_GOLDEN=1` and review the diff.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    missing_docs
)]

mod common;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{Anchors, Operand, area, node_count, run};
use curvyo_document_core::{Point, Vec2};
use curvyo_geometry_core::{BooleanError, BooleanOp, BooleanResult};

const OPS: [(&str, BooleanOp); 4] = [
    ("union", BooleanOp::Union),
    ("difference", BooleanOp::Difference),
    ("intersection", BooleanOp::Intersection),
    ("exclusion", BooleanOp::Exclusion),
];

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/boolean")
}

/// A fixture's text with line endings normalised: git may check the files out with CRLF on
/// Windows, and the comparison is about content.
fn read_fixture(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

fn parse_operands(text: &str) -> Vec<Operand> {
    let mut operands: Vec<Operand> = Vec::new();
    for line in text.lines().map(str::trim) {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("operand") => operands.push(Vec::new()),
            Some("outline") => {
                assert_eq!(
                    words.next(),
                    Some("closed"),
                    "only closed outlines in fixtures"
                );
                operands.last_mut().unwrap().push(Anchors::new());
            }
            Some("anchor") => {
                let n: Vec<f64> = words.map(|w| w.parse().unwrap()).collect();
                let handles = if n.len() == 6 { &n[2..] } else { &[0.0; 4] };
                let anchor = (
                    Point::new(n[0], n[1]),
                    Vec2::new(handles[0], handles[1]),
                    Vec2::new(handles[2], handles[3]),
                );
                operands
                    .last_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()
                    .push(anchor);
            }
            _ => {}
        }
    }
    operands
}

/// Grid integers of one outline: `x y x y ...`.
fn render_outline(outline: &[Point]) -> String {
    let mut text = String::new();
    for (index, p) in outline.iter().enumerate() {
        if index > 0 {
            text.push(' ');
        }
        write!(
            text,
            "{} {}",
            (p.x * 1000.0).round(),
            (p.y * 1000.0).round()
        )
        .unwrap();
    }
    text
}

fn render(result: &Result<BooleanResult, BooleanError>) -> String {
    match result {
        Ok(result) => {
            let mut text = String::new();
            for outline in result.outlines() {
                writeln!(text, "outline {}", render_outline(outline)).unwrap();
            }
            text
        }
        Err(error) => format!("refused {error:?}\n"),
    }
}

fn update_requested() -> bool {
    std::env::var_os("CURVYO_UPDATE_GOLDEN").is_some()
}

fn fixtures() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(fixture_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "fixture"))
        .collect();
    files.sort();
    files
}

/// The input part of a fixture file: everything before the first `expect` line.
fn input_part(text: &str) -> &str {
    text.find("expect ").map_or(text, |at| &text[..at])
}

/// Criteria 40 and 43: every fixture completes in under 2 s without panic and gives the golden
/// result or refusal, exactly.
#[test]
fn every_fixture_matches_its_golden_result() {
    let files = fixtures();
    assert!(files.len() >= 9, "fixtures are missing");
    for file in files {
        let text = read_fixture(&file);
        let operands = parse_operands(input_part(&text));
        let mut expected = String::from(input_part(&text));
        for (name, op) in OPS {
            let started = Instant::now();
            let result = run(op, &operands);
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "{} {name} took {:?}",
                file.display(),
                started.elapsed()
            );
            write!(expected, "expect {name}\n{}", render(&result)).unwrap();
        }
        if update_requested() {
            std::fs::write(&file, &expected).unwrap();
        }
        assert_eq!(
            expected,
            read_fixture(&file),
            "{} differs from the golden file",
            file.display()
        );
    }
}

fn result_of(file: &str, op: BooleanOp) -> Result<BooleanResult, BooleanError> {
    let text = read_fixture(&fixture_dir().join(file));
    run(op, &parse_operands(input_part(&text)))
}

/// Criterion 40, expectations for (a): union and intersection give the same square of 4 nodes,
/// difference and exclusion are refused as empty.
#[test]
fn identical_squares() {
    let file = "ac40a_identical_squares.fixture";
    let union = result_of(file, BooleanOp::Union).unwrap();
    assert_eq!(node_count(&union), 4);
    assert_eq!(result_of(file, BooleanOp::Intersection).unwrap(), union);
    for op in [BooleanOp::Difference, BooleanOp::Exclusion] {
        assert_eq!(result_of(file, op), Err(BooleanError::EmptyResult));
    }
}

/// Criterion 40, expectations for (c): the union is one compound path of two outlines, the
/// intersection is refused as empty.
#[test]
fn squares_sharing_only_a_corner() {
    let file = "ac40c_squares_sharing_a_corner.fixture";
    let union = result_of(file, BooleanOp::Union).unwrap();
    assert!(
        union.outlines().len() == 2 || (union.outlines().len() == 1 && node_count(&union) == 8),
        "{union:?}"
    );
    assert!((area(&union) - 800.0).abs() < 1e-9);
    assert_eq!(
        result_of(file, BooleanOp::Intersection),
        Err(BooleanError::EmptyResult)
    );
}

/// Criterion 40 (i): one compound operand of 5,000 disjoint tiny squares against a rectangle.
/// The result is summarised (outline count, area, a hash of every coordinate) instead of listed.
#[test]
fn five_thousand_tiny_squares() {
    let squares: Operand = (0..5000)
        .map(|i| {
            let (col, row) = (f64::from(i % 100), f64::from(i / 100));
            common::rect(col, row, col + 0.5, row + 0.5)
        })
        .collect();
    let bed = common::single(common::rect(-1.0, -1.0, 50.0, 25.0));
    let operands = [squares, bed];
    let mut summary = String::from(
        "# 5,000 disjoint squares of 0.5 mm on a 1 mm grid (one compound operand) against a\n# rectangle covering columns 0..49 and rows 0..24 (AC 40i). Regenerate with CURVYO_UPDATE_GOLDEN=1.\n",
    );
    for (name, op) in OPS {
        let started = Instant::now();
        let result = run(op, &operands);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{name} {:?}",
            started.elapsed()
        );
        let hash = result.as_ref().map_or(0, |r| {
            r.outlines()
                .iter()
                .flatten()
                .fold(0xcbf2_9ce4_8422_2325_u64, |h, p| {
                    let mix = |h: u64, v: f64| {
                        (h ^ ((v * 1000.0).round() as i64 as u64))
                            .wrapping_mul(0x0000_0100_0000_01b3)
                    };
                    mix(mix(h, p.x), p.y)
                })
        });
        match &result {
            Ok(r) => writeln!(
                summary,
                "{name}: outlines {} nodes {} area_mm2 {:.3} hash {hash:016x}",
                r.outlines().len(),
                node_count(r),
                area(r)
            ),
            Err(error) => writeln!(summary, "{name}: refused {error:?}"),
        }
        .unwrap();
    }
    let path = fixture_dir().join("ac40i_tiny_squares.summary");
    if update_requested() {
        std::fs::write(&path, &summary).unwrap();
    }
    assert_eq!(summary, read_fixture(&path));
}
