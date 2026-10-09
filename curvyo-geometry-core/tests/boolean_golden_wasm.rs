//! The golden results of the boolean kernel, compared in the build that ships to the browser
//! (`specs/0016-boolean-operations` criterion 43): the same fixtures as `boolean_golden.rs`,
//! embedded with `include_str!` so the test needs no file access, run natively by `cargo test`
//! and in Node on `wasm32-unknown-unknown` by `wasm-bindgen-test-runner`:
//!
//! ```text
//! cargo test --target wasm32-unknown-unknown -p curvyo-geometry-core --test boolean_golden_wasm
//! ```
//!
//! The comparison is exact (grid integers), so a wasm result that differs by one grid step fails.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    missing_docs
)]

mod common;

use common::golden::{expected_text, normalise, tiny_squares_summary};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const FIXTURES: &[(&str, &str)] = &[
    (
        "ac40a_identical_squares.fixture",
        include_str!("fixtures/boolean/ac40a_identical_squares.fixture"),
    ),
    (
        "ac40b_squares_sharing_an_edge.fixture",
        include_str!("fixtures/boolean/ac40b_squares_sharing_an_edge.fixture"),
    ),
    (
        "ac40c_squares_sharing_a_corner.fixture",
        include_str!("fixtures/boolean/ac40c_squares_sharing_a_corner.fixture"),
    ),
    (
        "ac40d_square_inside_sharing_an_edge.fixture",
        include_str!("fixtures/boolean/ac40d_square_inside_sharing_an_edge.fixture"),
    ),
    (
        "ac40e_duplicated_node.fixture",
        include_str!("fixtures/boolean/ac40e_duplicated_node.fixture"),
    ),
    (
        "ac40f_zero_length_segment.fixture",
        include_str!("fixtures/boolean/ac40f_zero_length_segment.fixture"),
    ),
    (
        "ac40g_bow_tie.fixture",
        include_str!("fixtures/boolean/ac40g_bow_tie.fixture"),
    ),
    (
        "ac40h_sliver.fixture",
        include_str!("fixtures/boolean/ac40h_sliver.fixture"),
    ),
    (
        "ac8_ring_and_island.fixture",
        include_str!("fixtures/boolean/ac8_ring_and_island.fixture"),
    ),
];

const TINY_SQUARES: &str = include_str!("fixtures/boolean/ac40i_tiny_squares.summary");

/// Every fixture gives its golden result, byte for byte.
#[test]
fn every_fixture_matches_its_golden_result() {
    assert!(FIXTURES.len() >= 9, "fixtures are missing");
    for (name, text) in FIXTURES {
        let text = normalise(text);
        assert_eq!(
            expected_text(name, &text),
            text,
            "{name} differs from the golden file"
        );
    }
}

/// The 5,000-squares case gives its summary, hash included.
#[test]
fn five_thousand_tiny_squares() {
    assert_eq!(tiny_squares_summary(), normalise(TINY_SQUARES));
}

/// The list above is written by hand because the wasm build cannot read a directory: this test
/// (native only) fails if a fixture file is missing from it, or listed but gone.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn the_fixture_list_matches_the_fixture_directory() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/boolean");
    let mut on_disk: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".fixture"))
        .collect();
    on_disk.sort();
    let mut listed: Vec<String> = FIXTURES
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect();
    listed.sort();
    assert_eq!(listed, on_disk);
    assert!(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/boolean/ac40i_tiny_squares.summary")
            .exists()
    );
}
