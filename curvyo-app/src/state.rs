//! The single open project this slice's app state holds: just the path it
//! was last saved to or opened from, if any
//! (`specs/0001-project-file-foundation/specification.md` — no multi-project,
//! multi-window support is in scope here). No document lives here any
//! more (`specs/0002-path-node-editing/adrs.md`'s PR review: "the host does
//! byte I/O only") — the real document is the `WasmSession` the frontend
//! owns; this crate reads and writes `.curvyo` bytes on its behalf and
//! otherwise never looks inside them. Also buffers at most one pending
//! open payload for the frontend to pick up once it has mounted (see
//! `take_pending_open`).

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

/// The app's one open project: nothing but where it lives on disk.
#[derive(Default)]
pub struct ProjectState {
    /// Where it was last saved to or opened from, or `None` for a
    /// never-saved new project.
    pub path: Option<PathBuf>,
}

impl ProjectState {
    /// A fresh, never-saved project (File → New, and the app's initial
    /// launch state).
    pub fn new_unsaved() -> Self {
        Self { path: None }
    }

    /// A project just read back from `path` — recorded only once the
    /// frontend confirms the bytes actually parsed (`main.rs`'s
    /// `confirm_project_opened`); a refusal never reaches this
    /// constructor, so an already-open project is never touched by one
    /// (acceptance criterion 7).
    pub fn opened(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    /// The file name (with extension) the title bar should show, if this
    /// project has ever been saved or opened.
    pub fn file_name(&self) -> Option<String> {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
    }
}

/// One pending "open" result, buffered for a not-yet-mounted frontend to
/// pick up (see [`AppState::take_pending_open`]'s own doc comment for
/// why this race exists at all).
pub enum PendingOpen {
    /// The file at `path` was read successfully; here are its raw bytes
    /// for the frontend to hand to `WasmSession::open` itself — this host
    /// never parses them (`specs/0002-path-node-editing/adrs.md`'s PR review).
    Bytes {
        /// The path these bytes were read from.
        path: PathBuf,
        /// The file's raw contents.
        bytes: Vec<u8>,
    },
    /// The path could not even be read (missing, permissions, not a
    /// file) — a host-level I/O failure, distinct from a `.curvyo` whose
    /// *content* is invalid (that refusal is the frontend's own, once it
    /// tries `WasmSession::open` on the bytes above).
    IoError(String),
}

/// Tauri-managed application state: the one open project, behind a mutex
/// since menu events and commands can both reach it (`tauri::State`), plus
/// at most one pending open payload.
///
/// The pending buffer exists for one reason: when the app is launched by
/// double-clicking a `.curvyo` (the OS file-association path), the open
/// attempt runs synchronously in Tauri's `.setup()`, before the
/// frontend's event listeners have attached. A live "open-bytes"/"open-error"
/// event does not survive that race, so a file opened this way could be
/// silently dropped — exactly what acceptance criterion 7 forbids. The
/// frontend polls `take_pending_open` once on mount.
#[derive(Default)]
pub struct AppState {
    project: Mutex<ProjectState>,
    pending_open: Mutex<Option<PendingOpen>>,
}

impl AppState {
    /// Locks the project state.
    ///
    /// # Panics
    /// Propagates a poisoned-lock panic if a previous holder panicked
    /// while holding the lock. Nothing in this crate panics while holding
    /// it, so this should not occur in practice.
    pub fn lock_project(&self) -> MutexGuard<'_, ProjectState> {
        #[allow(clippy::unwrap_used)]
        // invariant: nothing in this crate panics while holding the lock.
        self.project.lock().unwrap()
    }

    /// Records a pending open payload so a not-yet-mounted frontend can
    /// still pick it up via [`Self::take_pending_open`]. Overwrites any
    /// previous pending value — only the most recent open attempt
    /// matters.
    ///
    /// # Panics
    /// See [`Self::lock_project`]; the same invariant applies.
    pub fn set_pending_open(&self, pending: PendingOpen) {
        #[allow(clippy::unwrap_used)]
        // invariant: nothing in this crate panics while holding the lock.
        {
            *self.pending_open.lock().unwrap() = Some(pending);
        }
    }

    /// Returns and clears the pending open payload, if any. Meant to be
    /// called exactly once, at frontend mount time — a later call (after
    /// the live "open-bytes"/"open-error" event path has taken over)
    /// finds nothing new and returns `None`.
    ///
    /// # Panics
    /// See [`Self::lock_project`]; the same invariant applies.
    pub fn take_pending_open(&self) -> Option<PendingOpen> {
        #[allow(clippy::unwrap_used)]
        // invariant: nothing in this crate panics while holding the lock.
        self.pending_open.lock().unwrap().take()
    }
}
