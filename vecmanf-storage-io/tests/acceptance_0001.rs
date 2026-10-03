//! Black-box tests for the filesystem primitives
//! (`specs/project-file-foundation/specification.md`) that AC3/AC4/AC5/AC6
//! depend on at the host layer: a "Save As" must land bytes at the exact
//! chosen path, and a repeated "Save" must complete without corrupting
//! what is already there. Written against the public API only
//! (`write_atomic`, `read_to_vec`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vecmanf_storage_io::{read_to_vec, write_atomic};

/// AC3: "Save As" writes a file at exactly the chosen path, readable back
/// byte-for-byte.
#[test]
fn save_as_writes_the_file_at_the_exact_chosen_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("My Project.vmf");

    write_atomic(&path, b"vmf container bytes").expect("write");

    assert!(path.exists());
    assert_eq!(read_to_vec(&path).expect("read"), b"vmf container bytes");
}

/// AC4: "Save" again (no further changes) completes without error and
/// without disturbing the file's identity at that path — modelled here as
/// writing the same bytes to the same path a second time.
#[test]
fn saving_again_with_unchanged_bytes_completes_without_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("project.vmf");

    write_atomic(&path, b"same bytes").expect("first save");
    write_atomic(&path, b"same bytes").expect("second save (no changes) must not error");

    assert_eq!(read_to_vec(&path).expect("read"), b"same bytes");
}

/// AC5/AC6 underpinning: a file written by one `write_atomic` call is
/// fully and exactly readable by a later, independent `read_to_vec` call
/// — the minimum the "close, reopen" and "restart the process" claims
/// need to hold at the byte level.
#[test]
fn a_saved_file_round_trips_exactly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("roundtrip.vmf");
    let payload: Vec<u8> = (0..10_000u32).flat_map(u32::to_le_bytes).collect();

    write_atomic(&path, &payload).expect("write");
    let read_back = read_to_vec(&path).expect("read");

    assert_eq!(read_back, payload);
}
