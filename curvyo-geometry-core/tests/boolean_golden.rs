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

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::golden::{expected_text, input_part, normalise, parse_operands, tiny_squares_summary};
use common::{area, node_count, run};
use curvyo_geometry_core::{BooleanError, BooleanOp, BooleanResult};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/boolean")
}

fn read_fixture(path: &Path) -> String {
    normalise(&std::fs::read_to_string(path).unwrap())
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

/// Criteria 40 and 43: every fixture completes in under 2 s per operation without panic and
/// gives the golden result or refusal, exactly. The same comparison runs in the wasm build
/// (`boolean_golden_wasm.rs`).
#[test]
fn every_fixture_matches_its_golden_result() {
    let files = fixtures();
    assert!(files.len() >= 9, "fixtures are missing");
    for file in files {
        let text = read_fixture(&file);
        let started = Instant::now();
        let expected = expected_text(&file.display().to_string(), &text);
        assert!(
            started.elapsed() < Duration::from_secs(2) * 4,
            "{} took {:?}",
            file.display(),
            started.elapsed()
        );
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

/// Criterion 40 (i): the summary of 5,000 disjoint tiny squares against a rectangle.
#[test]
fn five_thousand_tiny_squares() {
    let started = Instant::now();
    let summary = tiny_squares_summary();
    assert!(
        started.elapsed() < Duration::from_secs(2) * 4,
        "{:?}",
        started.elapsed()
    );
    let path = fixture_dir().join("ac40i_tiny_squares.summary");
    if update_requested() {
        std::fs::write(&path, &summary).unwrap();
    }
    assert_eq!(summary, read_fixture(&path));
}
