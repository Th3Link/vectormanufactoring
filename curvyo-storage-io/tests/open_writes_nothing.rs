//! Opening a project file writes nothing to it
//! (`specs/0040-document-background` criterion 4): every older fixture is read from
//! disk and opened with the default background, and its bytes on disk are the same
//! afterwards.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use curvyo_document_core::{DocumentBackground, unpack};
use curvyo_storage_io::{read_to_vec, write_atomic};

const OLDER_FIXTURES: [&str; 9] = [
    "format_version_1.curvyo",
    "paths_v2.curvyo",
    "primitives_v3.curvyo",
    "rotation_v5.curvyo",
    "legacy_gradient_v7.curvyo",
    "compound_v8.curvyo",
    "dash_v9.curvyo",
    "markers_v9.curvyo",
    "display_unit_in_v7.curvyo",
];

#[test]
fn opening_an_older_file_shows_the_default_background_and_leaves_the_file_alone() {
    let fixtures =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../curvyo-document-core/tests/fixtures");
    let dir = tempfile::tempdir().unwrap();
    for name in OLDER_FIXTURES {
        let original = read_to_vec(&fixtures.join(name)).unwrap();
        let path = dir.path().join(name);
        write_atomic(&path, &original).unwrap();

        let document = unpack(2, &read_to_vec(&path).unwrap())
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));

        assert_eq!(document.background(), DocumentBackground::DEFAULT, "{name}");
        assert_eq!(
            read_to_vec(&path).unwrap(),
            original,
            "{name}: bytes changed on disk"
        );
    }
}
