//! Atomic file writes and plain reads — the one module this slice needs
//! from `curvyo-storage-io` (ADR 0011 §2; ADR 0004 §7: "atomic
//! write-temp-then-rename ... and no lock files").
//!
//! This module is impure by design (`-io`, not `-core`): it is the thing
//! `curvyo-document-core` is deliberately not allowed to be.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// A filesystem operation failed.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    /// Reading the file failed.
    #[error("failed to read {path}: {source}")]
    Read {
        /// The path that could not be read.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Writing the file failed.
    #[error("failed to write {path}: {source}")]
    Write {
        /// The path that could not be written.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// Reads an entire file into memory.
///
/// # Errors
/// Returns [`FsError::Read`] if the file cannot be opened or read.
pub fn read_to_vec(path: &Path) -> Result<Vec<u8>, FsError> {
    fs::read(path).map_err(|source| FsError::Read {
        path: path.display().to_string(),
        source,
    })
}

/// Writes `bytes` to `path` atomically: write to a temporary file in the
/// same directory, then rename over the destination. A reader never
/// observes a partially written file, and a crash mid-write leaves the
/// original file (if any) untouched (ADR 0004 §7).
///
/// No lock file is created, so this is safe to point at a folder-synced
/// directory (Nextcloud, Syncthing) without leaving a lock behind that a
/// sync tool would fight over.
///
/// # Errors
/// Returns [`FsError::Write`] if the temporary file cannot be written or
/// the rename cannot be completed. On a failed rename, the temporary file
/// is removed on a best-effort basis.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), FsError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp_path = temporary_sibling_path(dir, path);

    fs::write(&tmp_path, bytes).map_err(|source| FsError::Write {
        path: tmp_path.display().to_string(),
        source,
    })?;

    if let Err(source) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(FsError::Write {
            path: path.display().to_string(),
            source,
        });
    }

    Ok(())
}

fn temporary_sibling_path(dir: &Path, final_path: &Path) -> PathBuf {
    let file_name = final_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("curvyo-project");
    dir.join(format!("{file_name}.{}.tmp", unique_suffix()))
}

fn unique_suffix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{pid}-{nanos}-{sequence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("project.curvyo");

        write_atomic(&path, b"hello").expect("write");
        let read_back = read_to_vec(&path).expect("read");

        assert_eq!(read_back, b"hello");
    }

    #[test]
    fn write_atomic_replaces_existing_file_fully() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("project.curvyo");

        write_atomic(&path, b"first version, longer").expect("first write");
        write_atomic(&path, b"second").expect("second write");

        let read_back = read_to_vec(&path).expect("read");
        assert_eq!(read_back, b"second");
    }

    #[test]
    fn write_atomic_leaves_no_temp_file_behind() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("project.curvyo");

        write_atomic(&path, b"hello").expect("write");

        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .filter(|name| name.to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
    }

    #[test]
    fn read_missing_file_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("does-not-exist.curvyo");

        assert!(read_to_vec(&path).is_err());
    }
}
