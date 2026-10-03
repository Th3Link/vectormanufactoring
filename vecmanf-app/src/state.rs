//! The single open project this slice's app state holds: a [`vecmanf_document_core::Document`]
//! plus the path it was last saved to or opened from, if any
//! (`specs/project-file-foundation/specification.md` — no multi-project,
//! multi-window support is in scope here).

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use vecmanf_document_core::Document;

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
    /// launch state).
    pub fn new_unsaved() -> Self {
        Self {
            document: Document::new(),
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
/// since menu events and commands can both reach it (`tauri::State`).
#[derive(Default)]
pub struct AppState(Mutex<ProjectState>);

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
        self.0.lock().unwrap()
    }
}
