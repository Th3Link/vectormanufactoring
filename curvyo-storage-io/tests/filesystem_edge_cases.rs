//! White-box edge cases for `src/filesystem.rs`, written after reading the
//! implementation: a missing parent directory, reading a path that is a
//! directory rather than a file, and a zero-byte payload. None of these
//! are covered by the inline unit tests (`write_then_read_round_trips`,
//! `write_atomic_replaces_existing_file_fully`,
//! `write_atomic_leaves_no_temp_file_behind`, `read_missing_file_is_an_error`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use curvyo_storage_io::{FsError, read_to_vec, write_atomic};

/// Writing to a path whose parent directory does not exist must return a
/// clean error, not panic — `write_atomic` creates no directories itself
/// (ADR 0004 §7 only asks for atomic-write-then-rename, not directory
/// creation).
#[test]
fn write_atomic_to_a_missing_parent_directory_errors_cleanly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("does-not-exist-yet").join("project.curvyo");

    let result = write_atomic(&path, b"bytes");

    assert!(matches!(result, Err(FsError::Write { .. })));
}

/// Reading a path that is a directory, not a file, must return a clean
/// error, not panic.
#[test]
fn read_to_vec_of_a_directory_errors_cleanly() {
    let dir = tempfile::tempdir().expect("tempdir");

    let result = read_to_vec(dir.path());

    assert!(matches!(result, Err(FsError::Read { .. })));
}

/// A zero-byte write must round-trip to a zero-byte read, not be confused
/// with "no file" or trigger a special case in the atomic-rename path.
#[test]
fn zero_byte_payload_round_trips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("empty.curvyo");

    write_atomic(&path, b"").expect("write empty bytes");
    let read_back = read_to_vec(&path).expect("read");

    assert_eq!(read_back.len(), 0);
}

/// `write_atomic` must not leave its temporary sibling file behind even
/// when the destination path has no existing file to replace (the
/// "brand-new Save As" case, as opposed to the inline unit test's
/// "overwrite an existing file" case).
#[test]
fn write_atomic_leaves_no_temp_file_on_a_brand_new_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("new-project.curvyo");

    write_atomic(&path, b"first ever save").expect("write");

    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read_dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .filter(|name| name.to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
}
