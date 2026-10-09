//! The golden-file machinery of the boolean kernel's tests, shared by the native test
//! (`boolean_golden.rs`) and the one that also runs in the wasm build (`boolean_golden_wasm.rs`):
//! the fixture format, the rendering of a result as grid integers, and the summary of the
//! 5,000-squares case. No file or clock access, so it compiles for `wasm32-unknown-unknown`.

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::fmt::Write as _;

use super::{Anchors, Operand, area, check_invariants, node_count, rect, run, single};
use curvyo_document_core::{Point, Vec2};
use curvyo_geometry_core::{BooleanError, BooleanOp, BooleanResult};

/// The four operations a fixture records, with their names in the file.
pub const OPS: [(&str, BooleanOp); 4] = [
    ("union", BooleanOp::Union),
    ("difference", BooleanOp::Difference),
    ("intersection", BooleanOp::Intersection),
    ("exclusion", BooleanOp::Exclusion),
];

/// A fixture's text with line endings normalised: git may check the files out with CRLF on
/// Windows, and the comparison is about content.
pub fn normalise(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// The input part of a fixture file: everything before the first `expect` line.
pub fn input_part(text: &str) -> &str {
    text.find("expect ").map_or(text, |at| &text[..at])
}

/// The operands described by the input part of a fixture.
pub fn parse_operands(text: &str) -> Vec<Operand> {
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

/// One result as the golden file writes it.
pub fn render(result: &Result<BooleanResult, BooleanError>) -> String {
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

/// The text a fixture should hold: its input part followed by the result of every operation.
/// A result that breaks the output rules (criteria 24 and 41) panics with the fixture's name.
pub fn expected_text(name: &str, text: &str) -> String {
    let operands = parse_operands(input_part(text));
    let mut expected = String::from(input_part(text));
    for (op_name, op) in OPS {
        let result = run(op, &operands);
        if let Ok(valid) = &result {
            let rules = check_invariants(valid);
            assert!(
                rules.is_ok(),
                "{name} {op_name} breaks the output rules: {rules:?}"
            );
        }
        write!(expected, "expect {op_name}\n{}", render(&result)).unwrap();
    }
    expected
}

/// Criterion 40 (i): one compound operand of 5,000 disjoint tiny squares against a rectangle.
/// The result is summarised (outline count, area, a hash of every coordinate) instead of listed.
pub fn tiny_squares_summary() -> String {
    let squares: Operand = (0..5000)
        .map(|i| {
            let (col, row) = (f64::from(i % 100), f64::from(i / 100));
            rect(col, row, col + 0.5, row + 0.5)
        })
        .collect();
    let bed = single(rect(-1.0, -1.0, 50.0, 25.0));
    let operands = [squares, bed];
    let mut summary = String::from(
        "# 5,000 disjoint squares of 0.5 mm on a 1 mm grid (one compound operand) against a\n# rectangle covering columns 0..49 and rows 0..24 (AC 40i). Regenerate with CURVYO_UPDATE_GOLDEN=1.\n",
    );
    for (name, op) in OPS {
        let result = run(op, &operands);
        if let Ok(valid) = &result {
            assert_eq!(check_invariants(valid), Ok(()), "{name}");
        }
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
    summary
}
