//! The single open project this slice's app state holds: a [`vecmanf_document_core::Document`]
//! plus the path it was last saved to or opened from, if any
//! (`specs/project-file-foundation/specification.md` — no multi-project,
//! multi-window support is in scope here). Also buffers at most one
//! pending open-error message for the frontend to pick up once it has
//! mounted (see `take_pending_open_error`).

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use vecmanf_document_core::Document;

/// Mints a session-local Loro peer id
/// (`specs/project-file-foundation/adrs.md`, "a fresh Loro peer id per
/// open session"). `vecmanf-document-core` cannot draw this itself
/// (`CLAUDE.md` §6: `*-core` crates do no I/O, randomness included), so
/// the host mints it and passes it in.
fn mint_peer_id() -> u64 {
    getrandom::u64().unwrap_or_else(|_| {
        // Falls back to a process-start-derived value if the OS RNG is
        // somehow unavailable. Not a security boundary: within this
        // slice's single-user, non-collaborative session (ADR 0010 §1 is
        // out of scope), a Loro peer id only needs to avoid colliding with
        // a concurrent writer that does not exist yet, so
        // "overwhelmingly likely unique" is enough here.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0u64, |duration| {
                duration.as_secs() ^ u64::from(duration.subsec_nanos())
            });
        seed ^ u64::from(std::process::id())
    })
}

/// The app's one open project.
pub struct ProjectState {
    /// The in-memory document.
    pub document: Document,
    /// Where it was last saved to or opened from, or `None` for a
    /// never-saved new project.
    pub path: Option<PathBuf>,
}

impl ProjectState {
    /// A fresh, never-saved document (File → New, and the app's initial
    /// launch state), with its own freshly minted peer id.
    pub fn new_unsaved() -> Self {
        Self {
            document: Document::new(mint_peer_id()),
            path: None,
        }
    }

    /// A document just read back from `path`.
    pub fn opened(document: Document, path: PathBuf) -> Self {
        Self {
            document,
            path: Some(path),
        }
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

impl Default for ProjectState {
    fn default() -> Self {
        Self::new_unsaved()
    }
}

/// Tauri-managed application state: the one open project, behind a mutex
/// since menu events and commands can both reach it (`tauri::State`), plus
/// at most one pending open-error message.
///
/// The pending-error buffer exists for one reason: when the app is
/// launched by double-clicking a `.vmf` (the OS file-association path),
/// the open attempt runs synchronously in Tauri's `.setup()`, before the
/// frontend's event listeners have attached. A `project-state` update
/// surviving that race has a fallback (`get_project_state`, polled once on
/// mount); `open-error` did not, so a corrupted file opened this way could
/// fail silently — exactly what acceptance criterion 7 forbids. The
/// frontend polls `take_pending_open_error` once on mount, mirroring
/// `get_project_state`.
#[derive(Default)]
pub struct AppState {
    project: Mutex<ProjectState>,
    pending_open_error: Mutex<Option<String>>,
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

    /// Records an open-error message so a not-yet-mounted frontend can
    /// still pick it up via [`Self::take_pending_open_error`]. Overwrites
    /// any previous pending message — only the most recent open attempt
    /// matters.
    ///
    /// # Panics
    /// See [`Self::lock_project`]; the same invariant applies.
    pub fn set_pending_open_error(&self, message: String) {
        #[allow(clippy::unwrap_used)]
        // invariant: nothing in this crate panics while holding the lock.
        {
            *self.pending_open_error.lock().unwrap() = Some(message);
        }
    }

    /// Returns and clears the pending open-error message, if any. Meant
    /// to be called exactly once, at frontend mount time — a later call
    /// (after the live `open-error` event path has taken over) finds
    /// nothing new and returns `None`.
    ///
    /// # Panics
    /// See [`Self::lock_project`]; the same invariant applies.
    pub fn take_pending_open_error(&self) -> Option<String> {
        #[allow(clippy::unwrap_used)]
        // invariant: nothing in this crate panics while holding the lock.
        self.pending_open_error.lock().unwrap().take()
    }
}
