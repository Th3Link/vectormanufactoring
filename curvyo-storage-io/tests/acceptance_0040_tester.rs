//! Independent tester case for `0040-document-background` criterion 4 on disk: opening a file
//! written by any earlier build leaves the file byte-for-byte and mtime unchanged, also when the
//! file and its directory are read-only (a write attempt would fail loudly). Written from
//! `specification.md` before the implementation was read.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::fs;
use std::path::Path;

use curvyo_document_core::{BackgroundPaint, DocumentBackground, unpack};

const OLDER: &[&str] = &[
    "format_version_1.curvyo",
    "paths_v2.curvyo",
    "primitives_v3.curvyo",
    "legacy_corner_radius_v5.curvyo",
    "rotation_v5.curvyo",
    "legacy_gradient_v7.curvyo",
    "display_unit_in_v7.curvyo",
    "compound_v8.curvyo",
    "markers_v9.curvyo",
    "dash_v9.curvyo",
    "corner_radii_per_corner.curvyo",
    "valid.curvyo",
];

fn source(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../curvyo-document-core/tests/fixtures")
        .join(name)
}

#[test]
fn opening_an_older_project_leaves_the_file_byte_for_byte_and_mtime_unchanged() {
    for name in OLDER {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::copy(source(name), &path).unwrap();
        let before = fs::read(&path).unwrap();
        let mtime = fs::metadata(&path).unwrap().modified().unwrap();

        let bytes = curvyo_storage_io::read_to_vec(&path).unwrap();
        let document = unpack(1, &bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(document.background(), DocumentBackground::DEFAULT, "{name}");
        assert_eq!(
            document.background().paint,
            BackgroundPaint::Solid,
            "{name}"
        );
        drop(document);

        assert_eq!(fs::read(&path).unwrap(), before, "{name}: bytes changed");
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            mtime,
            "{name}"
        );
        let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(
            entries.len(),
            1,
            "{name}: opening left another file next to it"
        );
    }
}

#[cfg(unix)]
#[test]
fn opening_works_from_a_read_only_file_in_a_read_only_directory() {
    use std::os::unix::fs::PermissionsExt;
    for name in OLDER {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::copy(source(name), &path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();

        let bytes = curvyo_storage_io::read_to_vec(&path).unwrap();
        let opened = unpack(1, &bytes);

        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(opened.is_ok(), "{name}");
    }
}

#[test]
fn a_damaged_background_is_refused_and_the_file_is_not_touched() {
    for name in [
        "background_paint_unknown_v10.curvyo",
        "background_color_three_v10.curvyo",
        "background_color_range_v10.curvyo",
        "background_color_type_v10.curvyo",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::copy(source(name), &path).unwrap();
        let before = fs::read(&path).unwrap();
        let bytes = curvyo_storage_io::read_to_vec(&path).unwrap();
        assert!(unpack(1, &bytes).is_err(), "{name}");
        assert_eq!(fs::read(&path).unwrap(), before, "{name}");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1, "{name}");
    }
}
